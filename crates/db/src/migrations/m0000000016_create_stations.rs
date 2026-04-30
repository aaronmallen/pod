//! Migration that creates the `stations` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{ItemTypes, Races, SolarSystems, Stations};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Stations::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Stations::Table)
          .if_not_exists()
          .col(ColumnDef::new(Stations::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Stations::SolarSystemId).integer().not_null())
          .col(ColumnDef::new(Stations::ItemTypeId).integer().not_null())
          .col(ColumnDef::new(Stations::OwnerId).integer().null())
          .col(ColumnDef::new(Stations::RaceId).integer().null())
          .col(ColumnDef::new(Stations::Name).string().not_null())
          .col(ColumnDef::new(Stations::MaxDockableShipVolume).double().not_null())
          .col(ColumnDef::new(Stations::OfficeRentalCost).double().not_null())
          .col(ColumnDef::new(Stations::ReprocessingEfficiency).double().not_null())
          .col(ColumnDef::new(Stations::ReprocessingStationsTake).double().not_null())
          .col(ColumnDef::new(Stations::Services).text().not_null().default("[]"))
          .col(ColumnDef::new(Stations::PositionX).double().not_null())
          .col(ColumnDef::new(Stations::PositionY).double().not_null())
          .col(ColumnDef::new(Stations::PositionZ).double().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_stations_solar_system_id")
              .from(Stations::Table, Stations::SolarSystemId)
              .to(SolarSystems::Table, SolarSystems::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_stations_item_type_id")
              .from(Stations::Table, Stations::ItemTypeId)
              .to(ItemTypes::Table, ItemTypes::Id),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_stations_race_id")
              .from(Stations::Table, Stations::RaceId)
              .to(Races::Table, Races::Id),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (Stations::SolarSystemId, "idx_stations_solar_system_id"),
      (Stations::ItemTypeId, "idx_stations_item_type_id"),
      (Stations::Name, "idx_stations_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(Stations::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
