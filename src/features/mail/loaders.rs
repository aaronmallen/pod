use std::{collections::HashMap, path::PathBuf, sync::Arc};

use chrono::Utc;

use crate::{
  clients::{esi, eve_image, eve_sso},
  store::{
    Database, images,
    model::{CharacterMailLabel, OwnerType, character_mail_view::UnifiedMail, mail_overlay_state::MailOverlayState},
    repo::{character, mail, org},
  },
};

const MAX_RECIPIENT_RESULTS: usize = 20;

pub async fn search_recipients(
  db: Database,
  esi: Arc<esi::Client>,
  eve_image: Arc<eve_image::Client>,
  sso: Arc<eve_sso::Client>,
  owner_id: i64,
  query: String,
) -> Vec<(i64, String)> {
  let grant = match crate::sync::token::fresh_token(&db, &sso, owner_id, OwnerType::Character).await {
    Ok(Some(grant)) => grant,
    Ok(None) => return Vec::new(),
    Err(error) => {
      tracing::warn!(target: "pod::mail", %error, "recipient search: no usable token");
      return Vec::new();
    }
  };

  let characters = resolve_recipient_characters(&esi, &grant, &query).await;
  cache_recipient_portraits(&eve_image, characters.iter().map(|(id, _)| *id)).await;
  characters
}

async fn resolve_recipient_characters(esi: &esi::Client, grant: &eve_sso::Grant, query: &str) -> Vec<(i64, String)> {
  let result = match esi.universe().search(query, grant).await {
    Ok(result) => result,
    Err(error) => {
      tracing::warn!(target: "pod::mail", %error, query = %query, "recipient search failed");
      return Vec::new();
    }
  };

  let ids: Vec<i64> = result.character.into_iter().take(MAX_RECIPIENT_RESULTS).collect();
  if ids.is_empty() {
    return Vec::new();
  }

  match esi.universe().names(&ids).await {
    Ok(names) => names
      .into_iter()
      .filter(|record| record.category == "character")
      .map(|record| (record.id, record.name))
      .collect(),
    Err(error) => {
      tracing::warn!(target: "pod::mail", %error, "recipient name resolution failed");
      Vec::new()
    }
  }
}

async fn cache_recipient_portraits(eve_image: &eve_image::Client, ids: impl Iterator<Item = i64>) {
  let store = images::default_store();
  for id in ids {
    let path = store.character_portrait_path(id);
    if path.exists() {
      continue;
    }
    let url = eve_image.character_portrait_url(id, images::PORTRAIT_SIZE);
    if let Ok(bytes) = eve_image.fetch(&url).await {
      let _ = store.write(&path, &bytes);
    }
  }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterPilot {
  pub corp: String,
  pub id: i64,
  pub name: String,
  pub portrait: Option<PathBuf>,
  pub unread: i64,
}

pub(super) async fn load_roster(db: &Database) -> Vec<RosterPilot> {
  let characters = character::all_owned(db).await.unwrap_or_default();
  let mut roster = Vec::with_capacity(characters.len());
  for character in &characters {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|c| c.ticker().to_owned())
      .unwrap_or_default();
    let unread = mail::unread_count(db, character.id()).await.unwrap_or(0);
    let portrait_path = images::default_store().character_portrait_path(character.id());
    roster.push(RosterPilot {
      corp,
      id: character.id(),
      name: character.name().to_owned(),
      portrait: portrait_path.exists().then_some(portrait_path),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderLabel {
  pub label_id: i64,
  pub name: String,
  pub unread: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderPaneData {
  pub labels: Vec<FolderLabel>,
  pub standard_counts: StandardFolderCounts,
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
    .map(|label: &CharacterMailLabel| FolderLabel {
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutboxIndicator {
  pub pending: i64,
  pub failed: Vec<FailedMutation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedMutation {
  pub id: i64,
  pub kind: String,
  pub last_error: String,
}

pub(super) async fn load_outbox_indicator(db: &Database) -> OutboxIndicator {
  let pending = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM outbox WHERE kind LIKE 'mail.%' AND status IN ('pending', 'inflight')",
  )
  .fetch_one(&db.0)
  .await
  .unwrap_or(0);

  let failed = sqlx::query_as::<_, (i64, String, Option<String>)>(
    "SELECT id, kind, last_error FROM outbox WHERE kind LIKE 'mail.%' AND status = 'failed' ORDER BY updated_at DESC",
  )
  .fetch_all(&db.0)
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

  #[test]
  fn it_strips_tags_and_collapses_whitespace_into_a_one_line_snippet() {
    assert_eq!(strip_html_snippet("<p>Form up   at\n\nJita.</p>"), "Form up at Jita.");
    assert_eq!(strip_html_snippet("<b>bold</b> text"), "bold text");
    assert_eq!(strip_html_snippet(""), "");
    assert_eq!(strip_html_snippet("<br/>"), "");
  }

  #[tokio::test]
  async fn it_resolves_search_hits_to_character_id_name_pairs() {
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use crate::clients::{eve_sso::Grant, http};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
      .and(path("/characters/42/search/"))
      .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"character":[95,96]}"#, "application/json"))
      .mount(&server)
      .await;
    Mock::given(method("POST"))
      .and(path("/universe/names/"))
      .respond_with(ResponseTemplate::new(200).set_body_raw(
        r#"[{"id":95,"name":"Vex","category":"character"},{"id":96,"name":"A Corp","category":"corporation"}]"#,
        "application/json",
      ))
      .mount(&server)
      .await;
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db)).build();
    let esi = esi::Client::with_base_url(http, server.uri());
    let grant = Grant::new_test("tok", 42);

    let results = resolve_recipient_characters(&esi, &grant, "Vex").await;

    assert_eq!(results, vec![(95, "Vex".to_owned())]);
  }

  #[tokio::test]
  async fn it_yields_no_recipients_when_the_sender_has_no_usable_token() {
    use crate::clients::http;

    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = Arc::new(esi::Client::builder(http.clone()).user_agent("test").build().unwrap());
    let eve_image = Arc::new(eve_image::Client::new(http.clone()));
    let sso = Arc::new(eve_sso::Client::new(http, "test-client"));

    let results = search_recipients(db, esi, eve_image, sso, 999, "Vex".to_owned()).await;

    assert!(results.is_empty());
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
  async fn it_loads_the_folder_pane_with_labels_and_standard_counts() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    store_mail(&db, 1, 95_000_001, false).await;
    store_mail(&db, 2, 95_000_001, false).await;
    store_mail(&db, 3, 95_000_001, false).await;
    store_mail(&db, 4, 95_000_001, false).await;
    store_mail(&db, 5, 95_000_001, false).await;

    mail::set_triage(&db, CHAR, 2, true, false).await.unwrap();
    mail::assign_folder(&db, CHAR, 3, "archive", None, false).await.unwrap();
    let until = (Utc::now() + chrono::Duration::days(1)).to_rfc3339();
    mail::upsert_snoozed_mail(&db, CHAR, 4, &until).await.unwrap();
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
        mail_id: 5,
      }],
    )
    .await
    .unwrap();

    let data = load_folder_pane(&db, CHAR).await;

    assert_eq!(data.labels.len(), 1);
    assert_eq!(data.labels[0].name, "Fleet");
    assert_eq!(data.labels[0].unread, 1);
    assert_eq!(data.standard_counts.starred, 1);
    assert_eq!(data.standard_counts.archive, 1);
    assert_eq!(data.standard_counts.snoozed, 1);
    use crate::features::mail::StandardFolder;
    assert_eq!(data.standard_counts.unread_for(StandardFolder::Starred), 1);
    assert_eq!(data.standard_counts.unread_for(StandardFolder::Sent), 0);
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
  async fn it_loads_headers_overlays_and_the_unified_stream() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, CHAR).await;
    store_mail(&db, 1, 95_000_001, false).await;
    mail::set_triage(&db, CHAR, 1, true, false).await.unwrap();

    assert_eq!(load_headers(&db, CHAR).await.len(), 1);
    assert_eq!(load_overlays(&db, CHAR).await.len(), 1);
    assert_eq!(load_unified(&db).await.len(), 1);
    assert_eq!(load_unified_unread(&db).await, 1);
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
}
