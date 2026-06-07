use crate::{
  features::mail::{State, loaders::OutboxIndicator},
  store::{
    Database,
    model::OwnerType,
    repo::{infra, mail},
  },
};

pub(super) fn set_read_payload(character_id: i64, mail_id: i64) -> String {
  format!("{{\"character_id\":{character_id},\"mail_id\":{mail_id},\"read\":true}}")
}

pub(super) fn set_read_dedupe(mail_id: i64) -> String {
  format!("set_read:{mail_id}")
}

pub(super) fn open_target(state: &State, mail_id: i64) -> Option<(i64, i64)> {
  state
    .messages()
    .iter()
    .find(|row| row.mail_id == mail_id && !row.is_read)
    .map(|row| (row.character_id, row.mail_id))
}

pub(super) async fn mark_read_on_open(db: Database, character_id: i64, mail_id: i64) {
  let _ = mail::set_read(&db, character_id, mail_id, true).await;
  let _ = infra::append(
    &db,
    OwnerType::Character,
    character_id,
    "mail.set_read",
    &set_read_payload(character_id, mail_id),
    Some(&set_read_dedupe(mail_id)),
  )
  .await;
}

pub(super) async fn retry_outbox(db: Database, id: i64) -> OutboxIndicator {
  let now = chrono::Utc::now().to_rfc3339();
  let _ = infra::reschedule(&db, id, &now, "retry requested").await;
  super::loaders::load_outbox_indicator(&db).await
}

pub(super) async fn dismiss_outbox(db: Database, id: i64) -> OutboxIndicator {
  let _ = infra::mark_done(&db, id).await;
  super::loaders::load_outbox_indicator(&db).await
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn store_unread(db: &Database, character_id: i64, mail_id: i64) {
    let header = CharacterMail {
      character_id,
      from_id: 95_000_001,
      from_name: "Sender".to_owned(),
      is_read: false,
      mail_id,
      subject: Some("Subject".to_owned()),
      timestamp: "2026-06-01T10:00:00Z".to_owned(),
      ..Default::default()
    };
    let body = CharacterMailBody {
      body: "<p>hi</p>".to_owned(),
      character_id,
      mail_id,
    };
    mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
  }

  #[test]
  fn it_builds_the_handler_payload_and_dedupe_key() {
    assert_eq!(
      set_read_payload(42, 7),
      "{\"character_id\":42,\"mail_id\":7,\"read\":true}"
    );
    assert_eq!(set_read_dedupe(7), "set_read:7");
  }

  #[tokio::test]
  async fn it_flips_the_local_mirror_and_enqueues_the_outbox_write_on_open() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_unread(&db, 42, 7).await;

    mark_read_on_open(db.clone(), 42, 7).await;

    let headers = mail::headers(&db, 42).await.unwrap();
    assert!(headers.iter().find(|h| h.mail_id() == 7).unwrap().is_read());

    let pending = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM outbox WHERE kind = 'mail.set_read' AND status IN ('pending', 'inflight')",
    )
    .fetch_one(&db.0)
    .await
    .unwrap();
    assert_eq!(pending, 1);
  }

  #[tokio::test]
  async fn it_collapses_a_repeated_open_onto_one_outbox_row() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_unread(&db, 42, 7).await;

    mark_read_on_open(db.clone(), 42, 7).await;
    mark_read_on_open(db.clone(), 42, 7).await;

    let rows = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind = 'mail.set_read'")
      .fetch_one(&db.0)
      .await
      .unwrap();
    assert_eq!(rows, 1, "the per-mail dedupe key collapses the repeated mark-read");
  }

  #[tokio::test]
  async fn it_dismisses_a_failed_row_off_the_indicator() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    let row = infra::append(
      &db,
      OwnerType::Character,
      42,
      "mail.set_read",
      &set_read_payload(42, 7),
      Some(&set_read_dedupe(7)),
    )
    .await
    .unwrap();
    infra::mark_failed(&db, row.id(), "boom").await.unwrap();
    assert_eq!(super::super::loaders::load_outbox_indicator(&db).await.failed.len(), 1);

    let indicator = dismiss_outbox(db.clone(), row.id()).await;

    assert!(indicator.failed.is_empty());
  }

  #[tokio::test]
  async fn it_retries_a_failed_row_back_to_drainable() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    let row = infra::append(
      &db,
      OwnerType::Character,
      42,
      "mail.set_read",
      &set_read_payload(42, 7),
      Some(&set_read_dedupe(7)),
    )
    .await
    .unwrap();
    infra::mark_failed(&db, row.id(), "boom").await.unwrap();

    let indicator = retry_outbox(db.clone(), row.id()).await;

    assert!(indicator.failed.is_empty());
    assert_eq!(indicator.pending, 1);
  }

  #[tokio::test]
  async fn it_drops_the_unified_count_on_open_and_reconciles_idempotently() {
    use crate::store::repo::mail;

    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_unread(&db, 42, 7).await;
    store_unread(&db, 42, 8).await;
    let now = chrono::Utc::now().to_rfc3339();
    assert_eq!(
      mail::visible_unified_unread_count(&db, &now).await.unwrap(),
      2,
      "both unread mails count toward the rail dot"
    );

    mark_read_on_open(db.clone(), 42, 7).await;
    assert_eq!(
      mail::visible_unified_unread_count(&db, &now).await.unwrap(),
      1,
      "opening the unread mail drops the count right away"
    );
    let enqueued = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM outbox WHERE kind = 'mail.set_read' AND status IN ('pending', 'inflight')",
    )
    .fetch_one(&db.0)
    .await
    .unwrap();
    assert_eq!(enqueued, 1, "exactly one mark-read is enqueued through the outbox");

    mail::set_read(&db, 42, 7, true).await.unwrap();
    assert_eq!(
      mail::visible_unified_unread_count(&db, &now).await.unwrap(),
      1,
      "the reconcile is idempotent: the count stays at one"
    );
  }

  #[tokio::test]
  async fn it_excludes_a_snoozed_unread_mail_from_the_unified_count_until_it_wakes() {
    use crate::store::repo::mail;

    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    store_unread(&db, 42, 7).await;
    let now = chrono::Utc::now().to_rfc3339();
    assert_eq!(mail::visible_unified_unread_count(&db, &now).await.unwrap(), 1);

    mail::upsert_snoozed_mail(&db, 42, 7, "2099-01-01T00:00:00Z")
      .await
      .unwrap();
    assert_eq!(
      mail::visible_unified_unread_count(&db, &now).await.unwrap(),
      0,
      "a snoozed unread mail is excluded from the count"
    );

    mail::delete_snoozed_mail(&db, 42, 7).await.unwrap();
    assert_eq!(
      mail::visible_unified_unread_count(&db, &now).await.unwrap(),
      1,
      "waking the snooze restores the count"
    );
  }
}
