//! Migration that creates the `solar_systems` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{Constellations, SolarSystems, Stars};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(SolarSystems::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(SolarSystems::Table)
          .if_not_exists()
          .col(ColumnDef::new(SolarSystems::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(SolarSystems::ConstellationId).integer().not_null())
          .col(ColumnDef::new(SolarSystems::StarId).integer().null())
          .col(ColumnDef::new(SolarSystems::Name).string().not_null())
          .col(ColumnDef::new(SolarSystems::PositionX).double().not_null())
          .col(ColumnDef::new(SolarSystems::PositionY).double().not_null())
          .col(ColumnDef::new(SolarSystems::PositionZ).double().not_null())
          .col(ColumnDef::new(SolarSystems::SecurityClass).string().null())
          .col(ColumnDef::new(SolarSystems::SecurityStatus).double().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_solar_systems_constellation_id")
              .from(SolarSystems::Table, SolarSystems::ConstellationId)
              .to(Constellations::Table, Constellations::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          // circular: solar_systems.star_id ↔ stars.solar_system_id — no cascade to avoid loops
          .foreign_key(
            ForeignKey::create()
              .name("fk_solar_systems_star_id")
              .from(SolarSystems::Table, SolarSystems::StarId)
              .to(Stars::Table, Stars::Id),
          )
          .to_owned(),
      )
      .await?;

    for (col, name, unique) in [
      (
        SolarSystems::ConstellationId,
        "idx_solar_systems_constellation_id",
        false,
      ),
      (SolarSystems::Name, "udx_solar_systems_name", true),
    ] {
      let mut idx = Index::create()
        .if_not_exists()
        .name(name)
        .table(SolarSystems::Table)
        .col(col)
        .to_owned();

      if unique {
        idx.unique();
      }

      manager.create_index(idx).await?;
    }

    Ok(())
  }
}
