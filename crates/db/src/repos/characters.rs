//! Repository for character persistence.

use pod_model::{
  Character, CharacterAsset, CharacterContract, CharacterSkill, MailHeader, NeuralAttributes, WalletJournalEntry,
  WalletTransaction,
};
use sea_orm::{
  ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect,
  sea_query::OnConflict,
};
use validator::Validate;

use crate::{
  Error,
  entities::{
    character::{ActiveModel as CharacterActive, Column as CharacterColumn, Entity as CharacterEntity},
    character_asset::{ActiveModel as AssetActive, Column as AssetColumn, Entity as AssetEntity},
    character_contract::{ActiveModel as ContractActive, Column as ContractColumn, Entity as ContractEntity},
    character_skill::{ActiveModel as SkillActive, Column as SkillColumn, Entity as SkillEntity},
    entity_tag::{Column as EntityTagColumn, Entity as EntityTagEntity},
    mail_header::{ActiveModel as MailHeaderActive, Column as MailHeaderColumn, Entity as MailHeaderEntity},
    snoozed_mail::{
      ActiveModel as SnoozedActive, Column as SnoozedColumn, Entity as SnoozedEntity, Model as SnoozedModel,
    },
    wallet_journal_entry::{ActiveModel as JournalActive, Column as JournalColumn, Entity as JournalEntity},
    wallet_transaction::{ActiveModel as TxnActive, Column as TxnColumn, Entity as TxnEntity},
  },
};

/// Repository for character CRUD operations.
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

  /// Returns all characters ordered by sort_order, each with their skills loaded.
  pub async fn all(&self) -> Result<Vec<Character>, Error> {
    let rows = CharacterEntity::find()
      .order_by(CharacterColumn::SortOrder, Order::Asc)
      .all(self.db)
      .await?;
    let mut characters = Vec::with_capacity(rows.len());
    for row in rows {
      let mut character = Character::from(row);
      let skills = self.skills_for(*character.id()).await?;
      *character.skills_mut() = skills;
      characters.push(character);
    }
    Ok(characters)
  }

  /// Deletes a character and all associated skills, assets, and tag assignments.
  pub async fn delete(&self, character_id: i64) -> Result<(), Error> {
    EntityTagEntity::delete_many()
      .filter(EntityTagColumn::EntityId.eq(character_id))
      .filter(EntityTagColumn::EntityType.eq("character"))
      .exec(self.db)
      .await?;
    SkillEntity::delete_many()
      .filter(SkillColumn::CharacterId.eq(character_id))
      .exec(self.db)
      .await?;
    AssetEntity::delete_many()
      .filter(AssetColumn::CharacterId.eq(character_id))
      .exec(self.db)
      .await?;
    CharacterEntity::delete_by_id(character_id).exec(self.db).await?;
    Ok(())
  }

  /// Finds a character by EVE character ID, loading skills as well.
  pub async fn find(&self, id: i64) -> Result<Option<Character>, Error> {
    let Some(row) = CharacterEntity::find_by_id(id).one(self.db).await? else {
      return Ok(None);
    };
    let mut character = Character::from(row);
    let skills = self.skills_for(id).await?;
    *character.skills_mut() = skills;
    Ok(Some(character))
  }

  /// Updates only the location fields for a character.
  pub async fn update_location(
    &self,
    character_id: i64,
    location_name: Option<String>,
    location_docked: Option<bool>,
  ) -> Result<(), Error> {
    let active = CharacterActive {
      id: ActiveValue::Set(character_id),
      location_docked: ActiveValue::Set(location_docked),
      location_name: ActiveValue::Set(location_name),
      ..Default::default()
    };
    CharacterEntity::update(active).exec(self.db).await?;
    Ok(())
  }

  /// Updates only the OAuth token fields for a character.
  pub async fn update_token(
    &self,
    character_id: i64,
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
  ) -> Result<(), Error> {
    let active = CharacterActive {
      id: ActiveValue::Set(character_id),
      access_token: ActiveValue::Set(access_token.to_string()),
      refresh_token: ActiveValue::Set(refresh_token.to_string()),
      token_expires_at: ActiveValue::Set(expires_at),
      ..Default::default()
    };
    CharacterEntity::update(active).exec(self.db).await?;
    Ok(())
  }

  pub async fn update_granted_scopes(&self, character_id: i64, scopes: &str) -> Result<(), Error> {
    let active = CharacterActive {
      id: ActiveValue::Set(character_id),
      granted_scopes: ActiveValue::Set(Some(scopes.to_string())),
      ..Default::default()
    };
    CharacterEntity::update(active).exec(self.db).await?;
    Ok(())
  }

  /// Updates only the wallet balance for a character.
  pub async fn update_wallet(&self, character_id: i64, isk_balance: Option<f64>) -> Result<(), Error> {
    let active = CharacterActive {
      id: ActiveValue::Set(character_id),
      isk_balance: ActiveValue::Set(isk_balance),
      ..Default::default()
    };
    CharacterEntity::update(active).exec(self.db).await?;
    Ok(())
  }

  /// Updates only the corporation fields for a character.
  pub async fn update_corp(&self, character_id: i64, corp_id: i64, corp_name: String) -> Result<(), Error> {
    let active = CharacterActive {
      id: ActiveValue::Set(character_id),
      corp_id: ActiveValue::Set(corp_id),
      corp_name: ActiveValue::Set(corp_name),
      ..Default::default()
    };
    CharacterEntity::update(active).exec(self.db).await?;
    Ok(())
  }

  /// Inserts or updates a character row, validating first.
  pub async fn upsert(&self, character: &Character) -> Result<(), Error> {
    character.validate()?;
    let active = CharacterActive {
      id: ActiveValue::Set(*character.id()),
      name: ActiveValue::Set(character.name().clone()),
      corp_id: ActiveValue::Set(*character.corp_id()),
      corp_name: ActiveValue::Set(character.corp_name().clone()),
      granted_scopes: ActiveValue::Set(character.granted_scopes().clone()),
      portrait_tone: ActiveValue::Set(*character.portrait_tone()),
      access_token: ActiveValue::Set(character.access_token().clone()),
      refresh_token: ActiveValue::Set(character.refresh_token().clone()),
      sort_order: ActiveValue::Set(*character.sort_order()),
      token_expires_at: ActiveValue::Set(*character.token_expires_at()),
      isk_balance: ActiveValue::Set(*character.isk_balance()),
      location_name: ActiveValue::Set(character.location_name().clone()),
      location_docked: ActiveValue::Set(*character.location_docked()),
      charisma: ActiveValue::NotSet,
      intelligence: ActiveValue::NotSet,
      memory: ActiveValue::NotSet,
      perception: ActiveValue::NotSet,
      willpower: ActiveValue::NotSet,
    };
    CharacterEntity::insert(active)
      .on_conflict(
        OnConflict::column(CharacterColumn::Id)
          .update_columns([
            CharacterColumn::Name,
            CharacterColumn::CorpId,
            CharacterColumn::CorpName,
            CharacterColumn::GrantedScopes,
            CharacterColumn::PortraitTone,
            CharacterColumn::AccessToken,
            CharacterColumn::RefreshToken,
            CharacterColumn::TokenExpiresAt,
            CharacterColumn::IskBalance,
            CharacterColumn::LocationName,
            CharacterColumn::LocationDocked,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Updates the sort_order for each (character_id, order) pair.
  pub async fn update_sort_orders(&self, updates: &[(i64, i32)]) -> Result<(), Error> {
    for &(id, order) in updates {
      let active = CharacterActive {
        id: ActiveValue::Set(id),
        sort_order: ActiveValue::Set(order),
        ..Default::default()
      };
      CharacterEntity::update(active).exec(self.db).await?;
    }
    Ok(())
  }

  /// Upserts all asset rows for the given character.
  pub async fn upsert_assets(&self, character_id: i64, assets: &[CharacterAsset]) -> Result<(), Error> {
    use crate::entities::character_asset::Entity as AssetEntity;

    for asset in assets {
      asset.validate()?;
      let active = AssetActive {
        item_id: ActiveValue::Set(asset.item_id),
        character_id: ActiveValue::Set(character_id),
        type_id: ActiveValue::Set(asset.type_id),
        location_id: ActiveValue::Set(asset.location_id),
        location_type: ActiveValue::Set(asset.location_type.clone()),
        location_flag: ActiveValue::Set(asset.location_flag.clone()),
        quantity: ActiveValue::Set(asset.quantity),
        is_singleton: ActiveValue::Set(asset.is_singleton),
        is_blueprint_copy: ActiveValue::Set(asset.is_blueprint_copy),
      };
      AssetEntity::insert(active)
        .on_conflict(
          OnConflict::column(AssetColumn::ItemId)
            .update_columns([
              AssetColumn::CharacterId,
              AssetColumn::TypeId,
              AssetColumn::LocationId,
              AssetColumn::LocationType,
              AssetColumn::LocationFlag,
              AssetColumn::Quantity,
              AssetColumn::IsSingleton,
              AssetColumn::IsBlueprintCopy,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Deletes asset rows for `character_id` whose `item_id` is not in `keep_ids`.
  /// If `keep_ids` is empty all assets for the character are removed.
  pub async fn delete_stale_assets(&self, character_id: i64, keep_ids: &[i64]) -> Result<u64, Error> {
    let result = if keep_ids.is_empty() {
      AssetEntity::delete_many()
        .filter(AssetColumn::CharacterId.eq(character_id))
        .exec(self.db)
        .await?
    } else {
      AssetEntity::delete_many()
        .filter(AssetColumn::CharacterId.eq(character_id))
        .filter(AssetColumn::ItemId.is_not_in(keep_ids.to_vec()))
        .exec(self.db)
        .await?
    };
    Ok(result.rows_affected)
  }

  /// Upserts all skill rows for the given character.
  pub async fn upsert_skills(&self, character_id: i64, skills: &[CharacterSkill]) -> Result<(), Error> {
    for skill in skills {
      skill.validate()?;
      let active = SkillActive {
        character_id: ActiveValue::Set(character_id),
        skill_id: ActiveValue::Set(skill.skill_id),
        trained_level: ActiveValue::Set(skill.trained_level),
        active_level: ActiveValue::Set(skill.active_level),
        skillpoints: ActiveValue::Set(skill.skillpoints),
        training_end_time: ActiveValue::Set(skill.training_end_time),
        training_start_time: ActiveValue::Set(skill.training_start_time),
        training_start_sp: ActiveValue::Set(skill.training_start_sp),
        is_active_training: ActiveValue::Set(skill.is_active_training),
      };
      SkillEntity::insert(active)
        .on_conflict(
          OnConflict::columns([SkillColumn::CharacterId, SkillColumn::SkillId])
            .update_columns([
              SkillColumn::TrainedLevel,
              SkillColumn::ActiveLevel,
              SkillColumn::Skillpoints,
              SkillColumn::TrainingEndTime,
              SkillColumn::TrainingStartTime,
              SkillColumn::TrainingStartSp,
              SkillColumn::IsActiveTraining,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Loads all skill rows for the given character ID.
  async fn skills_for(&self, character_id: i64) -> Result<Vec<CharacterSkill>, Error> {
    let rows = SkillEntity::find()
      .filter(SkillColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(CharacterSkill::from).collect())
  }

  /// Upserts wallet journal entries for the given character.
  pub async fn upsert_journal_entries(&self, character_id: i64, entries: &[WalletJournalEntry]) -> Result<(), Error> {
    for entry in entries {
      entry.validate()?;
      let active = JournalActive {
        id: ActiveValue::NotSet,
        character_id: ActiveValue::Set(character_id),
        entry_id: ActiveValue::Set(entry.entry_id),
        ref_type: ActiveValue::Set(entry.ref_type.clone()),
        amount: ActiveValue::Set(entry.amount),
        balance: ActiveValue::Set(entry.balance),
        date: ActiveValue::Set(entry.date.clone()),
        description: ActiveValue::Set(entry.description.clone()),
        first_party_id: ActiveValue::Set(entry.first_party_id),
        second_party_id: ActiveValue::Set(entry.second_party_id),
      };
      JournalEntity::insert(active)
        .on_conflict(
          OnConflict::columns([JournalColumn::CharacterId, JournalColumn::EntryId])
            .update_columns([
              JournalColumn::RefType,
              JournalColumn::Amount,
              JournalColumn::Balance,
              JournalColumn::Date,
              JournalColumn::Description,
              JournalColumn::FirstPartyId,
              JournalColumn::SecondPartyId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Returns the 200 most-recent wallet journal entries for the given character.
  pub async fn journal_entries(&self, character_id: i64) -> Result<Vec<WalletJournalEntry>, Error> {
    let rows = JournalEntity::find()
      .filter(JournalColumn::CharacterId.eq(character_id))
      .order_by_desc(JournalColumn::Date)
      .limit(200)
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Upserts wallet transactions for the given character.
  pub async fn upsert_transactions(&self, character_id: i64, txns: &[WalletTransaction]) -> Result<(), Error> {
    for txn in txns {
      txn.validate()?;
      let active = TxnActive {
        id: ActiveValue::NotSet,
        character_id: ActiveValue::Set(character_id),
        transaction_id: ActiveValue::Set(txn.transaction_id),
        type_id: ActiveValue::Set(txn.type_id),
        quantity: ActiveValue::Set(txn.quantity),
        unit_price: ActiveValue::Set(txn.unit_price),
        is_buy: ActiveValue::Set(txn.is_buy),
        date: ActiveValue::Set(txn.date.clone()),
        location_id: ActiveValue::Set(txn.location_id),
        client_id: ActiveValue::Set(txn.client_id),
      };
      TxnEntity::insert(active)
        .on_conflict(
          OnConflict::columns([TxnColumn::CharacterId, TxnColumn::TransactionId])
            .update_columns([
              TxnColumn::TypeId,
              TxnColumn::Quantity,
              TxnColumn::UnitPrice,
              TxnColumn::IsBuy,
              TxnColumn::Date,
              TxnColumn::LocationId,
              TxnColumn::ClientId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Returns the 200 most-recent wallet transactions for the given character.
  pub async fn transactions(&self, character_id: i64) -> Result<Vec<WalletTransaction>, Error> {
    let rows = TxnEntity::find()
      .filter(TxnColumn::CharacterId.eq(character_id))
      .order_by_desc(TxnColumn::Date)
      .limit(200)
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Upserts mail headers for the given character.
  pub async fn upsert_mail_headers(&self, character_id: i64, headers: &[MailHeader]) -> Result<(), Error> {
    for header in headers {
      header.validate()?;
      let active = MailHeaderActive {
        id: ActiveValue::NotSet,
        character_id: ActiveValue::Set(character_id),
        mail_id: ActiveValue::Set(header.mail_id),
        subject: ActiveValue::Set(header.subject.clone()),
        from_id: ActiveValue::Set(header.from_id),
        is_read: ActiveValue::Set(header.is_read),
        timestamp: ActiveValue::Set(header.timestamp.clone()),
        recipients_display: ActiveValue::Set(header.recipients_display.clone()),
      };
      MailHeaderEntity::insert(active)
        .on_conflict(
          OnConflict::columns([MailHeaderColumn::CharacterId, MailHeaderColumn::MailId])
            .update_columns([
              MailHeaderColumn::Subject,
              MailHeaderColumn::FromId,
              MailHeaderColumn::IsRead,
              MailHeaderColumn::Timestamp,
              MailHeaderColumn::RecipientsDisplay,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Upserts a snoozed mail deadline.
  pub async fn upsert_snoozed_mail(&self, character_id: i64, mail_id: i64, snooze_until: &str) -> Result<(), Error> {
    let active = SnoozedActive {
      id: ActiveValue::NotSet,
      character_id: ActiveValue::Set(character_id),
      mail_id: ActiveValue::Set(mail_id),
      snooze_until: ActiveValue::Set(snooze_until.to_string()),
    };
    SnoozedEntity::insert(active)
      .on_conflict(
        OnConflict::columns([SnoozedColumn::CharacterId, SnoozedColumn::MailId])
          .update_column(SnoozedColumn::SnoozeUntil)
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Removes a snoozed mail record.
  pub async fn delete_snoozed_mail(&self, character_id: i64, mail_id: i64) -> Result<(), Error> {
    SnoozedEntity::delete_many()
      .filter(SnoozedColumn::CharacterId.eq(character_id))
      .filter(SnoozedColumn::MailId.eq(mail_id))
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Returns all snoozed mail records whose deadline is at or before `now_iso`.
  pub async fn expired_snoozed_mails(&self, now_iso: &str) -> Result<Vec<SnoozedModel>, Error> {
    let rows = SnoozedEntity::find()
      .filter(SnoozedColumn::SnoozeUntil.lte(now_iso))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Returns all snoozed mail records.
  pub async fn all_snoozed_mails(&self) -> Result<Vec<SnoozedModel>, Error> {
    let rows = SnoozedEntity::find().all(self.db).await?;
    Ok(rows)
  }

  /// Returns the stored ESI effective neural attributes for the given
  /// character, or `None` if attributes have not yet been synced.
  pub async fn effective_attributes(&self, character_id: i64) -> Result<Option<NeuralAttributes>, Error> {
    let Some(row) = CharacterEntity::find_by_id(character_id).one(self.db).await? else {
      return Ok(None);
    };
    match (
      row.charisma,
      row.intelligence,
      row.memory,
      row.perception,
      row.willpower,
    ) {
      (Some(cha), Some(int), Some(mem), Some(per), Some(wil)) => Ok(Some(NeuralAttributes {
        charisma: cha,
        intelligence: int,
        memory: mem,
        perception: per,
        willpower: wil,
      })),
      _ => Ok(None),
    }
  }

  /// Persists ESI effective neural attributes for the given character.
  pub async fn update_neural_attributes(&self, character_id: i64, attrs: &NeuralAttributes) -> Result<(), Error> {
    let active = CharacterActive {
      id: ActiveValue::Set(character_id),
      charisma: ActiveValue::Set(Some(attrs.charisma)),
      intelligence: ActiveValue::Set(Some(attrs.intelligence)),
      memory: ActiveValue::Set(Some(attrs.memory)),
      perception: ActiveValue::Set(Some(attrs.perception)),
      willpower: ActiveValue::Set(Some(attrs.willpower)),
      ..Default::default()
    };
    CharacterEntity::update(active).exec(self.db).await?;
    Ok(())
  }

  /// Returns raw asset entity rows for all given character IDs.
  pub async fn assets_for_character_ids(
    &self,
    char_ids: &[i64],
  ) -> Result<Vec<crate::entities::character_asset::Model>, Error> {
    let rows = AssetEntity::find()
      .filter(AssetColumn::CharacterId.is_in(char_ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Upserts character contracts for the given character.
  pub async fn upsert_contracts(&self, character_id: i64, contracts: &[CharacterContract]) -> Result<(), Error> {
    for contract in contracts {
      contract.validate()?;
      let active = ContractActive {
        id: ActiveValue::NotSet,
        character_id: ActiveValue::Set(character_id),
        contract_id: ActiveValue::Set(contract.contract_id),
        contract_type: ActiveValue::Set(contract.contract_type.clone()),
        status: ActiveValue::Set(contract.status.clone()),
        title: ActiveValue::Set(contract.title.clone()),
        issuer_id: ActiveValue::Set(contract.issuer_id),
        assignee_id: ActiveValue::Set(contract.assignee_id),
        acceptor_id: ActiveValue::Set(contract.acceptor_id),
        price: ActiveValue::Set(contract.price),
        collateral: ActiveValue::Set(contract.collateral),
        date_issued: ActiveValue::Set(contract.date_issued.clone()),
        date_expired: ActiveValue::Set(contract.date_expired.clone()),
        start_location_id: ActiveValue::Set(contract.start_location_id),
      };
      ContractEntity::insert(active)
        .on_conflict(
          OnConflict::columns([ContractColumn::CharacterId, ContractColumn::ContractId])
            .update_columns([
              ContractColumn::ContractType,
              ContractColumn::Status,
              ContractColumn::Title,
              ContractColumn::IssuerId,
              ContractColumn::AssigneeId,
              ContractColumn::AcceptorId,
              ContractColumn::Price,
              ContractColumn::Collateral,
              ContractColumn::DateIssued,
              ContractColumn::DateExpired,
              ContractColumn::StartLocationId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  /// Returns the 200 most-recent contracts for the given character.
  pub async fn contracts(&self, character_id: i64) -> Result<Vec<CharacterContract>, Error> {
    let rows = ContractEntity::find()
      .filter(ContractColumn::CharacterId.eq(character_id))
      .order_by_desc(ContractColumn::DateIssued)
      .limit(200)
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Deletes a single mail header by character and mail ID.
  pub async fn delete_mail_header(&self, character_id: i64, mail_id: i64) -> Result<(), Error> {
    MailHeaderEntity::delete_many()
      .filter(MailHeaderColumn::CharacterId.eq(character_id))
      .filter(MailHeaderColumn::MailId.eq(mail_id))
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Returns all mail headers for the given character, newest first.
  pub async fn mail_headers(&self, character_id: i64) -> Result<Vec<MailHeader>, Error> {
    let rows = MailHeaderEntity::find()
      .filter(MailHeaderColumn::CharacterId.eq(character_id))
      .order_by_desc(MailHeaderColumn::Timestamp)
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }
}

#[cfg(test)]
mod tests {
  use sea_orm::{Database, DatabaseConnection};

  use super::*;
  use crate::entities::character::ActiveModel as CharActive;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  async fn insert_character(db: &DatabaseConnection, id: i64, name: &str) {
    use sea_orm::ActiveValue::Set;
    crate::entities::character::Entity::insert(CharActive {
      access_token: Set(String::new()),
      charisma: Set(None),
      corp_id: Set(0),
      corp_name: Set(String::new()),
      granted_scopes: Set(None),
      id: Set(id),
      intelligence: Set(None),
      isk_balance: Set(None),
      location_docked: Set(None),
      location_name: Set(None),
      memory: Set(None),
      name: Set(name.to_string()),
      perception: Set(None),
      portrait_tone: Set(0),
      refresh_token: Set(String::new()),
      sort_order: Set(0),
      token_expires_at: Set(0),
      willpower: Set(None),
    })
    .exec(db)
    .await
    .unwrap();
  }

  mod effective_attributes {
    use pod_model::NeuralAttributes;

    use super::*;

    #[tokio::test]
    async fn returns_none_when_character_does_not_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.effective_attributes(999).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_none_when_attributes_not_set() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      let repo = Repo::new(&db);
      let result = repo.effective_attributes(1).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_some_when_all_five_attributes_are_set() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      let repo = Repo::new(&db);

      let attrs = NeuralAttributes {
        charisma: 20,
        intelligence: 24,
        memory: 22,
        perception: 23,
        willpower: 21,
      };
      repo.update_neural_attributes(1, &attrs).await.unwrap();

      let result = repo.effective_attributes(1).await.unwrap();
      assert!(result.is_some());
      let got = result.unwrap();
      assert_eq!(got.charisma, 20);
      assert_eq!(got.intelligence, 24);
      assert_eq!(got.memory, 22);
      assert_eq!(got.perception, 23);
      assert_eq!(got.willpower, 21);
    }

    #[tokio::test]
    async fn returns_none_when_only_some_attributes_are_set() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      let repo = Repo::new(&db);

      use sea_orm::ActiveValue::Set;
      crate::entities::character::Entity::update(CharActive {
        id: Set(1),
        charisma: Set(Some(20)),
        intelligence: Set(Some(24)),
        ..Default::default()
      })
      .exec(&db)
      .await
      .unwrap();

      let result = repo.effective_attributes(1).await.unwrap();
      assert!(result.is_none());
    }
  }

  mod delete_stale_assets {
    use pod_model::CharacterAsset;

    use super::*;

    async fn insert_asset(db: &DatabaseConnection, character_id: i64, item_id: i64) {
      let asset = CharacterAsset {
        item_id,
        character_id,
        type_id: 1,
        location_id: 60003760,
        location_type: "station".to_string(),
        location_flag: "Hangar".to_string(),
        quantity: 1,
        is_singleton: false,
        is_blueprint_copy: None,
      };
      let repo = Repo::new(db);
      repo.upsert_assets(character_id, &[asset]).await.unwrap();
    }

    #[tokio::test]
    async fn deletes_all_assets_when_keep_ids_empty() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      insert_asset(&db, 1, 100).await;
      insert_asset(&db, 1, 200).await;

      let repo = Repo::new(&db);
      let deleted = repo.delete_stale_assets(1, &[]).await.unwrap();
      assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn keeps_specified_ids_and_deletes_others() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      insert_asset(&db, 1, 100).await;
      insert_asset(&db, 1, 200).await;
      insert_asset(&db, 1, 300).await;

      let repo = Repo::new(&db);
      let deleted = repo.delete_stale_assets(1, &[100, 200]).await.unwrap();
      assert_eq!(deleted, 1);

      let remaining = repo.assets_for_character_ids(&[1]).await.unwrap();
      assert_eq!(remaining.len(), 2);
      let ids: Vec<i64> = remaining.iter().map(|a| a.item_id).collect();
      assert!(ids.contains(&100));
      assert!(ids.contains(&200));
    }

    #[tokio::test]
    async fn returns_zero_when_no_assets_exist() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;

      let repo = Repo::new(&db);
      let deleted = repo.delete_stale_assets(1, &[]).await.unwrap();
      assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn does_not_delete_assets_for_other_characters() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      insert_character(&db, 2, "Beta").await;
      insert_asset(&db, 1, 100).await;
      insert_asset(&db, 2, 200).await;

      let repo = Repo::new(&db);
      let deleted = repo.delete_stale_assets(1, &[]).await.unwrap();
      assert_eq!(deleted, 1);

      let remaining = repo.assets_for_character_ids(&[2]).await.unwrap();
      assert_eq!(remaining.len(), 1);
    }
  }

  mod find_and_upsert {
    use super::*;

    #[tokio::test]
    async fn find_returns_none_for_missing_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find(999).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_returns_some_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let mut character = pod_model::Character::new(1001, "Test Pilot");
      character.set_corp_id(98000000).set_corp_name("Test Corp".to_string());

      repo.upsert(&character).await.unwrap();

      let result = repo.find(1001).await.unwrap();
      assert!(result.is_some());
      let found = result.unwrap();
      assert_eq!(*found.id(), 1001);
      assert_eq!(found.name(), "Test Pilot");
    }

    #[tokio::test]
    async fn upsert_updates_existing_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let mut character = pod_model::Character::new(1001, "Test Pilot");
      character.set_corp_id(98000000).set_corp_name("Old Corp".to_string());
      repo.upsert(&character).await.unwrap();

      let mut updated = pod_model::Character::new(1001, "Test Pilot");
      updated.set_corp_id(98000001).set_corp_name("New Corp".to_string());
      repo.upsert(&updated).await.unwrap();

      let result = repo.find(1001).await.unwrap().unwrap();
      assert_eq!(result.corp_name(), "New Corp");
    }

    #[tokio::test]
    async fn second_upsert_for_same_character_id_yields_one_row_with_updated_token() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let mut first = pod_model::Character::new(1001, "Test Pilot");
      first
        .set_access_token("token-v1")
        .set_refresh_token("refresh-v1")
        .set_granted_scopes(Some("esi-skills.read_skills.v1".to_string()))
        .set_corp_id(98000000)
        .set_corp_name("Test Corp".to_string());
      repo.upsert(&first).await.unwrap();

      let mut second = pod_model::Character::new(1001, "Test Pilot");
      second
        .set_access_token("token-v2")
        .set_refresh_token("refresh-v2")
        .set_granted_scopes(Some(
          "esi-skills.read_skills.v1 esi-wallet.read_character_wallet.v1".to_string(),
        ))
        .set_corp_id(98000000)
        .set_corp_name("Test Corp".to_string());
      repo.upsert(&second).await.unwrap();

      let all = repo.all().await.unwrap();
      assert_eq!(all.len(), 1);
      let found = repo.find(1001).await.unwrap().unwrap();
      assert_eq!(found.access_token(), "token-v2");
      assert_eq!(found.refresh_token(), "refresh-v2");
      assert_eq!(
        found.granted_scopes().as_deref(),
        Some("esi-skills.read_skills.v1 esi-wallet.read_character_wallet.v1")
      );
    }
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_characters() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.all().await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_all_characters_ordered_by_sort_order() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let mut c1 = pod_model::Character::new(1001, "Alpha");
      c1.set_sort_order(2);
      let mut c2 = pod_model::Character::new(1002, "Beta");
      c2.set_sort_order(1);

      repo.upsert(&c1).await.unwrap();
      repo.upsert(&c2).await.unwrap();

      let result = repo.all().await.unwrap();
      assert_eq!(result.len(), 2);
      assert_eq!(*result[0].id(), 1002);
      assert_eq!(*result[1].id(), 1001);
    }
  }

  mod delete {
    use super::*;

    #[tokio::test]
    async fn delete_removes_the_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let character = pod_model::Character::new(1001, "To Delete");
      repo.upsert(&character).await.unwrap();
      assert!(repo.find(1001).await.unwrap().is_some());

      repo.delete(1001).await.unwrap();
      assert!(repo.find(1001).await.unwrap().is_none());
    }
  }

  mod update_sort_orders {
    use super::*;

    #[tokio::test]
    async fn updates_sort_order_for_each_pair() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let c1 = pod_model::Character::new(1001, "Alpha");
      let c2 = pod_model::Character::new(1002, "Beta");
      repo.upsert(&c1).await.unwrap();
      repo.upsert(&c2).await.unwrap();

      repo.update_sort_orders(&[(1001, 5), (1002, 3)]).await.unwrap();

      let all = repo.all().await.unwrap();
      assert_eq!(*all[0].id(), 1002);
      assert_eq!(*all[0].sort_order(), 3);
      assert_eq!(*all[1].id(), 1001);
      assert_eq!(*all[1].sort_order(), 5);
    }
  }

  mod snoozed_mail {
    use super::*;

    #[tokio::test]
    async fn expired_snoozed_mails_returns_only_past_deadline() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      let repo = Repo::new(&db);

      repo.upsert_snoozed_mail(1, 100, "2025-01-01T00:00:00Z").await.unwrap();
      repo.upsert_snoozed_mail(1, 200, "2099-01-01T00:00:00Z").await.unwrap();

      let expired = repo.expired_snoozed_mails("2025-06-01T00:00:00Z").await.unwrap();
      assert_eq!(expired.len(), 1);
      assert_eq!(expired[0].mail_id, 100);
    }

    #[tokio::test]
    async fn delete_snoozed_mail_removes_the_record() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      let repo = Repo::new(&db);

      repo.upsert_snoozed_mail(1, 100, "2025-01-01T00:00:00Z").await.unwrap();
      let all = repo.all_snoozed_mails().await.unwrap();
      assert_eq!(all.len(), 1);

      repo.delete_snoozed_mail(1, 100).await.unwrap();
      let all = repo.all_snoozed_mails().await.unwrap();
      assert!(all.is_empty());
    }

    #[tokio::test]
    async fn all_snoozed_mails_returns_all_records() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alpha").await;
      insert_character(&db, 2, "Beta").await;
      let repo = Repo::new(&db);

      repo.upsert_snoozed_mail(1, 100, "2025-01-01T00:00:00Z").await.unwrap();
      repo.upsert_snoozed_mail(2, 200, "2025-02-01T00:00:00Z").await.unwrap();

      let all = repo.all_snoozed_mails().await.unwrap();
      assert_eq!(all.len(), 2);
    }
  }
}
