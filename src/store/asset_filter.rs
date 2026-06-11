use sqlx::{QueryBuilder, Sqlite};

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

const KEY_ALIASES: &[(&str, &str)] = &[
  ("c", "constellation"),
  ("cat", "category"),
  ("g", "group"),
  ("loc", "location"),
  ("n", "name"),
  ("r", "region"),
  ("s", "system"),
];

#[derive(Clone, Debug, PartialEq)]
pub enum AssetFilterToken {
  FreeText(String),
  KeyValue {
    key: String,
    negated: bool,
    values: Vec<String>,
  },
}

#[derive(Clone, Copy, Debug)]
pub struct ColumnSchema {
  pub category: &'static str,
  pub character_id: &'static str,
  pub constellation_name: &'static str,
  pub group_name: &'static str,
  pub is_blueprint_copy: &'static str,
  pub is_singleton: &'static str,
  pub location_name: &'static str,
  pub name: &'static str,
  pub region_name: &'static str,
  pub system_name: &'static str,
  pub type_name: &'static str,
}

impl Default for ColumnSchema {
  fn default() -> Self {
    Self {
      category: "category",
      character_id: "character_id",
      constellation_name: "constellation_name",
      group_name: "group_name",
      is_blueprint_copy: "is_blueprint_copy",
      is_singleton: "is_singleton",
      location_name: "location_name",
      name: "name",
      region_name: "region_name",
      system_name: "system_name",
      type_name: "type_name",
    }
  }
}

struct Compiler<'a> {
  params: Vec<FilterParam>,
  schema: &'a ColumnSchema,
  sql: String,
}

impl Compiler<'_> {
  fn push_token(&mut self, token: &AssetFilterToken, context: FilterContext) {
    match token {
      AssetFilterToken::FreeText(text) => self.push_free_text(text),
      AssetFilterToken::KeyValue {
        key,
        negated,
        values,
      } => {
        if *negated {
          self.sql.push_str("NOT ");
        }
        self.sql.push('(');
        for (index, value) in values.iter().enumerate() {
          if index > 0 {
            self.sql.push_str(" OR ");
          }
          self.push_key_value(key, value, context);
        }
        self.sql.push(')');
      }
    }
  }

  fn push_free_text(&mut self, text: &str) {
    let pattern = like_pattern(text);
    self.sql.push('(');
    self.push_like(self.schema.name, pattern.clone());
    self.sql.push_str(" OR ");
    self.push_like(self.schema.type_name, pattern.clone());
    self.sql.push_str(" OR ");
    self.push_like(self.schema.group_name, pattern.clone());
    self.sql.push_str(" OR ");
    self.push_like(self.schema.location_name, pattern);
    self.sql.push(')');
  }

  fn push_key_value(&mut self, key: &str, value: &str, context: FilterContext) {
    match key {
      "name" => self.push_like(self.schema.type_name, like_pattern(value)),
      "group" => self.push_like(self.schema.group_name, like_pattern(value)),
      "system" => self.push_like(self.schema.system_name, like_pattern(value)),
      "location" => self.push_like(self.schema.location_name, like_pattern(value)),
      "category" => self.push_exact(self.schema.category, value),
      "region" => self.push_exact(self.schema.region_name, value),
      "constellation" => self.push_exact(self.schema.constellation_name, value),
      "owner" => self.push_owner(value, context),
      "type" => self.push_type(value),
      // Unknown facet key compiles to a predicate that matches no rows.
      _ => self.sql.push_str("0 = 1"),
    }
  }

  fn push_owner(&mut self, value: &str, context: FilterContext) {
    match (value, context.me_id) {
      ("me", Some(me_id)) => {
        self.sql.push_str(self.schema.character_id);
        self.sql.push_str(" = ");
        self.push_param(FilterParam::Int(me_id));
      }
      _ => self.sql.push_str("0 = 1"),
    }
  }

  fn push_type(&mut self, value: &str) {
    match value {
      "bpc" => {
        self.sql.push('(');
        self.sql.push_str(self.schema.is_blueprint_copy);
        self.sql.push_str(" = 1)");
      }
      "bpo" => {
        self.sql.push('(');
        self.sql.push_str(self.schema.is_blueprint_copy);
        self.sql.push_str(" IS NOT NULL AND ");
        self.sql.push_str(self.schema.is_blueprint_copy);
        self.sql.push_str(" = 0)");
      }
      "singleton" => {
        self.sql.push('(');
        self.sql.push_str(self.schema.is_singleton);
        self.sql.push_str(" = 1)");
      }
      "stack" => {
        self.sql.push('(');
        self.sql.push_str(self.schema.is_singleton);
        self.sql.push_str(" = 0)");
      }
      _ => self.sql.push_str("0 = 1"),
    }
  }

  fn push_like(&mut self, column: &str, pattern: String) {
    self.sql.push_str(column);
    self.sql.push_str(" LIKE ");
    self.push_param(FilterParam::Text(pattern));
    self.sql.push_str(" ESCAPE '\\'");
  }

  fn push_exact(&mut self, column: &str, value: &str) {
    self.sql.push_str(column);
    self.sql.push_str(" = ");
    self.push_param(FilterParam::Text(value.to_owned()));
    self.sql.push_str(" COLLATE NOCASE");
  }

  fn push_param(&mut self, param: FilterParam) {
    self.sql.push('?');
    self.params.push(param);
  }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FilterContext {
  pub me_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterParam {
  Int(i64),
  Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WhereClause {
  pub params: Vec<FilterParam>,
  pub sql: String,
}

impl WhereClause {
  pub fn bind_onto(&self, builder: &mut QueryBuilder<Sqlite>) {
    let mut params = self.params.iter();
    for segment in self.sql.split('?') {
      builder.push(segment);
      if let Some(param) = params.next() {
        match param {
          FilterParam::Int(value) => builder.push_bind(*value),
          FilterParam::Text(value) => builder.push_bind(value.clone()),
        };
      }
    }
  }
}

pub fn parse(input: &str) -> Vec<AssetFilterToken> {
  tokenize(input.trim()).into_iter().map(parse_raw_token).collect()
}

pub fn compile(tokens: &[AssetFilterToken], schema: &ColumnSchema, context: FilterContext) -> Option<WhereClause> {
  if tokens.is_empty() {
    return None;
  }
  let mut compiler = Compiler {
    params: Vec::new(),
    schema,
    sql: String::new(),
  };
  for (index, token) in tokens.iter().enumerate() {
    if index > 0 {
      compiler.sql.push_str(" AND ");
    }
    compiler.push_token(token, context);
  }
  Some(WhereClause {
    params: compiler.params,
    sql: compiler.sql,
  })
}

pub fn compile_query(input: &str, schema: &ColumnSchema, context: FilterContext) -> Option<WhereClause> {
  compile(&parse(input), schema, context)
}

fn escape_like(value: &str) -> String {
  value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn like_pattern(value: &str) -> String {
  format!("%{}%", escape_like(value))
}

fn is_recognized_key(key: &str) -> bool {
  RECOGNIZED_KEYS.contains(&key.to_lowercase().as_str())
}

fn normalize_key(key: &str) -> String {
  let lower = key.to_lowercase();
  KEY_ALIASES
    .iter()
    .find(|&&(alias, _)| alias == lower.as_str())
    .map(|&(_, canonical)| canonical.to_owned())
    .unwrap_or(lower)
}

fn parse_value_list(value_part: &str) -> Vec<String> {
  value_part
    .split(',')
    .filter(|value| !value.is_empty())
    .map(str::to_lowercase)
    .collect()
}

fn try_parse_key_value(negated: bool, rest: &str) -> Option<AssetFilterToken> {
  let colon = rest.find(':')?;
  if !is_recognized_key(&rest[..colon]) {
    return None;
  }
  let values = parse_value_list(&rest[colon + 1..]);
  if values.is_empty() {
    return None;
  }
  Some(AssetFilterToken::KeyValue {
    key: normalize_key(&rest[..colon]),
    negated,
    values,
  })
}

fn parse_raw_token(raw: String) -> AssetFilterToken {
  let (negated, rest) = match raw.strip_prefix('-') {
    Some(stripped) => (true, stripped.to_owned()),
    None => (false, raw),
  };
  if let Some(token) = try_parse_key_value(negated, &rest) {
    return token;
  }
  if negated {
    AssetFilterToken::FreeText(format!("-{rest}"))
  } else {
    AssetFilterToken::FreeText(rest)
  }
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

  fn compile_default(input: &str) -> Option<WhereClause> {
    compile_query(input, &ColumnSchema::default(), FilterContext::default())
  }

  fn compile_with_me(input: &str, me_id: i64) -> Option<WhereClause> {
    compile_query(
      input,
      &ColumnSchema::default(),
      FilterContext {
        me_id: Some(me_id),
      },
    )
  }

  fn text(value: &str) -> FilterParam {
    FilterParam::Text(value.to_owned())
  }

  mod parse {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_no_tokens_for_blank_input() {
      assert_eq!(parse("   "), vec![]);
    }

    #[test]
    fn it_treats_a_bare_word_as_free_text() {
      assert_eq!(parse("rifter"), vec![AssetFilterToken::FreeText("rifter".to_owned())]);
    }

    #[test]
    fn it_normalizes_every_short_alias() {
      for (alias, canonical) in [
        ("n", "name"),
        ("g", "group"),
        ("cat", "category"),
        ("r", "region"),
        ("c", "constellation"),
        ("s", "system"),
        ("loc", "location"),
      ] {
        assert_eq!(
          parse(&format!("{alias}:value")),
          vec![AssetFilterToken::KeyValue {
            key: canonical.to_owned(),
            negated: false,
            values: vec!["value".to_owned()],
          }],
          "alias {alias} should normalize to {canonical}"
        );
      }
    }

    #[test]
    fn it_splits_comma_values_as_or_within_a_key() {
      assert_eq!(
        parse("category:drone,ship"),
        vec![AssetFilterToken::KeyValue {
          key: "category".to_owned(),
          negated: false,
          values: vec!["drone".to_owned(), "ship".to_owned()],
        }]
      );
    }

    #[test]
    fn it_negates_a_facet_with_a_leading_dash() {
      assert_eq!(
        parse("-category:ship"),
        vec![AssetFilterToken::KeyValue {
          key: "category".to_owned(),
          negated: true,
          values: vec!["ship".to_owned()],
        }]
      );
    }

    #[test]
    fn it_keeps_a_quoted_phrase_as_a_single_value() {
      assert_eq!(
        parse("region:\"The Forge\""),
        vec![AssetFilterToken::KeyValue {
          key: "region".to_owned(),
          negated: false,
          values: vec!["the forge".to_owned()],
        }]
      );
    }

    #[test]
    fn it_degrades_an_unrecognized_key_to_free_text() {
      assert_eq!(
        parse("clone:omega"),
        vec![AssetFilterToken::FreeText("clone:omega".to_owned())]
      );
    }

    #[test]
    fn it_degrades_an_empty_value_to_free_text() {
      assert_eq!(parse("name:"), vec![AssetFilterToken::FreeText("name:".to_owned())]);
    }
  }

  mod compile {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_none_for_an_empty_query() {
      assert_eq!(compile_default(""), None);
      assert_eq!(compile_default("   "), None);
    }

    #[test]
    fn it_compiles_free_text_to_a_four_column_partial_match() {
      let clause = compile_default("rifter").unwrap();

      assert_eq!(
        clause.sql,
        "(name LIKE ? ESCAPE '\\' OR type_name LIKE ? ESCAPE '\\' OR group_name LIKE ? ESCAPE '\\' \
        OR location_name LIKE ? ESCAPE '\\')"
      );
      assert_eq!(
        clause.params,
        vec![text("%rifter%"), text("%rifter%"), text("%rifter%"), text("%rifter%")]
      );
    }

    #[test]
    fn it_compiles_name_and_its_alias_identically_to_a_type_name_like() {
      let expected_sql = "(type_name LIKE ? ESCAPE '\\')";
      let expected_params = vec![text("%rift%")];

      for input in ["name:rift", "n:rift"] {
        let clause = compile_default(input).unwrap();
        assert_eq!(clause.sql, expected_sql, "{input}");
        assert_eq!(clause.params, expected_params, "{input}");
      }
    }

    #[test]
    fn it_compiles_group_and_its_alias_to_a_group_name_like() {
      for input in ["group:frig", "g:frig"] {
        let clause = compile_default(input).unwrap();
        assert_eq!(clause.sql, "(group_name LIKE ? ESCAPE '\\')", "{input}");
        assert_eq!(clause.params, vec![text("%frig%")], "{input}");
      }
    }

    #[test]
    fn it_compiles_system_and_its_alias_to_a_system_name_like() {
      for input in ["system:jita", "s:jita"] {
        let clause = compile_default(input).unwrap();
        assert_eq!(clause.sql, "(system_name LIKE ? ESCAPE '\\')", "{input}");
        assert_eq!(clause.params, vec![text("%jita%")], "{input}");
      }
    }

    #[test]
    fn it_compiles_location_and_its_alias_to_a_location_name_like() {
      for input in ["location:hangar", "loc:hangar"] {
        let clause = compile_default(input).unwrap();
        assert_eq!(clause.sql, "(location_name LIKE ? ESCAPE '\\')", "{input}");
        assert_eq!(clause.params, vec![text("%hangar%")], "{input}");
      }
    }

    #[test]
    fn it_compiles_a_quoted_multi_word_location_value() {
      let clause = compile_default("loc:\"caldari navy\"").unwrap();
      assert_eq!(clause.sql, "(location_name LIKE ? ESCAPE '\\')");
      assert_eq!(clause.params, vec![text("%caldari navy%")]);
    }

    #[test]
    fn it_compiles_category_and_its_alias_to_an_exact_nocase_match() {
      for input in ["category:ship", "cat:ship"] {
        let clause = compile_default(input).unwrap();
        assert_eq!(clause.sql, "(category = ? COLLATE NOCASE)", "{input}");
        assert_eq!(clause.params, vec![text("ship")], "{input}");
      }
    }

    #[test]
    fn it_compiles_region_and_its_alias_to_an_exact_nocase_match() {
      let clause = compile_default("region:\"The Forge\"").unwrap();
      assert_eq!(clause.sql, "(region_name = ? COLLATE NOCASE)");
      assert_eq!(clause.params, vec![text("the forge")]);

      let aliased = compile_default("r:heimatar").unwrap();
      assert_eq!(aliased.sql, "(region_name = ? COLLATE NOCASE)");
      assert_eq!(aliased.params, vec![text("heimatar")]);
    }

    #[test]
    fn it_compiles_constellation_and_its_alias_to_an_exact_nocase_match() {
      for input in ["constellation:kimotoro", "c:kimotoro"] {
        let clause = compile_default(input).unwrap();
        assert_eq!(clause.sql, "(constellation_name = ? COLLATE NOCASE)", "{input}");
        assert_eq!(clause.params, vec![text("kimotoro")], "{input}");
      }
    }

    #[test]
    fn it_compiles_type_bpc_to_a_blueprint_copy_flag() {
      let clause = compile_default("type:bpc").unwrap();
      assert_eq!(clause.sql, "((is_blueprint_copy = 1))");
      assert!(clause.params.is_empty());
    }

    #[test]
    fn it_compiles_type_bpo_to_a_non_null_blueprint_original() {
      let clause = compile_default("type:bpo").unwrap();
      assert_eq!(
        clause.sql,
        "((is_blueprint_copy IS NOT NULL AND is_blueprint_copy = 0))"
      );
      assert!(clause.params.is_empty());
    }

    #[test]
    fn it_compiles_type_singleton_and_stack_to_the_singleton_flag() {
      assert_eq!(compile_default("type:singleton").unwrap().sql, "((is_singleton = 1))");
      assert_eq!(compile_default("type:stack").unwrap().sql, "((is_singleton = 0))");
    }

    #[test]
    fn it_compiles_owner_me_to_the_active_character_id() {
      let clause = compile_with_me("owner:me", 42).unwrap();
      assert_eq!(clause.sql, "(character_id = ?)");
      assert_eq!(clause.params, vec![FilterParam::Int(42)]);
    }

    #[test]
    fn it_compiles_owner_me_to_match_nothing_without_an_active_character() {
      let clause = compile_default("owner:me").unwrap();
      assert_eq!(clause.sql, "(0 = 1)");
      assert!(clause.params.is_empty());
    }

    #[test]
    fn it_compiles_a_non_me_owner_value_to_match_nothing() {
      let clause = compile_with_me("owner:someone", 42).unwrap();
      assert_eq!(clause.sql, "(0 = 1)");
      assert!(clause.params.is_empty());
    }

    #[test]
    fn it_compiles_an_unknown_type_value_to_match_nothing() {
      let clause = compile_default("type:nonsense").unwrap();
      assert_eq!(clause.sql, "(0 = 1)");
    }

    #[test]
    fn it_negates_a_facet_with_a_not_prefix() {
      let clause = compile_default("-category:ship").unwrap();
      assert_eq!(clause.sql, "NOT (category = ? COLLATE NOCASE)");
      assert_eq!(clause.params, vec![text("ship")]);
    }

    #[test]
    fn it_combines_multi_values_as_or_within_a_key() {
      let clause = compile_default("category:drone,ship").unwrap();
      assert_eq!(
        clause.sql,
        "(category = ? COLLATE NOCASE OR category = ? COLLATE NOCASE)"
      );
      assert_eq!(clause.params, vec![text("drone"), text("ship")]);
    }

    #[test]
    fn it_combines_multiple_tokens_as_and() {
      let clause = compile_default("category:ship name:rifter").unwrap();
      assert_eq!(
        clause.sql,
        "(category = ? COLLATE NOCASE) AND (type_name LIKE ? ESCAPE '\\')"
      );
      assert_eq!(clause.params, vec![text("ship"), text("%rifter%")]);
    }

    #[test]
    fn it_treats_a_quoted_phrase_as_a_single_bound_value() {
      let clause = compile_default("name:\"navy issue\"").unwrap();
      assert_eq!(clause.sql, "(type_name LIKE ? ESCAPE '\\')");
      assert_eq!(clause.params, vec![text("%navy issue%")]);
    }

    #[test]
    fn it_binds_like_metacharacters_rather_than_interpolating_them() {
      let clause = compile_default("name:50%_x").unwrap();
      assert_eq!(clause.sql, "(type_name LIKE ? ESCAPE '\\')");
      assert_eq!(clause.params, vec![text("%50\\%\\_x%")]);
    }

    #[test]
    fn it_honours_an_overridden_column_schema() {
      let schema = ColumnSchema {
        type_name: "it.name",
        ..ColumnSchema::default()
      };
      let clause = compile_query("name:rift", &schema, FilterContext::default()).unwrap();
      assert_eq!(clause.sql, "(it.name LIKE ? ESCAPE '\\')");
    }
  }

  mod bind_onto {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_placeholders_and_threads_the_params_in_order() {
      let clause = compile_default("category:ship name:rifter").unwrap();
      let mut builder = QueryBuilder::<Sqlite>::new("SELECT 1 WHERE ");

      clause.bind_onto(&mut builder);

      assert_eq!(
        builder.sql(),
        "SELECT 1 WHERE (category = ? COLLATE NOCASE) AND (type_name LIKE ? ESCAPE '\\')"
      );
    }

    #[test]
    fn it_binds_an_owner_me_integer_param() {
      let clause = compile_with_me("owner:me", 7).unwrap();
      let mut builder = QueryBuilder::<Sqlite>::new("SELECT 1 WHERE ");

      clause.bind_onto(&mut builder);

      assert_eq!(builder.sql(), "SELECT 1 WHERE (character_id = ?)");
    }
  }
}
