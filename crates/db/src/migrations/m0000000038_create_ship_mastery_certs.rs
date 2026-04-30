//! Migration: create ship_mastery_certs table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::ShipMasteryCerts;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(ShipMasteryCerts::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(ShipMasteryCerts::Table)
          .if_not_exists()
          .col(ColumnDef::new(ShipMasteryCerts::ShipId).integer().not_null())
          .col(ColumnDef::new(ShipMasteryCerts::MasteryLevel).integer().not_null())
          .col(
            ColumnDef::new(ShipMasteryCerts::CertIdsJson)
              .text()
              .not_null()
              .default("[]"),
          )
          .primary_key(
            Index::create()
              .col(ShipMasteryCerts::ShipId)
              .col(ShipMasteryCerts::MasteryLevel),
          )
          .to_owned(),
      )
      .await
  }
}
