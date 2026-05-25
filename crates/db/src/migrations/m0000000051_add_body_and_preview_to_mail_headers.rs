//! Migration: add body and preview columns to mail_headers table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::MailHeaders;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(MailHeaders::Table)
          .drop_column(MailHeaders::Body)
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(MailHeaders::Table)
          .drop_column(MailHeaders::Preview)
          .to_owned(),
      )
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(MailHeaders::Table)
          .add_column(ColumnDef::new(MailHeaders::Body).text().null())
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(MailHeaders::Table)
          .add_column(ColumnDef::new(MailHeaders::Preview).text().null())
          .to_owned(),
      )
      .await
  }
}
