use crate::store::model::{CharacterBlueprint, CorporationBlueprint};

// Blueprint storage aggregate; consumed by the industry sync/UI once it lands. Exercised only by unit tests
// until then.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
  pub character_blueprints: Vec<CharacterBlueprint>,
  pub corporation_blueprints: Vec<CorporationBlueprint>,
}
