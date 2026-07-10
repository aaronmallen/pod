pub fn parse(token: &str) -> Option<u8> {
  let token = token.trim();
  if let Ok(n) = token.parse::<u8>() {
    return (1..=5).contains(&n).then_some(n);
  }
  match token.to_uppercase().as_str() {
    "I" => Some(1),
    "II" => Some(2),
    "III" => Some(3),
    "IV" => Some(4),
    "V" => Some(5),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod parse {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_arabic_one_through_five() {
      assert_eq!(parse("1"), Some(1));
      assert_eq!(parse("5"), Some(5));
    }

    #[test]
    fn it_rejects_arabic_out_of_range() {
      assert_eq!(parse("0"), None);
      assert_eq!(parse("6"), None);
    }

    #[test]
    fn it_accepts_roman_numerals_case_insensitively() {
      assert_eq!(parse("i"), Some(1));
      assert_eq!(parse("II"), Some(2));
      assert_eq!(parse("iii"), Some(3));
      assert_eq!(parse("IV"), Some(4));
      assert_eq!(parse("v"), Some(5));
    }

    #[test]
    fn it_rejects_unknown_and_out_of_range_roman_tokens() {
      assert_eq!(parse("vi"), None);
      assert_eq!(parse("x"), None);
      assert_eq!(parse(""), None);
    }

    #[test]
    fn it_trims_surrounding_whitespace() {
      assert_eq!(parse("  IV  "), Some(4));
    }
  }
}
