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
