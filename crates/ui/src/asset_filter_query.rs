//! Structured query parser for the assets inventory filter bar.
use crate::views::assets::AssetRecord;

const RECOGNIZED_KEYS: &[&str] = &[
  "c",
  "cat",
  "category",
  "constellation",
  "g",
  "group",
  "loc",
  "location",
  "n",
  "name",
  "owner",
  "r",
  "region",
  "s",
  "system",
  "type",
];

/// Parsed asset filter query. Build with [`AssetFilterQuery::parse`].
#[derive(Clone, Debug)]
pub struct AssetFilterQuery {
  me_id: Option<i64>,
  tokens: Vec<AssetFilterToken>,
}

impl AssetFilterQuery {
  /// Parses a query string into a structured filter.
  pub fn parse(input: &str) -> Self {
    let raw = tokenize(input.trim());
    let mut tokens = Vec::new();
    for raw_tok in raw {
      tokens.push(parse_raw_token(raw_tok));
    }
    Self {
      me_id: None,
      tokens,
    }
  }

  /// Returns `true` if `asset` satisfies all tokens in the query (AND logic).
  pub fn matches(&self, asset: &AssetRecord) -> bool {
    self.tokens.iter().all(|tok| match_token(tok, asset, self.me_id))
  }

  /// Sets the character ID that `owner:me` resolves to.
  pub fn with_me(mut self, me_id: Option<i64>) -> Self {
    self.me_id = me_id;
    self
  }
}

#[derive(Clone, Debug)]
enum AssetFilterToken {
  FreeText(String),
  KeyValue {
    key: String,
    negated: bool,
    values: Vec<String>,
  },
}

fn is_recognized_key(key: &str) -> bool {
  RECOGNIZED_KEYS.contains(&key.to_lowercase().as_str())
}

fn try_parse_key_value(negated: bool, rest: &str) -> Option<AssetFilterToken> {
  let colon_pos = rest.find(':')?;
  let key_part = &rest[..colon_pos];
  if !is_recognized_key(key_part) {
    return None;
  }
  let value_part = &rest[colon_pos + 1..];
  let values: Vec<String> = value_part
    .split(',')
    .filter(|v| !v.is_empty())
    .map(|v| v.to_lowercase())
    .collect();
  if values.is_empty() {
    return None;
  }
  Some(AssetFilterToken::KeyValue {
    key: normalize_key(key_part),
    negated,
    values,
  })
}

fn parse_raw_token(raw_tok: String) -> AssetFilterToken {
  let (negated, rest) = match raw_tok.strip_prefix('-') {
    Some(s) => (true, s.to_string()),
    None => (false, raw_tok),
  };
  if let Some(tok) = try_parse_key_value(negated, &rest) {
    return tok;
  }
  if negated {
    AssetFilterToken::FreeText(format!("-{rest}"))
  } else {
    AssetFilterToken::FreeText(rest)
  }
}

fn match_category(values: &[String], asset: &AssetRecord) -> bool {
  let cat = asset.category_key.to_lowercase();
  values.iter().any(|v| cat == v.as_str())
}

fn match_constellation(values: &[String], asset: &AssetRecord) -> bool {
  let c = asset.constellation_name.to_lowercase();
  values.iter().any(|v| c == v.as_str())
}

fn match_group(values: &[String], asset: &AssetRecord) -> bool {
  let g = asset.group_name.to_lowercase();
  values.iter().any(|v| g.contains(v.as_str()))
}

type MatchFn = fn(&[String], &AssetRecord, Option<i64>) -> bool;

const KEY_MATCHERS: &[(&str, MatchFn)] = &[
  ("category", |v, a, _| match_category(v, a)),
  ("constellation", |v, a, _| match_constellation(v, a)),
  ("group", |v, a, _| match_group(v, a)),
  ("location", |v, a, _| match_location(v, a)),
  ("name", |v, a, _| match_name(v, a)),
  ("owner", match_owner),
  ("region", |v, a, _| match_region(v, a)),
  ("system", |v, a, _| match_system(v, a)),
  ("type", |v, a, _| match_type(v, a)),
];

fn match_key_value(key: &str, values: &[String], asset: &AssetRecord, me_id: Option<i64>) -> bool {
  KEY_MATCHERS
    .iter()
    .find(|&&(k, _)| k == key)
    .map(|&(_, f)| f(values, asset, me_id))
    .unwrap_or(false)
}

fn match_location(values: &[String], asset: &AssetRecord) -> bool {
  let loc = asset.location_name.to_lowercase();
  values.iter().any(|v| loc.contains(v.as_str()))
}

fn match_name(values: &[String], asset: &AssetRecord) -> bool {
  let name = asset.type_name.to_lowercase();
  values.iter().any(|v| name.contains(v.as_str()))
}

fn match_owner(values: &[String], asset: &AssetRecord, me_id: Option<i64>) -> bool {
  values
    .iter()
    .any(|v| v == "me" && me_id.is_some_and(|id| asset.character_id == id))
}

fn match_region(values: &[String], asset: &AssetRecord) -> bool {
  let r = asset.region_name.to_lowercase();
  values.iter().any(|v| r == v.as_str())
}

fn match_system(values: &[String], asset: &AssetRecord) -> bool {
  let s = asset.system_name.to_lowercase();
  values.iter().any(|v| s.contains(v.as_str()))
}

fn match_token(tok: &AssetFilterToken, asset: &AssetRecord, me_id: Option<i64>) -> bool {
  match tok {
    AssetFilterToken::FreeText(s) => {
      let needle = s.to_lowercase();
      asset.type_name.to_lowercase().contains(&needle)
        || asset.group_name.to_lowercase().contains(&needle)
        || asset.location_name.to_lowercase().contains(&needle)
    }
    AssetFilterToken::KeyValue {
      key,
      negated,
      values,
    } => {
      let matched = match_key_value(key, values, asset, me_id);
      if *negated { !matched } else { matched }
    }
  }
}

fn asset_matches_type_value(v: &str, asset: &AssetRecord) -> bool {
  match v {
    "bpc" => asset.icon_variant == "bpc",
    "bpo" => asset.icon_variant == "bpo",
    "singleton" => asset.is_singleton,
    "stack" => !asset.is_singleton,
    _ => false,
  }
}

fn match_type(values: &[String], asset: &AssetRecord) -> bool {
  values.iter().any(|v| asset_matches_type_value(v.as_str(), asset))
}

const KEY_ALIASES: &[(&str, &str)] = &[
  ("c", "constellation"),
  ("cat", "category"),
  ("g", "group"),
  ("loc", "location"),
  ("n", "name"),
  ("r", "region"),
  ("s", "system"),
];

fn normalize_key(key: &str) -> String {
  let lower = key.to_lowercase();
  KEY_ALIASES
    .iter()
    .find(|&&(alias, _)| alias == lower.as_str())
    .map(|&(_, canonical)| canonical.to_string())
    .unwrap_or(lower)
}

fn tokenize(input: &str) -> Vec<String> {
  let mut tokens = Vec::new();
  let mut chars = input.chars().peekable();
  let mut current = String::new();

  while let Some(&ch) = chars.peek() {
    if ch.is_whitespace() {
      flush_token(&mut tokens, &mut current);
      chars.next();
    } else if ch == '"' {
      chars.next();
      collect_quoted(&mut chars, &mut current);
    } else {
      current.push(ch);
      chars.next();
    }
  }

  flush_token(&mut tokens, &mut current);
  tokens
}

fn flush_token(tokens: &mut Vec<String>, current: &mut String) {
  if !current.is_empty() {
    tokens.push(current.clone());
    current.clear();
  }
}

fn collect_quoted(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, current: &mut String) {
  for c in chars.by_ref() {
    if c == '"' {
      break;
    }
    current.push(c);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::views::assets::AssetRecord;

  fn make_asset() -> AssetRecord {
    AssetRecord {
      category_key: "ship".to_string(),
      character_id: 42,
      constellation_id: 20000020,
      constellation_name: "Kimotoro".to_string(),
      container_id: 0,
      container_path: String::new(),
      depth: 0,
      group_name: "Frigate".to_string(),
      icon_variant: "icon".to_string(),
      is_container: false,
      is_singleton: false,
      item_id: 1001,
      location_id: 60003760,
      location_name: "Jita IV - Moon 4 - Caldari Navy Assembly Plant".to_string(),
      quantity: 10,
      region_id: 10000002,
      region_name: "The Forge".to_string(),
      system_name: "Jita".to_string(),
      type_id: 587,
      type_name: "Rifter".to_string(),
      unit_price: 100_000.0,
      volume: 27289.0,
    }
  }

  mod asset_filter_query {
    use super::*;

    mod matches {
      use super::*;

      #[test]
      fn it_matches_everything_when_empty() {
        let q = AssetFilterQuery::parse("");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_freetext_against_type_name() {
        let q = AssetFilterQuery::parse("rifter");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_freetext_against_group_name() {
        let q = AssetFilterQuery::parse("frigate");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_freetext_against_location_name() {
        let q = AssetFilterQuery::parse("jita iv");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_does_not_match_freetext_against_region_name() {
        let q = AssetFilterQuery::parse("the forge");

        assert!(!q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_name_key() {
        let q = AssetFilterQuery::parse("name:rift");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_name_alias() {
        let q = AssetFilterQuery::parse("n:rift");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_group_key() {
        let q = AssetFilterQuery::parse("group:frigate");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_group_alias() {
        let q = AssetFilterQuery::parse("g:frigate");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_category_key_exactly() {
        let q_hit = AssetFilterQuery::parse("category:ship");
        let q_miss = AssetFilterQuery::parse("category:shi");

        assert!(q_hit.matches(&make_asset()));
        assert!(!q_miss.matches(&make_asset()));
      }

      #[test]
      fn it_matches_category_alias() {
        let q = AssetFilterQuery::parse("cat:ship");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_region_key_exactly() {
        let mut asset = make_asset();
        asset.region_name = "The Forge".to_string();
        let q_hit = AssetFilterQuery::parse("region:\"The Forge\"");
        let q_miss = AssetFilterQuery::parse("region:forge");

        assert!(q_hit.matches(&asset));
        assert!(!q_miss.matches(&asset));
      }

      #[test]
      fn it_matches_region_alias() {
        let mut asset = make_asset();
        asset.region_name = "Heimatar".to_string();
        let q = AssetFilterQuery::parse("r:heimatar");

        assert!(q.matches(&asset));
      }

      #[test]
      fn it_matches_constellation_key_exactly() {
        let q_hit = AssetFilterQuery::parse("constellation:kimotoro");
        let q_miss = AssetFilterQuery::parse("constellation:kimo");

        assert!(q_hit.matches(&make_asset()));
        assert!(!q_miss.matches(&make_asset()));
      }

      #[test]
      fn it_matches_constellation_alias() {
        let q = AssetFilterQuery::parse("c:kimotoro");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_system_key() {
        let q = AssetFilterQuery::parse("system:jita");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_system_alias() {
        let q = AssetFilterQuery::parse("s:jita");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_location_key() {
        let q = AssetFilterQuery::parse("location:caldari navy");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_location_alias() {
        let q = AssetFilterQuery::parse("loc:caldari navy");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_type_bpc() {
        let mut asset = make_asset();
        asset.icon_variant = "bpc".to_string();
        let q = AssetFilterQuery::parse("type:bpc");

        assert!(q.matches(&asset));
        assert!(!q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_type_bpo() {
        let mut asset = make_asset();
        asset.icon_variant = "bpo".to_string();
        let q = AssetFilterQuery::parse("type:bpo");

        assert!(q.matches(&asset));
        assert!(!q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_type_singleton() {
        let mut asset = make_asset();
        asset.is_singleton = true;
        let q = AssetFilterQuery::parse("type:singleton");

        assert!(q.matches(&asset));
        assert!(!q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_type_stack() {
        let q = AssetFilterQuery::parse("type:stack");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_matches_owner_me_when_me_id_matches() {
        let q = AssetFilterQuery::parse("owner:me").with_me(Some(42));

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_does_not_match_owner_me_when_me_id_differs() {
        let q = AssetFilterQuery::parse("owner:me").with_me(Some(99));

        assert!(!q.matches(&make_asset()));
      }

      #[test]
      fn it_does_not_match_owner_me_without_me_id() {
        let q = AssetFilterQuery::parse("owner:me");

        assert!(!q.matches(&make_asset()));
      }

      #[test]
      fn it_negates_a_key_value_token() {
        let q = AssetFilterQuery::parse("-category:ship");

        assert!(!q.matches(&make_asset()));
      }

      #[test]
      fn it_negation_passes_when_key_does_not_match() {
        let q = AssetFilterQuery::parse("-category:drone");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_applies_multivalue_as_or_within_key() {
        let q = AssetFilterQuery::parse("category:drone,ship");

        assert!(q.matches(&make_asset()));
      }

      #[test]
      fn it_applies_multiple_tokens_as_and() {
        let q_pass = AssetFilterQuery::parse("category:ship name:rifter");
        let q_fail = AssetFilterQuery::parse("category:ship name:raven");

        assert!(q_pass.matches(&make_asset()));
        assert!(!q_fail.matches(&make_asset()));
      }

      #[test]
      fn it_handles_quoted_values_with_spaces() {
        let q = AssetFilterQuery::parse("region:\"The Forge\"");

        assert!(q.matches(&make_asset()));
      }
    }
  }

  mod match_category {
    use super::*;

    #[test]
    fn it_returns_true_for_an_exact_match() {
      let asset = make_asset();
      let values = vec!["ship".to_string()];

      assert!(match_category(&values, &asset));
    }

    #[test]
    fn it_returns_false_when_no_value_matches() {
      let asset = make_asset();
      let values = vec!["drone".to_string()];

      assert!(!match_category(&values, &asset));
    }
  }

  mod match_constellation {
    use super::*;

    #[test]
    fn it_returns_true_for_an_exact_match() {
      let asset = make_asset();
      let values = vec!["kimotoro".to_string()];

      assert!(match_constellation(&values, &asset));
    }

    #[test]
    fn it_returns_false_for_a_partial_match() {
      let asset = make_asset();
      let values = vec!["kimo".to_string()];

      assert!(!match_constellation(&values, &asset));
    }
  }

  mod match_group {
    use super::*;

    #[test]
    fn it_returns_true_for_a_substring_match() {
      let asset = make_asset();
      let values = vec!["frig".to_string()];

      assert!(match_group(&values, &asset));
    }

    #[test]
    fn it_returns_false_when_no_value_matches() {
      let asset = make_asset();
      let values = vec!["cruiser".to_string()];

      assert!(!match_group(&values, &asset));
    }
  }

  mod match_location {
    use super::*;

    #[test]
    fn it_returns_true_for_a_substring_match() {
      let asset = make_asset();
      let values = vec!["caldari navy".to_string()];

      assert!(match_location(&values, &asset));
    }

    #[test]
    fn it_returns_false_when_no_value_matches() {
      let asset = make_asset();
      let values = vec!["amarr".to_string()];

      assert!(!match_location(&values, &asset));
    }
  }

  mod match_name {
    use super::*;

    #[test]
    fn it_returns_true_for_a_substring_match() {
      let asset = make_asset();
      let values = vec!["rift".to_string()];

      assert!(match_name(&values, &asset));
    }

    #[test]
    fn it_returns_false_when_no_value_matches() {
      let asset = make_asset();
      let values = vec!["raven".to_string()];

      assert!(!match_name(&values, &asset));
    }
  }

  mod match_owner {
    use super::*;

    #[test]
    fn it_returns_true_when_me_id_matches_character_id() {
      let asset = make_asset();
      let values = vec!["me".to_string()];

      assert!(match_owner(&values, &asset, Some(42)));
    }

    #[test]
    fn it_returns_false_when_me_id_does_not_match() {
      let asset = make_asset();
      let values = vec!["me".to_string()];

      assert!(!match_owner(&values, &asset, Some(99)));
    }

    #[test]
    fn it_returns_false_when_me_id_is_none() {
      let asset = make_asset();
      let values = vec!["me".to_string()];

      assert!(!match_owner(&values, &asset, None));
    }

    #[test]
    fn it_returns_false_for_non_me_values() {
      let asset = make_asset();
      let values = vec!["other".to_string()];

      assert!(!match_owner(&values, &asset, Some(42)));
    }
  }

  mod match_region {
    use super::*;

    #[test]
    fn it_returns_true_for_an_exact_match() {
      let asset = make_asset();
      let values = vec!["the forge".to_string()];

      assert!(match_region(&values, &asset));
    }

    #[test]
    fn it_returns_false_for_a_partial_match() {
      let asset = make_asset();
      let values = vec!["forge".to_string()];

      assert!(!match_region(&values, &asset));
    }
  }

  mod match_system {
    use super::*;

    #[test]
    fn it_returns_true_for_a_substring_match() {
      let asset = make_asset();
      let values = vec!["jita".to_string()];

      assert!(match_system(&values, &asset));
    }

    #[test]
    fn it_returns_false_when_no_value_matches() {
      let asset = make_asset();
      let values = vec!["amarr".to_string()];

      assert!(!match_system(&values, &asset));
    }
  }

  mod match_type {
    use super::*;

    #[test]
    fn it_matches_bpc_by_icon_variant() {
      let mut asset = make_asset();
      asset.icon_variant = "bpc".to_string();
      let values = vec!["bpc".to_string()];

      assert!(match_type(&values, &asset));
    }

    #[test]
    fn it_matches_bpo_by_icon_variant() {
      let mut asset = make_asset();
      asset.icon_variant = "bpo".to_string();
      let values = vec!["bpo".to_string()];

      assert!(match_type(&values, &asset));
    }

    #[test]
    fn it_matches_singleton_when_flag_is_true() {
      let mut asset = make_asset();
      asset.is_singleton = true;
      let values = vec!["singleton".to_string()];

      assert!(match_type(&values, &asset));
    }

    #[test]
    fn it_matches_stack_when_singleton_is_false() {
      let asset = make_asset();
      let values = vec!["stack".to_string()];

      assert!(match_type(&values, &asset));
    }

    #[test]
    fn it_returns_false_for_an_unknown_type_value() {
      let asset = make_asset();
      let values = vec!["unknown".to_string()];

      assert!(!match_type(&values, &asset));
    }
  }
}
