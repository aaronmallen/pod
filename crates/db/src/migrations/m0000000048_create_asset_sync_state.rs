//! Migration: create asset_sync_state table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::AssetSyncState;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(AssetSyncState::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(AssetSyncState::Table)
          .if_not_exists()
          .col(ColumnDef::new(AssetSyncState::OwnerType).string().not_null())
          .col(ColumnDef::new(AssetSyncState::OwnerId).big_integer().not_null())
          .col(ColumnDef::new(AssetSyncState::LastSyncedAt).big_integer().null())
          .col(ColumnDef::new(AssetSyncState::CacheExpiresAt).big_integer().null())
          .primary_key(
            Index::create()
              .col(AssetSyncState::OwnerType)
              .col(AssetSyncState::OwnerId),
          )
          .to_owned(),
      )
      .await
  }
}
