//! Migration that creates the `races` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::Races;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Races::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Races::Table)
          .if_not_exists()
          .col(ColumnDef::new(Races::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Races::AllianceId).integer().not_null())
          .col(ColumnDef::new(Races::Name).string().not_null())
          .col(ColumnDef::new(Races::Description).text().not_null())
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (Races::AllianceId, "idx_races_alliance_id"),
      (Races::Name, "idx_races_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(Races::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
