//! Migration that creates the `item_categories` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::ItemCategories;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(ItemCategories::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(ItemCategories::Table)
          .if_not_exists()
          .col(ColumnDef::new(ItemCategories::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(ItemCategories::Name).string().not_null())
          .col(ColumnDef::new(ItemCategories::Published).boolean().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name("idx_item_categories_name")
          .table(ItemCategories::Table)
          .col(ItemCategories::Name)
          .to_owned(),
      )
      .await
  }
}
