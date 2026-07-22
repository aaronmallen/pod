pub fn separated(token: &str) -> Option<String> {
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

  mod separated {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_the_x_prefix_form() {
      assert_eq!(separated("x42"), Some("42".to_owned()));
      assert_eq!(separated("X42"), Some("42".to_owned()));
    }

    #[test]
    fn it_parses_a_bare_digit_run() {
      assert_eq!(separated("42"), Some("42".to_owned()));
      assert_eq!(separated("1000"), Some("1000".to_owned()));
    }

    #[test]
    fn it_strips_comma_period_and_apostrophe_separators() {
      assert_eq!(separated("1,000"), Some("1000".to_owned()));
      assert_eq!(separated("1.000"), Some("1000".to_owned()));
      assert_eq!(separated("1'000"), Some("1000".to_owned()));
    }

    #[test]
    fn it_strips_unicode_space_and_underscore_separators() {
      assert_eq!(separated("1\u{a0}000"), Some("1000".to_owned()));
      assert_eq!(separated("1\u{202f}000"), Some("1000".to_owned()));
      assert_eq!(separated("1_000"), Some("1000".to_owned()));
    }

    #[test]
    fn it_strips_an_x_prefix_ahead_of_separators() {
      assert_eq!(separated("x1,000"), Some("1000".to_owned()));
    }

    #[test]
    fn it_rejects_non_numeric_tokens() {
      assert_eq!(separated("abc"), None);
      assert_eq!(separated("-5"), None);
      assert_eq!(separated("x"), None);
      assert_eq!(separated(""), None);
    }
  }
}
