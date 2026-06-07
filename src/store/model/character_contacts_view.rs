use crate::store::model::{CharacterContact, CharacterContactLabel};

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterContacts {
  pub contacts: Vec<CharacterContact>,
  pub labels: Vec<CharacterContactLabel>,
}
