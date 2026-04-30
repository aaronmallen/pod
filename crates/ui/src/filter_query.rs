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

fn tokenize(input: &str) -> Vec<String> {
  let mut tokens = Vec::new();
  let mut chars = input.chars().peekable();
  let mut current = String::new();

  while let Some(&ch) = chars.peek() {
    if ch == '"' {
      chars.next();
      let mut quoted = String::new();
      for c in chars.by_ref() {
        if c == '"' {
          break;
        }
        quoted.push(c);
      }
      if !current.is_empty() {
        tokens.push(current.clone());
        current.clear();
      }
      if !quoted.is_empty() {
        tokens.push(quoted);
      }
    } else if ch.is_whitespace() {
      if !current.is_empty() {
        tokens.push(current.clone());
        current.clear();
      }
      chars.next();
    } else {
      current.push(ch);
      chars.next();
    }
  }

  if !current.is_empty() {
    tokens.push(current);
  }

  tokens
}

pub fn parse(input: &str) -> ParsedQuery {
  let raw_tokens = tokenize(input.trim());
  let mut tokens = Vec::new();

  for raw in raw_tokens {
    let (negated, rest) = if raw.starts_with('-') {
      (true, &raw[1..])
    } else {
      (false, raw.as_str())
    };

    if let Some(colon_pos) = rest.find(':') {
      let key_part = &rest[..colon_pos];
      let value_part = &rest[colon_pos + 1..];

      if is_recognized_key(key_part) {
        let key = normalize_key(key_part);
        let values: Vec<String> = value_part
          .split(',')
          .filter(|v| !v.is_empty())
          .map(|v| v.to_lowercase())
          .collect();

        if !values.is_empty() {
          tokens.push(FilterToken::KeyValue {
            key,
            negated,
            values,
          });
          continue;
        }
      }
    }

    if negated {
      tokens.push(FilterToken::FreeText(format!("-{rest}")));
    } else {
      tokens.push(FilterToken::FreeText(rest.to_string()));
    }
  }

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
    FilterToken::FreeText(s) => {
      let needle = s.to_lowercase();
      let mut haystack_parts: Vec<String> = vec![
        character.name().to_lowercase(),
        character.corp_name().to_lowercase(),
        character.location_name().as_deref().unwrap_or("").to_lowercase(),
      ];
      if let Some(active) = character.skills().iter().find(|s| s.is_active_training)
        && let Some(ref skill_name) = active.skill_name
      {
        haystack_parts.push(skill_name.to_lowercase());
      }
      for (_, tag_name) in character.tags() {
        haystack_parts.push(tag_name.to_lowercase());
      }
      haystack_parts.iter().any(|part| part.contains(needle.as_str()))
    }
  }
}

fn match_key_value(key: &str, values: &[String], character: &Character) -> bool {
  match key {
    "tag" => {
      let char_tags: Vec<String> = character.tags().iter().map(|(_, name)| name.to_lowercase()).collect();
      values.iter().any(|v| char_tags.contains(v))
    }
    "corp" => {
      let corp = character.corp_name().to_lowercase();
      values.iter().any(|v| corp.contains(v.as_str()))
    }
    "loc" => {
      let loc = character.location_name().as_deref().unwrap_or("").to_lowercase();
      values.iter().any(|v| loc.contains(v.as_str()))
    }
    "status" => {
      let docked = character.location_docked().unwrap_or(false);
      values.iter().any(|v| match v.as_str() {
        "docked" => docked,
        "in-space" => !docked,
        _ => false,
      })
    }
    "training" => {
      let is_active = character.skills().iter().any(|s| s.is_active_training);
      values.iter().any(|v| match v.as_str() {
        "active" => is_active,
        "idle" => !is_active,
        _ => false,
      })
    }
    "name" => {
      let name = character.name().to_lowercase();
      values.iter().any(|v| name.contains(v.as_str()))
    }
    _ => false,
  }
}
