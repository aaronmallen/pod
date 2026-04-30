//! Migration: create characters table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Characters;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(Characters::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Characters::Table)
          .if_not_exists()
          .col(ColumnDef::new(Characters::Id).big_integer().not_null().primary_key())
          .col(ColumnDef::new(Characters::Name).string().not_null())
          .col(ColumnDef::new(Characters::CorpId).big_integer().not_null())
          .col(ColumnDef::new(Characters::CorpName).string().not_null())
          .col(ColumnDef::new(Characters::PortraitTone).integer().not_null())
          .col(ColumnDef::new(Characters::AccessToken).text().not_null())
          .col(ColumnDef::new(Characters::RefreshToken).text().not_null())
          .col(ColumnDef::new(Characters::TokenExpiresAt).big_integer().not_null())
          .col(ColumnDef::new(Characters::IskBalance).double().null())
          .col(ColumnDef::new(Characters::LocationName).string().null())
          .col(ColumnDef::new(Characters::LocationDocked).boolean().null())
          .col(ColumnDef::new(Characters::SortOrder).integer().not_null().default(0))
          .col(ColumnDef::new(Characters::Charisma).integer().null())
          .col(ColumnDef::new(Characters::Intelligence).integer().null())
          .col(ColumnDef::new(Characters::Memory).integer().null())
          .col(ColumnDef::new(Characters::Perception).integer().null())
          .col(ColumnDef::new(Characters::Willpower).integer().null())
          .to_owned(),
      )
      .await
  }
}
