//! Migration: create stockpile_items table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{StockpileItems, Stockpiles};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(StockpileItems::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(StockpileItems::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(StockpileItems::Id)
              .big_integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(StockpileItems::StockpileId).big_integer().not_null())
          .col(ColumnDef::new(StockpileItems::TypeId).integer().not_null())
          .col(ColumnDef::new(StockpileItems::TargetQuantity).integer().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_stockpile_items_stockpile_id")
              .from(StockpileItems::Table, StockpileItems::StockpileId)
              .to(Stockpiles::Table, Stockpiles::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_stockpile_items_stockpile_id")
          .table(StockpileItems::Table)
          .col(StockpileItems::StockpileId)
          .to_owned(),
      )
      .await
  }
}
