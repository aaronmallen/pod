//! Migration: create `abyssal_source_types` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::AbyssalSourceTypes;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(AbyssalSourceTypes::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(AbyssalSourceTypes::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(AbyssalSourceTypes::SourceTypeId)
              .integer()
              .not_null()
              .primary_key(),
          )
          .to_owned(),
      )
      .await
  }
}
