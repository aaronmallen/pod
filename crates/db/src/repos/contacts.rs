//! Repository for character contact and contact label persistence.

use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::{
    character_contact::{
      ActiveModel as ContactActive, Column as ContactColumn, Entity as ContactEntity, Model as ContactModel,
    },
    character_contact_label::{
      ActiveModel as LabelActive, Column as LabelColumn, Entity as LabelEntity, Model as LabelModel,
    },
  },
};

/// Repository for character contact and label CRUD operations.
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

  /// Returns all contact rows for the given character.
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<ContactModel>, Error> {
    let rows = ContactEntity::find()
      .filter(ContactColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Returns all contact label rows for the given character.
  pub async fn find_labels_for_character(&self, character_id: i64) -> Result<Vec<LabelModel>, Error> {
    let rows = LabelEntity::find()
      .filter(LabelColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Upserts contacts and labels for the given character using ON CONFLICT DO UPDATE.
  pub async fn upsert_for_character(
    &self,
    character_id: i64,
    contacts: &[ContactModel],
    labels: &[LabelModel],
  ) -> Result<(), Error> {
    for contact in contacts {
      let active = ContactActive {
        character_id: ActiveValue::Set(character_id),
        contact_id: ActiveValue::Set(contact.contact_id),
        contact_name: ActiveValue::Set(contact.contact_name.clone()),
        contact_type: ActiveValue::Set(contact.contact_type.clone()),
        id: ActiveValue::NotSet,
        is_watchlist: ActiveValue::Set(contact.is_watchlist),
        label_ids: ActiveValue::Set(contact.label_ids.clone()),
        standing: ActiveValue::Set(contact.standing),
        synced_at: ActiveValue::Set(contact.synced_at.clone()),
      };
      ContactEntity::insert(active)
        .on_conflict(
          OnConflict::columns([ContactColumn::CharacterId, ContactColumn::ContactId])
            .update_columns([
              ContactColumn::ContactName,
              ContactColumn::ContactType,
              ContactColumn::IsWatchlist,
              ContactColumn::LabelIds,
              ContactColumn::Standing,
              ContactColumn::SyncedAt,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }

    for label in labels {
      let active = LabelActive {
        character_id: ActiveValue::Set(character_id),
        id: ActiveValue::NotSet,
        label_id: ActiveValue::Set(label.label_id),
        label_name: ActiveValue::Set(label.label_name.clone()),
      };
      LabelEntity::insert(active)
        .on_conflict(
          OnConflict::columns([LabelColumn::CharacterId, LabelColumn::LabelId])
            .update_column(LabelColumn::LabelName)
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}
