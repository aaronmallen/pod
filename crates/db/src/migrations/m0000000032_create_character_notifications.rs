//! Migration: create character_notifications table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterNotifications;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterNotifications::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterNotifications::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterNotifications::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(
            ColumnDef::new(CharacterNotifications::CharacterId)
              .big_integer()
              .not_null(),
          )
          .col(
            ColumnDef::new(CharacterNotifications::NotificationId)
              .integer()
              .not_null(),
          )
          .col(ColumnDef::new(CharacterNotifications::NotifType).text().not_null())
          .col(ColumnDef::new(CharacterNotifications::SenderId).integer().null())
          .col(ColumnDef::new(CharacterNotifications::SenderType).text().null())
          .col(ColumnDef::new(CharacterNotifications::Timestamp).text().not_null())
          .col(ColumnDef::new(CharacterNotifications::IsRead).boolean().not_null())
          .col(ColumnDef::new(CharacterNotifications::Text).text().null())
          .col(ColumnDef::new(CharacterNotifications::SyncedAt).timestamp().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_notifications_char_notif")
          .table(CharacterNotifications::Table)
          .col(CharacterNotifications::CharacterId)
          .col(CharacterNotifications::NotificationId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
