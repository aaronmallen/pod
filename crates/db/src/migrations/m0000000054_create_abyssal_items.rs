//! Migration: create `abyssal_items` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::AbyssalItems;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(AbyssalItems::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(AbyssalItems::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(AbyssalItems::ItemId)
              .big_integer()
              .not_null()
              .primary_key(),
          )
          .col(ColumnDef::new(AbyssalItems::CharacterId).big_integer().not_null())
          .col(
            ColumnDef::new(AbyssalItems::DogmaAttributes)
              .text()
              .not_null()
              .default("[]"),
          )
          .col(ColumnDef::new(AbyssalItems::MutaPriceIsk).double().null())
          .col(ColumnDef::new(AbyssalItems::MutaPriceSynced).big_integer().null())
          .col(ColumnDef::new(AbyssalItems::MutatorTypeId).integer().not_null())
          .col(ColumnDef::new(AbyssalItems::SourceTypeId).integer().not_null())
          .col(ColumnDef::new(AbyssalItems::SyncedAt).big_integer().not_null())
          .col(ColumnDef::new(AbyssalItems::TypeId).integer().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name("idx_abyssal_items_character_id")
          .table(AbyssalItems::Table)
          .col(AbyssalItems::CharacterId)
          .to_owned(),
      )
      .await
  }
}
