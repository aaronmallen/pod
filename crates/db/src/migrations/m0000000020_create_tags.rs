//! Migration: create tags table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Tags;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.drop_table(Table::drop().table(Tags::Table).to_owned()).await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Tags::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(Tags::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(Tags::Name).text().not_null().unique_key())
          .to_owned(),
      )
      .await
  }
}
