use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::{
  config::FeatureFlags,
  store::{
    Database,
    model::{
      NewNotification, Notification, NotificationDestination, NotificationKind, NotificationOwner, NotificationTarget,
    },
    repo::{calendar, character, industry, mail, notifications, org, sde},
  },
  sync::JobKind,
};

/// How many surfaced notifications the center caches and the bell badge counts over.
const LIST_LIMIT: i64 = 200;

/// A consistent read of notification state for the UI: the surfaced list (newest-first), the unread
/// count, the notifications that surfaced for the first time on this pass (to pop as toasts), and a
/// per-owner display name map so the row "who" line can be rendered without a second DB round-trip.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
  pub list: Vec<Notification>,
  pub surfaced: Vec<Notification>,
  pub unread: i64,
  pub who: HashMap<NotificationOwner, String>,
}

/// Runs every detector (optionally), then reads back the surfaced list + unread count and resolves a
/// display name per owner. `run_detectors` is `true` when the pulse should re-scan sources (after a
/// relevant sync, or on the idle time-threshold cadence) and `false` for a pure UI refresh (panel
/// open, mark-read) where re-scanning would be wasted work.
pub async fn refresh(
  db: &Database,
  now: DateTime<Utc>,
  characters: &[i64],
  corporations: &[i64],
  features: &FeatureFlags,
  run_detectors: bool,
) -> Snapshot {
  let surfaced = if run_detectors {
    detect(db, now, characters, corporations, features).await
  } else {
    Vec::new()
  };

  let list = notifications::list(db, LIST_LIMIT).await.unwrap_or_default();
  let unread = notifications::unread_count(db).await.unwrap_or(0);
  let who = resolve_owner_names(db, &list, &surfaced).await;

  Snapshot {
    list,
    surfaced,
    unread,
    who,
  }
}

/// Drives the seven event detectors over the already-synced data for every owned subject and returns
/// the notifications that surfaced for the first time this pass. Each detector dedups via
/// `notifications::emit` (insert-if-absent) and watermarks the whole pre-existing history on a
/// subject's first sync so it never floods. The time-threshold detectors (skill/industry/extraction-
/// cracked) run every pass so an event that matured while the app sat idle still fires.
///
/// Errors from a single subject/detector are logged and skipped rather than aborting the sweep: one
/// character's transient read failure must not silence every other notification.
pub async fn detect(
  db: &Database,
  now: DateTime<Utc>,
  characters: &[i64],
  corporations: &[i64],
  features: &FeatureFlags,
) -> Vec<Notification> {
  let mut surfaced = Vec::new();
  for &character_id in characters {
    run(mail_detector(db, character_id, features), "mail", &mut surfaced).await;
    run(calendar_detector(db, character_id, features), "calendar", &mut surfaced).await;
    run(
      killmail_detector(db, NotificationOwner::Character(character_id), features),
      "killmail",
      &mut surfaced,
    )
    .await;
    run(skill_detector(db, character_id, now, features), "skill", &mut surfaced).await;
    run(
      industry_detector(db, NotificationOwner::Character(character_id), features),
      "industry",
      &mut surfaced,
    )
    .await;
  }
  for &corporation_id in corporations {
    run(
      killmail_detector(db, NotificationOwner::Corporation(corporation_id), features),
      "corp killmail",
      &mut surfaced,
    )
    .await;
    run(
      industry_detector(db, NotificationOwner::Corporation(corporation_id), features),
      "corp industry",
      &mut surfaced,
    )
    .await;
    run(
      extraction_scheduled_detector(db, corporation_id, features),
      "extraction scheduled",
      &mut surfaced,
    )
    .await;
    run(
      extraction_cracked_detector(db, corporation_id, now, features),
      "extraction cracked",
      &mut surfaced,
    )
    .await;
  }
  surfaced
}

async fn run(
  detector: impl std::future::Future<Output = Result<Vec<Notification>, crate::store::Error>>,
  label: &str,
  surfaced: &mut Vec<Notification>,
) {
  match detector.await {
    Ok(mut emitted) => surfaced.append(&mut emitted),
    Err(error) => {
      tracing::warn!(target: "pod::notifications", %error, detector = label, "notification detector failed");
    }
  }
}

async fn resolve_owner_names(
  db: &Database,
  list: &[Notification],
  surfaced: &[Notification],
) -> HashMap<NotificationOwner, String> {
  let mut names = HashMap::new();
  resolve_into(db, list, &mut names).await;
  resolve_into(db, surfaced, &mut names).await;
  names
}

/// Resolves a display name ("who") per distinct owner across an arbitrary slice of notifications, so a
/// UI labelling freshly-paged History rows can fill the "who" line without a second DB round-trip.
/// Each owner is looked up once even when many notifications share it.
#[allow(dead_code)]
pub async fn resolve_names(db: &Database, notifications: &[Notification]) -> HashMap<NotificationOwner, String> {
  let mut names = HashMap::new();
  resolve_into(db, notifications, &mut names).await;
  names
}

async fn resolve_into(db: &Database, notifications: &[Notification], names: &mut HashMap<NotificationOwner, String>) {
  for owner in notifications.iter().map(Notification::owner) {
    if names.contains_key(&owner) {
      continue;
    }
    let name = match owner {
      NotificationOwner::Character(id) => character_name(db, id).await,
      NotificationOwner::Corporation(id) => corporation_name(db, id).await,
    };
    names.insert(owner, name);
  }
}

async fn character_name(db: &Database, character_id: i64) -> String {
  character::get(db, character_id)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().clone())
    .unwrap_or_default()
}

/// Whether this is the first scan of `(owner, kind)`: the notifications table holds no row — surfaced or
/// suppressed watermark — for it yet. Deliberately independent of the sync ledger: the sync engine writes
/// the ledger's `last_success_at` BEFORE the detector pulse, so a ledger-based check reads false on the
/// very first sync and would flood the whole pre-existing history. On a true first scan the caller
/// watermarks the current items (and a sentinel) so later passes only surface genuinely new items.
async fn is_first_scan(
  db: &Database,
  owner: NotificationOwner,
  kind: NotificationKind,
) -> Result<bool, crate::store::Error> {
  Ok(!notifications::has_any(db, &owner, kind).await?)
}

async fn corporation_name(db: &Database, corporation_id: i64) -> String {
  org::get_corporation(db, corporation_id)
    .await
    .ok()
    .flatten()
    .map(|corporation| corporation.name().clone())
    .unwrap_or_default()
}

async fn type_name(db: &Database, type_id: i64) -> Option<String> {
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
}

async fn mail_detector(
  db: &Database,
  character_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterMail.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::Mail).await?;
  let headers = mail::headers(db, character_id).await?;
  // Default: received mail not authored by the owner (drop self-sent copies in the Sent box).
  let received: Vec<_> = headers.into_iter().filter(|m| m.from_id() != character_id).collect();
  let watermarks: Vec<String> = received.iter().map(|m| mail_key(character_id, m.mail_id())).collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Mail, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for header in received {
    let title = header
      .subject()
      .clone()
      .filter(|s| !s.is_empty())
      .unwrap_or_else(|| t!("shell.notification.mail_title_fallback").into_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.mail_body", name => header.from_name()).into_owned(),
        dedup_key: mail_key(character_id, header.mail_id()),
        kind: NotificationKind::Mail,
        owner: NotificationOwner::Character(character_id),
        target: NotificationTarget {
          character: Some(character_id),
          destination: NotificationDestination::Mail,
          sub: None,
        },
        title,
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn calendar_detector(
  db: &Database,
  character_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterCalendar.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::Calendar).await?;
  // The table holds only real ESI events; Pod-derived overlays live in memory, so reading the table
  // already excludes synthetic ones.
  let events = calendar::events(db, character_id).await?;
  let watermarks: Vec<String> = events
    .iter()
    .map(|e| calendar_key(character_id, e.event_id()))
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Calendar, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for event in events {
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: event.owner_name().clone(),
        dedup_key: calendar_key(character_id, event.event_id()),
        kind: NotificationKind::Calendar,
        owner: NotificationOwner::Character(character_id),
        target: NotificationTarget {
          character: Some(character_id),
          destination: NotificationDestination::Calendar,
          sub: None,
        },
        title: event.title().clone(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn killmail_detector(
  db: &Database,
  owner: NotificationOwner,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  let (job, subject_id) = match owner {
    NotificationOwner::Character(id) => (JobKind::CharacterKillmails, id),
    NotificationOwner::Corporation(id) => (JobKind::CorporationKillmails, id),
  };
  if !job.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let first = is_first_scan(db, owner, NotificationKind::Killmail).await?;
  let rows: Vec<(i64, i64)> = match owner {
    NotificationOwner::Character(id) => character::killmails(db, id)
      .await?
      .into_iter()
      .map(|k| (k.killmail_id(), k.ship_type_id()))
      .collect(),
    NotificationOwner::Corporation(id) => org::corporation_killmails(db, id)
      .await?
      .into_iter()
      .map(|k| (k.killmail_id(), k.ship_type_id()))
      .collect(),
  };
  let watermarks: Vec<String> = rows.iter().map(|(killmail_id, _)| killmail_key(*killmail_id)).collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Killmail, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (killmail_id, ship_type_id) in rows {
    let ship = type_name(db, ship_type_id)
      .await
      .unwrap_or_else(|| t!("shell.notification.killmail_ship_fallback").into_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.killmail_destroyed", ship => ship).into_owned(),
        dedup_key: killmail_key(killmail_id),
        kind: NotificationKind::Killmail,
        owner,
        target: NotificationTarget {
          character: matches!(owner, NotificationOwner::Character(_)).then_some(subject_id),
          destination: NotificationDestination::CharacterDetail,
          sub: None,
        },
        title: t!("shell.notification.killmail_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn skill_detector(
  db: &Database,
  character_id: i64,
  now: DateTime<Utc>,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterSkills.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::Skill).await?;
  // A finished skill leaves no completion row; "new" is a finish_date crossing wall-clock. Scan the
  // whole queue so a multi-skill burst that matured while idle each fires once.
  let now_rfc = now.to_rfc3339();
  let matured: Vec<(i64, String)> = character::skillqueue(db, character_id)
    .await?
    .into_iter()
    .filter_map(|entry| entry.finish_date().clone().map(|finish| (entry.skill_id(), finish)))
    .filter(|(_, finish)| crossed(finish, &now_rfc))
    .collect();
  let watermarks: Vec<String> = matured
    .iter()
    .map(|(skill_id, finish)| skill_key(character_id, *skill_id, finish))
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Skill, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (skill_id, finish) in matured {
    let skill = type_name(db, skill_id)
      .await
      .unwrap_or_else(|| t!("shell.notification.skill_fallback").into_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.skill_body").into_owned(),
        dedup_key: skill_key(character_id, skill_id, &finish),
        kind: NotificationKind::Skill,
        owner: NotificationOwner::Character(character_id),
        target: NotificationTarget {
          character: Some(character_id),
          destination: NotificationDestination::Skills,
          sub: None,
        },
        title: t!("shell.notification.skill_title", skill => skill).into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn industry_detector(
  db: &Database,
  owner: NotificationOwner,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  let (job, subject_id) = match owner {
    NotificationOwner::Character(id) => (JobKind::CharacterIndustryJobs, id),
    NotificationOwner::Corporation(id) => (JobKind::CorporationIndustryJobs, id),
  };
  if !job.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let first = is_first_scan(db, owner, NotificationKind::Industry).await?;
  let jobs: Vec<(i64, Option<i64>)> = match owner {
    NotificationOwner::Character(id) => industry::list_for_character(db, id)
      .await?
      .into_iter()
      .filter(|j| is_industry_done(j.status()))
      .map(|j| (j.job_id(), j.product_type_id()))
      .collect(),
    NotificationOwner::Corporation(id) => industry::list_for_corporation(db, id)
      .await?
      .into_iter()
      .filter(|j| is_industry_done(j.status()))
      .map(|j| (j.job_id(), j.product_type_id()))
      .collect(),
  };
  let watermarks: Vec<String> = jobs.iter().map(|(job_id, _)| industry_key(*job_id)).collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Industry, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (job_id, product_type_id) in jobs {
    let product = match product_type_id {
      Some(id) => type_name(db, id)
        .await
        .unwrap_or_else(|| t!("shell.notification.industry_fallback").into_owned()),
      None => t!("shell.notification.industry_fallback").into_owned(),
    };
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.industry_ready", product => product).into_owned(),
        dedup_key: industry_key(job_id),
        kind: NotificationKind::Industry,
        owner,
        target: NotificationTarget {
          character: matches!(owner, NotificationOwner::Character(_)).then_some(subject_id),
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: t!("shell.notification.industry_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn extraction_scheduled_detector(
  db: &Database,
  corporation_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CorporationMiningExtractions.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Corporation(corporation_id);
  let first = is_first_scan(db, owner, NotificationKind::ExtractionScheduled).await?;
  let scheduled: Vec<_> = org::corporation_mining_extractions(db, corporation_id)
    .await?
    .into_iter()
    .filter_map(|e| {
      e.chunk_arrival_time()
        .clone()
        .map(|arrival| (e.structure_id(), e.moon_id(), arrival, e.moon_name().clone()))
    })
    .collect();
  let watermarks: Vec<String> = scheduled
    .iter()
    .map(|(structure_id, moon_id, arrival, _)| {
      extraction_scheduled_key(corporation_id, *structure_id, *moon_id, arrival)
    })
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::ExtractionScheduled, &watermarks).await?;
    return Ok(Vec::new());
  }

  let who = corporation_name(db, corporation_id).await;
  let mut surfaced = Vec::new();
  for (structure_id, moon_id, arrival, moon_name) in scheduled {
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: moon_name.unwrap_or_else(|| who.clone()),
        dedup_key: extraction_scheduled_key(corporation_id, structure_id, moon_id, &arrival),
        kind: NotificationKind::ExtractionScheduled,
        owner: NotificationOwner::Corporation(corporation_id),
        target: NotificationTarget {
          character: None,
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: t!("shell.notification.extraction_scheduled_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn extraction_cracked_detector(
  db: &Database,
  corporation_id: i64,
  now: DateTime<Utc>,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CorporationMiningExtractions.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Corporation(corporation_id);
  let first = is_first_scan(db, owner, NotificationKind::ExtractionCracked).await?;
  let now_rfc = now.to_rfc3339();
  let cracked: Vec<_> = org::corporation_mining_extractions(db, corporation_id)
    .await?
    .into_iter()
    .filter_map(|e| {
      e.chunk_arrival_time()
        .clone()
        .map(|arrival| (e.structure_id(), e.moon_id(), arrival, e.moon_name().clone()))
    })
    .filter(|(_, _, arrival, _)| crossed(arrival, &now_rfc))
    .collect();
  let watermarks: Vec<String> = cracked
    .iter()
    .map(|(structure_id, moon_id, arrival, _)| extraction_cracked_key(corporation_id, *structure_id, *moon_id, arrival))
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::ExtractionCracked, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (structure_id, moon_id, arrival, moon_name) in cracked {
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: moon_name.unwrap_or_else(|| t!("shell.notification.extraction_cracked_fallback").into_owned()),
        dedup_key: extraction_cracked_key(corporation_id, structure_id, moon_id, &arrival),
        kind: NotificationKind::ExtractionCracked,
        owner: NotificationOwner::Corporation(corporation_id),
        target: NotificationTarget {
          character: None,
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: t!("shell.notification.extraction_cracked_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

/// A stored RFC3339 timestamp has matured: it is at or before `now`. String comparison is exact for
/// the RFC3339 timestamps both `chunk_arrival_time` and `finish_date` carry once normalised to UTC.
fn crossed(timestamp: &str, now_rfc: &str) -> bool {
  match (
    DateTime::parse_from_rfc3339(timestamp),
    DateTime::parse_from_rfc3339(now_rfc),
  ) {
    (Ok(when), Ok(now)) => when <= now,
    _ => false,
  }
}

fn is_industry_done(status: &str) -> bool {
  matches!(status, "ready" | "delivered")
}

fn calendar_key(character_id: i64, event_id: i64) -> String {
  format!("calendar:{character_id}:{event_id}")
}

fn extraction_cracked_key(corporation_id: i64, structure_id: i64, moon_id: i64, arrival: &str) -> String {
  format!("extraction_cracked:{corporation_id}:{structure_id}:{moon_id}:{arrival}")
}

fn extraction_scheduled_key(corporation_id: i64, structure_id: i64, moon_id: i64, arrival: &str) -> String {
  format!("extraction_scheduled:{corporation_id}:{structure_id}:{moon_id}:{arrival}")
}

fn industry_key(job_id: i64) -> String {
  format!("industry:{job_id}")
}

// Keyed on the killmail id ALONE (no owner): a kill attributable to both an owned character and its
// owned corporation must notify exactly once, so whichever owner's detector reaches it first wins and
// the other's emit is an INSERT OR IGNORE no-op.
fn killmail_key(killmail_id: i64) -> String {
  format!("killmail:{killmail_id}")
}

fn mail_key(character_id: i64, mail_id: i64) -> String {
  format!("mail:{character_id}:{mail_id}")
}

fn skill_key(character_id: i64, skill_id: i64, finish_date: &str) -> String {
  format!("skill:{character_id}:{skill_id}:{finish_date}")
}

#[cfg(test)]
mod tests {
  use super::*;

  mod crossed {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_a_past_timestamp_as_matured() {
      assert!(crossed("2026-01-01T00:00:00+00:00", "2026-06-01T00:00:00+00:00"));
    }

    #[test]
    fn it_does_not_flag_a_future_timestamp() {
      assert!(!crossed("2026-12-01T00:00:00+00:00", "2026-06-01T00:00:00+00:00"));
    }

    #[test]
    fn it_treats_an_unparseable_timestamp_as_not_matured() {
      assert!(!crossed("not-a-date", "2026-06-01T00:00:00+00:00"));
    }

    #[test]
    fn it_treats_the_exact_boundary_as_matured() {
      assert_eq!(crossed("2026-06-01T00:00:00+00:00", "2026-06-01T00:00:00+00:00"), true);
    }
  }

  mod is_industry_done {
    use super::*;

    #[test]
    fn it_only_fires_on_ready_or_delivered() {
      assert!(is_industry_done("ready"));
      assert!(is_industry_done("delivered"));
      assert!(!is_industry_done("active"));
      assert!(!is_industry_done("paused"));
    }
  }

  mod keys {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_stable_kind_prefixed_keys() {
      assert_eq!(mail_key(1, 2), "mail:1:2");
      assert_eq!(calendar_key(1, 2), "calendar:1:2");
      assert_eq!(killmail_key(7), "killmail:7");
      assert_eq!(
        skill_key(1, 3300, "2026-06-22T00:00:00+00:00"),
        "skill:1:3300:2026-06-22T00:00:00+00:00"
      );
      assert_eq!(industry_key(55), "industry:55");
      assert_eq!(
        extraction_scheduled_key(98, 1001, 40_000, "2026-06-22T00:00:00+00:00"),
        "extraction_scheduled:98:1001:40000:2026-06-22T00:00:00+00:00"
      );
      assert_eq!(
        extraction_cracked_key(98, 1001, 40_000, "2026-06-22T00:00:00+00:00"),
        "extraction_cracked:98:1001:40000:2026-06-22T00:00:00+00:00"
      );
    }
  }

  mod killmail_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{
        self,
        model::{CharacterKillEntry, CorporationKillEntry},
      },
    };

    const CHARACTER: i64 = 95_465_499;

    fn flags() -> FeatureFlags {
      FeatureFlags::default()
    }

    fn killmails_disabled() -> FeatureFlags {
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::CombatLog, false);
      flags
    }

    // The killmail row FKs to characters(id), which in turn FKs to races/bloodlines/corporations.
    // Seed that minimal graph directly so the detector can read a real source row.
    async fn seed_character(db: &Database) {
      sqlx::query("INSERT INTO races (id, alliance_id, description, name) VALUES (1, 1, '', 'Caldari')")
        .execute(db.writer())
        .await
        .unwrap();
      let bloodline = "INSERT INTO bloodlines \
        (id, corporation_id, race_id, charisma, description, intelligence, memory, name, perception, willpower) \
        VALUES (1, 1, 1, 1, '', 1, 1, 'Civire', 1, 1)";
      sqlx::query(bloodline).execute(db.writer()).await.unwrap();
      let corporation = "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
        VALUES (1, 1, 1, 1, 'Test Corp', 0.0, 'TEST')";
      sqlx::query(corporation).execute(db.writer()).await.unwrap();
      let character = "INSERT INTO characters (id, bloodline_id, corporation_id, race_id, birthday, gender, name) \
        VALUES (?, 1, 1, 1, '2020-01-01T00:00:00Z', 'male', 'Test Pilot')";
      sqlx::query(character)
        .bind(CHARACTER)
        .execute(db.writer())
        .await
        .unwrap();
    }

    fn kill(killmail_id: i64) -> CharacterKillEntry {
      CharacterKillEntry {
        attacker_count: 1,
        character_id: CHARACTER,
        final_blow: true,
        is_kill: true,
        kill_hash: format!("hash-{killmail_id}"),
        kill_time: "2026-06-20T00:00:00+00:00".to_owned(),
        killmail_id,
        ship_type_id: 587,
        synced_at: "2026-06-22T00:00:00+00:00".to_owned(),
        system_id: 30_000_142,
        value_destroyed_isk: 0.0,
        value_final: true,
        value_isk: 0.0,
        value_recheck_count: 0,
        value_source: "zkill".to_owned(),
        victim_alliance_id: None,
        victim_corp_id: None,
        victim_damage_taken: 100,
        victim_id: None,
      }
    }

    const CORPORATION: i64 = 1;

    fn corp_kill(killmail_id: i64) -> CorporationKillEntry {
      CorporationKillEntry {
        attacker_count: 1,
        corporation_id: CORPORATION,
        final_blow: true,
        is_kill: true,
        kill_hash: format!("corp-hash-{killmail_id}"),
        kill_time: "2026-06-20T00:00:00+00:00".to_owned(),
        killmail_id,
        ship_type_id: 587,
        synced_at: "2026-06-22T00:00:00+00:00".to_owned(),
        system_id: 30_000_142,
        value_destroyed_isk: 0.0,
        value_final: true,
        value_isk: 0.0,
        value_recheck_count: 0,
        value_source: "zkill".to_owned(),
        victim_alliance_id: None,
        victim_corp_id: None,
        victim_damage_taken: 100,
        victim_id: None,
      }
    }

    #[tokio::test]
    async fn it_watermarks_existing_history_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();

      let surfaced = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "pre-existing history is watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_exactly_one_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      character::upsert_killmail(&db, &kill(2)).await.unwrap();

      let surfaced = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "killmail:2");
      assert_eq!(notifications::unread_count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_is_a_no_op_on_a_rerun_over_unchanged_data() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      character::upsert_killmail(&db, &kill(2)).await.unwrap();
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      let rerun = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      assert!(rerun.is_empty(), "re-running over unchanged data surfaces nothing new");
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    // Bug 3: a kill attributable to both an owned character and its owned corporation must notify once,
    // because the dedup_key is keyed on the killmail id alone.
    #[tokio::test]
    async fn it_notifies_once_when_owned_by_both_a_character_and_its_corporation() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      // First scan over empty history watermarks nothing real for either owner.
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      killmail_detector(&db, NotificationOwner::Corporation(CORPORATION), &flags())
        .await
        .unwrap();
      character::upsert_killmail(&db, &kill(42)).await.unwrap();
      org::upsert_corporation_killmail(&db, &corp_kill(42)).await.unwrap();

      let from_char = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      let from_corp = killmail_detector(&db, NotificationOwner::Corporation(CORPORATION), &flags())
        .await
        .unwrap();

      assert_eq!(from_char.len(), 1, "the character detector surfaces the kill once");
      assert!(
        from_corp.is_empty(),
        "the corp detector sees the same key already taken"
      );
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();

      let surfaced = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &killmails_disabled())
        .await
        .unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  // The character-scoped detectors (mail/calendar/skill/industry) all FK to characters(id), which in
  // turn FKs to races/bloodlines/corporations. Seed that minimal graph directly so the source readers
  // can return real rows.
  async fn seed_character(db: &Database, character_id: i64) {
    sqlx::query(
      "INSERT INTO races (id, alliance_id, description, name) VALUES (1, 1, '', 'Caldari') ON CONFLICT DO NOTHING",
    )
    .execute(db.writer())
    .await
    .unwrap();
    let bloodline = "INSERT INTO bloodlines \
      (id, corporation_id, race_id, charisma, description, intelligence, memory, name, perception, willpower) \
      VALUES (1, 1, 1, 1, '', 1, 1, 'Civire', 1, 1) ON CONFLICT DO NOTHING";
    sqlx::query(bloodline).execute(db.writer()).await.unwrap();
    let corporation = "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (1, 1, 1, 1, 'Test Corp', 0.0, 'TEST') ON CONFLICT DO NOTHING";
    sqlx::query(corporation).execute(db.writer()).await.unwrap();
    let character = "INSERT INTO characters (id, bloodline_id, corporation_id, race_id, birthday, gender, name) \
      VALUES (?, 1, 1, 1, '2020-01-01T00:00:00Z', 'male', 'Test Pilot')";
    sqlx::query(character)
      .bind(character_id)
      .execute(db.writer())
      .await
      .unwrap();
  }

  // The corp-scoped extraction tables FK to corporations(id); seed just that parent row.
  async fn seed_corporation(db: &Database, corporation_id: i64) {
    sqlx::query(
      "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (?, 1, 1, 1, 'Test Corp', 0.0, 'TEST') ON CONFLICT DO NOTHING",
    )
    .bind(corporation_id)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod calendar_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CharacterCalendarEvent},
    };

    const CHARACTER: i64 = 7001;

    fn event(event_id: i64) -> CharacterCalendarEvent {
      CharacterCalendarEvent {
        character_id: CHARACTER,
        event_id,
        owner_name: "Fleet Command".to_owned(),
        timestamp: "2026-06-20T00:00:00+00:00".to_owned(),
        title: "Op briefing".to_owned(),
        ..Default::default()
      }
    }

    async fn seed_event(db: &Database, event_id: i64) {
      calendar::upsert_complete(db, &event(event_id), &[]).await.unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_existing_history_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;

      let surfaced = calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "pre-existing events are watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_a_new_event_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;
      calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();
      seed_event(&db, 2).await;

      let surfaced = calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "calendar:7001:2");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].title(), "Op briefing");
      assert_eq!(surfaced[0].body(), "Fleet Command");
      assert_eq!(
        surfaced[0].target().destination,
        crate::store::model::NotificationDestination::Calendar
      );
    }

    #[tokio::test]
    async fn it_is_a_no_op_on_a_rerun_over_unchanged_data() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;
      calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();
      seed_event(&db, 2).await;
      calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      let rerun = calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(rerun.is_empty());
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Calendar, false);

      let surfaced = calendar_detector(&db, CHARACTER, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod extraction_cracked_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CorporationMiningExtraction},
    };

    const CORPORATION: i64 = 98_000_001;

    const NOW: &str = "2026-06-21T00:00:00+00:00";

    fn now() -> DateTime<Utc> {
      DateTime::parse_from_rfc3339(NOW).unwrap().with_timezone(&Utc)
    }

    fn extraction(structure_id: i64, moon_id: i64, arrival: &str) -> CorporationMiningExtraction {
      CorporationMiningExtraction {
        chunk_arrival_time: Some(arrival.to_owned()),
        corporation_id: CORPORATION,
        extraction_start_time: None,
        moon_id,
        moon_name: None,
        natural_decay_time: None,
        security_status: None,
        solar_system_id: None,
        structure_id,
      }
    }

    async fn seed(db: &Database, extractions: &[CorporationMiningExtraction]) {
      seed_corporation(db, CORPORATION).await;
      org::replace_extractions_for_corporation(db, CORPORATION, extractions)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_already_cracked_chunks_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-06-20T00:00:00+00:00")]).await;

      let surfaced = extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "a past arrival is watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_ignores_a_future_arrival() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      // First scan over no matured rows watermarks nothing.
      extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();

      let surfaced = extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "a chunk whose arrival is still in the future never cracks"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_when_a_chunk_matures_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();
      // The same chunk's arrival has now passed relative to a later `now`.
      let later = DateTime::parse_from_rfc3339("2026-12-02T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc);

      let surfaced = extraction_cracked_detector(&db, CORPORATION, later, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(
        surfaced[0].dedup_key(),
        "extraction_cracked:98000001:1001:40000:2026-12-01T00:00:00+00:00"
      );
      assert_eq!(surfaced[0].owner(), NotificationOwner::Corporation(CORPORATION));
      assert_eq!(surfaced[0].title(), "Moon chunk fractured");
      // No moons row was seeded, so the body falls back to the static label.
      assert_eq!(surfaced[0].body(), "Ready to mine");
      assert_eq!(surfaced[0].target().character, None);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-06-20T00:00:00+00:00")]).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Industry, false);

      let surfaced = extraction_cracked_detector(&db, CORPORATION, now(), &flags)
        .await
        .unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod extraction_scheduled_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CorporationMiningExtraction},
    };

    const CORPORATION: i64 = 98_000_002;

    fn extraction(structure_id: i64, moon_id: i64, arrival: &str) -> CorporationMiningExtraction {
      CorporationMiningExtraction {
        chunk_arrival_time: Some(arrival.to_owned()),
        corporation_id: CORPORATION,
        extraction_start_time: None,
        moon_id,
        moon_name: None,
        natural_decay_time: None,
        security_status: None,
        solar_system_id: None,
        structure_id,
      }
    }

    fn no_arrival(structure_id: i64, moon_id: i64) -> CorporationMiningExtraction {
      CorporationMiningExtraction {
        chunk_arrival_time: None,
        corporation_id: CORPORATION,
        extraction_start_time: None,
        moon_id,
        moon_name: None,
        natural_decay_time: None,
        security_status: None,
        solar_system_id: None,
        structure_id,
      }
    }

    async fn seed(db: &Database, extractions: &[CorporationMiningExtraction]) {
      seed_corporation(db, CORPORATION).await;
      org::replace_extractions_for_corporation(db, CORPORATION, extractions)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_existing_schedules_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "pre-existing schedules are watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_an_extraction_without_an_arrival_time() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[no_arrival(1001, 40_000)]).await;
      extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();
      // Add a second arrival-less row: still nothing to schedule.
      seed(&db, &[no_arrival(1001, 40_000), no_arrival(1002, 40_001)]).await;

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "an extraction with no chunk arrival has nothing to schedule"
      );
    }

    #[tokio::test]
    async fn it_emits_a_new_schedule_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();
      seed(
        &db,
        &[
          extraction(1001, 40_000, "2026-12-01T00:00:00+00:00"),
          extraction(1002, 40_001, "2026-12-05T00:00:00+00:00"),
        ],
      )
      .await;

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(
        surfaced[0].dedup_key(),
        "extraction_scheduled:98000002:1002:40001:2026-12-05T00:00:00+00:00"
      );
      assert_eq!(surfaced[0].owner(), NotificationOwner::Corporation(CORPORATION));
      assert_eq!(surfaced[0].title(), "Extraction scheduled");
      assert_eq!(
        surfaced[0].target().destination,
        crate::store::model::NotificationDestination::Industry
      );
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Industry, false);

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod industry_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{
        self,
        model::{CharacterIndustryJob, CorporationIndustryJob, CorporationMemberRole, OwnerType},
        repo::infra,
      },
    };

    const CHARACTER: i64 = 7100;

    const CORPORATION: i64 = 1;

    const DIRECTOR: i64 = 7100;

    fn job(job_id: i64, status: &str) -> CharacterIndustryJob {
      CharacterIndustryJob {
        activity_id: 1,
        blueprint_id: 100,
        blueprint_location_id: 60_003_760,
        blueprint_type_id: 12_345,
        character_id: CHARACTER,
        completed_character_id: None,
        completed_date: None,
        cost: None,
        duration: 3600,
        end_date: "2026-06-20T00:00:00+00:00".to_owned(),
        facility_id: 60_003_760,
        installer_id: CHARACTER,
        job_id,
        licensed_runs: None,
        output_location_id: 60_003_760,
        pause_date: None,
        probability: None,
        product_type_id: Some(587),
        runs: 1,
        start_date: "2026-06-19T00:00:00+00:00".to_owned(),
        station_id: None,
        status: status.to_owned(),
        successful_runs: None,
      }
    }

    fn corp_job(job_id: i64, status: &str) -> CorporationIndustryJob {
      CorporationIndustryJob {
        activity_id: 1,
        blueprint_id: 100,
        blueprint_location_id: 60_003_760,
        blueprint_type_id: 12_345,
        completed_character_id: None,
        completed_date: None,
        corporation_id: CORPORATION,
        cost: None,
        duration: 3600,
        end_date: "2026-06-20T00:00:00+00:00".to_owned(),
        facility_id: 60_003_760,
        installer_id: DIRECTOR,
        job_id,
        licensed_runs: None,
        output_location_id: 60_003_760,
        pause_date: None,
        probability: None,
        product_type_id: Some(587),
        runs: 1,
        start_date: "2026-06-19T00:00:00+00:00".to_owned(),
        station_id: None,
        status: status.to_owned(),
        successful_runs: None,
      }
    }

    // The corp industry reader gates on `corp_is_authorized`: an owned corp whose Director-roled
    // credential authorizer is a member holding the Director role. Seed that full chain.
    async fn authorize_corp(db: &Database) {
      seed_character(db, DIRECTOR).await;
      infra::upsert(
        db,
        CORPORATION,
        OwnerType::Corporation,
        "tok",
        "rt",
        9999,
        Some(DIRECTOR),
        None,
      )
      .await
      .unwrap();
      org::replace_for_corporation(
        db,
        CORPORATION,
        &[CorporationMemberRole::from((
          CORPORATION,
          DIRECTOR,
          "Director".to_owned(),
        ))],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_finished_jobs_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      industry::replace_for_character(&db, CHARACTER, &[job(1, "ready")])
        .await
        .unwrap();

      let surfaced = industry_detector(&db, NotificationOwner::Character(CHARACTER), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "pre-existing finished jobs are watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_only_surfaces_done_jobs() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      industry::replace_for_character(&db, CHARACTER, &[job(1, "ready")])
        .await
        .unwrap();
      industry_detector(&db, NotificationOwner::Character(CHARACTER), &FeatureFlags::default())
        .await
        .unwrap();
      // An active job is not done; a newly-delivered one is.
      industry::replace_for_character(
        &db,
        CHARACTER,
        &[job(1, "ready"), job(2, "active"), job(3, "delivered")],
      )
      .await
      .unwrap();

      let surfaced = industry_detector(&db, NotificationOwner::Character(CHARACTER), &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1, "only the newly-delivered job surfaces");
      assert_eq!(surfaced[0].dedup_key(), "industry:3");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].title(), "Industry job complete");
      assert_eq!(surfaced[0].target().character, Some(CHARACTER));
    }

    #[tokio::test]
    async fn it_emits_for_a_corporation_owner() {
      let db = store::open_test().await.unwrap();
      authorize_corp(&db).await;
      industry::replace_for_corporation(&db, CORPORATION, &[corp_job(10, "ready")])
        .await
        .unwrap();
      industry_detector(
        &db,
        NotificationOwner::Corporation(CORPORATION),
        &FeatureFlags::default(),
      )
      .await
      .unwrap();
      industry::replace_for_corporation(&db, CORPORATION, &[corp_job(10, "ready"), corp_job(11, "delivered")])
        .await
        .unwrap();

      let surfaced = industry_detector(
        &db,
        NotificationOwner::Corporation(CORPORATION),
        &FeatureFlags::default(),
      )
      .await
      .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "industry:11");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Corporation(CORPORATION));
      assert_eq!(
        surfaced[0].target().character,
        None,
        "a corp job carries no character target"
      );
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      industry::replace_for_character(&db, CHARACTER, &[job(1, "ready")])
        .await
        .unwrap();
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Industry, false);

      let surfaced = industry_detector(&db, NotificationOwner::Character(CHARACTER), &flags)
        .await
        .unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod mail_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{
        self,
        model::{CharacterMail, CharacterMailBody, CharacterMailRecipient},
      },
    };

    const CHARACTER: i64 = 7200;

    fn header(mail_id: i64, from_id: i64, subject: Option<&str>) -> CharacterMail {
      CharacterMail {
        character_id: CHARACTER,
        from_id,
        from_name: "Sender".to_owned(),
        mail_id,
        subject: subject.map(str::to_owned),
        timestamp: "2026-06-20T00:00:00+00:00".to_owned(),
        ..Default::default()
      }
    }

    async fn seed_mail(db: &Database, mail_id: i64, from_id: i64, subject: Option<&str>) {
      let body = CharacterMailBody {
        body: "<p>hi</p>".to_owned(),
        character_id: CHARACTER,
        mail_id,
      };
      mail::upsert_complete(
        db,
        &header(mail_id, from_id, subject),
        &body,
        &[] as &[CharacterMailRecipient],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_existing_mail_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("Hello")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert!(surfaced.is_empty(), "pre-existing mail is watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_a_new_received_mail_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("Hello")).await;
      mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();
      seed_mail(&db, 2, 888, Some("Re: contract")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "mail:7200:2");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].title(), "Re: contract");
      assert_eq!(surfaced[0].body(), "From Sender");
    }

    #[tokio::test]
    async fn it_titles_an_empty_subject_mail_with_a_fallback() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("seed")).await;
      mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();
      seed_mail(&db, 2, 888, Some("")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].title(), "New EVE mail");
    }

    #[tokio::test]
    async fn it_drops_self_sent_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("seed")).await;
      mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();
      // A mail whose author is the owner is a Sent-box copy and must not notify.
      seed_mail(&db, 2, CHARACTER, Some("My own message")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert!(surfaced.is_empty(), "mail authored by the owner is filtered out");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("Hello")).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Mail, false);

      let surfaced = mail_detector(&db, CHARACTER, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod skill_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CharacterSkillqueue},
    };

    const CHARACTER: i64 = 7300;

    const NOW: &str = "2026-06-21T00:00:00+00:00";

    fn now() -> DateTime<Utc> {
      DateTime::parse_from_rfc3339(NOW).unwrap().with_timezone(&Utc)
    }

    fn entry(skill_id: i64, queue_position: i64, finish: Option<&str>) -> CharacterSkillqueue {
      CharacterSkillqueue {
        character_id: CHARACTER,
        finish_date: finish.map(str::to_owned),
        finished_level: 5,
        level_end_sp: None,
        level_start_sp: None,
        queue_position,
        skill_id,
        start_date: None,
        training_start_sp: None,
      }
    }

    async fn seed_queue(db: &Database, entries: &[CharacterSkillqueue]) {
      character::replace_skillqueue(db, CHARACTER, entries).await.unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_already_matured_skills_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-06-20T00:00:00+00:00"))]).await;

      let surfaced = skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "a skill already finished is watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_ignores_a_skill_still_training_and_one_without_a_finish_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(
        &db,
        &[entry(3300, 0, Some("2026-12-01T00:00:00+00:00")), entry(3301, 1, None)],
      )
      .await;
      // First scan over no matured rows watermarks nothing real.
      skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();

      let surfaced = skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "future and not-yet-started skills never mature");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_when_a_skill_matures_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-12-01T00:00:00+00:00"))]).await;
      skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();
      // The skill's finish_date has now passed relative to a later `now`.
      let later = DateTime::parse_from_rfc3339("2026-12-02T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc);

      let surfaced = skill_detector(&db, CHARACTER, later, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "skill:7300:3300:2026-12-01T00:00:00+00:00");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].body(), "Training complete");
      assert_eq!(surfaced[0].target().character, Some(CHARACTER));
      assert_eq!(
        surfaced[0].target().destination,
        crate::store::model::NotificationDestination::Skills
      );
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-06-20T00:00:00+00:00"))]).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::SkillMonitoring, false);

      let surfaced = skill_detector(&db, CHARACTER, now(), &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }
}
