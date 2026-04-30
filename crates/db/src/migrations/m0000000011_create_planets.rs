//! Migration that creates the `planets` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{ItemTypes, Planets, SolarSystems};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Planets::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Planets::Table)
          .if_not_exists()
          .col(ColumnDef::new(Planets::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Planets::SolarSystemId).integer().not_null())
          .col(ColumnDef::new(Planets::ItemTypeId).integer().not_null())
          .col(ColumnDef::new(Planets::Name).string().not_null())
          .col(ColumnDef::new(Planets::PositionX).double().not_null())
          .col(ColumnDef::new(Planets::PositionY).double().not_null())
          .col(ColumnDef::new(Planets::PositionZ).double().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_planets_solar_system_id")
              .from(Planets::Table, Planets::SolarSystemId)
              .to(SolarSystems::Table, SolarSystems::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_planets_item_type_id")
              .from(Planets::Table, Planets::ItemTypeId)
              .to(ItemTypes::Table, ItemTypes::Id),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (Planets::SolarSystemId, "idx_planets_solar_system_id"),
      (Planets::ItemTypeId, "idx_planets_item_type_id"),
      (Planets::Name, "idx_planets_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(Planets::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
