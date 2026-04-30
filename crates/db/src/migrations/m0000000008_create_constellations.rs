//! Migration that creates the `constellations` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::{Constellations, Regions};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(Constellations::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Constellations::Table)
          .if_not_exists()
          .col(ColumnDef::new(Constellations::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(Constellations::RegionId).integer().not_null())
          .col(ColumnDef::new(Constellations::Name).string().not_null())
          .col(ColumnDef::new(Constellations::PositionX).double().not_null())
          .col(ColumnDef::new(Constellations::PositionY).double().not_null())
          .col(ColumnDef::new(Constellations::PositionZ).double().not_null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_constellations_region_id")
              .from(Constellations::Table, Constellations::RegionId)
              .to(Regions::Table, Regions::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    for (col, name, unique) in [
      (Constellations::RegionId, "idx_constellations_region_id", false),
      (Constellations::Name, "udx_constellations_name", true),
    ] {
      let mut idx = Index::create()
        .if_not_exists()
        .name(name)
        .table(Constellations::Table)
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
