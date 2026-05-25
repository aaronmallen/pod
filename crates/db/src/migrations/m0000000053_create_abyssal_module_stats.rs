//! Migration: create `abyssal_module_stats` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::AbyssalModuleStats;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(AbyssalModuleStats::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(AbyssalModuleStats::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(AbyssalModuleStats::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(AbyssalModuleStats::AbyssalTypeId).integer().not_null())
          .col(ColumnDef::new(AbyssalModuleStats::AttributeId).integer().not_null())
          .col(ColumnDef::new(AbyssalModuleStats::MaxMult).double().not_null())
          .col(ColumnDef::new(AbyssalModuleStats::MinMult).double().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name("idx_abyssal_module_stats_abyssal_type_id")
          .table(AbyssalModuleStats::Table)
          .col(AbyssalModuleStats::AbyssalTypeId)
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .unique()
          .name("uq_abyssal_module_stats_type_attr")
          .table(AbyssalModuleStats::Table)
          .col(AbyssalModuleStats::AbyssalTypeId)
          .col(AbyssalModuleStats::AttributeId)
          .to_owned(),
      )
      .await
  }
}
