//! Character service: token management, skill building, and portrait fetching.

use pod_esi::models::{
  auth::Grant,
  character::{SkillEntry, SkillQueueEntry},
};
use pod_model::{Character, CharacterSkill, TrainingQueueEntry};

/// Builds the full training queue from raw ESI queue entries, resolving skill names from the
/// already-name-resolved `skills` slice.
pub fn build_training_queue(queue: &[SkillQueueEntry], skills: &[CharacterSkill]) -> Vec<TrainingQueueEntry> {
  let name_map: std::collections::HashMap<i32, Option<String>> =
    skills.iter().map(|s| (s.skill_id, s.skill_name.clone())).collect();

  queue
    .iter()
    .map(|entry| TrainingQueueEntry {
      skill_id: entry.skill_id,
      skill_name: name_map.get(&entry.skill_id).and_then(|n| n.clone()),
      from_level: (entry.finished_level - 1).max(0),
      to_level: entry.finished_level,
      start_date: entry.start_date.as_deref().and_then(parse_eve_datetime),
      finish_date: entry.finish_date.as_deref().and_then(parse_eve_datetime),
      training_start_sp: entry.training_start_sp.map(|sp| sp as i64),
      level_start_sp: entry.level_start_sp.map(|sp| sp as i64),
      level_end_sp: entry.level_end_sp.map(|sp| sp as i64),
    })
    .collect()
}

/// Returns the queue entry that is currently being trained, or `None` if the queue is idle.
///
/// A skill is considered actively training only when it has a `start_date` **and** a `finish_date`
/// that is still in the future. ESI can return a completed skill at position 0 with a past or
/// missing `finish_date` before removing it from the queue; those entries are excluded.
pub fn find_active_queue_entry(queue: &[SkillQueueEntry]) -> Option<&SkillQueueEntry> {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  queue.iter().find(|e| {
    e.start_date.is_some()
      && e
        .finish_date
        .as_deref()
        .and_then(parse_eve_datetime)
        .map(|t| t > now)
        .unwrap_or(false)
  })
}

/// Reconciles existing skill rows against a fresh queue, using DB-sourced skills as the base.
///
/// Unlike `build_character_skills`, which requires a full ESI skills snapshot, this overload
/// derives `SkillEntry` values from the character's current in-memory (DB-loaded) skills so that
/// every 120-second queue tick produces a complete, correct picture of all skills — not just the
/// one currently training. Skills absent from the queue have their active-training state cleared.
pub fn reconcile_skills(
  character_id: i64,
  existing: &[CharacterSkill],
  queue: Vec<SkillQueueEntry>,
) -> Vec<CharacterSkill> {
  let entries: Vec<SkillEntry> = existing
    .iter()
    .map(|s| SkillEntry {
      skill_id: s.skill_id,
      trained_skill_level: s.trained_level,
      active_skill_level: s.active_level,
      skillpoints_in_skill: s.skillpoints,
    })
    .collect();
  build_character_skills(character_id, entries, queue)
}

/// Builds a `CharacterSkill` list by merging a skill snapshot with the active skill queue.
pub fn build_character_skills(
  character_id: i64,
  skills: Vec<SkillEntry>,
  queue: Vec<SkillQueueEntry>,
) -> Vec<CharacterSkill> {
  let active = find_active_queue_entry(&queue);

  let mut result: Vec<CharacterSkill> = skills
    .into_iter()
    .map(|s| {
      let is_active = active.map(|a| a.skill_id == s.skill_id).unwrap_or(false);
      let (training_end_time, training_start_time, training_start_sp) = if is_active {
        let a = active.unwrap();
        (
          a.finish_date.as_deref().and_then(parse_eve_datetime),
          a.start_date.as_deref().and_then(parse_eve_datetime),
          a.training_start_sp.map(|sp| sp as i64),
        )
      } else {
        (None, None, None)
      };
      CharacterSkill {
        character_id,
        skill_id: s.skill_id,
        trained_level: if is_active {
          (active.unwrap().finished_level - 1).max(0)
        } else {
          s.trained_skill_level
        },
        active_level: if is_active {
          active.unwrap().finished_level
        } else {
          s.active_skill_level
        },
        skillpoints: s.skillpoints_in_skill,
        training_end_time,
        training_level_end_sp: if is_active {
          active.unwrap().level_end_sp.map(|sp| sp as i64)
        } else {
          None
        },
        training_level_start_sp: if is_active {
          active.unwrap().level_start_sp.map(|sp| sp as i64)
        } else {
          None
        },
        training_start_time,
        training_start_sp,
        is_active_training: is_active,
        skill_name: None,
      }
    })
    .collect();

  if let Some(a) = active
    && !result.iter().any(|s| s.skill_id == a.skill_id)
  {
    result.push(CharacterSkill {
      character_id,
      skill_id: a.skill_id,
      trained_level: (a.finished_level - 1).max(0),
      active_level: a.finished_level,
      skillpoints: a.training_start_sp.unwrap_or(0) as i64,
      training_end_time: a.finish_date.as_deref().and_then(parse_eve_datetime),
      training_level_end_sp: a.level_end_sp.map(|sp| sp as i64),
      training_level_start_sp: a.level_start_sp.map(|sp| sp as i64),
      training_start_time: a.start_date.as_deref().and_then(parse_eve_datetime),
      training_start_sp: a.training_start_sp.map(|sp| sp as i64),
      is_active_training: true,
      skill_name: None,
    });
  }

  result
}

/// Ensures the character's access token is valid, refreshing via ESI if it has expired.
///
/// Returns `Some(token)` on success or `None` if the refresh failed (caller should skip
/// the character silently).
pub async fn ensure_valid_token(character: &Character, esi: &pod_esi::Client, db: &pod_db::Repo) -> Option<String> {
  if !character.access_token_expired() {
    return Some(character.access_token().clone());
  }

  let expires_at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(*character.token_expires_at() as u64);
  let grant = Grant::new(
    character.access_token().clone(),
    *character.id(),
    character.name().clone(),
    expires_at,
    character.refresh_token().clone(),
    vec![],
  );

  let new_grant = esi.auth().refresh(&grant).await.ok()?;

  let new_expires_at = new_grant
    .expires_at()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);

  db.characters()
    .update_token(
      *character.id(),
      new_grant.access_token(),
      new_grant.refresh_token(),
      new_expires_at,
    )
    .await
    .ok()?;

  Some(new_grant.access_token().clone())
}

/// Decodes the payload of an EVE SSO JWT access token and returns the granted
/// scopes as a space-separated string. Returns `None` if the token cannot be
/// decoded or contains no scopes.
///
/// EVE's `scp` claim is a string when one scope is granted and an array when
/// multiple scopes are granted.
fn scopes_from_access_token(token: &str) -> Option<String> {
  use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

  let payload_b64 = token.split('.').nth(1)?;
  let bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
  let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;

  match json.get("scp")? {
    serde_json::Value::String(s) => Some(s.clone()),
    serde_json::Value::Array(arr) => {
      let scopes: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
      if scopes.is_empty() {
        None
      } else {
        Some(scopes.join(" "))
      }
    }
    _ => None,
  }
}

/// Backfills `granted_scopes` for a character whose token predates the column.
/// Decodes the access token JWT locally — no network call required.
pub async fn backfill_granted_scopes(character: &mut Character, access_token: &str, db: &pod_db::Repo) {
  if character.granted_scopes().as_deref().is_some_and(|s| !s.is_empty()) {
    return;
  }
  let Some(scopes) = scopes_from_access_token(access_token) else {
    return;
  };
  if db
    .characters()
    .update_granted_scopes(*character.id(), &scopes)
    .await
    .is_ok()
  {
    character.set_granted_scopes(Some(scopes));
  }
}

/// Fetches a character portrait from the EVE image server, returning the raw PNG bytes.
pub async fn fetch_portrait(character_id: i64, esi: &pod_esi::Client) -> Option<Vec<u8>> {
  esi.images().character_portrait(character_id, 256).await.ok()
}

/// Resolves skill names by querying the ESI universe endpoint and annotates each skill in place.
pub async fn inject_skill_names(mut skills: Vec<CharacterSkill>, esi: &pod_esi::Client) -> Vec<CharacterSkill> {
  let ids: Vec<i64> = skills.iter().map(|s| s.skill_id as i64).collect();
  if ids.is_empty() {
    return skills;
  }
  if let Ok(names) = esi.universe().names(&ids).await {
    let map: std::collections::HashMap<i32, String> = names.into_iter().map(|n| (n.id as i32, n.name)).collect();
    for skill in &mut skills {
      if let Some(name) = map.get(&skill.skill_id) {
        skill.skill_name = Some(name.clone());
      }
    }
  }
  skills
}

/// Constructs a refreshed `Grant` for a character using the given access token.
pub fn refresh_grant(character: &Character, access_token: &str) -> Grant {
  let expires_at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(*character.token_expires_at() as u64);
  Grant::new(
    access_token,
    *character.id(),
    character.name().clone(),
    expires_at,
    character.refresh_token().clone(),
    vec![],
  )
}

/// Parses an EVE datetime string (`YYYY-MM-DDTHH:MM:SSZ`) into a Unix timestamp.
fn parse_eve_datetime(s: &str) -> Option<i64> {
  let s = s.trim_end_matches('Z');
  let (date, time) = s.split_once('T')?;
  let mut dp = date.split('-');
  let y: i64 = dp.next()?.parse().ok()?;
  let mo: i64 = dp.next()?.parse().ok()?;
  let d: i64 = dp.next()?.parse().ok()?;
  let mut tp = time.split(':');
  let h: i64 = tp.next()?.parse().ok()?;
  let mi: i64 = tp.next()?.parse().ok()?;
  let sec: i64 = tp.next().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) as i64;

  let y = if mo <= 2 { y - 1 } else { y };
  let mo = if mo > 2 { mo - 3 } else { mo + 9 };
  let c = y / 100;
  let ya = y - 100 * c;
  let j = (146097 * c) / 4 + (1461 * ya) / 4 + (153 * mo + 2) / 5 + d + 1721119;
  let unix_days = j - 2440588;
  Some(unix_days * 86400 + h * 3600 + mi * 60 + sec)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod parse_eve_datetime {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_a_known_datetime() {
      let ts = parse_eve_datetime("2024-01-15T12:30:00Z");

      assert_eq!(ts, Some(1705321800));
    }

    #[test]
    fn it_handles_missing_z_suffix() {
      let with_z = parse_eve_datetime("2024-01-15T12:30:00Z");
      let without_z = parse_eve_datetime("2024-01-15T12:30:00");

      assert_eq!(with_z, without_z);
    }

    #[test]
    fn it_returns_none_for_malformed_input() {
      let result = parse_eve_datetime("not-a-date");

      assert_eq!(result, None);
    }
  }

  mod build_character_skills {
    use pod_esi::models::character::{SkillEntry, SkillQueueEntry};
    use pretty_assertions::assert_eq;

    use super::*;

    fn skill_entry(skill_id: i32, trained: i32, active: i32, sp: i64) -> SkillEntry {
      SkillEntry {
        skill_id,
        trained_skill_level: trained,
        active_skill_level: active,
        skillpoints_in_skill: sp,
      }
    }

    fn queue_entry(skill_id: i32, finished_level: i32) -> SkillQueueEntry {
      SkillQueueEntry {
        skill_id,
        finished_level,
        queue_position: 0,
        start_date: Some("2099-12-31T10:00:00Z".to_string()),
        finish_date: Some("2099-12-31T12:30:00Z".to_string()),
        level_end_sp: Some(45255),
        level_start_sp: Some(14142),
        training_start_sp: Some(14142),
      }
    }

    #[test]
    fn it_builds_skills_without_a_queue() {
      let skills = vec![skill_entry(3300, 3, 3, 24000)];
      let result = build_character_skills(123, skills, vec![]);

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skill_id, 3300);
      assert_eq!(result[0].trained_level, 3);
      assert!(!result[0].is_active_training);
    }

    #[test]
    fn it_marks_active_training_skill() {
      let skills = vec![skill_entry(3300, 3, 3, 24000)];
      let queue = vec![queue_entry(3300, 4)];
      let result = build_character_skills(123, skills, queue);

      assert_eq!(result.len(), 1);
      assert!(result[0].is_active_training);
      assert_eq!(result[0].active_level, 4);
    }

    #[test]
    fn it_derives_trained_level_from_queue_not_db_for_active_skill() {
      let skills = vec![skill_entry(3300, 1, 1, 500)];
      let queue = vec![queue_entry(3300, 5)];
      let result = build_character_skills(123, skills, queue);

      assert_eq!(result[0].trained_level, 4);
      assert_eq!(result[0].active_level, 5);
    }

    #[test]
    fn it_adds_queue_only_skill_not_in_snapshot() {
      let queue = vec![queue_entry(3301, 1)];
      let result = build_character_skills(123, vec![], queue);

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skill_id, 3301);
      assert!(result[0].is_active_training);
    }

    #[test]
    fn it_does_not_mark_skill_as_active_when_finish_date_is_in_the_past() {
      let skills = vec![skill_entry(3300, 2, 2, 45255)];
      let stale_entry = SkillQueueEntry {
        skill_id: 3300,
        finished_level: 2,
        queue_position: 0,
        start_date: Some("2024-01-15T10:00:00Z".to_string()),
        finish_date: Some("2024-01-15T12:30:00Z".to_string()),
        level_end_sp: Some(45255),
        level_start_sp: Some(14142),
        training_start_sp: Some(14142),
      };
      let result = build_character_skills(123, skills, vec![stale_entry]);

      assert_eq!(result.len(), 1);
      assert!(!result[0].is_active_training);
      assert!(result[0].training_end_time.is_none());
    }

    #[test]
    fn it_does_not_mark_skill_as_active_when_finish_date_is_absent() {
      let skills = vec![skill_entry(3300, 2, 2, 45255)];
      let no_finish = SkillQueueEntry {
        skill_id: 3300,
        finished_level: 2,
        queue_position: 0,
        start_date: Some("2024-01-15T10:00:00Z".to_string()),
        finish_date: None,
        level_end_sp: Some(45255),
        level_start_sp: Some(14142),
        training_start_sp: Some(14142),
      };
      let result = build_character_skills(123, skills, vec![no_finish]);

      assert_eq!(result.len(), 1);
      assert!(!result[0].is_active_training);
    }
  }
}
