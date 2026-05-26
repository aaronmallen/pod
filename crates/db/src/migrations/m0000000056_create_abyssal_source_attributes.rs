//! Migration: create `abyssal_source_attributes` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::AbyssalSourceAttributes;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(AbyssalSourceAttributes::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(AbyssalSourceAttributes::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(AbyssalSourceAttributes::SourceTypeId)
              .integer()
              .not_null(),
          )
          .col(ColumnDef::new(AbyssalSourceAttributes::AttrId).integer().not_null())
          .primary_key(
            Index::create()
              .col(AbyssalSourceAttributes::SourceTypeId)
              .col(AbyssalSourceAttributes::AttrId),
          )
          .to_owned(),
      )
      .await
  }
}
