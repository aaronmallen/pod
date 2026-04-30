//! Migration: create mail_headers table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::MailHeaders;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(MailHeaders::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(MailHeaders::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(MailHeaders::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(MailHeaders::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(MailHeaders::MailId).big_integer().not_null())
          .col(ColumnDef::new(MailHeaders::Subject).text().not_null().default(""))
          .col(ColumnDef::new(MailHeaders::FromId).big_integer().null())
          .col(ColumnDef::new(MailHeaders::IsRead).boolean().not_null().default(false))
          .col(ColumnDef::new(MailHeaders::Timestamp).text().not_null().default(""))
          .col(
            ColumnDef::new(MailHeaders::RecipientsDisplay)
              .text()
              .not_null()
              .default(""),
          )
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_mail_headers_char_mail")
          .table(MailHeaders::Table)
          .col(MailHeaders::CharacterId)
          .col(MailHeaders::MailId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
