use chrono::{DateTime, Utc};

use super::format::sp_cost;
use crate::store::model::CharacterSkillqueue;

pub fn roman(level: i64) -> String {
  match level {
    1 => "I".to_owned(),
    2 => "II".to_owned(),
    3 => "III".to_owned(),
    4 => "IV".to_owned(),
    5 => "V".to_owned(),
    other => other.to_string(),
  }
}

/// Drops queue entries whose `finish_date` has already passed so the first remaining entry is the
/// one genuinely in progress.
///
/// ESI does not always prune a just-finished skill before the next sync, leaving a completed entry
/// at the head of the raw queue. An entry is dropped only when its `finish_date` parses and is
/// `<= now`; entries with a null or unparseable `finish_date` (paused queue) are preserved. Mirrors
/// the `finish_date > now` predicate in `character::current_skillqueue`.
pub fn active_queue(queue: Vec<CharacterSkillqueue>, now: DateTime<Utc>) -> Vec<CharacterSkillqueue> {
  queue
    .into_iter()
    .filter(|entry| {
      entry
        .finish_date()
        .as_deref()
        .and_then(parse_timestamp)
        .is_none_or(|finish| finish > now)
    })
    .collect()
}

pub fn queue_entry_progress_sp(
  level_start_sp: i64,
  level_end_sp: i64,
  training_start_sp: i64,
  start_date: DateTime<Utc>,
  finish_date: DateTime<Utc>,
  now: DateTime<Utc>,
) -> f32 {
  let level_range = (level_end_sp - level_start_sp) as f64;
  if level_range <= 0.0 {
    return 1.0;
  }

  let run_duration = (finish_date - start_date).num_seconds() as f64;
  if run_duration <= 0.0 {
    return 1.0;
  }

  let sp_rate = (level_end_sp - training_start_sp) as f64 / run_duration;
  let elapsed = (now - start_date).num_seconds().max(0) as f64;
  let current_sp = training_start_sp as f64 + elapsed * sp_rate;

  (((current_sp - level_start_sp as f64) / level_range).clamp(0.0, 1.0)) as f32
}

pub fn queue_entry_progress(entry: &CharacterSkillqueue, now: DateTime<Utc>) -> f32 {
  let start = entry.start_date().as_deref().and_then(parse_timestamp);
  let finish = entry.finish_date().as_deref().and_then(parse_timestamp);

  if let (Some(level_start_sp), Some(level_end_sp), Some(training_start_sp), Some(start_date), Some(finish_date)) = (
    entry.level_start_sp(),
    entry.level_end_sp(),
    entry.training_start_sp(),
    start,
    finish,
  ) {
    return queue_entry_progress_sp(
      level_start_sp,
      level_end_sp,
      training_start_sp,
      start_date,
      finish_date,
      now,
    );
  }

  if let (Some(start_date), Some(finish_date)) = (start, finish)
    && finish_date > start_date
  {
    let elapsed = (now - start_date).num_seconds() as f32;
    let span = (finish_date - start_date).num_seconds() as f32;
    return (elapsed / span).clamp(0.0, 1.0);
  }

  0.0
}

pub fn sp_for_range(rank: u8, from_level: u8, to_level: u8) -> u64 {
  ((from_level + 1)..=to_level)
    .map(|level| sp_cost(f64::from(rank), level))
    .sum()
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;

  fn at(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, 0, 0).unwrap()
  }

  fn entry(
    start_date: Option<&str>,
    finish_date: Option<&str>,
    level_start_sp: Option<i64>,
    level_end_sp: Option<i64>,
    training_start_sp: Option<i64>,
  ) -> CharacterSkillqueue {
    CharacterSkillqueue {
      character_id: 42,
      finish_date: finish_date.map(ToOwned::to_owned),
      finished_level: 5,
      level_end_sp,
      level_start_sp,
      queue_position: 0,
      skill_id: 3300,
      start_date: start_date.map(ToOwned::to_owned),
      training_start_sp,
    }
  }

  mod active_queue {
    use pretty_assertions::assert_eq;

    use super::*;

    const LONG_RANGE_TARGETING: i64 = 3428;

    const SIGNATURE_ANALYSIS: i64 = 3426;

    fn queued(queue_position: i64, skill_id: i64, finish_date: Option<&str>) -> CharacterSkillqueue {
      CharacterSkillqueue {
        character_id: 42,
        finish_date: finish_date.map(ToOwned::to_owned),
        finished_level: 5,
        level_end_sp: None,
        level_start_sp: None,
        queue_position,
        skill_id,
        start_date: Some("2026-06-01T00:00:00Z".to_owned()),
        training_start_sp: None,
      }
    }

    #[test]
    fn it_drops_a_finished_head_and_surfaces_the_next_entry() {
      let queue = vec![
        queued(0, LONG_RANGE_TARGETING, Some("2026-06-05T00:00:00Z")),
        queued(1, SIGNATURE_ANALYSIS, Some("2026-06-20T00:00:00Z")),
      ];
      let now = at(2026, 6, 6, 0);

      let remaining = active_queue(queue, now);

      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].skill_id(), SIGNATURE_ANALYSIS);
    }

    #[test]
    fn it_keeps_a_future_finish_date_entry() {
      let queue = vec![queued(0, SIGNATURE_ANALYSIS, Some("2026-06-20T00:00:00Z"))];

      let remaining = active_queue(queue, at(2026, 6, 6, 0));

      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].skill_id(), SIGNATURE_ANALYSIS);
    }

    #[test]
    fn it_preserves_a_paused_entry_with_a_null_finish_date() {
      let queue = vec![queued(0, SIGNATURE_ANALYSIS, None)];

      let remaining = active_queue(queue, at(2026, 6, 6, 0));

      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].skill_id(), SIGNATURE_ANALYSIS);
    }

    #[test]
    fn it_returns_empty_when_every_entry_has_already_finished() {
      let queue = vec![
        queued(0, LONG_RANGE_TARGETING, Some("2026-06-05T00:00:00Z")),
        queued(1, SIGNATURE_ANALYSIS, Some("2026-06-05T12:00:00Z")),
      ];

      let remaining = active_queue(queue, at(2026, 6, 6, 0));

      assert!(remaining.is_empty());
    }
  }

  mod queue_entry_progress {
    use super::*;

    #[test]
    fn it_falls_back_to_linear_with_dates_only() {
      let entry = entry(
        Some("2026-06-01T00:00:00Z"),
        Some("2026-06-11T00:00:00Z"),
        None,
        None,
        None,
      );
      let now = at(2026, 6, 6, 0);

      let progress = queue_entry_progress(&entry, now);

      assert!((progress - 0.5).abs() < 0.001, "expected ~0.5, got {progress}");
    }

    #[test]
    fn it_returns_zero_with_no_dates() {
      let entry = entry(None, None, None, None, None);

      assert_eq!(queue_entry_progress(&entry, at(2026, 6, 6, 0)), 0.0);
    }

    #[test]
    fn it_uses_the_sp_path_when_all_five_fields_are_present() {
      let entry = entry(
        Some("2026-06-01T00:00:00Z"),
        Some("2026-06-11T00:00:00Z"),
        Some(45_255),
        Some(256_000),
        Some(45_255),
      );
      let now = at(2026, 6, 6, 0);

      let progress = queue_entry_progress(&entry, now);

      assert!((progress - 0.5).abs() < 0.001, "expected ~0.5, got {progress}");
    }
  }

  mod queue_entry_progress_sp {
    use super::*;

    #[test]
    fn it_clamps_to_one_when_finished() {
      let start = at(2026, 6, 1, 0);
      let finish = at(2026, 6, 11, 0);
      let now = at(2026, 6, 20, 0);

      assert_eq!(
        queue_entry_progress_sp(45_255, 256_000, 45_255, start, finish, now),
        1.0
      );
    }

    #[test]
    fn it_reports_partial_progress_mid_training() {
      let start = at(2026, 6, 1, 0);
      let finish = at(2026, 6, 11, 0);
      let now = at(2026, 6, 6, 0);

      let progress = queue_entry_progress_sp(45_255, 256_000, 45_255, start, finish, now);

      assert!((progress - 0.5).abs() < 0.001, "expected ~0.5, got {progress}");
    }

    #[test]
    fn it_returns_one_for_a_zero_level_range() {
      let start = at(2026, 6, 1, 0);
      let finish = at(2026, 6, 11, 0);
      let now = at(2026, 6, 6, 0);

      assert_eq!(
        queue_entry_progress_sp(256_000, 256_000, 45_255, start, finish, now),
        1.0
      );
    }

    #[test]
    fn it_returns_one_for_a_zero_run_duration() {
      let instant = at(2026, 6, 1, 0);

      assert_eq!(
        queue_entry_progress_sp(45_255, 256_000, 45_255, instant, instant, instant),
        1.0
      );
    }
  }

  mod sp_cost {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_re_exported_from_format() {
      assert_eq!(sp_cost(1.0, 5), 256_000);
    }
  }

  mod sp_for_range {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_zero_for_an_empty_range() {
      assert_eq!(sp_for_range(3, 5, 5), 0);
    }

    #[test]
    fn it_scales_with_rank() {
      assert_eq!(sp_for_range(2, 0, 1), 500);
    }

    #[test]
    fn it_sums_a_full_zero_to_five_run() {
      assert_eq!(sp_for_range(1, 0, 5), 250 + 1_414 + 8_000 + 45_255 + 256_000);
    }

    #[test]
    fn it_sums_each_crossed_level_boundary() {
      assert_eq!(sp_for_range(1, 1, 3), 9_414);
    }
  }

  mod sp_per_sec {
    use crate::features::skills::format::sp_per_sec;

    #[test]
    fn it_does_not_clamp_an_effective_attribute_above_twenty_seven() {
      let rate = sp_per_sec(32, 29);

      assert!((rate - (32.0 + 29.0 / 2.0) / 60.0).abs() < 1e-9, "got {rate}");
      assert!(rate > sp_per_sec(27, 24), "higher attributes must yield a higher rate");
    }
  }
}
