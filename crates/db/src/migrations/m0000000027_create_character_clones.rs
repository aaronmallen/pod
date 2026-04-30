//! Migration: create character_clones table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterClones;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterClones::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterClones::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterClones::Id)
              .big_integer()
              .not_null()
              .primary_key(),
          )
          .col(ColumnDef::new(CharacterClones::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(CharacterClones::IsActive).boolean().not_null())
          .col(ColumnDef::new(CharacterClones::Name).text().null())
          .col(ColumnDef::new(CharacterClones::StationName).text().not_null())
          .col(ColumnDef::new(CharacterClones::SystemId).integer().not_null())
          .col(ColumnDef::new(CharacterClones::RegionName).text().not_null())
          .col(ColumnDef::new(CharacterClones::LocationId).big_integer().not_null())
          .col(ColumnDef::new(CharacterClones::InstalledAt).text().null())
          .col(ColumnDef::new(CharacterClones::SyncedAt).timestamp().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_clones_character_id")
          .table(CharacterClones::Table)
          .col(CharacterClones::CharacterId)
          .to_owned(),
      )
      .await
  }
}
