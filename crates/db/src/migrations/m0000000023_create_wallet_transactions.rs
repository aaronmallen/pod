//! Migration: create wallet_transactions table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::WalletTransactions;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(WalletTransactions::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(WalletTransactions::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(WalletTransactions::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(WalletTransactions::CharacterId).big_integer().not_null())
          .col(
            ColumnDef::new(WalletTransactions::TransactionId)
              .big_integer()
              .not_null(),
          )
          .col(ColumnDef::new(WalletTransactions::TypeId).integer().not_null())
          .col(ColumnDef::new(WalletTransactions::Quantity).integer().not_null())
          .col(ColumnDef::new(WalletTransactions::UnitPrice).double().not_null())
          .col(ColumnDef::new(WalletTransactions::IsBuy).boolean().not_null())
          .col(ColumnDef::new(WalletTransactions::Date).text().not_null())
          .col(ColumnDef::new(WalletTransactions::LocationId).big_integer().not_null())
          .col(ColumnDef::new(WalletTransactions::ClientId).big_integer().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_wallet_transactions_char_txn")
          .table(WalletTransactions::Table)
          .col(WalletTransactions::CharacterId)
          .col(WalletTransactions::TransactionId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
