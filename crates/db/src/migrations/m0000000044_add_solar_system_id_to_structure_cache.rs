//! Migration: add solar_system_id column to structure_cache table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::StructureCache;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(StructureCache::Table)
          .drop_column(StructureCache::SolarSystemId)
          .to_owned(),
      )
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(StructureCache::Table)
          .add_column(ColumnDef::new(StructureCache::SolarSystemId).big_integer().null())
          .to_owned(),
      )
      .await
  }
}
