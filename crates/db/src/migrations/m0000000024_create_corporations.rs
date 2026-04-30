//! Migration: create corporations table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Corporations;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(Corporations::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Corporations::Table)
          .if_not_exists()
          .col(ColumnDef::new(Corporations::Id).big_integer().not_null().primary_key())
          .col(ColumnDef::new(Corporations::AccessToken).text().not_null().default(""))
          .col(ColumnDef::new(Corporations::AllianceId).big_integer().null())
          .col(ColumnDef::new(Corporations::AllianceName).text().null())
          .col(
            ColumnDef::new(Corporations::AuthCharacterId)
              .big_integer()
              .not_null()
              .default(0),
          )
          .col(
            ColumnDef::new(Corporations::CeoCharacterId)
              .big_integer()
              .not_null()
              .default(0),
          )
          .col(ColumnDef::new(Corporations::DateFounded).text().null())
          .col(ColumnDef::new(Corporations::Description).text().null())
          .col(ColumnDef::new(Corporations::FactionId).big_integer().null())
          .col(ColumnDef::new(Corporations::HomeStationId).big_integer().null())
          .col(ColumnDef::new(Corporations::IconData).blob().null())
          .col(
            ColumnDef::new(Corporations::MemberCount)
              .integer()
              .not_null()
              .default(0),
          )
          .col(ColumnDef::new(Corporations::Name).text().not_null().default(""))
          .col(ColumnDef::new(Corporations::RefreshToken).text().not_null().default(""))
          .col(ColumnDef::new(Corporations::Scopes).text().not_null().default("[]"))
          .col(ColumnDef::new(Corporations::Shares).big_integer().null())
          .col(ColumnDef::new(Corporations::TaxRate).double().not_null().default(0.0))
          .col(ColumnDef::new(Corporations::Ticker).text().not_null().default(""))
          .col(
            ColumnDef::new(Corporations::TokenExpiresAt)
              .big_integer()
              .not_null()
              .default(0),
          )
          .col(ColumnDef::new(Corporations::Url).text().null())
          .col(ColumnDef::new(Corporations::WarEligible).boolean().null())
          .to_owned(),
      )
      .await
  }
}
