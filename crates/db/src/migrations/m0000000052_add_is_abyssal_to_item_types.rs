//! Migration: add `is_abyssal` column to `item_types`.

use sea_orm_migration::prelude::*;

use crate::schema::iden::ItemTypes;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(ItemTypes::Table)
          .drop_column(ItemTypes::IsAbyssal)
          .to_owned(),
      )
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(ItemTypes::Table)
          .add_column(ColumnDef::new(ItemTypes::IsAbyssal).boolean().not_null().default(false))
          .to_owned(),
      )
      .await
  }
}
