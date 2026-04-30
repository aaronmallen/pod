//! Migration: create character_assets table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterAssets;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterAssets::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterAssets::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterAssets::ItemId)
              .big_integer()
              .not_null()
              .primary_key(),
          )
          .col(ColumnDef::new(CharacterAssets::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(CharacterAssets::TypeId).integer().not_null())
          .col(ColumnDef::new(CharacterAssets::LocationId).big_integer().not_null())
          .col(ColumnDef::new(CharacterAssets::LocationType).string().not_null())
          .col(ColumnDef::new(CharacterAssets::LocationFlag).string().not_null())
          .col(ColumnDef::new(CharacterAssets::IsBlueprintCopy).boolean().null())
          .col(ColumnDef::new(CharacterAssets::Quantity).integer().not_null())
          .col(ColumnDef::new(CharacterAssets::IsSingleton).boolean().not_null())
          .to_owned(),
      )
      .await
  }
}
