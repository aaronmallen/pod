use chrono::{DateTime, TimeZone, Utc};

pub(super) fn fmt_eve(dt: DateTime<Utc>) -> String {
  dt.format("%H:%M").to_string()
}

pub(super) fn fmt_local<Tz>(dt: DateTime<Utc>, tz: &Tz) -> String
where
  Tz: TimeZone,
  Tz::Offset: std::fmt::Display,
{
  let local = dt.with_timezone(tz);
  let clock = local.format("%H:%M").to_string();
  let abbrev = local.format("%Z").to_string();
  let numeric = local.format("%:z").to_string();

  format!("{clock} {}", zone_label(&abbrev, &numeric))
}

/// `%Z` does not always yield a name: on some platforms (e.g. Windows) and for a `FixedOffset` it
/// returns a numeric offset like `-05:00`, so fall back to the numeric offset when the abbreviation
/// is not alphabetic.
fn zone_label(abbrev: &str, numeric: &str) -> String {
  let trimmed = abbrev.trim();
  let usable = !trimmed.is_empty()
    && !trimmed.starts_with('+')
    && !trimmed.starts_with('-')
    && trimmed.chars().any(|ch| ch.is_ascii_alphabetic());

  if usable { trimmed.to_owned() } else { numeric.to_owned() }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn at(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp).unwrap().with_timezone(&Utc)
  }

  mod fmt_eve {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pads_the_utc_clock() {
      assert_eq!(fmt_eve(at("2026-06-12T09:05:00Z")), "09:05");
    }
  }

  mod fmt_local {
    use chrono::FixedOffset;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_converts_to_the_injected_offset() {
      let west = FixedOffset::west_opt(5 * 3_600).unwrap();

      assert_eq!(fmt_local(at("2026-06-12T14:00:00Z"), &west), "09:00 -05:00");
    }

    #[test]
    fn it_differs_from_the_eve_clock() {
      let east = FixedOffset::east_opt(3 * 3_600).unwrap();
      let start = at("2026-06-12T14:00:00Z");

      assert_ne!(fmt_eve(start), fmt_local(start, &east));
    }
  }

  mod zone_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_numeric_offset_for_a_numeric_abbreviation() {
      assert_eq!(zone_label("-05:00", "-05:00"), "-05:00");
    }

    #[test]
    fn it_falls_back_to_the_numeric_offset_when_empty() {
      assert_eq!(zone_label("", "+02:00"), "+02:00");
    }

    #[test]
    fn it_keeps_an_alphabetic_abbreviation() {
      assert_eq!(zone_label("EDT", "-04:00"), "EDT");
    }
  }
}
