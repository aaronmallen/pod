//! SeaORM entity for the entity_tags polymorphic join table.

use sea_orm::entity::prelude::*;

/// A row in the entity_tags table, associating any entity with a tag.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "entity_tags")]
pub struct Model {
  #[sea_orm(primary_key, auto_increment = false)]
  pub entity_id: i64,
  #[sea_orm(primary_key, auto_increment = false)]
  pub entity_type: String,
  #[sea_orm(primary_key, auto_increment = false)]
  pub tag_id: i32,
}

impl ActiveModelBehavior for ActiveModel {}
