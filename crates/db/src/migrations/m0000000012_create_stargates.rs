//! Migration that creates the `stargates` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{ItemTypes, SolarSystems, Stargates};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Stargates::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Stargates::Table)
          .if_not_exists()
          .col(ColumnDef::new(Stargates::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Stargates::SolarSystemId).integer().not_null())
          .col(ColumnDef::new(Stargates::ItemTypeId).integer().not_null())
          .col(ColumnDef::new(Stargates::DestinationStargateId).integer().not_null())
          .col(ColumnDef::new(Stargates::DestinationSolarSystemId).integer().not_null())
          .col(ColumnDef::new(Stargates::Name).string().not_null())
          .col(ColumnDef::new(Stargates::PositionX).double().not_null())
          .col(ColumnDef::new(Stargates::PositionY).double().not_null())
          .col(ColumnDef::new(Stargates::PositionZ).double().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_stargates_solar_system_id")
              .from(Stargates::Table, Stargates::SolarSystemId)
              .to(SolarSystems::Table, SolarSystems::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_stargates_item_type_id")
              .from(Stargates::Table, Stargates::ItemTypeId)
              .to(ItemTypes::Table, ItemTypes::Id),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_stargates_destination_stargate_id")
              .from(Stargates::Table, Stargates::DestinationStargateId)
              .to(Stargates::Table, Stargates::Id),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_stargates_destination_solar_system_id")
              .from(Stargates::Table, Stargates::DestinationSolarSystemId)
              .to(SolarSystems::Table, SolarSystems::Id),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (Stargates::SolarSystemId, "idx_stargates_solar_system_id"),
      (Stargates::ItemTypeId, "idx_stargates_item_type_id"),
      (Stargates::Name, "idx_stargates_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(Stargates::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
