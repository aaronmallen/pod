//! Migration: create wallet_journal_entries table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::WalletJournalEntries;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(WalletJournalEntries::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(WalletJournalEntries::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(WalletJournalEntries::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(
            ColumnDef::new(WalletJournalEntries::CharacterId)
              .big_integer()
              .not_null(),
          )
          .col(ColumnDef::new(WalletJournalEntries::EntryId).big_integer().not_null())
          .col(ColumnDef::new(WalletJournalEntries::RefType).text().not_null())
          .col(ColumnDef::new(WalletJournalEntries::Amount).double().null())
          .col(ColumnDef::new(WalletJournalEntries::Balance).double().null())
          .col(ColumnDef::new(WalletJournalEntries::Date).text().not_null())
          .col(ColumnDef::new(WalletJournalEntries::Description).text().not_null())
          .col(ColumnDef::new(WalletJournalEntries::FirstPartyId).big_integer().null())
          .col(ColumnDef::new(WalletJournalEntries::SecondPartyId).big_integer().null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_wallet_journal_char_entry")
          .table(WalletJournalEntries::Table)
          .col(WalletJournalEntries::CharacterId)
          .col(WalletJournalEntries::EntryId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
