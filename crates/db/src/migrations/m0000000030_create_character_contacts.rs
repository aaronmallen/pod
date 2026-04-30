//! Migration: create character_contacts table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterContacts;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterContacts::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterContacts::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterContacts::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(CharacterContacts::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(CharacterContacts::ContactId).integer().not_null())
          .col(ColumnDef::new(CharacterContacts::ContactName).text().not_null())
          .col(ColumnDef::new(CharacterContacts::ContactType).text().not_null())
          .col(ColumnDef::new(CharacterContacts::Standing).double().not_null())
          .col(ColumnDef::new(CharacterContacts::IsWatchlist).boolean().not_null())
          .col(ColumnDef::new(CharacterContacts::LabelIds).text().not_null())
          .col(ColumnDef::new(CharacterContacts::SyncedAt).timestamp().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_contacts_char_contact")
          .table(CharacterContacts::Table)
          .col(CharacterContacts::CharacterId)
          .col(CharacterContacts::ContactId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
