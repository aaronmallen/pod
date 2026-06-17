const DAY: i64 = 86_400;
const HOUR: i64 = 3_600;
const MINUTE: i64 = 60;
const SP_FACTORS: [u64; 5] = [250, 1_414, 8_000, 45_255, 256_000];

pub fn fmt_dur_short(seconds: i64) -> String {
  if seconds <= 0 {
    return "\u{2014}".to_owned();
  }

  let days = seconds / DAY;
  let hours = (seconds % DAY) / HOUR;
  let minutes = (seconds % HOUR) / MINUTE;
  let secs = seconds % MINUTE;

  if days > 0 {
    format!("{days}d")
  } else if hours > 0 {
    format!("{hours}h")
  } else if minutes > 0 {
    format!("{minutes}m")
  } else {
    format!("{secs}s")
  }
}

pub fn sp_cost(rank: f64, level: u8) -> u64 {
  if !(1..=5).contains(&level) {
    return 0;
  }

  (SP_FACTORS[(level - 1) as usize] as f64 * rank).round() as u64
}

pub fn sp_per_sec(primary: u32, secondary: u32) -> f64 {
  (f64::from(primary) + f64::from(secondary) / 2.0) / 60.0
}

#[cfg(test)]
mod tests {
  use super::*;

  mod fmt_dur_short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_an_em_dash_for_zero_or_negative() {
      assert_eq!(fmt_dur_short(0), "—");
      assert_eq!(fmt_dur_short(-30), "—");
    }

    #[test]
    fn it_renders_bare_seconds_below_a_minute() {
      assert_eq!(fmt_dur_short(30), "30s");
    }

    #[test]
    fn it_renders_only_days_when_days_are_present() {
      assert_eq!(fmt_dur_short(3 * DAY + 4 * HOUR + 9 * MINUTE), "3d");
    }

    #[test]
    fn it_renders_only_hours_below_a_day() {
      assert_eq!(fmt_dur_short(2 * HOUR + 5 * MINUTE), "2h");
    }

    #[test]
    fn it_renders_only_minutes_below_an_hour() {
      assert_eq!(fmt_dur_short(7 * MINUTE + 12), "7m");
    }
  }

  mod sp_cost {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_matches_the_factor_table_at_rank_one() {
      assert_eq!(sp_cost(1.0, 1), 250);
      assert_eq!(sp_cost(1.0, 2), 1_414);
      assert_eq!(sp_cost(1.0, 3), 8_000);
      assert_eq!(sp_cost(1.0, 4), 45_255);
      assert_eq!(sp_cost(1.0, 5), 256_000);
    }

    #[test]
    fn it_returns_zero_for_levels_outside_one_to_five() {
      assert_eq!(sp_cost(1.0, 0), 0);
      assert_eq!(sp_cost(1.0, 6), 0);
    }

    #[test]
    fn it_scales_linearly_with_rank() {
      assert_eq!(sp_cost(2.0, 1), 500);
      assert_eq!(sp_cost(2.0, 2), 2_828);
      assert_eq!(sp_cost(2.0, 5), 512_000);
    }
  }

  mod sp_per_sec {
    use super::*;

    #[test]
    fn it_computes_the_pod_rate_formula() {
      let rate = sp_per_sec(27, 24);

      assert!((rate - 0.65).abs() < 0.01);
    }

    #[test]
    fn it_does_not_clamp_zero_attributes() {
      assert_eq!(sp_per_sec(0, 0), 0.0);
    }
  }
}
