use chrono::Weekday;

const EM_DASH: &str = "\u{2014}";
const MONTHS_LONG: [&str; 12] = [
  "date.month.long_jan",
  "date.month.long_feb",
  "date.month.long_mar",
  "date.month.long_apr",
  "date.month.long_may",
  "date.month.long_jun",
  "date.month.long_jul",
  "date.month.long_aug",
  "date.month.long_sep",
  "date.month.long_oct",
  "date.month.long_nov",
  "date.month.long_dec",
];
const MONTHS_SHORT: [&str; 12] = [
  "date.month.short_jan",
  "date.month.short_feb",
  "date.month.short_mar",
  "date.month.short_apr",
  "date.month.short_may",
  "date.month.short_jun",
  "date.month.short_jul",
  "date.month.short_aug",
  "date.month.short_sep",
  "date.month.short_oct",
  "date.month.short_nov",
  "date.month.short_dec",
];
const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;
const WEEKDAYS_LONG: [&str; 7] = [
  "date.weekday.long_mon",
  "date.weekday.long_tue",
  "date.weekday.long_wed",
  "date.weekday.long_thu",
  "date.weekday.long_fri",
  "date.weekday.long_sat",
  "date.weekday.long_sun",
];
const WEEKDAYS_SHORT: [&str; 7] = [
  "date.weekday.short_mon",
  "date.weekday.short_tue",
  "date.weekday.short_wed",
  "date.weekday.short_thu",
  "date.weekday.short_fri",
  "date.weekday.short_sat",
  "date.weekday.short_sun",
];

pub fn corp_ticker_label(ticker: Option<&str>, corporation_id: i64) -> String {
  match ticker {
    Some(ticker) => ticker.to_owned(),
    None => format!("CORP {corporation_id}"),
  }
}

pub fn fmt_count(count: i64) -> String {
  group_digits(count)
}

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

pub fn fmt_isk_full(value: f64) -> String {
  group_digits(value.round() as i64)
}

pub fn fmt_isk_opt(balance: Option<f64>) -> String {
  match balance {
    Some(value) => fmt_isk(value),
    None => EM_DASH.to_owned(),
  }
}

pub fn fmt_sp(sp: i64) -> String {
  if sp >= 1_000_000 {
    format!("{:.2}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}K", sp as f64 / 1_000.0)
  } else {
    sp.to_string()
  }
}

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

/// Parses a friendly ISK string into a rounded ISK amount.
///
/// Accepts a trailing magnitude suffix (`k`/`m`/`b`/`t`, case-insensitive),
/// strips grouping separators (commas, spaces, underscores, and the narrow
/// no-break space EVE uses), and rounds to a whole number. Empty, blank, or
/// otherwise non-numeric input yields `0.0`. Mirrors the design's `parseIsk`.
pub fn month_long(month: u32) -> String {
  t!(MONTHS_LONG[month_index(month)]).into_owned()
}

pub fn month_short(month: u32) -> String {
  t!(MONTHS_SHORT[month_index(month)]).into_owned()
}

pub fn parse_isk(input: &str) -> f64 {
  let lowered = input.trim().to_lowercase();
  let stripped: String = lowered
    .chars()
    .filter(|ch| !matches!(ch, ',' | ' ' | '_' | '\u{202f}'))
    .collect();
  if stripped.is_empty() || stripped == "-" {
    return 0.0;
  }

  let (number, multiplier) = match stripped.chars().last() {
    Some('t') => (&stripped[..stripped.len() - 1], 1e12),
    Some('b') => (&stripped[..stripped.len() - 1], 1e9),
    Some('m') => (&stripped[..stripped.len() - 1], 1e6),
    Some('k') => (&stripped[..stripped.len() - 1], 1e3),
    _ => (stripped.as_str(), 1.0),
  };

  match number.parse::<f64>() {
    Ok(value) if value.is_finite() => (value * multiplier).round(),
    _ => 0.0,
  }
}

pub fn skill_label(name: Option<&str>, skill_id: i64) -> String {
  match name {
    Some(name) => name.to_owned(),
    None => format!("Skill {skill_id}"),
  }
}

pub fn weekday_long(weekday: Weekday) -> String {
  t!(WEEKDAYS_LONG[weekday.num_days_from_monday() as usize]).into_owned()
}

pub fn weekday_short(weekday: Weekday) -> String {
  t!(WEEKDAYS_SHORT[weekday.num_days_from_monday() as usize]).into_owned()
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

fn month_index(month: u32) -> usize {
  (month.clamp(1, 12) - 1) as usize
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::services::i18n::{Language, set_locale};

  mod corp_ticker_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_corporation_id() {
      assert_eq!(corp_ticker_label(None, 98_000_001), "CORP 98000001");
    }

    #[test]
    fn it_prefers_the_ticker() {
      assert_eq!(corp_ticker_label(Some("CBLT"), 98_000_001), "CBLT");
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
    fn it_renders_bare_seconds() {
      assert_eq!(fmt_duration(5), "5s");
    }

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
    fn it_renders_zero_as_zero_minutes() {
      assert_eq!(fmt_duration(0), "0m");
    }
  }

  mod fmt_duration_coarse {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_negatives_to_zero_minutes() {
      assert_eq!(fmt_duration_coarse(-5), "0m");
    }

    #[test]
    fn it_omits_seconds() {
      assert_eq!(fmt_duration_coarse(90_061), "1d 1h");
      assert_eq!(fmt_duration_coarse(3_661), "1h 1m");
      assert_eq!(fmt_duration_coarse(61), "1m");
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
    fn it_prefixes_negatives_with_a_sign() {
      assert_eq!(fmt_isk(-1_500_000_000.0), "-1.5B");
    }

    #[test]
    fn it_renders_small_values_without_a_suffix() {
      assert_eq!(fmt_isk(42.0), "42");
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
    fn it_scales_trillions() {
      assert_eq!(fmt_isk(2_500_000_000_000.0), "2.5T");
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
    fn it_formats_some() {
      assert_eq!(fmt_isk_opt(Some(2_500_000.0)), "2.5M");
    }

    #[test]
    fn it_renders_an_em_dash_for_none() {
      assert_eq!(fmt_isk_opt(None), EM_DASH);
    }
  }

  mod fmt_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_small_totals_verbatim() {
      assert_eq!(fmt_sp(420), "420");
    }

    #[test]
    fn it_scales_millions_with_two_decimals() {
      assert_eq!(fmt_sp(1_234_567), "1.23M");
    }

    #[test]
    fn it_scales_thousands() {
      assert_eq!(fmt_sp(12_400), "12K");
    }
  }

  mod fmt_sp_compact {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_scales_millions() {
      assert_eq!(fmt_sp_compact(1_500_000), "1.5M");
    }

    #[test]
    fn it_uses_a_lowercase_thousands_suffix() {
      assert_eq!(fmt_sp_compact(2_500), "2.5k");
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
    fn it_renders_small_volumes_in_cubic_metres() {
      assert_eq!(fmt_volume(42.0), "42m\u{b3}");
    }

    #[test]
    fn it_scales_to_kilo_cubic_metres() {
      assert_eq!(fmt_volume(3_400.0), "3.4km\u{b3}");
    }

    #[test]
    fn it_scales_to_mega_cubic_metres() {
      assert_eq!(fmt_volume(2_500_000.0), "2.5Mm\u{b3}");
    }
  }

  mod month_long {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_long_month_name() {
      set_locale(Language::En);

      assert_eq!(month_long(1), "January");
      assert_eq!(month_long(6), "June");
      assert_eq!(month_long(12), "December");
    }

    #[test]
    fn it_clamps_a_month_below_the_range() {
      set_locale(Language::En);

      assert_eq!(month_long(0), "January");
    }

    #[test]
    fn it_clamps_a_month_above_the_range() {
      set_locale(Language::En);

      assert_eq!(month_long(13), "December");
    }
  }

  mod month_short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_short_month_name() {
      set_locale(Language::En);

      assert_eq!(month_short(1), "Jan");
      assert_eq!(month_short(9), "Sep");
      assert_eq!(month_short(12), "Dec");
    }

    #[test]
    fn it_clamps_an_out_of_range_month() {
      set_locale(Language::En);

      assert_eq!(month_short(0), "Jan");
      assert_eq!(month_short(13), "Dec");
    }
  }

  mod parse_isk {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_a_thousands_suffix() {
      assert_eq!(parse_isk("250k"), 250_000.0);
      assert_eq!(parse_isk("1.5K"), 1_500.0);
    }

    #[test]
    fn it_parses_a_millions_suffix() {
      assert_eq!(parse_isk("250m"), 250_000_000.0);
      assert_eq!(parse_isk("42M"), 42_000_000.0);
    }

    #[test]
    fn it_parses_a_billions_suffix() {
      assert_eq!(parse_isk("3.46B"), 3_460_000_000.0);
      assert_eq!(parse_isk("2.1b"), 2_100_000_000.0);
    }

    #[test]
    fn it_parses_a_trillions_suffix() {
      assert_eq!(parse_isk("1.2t"), 1_200_000_000_000.0);
      assert_eq!(parse_isk("3T"), 3_000_000_000_000.0);
    }

    #[test]
    fn it_strips_grouping_separators() {
      assert_eq!(parse_isk("1,200,000"), 1_200_000.0);
      assert_eq!(parse_isk("1 200 000"), 1_200_000.0);
      assert_eq!(parse_isk("1_200_000"), 1_200_000.0);
      assert_eq!(parse_isk("12\u{202f}500"), 12_500.0);
    }

    #[test]
    fn it_parses_a_bare_number() {
      assert_eq!(parse_isk("250"), 250.0);
      assert_eq!(parse_isk("  4200  "), 4_200.0);
    }

    #[test]
    fn it_rounds_to_a_whole_number() {
      assert_eq!(parse_isk("3.466b"), 3_466_000_000.0);
      assert_eq!(parse_isk("1.4"), 1.0);
    }

    #[test]
    fn it_returns_zero_for_empty_or_invalid_input() {
      assert_eq!(parse_isk(""), 0.0);
      assert_eq!(parse_isk("   "), 0.0);
      assert_eq!(parse_isk("-"), 0.0);
      assert_eq!(parse_isk("abc"), 0.0);
      assert_eq!(parse_isk("12.3.4"), 0.0);
    }
  }

  mod skill_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_skill_id() {
      assert_eq!(skill_label(None, 3300), "Skill 3300");
    }

    #[test]
    fn it_prefers_the_skill_name() {
      assert_eq!(skill_label(Some("Gunnery"), 3300), "Gunnery");
    }
  }

  mod weekday_long {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_long_weekday_name() {
      set_locale(Language::En);

      assert_eq!(weekday_long(Weekday::Mon), "Monday");
      assert_eq!(weekday_long(Weekday::Sun), "Sunday");
    }
  }

  mod weekday_short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_short_weekday_name() {
      set_locale(Language::En);

      assert_eq!(weekday_short(Weekday::Mon), "Mon");
      assert_eq!(weekday_short(Weekday::Sat), "Sat");
    }
  }
}
