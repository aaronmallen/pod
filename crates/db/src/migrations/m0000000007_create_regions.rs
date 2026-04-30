//! Migration that creates the `regions` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Regions;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Regions::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Regions::Table)
          .if_not_exists()
          .col(ColumnDef::new(Regions::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Regions::Name).string().not_null())
          .col(ColumnDef::new(Regions::Description).text().null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name("udx_regions_name")
          .table(Regions::Table)
          .col(Regions::Name)
          .unique()
          .to_owned(),
      )
      .await
  }
}
