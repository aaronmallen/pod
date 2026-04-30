//! Migration: create type_prices table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::TypePrices;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(TypePrices::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(TypePrices::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(TypePrices::Id)
              .big_integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(TypePrices::TypeId).integer().not_null())
          .col(ColumnDef::new(TypePrices::Price).double().not_null())
          .col(ColumnDef::new(TypePrices::FetchedAt).timestamp().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name("idx_type_prices_type_id")
          .table(TypePrices::Table)
          .col(TypePrices::TypeId)
          .to_owned(),
      )
      .await
  }
}
