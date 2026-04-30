//! Migration: create snoozed_mails table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::SnoozedMails;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(SnoozedMails::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(SnoozedMails::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(SnoozedMails::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(SnoozedMails::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(SnoozedMails::MailId).big_integer().not_null())
          .col(ColumnDef::new(SnoozedMails::SnoozeUntil).text().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_snoozed_mails_char_mail")
          .table(SnoozedMails::Table)
          .col(SnoozedMails::CharacterId)
          .col(SnoozedMails::MailId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
