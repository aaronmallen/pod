//! Migration: create dogma_attributes table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::DogmaAttributes;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(DogmaAttributes::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(DogmaAttributes::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(DogmaAttributes::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(
            ColumnDef::new(DogmaAttributes::AttributeId)
              .integer()
              .not_null()
              .unique_key(),
          )
          .col(ColumnDef::new(DogmaAttributes::DefaultValue).double().null())
          .col(ColumnDef::new(DogmaAttributes::Description).text().null())
          .col(ColumnDef::new(DogmaAttributes::DisplayName).string().null())
          .col(ColumnDef::new(DogmaAttributes::HighIsGood).boolean().not_null())
          .col(ColumnDef::new(DogmaAttributes::IconId).integer().null())
          .col(ColumnDef::new(DogmaAttributes::Name).string().not_null())
          .col(ColumnDef::new(DogmaAttributes::Published).boolean().not_null())
          .col(ColumnDef::new(DogmaAttributes::Stackable).boolean().not_null())
          .col(ColumnDef::new(DogmaAttributes::UnitId).integer().null())
          .to_owned(),
      )
      .await
  }
}
