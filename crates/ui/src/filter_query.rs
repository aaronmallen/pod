use pod_model::Character;

#[derive(Clone, Debug, PartialEq)]
pub enum ChipKind {
  KeyValue,
  Negated,
  FreeText,
}

#[derive(Clone, Debug)]
pub enum FilterToken {
  KeyValue {
    key: String,
    negated: bool,
    values: Vec<String>,
  },
  FreeText(String),
}

#[derive(Clone, Debug)]
pub struct ParsedQuery {
  pub tokens: Vec<FilterToken>,
}

const RECOGNIZED_KEYS: &[&str] = &[
  "tag",
  "corp",
  "corporation",
  "loc",
  "location",
  "status",
  "training",
  "name",
];

fn normalize_key(key: &str) -> String {
  match key.to_lowercase().as_str() {
    "corporation" => "corp".to_string(),
    "location" => "loc".to_string(),
    k => k.to_string(),
  }
}

fn is_recognized_key(key: &str) -> bool {
  RECOGNIZED_KEYS.contains(&key.to_lowercase().as_str())
}

fn flush_current(current: &mut String, tokens: &mut Vec<String>) {
  if !current.is_empty() {
    tokens.push(current.clone());
    current.clear();
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

fn tokenize(input: &str) -> Vec<String> {
  let mut tokens = Vec::new();
  let mut chars = input.chars().peekable();
  let mut current = String::new();

  while let Some(&ch) = chars.peek() {
    if ch == '"' {
      chars.next();
      flush_current(&mut current, &mut tokens);
      let quoted = consume_quoted(&mut chars);
      if !quoted.is_empty() {
        tokens.push(quoted);
      }
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

fn parse_key_value(negated: bool, key_part: &str, value_part: &str) -> Option<FilterToken> {
  if !is_recognized_key(key_part) {
    return None;
  }
  let key = normalize_key(key_part);
  let values: Vec<String> = value_part
    .split(',')
    .filter(|v| !v.is_empty())
    .map(|v| v.to_lowercase())
    .collect();
  if values.is_empty() {
    return None;
  }
  Some(FilterToken::KeyValue {
    key,
    negated,
    values,
  })
}

fn free_text(negated: bool, rest: &str) -> FilterToken {
  if negated {
    FilterToken::FreeText(format!("-{rest}"))
  } else {
    FilterToken::FreeText(rest.to_string())
  }
}

fn parse_raw_token(raw: &str) -> FilterToken {
  let (negated, rest) = match raw.strip_prefix('-') {
    Some(stripped) => (true, stripped),
    None => (false, raw),
  };
  if let Some(colon_pos) = rest.find(':') {
    if let Some(token) = parse_key_value(negated, &rest[..colon_pos], &rest[colon_pos + 1..]) {
      return token;
    }
  }
  free_text(negated, rest)
}

pub fn parse(input: &str) -> ParsedQuery {
  let tokens = tokenize(input.trim())
    .into_iter()
    .map(|raw| parse_raw_token(&raw))
    .collect();
  ParsedQuery {
    tokens,
  }
}

impl ParsedQuery {
  pub fn matches_character(&self, character: &Character) -> bool {
    self.tokens.iter().all(|token| match_token(token, character))
  }

  pub fn display_chips(&self) -> Vec<(String, ChipKind)> {
    self
      .tokens
      .iter()
      .map(|token| match token {
        FilterToken::KeyValue {
          key,
          negated: false,
          values,
        } => (format!("{}:{}", key, values.join(",")), ChipKind::KeyValue),
        FilterToken::KeyValue {
          key,
          negated: true,
          values,
        } => (format!("-{}:{}", key, values.join(",")), ChipKind::Negated),
        FilterToken::FreeText(s) => (s.clone(), ChipKind::FreeText),
      })
      .collect()
  }
}

fn match_free_text(needle: &str, character: &Character) -> bool {
  let needle = needle.to_lowercase();
  let mut parts: Vec<String> = vec![
    character.name().to_lowercase(),
    character.corp_name().to_lowercase(),
    character.location_name().as_deref().unwrap_or("").to_lowercase(),
  ];
  if let Some(active) = character.skills().iter().find(|s| s.is_active_training)
    && let Some(ref skill_name) = active.skill_name
  {
    parts.push(skill_name.to_lowercase());
  }
  for (_, tag_name, _) in character.tags() {
    parts.push(tag_name.to_lowercase());
  }
  parts.iter().any(|part| part.contains(needle.as_str()))
}

fn match_token(token: &FilterToken, character: &Character) -> bool {
  match token {
    FilterToken::KeyValue {
      key,
      negated,
      values,
    } => {
      let matches = match_key_value(key, values, character);
      if *negated { !matches } else { matches }
    }
    FilterToken::FreeText(s) => match_free_text(s, character),
  }
}

fn match_tag(values: &[String], character: &Character) -> bool {
  let char_tags: Vec<String> = character
    .tags()
    .iter()
    .map(|(_, name, _)| name.to_lowercase())
    .collect();
  values.iter().any(|v| char_tags.contains(v))
}

fn match_corp(values: &[String], character: &Character) -> bool {
  let corp = character.corp_name().to_lowercase();
  values.iter().any(|v| corp.contains(v.as_str()))
}

fn match_loc(values: &[String], character: &Character) -> bool {
  let loc = character.location_name().as_deref().unwrap_or("").to_lowercase();
  values.iter().any(|v| loc.contains(v.as_str()))
}

fn match_status(values: &[String], character: &Character) -> bool {
  let docked = character.location_docked().unwrap_or(false);
  values.iter().any(|v| match v.as_str() {
    "docked" => docked,
    "in-space" => !docked,
    _ => false,
  })
}

fn match_training(values: &[String], character: &Character) -> bool {
  let is_active = character.skills().iter().any(|s| s.is_active_training);
  values.iter().any(|v| match v.as_str() {
    "active" => is_active,
    "idle" => !is_active,
    _ => false,
  })
}

fn match_name(values: &[String], character: &Character) -> bool {
  let name = character.name().to_lowercase();
  values.iter().any(|v| name.contains(v.as_str()))
}

type KeyMatchFn = fn(&[String], &Character) -> bool;

fn key_match_fn(key: &str) -> Option<KeyMatchFn> {
  match key {
    "tag" => Some(match_tag),
    "corp" => Some(match_corp),
    "loc" => Some(match_loc),
    _ => key_match_fn_secondary(key),
  }
}

fn key_match_fn_secondary(key: &str) -> Option<KeyMatchFn> {
  match key {
    "status" => Some(match_status),
    "training" => Some(match_training),
    "name" => Some(match_name),
    _ => None,
  }
}

fn match_key_value(key: &str, values: &[String], character: &Character) -> bool {
  key_match_fn(key).is_some_and(|f| f(values, character))
}
