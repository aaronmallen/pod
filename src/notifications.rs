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
  for owner in list.iter().chain(surfaced).map(Notification::owner) {
    if names.contains_key(&owner) {
      continue;
    }
    let name = match owner {
      NotificationOwner::Character(id) => character_name(db, id).await,
      NotificationOwner::Corporation(id) => corporation_name(db, id).await,
    };
    names.insert(owner, name);
  }
  names
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
      .unwrap_or_else(|| "New EVE mail".to_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: format!("From {}", header.from_name()),
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
    let ship = type_name(db, ship_type_id).await.unwrap_or_else(|| "ship".to_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: format!("{ship} destroyed"),
        dedup_key: killmail_key(killmail_id),
        kind: NotificationKind::Killmail,
        owner,
        target: NotificationTarget {
          character: matches!(owner, NotificationOwner::Character(_)).then_some(subject_id),
          destination: NotificationDestination::CharacterDetail,
          sub: None,
        },
        title: "New killmail".to_owned(),
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
    let skill = type_name(db, skill_id).await.unwrap_or_else(|| "Skill".to_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: "Training complete".to_owned(),
        dedup_key: skill_key(character_id, skill_id, &finish),
        kind: NotificationKind::Skill,
        owner: NotificationOwner::Character(character_id),
        target: NotificationTarget {
          character: Some(character_id),
          destination: NotificationDestination::Skills,
          sub: None,
        },
        title: format!("{skill} finished"),
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
      Some(id) => type_name(db, id).await.unwrap_or_else(|| "job".to_owned()),
      None => "job".to_owned(),
    };
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: format!("{product} ready"),
        dedup_key: industry_key(job_id),
        kind: NotificationKind::Industry,
        owner,
        target: NotificationTarget {
          character: matches!(owner, NotificationOwner::Character(_)).then_some(subject_id),
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: "Industry job complete".to_owned(),
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
        title: "Extraction scheduled".to_owned(),
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
        body: moon_name.unwrap_or_else(|| "Ready to mine".to_owned()),
        dedup_key: extraction_cracked_key(corporation_id, structure_id, moon_id, &arrival),
        kind: NotificationKind::ExtractionCracked,
        owner: NotificationOwner::Corporation(corporation_id),
        target: NotificationTarget {
          character: None,
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: "Moon chunk fractured".to_owned(),
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
}
