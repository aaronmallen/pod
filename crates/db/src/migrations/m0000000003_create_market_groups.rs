//! Migration that creates the `market_groups` table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::MarketGroups;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().if_exists().table(MarketGroups::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(MarketGroups::Table)
          .if_not_exists()
          .col(ColumnDef::new(MarketGroups::Id).integer().not_null().primary_key())
          .col(ColumnDef::new(MarketGroups::ParentMarketGroupId).integer().null())
          .col(ColumnDef::new(MarketGroups::Name).string().not_null())
          .col(ColumnDef::new(MarketGroups::Description).text().null())
          .foreign_key(
            ForeignKey::create()
              .name("fk_market_groups_parent_market_group_id")
              .from(MarketGroups::Table, MarketGroups::ParentMarketGroupId)
              .to(MarketGroups::Table, MarketGroups::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    for (col, name) in [
      (
        MarketGroups::ParentMarketGroupId,
        "idx_market_groups_parent_market_group_id",
      ),
      (MarketGroups::Name, "idx_market_groups_name"),
    ] {
      manager
        .create_index(
          Index::create()
            .if_not_exists()
            .name(name)
            .table(MarketGroups::Table)
            .col(col)
            .to_owned(),
        )
        .await?;
    }

    Ok(())
  }
}
