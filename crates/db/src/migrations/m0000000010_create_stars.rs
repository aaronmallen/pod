//! Migration that creates the `stars` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{ItemTypes, SolarSystems, Stars};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Stars::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Stars::Table)
          .if_not_exists()
          .col(ColumnDef::new(Stars::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Stars::SolarSystemId).integer().not_null())
          .col(ColumnDef::new(Stars::ItemTypeId).integer().not_null())
          .col(ColumnDef::new(Stars::Name).string().not_null())
          .col(ColumnDef::new(Stars::SpectralClass).string().not_null())
          .col(ColumnDef::new(Stars::Age).big_integer().not_null())
          .col(ColumnDef::new(Stars::Luminosity).double().not_null())
          .col(ColumnDef::new(Stars::Radius).big_integer().not_null())
          .col(ColumnDef::new(Stars::Temperature).integer().not_null())
          // circular: stars.solar_system_id ↔ solar_systems.star_id — no cascade to avoid loops
          .foreign_key(
            ForeignKey::create()
              .name("fk_stars_solar_system_id")
              .from(Stars::Table, Stars::SolarSystemId)
              .to(SolarSystems::Table, SolarSystems::Id),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_stars_item_type_id")
              .from(Stars::Table, Stars::ItemTypeId)
              .to(ItemTypes::Table, ItemTypes::Id),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (Stars::SolarSystemId, "idx_stars_solar_system_id"),
      (Stars::ItemTypeId, "idx_stars_item_type_id"),
      (Stars::Name, "idx_stars_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(Stars::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
