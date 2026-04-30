//! Migration that creates the `factions` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{Factions, SolarSystems};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Factions::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Factions::Table)
          .if_not_exists()
          .col(ColumnDef::new(Factions::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Factions::SolarSystemId).integer().null())
          .col(ColumnDef::new(Factions::Name).string().not_null())
          .col(ColumnDef::new(Factions::Description).text().not_null())
          .col(ColumnDef::new(Factions::IsUnique).boolean().not_null())
          .col(ColumnDef::new(Factions::SizeFactor).double().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_factions_solar_system_id")
              .from(Factions::Table, Factions::SolarSystemId)
              .to(SolarSystems::Table, SolarSystems::Id),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (Factions::SolarSystemId, "idx_factions_solar_system_id"),
      (Factions::Name, "idx_factions_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(Factions::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
