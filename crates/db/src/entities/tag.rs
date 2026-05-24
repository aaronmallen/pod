//! SeaORM entity for the tags table.

use sea_orm::entity::prelude::*;

/// A global tag that can be assigned to characters.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tags")]
pub struct Model {
  pub color: Option<String>,
  #[sea_orm(primary_key)]
  pub id: i32,
  pub name: String,
  pub sort_order: i32,
}

impl ActiveModelBehavior for ActiveModel {}
