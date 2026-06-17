const EM_DASH: &str = "\u{2014}";
const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;

pub fn corp_ticker_label(ticker: Option<&str>, corporation_id: i64) -> String {
  match ticker {
    Some(ticker) => ticker.to_owned(),
    None => format!("CORP {corporation_id}"),
  }
}

pub fn fmt_count(count: i64) -> String {
  group_digits(count)
}

/// Includes a seconds component; returns `"0m"` for non-positive input.
pub fn fmt_duration(seconds: i64) -> String {
  if seconds <= 0 {
    return "0m".to_owned();
  }
  let days = seconds / SECONDS_PER_DAY;
  let hours = (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
  let minutes = (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
  let secs = seconds % SECONDS_PER_MINUTE;
  if days > 0 {
    format!("{days}d {hours}h")
  } else if hours > 0 {
    format!("{hours}h {minutes}m")
  } else if minutes > 0 {
    format!("{minutes}m {secs}s")
  } else {
    format!("{secs}s")
  }
}

/// Omits seconds entirely; clamps negatives to zero and bottoms out at `"0m"`.
pub fn fmt_duration_coarse(seconds: i64) -> String {
  let total = seconds.max(0);
  let days = total / SECONDS_PER_DAY;
  let hours = (total % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
  let minutes = (total % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
  if days > 0 {
    format!("{days}d {hours}h")
  } else if hours > 0 {
    format!("{hours}h {minutes}m")
  } else {
    format!("{minutes}m")
  }
}

/// Zero-pads hours and minutes; returns an em-dash for non-positive input.
pub fn fmt_duration_padded(seconds: i64) -> String {
  if seconds <= 0 {
    return EM_DASH.to_owned();
  }
  let days = seconds / SECONDS_PER_DAY;
  let hours = (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
  let minutes = (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
  if days > 0 {
    format!("{days}d {hours:02}h {minutes:02}m")
  } else if hours > 0 {
    format!("{hours}h {minutes:02}m")
  } else {
    format!("{minutes}m")
  }
}

pub fn fmt_isk(value: f64) -> String {
  let sign = if value < 0.0 { "-" } else { "" };
  let magnitude = value.abs();
  if magnitude >= 1_000_000_000_000.0 {
    format!("{sign}{:.1}T", magnitude / 1_000_000_000_000.0)
  } else if magnitude >= 1_000_000_000.0 {
    format!("{sign}{:.1}B", magnitude / 1_000_000_000.0)
  } else if magnitude >= 1_000_000.0 {
    format!("{sign}{:.1}M", magnitude / 1_000_000.0)
  } else if magnitude >= 1_000.0 {
    format!("{sign}{:.1}K", magnitude / 1_000.0)
  } else {
    format!("{value:.0}")
  }
}

/// Full unabbreviated integer (rounded, comma-grouped); no suffix.
pub fn fmt_isk_full(value: f64) -> String {
  group_digits(value.round() as i64)
}

pub fn fmt_isk_opt(balance: Option<f64>) -> String {
  match balance {
    Some(value) => fmt_isk(value),
    None => EM_DASH.to_owned(),
  }
}

/// Two-decimal millions (`1.23M`), whole-number uppercase-K thousands (`12K`).
pub fn fmt_sp(sp: i64) -> String {
  if sp >= 1_000_000 {
    format!("{:.2}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}K", sp as f64 / 1_000.0)
  } else {
    sp.to_string()
  }
}

/// One-decimal thousands (`2.5k`, lowercase k) and millions (`1.5M`).
pub fn fmt_sp_compact(sp: u64) -> String {
  if sp >= 1_000_000 {
    format!("{:.1}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.1}k", sp as f64 / 1_000.0)
  } else {
    sp.to_string()
  }
}

pub fn fmt_sp_labeled(sp: u64) -> String {
  if sp >= 1_000_000 {
    format!("{:.1}M SP", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}k SP", sp as f64 / 1_000.0)
  } else {
    format!("{sp} SP")
  }
}

/// Em-dash for `None` or `Some(0)`; one-decimal millions, whole-number uppercase-K thousands.
pub fn fmt_sp_opt(total: Option<i64>) -> String {
  match total {
    None | Some(0) => EM_DASH.to_owned(),
    Some(value) => {
      let n = value as f64;
      if n >= 1e6 {
        format!("{:.1}M", n / 1e6)
      } else if n >= 1e3 {
        format!("{:.0}K", n / 1e3)
      } else {
        value.to_string()
      }
    }
  }
}

/// Whole-number thousands (`12k`, no decimal), one-decimal millions; no suffix.
pub fn fmt_sp_short(sp: u64) -> String {
  if sp >= 1_000_000 {
    format!("{:.1}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}k", sp as f64 / 1_000.0)
  } else {
    sp.to_string()
  }
}

pub fn fmt_volume(volume: f64) -> String {
  let magnitude = volume.abs();
  if magnitude >= 1e6 {
    format!("{:.1}Mm\u{b3}", volume / 1e6)
  } else if magnitude >= 1e3 {
    format!("{:.1}km\u{b3}", volume / 1e3)
  } else {
    format!("{volume:.0}m\u{b3}")
  }
}

pub fn skill_label(name: Option<&str>, skill_id: i64) -> String {
  match name {
    Some(name) => name.to_owned(),
    None => format!("Skill {skill_id}"),
  }
}

fn group_digits(value: i64) -> String {
  let digits = value.unsigned_abs().to_string();
  let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
  for (index, ch) in digits.chars().enumerate() {
    if index > 0 && (digits.len() - index).is_multiple_of(3) {
      grouped.push(',');
    }
    grouped.push(ch);
  }
  if value < 0 { format!("-{grouped}") } else { grouped }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod corp_ticker_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_the_ticker() {
      assert_eq!(corp_ticker_label(Some("CBLT"), 98_000_001), "CBLT");
    }

    #[test]
    fn it_falls_back_to_the_corporation_id() {
      assert_eq!(corp_ticker_label(None, 98_000_001), "CORP 98000001");
    }
  }

  mod fmt_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_thousands_with_commas() {
      assert_eq!(fmt_count(1_234_567), "1,234,567");
    }

    #[test]
    fn it_leaves_small_numbers_ungrouped() {
      assert_eq!(fmt_count(42), "42");
    }

    #[test]
    fn it_prefixes_negatives_with_a_sign() {
      assert_eq!(fmt_count(-12_345), "-12,345");
    }
  }

  mod fmt_duration {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_days_and_hours() {
      assert_eq!(fmt_duration(90_061), "1d 1h");
    }

    #[test]
    fn it_renders_hours_and_minutes() {
      assert_eq!(fmt_duration(3_661), "1h 1m");
    }

    #[test]
    fn it_renders_minutes_and_seconds() {
      assert_eq!(fmt_duration(61), "1m 1s");
    }

    #[test]
    fn it_renders_bare_seconds() {
      assert_eq!(fmt_duration(5), "5s");
    }

    #[test]
    fn it_renders_zero_as_zero_minutes() {
      assert_eq!(fmt_duration(0), "0m");
    }
  }

  mod fmt_duration_coarse {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_omits_seconds() {
      assert_eq!(fmt_duration_coarse(90_061), "1d 1h");
      assert_eq!(fmt_duration_coarse(3_661), "1h 1m");
      assert_eq!(fmt_duration_coarse(61), "1m");
    }

    #[test]
    fn it_clamps_negatives_to_zero_minutes() {
      assert_eq!(fmt_duration_coarse(-5), "0m");
    }
  }

  mod fmt_duration_padded {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pads_hours_and_minutes() {
      assert_eq!(fmt_duration_padded(90_061), "1d 01h 01m");
    }

    #[test]
    fn it_renders_an_em_dash_for_non_positive() {
      assert_eq!(fmt_duration_padded(0), EM_DASH);
    }
  }

  mod fmt_isk {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_scales_trillions() {
      assert_eq!(fmt_isk(2_500_000_000_000.0), "2.5T");
    }

    #[test]
    fn it_scales_billions() {
      assert_eq!(fmt_isk(1_234_567_890.0), "1.2B");
    }

    #[test]
    fn it_scales_millions() {
      assert_eq!(fmt_isk(2_500_000.0), "2.5M");
    }

    #[test]
    fn it_renders_small_values_without_a_suffix() {
      assert_eq!(fmt_isk(42.0), "42");
    }

    #[test]
    fn it_prefixes_negatives_with_a_sign() {
      assert_eq!(fmt_isk(-1_500_000_000.0), "-1.5B");
    }
  }

  mod fmt_isk_full {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_the_rounded_value() {
      assert_eq!(fmt_isk_full(1_234_567.8), "1,234,568");
    }
  }

  mod fmt_isk_opt {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_an_em_dash_for_none() {
      assert_eq!(fmt_isk_opt(None), EM_DASH);
    }

    #[test]
    fn it_formats_some() {
      assert_eq!(fmt_isk_opt(Some(2_500_000.0)), "2.5M");
    }
  }

  mod fmt_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_scales_millions_with_two_decimals() {
      assert_eq!(fmt_sp(1_234_567), "1.23M");
    }

    #[test]
    fn it_scales_thousands() {
      assert_eq!(fmt_sp(12_400), "12K");
    }

    #[test]
    fn it_renders_small_totals_verbatim() {
      assert_eq!(fmt_sp(420), "420");
    }
  }

  mod fmt_sp_compact {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_a_lowercase_thousands_suffix() {
      assert_eq!(fmt_sp_compact(2_500), "2.5k");
    }

    #[test]
    fn it_scales_millions() {
      assert_eq!(fmt_sp_compact(1_500_000), "1.5M");
    }
  }

  mod fmt_sp_labeled {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_a_unit_suffix() {
      assert_eq!(fmt_sp_labeled(1_500_000), "1.5M SP");
      assert_eq!(fmt_sp_labeled(12_400), "12k SP");
      assert_eq!(fmt_sp_labeled(420), "420 SP");
    }
  }

  mod fmt_sp_opt {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_an_em_dash_for_none_or_zero() {
      assert_eq!(fmt_sp_opt(None), EM_DASH);
      assert_eq!(fmt_sp_opt(Some(0)), EM_DASH);
    }

    #[test]
    fn it_scales_millions_with_one_decimal() {
      assert_eq!(fmt_sp_opt(Some(1_500_000)), "1.5M");
    }
  }

  mod fmt_sp_short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_the_unit_suffix() {
      assert_eq!(fmt_sp_short(1_500_000), "1.5M");
      assert_eq!(fmt_sp_short(12_400), "12k");
    }
  }

  mod fmt_volume {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_scales_to_mega_cubic_metres() {
      assert_eq!(fmt_volume(2_500_000.0), "2.5Mm\u{b3}");
    }

    #[test]
    fn it_scales_to_kilo_cubic_metres() {
      assert_eq!(fmt_volume(3_400.0), "3.4km\u{b3}");
    }

    #[test]
    fn it_renders_small_volumes_in_cubic_metres() {
      assert_eq!(fmt_volume(42.0), "42m\u{b3}");
    }
  }

  mod skill_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_the_skill_name() {
      assert_eq!(skill_label(Some("Gunnery"), 3300), "Gunnery");
    }

    #[test]
    fn it_falls_back_to_the_skill_id() {
      assert_eq!(skill_label(None, 3300), "Skill 3300");
    }
  }
}
