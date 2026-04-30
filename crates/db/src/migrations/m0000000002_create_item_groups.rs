//! Migration that creates the `item_groups` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{ItemCategories, ItemGroups};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(ItemGroups::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(ItemGroups::Table)
          .if_not_exists()
          .col(ColumnDef::new(ItemGroups::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(ItemGroups::ItemCategoryId).integer().not_null())
          .col(ColumnDef::new(ItemGroups::Name).string().not_null())
          .col(ColumnDef::new(ItemGroups::Published).boolean().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_item_groups_item_category_id")
              .from(ItemGroups::Table, ItemGroups::ItemCategoryId)
              .to(ItemCategories::Table, ItemCategories::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (ItemGroups::ItemCategoryId, "idx_item_groups_item_category_id"),
      (ItemGroups::Name, "idx_item_groups_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(ItemGroups::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
