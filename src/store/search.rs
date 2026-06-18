pub const AVAILABLE_KEYS: &[&str] = &["tag", "corp", "loc", "status", "training", "name"];

const RECOGNIZED_KEYS: &[&str] = &[
  "corp",
  "corporation",
  "loc",
  "location",
  "name",
  "status",
  "tag",
  "training",
];

#[derive(Clone, Debug, PartialEq)]
pub enum ChipKind {
  FreeText,
  KeyValue,
  Negated,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterToken {
  FreeText {
    negated: bool,
    text: String,
  },
  KeyValue {
    key: String,
    negated: bool,
    values: Vec<String>,
  },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedQuery {
  pub tokens: Vec<FilterToken>,
}

impl ParsedQuery {
  pub fn display_chips(&self) -> Vec<(String, ChipKind)> {
    self
      .tokens
      .iter()
      .map(|token| match token {
        FilterToken::FreeText {
          negated: false,
          text,
        } => (text.clone(), ChipKind::FreeText),
        FilterToken::FreeText {
          negated: true,
          text,
        } => (format!("-{text}"), ChipKind::FreeText),
        FilterToken::KeyValue {
          key,
          negated: false,
          values,
        } => (format!("{key}:{}", values.join(",")), ChipKind::KeyValue),
        FilterToken::KeyValue {
          key,
          negated: true,
          values,
        } => (format!("-{key}:{}", values.join(",")), ChipKind::Negated),
      })
      .collect()
  }
}

pub fn parse(input: &str) -> ParsedQuery {
  parse_with_keys(input, RECOGNIZED_KEYS)
}

pub fn parse_with_keys(input: &str, recognized_keys: &[&str]) -> ParsedQuery {
  let tokens = tokenize(input.trim())
    .iter()
    .map(|raw| parse_raw_token(raw, recognized_keys))
    .collect();
  ParsedQuery {
    tokens,
  }
}

fn consume_quoted(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
  let mut quoted = String::new();
  for c in chars.by_ref() {
    if c == '"' {
      break;
    }
    quoted.push(c);
  }
  quoted
}

fn flush_current(current: &mut String, tokens: &mut Vec<String>) {
  if !current.is_empty() {
    tokens.push(current.clone());
    current.clear();
  }
}

fn free_text(negated: bool, rest: &str) -> FilterToken {
  FilterToken::FreeText {
    negated,
    text: rest.to_string(),
  }
}

fn is_recognized_key(key: &str, recognized_keys: &[&str]) -> bool {
  recognized_keys.contains(&key.to_lowercase().as_str())
}

fn normalize_key(key: &str) -> String {
  match key.to_lowercase().as_str() {
    "corporation" => "corp".to_string(),
    "location" => "loc".to_string(),
    k => k.to_string(),
  }
}

fn parse_key_value(negated: bool, key_part: &str, value_part: &str, recognized_keys: &[&str]) -> Option<FilterToken> {
  if !is_recognized_key(key_part, recognized_keys) {
    return None;
  }

  let values: Vec<String> = value_part
    .split(',')
    .filter(|value| !value.is_empty())
    .map(str::to_lowercase)
    .collect();
  if values.is_empty() {
    return None;
  }

  Some(FilterToken::KeyValue {
    key: normalize_key(key_part),
    negated,
    values,
  })
}

fn parse_raw_token(raw: &str, recognized_keys: &[&str]) -> FilterToken {
  let (negated, rest) = match raw.strip_prefix('-') {
    Some(stripped) => (true, stripped),
    None => (false, raw),
  };

  if let Some(colon) = rest.find(':')
    && let Some(token) = parse_key_value(negated, &rest[..colon], &rest[colon + 1..], recognized_keys)
  {
    return token;
  }

  free_text(negated, rest)
}

fn tokenize(input: &str) -> Vec<String> {
  let mut tokens = Vec::new();
  let mut chars = input.chars().peekable();
  let mut current = String::new();

  while let Some(&ch) = chars.peek() {
    if ch == '"' {
      chars.next();
      current.push_str(&consume_quoted(&mut chars));
    } else if ch.is_whitespace() {
      flush_current(&mut current, &mut tokens);
      chars.next();
    } else {
      current.push(ch);
      chars.next();
    }
  }

  flush_current(&mut current, &mut tokens);
  tokens
}

#[cfg(test)]
mod tests {
  use super::*;

  mod display_chips {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_labels_key_value_negated_and_free_text_chips() {
      let chips = parse("tag:pvp,cruiser -corp:hostile black").display_chips();

      assert_eq!(
        chips,
        vec![
          ("tag:pvp,cruiser".to_string(), ChipKind::KeyValue),
          ("-corp:hostile".to_string(), ChipKind::Negated),
          ("black".to_string(), ChipKind::FreeText),
        ]
      );
    }
  }

  mod parse {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ands_multiple_tokens() {
      let tokens = parse("corp:cobalt tag:pvp").tokens;

      assert_eq!(
        tokens,
        vec![
          FilterToken::KeyValue {
            key: "corp".to_string(),
            negated: false,
            values: vec!["cobalt".to_string()]
          },
          FilterToken::KeyValue {
            key: "tag".to_string(),
            negated: false,
            values: vec!["pvp".to_string()]
          },
        ]
      );
    }

    #[test]
    fn it_degrades_an_empty_value_to_free_text() {
      let tokens = parse("corp:").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::FreeText {
          negated: false,
          text: "corp:".to_string()
        }]
      );
    }

    #[test]
    fn it_degrades_an_unrecognized_key_to_free_text() {
      let tokens = parse("clone:omega").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::FreeText {
          negated: false,
          text: "clone:omega".to_string()
        }]
      );
    }

    #[test]
    fn it_lowercases_key_values() {
      let tokens = parse("corp:CoBaLt").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::KeyValue {
          key: "corp".to_string(),
          negated: false,
          values: vec!["cobalt".to_string()],
        }]
      );
    }

    #[test]
    fn it_negates_a_key_value_with_a_leading_dash() {
      let tokens = parse("-tag:alt").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::KeyValue {
          key: "tag".to_string(),
          negated: true,
          values: vec!["alt".to_string()],
        }]
      );
    }

    #[test]
    fn it_negates_free_text_with_a_leading_dash() {
      let tokens = parse("-pvp").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::FreeText {
          negated: true,
          text: "pvp".to_string()
        }]
      );
    }

    #[test]
    fn it_normalizes_the_corporation_and_location_aliases() {
      let tokens = parse("corporation:a location:b").tokens;

      assert_eq!(
        tokens,
        vec![
          FilterToken::KeyValue {
            key: "corp".to_string(),
            negated: false,
            values: vec!["a".to_string()]
          },
          FilterToken::KeyValue {
            key: "loc".to_string(),
            negated: false,
            values: vec!["b".to_string()]
          },
        ]
      );
    }

    #[test]
    fn it_parses_a_bare_quoted_phrase_as_free_text() {
      let tokens = parse("\"black iris\"").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::FreeText {
          negated: false,
          text: "black iris".to_string()
        }]
      );
    }

    #[test]
    fn it_parses_a_quoted_multi_word_value() {
      let tokens = parse("loc:\"Jita IV\"").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::KeyValue {
          key: "loc".to_string(),
          negated: false,
          values: vec!["jita iv".to_string()],
        }]
      );
    }

    #[test]
    fn it_parses_each_recognized_key() {
      for key in ["name", "corp", "loc", "status", "training", "tag"] {
        let tokens = parse(&format!("{key}:value")).tokens;

        assert_eq!(
          tokens,
          vec![FilterToken::KeyValue {
            key: key.to_string(),
            negated: false,
            values: vec!["value".to_string()],
          }]
        );
      }
    }

    #[test]
    fn it_returns_no_tokens_for_blank_input() {
      assert_eq!(parse("   ").tokens, vec![]);
    }

    #[test]
    fn it_splits_comma_values_as_or_within_a_key() {
      let tokens = parse("tag:pvp,cruiser").tokens;

      assert_eq!(
        tokens,
        vec![FilterToken::KeyValue {
          key: "tag".to_string(),
          negated: false,
          values: vec!["pvp".to_string(), "cruiser".to_string()],
        }]
      );
    }

    #[test]
    fn it_treats_a_bare_word_as_free_text() {
      assert_eq!(
        parse("pvp").tokens,
        vec![FilterToken::FreeText {
          negated: false,
          text: "pvp".to_string()
        }]
      );
    }
  }
}
