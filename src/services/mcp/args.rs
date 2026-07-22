use std::borrow::Cow;

use serde_json::{Value, json};

use crate::services::mcp::tool::ToolError;

pub const DEFAULT_LIMIT: i64 = 50;

pub const MAX_LIMIT: i64 = 500;

pub const MIN_LIMIT: i64 = 1;

pub const MIN_PAGE: i64 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgType {
  Integer,
  IntegerArray,
  OptionalInteger {
    default: i64,
    max: Option<i64>,
    min: Option<i64>,
  },
  OptionalIntegerArray,
  OptionalNumber,
  OptionalString,
  String,
}

impl ArgType {
  fn json_schema(self) -> Value {
    match self {
      ArgType::Integer => json!({ "type": "integer" }),
      ArgType::OptionalInteger {
        default,
        max,
        min,
      } => {
        let mut schema = json!({ "type": "integer", "default": default });
        if let Value::Object(map) = &mut schema {
          if let Some(minimum) = min {
            map.insert("minimum".to_owned(), json!(minimum));
          }
          if let Some(maximum) = max {
            map.insert("maximum".to_owned(), json!(maximum));
          }
        }
        schema
      }
      ArgType::IntegerArray | ArgType::OptionalIntegerArray => {
        json!({ "type": "array", "items": { "type": "integer" } })
      }
      ArgType::OptionalNumber => json!({ "type": "number" }),
      ArgType::OptionalString | ArgType::String => json!({ "type": "string" }),
    }
  }

  fn required(self) -> bool {
    !matches!(
      self,
      ArgType::OptionalInteger { .. }
        | ArgType::OptionalIntegerArray
        | ArgType::OptionalNumber
        | ArgType::OptionalString
    )
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgSpec {
  description: Cow<'static, str>,
  name: &'static str,
  ty: ArgType,
}

impl ArgSpec {
  pub fn integer(name: &'static str, description: impl Into<Cow<'static, str>>) -> Self {
    Self {
      description: description.into(),
      name,
      ty: ArgType::Integer,
    }
  }

  pub fn integer_array(name: &'static str, description: impl Into<Cow<'static, str>>) -> Self {
    Self {
      description: description.into(),
      name,
      ty: ArgType::IntegerArray,
    }
  }

  pub fn optional_integer(name: &'static str, default: i64, description: impl Into<Cow<'static, str>>) -> Self {
    // Bounds are name-keyed: "limit" → [1, 500], "page" → [0, ∞), any other name → unconstrained.
    let (min, max) = pagination_bounds(name);
    Self {
      description: description.into(),
      name,
      ty: ArgType::OptionalInteger {
        default,
        max,
        min,
      },
    }
  }

  pub fn optional_integer_array(name: &'static str, description: impl Into<Cow<'static, str>>) -> Self {
    Self {
      description: description.into(),
      name,
      ty: ArgType::OptionalIntegerArray,
    }
  }

  pub fn optional_number(name: &'static str, description: impl Into<Cow<'static, str>>) -> Self {
    Self {
      description: description.into(),
      name,
      ty: ArgType::OptionalNumber,
    }
  }

  pub fn optional_string(name: &'static str, description: impl Into<Cow<'static, str>>) -> Self {
    Self {
      description: description.into(),
      name,
      ty: ArgType::OptionalString,
    }
  }

  pub fn string(name: &'static str, description: impl Into<Cow<'static, str>>) -> Self {
    Self {
      description: description.into(),
      name,
      ty: ArgType::String,
    }
  }

  // Accessors used by the tool-spec assertions; exercised only by the mcp tests today.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn name(&self) -> &'static str {
    self.name
  }

  // Accessors used by the tool-spec assertions; exercised only by the mcp tests today.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn ty(&self) -> ArgType {
    self.ty
  }

  fn property(&self) -> Value {
    let mut schema = self.ty.json_schema();
    if let Value::Object(map) = &mut schema {
      map.insert("description".to_owned(), json!(self.description.as_ref()));
    }
    schema
  }
}

pub fn input_schema(specs: &[ArgSpec]) -> Value {
  let mut properties = serde_json::Map::new();
  let mut required: Vec<Value> = Vec::new();
  for spec in specs {
    properties.insert(spec.name.to_owned(), spec.property());
    if spec.ty.required() {
      required.push(json!(spec.name));
    }
  }
  json!({ "type": "object", "properties": properties, "required": required })
}

/// Extracts an integer argument, accepting either a JSON integer or a numeric string so clients that
/// stringify large ids still succeed.
pub fn require_i64(args: &Value, key: &str) -> Result<i64, ToolError> {
  match args.get(key) {
    None | Some(Value::Null) => Err(ToolError::InvalidArguments(format!(
      "`{key}` is required and must be an integer (a JSON number or a numeric string)"
    ))),
    Some(value) => coerce_i64(Some(value)).ok_or_else(|| {
      ToolError::InvalidArguments(format!(
        "`{key}` must be an integer (a JSON number or a numeric string), but received {}",
        describe_value(value)
      ))
    }),
  }
}

pub fn require_i64_array(args: &Value, key: &str) -> Result<Vec<i64>, ToolError> {
  let items = args.get(key).and_then(Value::as_array).ok_or_else(|| {
    ToolError::InvalidArguments(format!(
      "`{key}` is required and must be an array of integers (JSON numbers or numeric strings)"
    ))
  })?;
  items
    .iter()
    .enumerate()
    .map(|(index, item)| {
      coerce_i64(Some(item)).ok_or_else(|| {
        ToolError::InvalidArguments(format!(
          "`{key}[{index}]` must be an integer (a JSON number or a numeric string), but received {}",
          describe_value(item)
        ))
      })
    })
    .collect()
}

pub fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
  match args.get(key) {
    None | Some(Value::Null) => Err(ToolError::InvalidArguments(format!(
      "`{key}` is required and must be a string"
    ))),
    Some(Value::String(text)) => Ok(text),
    Some(value) => Err(ToolError::InvalidArguments(format!(
      "`{key}` must be a string, but received {}",
      describe_value(value)
    ))),
  }
}

fn coerce_i64(value: Option<&Value>) -> Option<i64> {
  let value = value?;
  value
    .as_i64()
    .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
}

fn describe_value(value: &Value) -> &'static str {
  match value {
    Value::Array(_) => "an array",
    Value::Bool(_) => "a boolean",
    Value::Null => "null",
    Value::Number(_) => "a number",
    Value::Object(_) => "an object",
    Value::String(_) => "a string",
  }
}

fn pagination_bounds(name: &str) -> (Option<i64>, Option<i64>) {
  match name {
    "limit" => (Some(MIN_LIMIT), Some(MAX_LIMIT)),
    "page" => (Some(MIN_PAGE), None),
    _ => (None, None),
  }
}

pub fn pagination(args: &Value) -> (i64, i64) {
  let page = args
    .get("page")
    .and_then(Value::as_i64)
    .unwrap_or(MIN_PAGE)
    .max(MIN_PAGE);
  let limit = args
    .get("limit")
    .and_then(Value::as_i64)
    .unwrap_or(DEFAULT_LIMIT)
    .clamp(MIN_LIMIT, MAX_LIMIT);
  (page, limit)
}

pub fn paginate_vec<T>(rows: &mut Vec<T>, page: i64, limit: i64) -> (Vec<T>, bool) {
  let start = (page * limit).min(rows.len() as i64).max(0) as usize;
  let end = (start as i64 + limit).min(rows.len() as i64).max(0) as usize;
  let has_more = end < rows.len();
  (rows.drain(start..end).collect(), has_more)
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  mod require_i64 {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_a_json_integer() {
      assert_eq!(require_i64(&json!({ "id": 42 }), "id").unwrap(), 42);
    }

    #[test]
    fn it_accepts_a_numeric_string() {
      assert_eq!(
        require_i64(&json!({ "character_id": "2124367470" }), "character_id").unwrap(),
        2_124_367_470
      );
    }

    #[test]
    fn it_rejects_a_non_numeric_string() {
      let outcome = require_i64(&json!({ "id": "abc" }), "id");
      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn it_rejects_a_missing_value() {
      let outcome = require_i64(&json!({}), "id");
      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn it_names_the_expected_type_and_context_when_missing() {
      let ToolError::InvalidArguments(message) = require_i64(&json!({}), "character_id").unwrap_err() else {
        panic!("expected InvalidArguments");
      };

      assert!(message.contains("character_id"));
      assert!(message.contains("integer"));
    }

    #[test]
    fn it_names_the_received_kind_when_wrong_type() {
      let ToolError::InvalidArguments(message) = require_i64(&json!({ "id": [1] }), "id").unwrap_err() else {
        panic!("expected InvalidArguments");
      };

      assert!(message.contains("must be an integer"));
      assert!(message.contains("received an array"));
    }
  }

  mod require_i64_array {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_coerces_numeric_strings_element_wise() {
      let ids = require_i64_array(&json!({ "ids": [1, "2", 3] }), "ids").unwrap();
      assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn it_rejects_a_non_integer_element() {
      let outcome = require_i64_array(&json!({ "ids": [1, "x"] }), "ids");
      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn it_rejects_a_non_array() {
      let outcome = require_i64_array(&json!({ "ids": 7 }), "ids");
      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn it_names_the_offending_index_and_kind() {
      let ToolError::InvalidArguments(message) = require_i64_array(&json!({ "ids": [1, "x"] }), "ids").unwrap_err()
      else {
        panic!("expected InvalidArguments");
      };

      assert!(message.contains("`ids[1]`"));
      assert!(message.contains("integer"));
    }
  }

  mod require_str {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_a_string_value() {
      assert_eq!(require_str(&json!({ "name": "pod" }), "name").unwrap(), "pod");
    }

    #[test]
    fn it_rejects_a_missing_value() {
      let args = json!({});
      let outcome = require_str(&args, "name");
      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[test]
    fn it_names_the_received_kind_when_wrong_type() {
      let ToolError::InvalidArguments(message) = require_str(&json!({ "name": 7 }), "name").unwrap_err() else {
        panic!("expected InvalidArguments");
      };

      assert!(message.contains("must be a string"));
      assert!(message.contains("received a number"));
    }
  }

  mod pagination {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_and_clamps() {
      assert_eq!(super::pagination(&json!({})), (0, DEFAULT_LIMIT));
      assert_eq!(super::pagination(&json!({ "page": -3, "limit": 9000 })), (0, MAX_LIMIT));
      assert_eq!(super::pagination(&json!({ "page": 2, "limit": 0 })), (2, 1));
    }
  }

  mod paginate_vec {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_windows_and_reports_more() {
      let mut rows = vec![1, 2, 3, 4, 5];
      let (window, has_more) = super::paginate_vec(&mut rows, 0, 2);
      assert_eq!(window, vec![1, 2]);
      assert!(has_more);
    }

    #[test]
    fn it_reports_no_more_on_the_last_page() {
      let mut rows = vec![1, 2, 3];
      let (window, has_more) = super::paginate_vec(&mut rows, 1, 2);
      assert_eq!(window, vec![3]);
      assert!(!has_more);
    }
  }

  mod input_schema {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_emits_typed_properties_and_required_names() {
      let schema = input_schema(&[
        ArgSpec::integer("character_id", "The character id"),
        ArgSpec::optional_integer("page", 0, "Zero-based page"),
        ArgSpec::string("name", "A name"),
        ArgSpec::integer_array("ids", "A list of ids"),
      ]);

      assert_eq!(schema["type"], "object");
      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert_eq!(schema["properties"]["character_id"]["description"], "The character id");
      assert_eq!(schema["properties"]["page"]["type"], "integer");
      assert_eq!(schema["properties"]["name"]["type"], "string");
      assert_eq!(schema["properties"]["ids"]["type"], "array");
      assert_eq!(schema["properties"]["ids"]["items"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("character_id")));
      assert!(required.contains(&json!("name")));
      assert!(required.contains(&json!("ids")));
      assert!(!required.contains(&json!("page")));
    }

    #[test]
    fn it_keeps_optional_string_and_array_args_out_of_required() {
      let schema = input_schema(&[
        ArgSpec::optional_string("month", "A month"),
        ArgSpec::optional_integer_array("type_ids", "A list of type ids"),
      ]);

      assert_eq!(schema["properties"]["month"]["type"], "string");
      assert_eq!(schema["properties"]["type_ids"]["type"], "array");
      assert_eq!(schema["properties"]["type_ids"]["items"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(!required.contains(&json!("month")));
      assert!(!required.contains(&json!("type_ids")));
    }

    #[test]
    fn it_emits_a_number_type_and_keeps_optional_numbers_out_of_required() {
      let schema = input_schema(&[ArgSpec::optional_number("target_price", "A target price")]);

      assert_eq!(schema["properties"]["target_price"]["type"], "number");
      assert_eq!(schema["properties"]["target_price"]["description"], "A target price");

      let required = schema["required"].as_array().unwrap();
      assert!(!required.contains(&json!("target_price")));
    }

    #[test]
    fn it_publishes_bounds_for_pagination_args() {
      let schema = input_schema(&[
        ArgSpec::optional_integer("page", 0, "Zero-based page"),
        ArgSpec::optional_integer("limit", DEFAULT_LIMIT, "Page size"),
      ]);

      assert_eq!(schema["properties"]["page"]["default"], json!(0));
      assert_eq!(schema["properties"]["page"]["minimum"], json!(MIN_PAGE));
      assert!(schema["properties"]["page"].get("maximum").is_none());
      assert_eq!(schema["properties"]["limit"]["default"], json!(DEFAULT_LIMIT));
      assert_eq!(schema["properties"]["limit"]["minimum"], json!(MIN_LIMIT));
      assert_eq!(schema["properties"]["limit"]["maximum"], json!(MAX_LIMIT));
    }

    #[test]
    fn it_omits_bounds_for_a_plain_optional_integer() {
      let schema = input_schema(&[ArgSpec::optional_integer("label_id", 0, "A label id")]);

      assert_eq!(schema["properties"]["label_id"]["default"], json!(0));
      assert!(schema["properties"]["label_id"].get("minimum").is_none());
      assert!(schema["properties"]["label_id"].get("maximum").is_none());
    }
  }
}
