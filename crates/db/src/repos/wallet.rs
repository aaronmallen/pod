//! Repository for character wallet journal entries, transactions, and contracts.

use pod_model::{CharacterContract, WalletJournalEntry, WalletTransaction};
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::{
    character_contract::{ActiveModel as ContractActive, Column as ContractColumn, Entity as ContractEntity},
    wallet_journal_entry::{ActiveModel as JournalActive, Column as JournalColumn, Entity as JournalEntity},
    wallet_transaction::{ActiveModel as TxnActive, Column as TxnColumn, Entity as TxnEntity},
  },
};

/// Repository for character wallet journal entry, transaction, and contract operations.
pub struct Repo<'a> {
  db: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  /// Creates a new `Repo` bound to the given database connection.
  pub fn new(db: &'a DatabaseConnection) -> Self {
    Self {
      db,
    }
  }

  /// Returns all contracts for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn contracts_for_character(&self, character_id: i64) -> Result<Vec<CharacterContract>, Error> {
    let rows = ContractEntity::find()
      .filter(ContractColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(CharacterContract::from).collect())
  }

  /// Returns all wallet journal entries for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn journal_for_character(&self, character_id: i64) -> Result<Vec<WalletJournalEntry>, Error> {
    let rows = JournalEntity::find()
      .filter(JournalColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(WalletJournalEntry::from).collect())
  }

  /// Returns all wallet transactions for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn transactions_for_character(&self, character_id: i64) -> Result<Vec<WalletTransaction>, Error> {
    let rows = TxnEntity::find()
      .filter(TxnColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(WalletTransaction::from).collect())
  }

  /// Upserts contracts for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn upsert_contracts(&self, character_id: i64, contracts: &[CharacterContract]) -> Result<(), Error> {
    for contract in contracts {
      contract.validate()?;
      let active = ContractActive {
        acceptor_id: ActiveValue::Set(contract.acceptor_id),
        assignee_id: ActiveValue::Set(contract.assignee_id),
        character_id: ActiveValue::Set(character_id),
        collateral: ActiveValue::Set(contract.collateral),
        contract_id: ActiveValue::Set(contract.contract_id),
        contract_type: ActiveValue::Set(contract.contract_type.clone()),
        date_expired: ActiveValue::Set(contract.date_expired.clone()),
        date_issued: ActiveValue::Set(contract.date_issued.clone()),
        id: ActiveValue::NotSet,
        issuer_id: ActiveValue::Set(contract.issuer_id),
        price: ActiveValue::Set(contract.price),
        start_location_id: ActiveValue::Set(contract.start_location_id),
        status: ActiveValue::Set(contract.status.clone()),
        title: ActiveValue::Set(contract.title.clone()),
      };
      ContractEntity::insert(active)
        .on_conflict(
          OnConflict::columns([ContractColumn::CharacterId, ContractColumn::ContractId])
            .update_columns([
              ContractColumn::AcceptorId,
              ContractColumn::AssigneeId,
              ContractColumn::Collateral,
              ContractColumn::ContractType,
              ContractColumn::DateExpired,
              ContractColumn::DateIssued,
              ContractColumn::IssuerId,
              ContractColumn::Price,
              ContractColumn::StartLocationId,
              ContractColumn::Status,
              ContractColumn::Title,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Upserts wallet journal entries for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn upsert_journal_entries(&self, character_id: i64, entries: &[WalletJournalEntry]) -> Result<(), Error> {
    for entry in entries {
      entry.validate()?;
      let active = JournalActive {
        amount: ActiveValue::Set(entry.amount),
        balance: ActiveValue::Set(entry.balance),
        character_id: ActiveValue::Set(character_id),
        date: ActiveValue::Set(entry.date.clone()),
        description: ActiveValue::Set(entry.description.clone()),
        entry_id: ActiveValue::Set(entry.entry_id),
        first_party_id: ActiveValue::Set(entry.first_party_id),
        id: ActiveValue::NotSet,
        ref_type: ActiveValue::Set(entry.ref_type.clone()),
        second_party_id: ActiveValue::Set(entry.second_party_id),
      };
      JournalEntity::insert(active)
        .on_conflict(
          OnConflict::columns([JournalColumn::CharacterId, JournalColumn::EntryId])
            .update_columns([
              JournalColumn::Amount,
              JournalColumn::Balance,
              JournalColumn::Date,
              JournalColumn::Description,
              JournalColumn::FirstPartyId,
              JournalColumn::RefType,
              JournalColumn::SecondPartyId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Upserts wallet transactions for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn upsert_wallet_transactions(&self, character_id: i64, txns: &[WalletTransaction]) -> Result<(), Error> {
    for txn in txns {
      txn.validate()?;
      let active = TxnActive {
        character_id: ActiveValue::Set(character_id),
        client_id: ActiveValue::Set(txn.client_id),
        date: ActiveValue::Set(txn.date.clone()),
        id: ActiveValue::NotSet,
        is_buy: ActiveValue::Set(txn.is_buy),
        location_id: ActiveValue::Set(txn.location_id),
        quantity: ActiveValue::Set(txn.quantity),
        transaction_id: ActiveValue::Set(txn.transaction_id),
        type_id: ActiveValue::Set(txn.type_id),
        unit_price: ActiveValue::Set(txn.unit_price),
      };
      TxnEntity::insert(active)
        .on_conflict(
          OnConflict::columns([TxnColumn::CharacterId, TxnColumn::TransactionId])
            .update_columns([
              TxnColumn::ClientId,
              TxnColumn::Date,
              TxnColumn::IsBuy,
              TxnColumn::LocationId,
              TxnColumn::Quantity,
              TxnColumn::TypeId,
              TxnColumn::UnitPrice,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_contract(character_id: i64, contract_id: i64) -> CharacterContract {
    CharacterContract {
      acceptor_id: 0,
      assignee_id: 0,
      character_id,
      collateral: None,
      contract_id,
      contract_type: "item_exchange".to_string(),
      date_expired: "2024-01-15T00:00:00Z".to_string(),
      date_issued: "2024-01-01T00:00:00Z".to_string(),
      issuer_id: character_id,
      price: Some(5_000_000.0),
      start_location_id: Some(60_003_760),
      status: "outstanding".to_string(),
      title: "Rifter x10".to_string(),
    }
  }

  fn make_journal_entry(character_id: i64, entry_id: i64) -> WalletJournalEntry {
    WalletJournalEntry {
      amount: Some(-50_000.0),
      balance: Some(4_950_000.0),
      character_id,
      date: "2024-06-01T12:00:00Z".to_string(),
      description: "Market sell order".to_string(),
      entry_id,
      first_party_id: Some(character_id),
      ref_type: "market_transaction".to_string(),
      second_party_id: None,
    }
  }

  fn make_transaction(character_id: i64, transaction_id: i64) -> WalletTransaction {
    WalletTransaction {
      character_id,
      client_id: 90_000_002,
      date: "2024-06-01T12:00:00Z".to_string(),
      is_buy: false,
      location_id: 60_003_760,
      quantity: 1,
      transaction_id,
      type_id: 587,
      unit_price: 600_000.0,
    }
  }

  mod contracts_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_contracts_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.contracts_for_character(1).await.unwrap();

      assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_contracts_for_the_given_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_contracts(1, &[make_contract(1, 1001), make_contract(1, 1002)])
        .await
        .unwrap();
      repo.upsert_contracts(2, &[make_contract(2, 2001)]).await.unwrap();

      let result = repo.contracts_for_character(1).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod journal_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_entries_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.journal_for_character(1).await.unwrap();

      assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_entries_for_the_given_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_journal_entries(1, &[make_journal_entry(1, 5001), make_journal_entry(1, 5002)])
        .await
        .unwrap();
      repo
        .upsert_journal_entries(2, &[make_journal_entry(2, 6001)])
        .await
        .unwrap();

      let result = repo.journal_for_character(1).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod transactions_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_transactions_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.transactions_for_character(1).await.unwrap();

      assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_transactions_for_the_given_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_wallet_transactions(1, &[make_transaction(1, 7001), make_transaction(1, 7002)])
        .await
        .unwrap();
      repo
        .upsert_wallet_transactions(2, &[make_transaction(2, 8001)])
        .await
        .unwrap();

      let result = repo.transactions_for_character(1).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod upsert_contracts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_new_contracts() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_contracts(1, &[make_contract(1, 1001)]).await.unwrap();

      let rows = repo.contracts_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].contract_id, 1001);
    }

    #[tokio::test]
    async fn it_updates_existing_contract_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_contracts(1, &[make_contract(1, 1001)]).await.unwrap();

      let mut updated = make_contract(1, 1001);
      updated.status = "finished".to_string();
      repo.upsert_contracts(1, &[updated]).await.unwrap();

      let rows = repo.contracts_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].status, "finished");
    }
  }

  mod upsert_journal_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_new_entries() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_journal_entries(1, &[make_journal_entry(1, 5001)])
        .await
        .unwrap();

      let rows = repo.journal_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].entry_id, 5001);
    }

    #[tokio::test]
    async fn it_updates_existing_entry_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_journal_entries(1, &[make_journal_entry(1, 5001)])
        .await
        .unwrap();

      let mut updated = make_journal_entry(1, 5001);
      updated.amount = Some(-100_000.0);
      repo.upsert_journal_entries(1, &[updated]).await.unwrap();

      let rows = repo.journal_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].amount, Some(-100_000.0));
    }
  }

  mod upsert_wallet_transactions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_new_transactions() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_wallet_transactions(1, &[make_transaction(1, 7001)])
        .await
        .unwrap();

      let rows = repo.transactions_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].transaction_id, 7001);
    }

    #[tokio::test]
    async fn it_updates_existing_transaction_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_wallet_transactions(1, &[make_transaction(1, 7001)])
        .await
        .unwrap();

      let mut updated = make_transaction(1, 7001);
      updated.quantity = 5;
      repo.upsert_wallet_transactions(1, &[updated]).await.unwrap();

      let rows = repo.transactions_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].quantity, 5);
    }
  }
}
