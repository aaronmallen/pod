//! Migration: add adjusted_price column to type_prices table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::TypePrices;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(TypePrices::Table)
          .drop_column(TypePrices::AdjustedPrice)
          .to_owned(),
      )
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(TypePrices::Table)
          .add_column(ColumnDef::new(TypePrices::AdjustedPrice).double().null())
          .to_owned(),
      )
      .await
  }
}
