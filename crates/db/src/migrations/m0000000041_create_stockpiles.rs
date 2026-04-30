//! Migration: create stockpiles table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Stockpiles;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(Stockpiles::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Stockpiles::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(Stockpiles::Id)
              .big_integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(Stockpiles::Name).text().not_null())
          .col(ColumnDef::new(Stockpiles::LocationId).big_integer())
          .col(ColumnDef::new(Stockpiles::CharacterId).big_integer())
          .to_owned(),
      )
      .await
  }
}
