//! Migration: create polymorphic entity_tags join table.
//!
//! Replaces the old character_tags table with a single table that
//! can associate tags with any entity type via (entity_id, entity_type).

use sea_orm_migration::prelude::*;

use crate::schema::iden::EntityTags;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(EntityTags::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(EntityTags::Table)
          .if_not_exists()
          .col(ColumnDef::new(EntityTags::EntityId).big_integer().not_null())
          .col(ColumnDef::new(EntityTags::EntityType).text().not_null())
          .col(ColumnDef::new(EntityTags::TagId).integer().not_null())
          .primary_key(
            Index::create()
              .col(EntityTags::EntityId)
              .col(EntityTags::EntityType)
              .col(EntityTags::TagId),
          )
          .to_owned(),
      )
      .await
  }
}
