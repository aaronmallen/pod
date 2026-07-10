#![allow(dead_code)]

use std::collections::HashSet;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedFit {
  pub eft: String,
  pub hull: Option<String>,
  pub overflow: usize,
  pub rigs: Vec<i64>,
  pub unknown: Vec<String>,
}

pub fn parse_fit<I, S>(text: &str, structure_name: &str, facility_name: &str, catalog: I) -> ParsedFit
where
  I: IntoIterator<Item = (S, i64)>,
  S: AsRef<str>,
{
  let entries = build_entries(catalog);
  let is_eft = text.lines().any(|line| parse_header(line).is_some());

  let mut hull = None;
  let mut rigs: Vec<i64> = Vec::new();
  let mut seen: HashSet<i64> = HashSet::new();
  let mut overflow = 0;
  let mut unknown: Vec<String> = Vec::new();
  let mut body: Vec<String> = Vec::new();

  for raw in text.lines() {
    if let Some((parsed_hull, _)) = parse_header(raw) {
      if hull.is_none() {
        hull = Some(parsed_hull);
      }
      continue;
    }

    let name = clean_fit_line(raw);
    if name.is_empty() {
      if is_eft {
        body.push(String::new());
      }
      continue;
    }

    if is_section_header(&name) {
      continue;
    }

    let key = norm(&name);
    match match_rig(&entries, &key) {
      Some(id) => {
        if seen.insert(id) {
          if rigs.len() < 3 {
            rigs.push(id);
          } else {
            overflow += 1;
          }
        }
      }
      None => {
        if looks_like_rig(&key) {
          unknown.push(name.clone());
        }
      }
    }

    if is_eft {
      body.push(raw.to_string());
    } else {
      body.push(name);
    }
  }

  while body.last().is_some_and(|line| line.is_empty()) {
    body.pop();
  }

  let mut eft = format!("[{structure_name}, {facility_name}]");
  for line in &body {
    eft.push('\n');
    eft.push_str(line);
  }

  ParsedFit {
    eft,
    hull,
    overflow,
    rigs,
    unknown,
  }
}

pub fn splice_rigs(
  existing_eft: Option<&str>,
  rig_names: &[String],
  structure_name: &str,
  facility_name: &str,
) -> String {
  let header = format!("[{structure_name}, {facility_name}]");

  let Some(existing) = existing_eft else {
    let mut eft = header;
    for name in rig_names {
      eft.push('\n');
      eft.push_str(name);
    }
    return eft;
  };

  let mut lines: Vec<String> = Vec::new();
  let mut inserted = false;

  for raw in existing.lines() {
    if parse_header(raw).is_some() {
      lines.push(raw.to_string());
      continue;
    }

    if looks_like_rig(&norm(&clean_fit_line(raw))) {
      if !inserted {
        for name in rig_names {
          lines.push(name.clone());
        }
        inserted = true;
      }
      continue;
    }

    lines.push(raw.to_string());
  }

  if !inserted && !rig_names.is_empty() {
    if lines.last().is_some_and(|line| !line.is_empty()) {
      lines.push(String::new());
    }
    for name in rig_names {
      lines.push(name.clone());
    }
  }

  lines.join("\n")
}

fn build_entries<I, S>(catalog: I) -> Vec<(String, i64)>
where
  I: IntoIterator<Item = (S, i64)>,
  S: AsRef<str>,
{
  catalog
    .into_iter()
    .map(|(name, id)| (norm(name.as_ref()), id))
    .collect()
}

fn clean_fit_line(line: &str) -> String {
  let mut value = line.replace('\r', "");
  value = value.trim().to_string();
  if value.is_empty() {
    return String::new();
  }

  if let Some(first) = value.split('\t').next() {
    value = first.trim().to_string();
  }

  value = strip_leading_quantity(&value).to_string();
  value = strip_trailing_quantity(&value).to_string();
  value.trim().to_string()
}

fn is_section_header(line: &str) -> bool {
  let trimmed = line.trim().to_lowercase();
  trimmed.ends_with("slots") || trimmed.ends_with("slot")
}

fn looks_like_rig(key: &str) -> bool {
  key.contains("-set")
}

fn match_rig(entries: &[(String, i64)], key: &str) -> Option<i64> {
  if let Some((_, id)) = entries.iter().find(|(name, _)| name == key) {
    return Some(*id);
  }

  for (name, id) in entries {
    if key.contains(name.as_str()) || name.contains(key) {
      return Some(*id);
    }
  }

  None
}

fn norm(value: &str) -> String {
  let lowered = value.to_lowercase();
  let straightened: String = lowered
    .chars()
    .map(|ch| if ch == '\u{2019}' || ch == '\'' { '\'' } else { ch })
    .collect();
  straightened.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_header(line: &str) -> Option<(String, String)> {
  let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
  let (hull, name) = inner.split_once(',')?;
  let hull = hull.trim();
  let name = name.trim();
  if hull.is_empty() || name.is_empty() {
    return None;
  }
  Some((hull.to_string(), name.to_string()))
}

fn strip_leading_quantity(value: &str) -> &str {
  let bytes = value.as_bytes();
  if bytes.is_empty() || !bytes[0].is_ascii_digit() {
    return value;
  }

  let mut number_end = 0;
  while number_end < bytes.len()
    && (bytes[number_end].is_ascii_digit() || bytes[number_end] == b',' || bytes[number_end] == b'.')
  {
    number_end += 1;
  }

  let mut cursor = number_end;
  while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
    cursor += 1;
  }
  let leading_ws = cursor - number_end;

  let mut had_x = false;
  if cursor < bytes.len()
    && (bytes[cursor] | 0x20) == b'x'
    && cursor + 1 < bytes.len()
    && bytes[cursor + 1].is_ascii_whitespace()
  {
    had_x = true;
    cursor += 1;
  }

  let ws_start = cursor;
  while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
    cursor += 1;
  }
  let trailing_ws = cursor - ws_start;

  if (had_x && trailing_ws >= 1) || (!had_x && leading_ws >= 1) {
    &value[cursor..]
  } else {
    value
  }
}

fn strip_trailing_quantity(value: &str) -> &str {
  let bytes = value.as_bytes();
  let mut digits_start = bytes.len();
  while digits_start > 0
    && (bytes[digits_start - 1].is_ascii_digit() || bytes[digits_start - 1] == b',' || bytes[digits_start - 1] == b'.')
  {
    digits_start -= 1;
  }

  if digits_start == bytes.len() || !bytes[digits_start].is_ascii_digit() {
    return value;
  }

  let mut cursor = digits_start;
  if cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
    cursor -= 1;
  }

  if cursor == 0 || (bytes[cursor - 1] | 0x20) != b'x' {
    return value;
  }
  cursor -= 1;

  if cursor == 0 || !bytes[cursor - 1].is_ascii_whitespace() {
    return value;
  }
  while cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
    cursor -= 1;
  }

  value[..cursor].trim_end()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn catalog() -> Vec<(&'static str, i64)> {
    vec![
      ("Standup M-Set Moon Drilling Stability I", 1001),
      ("Standup M-Set Moon Ore Grading Processor I", 1002),
      ("Standup M-Set Reprocessing I", 1003),
      ("Standup L-Set Reaction Efficiency I", 1004),
    ]
  }

  const SCAN_SAMPLE: &str = "High Power Slots\nStandup Heavy Energy Neutralizer I\nStandup Heavy Energy Neutralizer I\nStandup Multirole Missile Launcher I\nMedium Power Slots\nStandup Target Painter I\nStandup Variable Spectrum ECM I\nStandup Variable Spectrum ECM I\nRig Slots\nStandup M-Set Moon Drilling Stability I\nStandup M-Set Moon Ore Grading Processor I\nService Slots\nStandup Cloning Center I\nStandup Moon Drill I\nStandup Reprocessing Facility I";

  #[test]
  fn parses_ship_scan_sample() {
    let parsed = parse_fit(SCAN_SAMPLE, "Athanor", "Reprocessing Post", catalog());

    assert_eq!(parsed.rigs, vec![1001, 1002]);
    assert_eq!(parsed.overflow, 0);
    assert!(parsed.unknown.is_empty());
    assert_eq!(parsed.hull, None);

    assert!(parsed.eft.starts_with("[Athanor, Reprocessing Post]\n"));
    assert!(!parsed.eft.contains("Slots"));
    assert!(parsed.eft.contains("Standup Heavy Energy Neutralizer I"));
    assert!(parsed.eft.contains("Standup Multirole Missile Launcher I"));
    assert!(parsed.eft.contains("Standup Target Painter I"));
    assert!(parsed.eft.contains("Standup Variable Spectrum ECM I"));
    assert!(parsed.eft.contains("Standup M-Set Moon Drilling Stability I"));
    assert!(parsed.eft.contains("Standup Cloning Center I"));
    assert!(parsed.eft.contains("Standup Reprocessing Facility I"));
  }

  #[test]
  fn parses_eft_and_reanchors_header() {
    let paste = "[Raitaru, Old Name]\n\nStandup Multirole Missile Launcher I\n\nStandup M-Set Moon Drilling Stability I\nStandup M-Set Moon Ore Grading Processor I\n\nStandup Cloning Center I\n";
    let parsed = parse_fit(paste, "Sotiyo", "New Facility", catalog());

    assert_eq!(parsed.hull, Some("Raitaru".to_string()));
    assert_eq!(parsed.rigs, vec![1001, 1002]);
    assert!(parsed.eft.starts_with("[Sotiyo, New Facility]"));
    assert!(!parsed.eft.contains("Raitaru"));
    assert!(!parsed.eft.contains("Old Name"));
    assert!(parsed.eft.contains("Standup Multirole Missile Launcher I"));
    assert!(parsed.eft.contains("Standup Cloning Center I"));
    assert!(parsed.eft.contains("\n\n"));
  }

  #[test]
  fn parses_cargo_scan_tab_separated() {
    let paste = "Standup M-Set Moon Drilling Stability I\t1\tRig Slot\nStandup M-Set Moon Ore Grading Processor I\t1\tRig Slot\nStandup Cloning Center I\t1\tService Slot";
    let parsed = parse_fit(paste, "Athanor", "Moon Post", catalog());

    assert_eq!(parsed.rigs, vec![1001, 1002]);
    assert_eq!(parsed.hull, None);
    assert!(parsed.eft.contains("Standup M-Set Moon Drilling Stability I"));
    assert!(!parsed.eft.contains('\t'));
    assert!(!parsed.eft.contains("Rig Slot"));
  }

  #[test]
  fn caps_rigs_at_three_with_overflow_and_dedup() {
    let paste = "Standup M-Set Moon Drilling Stability I\nStandup M-Set Moon Ore Grading Processor I\nStandup M-Set Moon Drilling Stability I\nStandup M-Set Reprocessing I\nStandup L-Set Reaction Efficiency I";
    let parsed = parse_fit(paste, "Tatara", "Reactor", catalog());

    assert_eq!(parsed.rigs, vec![1001, 1002, 1003]);
    assert_eq!(parsed.overflow, 1);
  }

  #[test]
  fn surfaces_unknown_structure_rigs() {
    let paste = "Standup M-Set Nonexistent Widget I\nStandup Moon Drill I\nStandup M-Set Moon Drilling Stability I";
    let parsed = parse_fit(paste, "Athanor", "Post", catalog());

    assert_eq!(parsed.rigs, vec![1001]);
    assert_eq!(parsed.unknown, vec!["Standup M-Set Nonexistent Widget I".to_string()]);
  }

  #[test]
  fn strips_leading_and_trailing_quantities() {
    let paste = "3x Standup M-Set Moon Drilling Stability I\nStandup M-Set Moon Ore Grading Processor I x2";
    let parsed = parse_fit(paste, "Athanor", "Post", catalog());

    assert_eq!(parsed.rigs, vec![1001, 1002]);
  }

  #[test]
  fn splice_rewrites_only_rig_lines() {
    let existing = "[Sotiyo, My Refinery]\nStandup Heavy Energy Neutralizer I\n\nStandup M-Set Moon Drilling Stability I\n\nStandup Cloning Center I";
    let spliced = splice_rigs(
      Some(existing),
      &["Standup L-Set Reaction Efficiency I".to_string()],
      "Sotiyo",
      "My Refinery",
    );

    assert!(spliced.contains("Standup L-Set Reaction Efficiency I"));
    assert!(!spliced.contains("Standup M-Set Moon Drilling Stability I"));
    assert!(spliced.contains("Standup Heavy Energy Neutralizer I"));
    assert!(spliced.contains("Standup Cloning Center I"));
    assert!(spliced.starts_with("[Sotiyo, My Refinery]"));
  }

  #[test]
  fn splice_synthesizes_when_no_existing_eft() {
    let spliced = splice_rigs(
      None,
      &[
        "Standup M-Set Moon Drilling Stability I".to_string(),
        "Standup M-Set Moon Ore Grading Processor I".to_string(),
      ],
      "Athanor",
      "Fresh Post",
    );

    assert!(spliced.starts_with("[Athanor, Fresh Post]"));
    assert!(spliced.contains("Standup M-Set Moon Drilling Stability I"));
    assert!(spliced.contains("Standup M-Set Moon Ore Grading Processor I"));
  }
}
