//! Domain models for the character contact list.

use getset::Getters;

/// The EVE entity type for a contact entry.
#[derive(Clone, Debug, PartialEq)]
pub enum ContactType {
  /// An EVE player alliance.
  Alliance,
  /// An EVE character.
  Character,
  /// An EVE corporation (player or NPC).
  Corp,
  /// An EVE NPC faction.
  Faction,
}

/// A label used to categorize contacts.
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct ContactLabel {
  /// Unique identifier for this label.
  #[get = "pub"]
  label_id: i32,
  /// Display name of the label.
  #[get = "pub"]
  label_name: String,
}

impl ContactLabel {
  /// Creates a new contact label.
  pub fn new(label_id: i32, label_name: impl Into<String>) -> Self {
    Self {
      label_id,
      label_name: label_name.into(),
    }
  }

  /// Sets the label ID.
  pub fn set_label_id(&mut self, label_id: i32) -> &mut Self {
    self.label_id = label_id;
    self
  }

  /// Sets the label name.
  pub fn set_label_name(&mut self, label_name: impl Into<String>) -> &mut Self {
    self.label_name = label_name.into();
    self
  }
}

/// A single entry in a character's contact list.
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct Contact {
  /// Unique EVE entity ID for this contact.
  #[get = "pub"]
  contact_id: i32,
  /// The entity category of this contact.
  #[get = "pub"]
  contact_type: ContactType,
  /// Whether the contact is on the character's watchlist.
  #[get = "pub"]
  is_watchlist: bool,
  /// IDs of labels applied to this contact.
  #[get = "pub"]
  labels: Vec<i32>,
  /// Display name of the contact.
  #[get = "pub"]
  name: String,
  /// Standing value toward this contact, in the range [-10.0, 10.0].
  #[get = "pub"]
  standing: f32,
}

impl Contact {
  /// Creates a new contact entry.
  pub fn new(contact_id: i32, name: impl Into<String>, contact_type: ContactType, standing: f32) -> Self {
    Self {
      contact_id,
      contact_type,
      is_watchlist: false,
      labels: Vec::new(),
      name: name.into(),
      standing,
    }
  }

  /// Sets the contact ID.
  pub fn set_contact_id(&mut self, contact_id: i32) -> &mut Self {
    self.contact_id = contact_id;
    self
  }

  /// Sets the contact type.
  pub fn set_contact_type(&mut self, contact_type: ContactType) -> &mut Self {
    self.contact_type = contact_type;
    self
  }

  /// Sets whether the contact is on the watchlist.
  pub fn set_is_watchlist(&mut self, is_watchlist: bool) -> &mut Self {
    self.is_watchlist = is_watchlist;
    self
  }

  /// Sets the label IDs applied to this contact.
  pub fn set_labels(&mut self, labels: Vec<i32>) -> &mut Self {
    self.labels = labels;
    self
  }

  /// Sets the contact name.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    self
  }

  /// Sets the standing value.
  pub fn set_standing(&mut self, standing: f32) -> &mut Self {
    self.standing = standing;
    self
  }
}
