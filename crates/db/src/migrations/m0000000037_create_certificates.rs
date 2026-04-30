//! Migration: create certificates table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Certificates;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(Certificates::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Certificates::Table)
          .if_not_exists()
          .col(ColumnDef::new(Certificates::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Certificates::Name).text().not_null())
          .col(ColumnDef::new(Certificates::Description).text())
          .col(ColumnDef::new(Certificates::Grade).integer().not_null().default(0))
          .col(ColumnDef::new(Certificates::SkillsJson).text().not_null().default("[]"))
          .to_owned(),
      )
      .await
  }
}
