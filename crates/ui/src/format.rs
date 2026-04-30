//! Formatting utilities for ISK, volume, duration, and counts.

/// Compact ISK abbreviation: "1.23B", "45.6M", "789k", "42"
pub fn fmt_isk(value: f64) -> String {
  let abs = value.abs();
  if abs >= 1_000_000_000.0 {
    format!("{:.2}B", value / 1_000_000_000.0)
  } else if abs >= 1_000_000.0 {
    format!("{:.2}M", value / 1_000_000.0)
  } else if abs >= 1_000.0 {
    format!("{:.1}k", value / 1_000.0)
  } else {
    format!("{:.0}", value)
  }
}

/// Full ISK with thin-space (`\u{2009}`) thousands separator.
/// Example: "1\u{2009}234\u{2009}567\u{2009}890"
pub fn fmt_isk_full(value: f64) -> String {
  let rounded = value.round() as i64;
  let s = rounded.unsigned_abs().to_string();
  let grouped = group_digits(&s, '\u{2009}');
  if rounded < 0 { format!("-{grouped}") } else { grouped }
}

/// Volume with comma thousands separator and two decimal places.
/// Example: "1,234.56 m³"
pub fn fmt_vol(m3: f64) -> String {
  let whole = m3.floor() as i64;
  let frac = ((m3 - m3.floor()) * 100.0).round() as u64;
  let grouped = group_digits(&whole.unsigned_abs().to_string(), ',');
  let sign = if whole < 0 { "-" } else { "" };
  format!("{sign}{grouped}.{frac:02} m³")
}

/// Count with comma thousands separator. Example: "1,234"
pub fn fmt_count(n: u64) -> String {
  group_digits(&n.to_string(), ',')
}

/// Percentage with one decimal place. Example: "12.3%"
pub fn fmt_pct(ratio: f64) -> String {
  format!("{:.1}%", ratio * 100.0)
}

/// Duration broken into days/hours/minutes/seconds.
/// Examples: "3d 4h 12m", "4h 12m", "12m 30s", "30s"
pub fn fmt_dur(secs: u64) -> String {
  let d = secs / 86_400;
  let h = (secs % 86_400) / 3_600;
  let m = (secs % 3_600) / 60;
  let s = secs % 60;
  if d > 0 {
    format!("{d}d {h}h {m}m")
  } else if h > 0 {
    format!("{h}h {m}m")
  } else if m > 0 {
    format!("{m}m {s}s")
  } else {
    format!("{s}s")
  }
}

/// UTC ETA formatted as "D Mon · HH:MM EVE" from seconds from now.
/// Returns "—" when `seconds_from_now` is 0.
pub fn fmt_eta(seconds_from_now: u64) -> String {
  if seconds_from_now == 0 {
    return "\u{2014}".to_string();
  }
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let ts = now + seconds_from_now;
  let hh = (ts % 86400) / 3600;
  let mm = (ts % 3600) / 60;
  let (_, month, day) = days_to_utc_date(ts / 86400);
  const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  ];
  format!("{} {} \u{00b7} {:02}:{:02}", day, MONTHS[month as usize - 1], hh, mm)
}

/// Gregorian civil-date from days since 1970-01-01 (Howard Hinnant algorithm).
fn days_to_utc_date(days: u64) -> (u32, u8, u8) {
  let z = days as i64 + 719468;
  let era = z / 146097;
  let doe = (z - era * 146097) as u64;
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
  let y = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = doy - (153 * mp + 2) / 5 + 1;
  let m = if mp < 10 { mp + 3 } else { mp - 9 };
  let y = if m <= 2 { y + 1 } else { y };
  (y as u32, m as u8, d as u8)
}

/// Shortest single-unit duration: "3d", "4h", "12m", "30s"
pub fn fmt_dur_short(secs: u64) -> String {
  let d = secs / 86_400;
  let h = (secs % 86_400) / 3_600;
  let m = (secs % 3_600) / 60;
  let s = secs % 60;
  if d > 0 {
    format!("{d}d")
  } else if h > 0 {
    format!("{h}h")
  } else if m > 0 {
    format!("{m}m")
  } else {
    format!("{s}s")
  }
}

/// SP cost to train a skill of `rank` to `level` (1–5).
/// Formula: round(250 × rank × 32^((level-1)/2))
pub fn sp_cost(rank: f64, level: u8) -> u64 {
  let exponent = (level.saturating_sub(1) as f64) / 2.0;
  (250.0 * rank * 32_f64.powf(exponent)).ceil() as u64
}

/// SP per second from primary and secondary attribute values.
/// Formula: (primary + secondary / 2) / 60
pub fn sp_per_sec(primary: u32, secondary: u32) -> f32 {
  (primary as f32 + secondary as f32 / 2.0) / 60.0
}

fn group_digits(s: &str, sep: char) -> String {
  let mut out = String::with_capacity(s.len() + s.len() / 3);
  let len = s.len();
  for (i, ch) in s.chars().enumerate() {
    if i > 0 && (len - i).is_multiple_of(3) {
      out.push(sep);
    }
    out.push(ch);
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_fmt_isk() {
    assert_eq!(fmt_isk(1_500_000_000.0), "1.50B");
    assert_eq!(fmt_isk(45_600_000.0), "45.60M");
    assert_eq!(fmt_isk(789_000.0), "789.0k");
    assert_eq!(fmt_isk(42.0), "42");
  }

  #[test]
  fn test_fmt_isk_full() {
    assert_eq!(fmt_isk_full(1_234_567.0), "1\u{2009}234\u{2009}567");
    assert_eq!(fmt_isk_full(1000.0), "1\u{2009}000");
    assert_eq!(fmt_isk_full(999.0), "999");
  }

  #[test]
  fn test_fmt_dur() {
    assert_eq!(fmt_dur(0), "0s");
    assert_eq!(fmt_dur(30), "30s");
    assert_eq!(fmt_dur(90), "1m 30s");
    assert_eq!(fmt_dur(3_661), "1h 1m");
    assert_eq!(fmt_dur(90_000), "1d 1h 0m");
  }

  #[test]
  fn test_sp_cost() {
    assert_eq!(sp_cost(1.0, 1), 250);
    assert_eq!(sp_cost(1.0, 2), 1_415);
    assert_eq!(sp_cost(1.0, 3), 8_000);
  }

  #[test]
  fn test_sp_per_sec() {
    let rate = sp_per_sec(27, 24);
    assert!((rate - 0.65).abs() < 0.01);
  }
}
