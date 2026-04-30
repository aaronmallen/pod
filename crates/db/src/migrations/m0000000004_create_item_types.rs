//! Migration that creates the `item_types` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{ItemGroups, ItemTypes, MarketGroups};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(ItemTypes::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(ItemTypes::Table)
          .if_not_exists()
          .col(ColumnDef::new(ItemTypes::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(ItemTypes::Name).string().not_null())
          .col(ColumnDef::new(ItemTypes::Description).text().not_null())
          .col(ColumnDef::new(ItemTypes::ItemGroupId).integer().not_null())
          .col(ColumnDef::new(ItemTypes::MarketGroupId).integer().null())
          .col(ColumnDef::new(ItemTypes::GraphicId).integer().null())
          .col(ColumnDef::new(ItemTypes::IconId).integer().null())
          .col(
            ColumnDef::new(ItemTypes::DogmaAttributes)
              .text()
              .not_null()
              .default("[]"),
          )
          .col(ColumnDef::new(ItemTypes::DogmaEffects).text().not_null().default("[]"))
          .col(ColumnDef::new(ItemTypes::Capacity).double().null())
          .col(ColumnDef::new(ItemTypes::Mass).double().null())
          .col(ColumnDef::new(ItemTypes::PackagedVolume).double().null())
          .col(ColumnDef::new(ItemTypes::PortionSize).integer().null())
          .col(ColumnDef::new(ItemTypes::Radius).double().null())
          .col(ColumnDef::new(ItemTypes::Volume).double().null())
          .col(ColumnDef::new(ItemTypes::Published).boolean().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_item_types_item_group_id")
              .from(ItemTypes::Table, ItemTypes::ItemGroupId)
              .to(ItemGroups::Table, ItemGroups::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_item_types_market_group_id")
              .from(ItemTypes::Table, ItemTypes::MarketGroupId)
              .to(MarketGroups::Table, MarketGroups::Id)
              .on_delete(ForeignKeyAction::SetNull),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (ItemTypes::ItemGroupId, "idx_item_types_item_group_id"),
      (ItemTypes::MarketGroupId, "idx_item_types_market_group_id"),
      (ItemTypes::Name, "idx_item_types_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(ItemTypes::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
