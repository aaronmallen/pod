//! Migration: create skill_plan_entries table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::SkillPlanEntries;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(SkillPlanEntries::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(SkillPlanEntries::Table)
          .if_not_exists()
          .col(ColumnDef::new(SkillPlanEntries::Id).text().not_null().primary_key())
          .col(ColumnDef::new(SkillPlanEntries::PlanId).text().not_null())
          .col(ColumnDef::new(SkillPlanEntries::SkillName).text().not_null())
          .col(ColumnDef::new(SkillPlanEntries::ToLevel).integer().not_null())
          .col(
            ColumnDef::new(SkillPlanEntries::Priority)
              .text()
              .not_null()
              .default("normal"),
          )
          .col(ColumnDef::new(SkillPlanEntries::Note).text())
          .col(ColumnDef::new(SkillPlanEntries::Position).integer().not_null())
          .col(ColumnDef::new(SkillPlanEntries::Auto).integer().not_null().default(0))
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_skill_plan_entries_plan_id")
          .table(SkillPlanEntries::Table)
          .col(SkillPlanEntries::PlanId)
          .to_owned(),
      )
      .await
  }
}
