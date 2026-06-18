use std::collections::HashMap;

use chrono::Utc;

use crate::store::{
  Database, images,
  model::{CharacterMailLabel, OwnerType, character_mail_view::UnifiedMail, mail_overlay_state::MailOverlayState},
  repo::{character, infra, mail, org},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailedMutation {
  pub id: i64,
  pub kind: String,
  pub last_error: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderLabel {
  pub color: Option<String>,
  pub label_id: i64,
  pub name: String,
  pub unread: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FolderPaneData {
  pub labels: Vec<FolderLabel>,
  pub standard_counts: StandardFolderCounts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageLabel {
  pub color: Option<String>,
  pub name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OutboxIndicator {
  pub failed: Vec<FailedMutation>,
  pub pending: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterPilot {
  pub corp: String,
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub name: String,
  pub portrait: images::ImageState,
  pub unread: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StandardFolderCounts {
  pub archive: i64,
  pub drafts: i64,
  pub inbox: i64,
  pub sent: i64,
  pub snoozed: i64,
  pub starred: i64,
  pub trash: i64,
}

impl StandardFolderCounts {
  pub fn unread_for(&self, folder: super::StandardFolder) -> i64 {
    use super::StandardFolder;
    match folder {
      StandardFolder::Archive => self.archive,
      StandardFolder::Drafts => self.drafts,
      StandardFolder::Inbox => self.inbox,
      StandardFolder::Sent => self.sent,
      StandardFolder::Snoozed => self.snoozed,
      StandardFolder::Starred => self.starred,
      StandardFolder::Trash => self.trash,
    }
  }
}

pub(super) fn resolve_sender_portrait(sender_id: i64) -> images::ImageState {
  images::resolve(
    &images::default_store(),
    images::ImageKind::CharacterPortrait,
    sender_id,
  )
}

pub(super) fn strip_html_snippet(html: &str) -> String {
  let mut out = String::with_capacity(html.len());
  let mut in_tag = false;
  for ch in html.chars() {
    match ch {
      '<' => in_tag = true,
      '>' => in_tag = false,
      _ if !in_tag => out.push(ch),
      _ => {}
    }
  }
  out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) async fn load_roster(db: &Database) -> Vec<RosterPilot> {
  let characters = character::all_owned(db).await.unwrap_or_default();
  let credentials = infra::all(db).await.unwrap_or_default();
  let scopes_by_id: std::collections::HashMap<i64, Option<String>> = credentials
    .into_iter()
    .filter(|cred| cred.owner_type() == OwnerType::Character)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();

  let mut roster = Vec::with_capacity(characters.len());
  for character in &characters {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|c| c.ticker().to_owned())
      .unwrap_or_default();
    let unread = mail::unread_count(db, character.id()).await.unwrap_or(0);
    let portrait = images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      character.id(),
    );
    roster.push(RosterPilot {
      corp,
      granted_scopes: scopes_by_id.get(&character.id()).cloned().flatten(),
      id: character.id(),
      name: character.name().to_owned(),
      portrait,
      unread,
    });
  }
  roster
}

pub(super) async fn load_unified(db: &Database) -> Vec<UnifiedMail> {
  mail::unified(db).await.unwrap_or_default()
}

pub(super) async fn load_headers(db: &Database, character_id: i64) -> Vec<crate::store::model::CharacterMail> {
  mail::headers(db, character_id).await.unwrap_or_default()
}

pub(super) async fn load_overlays(db: &Database, character_id: i64) -> HashMap<i64, MailOverlayState> {
  mail::all_overlay_states(db, character_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|state| (state.mail_id, state))
    .collect()
}

pub(super) async fn load_unified_unread(db: &Database) -> i64 {
  let now = Utc::now().to_rfc3339();
  mail::visible_unified_unread_count(db, &now).await.unwrap_or(0)
}

pub(super) async fn resolve_message_labels(db: &Database, character_id: i64, label_ids: &[i64]) -> Vec<MessageLabel> {
  if label_ids.is_empty() {
    return Vec::new();
  }
  let catalog = mail::labels(db, character_id).await.unwrap_or_default();
  label_ids
    .iter()
    .filter(|id| !super::labels::is_system_label(**id))
    .filter_map(|id| {
      catalog
        .iter()
        .find(|label| label.label_id() == *id)
        .map(|label| MessageLabel {
          color: label.color().clone(),
          name: label.name().to_owned(),
        })
    })
    .collect()
}

pub(super) async fn load_folder_pane(db: &Database, character_id: i64) -> FolderPaneData {
  let now = Utc::now().to_rfc3339();

  let catalog = mail::labels(db, character_id).await.unwrap_or_default();
  let unread_by_label: HashMap<i64, i64> = mail::visible_unread_counts_by_label(db, character_id, &now)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();
  let labels = catalog
    .iter()
    .filter(|label| !super::labels::is_system_label(label.label_id()))
    .map(|label: &CharacterMailLabel| FolderLabel {
      color: label.color().clone(),
      label_id: label.label_id(),
      name: label.name().to_owned(),
      unread: unread_by_label.get(&label.label_id()).copied().unwrap_or(0),
    })
    .collect();

  FolderPaneData {
    labels,
    standard_counts: load_standard_folder_counts(db, character_id, &now).await,
  }
}

pub(super) async fn load_folder_pane_unified(db: &Database, roster: &[RosterPilot]) -> FolderPaneData {
  let now = Utc::now().to_rfc3339();
  let mut labels: Vec<FolderLabel> = Vec::new();
  let mut standard_counts = StandardFolderCounts::default();

  for pilot in roster {
    let per = load_folder_pane(db, pilot.id).await;
    for label in per.labels {
      if let Some(existing) = labels.iter_mut().find(|l| l.name == label.name) {
        existing.unread += label.unread;
      } else {
        labels.push(label);
      }
    }
    let counts = load_standard_folder_counts(db, pilot.id, &now).await;
    standard_counts.archive += counts.archive;
    standard_counts.drafts += counts.drafts;
    standard_counts.inbox += counts.inbox;
    standard_counts.sent += counts.sent;
    standard_counts.snoozed += counts.snoozed;
    standard_counts.starred += counts.starred;
    standard_counts.trash += counts.trash;
  }

  FolderPaneData {
    labels,
    standard_counts,
  }
}

async fn load_standard_folder_counts(db: &Database, character_id: i64, now: &str) -> StandardFolderCounts {
  let headers = mail::headers(db, character_id).await.unwrap_or_default();
  let unread: HashMap<i64, bool> = headers
    .iter()
    .map(|h| (h.mail_id(), !h.is_read() && h.from_id() != character_id))
    .collect();
  let count_unread = |ids: Vec<i64>| ids.iter().filter(|id| *unread.get(id).unwrap_or(&false)).count() as i64;

  StandardFolderCounts {
    archive: count_unread(
      mail::folder_mail_ids(db, character_id, "archive")
        .await
        .unwrap_or_default(),
    ),
    drafts: 0,
    inbox: mail::visible_unread_count(db, character_id, now).await.unwrap_or(0),
    sent: 0,
    snoozed: count_snoozed_unread(db, character_id, now, &unread).await,
    starred: count_unread(mail::starred_mail_ids(db, character_id).await.unwrap_or_default()),
    trash: count_unread(
      mail::folder_mail_ids(db, character_id, "trash")
        .await
        .unwrap_or_default(),
    ),
  }
}

async fn count_snoozed_unread(db: &Database, character_id: i64, now: &str, unread: &HashMap<i64, bool>) -> i64 {
  mail::all_snoozed_mails(db, character_id)
    .await
    .unwrap_or_default()
    .iter()
    .filter(|s| s.snooze_until().as_str() > now && *unread.get(&s.mail_id()).unwrap_or(&false))
    .count() as i64
}

pub(super) async fn load_outbox_indicator(db: &Database) -> OutboxIndicator {
  let pending = infra::outbox_pending_count_by_kind(db, "mail.").await.unwrap_or(0);

  let failed = infra::outbox_failed_by_kind(db, "mail.")
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, kind, last_error)| FailedMutation {
      id,
      kind,
      last_error: last_error.unwrap_or_else(|| "unknown error".to_owned()),
    })
    .collect();

  OutboxIndicator {
    pending,
    failed,
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store::{
    self,
    model::{
      Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, CharacterMailLabel,
      CharacterMailLabelMembership, Corporation, Gender, OwnerType, Race,
    },
    repo::{character, infra, mail},
  };

  const CHAR: i64 = 42;

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
    infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
      .await
      .unwrap();
  }

  async fn store_mail(db: &Database, mail_id: i64, from_id: i64, is_read: bool) {
    let header = CharacterMail {
      character_id: CHAR,
      from_id,
      from_name: "Vex Voronova".to_owned(),
      is_read,
      mail_id,
      subject: Some(format!("Subject {mail_id}")),
      timestamp: "2026-06-01T10:00:00Z".to_owned(),
      ..Default::default()
    };
    let body = CharacterMailBody {
      body: "<p>Form up.</p>".to_owned(),
      character_id: CHAR,
      mail_id,
    };
    mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
  }

  #[tokio::test]
  async fn it_hides_system_labels_from_the_folder_pane_and_chips() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    store_mail(&db, 1, 95_000_001, false).await;
    // Mirror sync: synthesized system labels live alongside a real user label.
    mail::replace_labels_for_character(
      &db,
      CHAR,
      &[
        CharacterMailLabel {
          character_id: CHAR,
          color: None,
          label_id: 1,
          name: "Inbox".to_owned(),
        },
        CharacterMailLabel {
          character_id: CHAR,
          color: None,
          label_id: 8,
          name: "Alliance".to_owned(),
        },
        CharacterMailLabel {
          character_id: CHAR,
          color: Some("#ff6600".to_owned()),
          label_id: 7000,
          name: "Fleet".to_owned(),
        },
      ],
    )
    .await
    .unwrap();

    let data = load_folder_pane(&db, CHAR).await;
    assert_eq!(
      data.labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
      vec!["Fleet"]
    );

    // A mail tagged with both a system label and a user label only shows the user chip.
    let resolved = resolve_message_labels(&db, CHAR, &[1, 8, 7000]).await;
    assert_eq!(
      resolved,
      vec![MessageLabel {
        color: Some("#ff6600".to_owned()),
        name: "Fleet".to_owned(),
      }]
    );
  }

  #[tokio::test]
  async fn it_loads_headers_overlays_and_the_unified_stream() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    store_mail(&db, 1, 95_000_001, false).await;
    mail::set_triage(&db, CHAR, 1, true).await.unwrap();

    assert_eq!(load_headers(&db, CHAR).await.len(), 1);
    assert_eq!(load_overlays(&db, CHAR).await.len(), 1);
    assert_eq!(load_unified(&db).await.len(), 1);
    assert_eq!(load_unified_unread(&db).await, 1);
  }

  #[tokio::test]
  async fn it_loads_the_folder_pane_with_labels_and_standard_counts() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    store_mail(&db, 1, 95_000_001, false).await;
    store_mail(&db, 2, 95_000_001, false).await;
    store_mail(&db, 3, 95_000_001, false).await;
    store_mail(&db, 4, 95_000_001, false).await;
    store_mail(&db, 5, 95_000_001, false).await;

    mail::set_triage(&db, CHAR, 2, true).await.unwrap();
    mail::assign_folder(&db, CHAR, 3, "archive", None, false, "2026-06-01T00:00:00Z")
      .await
      .unwrap();
    let until = (Utc::now() + chrono::Duration::days(1)).to_rfc3339();
    mail::upsert_snoozed_mail(&db, CHAR, 4, &until).await.unwrap();
    mail::replace_labels_for_character(
      &db,
      CHAR,
      &[CharacterMailLabel {
        character_id: CHAR,
        color: Some("#ff6600".to_owned()),
        label_id: 7000,
        name: "Fleet".to_owned(),
      }],
    )
    .await
    .unwrap();
    mail::replace_membership_for_character(
      &db,
      CHAR,
      &[CharacterMailLabelMembership {
        character_id: CHAR,
        label_id: 7000,
        mail_id: 5,
      }],
    )
    .await
    .unwrap();

    let data = load_folder_pane(&db, CHAR).await;

    assert_eq!(data.labels.len(), 1);
    assert_eq!(data.labels[0].name, "Fleet");
    assert_eq!(data.labels[0].color, Some("#ff6600".to_owned()));
    assert_eq!(data.labels[0].unread, 1);
    assert_eq!(data.standard_counts.starred, 1);
    assert_eq!(data.standard_counts.archive, 1);
    assert_eq!(data.standard_counts.snoozed, 1);
    use crate::features::mail::StandardFolder;
    assert_eq!(data.standard_counts.unread_for(StandardFolder::Starred), 1);
    assert_eq!(data.standard_counts.unread_for(StandardFolder::Sent), 0);
  }

  #[tokio::test]
  async fn it_loads_the_outbox_indicator_with_pending_and_failed_rows() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    infra::append(&db, OwnerType::Character, CHAR, "mail.send", "{}", Some("a"))
      .await
      .unwrap();
    let failed = infra::append(&db, OwnerType::Character, CHAR, "mail.set_read", "{}", Some("b"))
      .await
      .unwrap();
    infra::mark_failed(&db, failed.id(), "boom").await.unwrap();

    let indicator = load_outbox_indicator(&db).await;

    assert_eq!(indicator.pending, 1);
    assert_eq!(indicator.failed.len(), 1);
    assert_eq!(indicator.failed[0].last_error, "boom");
  }

  #[tokio::test]
  async fn it_loads_the_owned_roster_with_unread_counts() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    store_mail(&db, 1, 95_000_001, false).await;

    let roster = load_roster(&db).await;

    assert_eq!(roster.len(), 1);
    assert_eq!(roster[0].id, CHAR);
    assert_eq!(roster[0].corp, "TSC");
    assert_eq!(roster[0].unread, 1);
  }

  #[tokio::test]
  async fn it_merges_per_character_folder_data_for_all_inboxes() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    store_mail(&db, 1, 95_000_001, false).await;
    mail::replace_labels_for_character(
      &db,
      CHAR,
      &[CharacterMailLabel {
        character_id: CHAR,
        color: None,
        label_id: 7000,
        name: "Fleet".to_owned(),
      }],
    )
    .await
    .unwrap();
    mail::replace_membership_for_character(
      &db,
      CHAR,
      &[CharacterMailLabelMembership {
        character_id: CHAR,
        label_id: 7000,
        mail_id: 1,
      }],
    )
    .await
    .unwrap();
    let roster = load_roster(&db).await;

    let data = load_folder_pane_unified(&db, &roster).await;

    assert_eq!(data.labels.len(), 1);
    assert_eq!(data.labels[0].name, "Fleet");
  }

  #[tokio::test]
  async fn it_resolves_membership_ids_to_named_colored_labels_and_drops_unknown_ids() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    mail::replace_labels_for_character(
      &db,
      CHAR,
      &[
        CharacterMailLabel {
          character_id: CHAR,
          color: Some("#ff6600".to_owned()),
          label_id: 7000,
          name: "Fleet".to_owned(),
        },
        CharacterMailLabel {
          character_id: CHAR,
          color: None,
          label_id: 7001,
          name: "Trade".to_owned(),
        },
      ],
    )
    .await
    .unwrap();

    let resolved = resolve_message_labels(&db, CHAR, &[7000, 7001, 9999]).await;

    assert_eq!(
      resolved,
      vec![
        MessageLabel {
          color: Some("#ff6600".to_owned()),
          name: "Fleet".to_owned(),
        },
        MessageLabel {
          color: None,
          name: "Trade".to_owned(),
        },
      ]
    );
    assert!(resolve_message_labels(&db, CHAR, &[]).await.is_empty());
  }

  #[test]
  fn it_strips_tags_and_collapses_whitespace_into_a_one_line_snippet() {
    assert_eq!(strip_html_snippet("<p>Form up   at\n\nJita.</p>"), "Form up at Jita.");
    assert_eq!(strip_html_snippet("<b>bold</b> text"), "bold text");
    assert_eq!(strip_html_snippet(""), "");
    assert_eq!(strip_html_snippet("<br/>"), "");
  }
}
