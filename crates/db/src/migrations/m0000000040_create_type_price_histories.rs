//! Migration: create type_price_histories table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::TypePriceHistories;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(TypePriceHistories::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(TypePriceHistories::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(TypePriceHistories::Id)
              .big_integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(TypePriceHistories::TypeId).integer().not_null())
          .col(ColumnDef::new(TypePriceHistories::Date).date().not_null())
          .col(ColumnDef::new(TypePriceHistories::Open).double().not_null())
          .col(ColumnDef::new(TypePriceHistories::High).double().not_null())
          .col(ColumnDef::new(TypePriceHistories::Low).double().not_null())
          .col(ColumnDef::new(TypePriceHistories::Close).double().not_null())
          .col(ColumnDef::new(TypePriceHistories::Avg).double().not_null())
          .col(ColumnDef::new(TypePriceHistories::SampleCount).integer().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name("udx_type_price_histories_type_id_date")
          .table(TypePriceHistories::Table)
          .col(TypePriceHistories::TypeId)
          .col(TypePriceHistories::Date)
          .unique()
          .to_owned(),
      )
      .await
  }
}
