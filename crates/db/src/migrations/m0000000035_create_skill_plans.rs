//! Migration: create skill_plans table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::SkillPlans;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(SkillPlans::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(SkillPlans::Table)
          .if_not_exists()
          .col(ColumnDef::new(SkillPlans::Id).text().not_null().primary_key())
          .col(ColumnDef::new(SkillPlans::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(SkillPlans::Name).text().not_null())
          .col(ColumnDef::new(SkillPlans::RemapJson).text())
          .col(
            ColumnDef::new(SkillPlans::ImplantSet)
              .text()
              .not_null()
              .default("current"),
          )
          .col(ColumnDef::new(SkillPlans::CreatedAt).big_integer().not_null())
          .col(ColumnDef::new(SkillPlans::UpdatedAt).big_integer().not_null())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_skill_plans_character_id")
          .table(SkillPlans::Table)
          .col(SkillPlans::CharacterId)
          .to_owned(),
      )
      .await
  }
}
