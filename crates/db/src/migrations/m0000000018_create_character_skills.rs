//! Migration: create character_skills table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterSkills;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterSkills::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterSkills::Table)
          .if_not_exists()
          .col(ColumnDef::new(CharacterSkills::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(CharacterSkills::SkillId).integer().not_null())
          .col(ColumnDef::new(CharacterSkills::TrainedLevel).integer().not_null())
          .col(ColumnDef::new(CharacterSkills::ActiveLevel).integer().not_null())
          .col(ColumnDef::new(CharacterSkills::Skillpoints).big_integer().not_null())
          .col(ColumnDef::new(CharacterSkills::TrainingEndTime).big_integer().null())
          .col(ColumnDef::new(CharacterSkills::TrainingStartTime).big_integer().null())
          .col(ColumnDef::new(CharacterSkills::TrainingStartSp).big_integer().null())
          .col(ColumnDef::new(CharacterSkills::IsActiveTraining).boolean().not_null())
          .primary_key(
            Index::create()
              .col(CharacterSkills::CharacterId)
              .col(CharacterSkills::SkillId),
          )
          .to_owned(),
      )
      .await
  }
}
