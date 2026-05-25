//! Domain model for a dogma attribute definition.

use getset::Getters;
use serde::{Deserialize, Serialize};

/// A dogma attribute definition from the SDE, including display metadata.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize)]
pub struct DogmaAttr {
  /// The EVE dogma attribute identifier.
  #[get = "pub"]
  attribute_id: i32,
  /// Default value for this attribute when not overridden on an item type.
  #[get = "pub"]
  default_value: Option<f64>,
  /// Long-form description of what this attribute represents.
  #[get = "pub"]
  description: Option<String>,
  /// Localized display name for this attribute (English).
  #[get = "pub"]
  display_name: Option<String>,
  /// Whether a higher value is generally better for this attribute.
  #[get = "pub"]
  high_is_good: bool,
  /// EVE icon ID for the attribute, used to fetch the attribute icon image.
  #[get = "pub"]
  icon_id: Option<i32>,
  /// Internal attribute name (non-localized EVE identifier).
  #[get = "pub"]
  name: String,
  /// Whether this attribute is visible in the public game client.
  #[get = "pub"]
  published: bool,
  /// Whether this attribute stacks (is affected by stacking penalties).
  #[get = "pub"]
  stackable: bool,
  /// EVE unit ID for formatting values (e.g. 114 = GJ, 71 = m³).
  #[get = "pub"]
  unit_id: Option<i32>,
}

impl DogmaAttr {
  /// Creates a new `DogmaAttr` with required fields; all optional fields are
  /// `None` / `false` by default.
  pub fn new(attribute_id: i32, name: impl Into<String>) -> Self {
    Self {
      attribute_id,
      default_value: None,
      description: None,
      display_name: None,
      high_is_good: false,
      icon_id: None,
      name: name.into(),
      published: false,
      stackable: true,
      unit_id: None,
    }
  }

  /// Sets the default value.
  pub fn set_default_value(&mut self, v: Option<f64>) -> &mut Self {
    self.default_value = v;
    self
  }

  /// Sets the long-form description.
  pub fn set_description(&mut self, v: Option<String>) -> &mut Self {
    self.description = v;
    self
  }

  /// Sets the localized display name.
  pub fn set_display_name(&mut self, v: Option<String>) -> &mut Self {
    self.display_name = v;
    self
  }

  /// Sets whether a higher value is better.
  pub fn set_high_is_good(&mut self, v: bool) -> &mut Self {
    self.high_is_good = v;
    self
  }

  /// Sets the icon ID.
  pub fn set_icon_id(&mut self, v: Option<i32>) -> &mut Self {
    self.icon_id = v;
    self
  }

  /// Sets the published flag.
  pub fn set_published(&mut self, v: bool) -> &mut Self {
    self.published = v;
    self
  }

  /// Sets whether this attribute is stackable.
  pub fn set_stackable(&mut self, v: bool) -> &mut Self {
    self.stackable = v;
    self
  }

  /// Sets the unit ID.
  pub fn set_unit_id(&mut self, v: Option<i32>) -> &mut Self {
    self.unit_id = v;
    self
  }
}
