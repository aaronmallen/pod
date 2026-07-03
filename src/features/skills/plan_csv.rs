pub const CSV_EXTENSION: &str = "csv";

const HEADER: [&str; 7] = [
  "Skill",
  "Group",
  "Primary Attribute",
  "Secondary Attribute",
  "Level",
  "SP",
  "Duration",
];
const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanCsvRow {
  pub skill: String,
  pub group: String,
  pub primary: String,
  pub secondary: String,
  pub level: u8,
  pub sp: f64,
  pub duration_secs: i64,
}

pub fn to_csv(rows: &[PlanCsvRow]) -> String {
  let mut out = String::new();
  out.push_str(
    &HEADER
      .iter()
      .map(|field| escape_field(field))
      .collect::<Vec<_>>()
      .join(","),
  );
  for row in rows {
    out.push('\n');
    let fields = [
      row.skill.clone(),
      row.group.clone(),
      row.primary.clone(),
      row.secondary.clone(),
      row.level.to_string(),
      round_sp(row.sp).to_string(),
      fmt_time_short_hrs(row.duration_secs),
    ];
    out.push_str(
      &fields
        .iter()
        .map(|field| escape_field(field))
        .collect::<Vec<_>>()
        .join(","),
    );
  }
  out.push('\n');
  out
}

/// Matches the "Skill" and "Level" columns by header name regardless of position, so a hand-edited or
/// reordered export still parses. Returns `None` only when the header row is missing one of those columns;
/// individual rows with a blank skill or an out-of-range level are silently skipped rather than failing the parse.
pub fn parse(raw: &str) -> Option<Vec<(String, u8)>> {
  let records = parse_records(raw);
  let header = records.first()?;
  let skill_idx = header
    .iter()
    .position(|field| field.trim().eq_ignore_ascii_case("Skill"))?;
  let level_idx = header
    .iter()
    .position(|field| field.trim().eq_ignore_ascii_case("Level"))?;

  let mut wishes = Vec::new();
  for record in records.iter().skip(1) {
    let (Some(skill), Some(level)) = (record.get(skill_idx), record.get(level_idx)) else {
      continue;
    };
    let skill = skill.trim();
    let Some(level) = parse_level(level) else {
      continue;
    };
    if skill.is_empty() {
      continue;
    }
    wishes.push((skill.to_owned(), level));
  }
  Some(wishes)
}

fn escape_field(value: &str) -> String {
  if value.contains(['"', ',', '\n']) {
    format!("\"{}\"", value.replace('"', "\"\""))
  } else {
    value.to_owned()
  }
}

fn round_sp(sp: f64) -> i64 {
  sp.round() as i64
}

/// Formats to the single coarsest unit that fits, truncating (not rounding) anything finer: `1d 1h` drops
/// minutes, `3h` drops minutes too. Only when a duration is under an hour do minutes show at all.
fn fmt_time_short_hrs(seconds: i64) -> String {
  if seconds <= 0 {
    return "0m".to_owned();
  }
  let days = seconds / SECONDS_PER_DAY;
  let hours = (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
  if days > 0 {
    return format!("{days}d {hours}h");
  }
  if hours > 0 {
    return format!("{hours}h");
  }
  let minutes = (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
  format!("{minutes}m")
}

fn parse_level(token: &str) -> Option<u8> {
  let token = token.trim();
  token.parse::<u8>().ok().filter(|n| (1..=5).contains(n))
}

fn parse_records(raw: &str) -> Vec<Vec<String>> {
  let mut records = Vec::new();
  let mut record = Vec::new();
  let mut field = String::new();
  let mut in_quotes = false;
  let mut chars = raw.chars().peekable();

  while let Some(c) = chars.next() {
    if in_quotes {
      match c {
        '"' if chars.peek() == Some(&'"') => {
          chars.next();
          field.push('"');
        }
        '"' => in_quotes = false,
        _ => field.push(c),
      }
      continue;
    }
    match c {
      '"' => in_quotes = true,
      ',' => {
        record.push(std::mem::take(&mut field));
      }
      '\r' => {
        if chars.peek() == Some(&'\n') {
          chars.next();
        }
        record.push(std::mem::take(&mut field));
        records.push(std::mem::take(&mut record));
      }
      '\n' => {
        record.push(std::mem::take(&mut field));
        records.push(std::mem::take(&mut record));
      }
      _ => field.push(c),
    }
  }

  if !field.is_empty() || !record.is_empty() {
    record.push(field);
    records.push(record);
  }

  records
}

#[cfg(test)]
mod tests {
  use super::*;

  fn row(skill: &str, group: &str, level: u8, sp: f64, duration_secs: i64) -> PlanCsvRow {
    PlanCsvRow {
      skill: skill.to_owned(),
      group: group.to_owned(),
      primary: "Perception".to_owned(),
      secondary: "Willpower".to_owned(),
      level,
      sp,
      duration_secs,
    }
  }

  mod to_csv {
    use super::*;

    #[test]
    fn it_writes_the_header_when_there_are_no_rows() {
      assert_eq!(
        to_csv(&[]),
        "Skill,Group,Primary Attribute,Secondary Attribute,Level,SP,Duration\n"
      );
    }

    #[test]
    fn it_writes_attributes_and_rounds_sp() {
      let csv = to_csv(&[
        row("Gunnery", "Gunnery", 5, 256_000.4, 3_600),
        row("Drones", "Drones", 3, 8_000.6, 90_000),
      ]);
      assert_eq!(
        csv,
        "Skill,Group,Primary Attribute,Secondary Attribute,Level,SP,Duration\n\
        Gunnery,Gunnery,Perception,Willpower,5,256000,1h\n\
        Drones,Drones,Perception,Willpower,3,8001,1d 1h\n"
      );
    }

    #[test]
    fn it_quotes_fields_with_commas() {
      let csv = to_csv(&[row("Advanced, Weapon", "Gunnery", 4, 100.0, 0)]);
      assert!(csv.contains("\"Advanced, Weapon\""));
    }

    #[test]
    fn it_doubles_inner_quotes() {
      let csv = to_csv(&[row("The \"Best\" Skill", "Gunnery", 4, 100.0, 0)]);
      assert!(csv.contains("\"The \"\"Best\"\" Skill\""));
    }

    #[test]
    fn it_quotes_fields_with_newlines() {
      let csv = to_csv(&[row("Line\nBreak", "Gunnery", 4, 100.0, 0)]);
      assert!(csv.contains("\"Line\nBreak\""));
    }

    #[test]
    fn it_formats_minutes_when_under_an_hour() {
      let csv = to_csv(&[row("Gunnery", "Gunnery", 1, 100.0, 1_800)]);
      assert!(csv.contains(",30m\n"));
    }
  }

  mod parse {
    use super::*;

    #[test]
    fn it_reads_skill_and_level_columns() {
      let parsed = parse("#,Skill,Group,Level,SP,Duration\n1,Gunnery,Gunnery,5,256000,1h\n").unwrap();
      assert_eq!(parsed, vec![("Gunnery".to_owned(), 5)]);
    }

    #[test]
    fn it_returns_none_without_a_header() {
      assert_eq!(parse("Gunnery V\nDrones 3\n"), None);
    }

    #[test]
    fn it_rejects_eft_first_line() {
      assert_eq!(parse("[Rifter, My Fit]\nSmall Hybrid Turret\n"), None);
    }

    #[test]
    fn it_reads_quoted_fields_with_commas_and_doubled_quotes() {
      let parsed = parse("#,Skill,Group,Level,SP,Duration\n1,\"Advanced, \"\"Best\"\"\",Gunnery,4,100,1h\n").unwrap();
      assert_eq!(parsed, vec![("Advanced, \"Best\"".to_owned(), 4)]);
    }

    #[test]
    fn it_skips_rows_with_invalid_levels() {
      let parsed =
        parse("#,Skill,Group,Level,SP,Duration\n1,Gunnery,Gunnery,9,100,1h\n2,Drones,Drones,3,100,1h\n").unwrap();
      assert_eq!(parsed, vec![("Drones".to_owned(), 3)]);
    }

    #[test]
    fn it_tolerates_reordered_columns() {
      let parsed = parse("Level,Skill\n5,Gunnery\n").unwrap();
      assert_eq!(parsed, vec![("Gunnery".to_owned(), 5)]);
    }

    #[test]
    fn it_round_trips_a_serialized_plan() {
      let csv = to_csv(&[
        row("Gunnery", "Gunnery", 5, 256_000.0, 3_600),
        row("Drones", "Drones", 3, 8_000.0, 90_000),
      ]);
      let parsed = parse(&csv).unwrap();
      assert_eq!(parsed, vec![("Gunnery".to_owned(), 5), ("Drones".to_owned(), 3)]);
    }
  }
}
