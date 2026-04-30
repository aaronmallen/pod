//! Migration: create character_clone_implants table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterCloneImplants;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterCloneImplants::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterCloneImplants::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterCloneImplants::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(CharacterCloneImplants::CloneId).big_integer().not_null())
          .col(ColumnDef::new(CharacterCloneImplants::Slot).integer().not_null())
          .col(ColumnDef::new(CharacterCloneImplants::TypeId).integer().not_null())
          .col(ColumnDef::new(CharacterCloneImplants::Name).text().not_null())
          .col(ColumnDef::new(CharacterCloneImplants::AttributeBonus).text().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_clone_implants_clone_id")
          .table(CharacterCloneImplants::Table)
          .col(CharacterCloneImplants::CloneId)
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_clone_implants_clone_slot")
          .table(CharacterCloneImplants::Table)
          .col(CharacterCloneImplants::CloneId)
          .col(CharacterCloneImplants::Slot)
          .unique()
          .to_owned(),
      )
      .await
  }
}
