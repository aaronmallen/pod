//! Migration: create character_standings table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterStandings;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterStandings::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterStandings::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterStandings::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(CharacterStandings::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(CharacterStandings::FromId).integer().not_null())
          .col(ColumnDef::new(CharacterStandings::FromType).text().not_null())
          .col(ColumnDef::new(CharacterStandings::FromName).text().not_null())
          .col(ColumnDef::new(CharacterStandings::RawStanding).double().not_null())
          .col(
            ColumnDef::new(CharacterStandings::EffectiveStanding)
              .double()
              .not_null(),
          )
          .col(ColumnDef::new(CharacterStandings::SyncedAt).timestamp().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_standings_char_from")
          .table(CharacterStandings::Table)
          .col(CharacterStandings::CharacterId)
          .col(CharacterStandings::FromId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
