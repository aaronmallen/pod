use super::loaders::strip_html_snippet;
use crate::store::{
  Database,
  model::{DraftInput, MailDraft},
  repo::mail,
};

const SNIPPET_MAX_CHARS: usize = 90;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftRow {
  pub character_id: i64,
  pub id: i64,
  pub recipients: String,
  pub snippet: String,
  pub subject: String,
}

impl DraftRow {
  fn from_model(row: &MailDraft) -> Self {
    let recipients = recipient_summary(row);
    DraftRow {
      character_id: row.character_id(),
      id: row.id(),
      recipients,
      snippet: snippet_preview(&strip_html_snippet(row.body())),
      subject: subject_or_no_subject(row.subject()),
    }
  }
}

pub(super) async fn delete(db: Database, id: i64) {
  let _ = mail::delete_draft(&db, id).await;
}

pub(super) async fn load_rows(db: Database, character_id: i64) -> Vec<DraftRow> {
  mail::list_drafts_for_character(&db, character_id)
    .await
    .unwrap_or_default()
    .iter()
    .map(DraftRow::from_model)
    .collect()
}

/// Upserts the live compose into its `mail_drafts` row, returning the row id so it can be threaded
/// back onto the open compose. Returns `None` for a blank draft, which is never persisted.
pub(super) async fn persist(db: Database, id: Option<i64>, input: DraftInput) -> Option<i64> {
  mail::upsert_draft(&db, id, &input).await.ok()
}

fn deserialize_names(json: &str) -> Vec<String> {
  serde_json::from_str::<Vec<super::compose::Recipient>>(json)
    .unwrap_or_default()
    .into_iter()
    .map(|recipient| recipient.name)
    .collect()
}

fn recipient_summary(row: &MailDraft) -> String {
  let mut names: Vec<String> = deserialize_names(row.recipients_to());
  names.extend(deserialize_names(row.recipients_cc()));
  if names.is_empty() {
    "(no recipients)".to_owned()
  } else {
    names.join(", ")
  }
}

fn snippet_preview(body: &str) -> String {
  if body.chars().count() <= SNIPPET_MAX_CHARS {
    return body.to_owned();
  }
  let cutoff: String = body.chars().take(SNIPPET_MAX_CHARS).collect();
  let trimmed = match cutoff.rfind(char::is_whitespace) {
    Some(pos) => cutoff[..pos].to_owned(),
    None => cutoff,
  };
  format!("{trimmed}\u{2026}")
}

fn subject_or_no_subject(subject: &str) -> String {
  if subject.trim().is_empty() {
    "(no subject)".to_owned()
  } else {
    subject.to_owned()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &store::Database, id: i64) {
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

  fn input(character_id: i64) -> DraftInput {
    DraftInput {
      body: "<b>Form up</b> at Jita.".to_owned(),
      character_id,
      kind: "New".to_owned(),
      quote: None,
      recipients_cc: "[]".to_owned(),
      recipients_to: r#"[{"id":95000001,"name":"Vex Voronova","recipient_type":"character"}]"#.to_owned(),
      subject: "CTA".to_owned(),
    }
  }

  mod load_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_projects_persisted_drafts_into_rows_with_a_stripped_snippet() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      mail::upsert_draft(&db, None, &input(42)).await.unwrap();

      let rows = load_rows(db.clone(), 42).await;

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].subject, "CTA");
      assert_eq!(rows[0].snippet, "Form up at Jita.");
      assert_eq!(rows[0].recipients, "Vex Voronova");
    }

    #[tokio::test]
    async fn it_labels_an_empty_subject_and_no_recipients() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut blank = input(42);
      blank.subject = String::new();
      blank.recipients_to = "[]".to_owned();
      blank.body = String::new();
      mail::upsert_draft(&db, None, &blank).await.unwrap();

      let rows = load_rows(db.clone(), 42).await;

      assert_eq!(rows[0].subject, "(no subject)");
      assert_eq!(rows[0].recipients, "(no recipients)");
    }
  }
}
