//! Migration: add color and sort_order columns to the tags table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Tags;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(Table::alter().table(Tags::Table).drop_column(Tags::Color).to_owned())
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(Tags::Table)
          .drop_column(Tags::SortOrder)
          .to_owned(),
      )
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Tags::Table)
          .add_column(ColumnDef::new(Tags::Color).text().null())
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(Tags::Table)
          .add_column(ColumnDef::new(Tags::SortOrder).integer().not_null().default(0))
          .to_owned(),
      )
      .await
  }
}
