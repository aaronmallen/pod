//! The sync engine: a single-task scheduler that dispatches per-subject and global jobs under a
//! concurrency cap, honors ESI error-limit/rate-limit backoff by pausing all dispatch, and drains the
//! optimistic write outbox on a fixed cadence.

use std::{
  any::Any,
  collections::{HashMap, HashSet},
  future::Future,
  panic::AssertUnwindSafe,
  pin::Pin,
  sync::Arc,
  time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures_util::FutureExt;
use tokio::{sync::mpsc, task::JoinSet, time::Instant};

use super::{
  command::Command,
  drain,
  event::Event,
  handle::Handle,
  job::{self, JobCtx, JobKey, JobKind},
  outbox::Registry,
  outcome::Outcome,
  schedule::Schedule,
  subject::Subject,
  token,
};
use crate::{
  clients::{Error, esi, eve_image, eve_sso},
  config::FeatureFlags,
  store::{
    Database, images,
    model::{OwnerType, SyncLedger},
    repo::{character, infra, org, sync_ledger},
  },
};

const DONE_RETENTION: Duration = Duration::from_secs(60 * 60);

const DRAIN_INTERVAL: Duration = Duration::from_secs(5);

const EVENT_BUFFER: usize = 64;

const GLOBAL_SUBJECT: Subject = Subject::Character(0);

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

const MAX_CONCURRENT_JOBS: usize = 4;

const NOT_READY_RETRY: Duration = Duration::from_secs(3);

const PENDING_RETRY: Duration = Duration::from_secs(120);

const RESPAWN_BACKOFF_BASE: Duration = Duration::from_secs(1);

const RESPAWN_BACKOFF_CAP: Duration = Duration::from_secs(30);

const RESPAWN_BREAKER_THRESHOLD: u32 = 5;

const RESPAWN_BREAKER_WINDOW: Duration = Duration::from_secs(60);

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Default)]
struct Breaker {
  attempts: u32,
  consecutive: u32,
}

impl Breaker {
  fn record_restart(&mut self, lived: Duration) -> Option<RestartPlan> {
    if lived >= RESPAWN_BREAKER_WINDOW {
      self.consecutive = 0;
    }
    self.consecutive += 1;
    if self.consecutive >= RESPAWN_BREAKER_THRESHOLD {
      return None;
    }
    self.attempts += 1;
    Some(RestartPlan {
      attempt: self.attempts,
      backoff: backoff_for(self.consecutive),
    })
  }

  fn reset(&mut self) {
    self.attempts = 0;
    self.consecutive = 0;
  }
}

enum Disposition {
  Restart,
  Stop { deliberate: bool },
}

struct Engine {
  db: Database,
  drain_at: Instant,
  esi: Arc<esi::Client>,
  events: mpsc::Sender<Event>,
  // A connection pool reserved exclusively for housekeeping (the post-job ledger upsert plus the
  // outbox drain/prune maintenance), kept separate from `db` so the worker pool can never starve
  // housekeeping of a connection. Workers hold up to MAX_CONCURRENT_JOBS of `db`'s connections; this
  // pool guarantees finish() always has a free connection and never queues behind `busy_timeout`.
  housekeeping: Database,
  image: Arc<eve_image::Client>,
  image_store: images::Store,
  outbox: Registry,
  paused_until: Instant,
  schedule: Schedule,
  sso: Arc<eve_sso::Client>,
}

impl Engine {
  async fn deferred_gathers(&self, subject: Subject) -> HashSet<JobKind> {
    if self.parent_row_exists(subject).await {
      return HashSet::new();
    }
    profile_kind(subject)
      .on_success_triggers()
      .iter()
      .copied()
      .filter(|kind| kind.applies_to(subject))
      .collect()
  }

  async fn discover(&mut self) {
    let now = Instant::now();
    let mut owned_characters = HashSet::new();
    if let Ok(credentials) = infra::all(&self.db).await {
      for credential in credentials {
        if credential.owner_type() == OwnerType::Character {
          owned_characters.insert(credential.owner_id());
        }
        let subject = subject_from(credential.owner_id(), credential.owner_type());
        let granted: HashSet<&str> = credential
          .scopes()
          .as_deref()
          .unwrap_or_default()
          .split_whitespace()
          .collect();
        let seeds = self.seeds_for(subject, now).await;
        let deferred = self.deferred_gathers(subject).await;
        // A flagged entity's token is known-bad, so enroll only its public (no-scope) jobs and keep
        // its privileged jobs out of the schedule until the TokenAudit clears the flag.
        let kinds = if credential.needs_reauth() {
          JobKind::public_for_subject(subject)
        } else {
          JobKind::granted_for_subject(subject, &granted)
        };
        self
          .schedule
          .enroll_kinds_deferred(subject, kinds, now, &seeds, &deferred);
      }
    }
    if let Ok(characters) = character::all(&self.db).await {
      for character in characters {
        let subject = Subject::Character(character.id());
        if !owned_characters.contains(&character.id()) {
          let seeds = self.seeds_for(subject, now).await;
          self
            .schedule
            .enroll_kinds_seeded(subject, JobKind::public_for_subject(subject), now, &seeds);
        }
      }
    }
  }

  fn emit(&self, event: Event) {
    let _ = self.events.try_send(event);
  }

  async fn enroll_global(&mut self) {
    let now = Instant::now();
    let seeds = self.seeds_for(GLOBAL_SUBJECT, now).await;
    self
      .schedule
      .enroll_kinds_seeded(GLOBAL_SUBJECT, global_kinds(), now, &seeds);
  }

  async fn enroll_subject(&mut self, subject: Subject, now: Instant) {
    let (owner_id, owner_type) = owner_of(subject);
    match infra::get(&self.db, owner_id, owner_type).await {
      Ok(Some(credential)) => {
        let granted: HashSet<&str> = credential
          .scopes()
          .as_deref()
          .unwrap_or_default()
          .split_whitespace()
          .collect();
        let kinds = if credential.needs_reauth() {
          JobKind::public_for_subject(subject)
        } else {
          JobKind::granted_for_subject(subject, &granted)
        };
        let seeds = self.seeds_for(subject, now).await;
        let deferred = self.deferred_gathers(subject).await;
        self
          .schedule
          .enroll_kinds_deferred(subject, kinds, now, &seeds, &deferred);
      }
      _ => self.schedule.enroll(subject, now),
    }
  }

  async fn finish(&mut self, key: JobKey, result: Result<Outcome, Error>) {
    let now = Instant::now();
    let attempt_at = Utc::now().to_rfc3339();
    let outcome = match result {
      Ok(outcome) => {
        if is_pending_outcome(&outcome) {
          self
            .schedule
            .reschedule_throttle(key, now, PENDING_RETRY.min(key.kind.interval()));
        } else {
          self.schedule.reschedule_success(key, now);
        }
        for triggered in key.kind.on_success_triggers() {
          if triggered.applies_to(key.subject) {
            self.schedule.make_due_now_for_subject(*triggered, key.subject, now);
          } else if triggered.is_global() {
            self.schedule.make_due_now(*triggered, now);
          }
        }
        tracing::debug!(?key, outcome = outcome.label(), "sync job finished");
        self.emit(Event::Finished {
          key,
          outcome: outcome.clone(),
        });
        outcome
      }
      Err(Error::NotReady) => {
        self.schedule.reschedule_throttle(key, now, NOT_READY_RETRY);
        tracing::info!(
          ?key,
          retry_secs = NOT_READY_RETRY.as_secs(),
          "parent record not yet persisted; short-retrying without an ESI call"
        );
        Outcome::NotReady
      }
      Err(Error::ErrorLimited {
        reset_secs,
      }) => {
        let delay = Duration::from_secs(reset_secs.max(1));
        self.paused_until = now + delay;
        self.schedule.reschedule_throttle(key, now, delay);
        tracing::warn!(?key, reset_secs, "ESI error-limited; pausing dispatch");
        self.emit(Event::BackingOff {
          key,
          retry_secs: reset_secs,
        });
        Outcome::Failed {
          reason: format!("ESI error-limited; resets in {reset_secs}s"),
        }
      }
      Err(Error::RateLimit {
        retry_after_secs,
      }) => {
        let delay = Duration::from_secs(retry_after_secs.max(1));
        self.schedule.reschedule_throttle(key, now, delay);
        tracing::warn!(?key, retry_after_secs, "rate-limited; backing off");
        self.emit(Event::BackingOff {
          key,
          retry_secs: retry_after_secs,
        });
        Outcome::Failed {
          reason: format!("rate-limited; retry after {retry_after_secs}s"),
        }
      }
      Err(error) if is_permanent_failure(&error) => {
        self.schedule.reschedule_permanent(key, now);
        let reason = error.to_string();
        tracing::warn!(?key, %error, "sync job failed permanently; parking until re-authentication");
        self.emit(Event::Failed {
          key,
          reason: reason.clone(),
        });
        Outcome::Failed {
          reason,
        }
      }
      Err(error) => {
        self.schedule.reschedule_failure(key, now);
        let reason = error.to_string();
        if error.is_foreign_key_violation() {
          tracing::warn!(
            ?key,
            %error,
            "sync job failed: foreign-key violation; error names the character/corp whose org row was missing"
          );
        } else {
          tracing::warn!(?key, %error, "sync job failed");
        }
        self.emit(Event::Failed {
          key,
          reason: reason.clone(),
        });
        Outcome::Failed {
          reason,
        }
      }
    };
    let next_in = self.schedule.next_in(key, now);
    let next_eligible_at = next_in.map(|delay| {
      (Utc::now() + ChronoDuration::from_std(delay).unwrap_or_else(|_| ChronoDuration::zero())).to_rfc3339()
    });
    self
      .record_ledger(key, &outcome, &attempt_at, next_eligible_at.as_deref())
      .await;
    if let Some(next_in) = next_in {
      self.emit(Event::Scheduled {
        key,
        next_in_secs: next_in.as_secs(),
      });
    }
  }

  async fn handle_command(&mut self, command: Command) {
    let now = Instant::now();
    match command {
      Command::Discover => self.discover().await,
      Command::Drain => self.drain_at = now,
      Command::Enroll(subject) => self.enroll_subject(subject, now).await,
      Command::RunNow(subject) => self.schedule.run_now(subject, now),
      Command::SetFeatures(features) => self.schedule.reconcile_features(features),
      Command::Shutdown => {} // intercepted in the run loop before reaching here; arm exists for exhaustiveness
      Command::Withdraw(subject) => self.schedule.withdraw(subject),
    }
  }

  async fn maybe_drain(&mut self, now: Instant) {
    if now < self.drain_at {
      return;
    }
    self.drain_at = now + DRAIN_INTERVAL;
    let events = self.events.clone();
    let emit = move |event: Event| {
      let _ = events.try_send(event);
    };
    match drain::drain(&self.housekeeping, &self.esi, &self.sso, &self.outbox, &emit).await {
      Ok(outcome) => {
        if let Some(reset_secs) = outcome.error_limit_reset_secs {
          let until = now + Duration::from_secs(reset_secs.max(1));
          self.paused_until = self.paused_until.max(until);
          tracing::warn!(reset_secs, "outbox drain error-limited; pausing dispatch");
        }
      }
      Err(error) => tracing::warn!(%error, "outbox drain pass failed"),
    }
    self.prune_done_rows().await;
  }

  fn next_wake(&self, now: Instant) -> Instant {
    let dispatch_at = if now < self.paused_until {
      Some(self.paused_until)
    } else {
      let job_deadline = self.schedule.next_deadline();
      let maintenance_at = self.drain_at;
      Some(job_deadline.map_or(maintenance_at, |job| job.min(maintenance_at)))
    };
    dispatch_at.map_or(now + HEARTBEAT_INTERVAL, |at| at.min(now + HEARTBEAT_INTERVAL))
  }

  async fn parent_row_exists(&self, subject: Subject) -> bool {
    match subject {
      Subject::Character(id) => matches!(character::get(&self.db, id).await, Ok(Some(_))),
      Subject::Corporation(id) => {
        matches!(org::get_corporation(&self.db, id).await, Ok(Some(_)))
      }
    }
  }

  async fn prune_done_rows(&self) {
    let before = (Utc::now() - ChronoDuration::from_std(DONE_RETENTION).unwrap_or(ChronoDuration::zero())).to_rfc3339();
    match infra::prune_done(&self.housekeeping, &before).await {
      Ok(pruned) if pruned > 0 => {
        tracing::debug!(pruned, "pruned done outbox rows past retention")
      }
      Ok(_) => {}
      Err(error) => tracing::warn!(%error, "pruning done outbox rows failed"),
    }
  }

  async fn record_ledger(&self, key: JobKey, outcome: &Outcome, attempt_at: &str, next_eligible_at: Option<&str>) {
    let (subject_id, subject_type) = owner_of(key.subject);
    let success_at = matches!(outcome, Outcome::Synced { .. } | Outcome::Empty).then_some(attempt_at);
    if let Err(error) = sync_ledger::upsert(
      &self.housekeeping,
      subject_type,
      subject_id,
      &format!("{:?}", key.kind),
      outcome.label(),
      outcome.rows_touched(),
      outcome.reason(),
      success_at,
      next_eligible_at,
    )
    .await
    {
      tracing::warn!(?key, %error, "failed to record sync ledger row");
    }
  }

  async fn run(mut self, commands: &mut mpsc::UnboundedReceiver<Command>) -> ExitReason {
    self.enroll_global().await;
    self.discover().await;
    let mut in_flight: JoinSet<(JobKey, Result<Outcome, Error>)> = JoinSet::new();
    let mut last_dispatched: Option<JobKey> = None;
    let reason = loop {
      let now = Instant::now();
      if now >= self.paused_until {
        let free = MAX_CONCURRENT_JOBS.saturating_sub(in_flight.len());
        for key in self.schedule.due(now).into_iter().take(free) {
          self.schedule.mark_in_flight(key);
          tracing::debug!(?key, "sync job started");
          self.emit(Event::Started {
            key,
          });
          let db = self.db.clone();
          let esi = Arc::clone(&self.esi);
          let sso = Arc::clone(&self.sso);
          let image = Arc::clone(&self.image);
          let image_store = self.image_store.clone();
          last_dispatched = Some(key);
          in_flight.spawn(async move { (key, run_job(&db, &esi, &sso, &image, &image_store, key).await) });
        }
        self.maybe_drain(now).await;
      }

      let deadline = self.next_wake(now);
      tokio::select! {
        finished = in_flight.join_next(), if !in_flight.is_empty() => match finished {
          Some(Ok((key, result))) => self.finish(key, result).await,
          Some(Err(join_error)) => tracing::error!(%join_error, "a sync job task panicked"),
          None => {}
        },
        _ = tokio::time::sleep_until(deadline) => {
          self.enroll_global().await;
          self.discover().await;
          self.emit(Event::Heartbeat);
        }
        command = commands.recv() => match command {
          Some(Command::Shutdown) => {
            in_flight.shutdown().await;
            break ExitReason::Shutdown;
          }
          Some(command) => self.handle_command(command).await,
          None => break ExitReason::ChannelClosed,
        },
      }
    };
    let in_flight_count = in_flight.len();
    if reason.is_deliberate() {
      tracing::debug!(
        target: "pod::lifecycle",
        reason = reason.label(),
        in_flight = in_flight_count,
        last_dispatched = ?last_dispatched,
        "sync engine run loop exited",
      );
    } else {
      tracing::warn!(
        target: "pod::lifecycle",
        reason = reason.label(),
        in_flight = in_flight_count,
        last_dispatched = ?last_dispatched,
        "sync engine run loop exited unexpectedly",
      );
    }
    reason
  }

  async fn seeds_for(&self, subject: Subject, now: Instant) -> HashMap<JobKind, Instant> {
    let (subject_id, subject_type) = owner_of(subject);
    let mut seeds = HashMap::new();
    let Ok(rows) = sync_ledger::for_subject(&self.db, subject_type, subject_id).await else {
      return seeds;
    };
    for row in &rows {
      let Some(kind) = JobKind::ALL
        .iter()
        .copied()
        .find(|kind| format!("{kind:?}") == *row.kind())
      else {
        continue;
      };
      if let Some(at) = future_seed(row, kind, now) {
        seeds.insert(kind, at);
      }
    }
    seeds
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExitReason {
  ChannelClosed,
  Shutdown,
}

impl ExitReason {
  fn is_deliberate(self) -> bool {
    matches!(self, ExitReason::Shutdown)
  }

  fn label(self) -> &'static str {
    match self {
      ExitReason::ChannelClosed => "channel-closed",
      ExitReason::Shutdown => "shutdown",
    }
  }
}

struct RestartPlan {
  attempt: u32,
  backoff: Duration,
}

struct Supervisor<O>
where
  O: Fn() -> Registry + Send + Sync,
{
  command_rx: mpsc::UnboundedReceiver<Command>,
  db: Database,
  esi: Arc<esi::Client>,
  events: mpsc::Sender<Event>,
  features: FeatureFlags,
  housekeeping: Database,
  image: Arc<eve_image::Client>,
  image_store: images::Store,
  outbox: O,
  restart_rx: mpsc::UnboundedReceiver<()>,
  sso: Arc<eve_sso::Client>,
}

impl<O> Supervisor<O>
where
  O: Fn() -> Registry + Send + Sync,
{
  async fn supervise(self) {
    let Supervisor {
      command_rx,
      db,
      esi,
      events,
      features,
      housekeeping,
      image,
      image_store,
      outbox,
      restart_rx,
      sso,
    } = self;
    let build = || Engine {
      db: db.clone(),
      drain_at: Instant::now(),
      esi: Arc::clone(&esi),
      events: events.clone(),
      housekeeping: housekeeping.clone(),
      image: Arc::clone(&image),
      image_store: image_store.clone(),
      outbox: outbox(),
      paused_until: Instant::now(),
      schedule: Schedule::with_features(features),
      sso: Arc::clone(&sso),
    };
    // Each attempt rebuilds a fresh Engine but feeds it the same command_rx and event_tx, so the
    // app's Handle and forward_sync_events stay wired to whichever engine is current.
    supervise_loop(&events, command_rx, restart_rx, |command_rx| {
      Box::pin(build().run(command_rx))
    })
    .await;
  }
}

pub fn spawn(
  db: Database,
  housekeeping: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  image: Arc<eve_image::Client>,
  image_store: images::Store,
  features: FeatureFlags,
) -> (Handle, mpsc::Receiver<Event>) {
  spawn_with_registry(db, housekeeping, esi, sso, image, image_store, features, || {
    super::mail_handlers::registry()
      .extend(super::calendar_handlers::registry())
      .extend(super::contact_handlers::registry())
  })
}

#[allow(clippy::too_many_arguments)]
fn spawn_with_registry<O>(
  db: Database,
  housekeeping: Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  image: Arc<eve_image::Client>,
  image_store: images::Store,
  features: FeatureFlags,
  outbox: O,
) -> (Handle, mpsc::Receiver<Event>)
where
  O: Fn() -> Registry + Send + Sync + 'static,
{
  let (command_tx, command_rx) = mpsc::unbounded_channel();
  let (event_tx, event_rx) = mpsc::channel(EVENT_BUFFER);
  let (restart_tx, restart_rx) = mpsc::unbounded_channel();
  let supervisor = Supervisor {
    command_rx,
    db,
    esi,
    events: event_tx,
    features,
    housekeeping,
    image,
    image_store,
    outbox,
    restart_rx,
    sso,
  };
  tokio::spawn(supervisor.supervise());
  (Handle::new(command_tx, restart_tx), event_rx)
}

fn backoff_for(consecutive: u32) -> Duration {
  let shift = consecutive.saturating_sub(1).min(u32::BITS - 1);
  RESPAWN_BACKOFF_BASE
    .saturating_mul(1u32.checked_shl(shift).unwrap_or(u32::MAX))
    .min(RESPAWN_BACKOFF_CAP)
}

fn disposition(outcome: Result<ExitReason, Box<dyn Any + Send>>) -> Disposition {
  match outcome {
    Ok(ExitReason::Shutdown) => Disposition::Stop {
      deliberate: true,
    },
    Ok(ExitReason::ChannelClosed) => Disposition::Stop {
      deliberate: false,
    },
    Err(panic) => {
      let detail = panic_message(&panic);
      tracing::error!(target: "pod::lifecycle", detail, "the sync engine loop panicked");
      Disposition::Restart
    }
  }
}

fn give_up_reason(consecutive: u32) -> String {
  format!("the sync engine died {consecutive} times in quick succession; auto-restart gave up")
}

fn panic_message(panic: &(dyn Any + Send)) -> &str {
  panic
    .downcast_ref::<&'static str>()
    .copied()
    .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
    .unwrap_or("unknown panic")
}

async fn supervise_loop<F>(
  events: &mpsc::Sender<Event>,
  mut command_rx: mpsc::UnboundedReceiver<Command>,
  mut restart_rx: mpsc::UnboundedReceiver<()>,
  mut run_engine: F,
) where
  F: for<'a> FnMut(&'a mut mpsc::UnboundedReceiver<Command>) -> BoxFuture<'a, ExitReason>,
{
  let mut breaker = Breaker::default();
  loop {
    let started = Instant::now();
    // catch_unwind isolates a panic in the run loop into an Err here instead of unwinding the
    // supervisor task itself, which would silently end auto-restart and defeat the whole point.
    let outcome = AssertUnwindSafe(run_engine(&mut command_rx)).catch_unwind().await;

    match disposition(outcome) {
      Disposition::Stop {
        deliberate,
      } => {
        if deliberate {
          tracing::info!(target: "pod::lifecycle", "the sync engine task stopped");
        } else {
          tracing::info!(
            target: "pod::lifecycle",
            "the sync engine command channel closed; no clients remain, so the supervisor stops"
          );
        }
        return;
      }
      Disposition::Restart => match breaker.record_restart(started.elapsed()) {
        None => {
          let consecutive = breaker.consecutive;
          tracing::error!(
            target: "pod::lifecycle",
            consecutive,
            "the sync engine died repeatedly in quick succession; the circuit breaker tripped, giving up auto-restart"
          );
          let _ = events.try_send(Event::GaveUp {
            reason: give_up_reason(consecutive),
          });
          // The breaker has tripped, so stop auto-restarting — but stay alive parked on the restart
          // channel so a user-triggered "Restart sync" can still reach the (otherwise dead) engine
          // and resume. The supervisor only truly exits when every Handle is dropped (app teardown).
          match restart_rx.recv().await {
            Some(()) => {
              breaker.reset();
              tracing::info!(target: "pod::lifecycle", "restarting the sync engine on request after giving up");
              let _ = events.try_send(Event::Restarted {
                attempt: 0,
              });
            }
            None => return,
          }
        }
        Some(plan) => {
          tracing::warn!(
            target: "pod::lifecycle",
            attempt = plan.attempt,
            consecutive = breaker.consecutive,
            backoff_secs = plan.backoff.as_secs(),
            "the sync engine died unexpectedly; restarting after backoff"
          );
          // A manual restart short-circuits the backoff wait and resets the breaker so retries
          // resume immediately rather than after the (possibly capped) delay.
          tokio::select! {
            _ = tokio::time::sleep(plan.backoff) => {}
            signal = restart_rx.recv() => {
              if signal.is_none() {
                return;
              }
              breaker.reset();
            }
          }
          let _ = events.try_send(Event::Restarted {
            attempt: plan.attempt,
          });
        }
      },
    }
  }
}

async fn run_job(
  db: &Database,
  esi: &Arc<esi::Client>,
  sso: &Arc<eve_sso::Client>,
  image: &Arc<eve_image::Client>,
  image_store: &images::Store,
  key: JobKey,
) -> Result<Outcome, Error> {
  let grant = if key.kind.required_scope().is_empty() {
    None
  } else {
    let (owner_id, owner_type) = owner_of(key.subject);
    // Honor the persisted needs-reauth flag at dispatch: a flagged entity's token is known-bad, so
    // skip the work entirely rather than refresh-and-fail against it. The job reschedules on its
    // normal interval (a benign no-op), not the tight backoff loop, and stays skipped until the
    // TokenAudit clears the flag or a re-auth's run_now revives it.
    if is_flagged_needs_reauth(db, owner_id, owner_type).await {
      tracing::debug!(?key, "skipping privileged job for an entity flagged needs-reauth");
      return Ok(Outcome::Skipped {
        reason: "entity needs re-authentication".to_string(),
      });
    }
    let Some(grant) = token::fresh_token(db, sso, owner_id, owner_type).await? else {
      tracing::warn!(
        ?key,
        "privileged job has no usable credential; surfacing as needs-reauth"
      );
      return Err(Error::Auth(format!(
        "{:?} for {:?} has no usable credential",
        key.kind, key.subject
      )));
    };
    let granted: HashSet<&str> = grant.scopes().iter().map(String::as_str).collect();
    if !key.kind.is_scope_granted(key.subject, &granted) {
      tracing::debug!(?key, "skipping job whose required scope the credential does not grant");
      return Ok(Outcome::Skipped {
        reason: "required scope not granted".to_string(),
      });
    }
    Some(grant)
  };
  let ctx = JobCtx {
    db,
    esi,
    image,
    image_store,
    key,
    grant: grant.as_ref(),
    // Only the global TokenAudit job needs raw SSO access (to force-refresh every credential); every
    // other job has already had its grant resolved above, so it gets None.
    sso: matches!(key.kind, JobKind::TokenAudit).then_some(sso.as_ref()),
  };
  job::run(&ctx).await
}

async fn is_flagged_needs_reauth(db: &Database, owner_id: i64, owner_type: OwnerType) -> bool {
  matches!(infra::get(db, owner_id, owner_type).await, Ok(Some(credential)) if credential.needs_reauth())
}

fn is_pending_outcome(outcome: &Outcome) -> bool {
  matches!(outcome, Outcome::Blocked { .. } | Outcome::NotReady)
}

fn is_permanent_failure(error: &Error) -> bool {
  match error {
    Error::Auth(message) | Error::Internal(message) => {
      message.contains("needs re-authentication")
        || message.contains("has no authorizing character")
        || message.contains("has no usable credential")
    }
    // A revoked refresh token surfaces as HTTP 400 (invalid_grant) from the SSO refresh path that
    // every privileged job runs through before its ESI call. Classifying it permanent parks the job
    // (reschedule_permanent) instead of looping it on the ~300s backoff against a dead token; the
    // TokenAudit job and a re-auth's run_now are what revive it.
    _ => token::is_revoked_refresh(error),
  }
}

fn future_seed(row: &SyncLedger, kind: JobKind, now: Instant) -> Option<Instant> {
  let eligible = ledger_eligible_at(row, kind)?;
  let delay = (eligible - Utc::now()).to_std().ok()?;
  Some(now + delay.min(kind.interval()))
}

fn ledger_eligible_at(row: &SyncLedger, kind: JobKind) -> Option<DateTime<Utc>> {
  if let Some(next_eligible_at) = row.next_eligible_at().as_deref().and_then(parse_rfc3339) {
    return Some(next_eligible_at);
  }
  let last_success_at = row.last_success_at().as_deref().and_then(parse_rfc3339)?;
  Some(last_success_at + ChronoDuration::from_std(kind.interval()).unwrap_or_else(|_| ChronoDuration::zero()))
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|at| at.with_timezone(&Utc))
}

fn global_kinds() -> Vec<JobKind> {
  JobKind::ALL.iter().copied().filter(|kind| kind.is_global()).collect()
}

fn owner_of(subject: Subject) -> (i64, OwnerType) {
  match subject {
    Subject::Character(id) => (id, OwnerType::Character),
    Subject::Corporation(id) => (id, OwnerType::Corporation),
  }
}

fn profile_kind(subject: Subject) -> JobKind {
  match subject {
    Subject::Character(_) => JobKind::CharacterProfile,
    Subject::Corporation(_) => JobKind::CorporationProfile,
  }
}

fn subject_from(owner_id: i64, owner_type: OwnerType) -> Subject {
  match owner_type {
    OwnerType::Character => Subject::Character(owner_id),
    OwnerType::Corporation => Subject::Corporation(owner_id),
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::http,
    store::{self, repo::character},
  };

  async fn spawn_engine(base_url: String) -> (Handle, mpsc::Receiver<Event>, Database, tempfile::TempDir) {
    let db = store::open_test().await.unwrap();
    let (handle, events, images_dir) = spawn_engine_with_db(db.clone(), base_url).await;
    (handle, events, db, images_dir)
  }

  async fn spawn_engine_with_db(db: Database, base_url: String) -> (Handle, mpsc::Receiver<Event>, tempfile::TempDir) {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url.clone()));
    let image = Arc::new(eve_image::Client::with_base_url(http.clone(), base_url));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    let images_dir = tempfile::tempdir().unwrap();
    let store = images::Store::new(images_dir.path().to_path_buf());
    let (handle, events) = spawn(db.clone(), db, esi, sso, image, store, FeatureFlags::default());
    (handle, events, images_dir)
  }

  async fn spawn_run(base_url: String) -> (mpsc::UnboundedSender<Command>, tokio::task::JoinHandle<ExitReason>) {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url.clone()));
    let image = Arc::new(eve_image::Client::with_base_url(http.clone(), base_url));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
    let images_dir = tempfile::tempdir().unwrap();
    let store = images::Store::new(images_dir.path().to_path_buf());
    let (events, _event_rx) = mpsc::channel(EVENT_BUFFER);
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let engine = Engine {
      db: db.clone(),
      drain_at: Instant::now(),
      esi,
      events,
      housekeeping: db,
      image,
      image_store: store,
      outbox: Registry::new(),
      paused_until: Instant::now(),
      schedule: Schedule::with_features(FeatureFlags::default()),
      sso,
    };
    let run = tokio::spawn(async move {
      let mut command_rx = command_rx;
      engine.run(&mut command_rx).await
    });
    (command_tx, run)
  }

  async fn wait_for<F: Fn(&Event) -> bool>(events: &mut mpsc::Receiver<Event>, predicate: F) -> Event {
    loop {
      let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("timed out waiting for event")
        .expect("event channel closed");
      if predicate(&event) {
        return event;
      }
    }
  }

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_character_profile(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/"),
      serde_json::json!({
        "alliance_id": 300, "birthday": "2010-01-01T00:00:00Z", "bloodline_id": 5,
        "corporation_id": 200, "gender": "male", "name": "Test Pilot", "race_id": 1,
      }),
    )
    .await;
    mount_json(
      server,
      "/corporations/200/",
      serde_json::json!({
        "alliance_id": 300, "ceo_id": character_id, "creator_id": character_id, "member_count": 42,
        "name": "Test Corp", "tax_rate": 0.1, "ticker": "TST",
      }),
    )
    .await;
    mount_json(
      server,
      "/alliances/300/",
      serde_json::json!({
        "creator_corporation_id": 200, "creator_id": character_id,
        "date_founded": "2005-01-01T00:00:00Z", "name": "Test Alliance", "ticker": "TSTA",
      }),
    )
    .await;
    mount_json(
      server,
      "/universe/races/",
      serde_json::json!([{ "alliance_id": 300, "description": "The Caldari.", "name": "Caldari", "race_id": 1 }]),
    )
    .await;
    mount_json(
      server,
      "/universe/bloodlines/",
      serde_json::json!([
        { "bloodline_id": 5, "charisma": 6, "corporation_id": 200, "description": "The Civire.",
          "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
          "ship_type_id": 601, "willpower": 5 },
      ]),
    )
    .await;
    Mock::given(method("GET"))
      .and(path(format!("/characters/{character_id}/portrait")))
      .respond_with(ResponseTemplate::new(200).set_body_raw(vec![1u8, 2, 3], "image/jpeg"))
      .mount(server)
      .await;
  }

  async fn seed_ship_type(db: &Database) {
    sqlx::query("INSERT INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query("INSERT INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
      .execute(db.writer())
      .await
      .unwrap();
    sqlx::query(
      "INSERT INTO item_types (id, group_id, description, name, published) VALUES (601, 25, 'Merlin', 'Merlin', 1)",
    )
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn seed_character(db: &Database, id: i64) {
    let mut tx = db.writer().begin().await.unwrap();
    sqlx::query("PRAGMA defer_foreign_keys = ON")
      .execute(&mut *tx)
      .await
      .unwrap();
    sqlx::query("INSERT INTO races (id, alliance_id, description, name) VALUES (1, 0, 'r', 'Race')")
      .execute(&mut *tx)
      .await
      .unwrap();
    sqlx::query(
      "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (1, 0, 0, 1, 'Corp', 0.0, 'CRP')",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO bloodlines (id, corporation_id, race_id, charisma, description, intelligence, \
      memory, name, perception, willpower) VALUES (1, 1, 1, 1, 'b', 1, 1, 'Bloodline', 1, 1)",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
      "INSERT INTO characters (id, bloodline_id, corporation_id, race_id, birthday, gender, name) \
      VALUES (?, 1, 1, 1, '2010-01-01T00:00:00Z', 'male', 'NPC Pilot')",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
  }

  async fn mount_skill_picture(server: &MockServer, character_id: i64) {
    mount_json(
      server,
      &format!("/characters/{character_id}/skills/"),
      serde_json::json!({
        "skills": [
          { "active_skill_level": 5, "skill_id": 3300, "skillpoints_in_skill": 256000, "trained_skill_level": 5 },
        ],
        "total_sp": 256000,
        "unallocated_sp": 15000,
      }),
    )
    .await;
    mount_json(
      server,
      &format!("/characters/{character_id}/skillqueue/"),
      serde_json::json!([
        { "finish_date": "2026-06-01T00:00:00Z", "finished_level": 5, "queue_position": 0, "skill_id": 3300 },
      ]),
    )
    .await;
    mount_json(
      server,
      &format!("/characters/{character_id}/attributes/"),
      serde_json::json!({
        "charisma": 20, "intelligence": 22, "memory": 21, "perception": 20, "willpower": 20,
        "bonus_remaps": 2, "last_remap_date": "2023-04-01T12:00:00Z",
        "accrued_remap_cooldown_date": "2024-04-01T12:00:00Z",
      }),
    )
    .await;
    mount_json(
      server,
      &format!("/characters/{character_id}/implants/"),
      serde_json::json!([9899]),
    )
    .await;
    mount_json(
      server,
      "/universe/types/9899/",
      serde_json::json!({
        "description": "A memory implant.", "group_id": 300, "name": "Memory Augmentation - Basic",
        "published": true, "type_id": 9899,
        "dogma_attributes": [{ "attribute_id": 177, "value": 3.0 }],
      }),
    )
    .await;
    mount_json(
      server,
      "/universe/types/3300/",
      serde_json::json!({
        "description": "Gunnery.", "group_id": 255, "market_group_id": 1112, "name": "Gunnery",
        "published": true, "type_id": 3300,
        "dogma_attributes": [
          { "attribute_id": 275, "value": 1.0 },
          { "attribute_id": 180, "value": 167.0 },
          { "attribute_id": 181, "value": 168.0 },
        ],
      }),
    )
    .await;
    mount_json(
      server,
      "/universe/groups/255/",
      serde_json::json!({ "category_id": 16, "group_id": 255, "name": "Gunnery", "published": true, "types": [3300] }),
    )
    .await;
    mount_json(
      server,
      "/universe/categories/16/",
      serde_json::json!({ "category_id": 16, "groups": [255], "name": "Skill", "published": true }),
    )
    .await;
    mount_json(
      server,
      "/markets/groups/1112/",
      serde_json::json!({
        "description": "Skill books.", "market_group_id": 1112, "name": "Skills",
        "parent_group_id": 1111, "types": [3300],
      }),
    )
    .await;
    mount_json(
      server,
      "/markets/groups/1111/",
      serde_json::json!({
        "description": "All market groups.", "market_group_id": 1111, "name": "Market", "types": [],
      }),
    )
    .await;
  }

  mod backoff_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_at_the_cap_and_never_overflows() {
      assert_eq!(super::super::backoff_for(20), RESPAWN_BACKOFF_CAP);
      assert_eq!(super::super::backoff_for(u32::MAX), RESPAWN_BACKOFF_CAP);
    }

    #[test]
    fn it_grows_exponentially_from_the_base() {
      assert_eq!(super::super::backoff_for(1), RESPAWN_BACKOFF_BASE);
      assert_eq!(super::super::backoff_for(2), RESPAWN_BACKOFF_BASE * 2);
      assert_eq!(super::super::backoff_for(3), RESPAWN_BACKOFF_BASE * 4);
    }
  }

  mod breaker {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clears_the_streak_and_attempt_count_on_reset() {
      let mut breaker = Breaker::default();
      for _ in 0..RESPAWN_BREAKER_THRESHOLD - 1 {
        breaker.record_restart(Duration::ZERO);
      }

      breaker.reset();
      let after_reset: Vec<_> = (0..RESPAWN_BREAKER_THRESHOLD - 1)
        .map(|_| breaker.record_restart(Duration::ZERO))
        .collect();

      assert!(
        after_reset.iter().all(Option::is_some),
        "after a reset the breaker grants a full fresh run of restarts before tripping again"
      );
      assert_eq!(
        after_reset
          .first()
          .and_then(|plan| plan.as_ref())
          .map(|plan| plan.attempt),
        Some(1),
        "a reset restarts the attempt counter from one"
      );
    }

    #[test]
    fn it_plans_a_restart_with_a_rising_attempt_count() {
      let mut breaker = Breaker::default();

      let first = breaker
        .record_restart(Duration::ZERO)
        .expect("a first death plans a restart");
      let second = breaker
        .record_restart(Duration::ZERO)
        .expect("a second death plans a restart");

      assert_eq!(first.attempt, 1);
      assert_eq!(second.attempt, 2);
    }

    #[test]
    fn it_resets_the_streak_when_an_engine_survives_the_window() {
      let mut breaker = Breaker::default();
      for _ in 0..RESPAWN_BREAKER_THRESHOLD - 1 {
        assert!(breaker.record_restart(Duration::ZERO).is_some());
      }

      let after_a_long_run = breaker.record_restart(RESPAWN_BREAKER_WINDOW);

      assert!(
        after_a_long_run.is_some(),
        "an engine that lived past the window resets the streak, so the next death restarts rather than trips"
      );
    }

    #[test]
    fn it_trips_and_gives_up_after_the_threshold_of_rapid_deaths() {
      let mut breaker = Breaker::default();

      let plans: Vec<_> = (0..RESPAWN_BREAKER_THRESHOLD)
        .map(|_| breaker.record_restart(Duration::ZERO))
        .collect();

      let restarts = plans.iter().take_while(|plan| plan.is_some()).count();

      assert_eq!(
        restarts,
        (RESPAWN_BREAKER_THRESHOLD - 1) as usize,
        "the supervisor restarts up to one shy of the threshold, then trips"
      );
      assert!(
        plans.last().expect("at least one plan").is_none(),
        "the threshold-th rapid death trips the breaker and stops restarting"
      );
    }
  }

  mod disposition {
    use super::*;

    #[test]
    fn it_restarts_on_a_panic_payload() {
      let restart = super::super::disposition(Err(Box::new("boom")));

      assert!(matches!(restart, Disposition::Restart));
    }

    #[test]
    fn it_stops_deliberately_on_shutdown() {
      let stop = super::super::disposition(Ok(ExitReason::Shutdown));

      assert!(matches!(
        stop,
        Disposition::Stop {
          deliberate: true
        }
      ));
    }

    #[test]
    fn it_stops_without_deliberation_on_channel_close() {
      let stop = super::super::disposition(Ok(ExitReason::ChannelClosed));

      assert!(matches!(
        stop,
        Disposition::Stop {
          deliberate: false
        }
      ));
    }
  }

  mod drain {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      store::{model::OwnerType, repo::infra},
      sync::outbox::{OutboxKind, test_support::*},
    };

    async fn spawn_engine_with_drain(base_url: String) -> (Database, StubCalls, tempfile::TempDir) {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url.clone()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), base_url));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let calls = StubCalls::default();
      let registry_calls = calls.clone();
      let (_handle, _events) = spawn_with_registry(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
        move || Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, registry_calls.clone()))),
      );
      (db, calls, images_dir)
    }

    async fn spawn_engine_with_drain_and_handle(
      base_url: String,
    ) -> (Handle, mpsc::Receiver<Event>, Database, StubCalls, tempfile::TempDir) {
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url.clone()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), base_url));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let calls = StubCalls::default();
      let registry_calls = calls.clone();
      let (handle, events) = spawn_with_registry(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
        move || Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, registry_calls.clone()))),
      );
      (handle, events, db, calls, images_dir)
    }

    async fn spawn_engine_over_db(
      db: &Database,
      base_url: String,
    ) -> (Handle, mpsc::Receiver<Event>, StubCalls, tempfile::TempDir) {
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), base_url.clone()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), base_url));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let calls = StubCalls::default();
      let registry_calls = calls.clone();
      let (handle, events) = spawn_with_registry(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
        move || Registry::new().with(Box::new(StubHandler::new(OutboxKind::MailSend, registry_calls.clone()))),
      );
      (handle, events, calls, images_dir)
    }

    async fn status_of(db: &Database, id: i64) -> String {
      sqlx::query_scalar::<_, String>("SELECT status FROM outbox WHERE id = ?")
        .bind(id)
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    async fn wait_for_status(db: &Database, id: i64, want: &str) {
      let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
      loop {
        if status_of(db, id).await == want {
          return;
        }
        if tokio::time::Instant::now() >= deadline {
          panic!(
            "outbox row {id} never reached status {want:?} (was {:?})",
            status_of(db, id).await
          );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
      }
    }

    #[tokio::test]
    async fn it_does_not_re_execute_an_already_done_row_after_restart() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      infra::upsert(&db, 7400, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      let row = infra::append(
        &db,
        OwnerType::Character,
        7400,
        "mail.send",
        "{\"body\":\"sent\"}",
        None,
      )
      .await
      .unwrap();
      sqlx::query("UPDATE outbox SET status = 'done', updated_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(row.id())
        .execute(db.writer())
        .await
        .unwrap();

      let (_handle, _events, calls, _images) = spawn_engine_over_db(&db, server.uri()).await;
      tokio::time::sleep(Duration::from_millis(200)).await;

      assert_eq!(
        calls.executes(),
        0,
        "a terminal done row must never be re-claimed or re-executed by a restarted engine"
      );
      assert_eq!(
        status_of(&db, row.id()).await,
        "done",
        "the done row stays done; replay is a no-op"
      );
    }

    #[tokio::test]
    async fn it_drains_a_durable_row_after_a_kill_and_restart() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();

      let row = infra::append(
        &db,
        OwnerType::Character,
        7300,
        "mail.send",
        "{\"body\":\"queued\"}",
        None,
      )
      .await
      .unwrap();

      {
        let (handle, events, first_calls, _images) = spawn_engine_over_db(&db, server.uri()).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let undrained = status_of(&db, row.id()).await;
        assert!(
          undrained == "pending" || undrained == "inflight",
          "the un-credentialed row must stay drainable while the first engine lives (was {undrained:?})"
        );
        assert_eq!(
          first_calls.executes(),
          0,
          "the first engine must not perform an ESI write for a subject it cannot authenticate"
        );
        drop(handle);
        drop(events);
      }

      infra::upsert(&db, 7300, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      let (_handle, _events, calls, _images) = spawn_engine_over_db(&db, server.uri()).await;

      wait_for_status(&db, row.id(), "done").await;
      assert_eq!(
        calls.executes(),
        1,
        "the restarted engine performs the queued ESI write exactly once"
      );
    }

    #[tokio::test]
    async fn it_drains_a_pre_seeded_credentialed_row_to_done_on_tick() {
      let server = MockServer::start().await;
      let (db, calls, _images) = spawn_engine_with_drain(server.uri()).await;
      infra::upsert(&db, 8800, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      let owned = infra::append(&db, OwnerType::Character, 8800, "mail.send", "{\"body\":\"hi\"}", None)
        .await
        .unwrap();
      let unowned = infra::append(&db, OwnerType::Character, 8801, "mail.send", "{\"body\":\"hi\"}", None)
        .await
        .unwrap();

      wait_for_status(&db, owned.id(), "done").await;

      assert_eq!(
        calls.executes(),
        1,
        "the credentialed row's ESI write should run exactly once"
      );
      let unowned_status = status_of(&db, unowned.id()).await;
      assert!(
        unowned_status == "pending" || unowned_status == "inflight",
        "an un-credentialed subject's row must stay drainable, never done or failed (was {unowned_status:?})"
      );
    }

    #[tokio::test]
    async fn it_drains_a_row_on_a_control_channel_nudge() {
      let server = MockServer::start().await;
      let (handle, _events, db, calls, _images) = spawn_engine_with_drain_and_handle(server.uri()).await;
      infra::upsert(&db, 9900, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      let row = infra::append(&db, OwnerType::Character, 9900, "mail.send", "{\"body\":\"hi\"}", None)
        .await
        .unwrap();

      handle.drain();

      wait_for_status(&db, row.id(), "done").await;
      assert!(calls.executes() >= 1, "the nudged drain ran the row's ESI write");
    }

    #[tokio::test]
    async fn it_prunes_done_rows_past_the_retention_window_on_tick() {
      let server = MockServer::start().await;
      let (db, _calls, _images) = spawn_engine_with_drain(server.uri()).await;
      let stale = infra::append(&db, OwnerType::Character, 8900, "mail.send", "{}", None)
        .await
        .unwrap();
      let fresh = infra::append(&db, OwnerType::Character, 8901, "mail.send", "{}", None)
        .await
        .unwrap();
      let long_ago = (Utc::now() - ChronoDuration::days(1)).to_rfc3339();
      let now = Utc::now().to_rfc3339();
      sqlx::query("UPDATE outbox SET status = 'done', updated_at = ? WHERE id = ?")
        .bind(&long_ago)
        .bind(stale.id())
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("UPDATE outbox SET status = 'done', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(fresh.id())
        .execute(db.writer())
        .await
        .unwrap();

      let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
      loop {
        let remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE id = ?")
          .bind(stale.id())
          .fetch_one(&db.0)
          .await
          .unwrap();
        if remaining == 0 {
          break;
        }
        if tokio::time::Instant::now() >= deadline {
          panic!("stale done row was never pruned");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
      }

      assert_eq!(
        status_of(&db, fresh.id()).await,
        "done",
        "a done row inside the retention window must be retained for the indicator"
      );
    }

    #[tokio::test]
    async fn it_redrains_a_row_stuck_inflight_by_a_crashed_engine() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      infra::upsert(&db, 7500, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      let row = infra::append(&db, OwnerType::Character, 7500, "mail.send", "{\"body\":\"mid\"}", None)
        .await
        .unwrap();
      sqlx::query("UPDATE outbox SET status = 'inflight', next_attempt_at = ?, updated_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .bind(row.id())
        .execute(db.writer())
        .await
        .unwrap();

      let (_handle, _events, calls, _images) = spawn_engine_over_db(&db, server.uri()).await;

      wait_for_status(&db, row.id(), "done").await;
      assert_eq!(
        calls.executes(),
        1,
        "the fresh engine re-claims the stuck inflight row and performs its ESI write"
      );
    }
  }

  mod exit_reason {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_channel_closed_when_the_command_sender_is_dropped() {
      let server = MockServer::start().await;
      let (commands, run) = spawn_run(server.uri()).await;

      drop(commands);
      let reason = tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .expect("the run loop should exit promptly once the command channel closes")
        .expect("the run task should not panic");

      assert_eq!(
        reason,
        ExitReason::ChannelClosed,
        "dropping the only command sender closes the channel and yields ExitReason::ChannelClosed"
      );
      assert!(
        !reason.is_deliberate(),
        "a channel close is an unexpected exit, not a deliberate shutdown"
      );
    }

    #[tokio::test]
    async fn it_returns_shutdown_when_a_shutdown_command_breaks_the_loop() {
      let server = MockServer::start().await;
      let (commands, run) = spawn_run(server.uri()).await;

      commands.send(Command::Shutdown).unwrap();
      let reason = tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .expect("the run loop should exit promptly on shutdown")
        .expect("the run task should not panic");

      assert_eq!(
        reason,
        ExitReason::Shutdown,
        "a deliberate Shutdown command yields the deliberate ExitReason::Shutdown"
      );
      assert!(
        reason.is_deliberate(),
        "Shutdown is a deliberate exit, not an unexpected one"
      );
    }
  }

  mod global_kinds {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_the_complement_of_the_per_subject_set() {
      let global = super::super::global_kinds();
      for kind in JobKind::ALL.iter().copied() {
        let per_subject = kind.applies_to(Subject::Character(1)) || kind.applies_to(Subject::Corporation(1));
        assert_eq!(
          global.contains(&kind),
          !per_subject,
          "{kind:?} must be global iff it applies to no real subject"
        );
      }
    }

    #[test]
    fn it_selects_kinds_that_apply_to_no_real_subject() {
      let kinds = super::super::global_kinds();

      assert!(
        kinds.contains(&JobKind::NetWorthSnapshot),
        "the global lane must include the net-worth snapshot, got {kinds:?}"
      );
      for kind in &kinds {
        assert!(
          !kind.applies_to(Subject::Character(1)) && !kind.applies_to(Subject::Corporation(1)),
          "{kind:?} applies to a real subject, so it belongs to per-subject discovery, not the global lane"
        );
      }
      assert!(!kinds.contains(&JobKind::CharacterProfile));
      assert!(!kinds.contains(&JobKind::CorporationWallet));
    }
  }

  mod global_lane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{model::OwnerType, repo::finance};

    async fn insert_journal(db: &Database, id: i64, character_id: i64, balance: f64) {
      sqlx::query(
        "INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) \
        VALUES (?, ?, ?, ?, ?, ?, ?)",
      )
      .bind(id)
      .bind(character_id)
      .bind("2026-01-01")
      .bind("Test")
      .bind("test")
      .bind(balance)
      .bind(balance)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_does_not_enroll_the_snapshot_when_wallet_and_assets_are_off() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 7701).await;
      infra::upsert(&db, 7701, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      insert_journal(&db, 1, 7701, 999.0).await;
      // Net worth is assets + wallet, so the snapshot serves either group; only with BOTH off does it
      // stop enrolling.
      let flags: FeatureFlags = toml::from_str("wallet = false\nasset_tracking = false").unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        flags,
      );

      let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
      while let Ok(Some(event)) = tokio::time::timeout_at(deadline, events.recv()).await {
        if let Event::Started {
          key,
        }
        | Event::Finished {
          key, ..
        } = event
        {
          assert_ne!(
            key.kind,
            JobKind::NetWorthSnapshot,
            "the snapshot lane must not enroll while both Wallet and Assets are off"
          );
        }
      }

      let rows = finance::for_character_since(&db, 7701, "2000-01-01").await.unwrap();
      assert!(
        rows.is_empty(),
        "no snapshot should be written while both Wallet and Assets are off"
      );
    }

    #[tokio::test]
    async fn it_forces_a_net_worth_snapshot_immediately_after_a_wallet_sync() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/7702/wallet/journal/",
        serde_json::json!([
          { "amount": 1000.0, "balance": 50_000.0, "date": "2026-05-30T12:00:00Z", "description": "Donation",
            "id": 123_456_789_i64, "ref_type": "player_donation" },
        ]),
      )
      .await;
      mount_json(&server, "/characters/7702/wallet/transactions/", serde_json::json!([])).await;
      mount_json(&server, "/markets/prices/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 7702).await;
      infra::upsert(
        &db,
        7702,
        OwnerType::Character,
        "tok",
        "rt",
        4_102_444_800,
        None,
        Some(esi::scopes::CHARACTER_WALLET),
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      wait_for(
        &mut events,
        |event| matches!(event, Event::Finished { key, .. } if key.kind == JobKind::CharacterWallet),
      )
      .await;
      wait_for(
        &mut events,
        |event| matches!(event, Event::Finished { key, .. } if key.kind == JobKind::NetWorthSnapshot),
      )
      .await;

      let rows = finance::for_character_since(&db, 7702, "2000-01-01").await.unwrap();
      assert_eq!(
        rows.len(),
        2,
        "the forced snapshot writes today's row plus the backfilled journal day"
      );
      assert_eq!(rows[0].liquid(), 50_000.0);
    }

    #[tokio::test]
    async fn it_runs_the_global_net_worth_snapshot_at_startup() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 7700).await;
      infra::upsert(&db, 7700, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      insert_journal(&db, 1, 7700, 1_234.0).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      wait_for(
        &mut events,
        |event| matches!(event, Event::Finished { key, .. } if key.kind == JobKind::NetWorthSnapshot),
      )
      .await;

      let rows = finance::for_character_since(&db, 7700, "2000-01-01").await.unwrap();
      assert_eq!(
        rows.len(),
        2,
        "the global lane writes today's snapshot row plus the backfilled journal day"
      );
      assert_eq!(rows[0].liquid(), 1_234.0);
      assert_eq!(rows[0].net_worth(), 1_234.0);
    }
  }

  mod hydrate {
    use super::*;

    async fn seed_ledger(db: &Database, kind: &str, next_eligible_at: Option<&str>, last_success_at: Option<&str>) {
      sync_ledger::upsert(
        db,
        OwnerType::Character,
        0,
        kind,
        "synced",
        0,
        None,
        last_success_at,
        next_eligible_at,
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_breaks_the_run_loop_on_a_shutdown_command() {
      let server = MockServer::start().await;
      let (handle, mut events, _db, _images) = spawn_engine(server.uri()).await;

      handle.shutdown();

      let drained =
        tokio::time::timeout(Duration::from_secs(5), async { while events.recv().await.is_some() {} }).await;

      assert!(
        drained.is_ok(),
        "shutdown breaks the loop, dropping the engine and closing its event channel"
      );
    }

    #[tokio::test]
    async fn it_does_not_rerun_a_job_whose_ledger_eligibility_is_in_the_future() {
      let server = MockServer::start().await;
      mount_json(&server, "/markets/prices/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      let future = (Utc::now() + ChronoDuration::minutes(30)).to_rfc3339();
      seed_ledger(&db, "IndustryCostIndices", Some(&future), None).await;
      seed_ledger(&db, "KillmailDetailBackfill", Some(&future), None).await;
      seed_ledger(&db, "KillmailReconcile", Some(&future), None).await;
      seed_ledger(&db, "MarketPrices", Some(&future), None).await;
      seed_ledger(&db, "NetWorthSnapshot", Some(&future), None).await;
      seed_ledger(&db, "TokenAudit", Some(&future), None).await;

      let (_handle, mut events, _images) = spawn_engine_with_db(db, server.uri()).await;

      let started = tokio::time::timeout(Duration::from_millis(300), async {
        loop {
          match events.recv().await {
            Some(Event::Started {
              key,
            }) => return Some(key),
            Some(_) => continue,
            None => return None,
          }
        }
      })
      .await;
      assert!(
        started.is_err(),
        "a job seeded with a future ledger eligibility must not run at startup, got {started:?}"
      );
    }

    #[tokio::test]
    async fn it_falls_back_to_last_success_plus_interval_when_next_eligible_is_absent() {
      let db = store::open_test().await.unwrap();
      let recent_success = (Utc::now() - ChronoDuration::minutes(1)).to_rfc3339();
      seed_ledger(&db, "CharacterProfile", None, Some(&recent_success)).await;
      let row = sync_ledger::get(&db, OwnerType::Character, 0, "CharacterProfile")
        .await
        .unwrap()
        .unwrap();

      assert!(
        future_seed(&row, JobKind::CharacterProfile, Instant::now()).is_some(),
        "a recent success within one interval defers the next run even without an explicit next-eligible"
      );
    }

    #[tokio::test]
    async fn it_reruns_a_job_whose_ledger_eligibility_has_passed() {
      let server = MockServer::start().await;
      mount_json(&server, "/markets/prices/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      let past = (Utc::now() - ChronoDuration::hours(1)).to_rfc3339();
      seed_ledger(&db, "MarketPrices", Some(&past), None).await;

      let (_handle, mut events, _images) = spawn_engine_with_db(db, server.uri()).await;

      wait_for(
        &mut events,
        |event| matches!(event, Event::Started { key } if key.kind == JobKind::MarketPrices),
      )
      .await;
    }

    #[tokio::test]
    async fn it_seeds_a_future_eligible_row_within_one_interval() {
      let db = store::open_test().await.unwrap();
      let future = (Utc::now() + ChronoDuration::minutes(30)).to_rfc3339();
      seed_ledger(&db, "CharacterProfile", Some(&future), None).await;
      let row = sync_ledger::get(&db, OwnerType::Character, 0, "CharacterProfile")
        .await
        .unwrap()
        .unwrap();
      let now = Instant::now();

      let seed = future_seed(&row, JobKind::CharacterProfile, now).expect("a future row seeds a deferred next-run");

      assert!(seed > now, "the seed defers the next run past now");
      assert!(
        seed <= now + JobKind::CharacterProfile.interval(),
        "the seed is clamped to at most one interval"
      );
    }

    #[tokio::test]
    async fn it_yields_no_seed_for_a_stale_row() {
      let db = store::open_test().await.unwrap();
      let past = (Utc::now() - ChronoDuration::hours(1)).to_rfc3339();
      seed_ledger(&db, "CharacterProfile", Some(&past), None).await;
      let row = sync_ledger::get(&db, OwnerType::Character, 0, "CharacterProfile")
        .await
        .unwrap()
        .unwrap();

      assert!(
        future_seed(&row, JobKind::CharacterProfile, Instant::now()).is_none(),
        "a past eligibility leaves the kind due now"
      );
    }
  }

  mod is_pending_outcome {
    use super::*;

    #[test]
    fn it_does_not_treat_a_benign_empty_or_synced_outcome_as_pending() {
      assert!(
        !super::super::is_pending_outcome(&Outcome::Empty),
        "a benign empty result is a success, not a pending retry"
      );
      assert!(!super::super::is_pending_outcome(&Outcome::Synced {
        rows_touched: 3
      }));
    }

    #[test]
    fn it_treats_blocked_and_not_ready_as_pending() {
      assert!(super::super::is_pending_outcome(&Outcome::Blocked {
        reason: "no scope".to_owned()
      }));
      assert!(super::super::is_pending_outcome(&Outcome::NotReady));
    }
  }

  mod is_permanent_failure {
    use super::*;

    /// Builds a real `Error::Http` carrying the given status by exercising reqwest against a server
    /// that returns it, so the classifier is tested against an authentic error rather than a mock.
    async fn http_error_with_status(status: u16) -> Error {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(status))
        .mount(&server)
        .await;
      let resp = reqwest::get(format!("{}/x", server.uri())).await.unwrap();
      Error::Http(resp.error_for_status().unwrap_err())
    }

    #[test]
    fn it_classifies_a_missing_authorizing_character_as_permanent() {
      let error = Error::Internal("corporation credential for 1 has no authorizing character".to_owned());

      assert!(super::super::is_permanent_failure(&error));
    }

    #[test]
    fn it_classifies_a_missing_credential_auth_error_as_permanent() {
      let error = Error::Auth("CharacterWallet for Character(7) has no usable credential".to_owned());

      assert!(super::super::is_permanent_failure(&error));
    }

    #[test]
    fn it_classifies_a_needs_reauthentication_failure_as_permanent() {
      let error = Error::Internal(
        "authorizing character 5 no longer holds an accounting role; needs re-authentication".to_owned(),
      );

      assert!(super::super::is_permanent_failure(&error));
    }

    #[tokio::test]
    async fn it_classifies_a_revoked_token_refresh_400_as_permanent() {
      let error = http_error_with_status(400).await;

      assert!(
        super::super::is_permanent_failure(&error),
        "a revoked refresh token surfaces as HTTP 400 and must park rather than backoff-hammer"
      );
    }

    #[tokio::test]
    async fn it_does_not_classify_a_transient_500_as_permanent() {
      let error = http_error_with_status(500).await;

      assert!(
        !super::super::is_permanent_failure(&error),
        "a server-side 500 is transient and must keep retrying on backoff, not park"
      );
    }

    #[test]
    fn it_does_not_classify_a_transient_failure_as_permanent() {
      assert!(!super::super::is_permanent_failure(&Error::NotReady));
      assert!(!super::super::is_permanent_failure(&Error::Internal(
        "write logo failed".to_owned()
      )));
    }
  }

  mod run {
    use super::*;

    #[tokio::test]
    async fn it_backs_off_when_esi_rate_limits() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/9/"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "30"))
        .mount(&server)
        .await;
      let (handle, mut events, db, _images) = spawn_engine(server.uri()).await;

      handle.enroll(Subject::Character(9));

      let backing_off = wait_for(&mut events, |event| matches!(event, Event::BackingOff { .. })).await;
      assert!(matches!(
        backing_off,
        Event::BackingOff {
          retry_secs: 30,
          ..
        }
      ));
      assert!(character::get(&db, 9).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_defers_a_gather_job_whose_parent_row_is_absent_until_the_profile_commits() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/5000/wallet/journal/"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
      Mock::given(method("GET"))
        .and(path("/characters/5000/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      infra::upsert(
        &db,
        5000,
        OwnerType::Character,
        "tok",
        "rt",
        4_102_444_800,
        None,
        Some(esi::scopes::CHARACTER_WALLET),
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db,
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      let wallet = JobKey::new(JobKind::CharacterWallet, Subject::Character(5000));
      let started = tokio::time::timeout(Duration::from_millis(300), async {
        while let Some(event) = events.recv().await {
          if matches!(event, Event::Started { key } if key == wallet) {
            return true;
          }
        }
        false
      })
      .await;
      assert!(
        started.is_err() || !started.unwrap(),
        "a parentless subject's wallet gather must stay deferred — it must not dispatch before the profile commits"
      );
    }

    #[tokio::test]
    async fn it_discovers_credentialed_subjects_without_an_enroll_command() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      infra::upsert(&db, 55, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db,
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      let started = wait_for(
        &mut events,
        |event| matches!(event, Event::Started { key } if key.subject == Subject::Character(55)),
      )
      .await;
      assert!(matches!(started, Event::Started { key } if key.subject == Subject::Character(55)));
    }

    #[tokio::test]
    async fn it_enrolls_a_newly_credentialed_subject_on_a_discover_command() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/6161/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (handle, mut events) = spawn(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      infra::upsert(&db, 6161, OwnerType::Character, "tok", "rt", 4_102_444_800, None, None)
        .await
        .unwrap();
      handle.discover();

      let started = wait_for(
        &mut events,
        |event| matches!(event, Event::Started { key } if key.subject == Subject::Character(6161)),
      )
      .await;
      assert!(
        matches!(started, Event::Started { key } if key.subject == Subject::Character(6161)),
        "a Discover command must enroll a credential added after startup without waiting for the heartbeat"
      );
    }

    #[tokio::test]
    async fn it_globally_pauses_dispatch_on_an_esi_error_limit() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/10/"))
        .respond_with(ResponseTemplate::new(420).insert_header("X-ESI-Error-Limit-Reset", "30"))
        .mount(&server)
        .await;
      let (handle, mut events, _db, _images) = spawn_engine(server.uri()).await;

      handle.enroll(Subject::Character(10));

      let backing_off = wait_for(&mut events, |event| {
        matches!(
          event,
          Event::BackingOff { key, retry_secs: 30 }
            if *key == JobKey::new(JobKind::CharacterProfile, Subject::Character(10))
        )
      })
      .await;
      let Event::BackingOff {
        key: limited, ..
      } = backing_off
      else {
        unreachable!("the predicate only matches a BackingOff event");
      };

      let redispatched = tokio::time::timeout(Duration::from_millis(300), async {
        while let Some(event) = events.recv().await {
          if matches!(event, Event::Started { key } if key == limited) {
            return;
          }
        }
      })
      .await;
      assert!(
        redispatched.is_err(),
        "the error-limited job re-dispatched during the governor window"
      );
    }

    #[tokio::test]
    async fn it_records_a_failure_outcome_without_a_success_timestamp() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/7/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let (handle, mut events, db, _images) = spawn_engine(server.uri()).await;

      handle.enroll(Subject::Character(7));

      wait_for(
        &mut events,
        |event| {
          matches!(event, Event::Scheduled { key, .. } if key == &JobKey::new(JobKind::CharacterProfile, Subject::Character(7)))
        },
      )
      .await;

      let row = sync_ledger::get(&db, OwnerType::Character, 7, "CharacterProfile")
        .await
        .unwrap()
        .expect("a failed job still records a ledger row");
      assert_eq!(row.outcome(), "failed");
      assert!(row.last_reason().is_some(), "a failure records why it failed");
      assert!(
        row.last_success_at().is_none(),
        "a failure must not stamp last_success_at"
      );
    }

    #[tokio::test]
    async fn it_records_a_synced_ledger_row_when_a_job_finishes() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/markets/prices/",
        serde_json::json!([{ "adjusted_price": 1.0, "average_price": 1.0, "type_id": 34 }]),
      )
      .await;
      let (_handle, mut events, db, _images) = spawn_engine(server.uri()).await;

      wait_for(
        &mut events,
        |event| matches!(event, Event::Scheduled { key, .. } if key.kind == JobKind::MarketPrices),
      )
      .await;

      let row = sync_ledger::get(&db, OwnerType::Character, 0, "MarketPrices")
        .await
        .unwrap()
        .expect("a finished job records a ledger row");
      assert_eq!(row.outcome(), "synced");
      assert!(!row.last_attempt_at().is_empty());
      assert!(
        row.last_success_at().is_some(),
        "a synced terminal stamps last_success_at"
      );
      assert!(
        row.next_eligible_at().is_some(),
        "the ledger carries the scheduled next-eligible time"
      );
    }

    #[tokio::test]
    async fn it_reruns_a_subject_on_run_now() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/30/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let (handle, mut events, _db, _images) = spawn_engine(server.uri()).await;

      handle.enroll(Subject::Character(30));
      wait_for(&mut events, |event| matches!(event, Event::Failed { .. })).await;

      handle.run_now(Subject::Character(30));

      let rerun = wait_for(
        &mut events,
        |event| matches!(event, Event::Started { key } if key.subject == Subject::Character(30)),
      )
      .await;
      assert!(matches!(rerun, Event::Started { key } if key.subject == Subject::Character(30)));
    }

    #[tokio::test]
    async fn it_reschedules_an_empty_outcome_at_its_normal_interval() {
      let server = MockServer::start().await;
      mount_json(&server, "/markets/prices/", serde_json::json!([])).await;
      let (_handle, mut events, _db, _images) = spawn_engine(server.uri()).await;

      let scheduled = wait_for(
        &mut events,
        |event| matches!(event, Event::Scheduled { key, .. } if key.kind == JobKind::MarketPrices),
      )
      .await;
      let Event::Scheduled {
        next_in_secs, ..
      } = scheduled
      else {
        unreachable!()
      };

      assert!(
        JobKind::MarketPrices.interval().as_secs() > PENDING_RETRY.as_secs(),
        "the pending retry is meaningfully shorter than the job's normal interval"
      );
      assert!(
        next_in_secs > PENDING_RETRY.as_secs(),
        "a benign empty result settles to the normal interval, not the pending window, got {next_in_secs}s"
      );
    }

    #[tokio::test]
    async fn it_runs_a_gather_for_an_existing_subject_without_waiting_for_a_fresh_profile() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/4478/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      mount_json(&server, "/characters/4478/wallet/journal/", serde_json::json!([])).await;
      mount_json(&server, "/characters/4478/wallet/transactions/", serde_json::json!([])).await;
      mount_json(&server, "/markets/prices/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 4478).await;
      infra::upsert(
        &db,
        4478,
        OwnerType::Character,
        "tok",
        "rt",
        4_102_444_800,
        None,
        Some(esi::scopes::CHARACTER_WALLET),
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db,
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      let wallet = JobKey::new(JobKind::CharacterWallet, Subject::Character(4478));
      let mut profile_succeeded = false;
      let ran = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
          match events.recv().await {
            Some(Event::Started {
              key,
            }) if key == wallet => return true,
            Some(Event::Finished {
              key, ..
            }) if key.kind == JobKind::CharacterProfile && key.subject == Subject::Character(4478) => {
              profile_succeeded = true;
            }
            Some(_) => continue,
            None => return false,
          }
        }
      })
      .await
      .expect("timed out waiting for the wallet gather to run");

      assert!(
        ran,
        "an existing subject's wallet gather must run on its own schedule, not wait for a profile trigger"
      );
      assert!(
        !profile_succeeded,
        "the profile job fails for this subject, so the gather ran without any fresh profile success"
      );
    }

    #[tokio::test]
    async fn it_runs_a_new_subjects_gather_only_after_the_profile_commits_its_parent_row() {
      let server = MockServer::start().await;
      mount_character_profile(&server, 101).await;
      mount_json(&server, "/characters/101/wallet/journal/", serde_json::json!([])).await;
      mount_json(&server, "/characters/101/wallet/transactions/", serde_json::json!([])).await;
      mount_json(&server, "/markets/prices/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      infra::upsert(
        &db,
        101,
        OwnerType::Character,
        "tok",
        "rt",
        4_102_444_800,
        None,
        Some(esi::scopes::CHARACTER_WALLET),
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (handle, mut events) = spawn(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );
      seed_ship_type(&db).await;

      handle.enroll(Subject::Character(101));

      let wallet = JobKey::new(JobKind::CharacterWallet, Subject::Character(101));
      let profile = JobKey::new(JobKind::CharacterProfile, Subject::Character(101));
      let mut profile_committed = false;
      let ordered = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
          match events.recv().await {
            Some(Event::Started {
              key,
            }) if key == wallet => return profile_committed,
            Some(Event::Finished {
              key, ..
            }) if key == profile => profile_committed = true,
            Some(_) => continue,
            None => return false,
          }
        }
      })
      .await
      .expect("timed out waiting for the wallet gather to start");

      assert!(
        ordered,
        "the wallet gather for a brand-new subject must not start until CharacterProfile has committed the parent row"
      );
    }

    #[tokio::test]
    async fn it_runs_an_enrolled_job_and_reports_failure_without_persisting() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/7/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let (handle, mut events, db, _images) = spawn_engine(server.uri()).await;

      handle.enroll(Subject::Character(7));

      let started = wait_for(
        &mut events,
        |event| matches!(event, Event::Started { key } if key.subject == Subject::Character(7)),
      )
      .await;
      assert!(matches!(started, Event::Started { key } if key.subject == Subject::Character(7)));
      wait_for(
        &mut events,
        |event| matches!(event, Event::Failed { key, .. } if key.subject == Subject::Character(7)),
      )
      .await;
      assert!(character::get(&db, 7).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_runs_character_skills_and_lands_skills_when_wallet_off_and_skill_monitoring_on() {
      let server = MockServer::start().await;
      mount_skill_picture(&server, 4242).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 4242).await;
      infra::upsert(
        &db,
        4242,
        OwnerType::Character,
        "tok",
        "rt",
        4_102_444_800,
        None,
        Some(esi::scopes::CHARACTER_SKILLS),
      )
      .await
      .unwrap();
      let flags: FeatureFlags = toml::from_str("wallet = false\nskill_monitoring = true").unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db.clone(),
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        flags,
      );

      let terminal = wait_for(&mut events, |event| {
        matches!(
          event,
          Event::Finished { key, .. } | Event::Failed { key, .. } if key.kind == JobKind::CharacterSkills
        )
      })
      .await;

      assert!(
        matches!(terminal, Event::Finished { .. }),
        "expected CharacterSkills to finish, got {terminal:?}"
      );
      let skills = crate::store::repo::character::skills(&db, 4242).await.unwrap();
      assert_eq!(skills.len(), 1, "the skill sheet should be persisted");
      let queue = crate::store::repo::character::skillqueue(&db, 4242).await.unwrap();
      assert_eq!(queue.len(), 1, "the skill queue should be persisted");
    }

    #[tokio::test]
    async fn it_runs_jobs_for_multiple_characters_concurrently() {
      let server = MockServer::start().await;
      for id in [41_i64, 42] {
        Mock::given(method("GET"))
          .and(path(format!("/characters/{id}/")))
          .respond_with(ResponseTemplate::new(500).set_delay(Duration::from_millis(300)))
          .mount(&server)
          .await;
      }
      let (handle, mut events, _db, _images) = spawn_engine(server.uri()).await;

      handle.enroll(Subject::Character(41));
      handle.enroll(Subject::Character(42));

      let mut profiles_started = HashSet::new();
      loop {
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
          .await
          .expect("timed out waiting for both profile jobs to start")
          .expect("event channel closed");
        match event {
          Event::Started {
            key,
          } if key.kind == JobKind::CharacterProfile => {
            profiles_started.insert(key.subject);
            if profiles_started.len() == 2 {
              break;
            }
          }
          Event::Finished {
            key, ..
          }
          | Event::Failed {
            key, ..
          } if key.kind == JobKind::CharacterProfile => {
            panic!("a profile job reached a terminal before both started — jobs ran serially: {key:?}");
          }
          _ => {}
        }
      }

      assert_eq!(profiles_started.len(), 2);
    }

    #[tokio::test]
    async fn it_schedules_only_jobs_whose_scopes_a_partial_grant_covers() {
      let server = MockServer::start().await;
      mount_json(&server, "/characters/4477/wallet/journal/", serde_json::json!([])).await;
      mount_json(&server, "/characters/4477/wallet/transactions/", serde_json::json!([])).await;
      mount_json(&server, "/markets/prices/", serde_json::json!([])).await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 4477).await;
      infra::upsert(
        &db,
        4477,
        OwnerType::Character,
        "tok",
        "rt",
        4_102_444_800,
        None,
        Some(esi::scopes::CHARACTER_WALLET),
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db,
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      let mut kinds = Vec::new();
      let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
      while let Ok(Some(event)) = tokio::time::timeout_at(deadline, events.recv()).await {
        if let Event::Started {
          key,
        }
        | Event::Failed {
          key, ..
        } = event
          && key.subject == Subject::Character(4477)
        {
          kinds.push(key.kind);
        }
      }

      assert!(
        kinds.contains(&JobKind::CharacterWallet),
        "the granted wallet scope schedules CharacterWallet, got {kinds:?}"
      );
      assert!(
        !kinds.contains(&JobKind::CharacterMarketOrders),
        "the ungranted market-orders scope must never schedule (no permanent 401 loop), got {kinds:?}"
      );
    }

    #[tokio::test]
    async fn it_schedules_only_public_jobs_for_a_non_owned_character() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/3004069/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      seed_character(&db, 3004069).await;
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let (_handle, mut events) = spawn(
        db.clone(),
        db,
        esi,
        sso,
        image,
        images::Store::new(images_dir.path().to_path_buf()),
        FeatureFlags::default(),
      );

      let mut kinds = Vec::new();
      let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
      while let Ok(Some(event)) = tokio::time::timeout_at(deadline, events.recv()).await {
        if let Event::Started {
          key,
        }
        | Event::Failed {
          key, ..
        } = event
          && key.subject == Subject::Character(3004069)
        {
          kinds.push(key.kind);
        }
      }

      assert!(
        kinds.contains(&JobKind::CharacterProfile),
        "the non-owned character's public profile job should still run, got {kinds:?}"
      );
      assert!(
        !kinds.contains(&JobKind::CharacterTelemetry) && !kinds.contains(&JobKind::CharacterWallet),
        "no privileged job should ever be scheduled for a non-owned character, got {kinds:?}"
      );
    }

    #[tokio::test]
    async fn it_surfaces_a_privileged_job_without_a_credential_as_needs_reauth() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = Arc::new(esi::Client::with_base_url(http.clone(), server.uri()));
      let image = Arc::new(eve_image::Client::with_base_url(http.clone(), server.uri()));
      let sso = Arc::new(eve_sso::Client::new(http, "test-client"));
      let images_dir = tempfile::tempdir().unwrap();
      let store = images::Store::new(images_dir.path().to_path_buf());
      let key = JobKey::new(JobKind::CharacterWallet, Subject::Character(7));

      let result = run_job(&db, &esi, &sso, &image, &store, key).await;

      assert!(
        matches!(result, Err(Error::Auth(_))),
        "a privileged job with no usable credential must surface as needs-reauth, not a clean Finished"
      );
    }

    #[tokio::test]
    async fn it_syncs_an_enrolled_character_end_to_end() {
      let server = MockServer::start().await;
      mount_json(
        &server,
        "/characters/100/",
        serde_json::json!({
          "alliance_id": 300,
          "birthday": "2010-01-01T00:00:00Z",
          "bloodline_id": 5,
          "corporation_id": 200,
          "gender": "male",
          "name": "Test Pilot",
          "race_id": 1,
        }),
      )
      .await;
      mount_json(
        &server,
        "/corporations/200/",
        serde_json::json!({
          "alliance_id": 300, "ceo_id": 100, "creator_id": 100, "member_count": 42,
          "name": "Test Corp", "tax_rate": 0.1, "ticker": "TST",
        }),
      )
      .await;
      mount_json(
        &server,
        "/alliances/300/",
        serde_json::json!({
          "creator_corporation_id": 200, "creator_id": 100,
          "date_founded": "2005-01-01T00:00:00Z", "name": "Test Alliance", "ticker": "TSTA",
        }),
      )
      .await;
      mount_json(
        &server,
        "/universe/races/",
        serde_json::json!([
          { "alliance_id": 300, "description": "The Caldari.", "name": "Caldari", "race_id": 1 },
        ]),
      )
      .await;
      mount_json(
        &server,
        "/universe/bloodlines/",
        serde_json::json!([
          { "bloodline_id": 5, "charisma": 6, "corporation_id": 200, "description": "The Civire.",
            "intelligence": 7, "memory": 5, "name": "Civire", "perception": 5, "race_id": 1,
            "ship_type_id": 601, "willpower": 5 },
        ]),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/characters/100/portrait"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![1u8, 2, 3], "image/jpeg"))
        .mount(&server)
        .await;
      let (handle, mut events, db, images_dir) = spawn_engine(server.uri()).await;
      seed_ship_type(&db).await;

      handle.enroll(Subject::Character(100));

      let terminal = wait_for(&mut events, |event| {
        matches!(
          event,
          Event::Finished { key, .. } | Event::Failed { key, .. } if key.kind == JobKind::CharacterProfile
        )
      })
      .await;
      assert!(
        matches!(terminal, Event::Finished { .. }),
        "expected Finished, got {terminal:?}"
      );
      let character = character::get(&db, 100).await.unwrap().expect("character persisted");
      assert_eq!(character.name(), "Test Pilot");
      assert!(
        crate::store::repo::org::get_corporation(&db, 200)
          .await
          .unwrap()
          .is_some()
      );
      assert!(
        images_dir.path().join("characters").join("100.jpg").exists(),
        "the portrait should be written to the image store as part of the dataset"
      );
    }
  }

  mod supervise_loop {
    use std::sync::{
      Arc,
      atomic::{AtomicU32, Ordering},
    };

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn it_gives_up_and_parks_after_the_circuit_breaker_trips() {
      let (events_tx, mut events_rx) = mpsc::channel(EVENT_BUFFER);
      let (_command_tx, command_rx) = mpsc::unbounded_channel();
      let (restart_tx, restart_rx) = mpsc::unbounded_channel();
      let runs = Arc::new(AtomicU32::new(0));
      let runs_in_loop = Arc::clone(&runs);

      let loop_handle = tokio::spawn(async move {
        super::super::supervise_loop(&events_tx, command_rx, restart_rx, move |_command_rx| {
          let runs = Arc::clone(&runs_in_loop);
          Box::pin(async move {
            runs.fetch_add(1, Ordering::SeqCst);
            panic!("engine hot-loops");
          })
        })
        .await;
      });

      let gave_up = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
          match events_rx.recv().await {
            Some(Event::GaveUp {
              reason,
            }) => return reason,
            Some(_) => continue,
            None => panic!("the event channel closed before the supervisor gave up"),
          }
        }
      })
      .await
      .expect("the breaker should trip and emit a give-up signal rather than hot-loop forever");

      assert_eq!(
        runs.load(Ordering::SeqCst),
        RESPAWN_BREAKER_THRESHOLD,
        "the engine runs exactly threshold times: each death restarts until the threshold-th trips"
      );
      assert!(
        gave_up.contains("gave up"),
        "the give-up signal carries a human-meaningful reason, got {gave_up:?}"
      );

      // Parking after give-up keeps the supervisor alive; dropping the last restart sender (app
      // teardown) is what finally lets it exit.
      drop(restart_tx);
      tokio::time::timeout(Duration::from_secs(5), loop_handle)
        .await
        .expect("the supervisor exits once its restart channel closes")
        .expect("the supervisor task should not panic");
    }

    #[tokio::test(start_paused = true)]
    async fn it_respawns_the_engine_after_an_unexpected_death_and_keeps_the_command_path() {
      let (events_tx, mut events_rx) = mpsc::channel(EVENT_BUFFER);
      let (command_tx, command_rx) = mpsc::unbounded_channel();
      let (_restart_tx, restart_rx) = mpsc::unbounded_channel();
      let runs = Arc::new(AtomicU32::new(0));
      let runs_in_loop = Arc::clone(&runs);
      let reached_new_engine = Arc::new(AtomicU32::new(0));
      let reached_in_loop = Arc::clone(&reached_new_engine);

      // A command enqueued before the first engine dies must still be deliverable to the respawned
      // engine over the same receiver the supervisor owns.
      command_tx.send(Command::Drain).unwrap();

      let loop_handle = tokio::spawn(async move {
        super::super::supervise_loop(&events_tx, command_rx, restart_rx, move |command_rx| {
          let runs = Arc::clone(&runs_in_loop);
          let reached = Arc::clone(&reached_in_loop);
          Box::pin(async move {
            // Panic on the first attempt (an unexpected death), then on the respawn prove the same
            // receiver still carries the queued command before settling with a deliberate Shutdown.
            if runs.fetch_add(1, Ordering::SeqCst) == 0 {
              panic!("simulated engine panic");
            }
            if matches!(command_rx.recv().await, Some(Command::Drain)) {
              reached.fetch_add(1, Ordering::SeqCst);
            }
            ExitReason::Shutdown
          })
        })
        .await;
      });

      let restarted = tokio::time::timeout(Duration::from_secs(5), events_rx.recv())
        .await
        .expect("the supervisor should emit a restart signal")
        .expect("the event channel should stay open across a respawn");
      tokio::time::timeout(Duration::from_secs(5), loop_handle)
        .await
        .expect("the loop should terminate once the second attempt shuts down")
        .expect("the supervisor task should not panic");

      assert!(
        matches!(
          restarted,
          Event::Restarted {
            attempt: 1
          }
        ),
        "the first unexpected death emits a Restarted signal for attempt 1, got {restarted:?}"
      );
      assert_eq!(
        runs.load(Ordering::SeqCst),
        2,
        "the engine ran twice: the panicking attempt and its respawn"
      );
      assert_eq!(
        reached_new_engine.load(Ordering::SeqCst),
        1,
        "the command queued before the death reached the respawned engine over the surviving receiver"
      );
    }

    #[tokio::test(start_paused = true)]
    async fn it_resumes_from_a_parked_give_up_when_a_manual_restart_arrives() {
      let (events_tx, mut events_rx) = mpsc::channel(EVENT_BUFFER);
      let (_command_tx, command_rx) = mpsc::unbounded_channel();
      let (restart_tx, restart_rx) = mpsc::unbounded_channel();
      let runs = Arc::new(AtomicU32::new(0));
      let runs_in_loop = Arc::clone(&runs);

      let loop_handle = tokio::spawn(async move {
        super::super::supervise_loop(&events_tx, command_rx, restart_rx, move |_command_rx| {
          let runs = Arc::clone(&runs_in_loop);
          Box::pin(async move {
            // Panic until the breaker trips, then on the manual respawn settle cleanly so the loop
            // ends and the run count proves the restart reached a fresh engine attempt.
            if runs.fetch_add(1, Ordering::SeqCst) < RESPAWN_BREAKER_THRESHOLD {
              panic!("engine hot-loops");
            }
            ExitReason::Shutdown
          })
        })
        .await;
      });

      tokio::time::timeout(Duration::from_secs(30), async {
        loop {
          match events_rx.recv().await {
            Some(Event::GaveUp {
              ..
            }) => return,
            Some(_) => continue,
            None => panic!("the event channel closed before the supervisor gave up"),
          }
        }
      })
      .await
      .expect("the supervisor should give up and park");

      restart_tx.send(()).unwrap();
      tokio::time::timeout(Duration::from_secs(5), loop_handle)
        .await
        .expect("the manual restart should unpark the supervisor and let the fresh engine settle")
        .expect("the supervisor task should not panic");

      assert_eq!(
        runs.load(Ordering::SeqCst),
        RESPAWN_BREAKER_THRESHOLD + 1,
        "the manual restart resets the breaker and respawns the engine one more time past the trip"
      );
    }

    #[tokio::test(start_paused = true)]
    async fn it_short_circuits_backoff_when_a_manual_restart_arrives() {
      let (events_tx, mut events_rx) = mpsc::channel(EVENT_BUFFER);
      let (_command_tx, command_rx) = mpsc::unbounded_channel();
      let (restart_tx, restart_rx) = mpsc::unbounded_channel();
      let runs = Arc::new(AtomicU32::new(0));
      let runs_in_loop = Arc::clone(&runs);

      // A manual restart sent while the supervisor is backing off should resume immediately and
      // reset the streak rather than counting toward the breaker.
      restart_tx.send(()).unwrap();

      let loop_handle = tokio::spawn(async move {
        super::super::supervise_loop(&events_tx, command_rx, restart_rx, move |_command_rx| {
          let runs = Arc::clone(&runs_in_loop);
          Box::pin(async move {
            if runs.fetch_add(1, Ordering::SeqCst) == 0 {
              panic!("first attempt dies");
            }
            ExitReason::Shutdown
          })
        })
        .await;
      });

      tokio::time::timeout(Duration::from_secs(5), async {
        loop {
          match events_rx.recv().await {
            Some(Event::Restarted {
              ..
            }) => return,
            Some(_) => continue,
            None => panic!("the event channel closed before a restart was emitted"),
          }
        }
      })
      .await
      .expect("a queued manual restart short-circuits the backoff and emits a restart promptly");
      tokio::time::timeout(Duration::from_secs(5), loop_handle)
        .await
        .expect("the loop should terminate once the respawn shuts down")
        .expect("the supervisor task should not panic");

      assert_eq!(
        runs.load(Ordering::SeqCst),
        2,
        "the engine ran twice: the dying attempt and the manually-resumed respawn"
      );
    }

    #[tokio::test(start_paused = true)]
    async fn it_stops_without_respawn_on_a_deliberate_shutdown() {
      let (events_tx, _events_rx) = mpsc::channel(EVENT_BUFFER);
      let (_command_tx, command_rx) = mpsc::unbounded_channel();
      let (_restart_tx, restart_rx) = mpsc::unbounded_channel();
      let runs = Arc::new(AtomicU32::new(0));
      let runs_in_loop = Arc::clone(&runs);

      let loop_handle = tokio::spawn(async move {
        super::super::supervise_loop(&events_tx, command_rx, restart_rx, move |_command_rx| {
          let runs = Arc::clone(&runs_in_loop);
          Box::pin(async move {
            runs.fetch_add(1, Ordering::SeqCst);
            ExitReason::Shutdown
          })
        })
        .await;
      });

      tokio::time::timeout(Duration::from_secs(5), loop_handle)
        .await
        .expect("a deliberate shutdown ends the loop immediately")
        .expect("the supervisor task should not panic");

      assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "a deliberate Shutdown never respawns; the engine runs exactly once"
      );
    }
  }
}
