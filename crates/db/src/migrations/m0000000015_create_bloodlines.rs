//! Migration that creates the `bloodlines` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{Bloodlines, ItemTypes, Races};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Bloodlines::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Bloodlines::Table)
          .if_not_exists()
          .col(ColumnDef::new(Bloodlines::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Bloodlines::RaceId).integer().not_null())
          .col(ColumnDef::new(Bloodlines::CorporationId).integer().not_null())
          .col(ColumnDef::new(Bloodlines::ShipItemTypeId).integer().not_null())
          .col(ColumnDef::new(Bloodlines::Name).string().not_null())
          .col(ColumnDef::new(Bloodlines::Description).text().not_null())
          .col(ColumnDef::new(Bloodlines::Charisma).integer().not_null())
          .col(ColumnDef::new(Bloodlines::Intelligence).integer().not_null())
          .col(ColumnDef::new(Bloodlines::Memory).integer().not_null())
          .col(ColumnDef::new(Bloodlines::Perception).integer().not_null())
          .col(ColumnDef::new(Bloodlines::WillPower).integer().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_bloodlines_race_id")
              .from(Bloodlines::Table, Bloodlines::RaceId)
              .to(Races::Table, Races::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_bloodlines_ship_item_type_id")
              .from(Bloodlines::Table, Bloodlines::ShipItemTypeId)
              .to(ItemTypes::Table, ItemTypes::Id),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (Bloodlines::RaceId, "idx_bloodlines_race_id"),
      (Bloodlines::Name, "idx_bloodlines_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(Bloodlines::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
