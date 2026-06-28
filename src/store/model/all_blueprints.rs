use crate::store::model::{CharacterBlueprint, CorporationBlueprint};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
  pub character_blueprints: Vec<CharacterBlueprint>,
  pub corporation_blueprints: Vec<CorporationBlueprint>,
}
