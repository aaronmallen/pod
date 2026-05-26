//! Repository for character mail headers and snoozed mail.

use pod_model::MailHeader;
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::mail_header::{ActiveModel as HeaderActive, Column as HeaderColumn, Entity as HeaderEntity},
};

/// Repository for character mail header read and write operations.
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

  /// Returns all mail header rows for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn mail_for_character(&self, character_id: i64) -> Result<Vec<MailHeader>, Error> {
    let rows = HeaderEntity::find()
      .filter(HeaderColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(MailHeader::from).collect())
  }

  /// Upserts mail headers for the given character.
  ///
  /// Body and preview fields are not modified by this method — existing cached
  /// content is preserved on conflict.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn upsert_mail_headers(&self, character_id: i64, headers: &[MailHeader]) -> Result<(), Error> {
    for header in headers {
      header.validate()?;
      let active = HeaderActive {
        body: ActiveValue::NotSet,
        character_id: ActiveValue::Set(character_id),
        from_id: ActiveValue::Set(header.from_id),
        id: ActiveValue::NotSet,
        is_read: ActiveValue::Set(header.is_read),
        mail_id: ActiveValue::Set(header.mail_id),
        preview: ActiveValue::NotSet,
        recipients_display: ActiveValue::Set(header.recipients_display.clone()),
        subject: ActiveValue::Set(header.subject.clone()),
        timestamp: ActiveValue::Set(header.timestamp.clone()),
      };
      HeaderEntity::insert(active)
        .on_conflict(
          OnConflict::columns([HeaderColumn::CharacterId, HeaderColumn::MailId])
            .update_columns([
              HeaderColumn::FromId,
              HeaderColumn::IsRead,
              HeaderColumn::RecipientsDisplay,
              HeaderColumn::Subject,
              HeaderColumn::Timestamp,
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

  fn make_header(character_id: i64, mail_id: i64) -> MailHeader {
    MailHeader {
      body: None,
      character_id,
      from_id: Some(90_000_002),
      is_read: false,
      mail_id,
      preview: None,
      recipients_display: "Test Pilot".to_string(),
      subject: "Hello capsuleer".to_string(),
      timestamp: "2024-06-01T12:00:00Z".to_string(),
    }
  }

  mod mail_for_character {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_mail_exists() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.mail_for_character(1).await.unwrap();

      assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_headers_for_the_given_character() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo
        .upsert_mail_headers(1, &[make_header(1, 1001), make_header(1, 1002)])
        .await
        .unwrap();
      repo.upsert_mail_headers(2, &[make_header(2, 2001)]).await.unwrap();

      let result = repo.mail_for_character(1).await.unwrap();

      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_return_mail_for_other_characters() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_mail_headers(1, &[make_header(1, 1001)]).await.unwrap();

      let result = repo.mail_for_character(2).await.unwrap();

      assert_eq!(result.len(), 0);
    }
  }

  mod upsert_mail_headers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_new_headers() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_mail_headers(1, &[make_header(1, 1001)]).await.unwrap();

      let rows = repo.mail_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].mail_id, 1001);
    }

    #[tokio::test]
    async fn it_updates_existing_header_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_mail_headers(1, &[make_header(1, 1001)]).await.unwrap();

      let mut updated = make_header(1, 1001);
      updated.is_read = true;
      updated.subject = "Updated subject".to_string();
      repo.upsert_mail_headers(1, &[updated]).await.unwrap();

      let rows = repo.mail_for_character(1).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert!(rows[0].is_read);
      assert_eq!(rows[0].subject, "Updated subject");
    }

    #[tokio::test]
    async fn it_does_not_overwrite_cached_body_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_mail_headers(1, &[make_header(1, 1001)]).await.unwrap();

      // Manually insert body to simulate a previously-fetched body.
      HeaderEntity::update_many()
        .col_expr(HeaderColumn::Body, sea_orm::sea_query::Expr::value("Cached body text"))
        .filter(HeaderColumn::CharacterId.eq(1_i64))
        .filter(HeaderColumn::MailId.eq(1001_i64))
        .exec(&db)
        .await
        .unwrap();

      // Upsert again — should not wipe the body.
      repo.upsert_mail_headers(1, &[make_header(1, 1001)]).await.unwrap();

      let rows = repo.mail_for_character(1).await.unwrap();
      assert_eq!(rows[0].body.as_deref(), Some("Cached body text"));
    }
  }
}
