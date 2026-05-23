//! Migration: create corporation_assets table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CorporationAssets;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CorporationAssets::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CorporationAssets::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CorporationAssets::ItemId)
              .big_integer()
              .not_null()
              .primary_key(),
          )
          .col(
            ColumnDef::new(CorporationAssets::CorporationId)
              .big_integer()
              .not_null(),
          )
          .col(ColumnDef::new(CorporationAssets::TypeId).integer().not_null())
          .col(ColumnDef::new(CorporationAssets::LocationId).big_integer().not_null())
          .col(ColumnDef::new(CorporationAssets::LocationType).string().not_null())
          .col(ColumnDef::new(CorporationAssets::LocationFlag).string().not_null())
          .col(ColumnDef::new(CorporationAssets::IsBlueprintCopy).boolean().null())
          .col(ColumnDef::new(CorporationAssets::Quantity).integer().not_null())
          .col(ColumnDef::new(CorporationAssets::IsSingleton).boolean().not_null())
          .to_owned(),
      )
      .await
  }
}
