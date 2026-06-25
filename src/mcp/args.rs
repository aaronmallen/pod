#![allow(dead_code)]

use serde_json::{Value, json};

use crate::mcp::tool::ToolError;

pub const DEFAULT_LIMIT: i64 = 50;

pub const MAX_LIMIT: i64 = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgType {
  Integer,
  IntegerArray,
  OptionalInteger { default: i64 },
  OptionalIntegerArray,
  OptionalString,
  String,
}

impl ArgType {
  fn json_schema(self) -> Value {
    match self {
      ArgType::Integer
      | ArgType::OptionalInteger {
        ..
      } => json!({ "type": "integer" }),
      ArgType::IntegerArray | ArgType::OptionalIntegerArray => {
        json!({ "type": "array", "items": { "type": "integer" } })
      }
      ArgType::OptionalString | ArgType::String => json!({ "type": "string" }),
    }
  }

  fn required(self) -> bool {
    !matches!(
      self,
      ArgType::OptionalInteger { .. } | ArgType::OptionalIntegerArray | ArgType::OptionalString
    )
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArgSpec {
  description: &'static str,
  name: &'static str,
  ty: ArgType,
}

impl ArgSpec {
  pub fn integer(name: &'static str, description: &'static str) -> Self {
    Self {
      description,
      name,
      ty: ArgType::Integer,
    }
  }

  pub fn integer_array(name: &'static str, description: &'static str) -> Self {
    Self {
      description,
      name,
      ty: ArgType::IntegerArray,
    }
  }

  pub fn optional_integer(name: &'static str, default: i64, description: &'static str) -> Self {
    Self {
      description,
      name,
      ty: ArgType::OptionalInteger {
        default,
      },
    }
  }

  pub fn optional_integer_array(name: &'static str, description: &'static str) -> Self {
    Self {
      description,
      name,
      ty: ArgType::OptionalIntegerArray,
    }
  }

  pub fn optional_string(name: &'static str, description: &'static str) -> Self {
    Self {
      description,
      name,
      ty: ArgType::OptionalString,
    }
  }

  pub fn string(name: &'static str, description: &'static str) -> Self {
    Self {
      description,
      name,
      ty: ArgType::String,
    }
  }

  pub fn name(&self) -> &'static str {
    self.name
  }

  pub fn ty(&self) -> ArgType {
    self.ty
  }

  fn property(&self) -> Value {
    let mut schema = self.ty.json_schema();
    if let Value::Object(map) = &mut schema {
      map.insert("description".to_owned(), json!(self.description));
    }
    schema
  }
}

/// Builds the JSON Schema for a tool's argument list: an object whose `properties` describe each
/// argument by wire type and description, and whose `required` array names the non-optional ones.
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
  coerce_i64(args.get(key))
    .ok_or_else(|| ToolError::InvalidArguments(format!("`{key}` is required and must be an integer")))
}

/// Extracts an integer-array argument, applying the same lenient string coercion element-wise.
pub fn require_i64_array(args: &Value, key: &str) -> Result<Vec<i64>, ToolError> {
  let items = args
    .get(key)
    .and_then(Value::as_array)
    .ok_or_else(|| ToolError::InvalidArguments(format!("`{key}` must be an array of integers")))?;
  items
    .iter()
    .map(|item| {
      coerce_i64(Some(item)).ok_or_else(|| ToolError::InvalidArguments(format!("`{key}` must contain only integers")))
    })
    .collect()
}

pub fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
  args
    .get(key)
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments(format!("`{key}` is required and must be a string")))
}

fn coerce_i64(value: Option<&Value>) -> Option<i64> {
  let value = value?;
  value
    .as_i64()
    .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
}

pub fn pagination(args: &Value) -> (i64, i64) {
  let page = args.get("page").and_then(Value::as_i64).unwrap_or(0).max(0);
  let limit = args
    .get("limit")
    .and_then(Value::as_i64)
    .unwrap_or(DEFAULT_LIMIT)
    .clamp(1, MAX_LIMIT);
  (page, limit)
}

/// Slices `rows` to the requested page and reports whether further pages remain, taking ownership of
/// the window so callers serialize without an extra clone.
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
  }
}
