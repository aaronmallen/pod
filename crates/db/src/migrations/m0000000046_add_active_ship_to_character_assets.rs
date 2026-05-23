//! Migration: add is_active_ship and ship_name columns to character_assets.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterAssets;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(CharacterAssets::Table)
          .drop_column(CharacterAssets::IsActiveShip)
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(CharacterAssets::Table)
          .drop_column(CharacterAssets::ShipName)
          .to_owned(),
      )
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(CharacterAssets::Table)
          .add_column(
            ColumnDef::new(CharacterAssets::IsActiveShip)
              .boolean()
              .not_null()
              .default(false),
          )
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(CharacterAssets::Table)
          .add_column(ColumnDef::new(CharacterAssets::ShipName).text().null())
          .to_owned(),
      )
      .await
  }
}
