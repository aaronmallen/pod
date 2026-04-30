//! Migration: create type_icons cache table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::TypeIcons;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(TypeIcons::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(TypeIcons::Table)
          .if_not_exists()
          .col(ColumnDef::new(TypeIcons::TypeId).integer().not_null())
          .col(ColumnDef::new(TypeIcons::Variant).string().not_null().default("icon"))
          .col(ColumnDef::new(TypeIcons::Data).blob().not_null())
          .primary_key(Index::create().col(TypeIcons::TypeId).col(TypeIcons::Variant))
          .to_owned(),
      )
      .await
  }
}
