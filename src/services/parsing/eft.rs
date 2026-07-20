pub fn item_names(raw: &str) -> Vec<String> {
  let mut lines = raw.lines().map(str::trim).skip_while(|line| line.is_empty());
  let Some((hull, _fit_name)) = lines.next().and_then(parse_header) else {
    return Vec::new();
  };

  let mut names = vec![hull];
  for line in lines {
    collect_line_names(line, &mut names);
  }
  names
}

pub fn parse_header(line: &str) -> Option<(String, String)> {
  let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
  let (hull, name) = inner.split_once(',')?;
  let hull = hull.trim();
  let name = name.trim();
  if hull.is_empty() || name.is_empty() {
    return None;
  }
  Some((hull.to_owned(), name.to_owned()))
}

fn collect_line_names(line: &str, names: &mut Vec<String>) {
  if line.is_empty() || line.starts_with('[') {
    return;
  }
  for token in line.split(',') {
    let name = strip_quantity(strip_offline(token.trim()));
    if !name.is_empty() {
      names.push(name.to_owned());
    }
  }
}

fn strip_offline(token: &str) -> &str {
  let Some(head) = token
    .strip_suffix("/offline")
    .or_else(|| token.strip_suffix("/OFFLINE"))
  else {
    return token;
  };
  head.trim_end()
}

fn strip_quantity(token: &str) -> &str {
  let Some((head, tail)) = token.rsplit_once(char::is_whitespace) else {
    return token;
  };
  let Some(digits) = tail.strip_prefix(['x', 'X']) else {
    return token;
  };
  if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
    head.trim_end()
  } else {
    token
  }
}

#[allow(dead_code)]
pub mod slots {
  use super::{parse_header, strip_offline};
  use crate::services::parsing::quantity;

  #[derive(Clone, Debug, Eq, PartialEq)]
  pub struct FitEntry {
    pub name: String,
    pub quantity: u64,
    pub empty: bool,
  }

  #[derive(Clone, Debug, Eq, PartialEq)]
  pub struct FitSection {
    pub entries: Vec<FitEntry>,
  }

  #[derive(Clone, Debug, Eq, PartialEq)]
  pub struct Fit {
    pub hull: String,
    pub name: String,
    pub sections: Vec<FitSection>,
  }

  pub fn parse_fit(raw: &str) -> Option<Fit> {
    let mut lines = raw.lines().map(str::trim).skip_while(|line| line.is_empty());
    let (hull, name) = lines.next().and_then(parse_header)?;

    let mut sections: Vec<FitSection> = Vec::new();
    let mut current: Vec<FitEntry> = Vec::new();
    for line in lines {
      if line.is_empty() {
        flush(&mut sections, &mut current);
        continue;
      }
      if let Some(entry) = parse_entry(line) {
        current.push(entry);
      }
    }
    flush(&mut sections, &mut current);

    Some(Fit {
      hull,
      name,
      sections,
    })
  }

  fn flush(sections: &mut Vec<FitSection>, current: &mut Vec<FitEntry>) {
    if current.is_empty() {
      return;
    }
    sections.push(FitSection {
      entries: std::mem::take(current),
    });
  }

  fn parse_entry(line: &str) -> Option<FitEntry> {
    if let Some(entry) = empty_entry(line) {
      return Some(entry);
    }
    if line.starts_with('[') {
      return None;
    }

    let token = strip_offline(module_token(line));
    let (name, quantity) = split_quantity(token);
    let name = name.trim();
    if name.is_empty() {
      return None;
    }

    Some(FitEntry {
      name: name.to_owned(),
      quantity,
      empty: false,
    })
  }

  fn empty_entry(line: &str) -> Option<FitEntry> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    if !inner.trim_start().to_lowercase().starts_with("empty") {
      return None;
    }
    Some(FitEntry {
      name: String::new(),
      quantity: 0,
      empty: true,
    })
  }

  fn module_token(line: &str) -> &str {
    line.split(',').next().unwrap_or(line).trim()
  }

  fn split_quantity(token: &str) -> (&str, u64) {
    let Some((head, tail)) = token.rsplit_once(char::is_whitespace) else {
      return (token, 1);
    };
    if !tail.starts_with(['x', 'X']) {
      return (token, 1);
    }
    match quantity::separated(tail).and_then(|digits| digits.parse::<u64>().ok()) {
      Some(quantity) => (head.trim_end(), quantity),
      None => (token, 1),
    }
  }

  #[cfg(test)]
  mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn names(section: &FitSection) -> Vec<&str> {
      section.entries.iter().map(|entry| entry.name.as_str()).collect()
    }

    #[test]
    fn it_returns_none_when_the_header_is_missing() {
      assert_eq!(parse_fit("200mm AutoCannon I\nDamage Control I"), None);
    }

    #[test]
    fn it_returns_none_when_the_header_has_no_fit_name() {
      assert_eq!(parse_fit("[Empty High slot]\nGyrostabilizer I"), None);
    }

    #[test]
    fn it_parses_the_hull_and_fit_name_and_tolerates_leading_blank_lines() {
      let fit = "\n\n[Rifter, Cheap Tackle]\nDamage Control I";

      let parsed = parse_fit(fit).expect("parses");

      assert_eq!(parsed.hull, "Rifter");
      assert_eq!(parsed.name, "Cheap Tackle");
    }

    #[test]
    fn it_splits_a_ship_fit_into_ordered_sections() {
      let fit = concat!(
        "[Rifter, Cheap Tackle]\n",
        "Damage Control I\n",
        "Small Armor Repairer I\n",
        "\n",
        "Stasis Webifier I\n",
        "\n",
        "200mm AutoCannon I, EMP S\n",
        "200mm AutoCannon I, EMP S\n",
        "\n",
        "Small Projectile Collision Accelerator I\n",
        "\n",
        "Hobgoblin I x5\n"
      );

      let parsed = parse_fit(fit).expect("parses");

      assert_eq!(parsed.sections.len(), 5);
      assert_eq!(
        names(&parsed.sections[0]),
        vec!["Damage Control I", "Small Armor Repairer I"]
      );
      assert_eq!(names(&parsed.sections[1]), vec!["Stasis Webifier I"]);
      assert_eq!(
        names(&parsed.sections[2]),
        vec!["200mm AutoCannon I", "200mm AutoCannon I"]
      );
      assert_eq!(
        names(&parsed.sections[3]),
        vec!["Small Projectile Collision Accelerator I"]
      );
      assert_eq!(names(&parsed.sections[4]), vec!["Hobgoblin I"]);
      assert_eq!(parsed.sections[4].entries[0].quantity, 5);
    }

    #[test]
    fn it_keeps_only_the_module_when_a_line_carries_a_charge() {
      let fit = "[Rifter, Fit]\n200mm AutoCannon I, Republic Fleet EMP S";

      let parsed = parse_fit(fit).expect("parses");

      let entry = &parsed.sections[0].entries[0];
      assert_eq!(entry.name, "200mm AutoCannon I");
      assert_eq!(entry.quantity, 1);
      assert!(!entry.empty);
    }

    #[test]
    fn it_parses_quantity_suffixes_and_defaults_to_one() {
      let fit = "[Rifter, Fit]\nHobgoblin I x5\nNanite Repair Paste x1000\nSpodumain 5000x Special\nEMP S";

      let parsed = parse_fit(fit).expect("parses");
      let entries = &parsed.sections[0].entries;

      assert_eq!((entries[0].name.as_str(), entries[0].quantity), ("Hobgoblin I", 5));
      assert_eq!(
        (entries[1].name.as_str(), entries[1].quantity),
        ("Nanite Repair Paste", 1000)
      );
      assert_eq!(
        (entries[2].name.as_str(), entries[2].quantity),
        ("Spodumain 5000x Special", 1)
      );
      assert_eq!((entries[3].name.as_str(), entries[3].quantity), ("EMP S", 1));
    }

    #[test]
    fn it_preserves_empty_slot_lines_as_explicit_empty_entries() {
      let fit = "[Rifter, Fit]\n[Empty High slot]\nGyrostabilizer I\n\n[Empty Rig slot]";

      let parsed = parse_fit(fit).expect("parses");

      let high = &parsed.sections[0].entries;
      assert!(high[0].empty);
      assert_eq!(high[0].name, "");
      assert!(!high[1].empty);
      assert_eq!(high[1].name, "Gyrostabilizer I");

      let rigs = &parsed.sections[1].entries;
      assert!(rigs[0].empty);
    }

    #[test]
    fn it_strips_offline_suffixes() {
      let fit = "[Rifter, Fit]\nCyno Field Generator I /offline";

      let parsed = parse_fit(fit).expect("parses");

      assert_eq!(parsed.sections[0].entries[0].name, "Cyno Field Generator I");
    }

    #[test]
    fn it_collapses_extra_blank_lines_between_sections() {
      let fit = "[Rifter, Fit]\nDamage Control I\n\n\n\nStasis Webifier I\n\n";

      let parsed = parse_fit(fit).expect("parses");

      assert_eq!(parsed.sections.len(), 2);
      assert_eq!(names(&parsed.sections[0]), vec!["Damage Control I"]);
      assert_eq!(names(&parsed.sections[1]), vec!["Stasis Webifier I"]);
    }

    #[test]
    fn it_folds_missing_blank_lines_into_a_single_section() {
      let fit = "[Rifter, Fit]\nDamage Control I\nStasis Webifier I\n200mm AutoCannon I";

      let parsed = parse_fit(fit).expect("parses");

      assert_eq!(parsed.sections.len(), 1);
      assert_eq!(
        names(&parsed.sections[0]),
        vec!["Damage Control I", "Stasis Webifier I", "200mm AutoCannon I"]
      );
    }

    #[test]
    fn it_parses_a_full_upwell_structure_fit() {
      let fit = concat!(
        "[Athanor, Reprocessing Post]\n",
        "Standup Cloning Center I\n",
        "Standup Reprocessing Facility I\n",
        "\n",
        "Standup Heavy Energy Neutralizer I\n",
        "Standup Multirole Missile Launcher I\n",
        "\n",
        "Standup Target Painter I\n",
        "Standup Variable Spectrum ECM I\n",
        "\n",
        "Standup M-Set Moon Drilling Stability I\n",
        "Standup M-Set Reprocessing I\n"
      );

      let parsed = parse_fit(fit).expect("parses");

      assert_eq!(parsed.hull, "Athanor");
      assert_eq!(parsed.name, "Reprocessing Post");
      assert_eq!(parsed.sections.len(), 4);
      assert_eq!(
        names(&parsed.sections[0]),
        vec!["Standup Cloning Center I", "Standup Reprocessing Facility I"]
      );
      assert_eq!(
        names(&parsed.sections[1]),
        vec![
          "Standup Heavy Energy Neutralizer I",
          "Standup Multirole Missile Launcher I"
        ]
      );
      assert_eq!(
        names(&parsed.sections[2]),
        vec!["Standup Target Painter I", "Standup Variable Spectrum ECM I"]
      );
      assert_eq!(
        names(&parsed.sections[3]),
        vec![
          "Standup M-Set Moon Drilling Stability I",
          "Standup M-Set Reprocessing I"
        ]
      );
    }

    #[test]
    fn it_degrades_gracefully_on_a_malformed_paste() {
      let fit = "[Rifter, Fit]\nDamage Control I\n[Some Junk Header]\n\n\nStasis Webifier I";

      let parsed = parse_fit(fit).expect("parses");

      assert_eq!(parsed.sections.len(), 2);
      assert_eq!(names(&parsed.sections[0]), vec!["Damage Control I"]);
      assert_eq!(names(&parsed.sections[1]), vec!["Stasis Webifier I"]);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod item_names {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_the_header_hull_and_blank_line_separated_sections() {
      let fit = "[Rifter, Cheap Tackle]\n200mm AutoCannon I\n\nStasis Webifier I\n\nDamage Control I\n";

      let names = item_names(fit);

      assert_eq!(
        names,
        vec!["Rifter", "200mm AutoCannon I", "Stasis Webifier I", "Damage Control I"]
      );
    }

    #[test]
    fn it_splits_a_module_and_charge_comma_line_into_both_names() {
      let fit = "[Rifter, Fit]\n200mm AutoCannon I, EMP S";

      let names = item_names(fit);

      assert_eq!(names, vec!["Rifter", "200mm AutoCannon I", "EMP S"]);
    }

    #[test]
    fn it_strips_quantity_suffixes_but_keeps_names_ending_in_x_tokens() {
      let fit = "[Rifter, Fit]\nHobgoblin I x5\nEMP S x1000\nSpodumain 5000x Special";

      let names = item_names(fit);

      assert_eq!(names, vec!["Rifter", "Hobgoblin I", "EMP S", "Spodumain 5000x Special"]);
    }

    #[test]
    fn it_ignores_empty_slot_lines() {
      let fit = "[Rifter, Fit]\n[Empty High slot]\n[Empty Rig slot]\nGyrostabilizer I";

      let names = item_names(fit);

      assert_eq!(names, vec!["Rifter", "Gyrostabilizer I"]);
    }

    #[test]
    fn it_strips_offline_suffixes() {
      let fit = "[Rifter, Fit]\nDamage Control I /offline";

      let names = item_names(fit);

      assert_eq!(names, vec!["Rifter", "Damage Control I"]);
    }

    #[test]
    fn it_tolerates_leading_blank_lines_before_the_header() {
      let fit = "\n\n[Rifter, Fit]\nGyrostabilizer I";

      let names = item_names(fit);

      assert_eq!(names, vec!["Rifter", "Gyrostabilizer I"]);
    }

    #[test]
    fn it_flattens_a_facility_style_body() {
      let fit = "[Athanor, Reprocessing Post]\nStandup M-Set Moon Drilling Stability I\nStandup Cloning Center I";

      let names = item_names(fit);

      assert_eq!(
        names,
        vec![
          "Athanor",
          "Standup M-Set Moon Drilling Stability I",
          "Standup Cloning Center I"
        ]
      );
    }

    #[test]
    fn it_returns_empty_when_the_header_is_missing() {
      assert_eq!(item_names("Gunnery 5\nSmall Hybrid Turret 3"), Vec::<String>::new());
    }

    #[test]
    fn it_returns_empty_when_the_header_has_no_fit_name_separator() {
      assert_eq!(item_names("[Empty High slot]\nGyrostabilizer I"), Vec::<String>::new());
    }
  }

  mod parse_header {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_a_hull_and_fit_name() {
      assert_eq!(
        parse_header("[Rifter, Cheap Tackle]"),
        Some(("Rifter".to_owned(), "Cheap Tackle".to_owned()))
      );
    }

    #[test]
    fn it_parses_a_facility_header() {
      assert_eq!(
        parse_header("[Athanor, Reprocessing Post]"),
        Some(("Athanor".to_owned(), "Reprocessing Post".to_owned()))
      );
    }

    #[test]
    fn it_trims_whitespace_around_the_brackets_and_fields() {
      assert_eq!(
        parse_header("  [ Rifter ,  Cheap Tackle ] "),
        Some(("Rifter".to_owned(), "Cheap Tackle".to_owned()))
      );
    }

    #[test]
    fn it_rejects_a_line_without_brackets() {
      assert_eq!(parse_header("Rifter, Cheap Tackle"), None);
    }

    #[test]
    fn it_rejects_a_header_without_a_comma() {
      assert_eq!(parse_header("[Empty High slot]"), None);
    }

    #[test]
    fn it_rejects_an_empty_hull_or_fit_name() {
      assert_eq!(parse_header("[, Cheap Tackle]"), None);
      assert_eq!(parse_header("[Rifter, ]"), None);
    }
  }
}
