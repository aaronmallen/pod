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
    .map(|s| skill_from_entry(character_id, s, active))
    .collect();

  if let Some(a) = active
    && !result.iter().any(|s| s.skill_id == a.skill_id)
  {
    result.push(skill_from_active_entry(character_id, a));
  }

  result
}

/// Builds a [`CharacterSkill`] from a `SkillEntry`, overlaying active-training state when the
/// entry matches the currently-training queue slot.
fn skill_from_entry(character_id: i64, s: SkillEntry, active: Option<&SkillQueueEntry>) -> CharacterSkill {
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
    active_level: if is_active {
      active.unwrap().finished_level
    } else {
      s.active_skill_level
    },
    character_id,
    is_active_training: is_active,
    skill_id: s.skill_id,
    skill_name: None,
    skillpoints: s.skillpoints_in_skill,
    trained_level: if is_active {
      (active.unwrap().finished_level - 1).max(0)
    } else {
      s.trained_skill_level
    },
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
    training_start_sp,
    training_start_time,
  }
}

/// Builds a [`CharacterSkill`] for a skill that appears only in the queue, not in the snapshot.
fn skill_from_active_entry(character_id: i64, a: &SkillQueueEntry) -> CharacterSkill {
  CharacterSkill {
    active_level: a.finished_level,
    character_id,
    is_active_training: true,
    skill_id: a.skill_id,
    skill_name: None,
    skillpoints: a.training_start_sp.unwrap_or(0) as i64,
    trained_level: (a.finished_level - 1).max(0),
    training_end_time: a.finish_date.as_deref().and_then(parse_eve_datetime),
    training_level_end_sp: a.level_end_sp.map(|sp| sp as i64),
    training_level_start_sp: a.level_start_sp.map(|sp| sp as i64),
    training_start_sp: a.training_start_sp.map(|sp| sp as i64),
    training_start_time: a.start_date.as_deref().and_then(parse_eve_datetime),
  }
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

  fn future_queue_entry(skill_id: i32, finished_level: i32) -> SkillQueueEntry {
    SkillQueueEntry {
      finish_date: Some("2099-12-31T12:30:00Z".to_string()),
      finished_level,
      level_end_sp: Some(45255),
      level_start_sp: Some(14142),
      queue_position: 0,
      skill_id,
      start_date: Some("2099-12-31T10:00:00Z".to_string()),
      training_start_sp: Some(14142),
    }
  }

  fn skill_entry(skill_id: i32, trained: i32, active: i32, sp: i64) -> SkillEntry {
    SkillEntry {
      active_skill_level: active,
      skill_id,
      skillpoints_in_skill: sp,
      trained_skill_level: trained,
    }
  }

  mod build_training_queue {
    use pretty_assertions::assert_eq;

    use super::*;

    fn character_skill(skill_id: i32, name: Option<&str>) -> CharacterSkill {
      CharacterSkill {
        active_level: 3,
        character_id: 123,
        is_active_training: false,
        skill_id,
        skill_name: name.map(str::to_owned),
        skillpoints: 10000,
        trained_level: 3,
        training_end_time: None,
        training_level_end_sp: None,
        training_level_start_sp: None,
        training_start_sp: None,
        training_start_time: None,
      }
    }

    #[test]
    fn it_returns_empty_for_empty_queue() {
      let result = build_training_queue(&[], &[]);

      assert_eq!(result.len(), 0);
    }

    #[test]
    fn it_maps_queue_entries_to_training_entries() {
      let queue = vec![future_queue_entry(3300, 4)];
      let skills = vec![character_skill(3300, Some("Gunnery"))];

      let result = build_training_queue(&queue, &skills);

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skill_id, 3300);
      assert_eq!(result[0].to_level, 4);
      assert_eq!(result[0].from_level, 3);
    }

    #[test]
    fn it_resolves_skill_name_from_skills_slice() {
      let queue = vec![future_queue_entry(3300, 4)];
      let skills = vec![character_skill(3300, Some("Gunnery"))];

      let result = build_training_queue(&queue, &skills);

      assert_eq!(result[0].skill_name, Some("Gunnery".to_owned()));
    }

    #[test]
    fn it_leaves_skill_name_none_when_not_in_skills_slice() {
      let queue = vec![future_queue_entry(9999, 1)];
      let skills: Vec<CharacterSkill> = vec![];

      let result = build_training_queue(&queue, &skills);

      assert_eq!(result[0].skill_name, None);
    }

    #[test]
    fn it_computes_from_level_as_finished_level_minus_one() {
      let queue = vec![future_queue_entry(3300, 3)];
      let skills: Vec<CharacterSkill> = vec![];

      let result = build_training_queue(&queue, &skills);

      assert_eq!(result[0].from_level, 2);
    }

    #[test]
    fn it_clamps_from_level_at_zero_for_level_one_entry() {
      let queue = vec![future_queue_entry(3300, 1)];
      let skills: Vec<CharacterSkill> = vec![];

      let result = build_training_queue(&queue, &skills);

      assert_eq!(result[0].from_level, 0);
    }

    #[test]
    fn it_parses_start_and_finish_dates() {
      let queue = vec![future_queue_entry(3300, 4)];
      let skills: Vec<CharacterSkill> = vec![];

      let result = build_training_queue(&queue, &skills);

      assert!(result[0].start_date.is_some());
      assert!(result[0].finish_date.is_some());
    }

    #[test]
    fn it_propagates_sp_fields() {
      let queue = vec![future_queue_entry(3300, 4)];
      let skills: Vec<CharacterSkill> = vec![];

      let result = build_training_queue(&queue, &skills);

      assert_eq!(result[0].training_start_sp, Some(14142));
      assert_eq!(result[0].level_start_sp, Some(14142));
      assert_eq!(result[0].level_end_sp, Some(45255));
    }
  }

  mod build_character_skills {
    use pretty_assertions::assert_eq;

    use super::*;

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
      let queue = vec![future_queue_entry(3300, 4)];
      let result = build_character_skills(123, skills, queue);

      assert_eq!(result.len(), 1);
      assert!(result[0].is_active_training);
      assert_eq!(result[0].active_level, 4);
    }

    #[test]
    fn it_derives_trained_level_from_queue_not_db_for_active_skill() {
      let skills = vec![skill_entry(3300, 1, 1, 500)];
      let queue = vec![future_queue_entry(3300, 5)];
      let result = build_character_skills(123, skills, queue);

      assert_eq!(result[0].trained_level, 4);
      assert_eq!(result[0].active_level, 5);
    }

    #[test]
    fn it_adds_queue_only_skill_not_in_snapshot() {
      let queue = vec![future_queue_entry(3301, 1)];
      let result = build_character_skills(123, vec![], queue);

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skill_id, 3301);
      assert!(result[0].is_active_training);
    }

    #[test]
    fn it_does_not_mark_skill_as_active_when_finish_date_is_in_the_past() {
      let skills = vec![skill_entry(3300, 2, 2, 45255)];
      let stale_entry = SkillQueueEntry {
        finish_date: Some("2024-01-15T12:30:00Z".to_string()),
        finished_level: 2,
        level_end_sp: Some(45255),
        level_start_sp: Some(14142),
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2024-01-15T10:00:00Z".to_string()),
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
        finish_date: None,
        finished_level: 2,
        level_end_sp: Some(45255),
        level_start_sp: Some(14142),
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2024-01-15T10:00:00Z".to_string()),
        training_start_sp: Some(14142),
      };
      let result = build_character_skills(123, skills, vec![no_finish]);

      assert_eq!(result.len(), 1);
      assert!(!result[0].is_active_training);
    }

    #[test]
    fn it_populates_training_sp_fields_for_active_skill() {
      let skills = vec![skill_entry(3300, 3, 3, 24000)];
      let queue = vec![future_queue_entry(3300, 4)];
      let result = build_character_skills(123, skills, queue);

      assert_eq!(result[0].training_level_end_sp, Some(45255));
      assert_eq!(result[0].training_level_start_sp, Some(14142));
      assert_eq!(result[0].training_start_sp, Some(14142));
    }

    #[test]
    fn it_clears_training_sp_fields_for_inactive_skill() {
      let skills = vec![skill_entry(3300, 3, 3, 24000)];
      let result = build_character_skills(123, skills, vec![]);

      assert!(result[0].training_level_end_sp.is_none());
      assert!(result[0].training_level_start_sp.is_none());
      assert!(result[0].training_start_sp.is_none());
    }

    #[test]
    fn it_uses_training_start_sp_as_skillpoints_for_queue_only_skill() {
      let queue = vec![future_queue_entry(3300, 1)];
      let result = build_character_skills(123, vec![], queue);

      assert_eq!(result[0].skillpoints, 14142);
    }
  }

  mod find_active_queue_entry {
    use super::*;

    #[test]
    fn it_returns_none_for_empty_queue() {
      let result = find_active_queue_entry(&[]);

      assert!(result.is_none());
    }

    #[test]
    fn it_returns_none_when_all_entries_lack_start_date() {
      let queue = vec![SkillQueueEntry {
        finish_date: Some("2099-12-31T12:30:00Z".to_string()),
        finished_level: 4,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 0,
        skill_id: 3300,
        start_date: None,
        training_start_sp: None,
      }];

      let result = find_active_queue_entry(&queue);

      assert!(result.is_none());
    }

    #[test]
    fn it_returns_none_when_finish_date_is_in_the_past() {
      let queue = vec![SkillQueueEntry {
        finish_date: Some("2020-01-01T00:00:00Z".to_string()),
        finished_level: 4,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2019-12-31T00:00:00Z".to_string()),
        training_start_sp: None,
      }];

      let result = find_active_queue_entry(&queue);

      assert!(result.is_none());
    }

    #[test]
    fn it_returns_the_entry_when_finish_date_is_in_the_future() {
      let queue = vec![future_queue_entry(3300, 4)];

      let result = find_active_queue_entry(&queue);

      assert!(result.is_some());
      assert_eq!(result.unwrap().skill_id, 3300);
    }

    #[test]
    fn it_returns_none_when_finish_date_is_absent() {
      let queue = vec![SkillQueueEntry {
        finish_date: None,
        finished_level: 4,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2099-12-31T10:00:00Z".to_string()),
        training_start_sp: None,
      }];

      let result = find_active_queue_entry(&queue);

      assert!(result.is_none());
    }
  }

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

  mod reconcile_skills {
    use pretty_assertions::assert_eq;

    use super::*;

    fn character_skill(skill_id: i32, trained: i32, active: i32, sp: i64) -> CharacterSkill {
      CharacterSkill {
        active_level: active,
        character_id: 123,
        is_active_training: false,
        skill_id,
        skill_name: None,
        skillpoints: sp,
        trained_level: trained,
        training_end_time: None,
        training_level_end_sp: None,
        training_level_start_sp: None,
        training_start_sp: None,
        training_start_time: None,
      }
    }

    #[test]
    fn it_returns_empty_when_both_inputs_are_empty() {
      let result = reconcile_skills(123, &[], vec![]);

      assert_eq!(result.len(), 0);
    }

    #[test]
    fn it_preserves_existing_skills_when_queue_is_empty() {
      let existing = vec![character_skill(3300, 3, 3, 24000)];

      let result = reconcile_skills(123, &existing, vec![]);

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skill_id, 3300);
      assert_eq!(result[0].trained_level, 3);
      assert!(!result[0].is_active_training);
    }

    #[test]
    fn it_marks_existing_skill_as_active_when_it_appears_in_queue() {
      let existing = vec![character_skill(3300, 3, 3, 24000)];
      let queue = vec![future_queue_entry(3300, 4)];

      let result = reconcile_skills(123, &existing, queue);

      assert_eq!(result.len(), 1);
      assert!(result[0].is_active_training);
      assert_eq!(result[0].active_level, 4);
    }

    #[test]
    fn it_clears_active_training_state_when_skill_not_in_queue() {
      let mut existing = vec![character_skill(3300, 3, 3, 24000)];
      existing[0].is_active_training = true;
      existing[0].training_end_time = Some(9999999999);

      let result = reconcile_skills(123, &existing, vec![]);

      assert!(!result[0].is_active_training);
      assert!(result[0].training_end_time.is_none());
    }

    #[test]
    fn it_derives_skill_entries_from_existing_character_skills() {
      let existing = vec![character_skill(3300, 4, 4, 45255)];
      let queue = vec![future_queue_entry(3300, 5)];

      let result = reconcile_skills(123, &existing, queue);

      assert_eq!(result[0].skill_id, 3300);
      assert_eq!(result[0].active_level, 5);
      assert_eq!(result[0].trained_level, 4);
    }
  }

  mod refresh_grant {
    use std::time::UNIX_EPOCH;

    use pretty_assertions::assert_eq;

    use super::*;

    fn make_character(id: i64, name: &str, access_token: &str, refresh_token: &str, expires_at: i64) -> Character {
      let mut c = Character::new(id, name);
      c.set_access_token(access_token)
        .set_refresh_token(refresh_token)
        .set_token_expires_at(expires_at);
      c
    }

    #[test]
    fn it_creates_a_grant_with_character_id_and_tokens() {
      let c = make_character(12345, "Test Pilot", "access_tok", "refresh_tok", 9999999999);

      let grant = refresh_grant(&c, "new_access_tok");

      assert_eq!(grant.character_id(), &12345i64);
      assert_eq!(grant.access_token(), "new_access_tok");
      assert_eq!(grant.refresh_token(), "refresh_tok");
    }

    #[test]
    fn it_sets_expires_at_from_token_expires_at_field() {
      let expires_ts: i64 = 9999999999;
      let c = make_character(12345, "Test Pilot", "at", "rt", expires_ts);

      let grant = refresh_grant(&c, "at");

      let expected = UNIX_EPOCH + std::time::Duration::from_secs(expires_ts as u64);
      assert_eq!(grant.expires_at(), &expected);
    }

    #[test]
    fn it_uses_the_supplied_access_token_not_the_character_one() {
      let c = make_character(12345, "Test Pilot", "old_access_tok", "refresh_tok", 9999999999);

      let grant = refresh_grant(&c, "brand_new_token");

      assert_eq!(grant.access_token(), "brand_new_token");
    }
  }

  mod scopes_from_access_token {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_token(payload_b64: &str) -> String {
      format!("header.{payload_b64}.signature")
    }

    #[test]
    fn it_returns_single_scope_string() {
      let payload = "eyJzY3AiOiAiZXNpLXNraWxscy5yZWFkX3NraWxscy52MSIsICJzdWIiOiAiQ0hBUkFDVEVSOkVWRToxMjM0NTY3In0";
      let token = make_token(payload);

      let result = scopes_from_access_token(&token);

      assert_eq!(result, Some("esi-skills.read_skills.v1".to_owned()));
    }

    #[test]
    fn it_joins_multiple_scopes_with_space() {
      let payload = "eyJzY3AiOiBbImVzaS1za2lsbHMucmVhZF9za2lsbHMudjEiLCAiZXNpLXdhbGxldC5yZWFkX2NoYXJhY3Rlcl93YWxsZXQudjEiXSwgInN1YiI6ICJDSEFSQUNURVI6RVZFOjEyMzQ1NjcifQ";
      let token = make_token(payload);

      let result = scopes_from_access_token(&token);

      assert_eq!(
        result,
        Some("esi-skills.read_skills.v1 esi-wallet.read_character_wallet.v1".to_owned())
      );
    }

    #[test]
    fn it_returns_none_when_scp_claim_is_absent() {
      let payload = "eyJzdWIiOiAiQ0hBUkFDVEVSOkVWRToxMjM0NTY3In0";
      let token = make_token(payload);

      let result = scopes_from_access_token(&token);

      assert_eq!(result, None);
    }

    #[test]
    fn it_returns_none_for_empty_scope_array() {
      let payload = "eyJzY3AiOiBbXSwgInN1YiI6ICJDSEFSQUNURVI6RVZFOjEyMzQ1NjcifQ";
      let token = make_token(payload);

      let result = scopes_from_access_token(&token);

      assert_eq!(result, None);
    }

    #[test]
    fn it_returns_none_for_token_with_no_dot_separator() {
      let result = scopes_from_access_token("notavalidjwt");

      assert_eq!(result, None);
    }

    #[test]
    fn it_returns_none_for_invalid_base64_payload() {
      let token = "header.!!!notbase64!!!.signature";

      let result = scopes_from_access_token(token);

      assert_eq!(result, None);
    }
  }
}
