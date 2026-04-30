//! Migration: create character_killmails table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterKillmails;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterKillmails::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterKillmails::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterKillmails::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(CharacterKillmails::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(CharacterKillmails::KillmailId).integer().not_null())
          .col(ColumnDef::new(CharacterKillmails::KillHash).text().not_null())
          .col(ColumnDef::new(CharacterKillmails::IsKill).boolean().not_null())
          .col(ColumnDef::new(CharacterKillmails::ShipTypeId).integer().not_null())
          .col(ColumnDef::new(CharacterKillmails::ShipName).text().not_null())
          .col(ColumnDef::new(CharacterKillmails::VictimName).text().not_null())
          .col(ColumnDef::new(CharacterKillmails::VictimCorpName).text().not_null())
          .col(ColumnDef::new(CharacterKillmails::SystemId).integer().not_null())
          .col(ColumnDef::new(CharacterKillmails::SystemName).text().not_null())
          .col(ColumnDef::new(CharacterKillmails::SystemSec).double().not_null())
          .col(ColumnDef::new(CharacterKillmails::ValueIsk).double().not_null())
          .col(ColumnDef::new(CharacterKillmails::AttackerCount).integer().not_null())
          .col(ColumnDef::new(CharacterKillmails::FinalBlow).boolean().not_null())
          .col(ColumnDef::new(CharacterKillmails::KillTime).text().not_null())
          .col(ColumnDef::new(CharacterKillmails::SyncedAt).timestamp().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_killmails_char_kill")
          .table(CharacterKillmails::Table)
          .col(CharacterKillmails::CharacterId)
          .col(CharacterKillmails::KillmailId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
