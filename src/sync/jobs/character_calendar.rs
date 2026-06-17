use chrono::{DateTime, Duration, Utc};

use crate::{
  clients::{
    Error,
    esi::{
      character::AuthenticatedClient,
      models::character::{CalendarAttendee, CalendarEvent, CalendarEventDetail},
    },
  },
  store::{
    model::{CharacterCalendarAttendee, CharacterCalendarEvent, OwnerType},
    repo::{calendar, character, infra},
  },
  sync::{job::JobCtx, jobs::names::resolve_names, outcome::Outcome, subject::Subject},
};

/// How far into the past and future to sync events, chosen to cap per-run ESI volume rather than
/// reflect any hard API limit.
const WINDOW_DAYS_BACK: i64 = 7;
const WINDOW_DAYS_FORWARD: i64 = 90;

const OUTBOX_KIND_RESPOND: &str = "calendar.respond";

const RESPONSE_NOT_RESPONDED: &str = "not_responded";

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Subject::Character(character_id) = ctx.key.subject else {
    return Ok(Outcome::synced());
  };
  let Some(grant) = ctx.grant else {
    return Err(Error::Internal(format!(
      "character calendar job for {character_id} requires a grant"
    )));
  };
  if character::get(ctx.db, character_id).await?.is_none() {
    return Err(Error::NotReady);
  }
  let authenticated = ctx.esi.character_authenticated(grant);

  let now = Utc::now();
  let events = authenticated.calendar_events().await?;

  let mut synced = 0usize;
  let mut skipped = 0usize;
  for summary in &events {
    if !within_window(summary, now) {
      continue;
    }
    match sync_event(ctx, &authenticated, character_id, summary, now).await {
      Ok(()) => synced += 1,
      Err(error) => {
        skipped += 1;
        tracing::warn!(
          character_id,
          event_id = summary.event_id,
          "character calendar: skipping event whose ESI fetch failed: {error}"
        );
      }
    }
  }

  if skipped > 0 && synced > 0 {
    tracing::warn!(
      character_id,
      synced,
      skipped,
      "character calendar: some events failed to fetch and were skipped"
    );
  }
  if synced == 0 && skipped > 0 {
    return Ok(Outcome::Skipped {
      reason: format!("{skipped} event(s) failed to fetch"),
    });
  }

  Ok(Outcome::from_rows(synced))
}

fn build_attendee(character_id: i64, event_id: i64, attendee: &CalendarAttendee) -> Option<CharacterCalendarAttendee> {
  let attendee_id = attendee.character_id?;
  Some(CharacterCalendarAttendee {
    attendee_id,
    character_id,
    event_id,
    event_response: attendee
      .event_response
      .clone()
      .unwrap_or_else(|| RESPONSE_NOT_RESPONDED.to_owned()),
  })
}

async fn build_event(
  ctx: &JobCtx<'_>,
  character_id: i64,
  summary: &CalendarEvent,
  detail: Option<&CalendarEventDetail>,
  cached: Option<&CharacterCalendarEvent>,
  now: DateTime<Utc>,
) -> Result<CharacterCalendarEvent, Error> {
  let timestamp = summary
    .event_date
    .clone()
    .or_else(|| detail.and_then(|d| d.date.clone()))
    .or_else(|| cached.map(|c| c.timestamp().clone()))
    .unwrap_or_default();
  let importance = summary
    .importance
    .map(i64::from)
    .or_else(|| detail.and_then(|d| d.importance).map(i64::from))
    .or_else(|| cached.map(CharacterCalendarEvent::importance))
    .unwrap_or(0);
  let title = summary
    .title
    .clone()
    .or_else(|| detail.and_then(|d| d.title.clone()))
    .or_else(|| cached.map(|c| c.title().clone()))
    .unwrap_or_default();
  let response = summary
    .event_response
    .clone()
    .or_else(|| detail.and_then(|d| d.response.clone()))
    .or_else(|| cached.map(|c| c.response().clone()))
    .unwrap_or_else(|| RESPONSE_NOT_RESPONDED.to_owned());

  let owner_id = detail
    .and_then(|d| d.owner_id)
    .or_else(|| cached.map(CharacterCalendarEvent::owner_id))
    .unwrap_or(0);
  let owner_type = detail
    .and_then(|d| d.owner_type.clone())
    .or_else(|| cached.map(|c| c.owner_type().clone()))
    .unwrap_or_default();
  let owner_name = resolve_owner_name(ctx, detail, cached, owner_id).await?;

  let body = detail
    .and_then(|d| d.text.clone())
    .or_else(|| cached.and_then(|c| c.body().clone()));
  let duration_minutes = detail
    .and_then(|d| d.duration)
    .map(i64::from)
    .or_else(|| cached.map(CharacterCalendarEvent::duration_minutes))
    .unwrap_or(0);

  Ok(CharacterCalendarEvent {
    body,
    character_id,
    duration_minutes,
    event_id: summary.event_id,
    fetched_at: now.to_rfc3339(),
    importance,
    owner_id,
    owner_name,
    owner_type,
    response,
    timestamp,
    title,
  })
}

async fn fetch_attendees(
  authenticated: &AuthenticatedClient<'_>,
  character_id: i64,
  event_id: i64,
) -> Result<Vec<CharacterCalendarAttendee>, Error> {
  let fetched = authenticated.calendar_attendees(event_id).await?;
  Ok(
    fetched
      .iter()
      .filter_map(|attendee| build_attendee(character_id, event_id, attendee))
      .collect(),
  )
}

fn is_important(summary: &CalendarEvent) -> bool {
  summary.importance.is_some_and(|importance| importance != 0)
}

fn is_upcoming(summary: &CalendarEvent, now: DateTime<Utc>) -> bool {
  summary
    .event_date
    .as_deref()
    .and_then(parse_timestamp)
    .is_some_and(|date| date >= now)
}

fn parse_timestamp(raw: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(raw)
    .ok()
    .map(|date| date.with_timezone(&Utc))
}

async fn resolve_owner_name(
  ctx: &JobCtx<'_>,
  detail: Option<&CalendarEventDetail>,
  cached: Option<&CharacterCalendarEvent>,
  owner_id: i64,
) -> Result<String, Error> {
  if let Some(name) = detail
    .and_then(|d| d.owner_name.clone())
    .filter(|name| !name.is_empty())
  {
    return Ok(name);
  }
  if let Some(name) = cached.map(|c| c.owner_name().clone()).filter(|name| !name.is_empty()) {
    return Ok(name);
  }
  if owner_id <= 0 {
    return Ok(String::new());
  }
  let resolved = resolve_names(ctx, &[owner_id]).await?;
  Ok(
    resolved
      .get(&owner_id)
      .map(|record| record.name.clone())
      .unwrap_or_default(),
  )
}

fn should_fetch_attendees(summary: &CalendarEvent, now: DateTime<Utc>) -> bool {
  is_important(summary) || is_upcoming(summary, now)
}

fn summary_changed(summary: &CalendarEvent, cached: &CharacterCalendarEvent) -> bool {
  let importance_changed = summary
    .importance
    .map(i64::from)
    .is_some_and(|i| i != cached.importance());
  let date_changed = summary.event_date.as_deref().is_some_and(|d| d != cached.timestamp());
  let title_changed = summary.title.as_deref().is_some_and(|t| t != cached.title());
  let response_changed = summary
    .event_response
    .as_deref()
    .is_some_and(|r| r != cached.response());

  importance_changed || date_changed || title_changed || response_changed
}

async fn sync_event(
  ctx: &JobCtx<'_>,
  authenticated: &AuthenticatedClient<'_>,
  character_id: i64,
  summary: &CalendarEvent,
  now: DateTime<Utc>,
) -> Result<(), Error> {
  let event_id = summary.event_id;
  let cached = calendar::event(ctx.db, character_id, event_id).await?;

  // The event detail (owner, body, duration) is immutable for a given event, so a cached row whose
  // mutable list fields (date, importance, title, viewer response) still match is reused without
  // re-fetching the detail body — mirroring the mail-body caching policy.
  let detail = match &cached {
    Some(existing) if !summary_changed(summary, existing) => None,
    _ => Some(authenticated.calendar_event(event_id).await?),
  };

  // Attendees are fetched eagerly only for events worth the extra round trip: upcoming events and
  // important ones. Past, unimportant events keep whatever attendee set was previously cached, since
  // their tallies no longer change and rarely matter — a lazy policy that caps per-sync ESI volume.
  let attendees = if should_fetch_attendees(summary, now) {
    fetch_attendees(authenticated, character_id, event_id).await?
  } else {
    calendar::attendees(ctx.db, character_id, event_id).await?
  };

  let mut event = build_event(ctx, character_id, summary, detail.as_ref(), cached.as_ref(), now).await?;

  // A pending calendar.respond outbox row is an RSVP the server has not acknowledged yet, so the summary still
  // reports the old response; keep the optimistic local response rather than let the full-replace sync revert it.
  if let Some(cached) = cached.as_ref()
    && has_pending_response(ctx, character_id, event_id).await?
  {
    event.response = cached.response().clone();
  }

  calendar::upsert_complete(ctx.db, &event, &attendees).await?;
  Ok(())
}

async fn has_pending_response(ctx: &JobCtx<'_>, character_id: i64, event_id: i64) -> Result<bool, Error> {
  let payloads =
    infra::outbox_pending_payloads(ctx.db, OwnerType::Character, character_id, OUTBOX_KIND_RESPOND).await?;
  Ok(payloads.iter().any(|payload| {
    serde_json::from_str::<serde_json::Value>(payload)
      .ok()
      .and_then(|value| value.get("event_id").and_then(serde_json::Value::as_i64))
      .is_some_and(|id| id == event_id)
  }))
}

fn within_window(summary: &CalendarEvent, now: DateTime<Utc>) -> bool {
  let Some(date) = summary.event_date.as_deref().and_then(parse_timestamp) else {
    // Keep events with an unparseable or absent date: the detail fetch may still resolve them.
    return true;
  };
  date >= now - Duration::days(WINDOW_DAYS_BACK) && date <= now + Duration::days(WINDOW_DAYS_FORWARD)
}

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  };

  use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{
    clients::{esi, eve_image, eve_sso::Grant, http},
    store::{self, images, repo::calendar},
    sync::job::{JobKey, JobKind},
  };

  fn upcoming(days: i64) -> String {
    (Utc::now() + Duration::days(days)).to_rfc3339()
  }

  async fn mount_json(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path(route))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  // The list endpoint is paginated by from_event_id; the empty terminator (matched first, since it
  // carries the cursor param) stops the walk after the single seed page is served.
  async fn mount_calendar_list(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
      .and(path("/characters/42/calendar/"))
      .and(wiremock::matchers::query_param_contains("from_event_id", ""))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
      .mount(server)
      .await;
    Mock::given(method("GET"))
      .and(path("/characters/42/calendar/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn mount_names(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
      .and(path("/universe/names/"))
      .respond_with(ResponseTemplate::new(200).set_body_json(body))
      .mount(server)
      .await;
  }

  async fn seed_character(db: &store::Database, id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn ctx_with_grant<'a>(
    db: &'a store::Database,
    esi: &'a esi::Client,
    image: &'a eve_image::Client,
    image_store: &'a images::Store,
    grant: &'a Grant,
    character_id: i64,
  ) -> JobCtx<'a> {
    JobCtx {
      db,
      esi,
      image,
      image_store,
      key: JobKey::new(JobKind::CharacterCalendar, Subject::Character(character_id)),
      grant: Some(grant),
      sso: None,
    }
  }

  struct Fixture {
    db: store::Database,
    esi: esi::Client,
    grant: Grant,
    image: eve_image::Client,
    image_store: images::Store,
    _images_dir: tempfile::TempDir,
    _server: MockServer,
  }

  async fn fixture(server: MockServer, character_id: i64) -> Fixture {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), server.uri());
    let image = eve_image::Client::with_base_url(http, server.uri());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    let grant = Grant::new_test("token", character_id);
    Fixture {
      _server: server,
      db,
      esi,
      image,
      image_store,
      _images_dir: images_dir,
      grant,
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_aborts_without_writing_when_the_event_list_fetch_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/calendar/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      let result = run(&ctx).await;

      assert!(result.is_err());
      assert!(calendar::events(&fx.db, 42).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_commits_event_detail_and_attendees_together() {
      let server = MockServer::start().await;
      let date = upcoming(5);
      mount_calendar_list(
        &server,
        serde_json::json!([
          { "event_id": 1234, "event_date": date, "importance": 1, "title": "CTA",
            "event_response": "not_responded" }
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/calendar/1234/",
        serde_json::json!({ "event_id": 1234, "date": date, "duration": 60, "importance": 1,
          "owner_id": 98_000_001_i64, "owner_name": "Test Corp", "owner_type": "corporation",
          "response": "not_responded", "title": "CTA", "text": "<p>Form up.</p>" }),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/calendar/1234/attendees/",
        serde_json::json!([
          { "character_id": 95_000_001_i64, "event_response": "accepted" },
          { "character_id": 95_000_002_i64, "event_response": "declined" }
        ]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
      let event = calendar::event(&fx.db, 42, 1234).await.unwrap().unwrap();
      assert_eq!(event.title(), "CTA");
      assert_eq!(event.owner_name(), "Test Corp");
      assert_eq!(event.duration_minutes(), 60);
      assert_eq!(event.body().as_deref(), Some("<p>Form up.</p>"));

      let attendees = calendar::attendees(&fx.db, 42, 1234).await.unwrap();
      assert_eq!(
        attendees.iter().map(|a| a.attendee_id()).collect::<Vec<_>>(),
        [95_000_001, 95_000_002]
      );
    }

    #[tokio::test]
    async fn it_does_not_refetch_the_detail_when_the_cached_event_is_unchanged() {
      let server = MockServer::start().await;
      let date = upcoming(5);
      mount_calendar_list(
        &server,
        serde_json::json!([{ "event_id": 1234, "event_date": date, "importance": 1, "title": "CTA",
          "event_response": "accepted" }]),
      )
      .await;
      let detail_hits = Arc::new(AtomicUsize::new(0));
      struct CountingDetail(Arc<AtomicUsize>, String);
      impl Respond for CountingDetail {
        fn respond(&self, _: &Request) -> ResponseTemplate {
          self.0.fetch_add(1, Ordering::SeqCst);
          ResponseTemplate::new(200).set_body_json(serde_json::json!({ "event_id": 1234, "date": self.1,
            "duration": 60, "owner_id": 98_000_001_i64, "owner_name": "Test Corp", "owner_type": "corporation",
            "response": "accepted", "title": "CTA", "text": "<p>REFETCH</p>" }))
        }
      }
      Mock::given(method("GET"))
        .and(path("/characters/42/calendar/1234/"))
        .respond_with(CountingDetail(detail_hits.clone(), date.clone()))
        .mount(&server)
        .await;
      mount_json(
        &server,
        "/characters/42/calendar/1234/attendees/",
        serde_json::json!([]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      calendar::upsert_complete(
        &fx.db,
        &CharacterCalendarEvent {
          body: Some("<p>cached</p>".to_owned()),
          character_id: 42,
          duration_minutes: 60,
          event_id: 1234,
          fetched_at: "2026-06-12T00:00:00Z".to_owned(),
          importance: 1,
          owner_id: 98_000_001,
          owner_name: "Test Corp".to_owned(),
          owner_type: "corporation".to_owned(),
          response: "accepted".to_owned(),
          timestamp: date.clone(),
          title: "CTA".to_owned(),
        },
        &[],
      )
      .await
      .unwrap();
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert_eq!(detail_hits.load(Ordering::SeqCst), 0);
      assert_eq!(
        calendar::event(&fx.db, 42, 1234)
          .await
          .unwrap()
          .unwrap()
          .body()
          .as_deref(),
        Some("<p>cached</p>")
      );
    }

    #[tokio::test]
    async fn it_keeps_an_optimistic_rsvp_when_the_respond_outbox_write_is_still_pending() {
      let server = MockServer::start().await;
      let date = upcoming(5);
      mount_calendar_list(
        &server,
        serde_json::json!([
          { "event_id": 1234, "event_date": date, "importance": 1, "title": "CTA",
            "event_response": "not_responded" }
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/calendar/1234/",
        serde_json::json!({ "event_id": 1234, "date": date, "duration": 60, "importance": 1,
          "owner_id": 98_000_001_i64, "owner_name": "Test Corp", "owner_type": "corporation",
          "response": "not_responded", "title": "CTA", "text": "<p>Form up.</p>" }),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/calendar/1234/attendees/",
        serde_json::json!([]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let cached = CharacterCalendarEvent {
        body: Some("<p>Form up.</p>".to_owned()),
        character_id: 42,
        duration_minutes: 60,
        event_id: 1234,
        fetched_at: "2026-06-12T00:00:00Z".to_owned(),
        importance: 1,
        owner_id: 98_000_001,
        owner_name: "Test Corp".to_owned(),
        owner_type: "corporation".to_owned(),
        response: "accepted".to_owned(),
        timestamp: date.clone(),
        title: "CTA".to_owned(),
      };
      calendar::upsert_complete(&fx.db, &cached, &[]).await.unwrap();
      infra::append(
        &fx.db,
        OwnerType::Character,
        42,
        OUTBOX_KIND_RESPOND,
        "{\"character_id\":42,\"event_id\":1234,\"response\":\"accepted\",\"previous_response\":\"not_responded\"}",
        Some("respond:1234"),
      )
      .await
      .unwrap();
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      assert_eq!(
        calendar::event(&fx.db, 42, 1234).await.unwrap().unwrap().response(),
        "accepted",
        "a pending RSVP outbox row protects the optimistic response against a stale not_responded sync"
      );
    }

    #[tokio::test]
    async fn it_resolves_a_missing_owner_name_via_the_name_endpoint() {
      let server = MockServer::start().await;
      let date = upcoming(5);
      mount_calendar_list(
        &server,
        serde_json::json!([{ "event_id": 1234, "event_date": date, "importance": 0, "title": "Op" }]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/calendar/1234/",
        serde_json::json!({ "event_id": 1234, "date": date, "duration": 30,
          "owner_id": 99_000_001_i64, "owner_type": "alliance", "response": "accepted", "title": "Op",
          "text": "<p>Roam.</p>" }),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/calendar/1234/attendees/",
        serde_json::json!([]),
      )
      .await;
      mount_names(
        &server,
        serde_json::json!([{ "category": "alliance", "id": 99_000_001_i64, "name": "Test Alliance" }]),
      )
      .await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      run(&ctx).await.unwrap();

      let event = calendar::event(&fx.db, 42, 1234).await.unwrap().unwrap();
      assert_eq!(event.owner_name(), "Test Alliance");
    }

    #[tokio::test]
    async fn it_skips_a_failed_event_without_aborting_the_rest() {
      let server = MockServer::start().await;
      let date = upcoming(5);
      mount_calendar_list(
        &server,
        serde_json::json!([
          { "event_id": 1, "event_date": date, "importance": 1, "title": "Good" },
          { "event_id": 2, "event_date": date, "importance": 1, "title": "Bad" }
        ]),
      )
      .await;
      mount_json(
        &server,
        "/characters/42/calendar/1/",
        serde_json::json!({ "event_id": 1, "date": date, "duration": 30, "owner_id": 98_000_001_i64,
          "owner_name": "Test Corp", "owner_type": "corporation", "response": "accepted", "title": "Good",
          "text": "<p>ok</p>" }),
      )
      .await;
      Mock::given(method("GET"))
        .and(path("/characters/42/calendar/2/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      mount_json(&server, "/characters/42/calendar/1/attendees/", serde_json::json!([])).await;
      let fx = fixture(server, 42).await;
      seed_character(&fx.db, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      let outcome = run(&ctx).await.unwrap();

      assert_eq!(
        outcome,
        Outcome::Synced {
          rows_touched: 1
        }
      );
      assert!(calendar::event(&fx.db, 42, 1).await.unwrap().is_some());
      assert!(calendar::event(&fx.db, 42, 2).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_skips_without_fetching_when_the_character_is_not_yet_persisted() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characters/42/calendar/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;
      let fx = fixture(server, 42).await;
      let ctx = ctx_with_grant(&fx.db, &fx.esi, &fx.image, &fx.image_store, &fx.grant, 42);

      let result = run(&ctx).await;

      assert!(matches!(result, Err(Error::NotReady)));
    }
  }
}
