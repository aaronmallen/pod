//! Migration: create character_contact_labels table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterContactLabels;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterContactLabels::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterContactLabels::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterContactLabels::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(
            ColumnDef::new(CharacterContactLabels::CharacterId)
              .big_integer()
              .not_null(),
          )
          .col(ColumnDef::new(CharacterContactLabels::LabelId).integer().not_null())
          .col(ColumnDef::new(CharacterContactLabels::LabelName).text().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_contact_labels_char_label")
          .table(CharacterContactLabels::Table)
          .col(CharacterContactLabels::CharacterId)
          .col(CharacterContactLabels::LabelId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
