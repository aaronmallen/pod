//! Migration: add granted_scopes column to characters table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Characters;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Characters::Table)
          .drop_column(Characters::GrantedScopes)
          .to_owned(),
      )
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Characters::Table)
          .add_column(ColumnDef::new(Characters::GrantedScopes).text().null())
          .to_owned(),
      )
      .await
  }
}
