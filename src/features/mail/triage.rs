use super::StandardFolder;
use crate::store::{Database, repo::mail};

pub(super) async fn toggle_star(db: Database, character_id: i64, mail_id: i64) {
  let overlay = mail::overlay_state(&db, character_id, mail_id)
    .await
    .unwrap_or_default();
  set_triage(&db, character_id, mail_id, !overlay.is_starred, overlay.is_pinned).await;
}

pub(super) async fn toggle_pin(db: Database, character_id: i64, mail_id: i64) {
  let overlay = mail::overlay_state(&db, character_id, mail_id)
    .await
    .unwrap_or_default();
  set_triage(&db, character_id, mail_id, overlay.is_starred, !overlay.is_pinned).await;
}

async fn set_triage(db: &Database, character_id: i64, mail_id: i64, star: bool, pin: bool) {
  if star || pin {
    let _ = mail::set_triage(db, character_id, mail_id, star, pin).await;
  } else {
    let _ = mail::clear_triage(db, character_id, mail_id).await;
  }
}

pub(super) async fn archive(db: Database, character_id: i64, mail_id: i64) {
  let _ = mail::assign_folder(&db, character_id, mail_id, "archive", None, false).await;
}

pub(super) async fn trash(db: Database, character_id: i64, mail_id: i64) {
  let _ = mail::assign_folder(&db, character_id, mail_id, "trash", None, false).await;
}

/// Move a dragged message into one of the standard boxes as a pure local move (no ESI write).
/// Any active snooze is cleared first so the mail surfaces in its new box immediately. Inbox is the
/// "no overlay" state, so it clears any folder assignment; Archive/Trash file the mail into that box.
pub(super) async fn move_to_box(db: Database, character_id: i64, mail_id: i64, folder: StandardFolder) {
  let _ = mail::delete_snoozed_mail(&db, character_id, mail_id).await;
  match folder {
    StandardFolder::Inbox => {
      let _ = mail::clear_folder(&db, character_id, mail_id).await;
    }
    StandardFolder::Archive => {
      let _ = mail::assign_folder(&db, character_id, mail_id, "archive", None, false).await;
    }
    StandardFolder::Trash => {
      let _ = mail::assign_folder(&db, character_id, mail_id, "trash", None, false).await;
    }
    StandardFolder::Drafts | StandardFolder::Sent | StandardFolder::Snoozed | StandardFolder::Starred => {}
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
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

  #[tokio::test]
  async fn it_archives_as_a_pure_local_move_with_no_outbox_write() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;

    archive(db.clone(), 42, 7).await;

    assert_eq!(mail::folder(&db, 42, 7).await.unwrap().unwrap().folder(), "archive");
    let outbox = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox")
      .fetch_one(&db.0)
      .await
      .unwrap();
    assert_eq!(outbox, 0);
  }

  #[tokio::test]
  async fn it_clears_the_triage_row_when_both_flags_drop_to_false() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    mail::set_triage(&db, 42, 7, true, false).await.unwrap();

    toggle_star(db.clone(), 42, 7).await;

    assert!(mail::triage(&db, 42, 7).await.unwrap().is_none());
    assert!(mail::starred_mail_ids(&db, 42).await.unwrap().is_empty());
  }

  #[tokio::test]
  async fn it_toggles_pin_without_clobbering_star() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    mail::set_triage(&db, 42, 7, true, false).await.unwrap();

    toggle_pin(db.clone(), 42, 7).await;

    let row = mail::triage(&db, 42, 7).await.unwrap().unwrap();
    assert!(row.star(), "star is preserved when toggling pin");
    assert!(row.pin());
  }

  #[tokio::test]
  async fn it_toggles_star_without_clobbering_pin() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    mail::set_triage(&db, 42, 7, false, true).await.unwrap();

    toggle_star(db.clone(), 42, 7).await;

    let row = mail::triage(&db, 42, 7).await.unwrap().unwrap();
    assert!(row.star());
    assert!(row.pin(), "pin is preserved when toggling star");
  }

  #[tokio::test]
  async fn it_trashes_as_a_pure_local_move_with_no_outbox_write() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;

    trash(db.clone(), 42, 7).await;

    assert_eq!(mail::folder(&db, 42, 7).await.unwrap().unwrap().folder(), "trash");
    let outbox = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox")
      .fetch_one(&db.0)
      .await
      .unwrap();
    assert_eq!(outbox, 0);
  }

  #[tokio::test]
  async fn it_clears_the_snooze_when_a_snoozed_mail_is_dropped_into_a_box() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    mail::upsert_snoozed_mail(&db, 42, 7, "2099-01-01T00:00:00Z")
      .await
      .unwrap();

    move_to_box(db.clone(), 42, 7, StandardFolder::Archive).await;

    assert!(mail::all_snoozed_mails(&db, 42).await.unwrap().is_empty());
    assert_eq!(mail::folder(&db, 42, 7).await.unwrap().unwrap().folder(), "archive");
  }

  #[tokio::test]
  async fn it_returns_a_trashed_mail_to_the_inbox_by_clearing_its_folder() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    mail::assign_folder(&db, 42, 7, "trash", None, false).await.unwrap();

    move_to_box(db.clone(), 42, 7, StandardFolder::Inbox).await;

    assert!(mail::folder(&db, 42, 7).await.unwrap().is_none());
    let outbox = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox")
      .fetch_one(&db.0)
      .await
      .unwrap();
    assert_eq!(outbox, 0);
  }

  #[tokio::test]
  async fn it_trashes_a_mail_dropped_onto_the_trash_box() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;

    move_to_box(db.clone(), 42, 7, StandardFolder::Trash).await;

    assert_eq!(mail::folder(&db, 42, 7).await.unwrap().unwrap().folder(), "trash");
  }
}
