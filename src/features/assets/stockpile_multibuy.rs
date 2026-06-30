pub fn parse(text: &str) -> Vec<(String, u64)> {
  let mut totals: Vec<(String, u64)> = Vec::new();

  for line in text.lines() {
    let Some((name, quantity)) = parse_line(line) else {
      continue;
    };
    match totals.iter_mut().find(|(existing, _)| existing == &name) {
      Some((_, running)) => *running = running.saturating_add(quantity),
      None => totals.push((name, quantity)),
    }
  }

  totals
}

pub fn serialize(items: &[(String, u64)]) -> String {
  items
    .iter()
    .map(|(name, quantity)| format!("{name}\t{quantity}"))
    .collect::<Vec<_>>()
    .join("\n")
}

fn parse_line(line: &str) -> Option<(String, u64)> {
  let line = line.trim();
  if line.is_empty() {
    return None;
  }

  let tokens: Vec<&str> = line.split_whitespace().collect();
  let bare = || Some((line.to_owned(), 1));

  let (mut name_end, mut trailing) = scan_trailing(&tokens);
  if name_end == tokens.len()
    && name_end > 0
    && let Some(group) = parse_separated_quantity(tokens[name_end - 1])
  {
    trailing = group;
    name_end -= 1;
  }

  let (mut name_start, mut leading) = scan_leading(&tokens, name_end);
  if name_start == 0
    && name_end > 0
    && let Some(group) = parse_separated_quantity(tokens[0])
  {
    leading = group;
    name_start = 1;
  }

  if name_start < name_end
    && let Some(quantity) = leading.parse::<u64>().ok().filter(|&quantity| quantity > 0)
  {
    return Some((tokens[name_start..name_end].join(" "), quantity));
  }

  if name_end == 0 || trailing.is_empty() {
    return bare();
  }

  let Some(quantity) = trailing.parse::<u64>().ok().filter(|&quantity| quantity > 0) else {
    return bare();
  };

  let name = tokens[..name_end].join(" ");
  Some((name, quantity))
}

fn scan_trailing(tokens: &[&str]) -> (usize, String) {
  let mut name_end = tokens.len();
  let mut trailing = String::new();
  while name_end > 0 {
    let Some(group) = parse_digit_group(tokens[name_end - 1]) else {
      break;
    };
    trailing.insert_str(0, &group);
    name_end -= 1;
  }
  (name_end, trailing)
}

fn scan_leading(tokens: &[&str], name_end: usize) -> (usize, String) {
  let mut name_start = 0;
  let mut leading = String::new();
  while name_start < name_end {
    let Some(group) = parse_digit_group(tokens[name_start]) else {
      break;
    };
    leading.push_str(&group);
    name_start += 1;
  }
  (name_start, leading)
}

fn parse_digit_group(token: &str) -> Option<String> {
  if token.is_empty() || !token.chars().all(|c| c.is_ascii_digit()) {
    return None;
  }
  Some(token.to_owned())
}

fn parse_separated_quantity(token: &str) -> Option<String> {
  let digits = token.strip_prefix(['x', 'X']).unwrap_or(token);
  if digits.is_empty() {
    return None;
  }

  let mut cleaned = String::with_capacity(digits.len());
  for ch in digits.chars() {
    match ch {
      '0'..='9' => cleaned.push(ch),
      ',' | '.' | '\'' | '\u{a0}' | '\u{202f}' | '_' => {}
      _ => return None,
    }
  }

  (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod parse {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_a_bare_name_to_quantity_one() {
      assert_eq!(parse("Tritanium"), vec![("Tritanium".to_owned(), 1)]);
    }

    #[test]
    fn it_defaults_a_multi_word_bare_name_to_quantity_one() {
      assert_eq!(parse("Damage Control II"), vec![("Damage Control II".to_owned(), 1)]);
    }

    #[test]
    fn it_does_not_misread_a_unit_glued_leading_number() {
      assert_eq!(parse("425mm Railgun II 50"), vec![("425mm Railgun II".to_owned(), 50)]);
    }

    #[test]
    fn it_ignores_blank_and_whitespace_only_lines() {
      assert_eq!(
        parse("Tritanium 5\n\n   \n\t\nPyerite 3"),
        vec![("Tritanium".to_owned(), 5), ("Pyerite".to_owned(), 3)]
      );
    }

    #[test]
    fn it_keeps_a_trailing_quantity_with_a_leading_unit_glued_number() {
      assert_eq!(
        parse("Mobile Tractor Unit               25"),
        vec![("Mobile Tractor Unit".to_owned(), 25)]
      );
      assert_eq!(
        parse("Mobile Tractor Unit         x25"),
        vec![("Mobile Tractor Unit".to_owned(), 25)]
      );
    }

    #[test]
    fn it_keeps_multi_word_names_with_a_trailing_quantity() {
      assert_eq!(
        parse("Medium Shield Extender II\t3"),
        vec![("Medium Shield Extender II".to_owned(), 3)]
      );
    }

    #[test]
    fn it_parses_a_leading_bare_quantity() {
      assert_eq!(
        parse("25      Mobile Tractor Unit"),
        vec![("Mobile Tractor Unit".to_owned(), 25)]
      );
    }

    #[test]
    fn it_parses_a_leading_space_thousands_quantity() {
      assert_eq!(
        parse("1 000 Mobile Tractor Unit"),
        vec![("Mobile Tractor Unit".to_owned(), 1000)]
      );
    }

    #[test]
    fn it_parses_a_leading_x_quantity() {
      assert_eq!(
        parse("x25      Mobile Tractor Unit"),
        vec![("Mobile Tractor Unit".to_owned(), 25)]
      );
    }

    #[test]
    fn it_parses_a_multi_space_separated_line() {
      assert_eq!(parse("Tritanium    1000"), vec![("Tritanium".to_owned(), 1000)]);
    }

    #[test]
    fn it_parses_a_native_eve_copy_with_a_leading_quantity_form_present() {
      assert_eq!(parse("Tritanium\t1,000,000"), vec![("Tritanium".to_owned(), 1_000_000)]);
    }

    #[test]
    fn it_parses_a_realistic_mixed_multibuy_blob() {
      let blob = "\
Tritanium\t1,000,000
Pyerite 250 000

Mexallon x5
Damage Control II
Medium Shield Extender II    2
Tritanium 500
";

      assert_eq!(
        parse(blob),
        vec![
          ("Tritanium".to_owned(), 1_000_500),
          ("Pyerite".to_owned(), 250_000),
          ("Mexallon".to_owned(), 5),
          ("Damage Control II".to_owned(), 1),
          ("Medium Shield Extender II".to_owned(), 2),
        ]
      );
    }

    #[test]
    fn it_parses_a_single_space_separated_line() {
      assert_eq!(parse("Tritanium 100"), vec![("Tritanium".to_owned(), 100)]);
    }

    #[test]
    fn it_parses_a_tab_separated_line() {
      assert_eq!(parse("Tritanium\t100"), vec![("Tritanium".to_owned(), 100)]);
    }

    #[test]
    fn it_parses_the_x_quantity_form() {
      assert_eq!(parse("Tritanium x5"), vec![("Tritanium".to_owned(), 5)]);
      assert_eq!(parse("Tritanium X5"), vec![("Tritanium".to_owned(), 5)]);
    }

    #[test]
    fn it_prefers_a_leading_quantity_over_a_trailing_one() {
      assert_eq!(parse("425 Railgun II 50"), vec![("Railgun II".to_owned(), 425)]);
    }

    #[test]
    fn it_sums_duplicate_names_preserving_first_seen_order() {
      assert_eq!(
        parse("Tritanium 100\nPyerite 50\nTritanium 25"),
        vec![("Tritanium".to_owned(), 125), ("Pyerite".to_owned(), 50)]
      );
    }

    #[test]
    fn it_sums_duplicates_across_prefix_suffix_and_bare_forms() {
      assert_eq!(
        parse("x10 Tritanium\nTritanium 5\nTritanium"),
        vec![("Tritanium".to_owned(), 16)]
      );
    }

    #[test]
    fn it_sums_duplicates_across_quantity_forms() {
      assert_eq!(
        parse("Tritanium x10\nTritanium\t5\nTritanium"),
        vec![("Tritanium".to_owned(), 16)]
      );
    }

    #[test]
    fn it_tolerates_a_thin_space_thousands_separator_inside_a_token() {
      assert_eq!(parse("Tritanium 1\u{202f}000"), vec![("Tritanium".to_owned(), 1000)]);
    }

    #[test]
    fn it_tolerates_comma_thousands_separators() {
      assert_eq!(parse("Tritanium 1,000"), vec![("Tritanium".to_owned(), 1000)]);
      assert_eq!(parse("Tritanium 1,234,567"), vec![("Tritanium".to_owned(), 1_234_567)]);
    }

    #[test]
    fn it_tolerates_period_thousands_separators() {
      assert_eq!(parse("Tritanium 1.000"), vec![("Tritanium".to_owned(), 1000)]);
    }

    #[test]
    fn it_tolerates_space_thousands_separators() {
      assert_eq!(parse("Tritanium 1 000"), vec![("Tritanium".to_owned(), 1000)]);
      assert_eq!(parse("Tritanium 1 234 567"), vec![("Tritanium".to_owned(), 1_234_567)]);
    }

    #[test]
    fn it_treats_a_garbage_trailing_token_as_part_of_the_name() {
      assert_eq!(parse("Tritanium abc"), vec![("Tritanium abc".to_owned(), 1)]);
    }

    #[test]
    fn it_treats_a_negative_quantity_as_a_bare_name() {
      assert_eq!(parse("Tritanium -5"), vec![("Tritanium -5".to_owned(), 1)]);
    }

    #[test]
    fn it_treats_a_zero_quantity_as_a_bare_name() {
      assert_eq!(parse("Tritanium 0"), vec![("Tritanium 0".to_owned(), 1)]);
    }

    #[test]
    fn it_trims_leading_and_trailing_whitespace() {
      assert_eq!(parse("   Tritanium 5   "), vec![("Tritanium".to_owned(), 5)]);
    }
  }

  mod parse_digit_group {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_a_bare_digit_group() {
      assert_eq!(parse_digit_group("000"), Some("000".to_owned()));
      assert_eq!(parse_digit_group("42"), Some("42".to_owned()));
    }

    #[test]
    fn it_rejects_non_digit_tokens() {
      assert_eq!(parse_digit_group("x5"), None);
      assert_eq!(parse_digit_group("1,000"), None);
      assert_eq!(parse_digit_group("abc"), None);
      assert_eq!(parse_digit_group(""), None);
    }
  }

  mod parse_separated_quantity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_the_x_prefix_form() {
      assert_eq!(parse_separated_quantity("x42"), Some("42".to_owned()));
      assert_eq!(parse_separated_quantity("X42"), Some("42".to_owned()));
    }

    #[test]
    fn it_rejects_non_numeric_tokens() {
      assert_eq!(parse_separated_quantity("abc"), None);
      assert_eq!(parse_separated_quantity("-5"), None);
      assert_eq!(parse_separated_quantity("x"), None);
      assert_eq!(parse_separated_quantity(""), None);
    }

    #[test]
    fn it_strips_separators() {
      assert_eq!(parse_separated_quantity("1,000"), Some("1000".to_owned()));
      assert_eq!(parse_separated_quantity("1.000"), Some("1000".to_owned()));
      assert_eq!(parse_separated_quantity("1'000"), Some("1000".to_owned()));
    }
  }

  mod serialize {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_each_line_as_name_tab_quantity_joined_by_newlines() {
      let items = vec![
        ("Tritanium".to_owned(), 1000),
        ("Medium Shield Extender II".to_owned(), 3),
      ];

      assert_eq!(serialize(&items), "Tritanium\t1000\nMedium Shield Extender II\t3");
    }

    #[test]
    fn it_round_trips_through_parse() {
      let items = vec![("Tritanium".to_owned(), 1000), ("Pyerite".to_owned(), 50)];

      assert_eq!(parse(&serialize(&items)), items);
    }

    #[test]
    fn it_serializes_an_empty_list_to_an_empty_string() {
      assert_eq!(serialize(&[]), "");
    }

    #[test]
    fn it_writes_quantities_without_thousands_separators() {
      assert_eq!(serialize(&[("Tritanium".to_owned(), 1_234_567)]), "Tritanium\t1234567");
    }
  }
}
