//! Migration: create structure_cache table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::StructureCache;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(StructureCache::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(StructureCache::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(StructureCache::Id)
              .big_integer()
              .not_null()
              .primary_key(),
          )
          .col(ColumnDef::new(StructureCache::Name).text().not_null())
          .to_owned(),
      )
      .await
  }
}
