//! Drains the durable write outbox: claims due rows, executes each kind's ESI mutation, and on a
//! permanent failure runs the kind's compensation to undo the optimistic local mirror. Transient
//! failures reschedule with backoff; an ESI error-limit is surfaced so the engine pauses all dispatch.

use chrono::{Duration as ChronoDuration, Utc};
use rand::RngExt;

use super::{
  event::Event,
  outbox::{KindHandler, Registry},
  token,
};
use crate::{
  clients::{self, Error, esi, eve_sso},
  store::{Database, model::Outbox, repo::infra},
};

const BACKOFF_BASE_SECS: i64 = 2;
const BACKOFF_CAP_SECS: i64 = 300;
const BACKOFF_EXPONENT_CAP: u32 = 8;
const DRAIN_BATCH: i64 = 16;
const MAX_ATTEMPTS: i64 = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct DrainOutcome {
  pub(super) error_limit_reset_secs: Option<u64>,
}

enum Delay {
  Curve,
  Fixed(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
  Done,
  Failed,
  Rescheduled { error_limit_reset_secs: Option<u64> },
  Skipped,
}

enum Transience {
  Permanent,
  Transient {
    delay: Delay,
    error_limit_reset_secs: Option<u64>,
  },
}

pub(super) async fn drain(
  db: &Database,
  esi: &esi::Client,
  sso: &eve_sso::Client,
  registry: &Registry,
  emit: &impl Fn(Event),
) -> Result<DrainOutcome, clients::Error> {
  let now = Utc::now().to_rfc3339();
  let claimed = infra::claim_due(db, &now, DRAIN_BATCH).await?;
  let mut error_limit_reset_secs = None;
  for row in claimed {
    emit(Event::OutboxInflight {
      id: row.id(),
    });
    let outcome = drain_row(db, esi, sso, registry, &row, emit).await;
    if let Outcome::Rescheduled {
      error_limit_reset_secs: Some(secs),
    } = outcome
    {
      error_limit_reset_secs = Some(error_limit_reset_secs.map_or(secs, |seen: u64| seen.max(secs)));
    }
    tracing::debug!(id = row.id(), kind = row.kind(), ?outcome, "outbox row drained");
  }
  Ok(DrainOutcome {
    error_limit_reset_secs,
  })
}

async fn drain_row(
  db: &Database,
  esi: &esi::Client,
  sso: &eve_sso::Client,
  registry: &Registry,
  row: &Outbox,
  emit: &impl Fn(Event),
) -> Outcome {
  let handler = match registry.resolve(row.kind()) {
    Ok(handler) => handler,
    Err(error) => {
      tracing::warn!(id = row.id(), kind = row.kind(), %error, "no handler for outbox kind; failing row");
      return terminalize(db, row, None, &error.to_string(), emit).await;
    }
  };

  let grant = match token::fresh_token(db, sso, row.subject_id(), row.subject_type()).await {
    Ok(Some(grant)) => grant,
    Ok(None) => {
      tracing::debug!(id = row.id(), "skipping outbox row: no credential for subject");
      return Outcome::Skipped;
    }
    Err(error) => {
      tracing::warn!(id = row.id(), %error, "outbox row authentication failed; rescheduling");
      return classify(db, row, handler, &error, emit).await;
    }
  };

  match handler.execute(db, esi, &grant, row.payload()).await {
    Ok(()) => match infra::mark_done(db, row.id()).await {
      Ok(()) => {
        emit(Event::OutboxDone {
          id: row.id(),
        });
        Outcome::Done
      }
      Err(error) => {
        tracing::error!(id = row.id(), %error, "failed to mark drained outbox row done");
        Outcome::Skipped
      }
    },
    Err(error) => classify(db, row, handler, &error, emit).await,
  }
}

async fn classify(
  db: &Database,
  row: &Outbox,
  handler: &dyn KindHandler,
  error: &Error,
  emit: &impl Fn(Event),
) -> Outcome {
  let Transience::Transient {
    delay,
    error_limit_reset_secs,
  } = transience(error)
  else {
    return terminalize(db, row, Some(handler), &error.to_string(), emit).await;
  };

  if row.attempts() >= MAX_ATTEMPTS {
    tracing::warn!(id = row.id(), attempts = row.attempts(), %error, "outbox row exhausted retries; failing");
    return terminalize(db, row, Some(handler), &error.to_string(), emit).await;
  }

  let delay_secs = match delay {
    Delay::Fixed(secs) => secs,
    Delay::Curve => backoff_secs(row.attempts()),
  };
  let next_attempt_at = (Utc::now() + ChronoDuration::seconds(delay_secs)).to_rfc3339();
  match infra::reschedule(db, row.id(), &next_attempt_at, &error.to_string()).await {
    Ok(()) => {
      tracing::warn!(id = row.id(), delay_secs, %error, "outbox ESI write transiently failed; rescheduled");
      emit(Event::OutboxRetrying {
        id: row.id(),
        retry_secs: u64::try_from(delay_secs).unwrap_or(0),
      });
      Outcome::Rescheduled {
        error_limit_reset_secs,
      }
    }
    Err(store_error) => {
      tracing::error!(id = row.id(), %store_error, "failed to reschedule transient outbox row");
      Outcome::Skipped
    }
  }
}

async fn terminalize(
  db: &Database,
  row: &Outbox,
  handler: Option<&dyn KindHandler>,
  error: &str,
  emit: &impl Fn(Event),
) -> Outcome {
  match mark_failed_if_inflight(db, row.id(), error).await {
    Ok(true) => {
      emit(Event::OutboxFailed {
        id: row.id(),
        reason: error.to_string(),
      });
      if let Some(handler) = handler
        && let Err(compensate_error) = handler.compensate(db, row.payload()).await
      {
        tracing::error!(id = row.id(), %compensate_error, "outbox compensation failed; mirror heals on next read-sync");
      }
      Outcome::Failed
    }
    Ok(false) => {
      tracing::debug!(
        id = row.id(),
        "outbox row no longer inflight at terminalize; skipping compensation"
      );
      Outcome::Skipped
    }
    Err(store_error) => {
      tracing::error!(id = row.id(), %store_error, "failed to mark outbox row failed");
      Outcome::Skipped
    }
  }
}

async fn mark_failed_if_inflight(db: &Database, id: i64, error: &str) -> Result<bool, clients::Error> {
  let now = Utc::now().to_rfc3339();
  let affected = sqlx::query(
    "UPDATE outbox SET status = 'failed', last_error = ?, updated_at = ? WHERE id = ? AND status = 'inflight'",
  )
  .bind(error)
  .bind(&now)
  .bind(id)
  .execute(db.writer())
  .await
  .map_err(crate::store::Error::from)?
  .rows_affected();
  Ok(affected == 1)
}

fn transience(error: &Error) -> Transience {
  match error {
    Error::ErrorLimited {
      reset_secs,
    } => Transience::Transient {
      delay: Delay::Fixed(i64::try_from(*reset_secs).unwrap_or(i64::MAX).max(1)),
      error_limit_reset_secs: Some(*reset_secs),
    },
    Error::RateLimit {
      retry_after_secs,
    } => Transience::Transient {
      delay: Delay::Fixed(i64::try_from(*retry_after_secs).unwrap_or(i64::MAX).max(1)),
      error_limit_reset_secs: None,
    },
    Error::Http(http) => match http.status().map(|s| s.as_u16()) {
      Some(status) if !(500..600).contains(&status) => Transience::Permanent,
      _ => Transience::Transient {
        delay: Delay::Curve,
        error_limit_reset_secs: None,
      },
    },
    Error::Auth(_) | Error::NotReady => Transience::Transient {
      delay: Delay::Curve,
      error_limit_reset_secs: None,
    },
    Error::Db(_) | Error::Json(_) | Error::Internal(_) => Transience::Permanent,
  }
}

fn backoff_secs(attempts: i64) -> i64 {
  let exponent = u32::try_from(attempts)
    .unwrap_or(BACKOFF_EXPONENT_CAP)
    .min(BACKOFF_EXPONENT_CAP);
  let base = BACKOFF_BASE_SECS.saturating_mul(1i64 << exponent).min(BACKOFF_CAP_SECS);
  let jitter = rand::rng().random_range(0..=BACKOFF_BASE_SECS);
  base.saturating_add(jitter)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    clients::http,
    store::{self, model::OwnerType, repo::infra},
    sync::outbox::{OutboxKind, test_support::*},
  };

  fn esi_client(db: &Database) -> esi::Client {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    esi::Client::with_base_url(http, "http://localhost")
  }

  fn sso_client(db: &Database) -> eve_sso::Client {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    eve_sso::Client::new(http, "test-client")
  }

  async fn append_send(db: &Database, subject_id: i64) -> Outbox {
    infra::append(
      db,
      OwnerType::Character,
      subject_id,
      "mail.send",
      "{\"body\":\"hi\"}",
      None,
    )
    .await
    .unwrap()
  }

  async fn status_of(db: &Database, id: i64) -> String {
    sqlx::query_scalar::<_, String>("SELECT status FROM outbox WHERE id = ?")
      .bind(id)
      .fetch_one(&db.0)
      .await
      .unwrap()
  }

  async fn reload(db: &Database, id: i64) -> Outbox {
    sqlx::query_as::<_, Outbox>(
      "SELECT attempts, created_at, dedupe_key, id, kind, last_error, next_attempt_at, payload, status, \
      subject_id, subject_type, updated_at FROM outbox WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&db.0)
    .await
    .unwrap()
  }

  async fn http_error(status: u16) -> Error {
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };
    let server = MockServer::start().await;
    Mock::given(method("GET"))
      .and(path("/boom"))
      .respond_with(ResponseTemplate::new(status))
      .mount(&server)
      .await;
    let resp = reqwest::get(format!("{}/boom", server.uri())).await.unwrap();
    Error::Http(resp.error_for_status().unwrap_err())
  }

  fn failing_registry(make: impl Fn() -> Error + Send + Sync + 'static) -> Registry {
    Registry::new().with(Box::new(
      StubHandler::new(OutboxKind::MailSend, StubCalls::default()).failing_execute(make),
    ))
  }

  fn one_shot_failing_registry(error: Error) -> Registry {
    let slot = std::sync::Mutex::new(Some(error));
    Registry::new().with(Box::new(
      StubHandler::new(OutboxKind::MailSend, StubCalls::default())
        .failing_execute(move || slot.lock().unwrap().take().expect("execute called more than once")),
    ))
  }

  fn observed_one_shot_failing_registry(calls: StubCalls, error: Error) -> Registry {
    let slot = std::sync::Mutex::new(Some(error));
    Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, calls).failing_execute(
      move || slot.lock().unwrap().take().expect("execute called more than once"),
    )))
  }

  async fn credential_for(db: &Database, subject_id: i64) {
    infra::upsert(
      db,
      subject_id,
      OwnerType::Character,
      "tok",
      "rt",
      4_102_444_800,
      None,
      None,
    )
    .await
    .unwrap();
  }

  fn noop_emit() -> impl Fn(Event) {
    |_event: Event| {}
  }

  fn recording_emit() -> (std::sync::Arc<std::sync::Mutex<Vec<Event>>>, impl Fn(Event)) {
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = events.clone();
    (events, move |event: Event| sink.lock().unwrap().push(event))
  }

  mod classify {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drives_a_transiently_failing_then_succeeding_row_to_done() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;

      let throttled = failing_registry(|| Error::RateLimit {
        retry_after_secs: 1,
      });
      drain(&db, &esi_client(&db), &sso_client(&db), &throttled, &noop_emit())
        .await
        .unwrap();
      assert_eq!(reload(&db, row.id()).await.status(), "pending");

      infra::reschedule(&db, row.id(), &Utc::now().to_rfc3339(), "transient")
        .await
        .unwrap();
      let recovered = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, StubCalls::default())));
      drain(&db, &esi_client(&db), &sso_client(&db), &recovered, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        reload(&db, row.id()).await.status(),
        "done",
        "once ESI recovers the row drains to done unaided"
      );
    }

    #[tokio::test]
    async fn it_fails_a_row_on_a_real_4xx_rejection() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let registry = one_shot_failing_registry(http_error(403).await);

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(after.status(), "failed", "a 4xx is a permanent rejection");
      assert!(
        after.last_error().is_some(),
        "the rejection is recorded for compensation"
      );
    }

    #[tokio::test]
    async fn it_fails_a_row_that_exhausts_the_max_attempts_cap() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      for _ in 0..MAX_ATTEMPTS {
        infra::reschedule(&db, row.id(), &Utc::now().to_rfc3339(), "transient")
          .await
          .unwrap();
      }
      assert_eq!(reload(&db, row.id()).await.attempts(), MAX_ATTEMPTS);
      let registry = failing_registry(|| Error::RateLimit {
        retry_after_secs: 1,
      });

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        reload(&db, row.id()).await.status(),
        "failed",
        "a transient failure past the cap terminalizes the row"
      );
    }

    #[tokio::test]
    async fn it_pauses_the_engine_for_an_error_limit_reset() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let registry = failing_registry(|| Error::ErrorLimited {
        reset_secs: 45,
      });

      let before = Utc::now();
      let outcome = drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(
        after.status(),
        "pending",
        "an error-limit is transient: the row stays drainable"
      );
      assert_eq!(after.attempts(), 1);
      let next: chrono::DateTime<Utc> = after.next_attempt_at().parse().unwrap();
      assert!(
        next >= before + ChronoDuration::seconds(45),
        "the reset window is honored exactly for the row"
      );
      assert_eq!(
        outcome.error_limit_reset_secs,
        Some(45),
        "the reset window is surfaced so the engine pauses all dispatch"
      );
    }

    #[tokio::test]
    async fn it_reports_the_longest_error_limit_reset_across_a_batch() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      append_send(&db, 100).await;
      append_send(&db, 100).await;
      let next = std::sync::atomic::AtomicU64::new(0);
      let registry = Registry::new().with(Box::new(
        StubHandler::new(OutboxKind::MailSend, StubCalls::default()).failing_execute(move || {
          let resets = [20_u64, 90];
          let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as usize % resets.len();
          Error::ErrorLimited {
            reset_secs: resets[i],
          }
        }),
      ));

      let outcome = drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        outcome.error_limit_reset_secs,
        Some(90),
        "the engine pauses for at least the most-throttled row's window"
      );
    }

    #[tokio::test]
    async fn it_reschedules_a_5xx_row_on_the_backoff_curve() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let registry = one_shot_failing_registry(http_error(503).await);

      let before = Utc::now();
      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(
        after.status(),
        "pending",
        "a 5xx is transient and leaves the row drainable"
      );
      assert_eq!(after.attempts(), 1);
      let next: chrono::DateTime<Utc> = after.next_attempt_at().parse().unwrap();
      assert!(
        next >= before + ChronoDuration::seconds(BACKOFF_BASE_SECS),
        "the first curve retry waits at least one backoff base"
      );
    }

    #[tokio::test]
    async fn it_reschedules_a_rate_limited_row_with_a_bumped_attempt_and_future_retry() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let registry = failing_registry(|| Error::RateLimit {
        retry_after_secs: 30,
      });

      let before = Utc::now();
      let outcome = drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(
        after.status(),
        "pending",
        "a transient failure leaves the row drainable"
      );
      assert_eq!(after.attempts(), 1, "the retry bumps the attempt count");
      assert!(after.last_error().is_some(), "the transient error is recorded");
      let next: chrono::DateTime<Utc> = after.next_attempt_at().parse().unwrap();
      assert!(
        next >= before + ChronoDuration::seconds(30),
        "next_attempt_at must wait out the full Retry-After"
      );
      assert_eq!(
        outcome.error_limit_reset_secs, None,
        "a plain rate-limit does not pause the whole engine"
      );
    }
  }

  mod drain {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drains_a_credentialed_subjects_row_to_done() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, calls.clone())));
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;

      let outcome = drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(calls.executes(), 1, "the handler's ESI write should run once");
      assert_eq!(calls.payloads(), ["{\"body\":\"hi\"}"]);
      assert_eq!(status_of(&db, row.id()).await, "done");
      assert_eq!(
        outcome.error_limit_reset_secs, None,
        "a clean pass requests no engine pause"
      );
    }

    #[tokio::test]
    async fn it_fails_a_row_whose_kind_has_no_handler() {
      let db = store::open_test().await.unwrap();
      let registry = Registry::new();
      credential_for(&db, 300).await;
      let row = append_send(&db, 300).await;

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      let after = reload(&db, row.id()).await;
      assert_eq!(after.status(), "failed");
      assert!(
        after.last_error().is_some(),
        "the unregistered-kind error should be recorded"
      );
    }

    #[tokio::test]
    async fn it_leaves_an_uncredentialed_subjects_row_pending() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, calls.clone())));
      let row = append_send(&db, 200).await;

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(calls.executes(), 0, "no ESI write may run without a grant");
      assert_ne!(
        status_of(&db, row.id()).await,
        "done",
        "an un-credentialed row must not be marked done"
      );
      assert_ne!(
        status_of(&db, row.id()).await,
        "failed",
        "an un-credentialed row must not be failed"
      );
      let now = Utc::now().to_rfc3339();
      let reclaimed = infra::claim_due(&db, &now, DRAIN_BATCH).await.unwrap();
      assert_eq!(reclaimed.iter().map(|r| r.id()).collect::<Vec<_>>(), [row.id()]);
    }
  }

  mod events {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_does_not_emit_failed_when_terminalize_loses_the_cas() {
      let db = store::open_test().await.unwrap();
      let row = append_send(&db, 100).await;
      assert_eq!(row.status(), "pending");
      let handler = StubHandler::new(OutboxKind::MailSend, StubCalls::default());
      let (events, emit) = recording_emit();

      let outcome = terminalize(&db, &row, Some(&handler), "permanent", &emit).await;

      assert_eq!(outcome, Outcome::Skipped);
      assert!(
        events.lock().unwrap().is_empty(),
        "a lost-CAS terminalization emits no failure event"
      );
    }

    #[tokio::test]
    async fn it_emits_failed_with_the_error_for_a_permanent_rejection() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let registry = one_shot_failing_registry(http_error(403).await);
      let (events, emit) = recording_emit();

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &emit)
        .await
        .unwrap();

      let events = events.lock().unwrap();
      assert!(matches!(events[0], Event::OutboxInflight { id } if id == row.id()));
      let failed = events
        .iter()
        .find_map(|e| match e {
          Event::OutboxFailed {
            id,
            reason,
          } if *id == row.id() => Some(reason.clone()),
          _ => None,
        })
        .expect("a permanent rejection emits OutboxFailed");
      assert!(!failed.is_empty(), "the failure carries the recorded error message");
    }

    #[tokio::test]
    async fn it_emits_inflight_then_done_for_a_drained_row() {
      let db = store::open_test().await.unwrap();
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, StubCalls::default())));
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let (events, emit) = recording_emit();

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &emit)
        .await
        .unwrap();

      let events = events.lock().unwrap();
      assert_eq!(events.len(), 2, "a clean drain emits inflight then done");
      assert!(matches!(events[0], Event::OutboxInflight { id } if id == row.id()));
      assert!(matches!(events[1], Event::OutboxDone { id } if id == row.id()));
    }

    #[tokio::test]
    async fn it_emits_retrying_for_a_transient_failure() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let registry = failing_registry(|| Error::RateLimit {
        retry_after_secs: 30,
      });
      let (events, emit) = recording_emit();

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &emit)
        .await
        .unwrap();

      let events = events.lock().unwrap();
      assert!(matches!(events[0], Event::OutboxInflight { id } if id == row.id()));
      assert!(matches!(
        events[1],
        Event::OutboxRetrying { id, retry_secs: 30 } if id == row.id()
      ));
      assert!(
        !events
          .iter()
          .any(|e| matches!(e, Event::OutboxDone { .. } | Event::OutboxFailed { .. })),
        "a rescheduled row is neither done nor failed"
      );
    }
  }

  mod idempotency {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn append_set_read(db: &Database, subject_id: i64, dedupe: Option<&str>) -> Outbox {
      infra::append(
        db,
        OwnerType::Character,
        subject_id,
        "mail.set_read",
        "{\"mail_id\":42,\"is_read\":true}",
        dedupe,
      )
      .await
      .unwrap()
    }

    #[tokio::test]
    async fn it_applies_optimistically_before_any_esi_execute() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let handler = StubHandler::new(OutboxKind::MailSend, calls.clone());
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;

      handler.apply(&db, row.payload()).await.unwrap();
      assert_eq!(calls.applies(), 1, "the optimistic mirror apply ran at append time");
      assert_eq!(
        calls.executes(),
        0,
        "no ESI execute may fire before the drainer claims the row"
      );

      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, calls.clone())));
      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(calls.executes(), 1, "the ESI execute runs only on drain, after apply");
      assert_eq!(status_of(&db, row.id()).await, "done");
    }

    #[tokio::test]
    async fn it_does_not_re_execute_a_done_row_on_a_second_drain_pass() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, calls.clone())));
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();
      assert_eq!(status_of(&db, row.id()).await, "done");
      assert_eq!(calls.executes(), 1);

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        calls.executes(),
        1,
        "a done row must not be re-claimed or re-executed on a later drain pass"
      );
      assert_eq!(status_of(&db, row.id()).await, "done");
    }

    #[tokio::test]
    async fn it_drains_a_deduped_mutation_exactly_once() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSetRead, calls.clone())));
      credential_for(&db, 100).await;

      let first = append_set_read(&db, 100, Some("mail:42:read")).await;
      let second = append_set_read(&db, 100, Some("mail:42:read")).await;
      assert_eq!(
        second.id(),
        first.id(),
        "the redundant mutation collapses onto the first row"
      );

      let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE subject_id = 100 AND kind = ?")
        .bind("mail.set_read")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(count, 1, "dedupe never creates a second outbox row");

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        calls.executes(),
        1,
        "the single collapsed row drains with exactly one ESI write"
      );
      assert_eq!(status_of(&db, first.id()).await, "done");
    }

    #[tokio::test]
    async fn it_redrains_a_row_left_inflight_by_a_crash() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let registry = Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, calls.clone())));
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let claimed = infra::claim_due(&db, &Utc::now().to_rfc3339(), DRAIN_BATCH)
        .await
        .unwrap();
      assert_eq!(claimed.len(), 1);
      assert_eq!(claimed[0].status(), "inflight");
      assert_eq!(calls.executes(), 0, "no write ran before the crash");

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        status_of(&db, row.id()).await,
        "done",
        "the abandoned inflight row is re-claimed and drained to done"
      );
      assert_eq!(
        calls.executes(),
        1,
        "the re-drain performs the ESI write (at-least-once redelivery after a crash)"
      );
    }
  }

  mod reconcile {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_compensates_once_when_it_still_owns_the_inflight_row() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let row = append_send(&db, 100).await;
      let claimed = infra::claim_due(&db, &Utc::now().to_rfc3339(), 1).await.unwrap();
      assert_eq!(claimed.len(), 1);
      assert_eq!(claimed[0].status(), "inflight");
      let handler = StubHandler::new(OutboxKind::MailSend, calls.clone());

      let outcome = terminalize(&db, &claimed[0], Some(&handler), "permanent", &noop_emit()).await;

      assert_eq!(outcome, Outcome::Failed);
      assert_eq!(reload(&db, row.id()).await.status(), "failed");
      assert_eq!(
        calls.compensates(),
        1,
        "the owned inflight change is compensated exactly once"
      );
    }

    #[tokio::test]
    async fn it_compensates_the_optimistic_change_on_a_permanent_failure() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      let registry = observed_one_shot_failing_registry(calls.clone(), http_error(403).await);

      let outcome = drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        reload(&db, row.id()).await.status(),
        "failed",
        "a 4xx terminalizes the row"
      );
      assert_eq!(
        calls.compensates(),
        1,
        "the kind's compensate must run once to undo the optimistic mirror change"
      );
      assert!(
        calls.payloads().contains(&"{\"body\":\"hi\"}".to_string()),
        "compensate is handed the row's payload"
      );
      assert_eq!(
        outcome.error_limit_reset_secs, None,
        "a permanent failure requests no engine pause"
      );
    }

    #[tokio::test]
    async fn it_compensates_when_a_transiently_failing_row_exhausts_its_retries() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      credential_for(&db, 100).await;
      let row = append_send(&db, 100).await;
      for _ in 0..MAX_ATTEMPTS {
        infra::reschedule(&db, row.id(), &Utc::now().to_rfc3339(), "transient")
          .await
          .unwrap();
      }
      let registry = Registry::new().with(Box::new(
        StubHandler::new(OutboxKind::MailSend, calls.clone()).failing_execute(|| Error::RateLimit {
          retry_after_secs: 1,
        }),
      ));

      drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(reload(&db, row.id()).await.status(), "failed");
      assert_eq!(
        calls.compensates(),
        1,
        "exhausting the retry cap is a permanent failure, so compensate runs too"
      );
    }

    #[tokio::test]
    async fn it_does_not_compensate_when_the_row_already_drained_off_inflight() {
      let db = store::open_test().await.unwrap();
      let calls = StubCalls::default();
      let row = append_send(&db, 100).await;
      assert_eq!(row.status(), "pending");
      let handler = StubHandler::new(OutboxKind::MailSend, calls.clone());

      let outcome = terminalize(&db, &row, Some(&handler), "permanent", &noop_emit()).await;

      assert_eq!(
        outcome,
        Outcome::Skipped,
        "a non-inflight row loses the terminalize CAS"
      );
      assert_eq!(
        reload(&db, row.id()).await.status(),
        "pending",
        "the racing reconciliation's status wins; terminalize must not clobber it to failed"
      );
      assert_eq!(
        calls.compensates(),
        0,
        "a row that is no longer inflight is not reverted by a stale optimistic compensation"
      );
    }

    #[tokio::test]
    async fn it_leaves_an_unparseable_kind_failed_without_compensating() {
      let db = store::open_test().await.unwrap();
      credential_for(&db, 100).await;
      let row = infra::append(&db, OwnerType::Character, 100, "mail.send", "{\"body\":\"hi\"}", None)
        .await
        .unwrap();
      let registry = Registry::new();

      let outcome = drain(&db, &esi_client(&db), &sso_client(&db), &registry, &noop_emit())
        .await
        .unwrap();

      assert_eq!(
        reload(&db, row.id()).await.status(),
        "failed",
        "an unhandled kind is terminalized"
      );
      assert_eq!(
        outcome.error_limit_reset_secs, None,
        "an unhandled-kind failure requests no engine pause"
      );
    }
  }

  mod transience {
    use super::*;

    #[tokio::test]
    async fn it_makes_a_4xx_permanent() {
      assert!(matches!(transience(&http_error(400).await), Transience::Permanent));
      assert!(matches!(transience(&http_error(404).await), Transience::Permanent));
    }

    #[tokio::test]
    async fn it_makes_a_5xx_a_curve_transient() {
      assert!(matches!(
        transience(&http_error(503).await),
        Transience::Transient {
          delay: Delay::Curve,
          ..
        }
      ));
    }

    #[tokio::test]
    async fn it_makes_a_non_http_application_error_permanent() {
      assert!(matches!(
        transience(&Error::Internal("bad payload".into())),
        Transience::Permanent
      ));
    }

    #[tokio::test]
    async fn it_makes_a_rate_limit_or_error_limit_a_fixed_delay_transient() {
      assert!(matches!(
        transience(&Error::ErrorLimited {
          reset_secs: 45
        }),
        Transience::Transient {
          delay: Delay::Fixed(45),
          error_limit_reset_secs: Some(45),
        }
      ));

      assert!(matches!(
        transience(&Error::RateLimit {
          retry_after_secs: 30
        }),
        Transience::Transient {
          delay: Delay::Fixed(30),
          error_limit_reset_secs: None,
        }
      ));
    }
  }
}
