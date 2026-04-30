//! Migration: create character_contracts table.

use sea_orm_migration::prelude::*;

use crate::schema::iden::CharacterContracts;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(CharacterContracts::Table).to_owned())
      .await
  }

  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(CharacterContracts::Table)
          .if_not_exists()
          .col(
            ColumnDef::new(CharacterContracts::Id)
              .integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(CharacterContracts::CharacterId).big_integer().not_null())
          .col(ColumnDef::new(CharacterContracts::ContractId).big_integer().not_null())
          .col(ColumnDef::new(CharacterContracts::ContractType).text().not_null())
          .col(ColumnDef::new(CharacterContracts::Status).text().not_null())
          .col(ColumnDef::new(CharacterContracts::Title).text().not_null())
          .col(ColumnDef::new(CharacterContracts::IssuerId).big_integer().not_null())
          .col(ColumnDef::new(CharacterContracts::AssigneeId).big_integer().not_null())
          .col(ColumnDef::new(CharacterContracts::AcceptorId).big_integer().not_null())
          .col(ColumnDef::new(CharacterContracts::Price).double())
          .col(ColumnDef::new(CharacterContracts::Collateral).double())
          .col(ColumnDef::new(CharacterContracts::DateIssued).text().not_null())
          .col(ColumnDef::new(CharacterContracts::DateExpired).text().not_null())
          .col(ColumnDef::new(CharacterContracts::StartLocationId).big_integer())
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .name("idx_character_contracts_char_contract")
          .table(CharacterContracts::Table)
          .col(CharacterContracts::CharacterId)
          .col(CharacterContracts::ContractId)
          .unique()
          .to_owned(),
      )
      .await
  }
}
