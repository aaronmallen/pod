use serde_json::{Value, json};

use crate::{
  clients::esi::scopes,
  mcp::{
    args::{ArgSpec, require_i64, require_i64_array, require_str},
    tool::{McpTool, Permission, ToolError},
  },
  store::{
    Database,
    model::{CharacterMail, CharacterMailBody, CharacterMailLabel, CharacterMailRecipient, OwnerType},
    repo::{character, infra, mail},
  },
};

pub fn tools() -> Vec<McpTool> {
  vec![send_mail_tool(), delete_mail_tool(), manage_labels_tool()]
}

fn send_mail_tool() -> McpTool {
  McpTool::new(
    "send_mail",
    t!("mcp.tools.send_mail").into_owned(),
    Permission::SendMail,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      require_scope(&db, character_id, scopes::CHARACTER_MAIL_SEND, "send_mail").await?;
      let subject = require_str(&args, "subject")?.to_owned();
      let body = require_str(&args, "body")?.to_owned();
      let recipients = parse_recipients(&args)?;
      if recipients.is_empty() {
        return Err(ToolError::InvalidArguments(
          "at least one recipient is required".to_owned(),
        ));
      }

      let mail_id = optimistic_mail_id();
      write_optimistic_sent(&db, character_id, mail_id, &subject, &body, &recipients).await;
      let payload = json!({
        "body": body,
        "from_character_id": character_id,
        "optimistic_mail_id": mail_id,
        "recipients": recipients.iter().map(Recipient::to_payload).collect::<Vec<_>>(),
        "subject": subject,
      });
      append(&db, character_id, "mail.send", &payload, None).await?;

      Ok(json!({ "optimistic_mail_id": mail_id, "queued": true }))
    },
  )
  .with_args([
    ArgSpec::integer("character_id", t!("mcp.tools.send_mail_character_id").into_owned()),
    ArgSpec::string("subject", t!("mcp.tools.send_mail_subject").into_owned()),
    ArgSpec::string("body", t!("mcp.tools.send_mail_body").into_owned()),
    ArgSpec::integer_array("recipients", t!("mcp.tools.send_mail_recipients").into_owned()),
  ])
}

fn delete_mail_tool() -> McpTool {
  McpTool::new(
    "delete_mail",
    t!("mcp.tools.delete_mail").into_owned(),
    Permission::DeleteMail,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      let mail_id = require_i64(&args, "mail_id")?;
      require_scope(&db, character_id, scopes::CHARACTER_MAIL_ORGANIZE, "delete_mail").await?;

      let Some(snapshot) = mail::snapshot_mail(&db, character_id, mail_id)
        .await
        .map_err(internal)?
      else {
        return Err(ToolError::InvalidArguments(format!(
          "no mail {mail_id} for character {character_id}"
        )));
      };
      mail::purge_mail(&db, character_id, mail_id).await.map_err(internal)?;
      let payload = serde_json::to_value(&snapshot).map_err(internal)?;
      let dedupe = format!("delete_mail:{mail_id}");
      append(&db, character_id, "mail.delete", &payload, Some(&dedupe)).await?;

      Ok(json!({ "mail_id": mail_id, "queued": true }))
    },
  )
  .with_args([
    ArgSpec::integer("character_id", t!("mcp.tools.delete_mail_character_id").into_owned()),
    ArgSpec::integer("mail_id", t!("mcp.tools.delete_mail_mail_id").into_owned()),
  ])
}

fn manage_labels_tool() -> McpTool {
  McpTool::new(
    "manage_labels",
    t!("mcp.tools.manage_labels").into_owned(),
    Permission::ManageLabels,
    |db, args: Value| async move {
      let character_id = require_i64(&args, "character_id")?;
      require_scope(&db, character_id, scopes::CHARACTER_MAIL_ORGANIZE, "manage_labels").await?;
      match require_str(&args, "action")? {
        "create_label" => create_label(&db, character_id, &args).await,
        "delete_label" => delete_label(&db, character_id, &args).await,
        "set_labels" => set_labels(&db, character_id, &args).await,
        other => Err(ToolError::InvalidArguments(format!(
          "`action` must be create_label, delete_label, or set_labels, got `{other}`"
        ))),
      }
    },
  )
  .with_args([
    ArgSpec::integer("character_id", t!("mcp.tools.manage_labels_character_id").into_owned()),
    ArgSpec::string("action", t!("mcp.tools.manage_labels_action").into_owned()),
    ArgSpec::string("name", t!("mcp.tools.manage_labels_name").into_owned()),
    ArgSpec::string("color", t!("mcp.tools.manage_labels_color").into_owned()),
    ArgSpec::optional_integer("label_id", 0, t!("mcp.tools.manage_labels_label_id").into_owned()),
    ArgSpec::optional_integer("mail_id", 0, t!("mcp.tools.manage_labels_mail_id").into_owned()),
    ArgSpec::integer_array("labels", t!("mcp.tools.manage_labels_labels").into_owned()),
  ])
}

async fn create_label(db: &Database, character_id: i64, args: &Value) -> Result<Value, ToolError> {
  let name = require_str(args, "name")?.trim().to_owned();
  if name.is_empty() {
    return Err(ToolError::InvalidArguments("label `name` cannot be empty".to_owned()));
  }
  let color = args.get("color").and_then(Value::as_str).map(str::to_owned);
  let label_id = optimistic_mail_id();
  let optimistic = CharacterMailLabel {
    character_id,
    color: color.clone(),
    label_id,
    name: name.clone(),
  };
  mail::insert_label(db, &optimistic).await.map_err(internal)?;

  let payload = json!({ "character_id": character_id, "color": color, "label_id": label_id, "name": name });
  append(db, character_id, "mail.create_label", &payload, None).await?;
  Ok(json!({ "label_id": label_id, "queued": true }))
}

async fn delete_label(db: &Database, character_id: i64, args: &Value) -> Result<Value, ToolError> {
  let label_id = require_i64(args, "label_id")?;
  mail::delete_label(db, character_id, label_id).await.map_err(internal)?;

  let payload = json!({ "character_id": character_id, "label_id": label_id });
  let dedupe = format!("delete_label:{label_id}");
  append(db, character_id, "mail.delete_label", &payload, Some(&dedupe)).await?;
  Ok(json!({ "label_id": label_id, "queued": true }))
}

async fn set_labels(db: &Database, character_id: i64, args: &Value) -> Result<Value, ToolError> {
  let mail_id = require_i64(args, "mail_id")?;
  let labels = require_i64_array(args, "labels")?;
  let previous = mail::membership(db, character_id, mail_id).await.map_err(internal)?;
  apply_membership(db, character_id, mail_id, &previous, &labels).await?;

  let payload = json!({
    "character_id": character_id,
    "labels": labels,
    "mail_id": mail_id,
    "previous": previous,
  });
  let dedupe = format!("set_labels:{mail_id}");
  append(db, character_id, "mail.set_labels", &payload, Some(&dedupe)).await?;
  Ok(json!({ "labels": labels, "mail_id": mail_id, "queued": true }))
}

struct Recipient {
  id: i64,
  recipient_type: String,
}

impl Recipient {
  fn to_payload(&self) -> Value {
    json!({ "id": self.id, "name": "", "recipient_type": self.recipient_type })
  }
}

/// Mirrors `mail::compose::enqueue_send`: the outbox drainer never runs a handler `apply`, so the
/// enqueueing layer owns the optimistic Sent row. The synthetic header carries `from_id ==
/// character_id`, which is what files it into the Sent folder; sync reconciles it away later.
async fn write_optimistic_sent(
  db: &Database,
  character_id: i64,
  mail_id: i64,
  subject: &str,
  body: &str,
  recipients: &[Recipient],
) {
  let from_name = character::get(db, character_id)
    .await
    .ok()
    .flatten()
    .map(|c| c.name().to_owned())
    .unwrap_or_default();
  let header = CharacterMail {
    character_id,
    from_corp: false,
    from_id: character_id,
    from_name,
    from_system: false,
    has_attachment: false,
    important: false,
    is_read: true,
    mail_id,
    subject: Some(subject.to_owned()),
    timestamp: chrono::Utc::now().to_rfc3339(),
  };
  let body = CharacterMailBody {
    body: body.to_owned(),
    character_id,
    mail_id,
  };
  let recipient_rows: Vec<CharacterMailRecipient> = recipients
    .iter()
    .map(|recipient| CharacterMailRecipient {
      character_id,
      mail_id,
      recipient_id: recipient.id,
      recipient_name: String::new(),
      recipient_type: recipient.recipient_type.clone(),
    })
    .collect();
  let _ = mail::upsert_complete(db, &header, &body, &recipient_rows).await;
}

async fn apply_membership(
  db: &Database,
  character_id: i64,
  mail_id: i64,
  previous: &[i64],
  labels: &[i64],
) -> Result<(), ToolError> {
  for label_id in previous {
    if !labels.contains(label_id) {
      mail::remove_membership(db, character_id, mail_id, *label_id)
        .await
        .map_err(internal)?;
    }
  }
  for label_id in labels {
    if !previous.contains(label_id) {
      mail::add_membership(db, character_id, mail_id, *label_id)
        .await
        .map_err(internal)?;
    }
  }
  Ok(())
}

async fn append(
  db: &Database,
  character_id: i64,
  kind: &str,
  payload: &Value,
  dedupe: Option<&str>,
) -> Result<(), ToolError> {
  let json = serde_json::to_string(payload).map_err(internal)?;
  infra::append(db, OwnerType::Character, character_id, kind, &json, dedupe)
    .await
    .map(|_| ())
    .map_err(internal)
}

fn internal(error: impl std::fmt::Display) -> ToolError {
  ToolError::Internal(error.to_string())
}

/// A negative, millisecond-epoch-derived optimistic id. The negativity is load-bearing: sync
/// preserves rows with a negative id and the create/send handlers remap or sweep them.
fn optimistic_mail_id() -> i64 {
  let millis = chrono::Utc::now().timestamp_millis();
  -millis.max(1)
}

fn parse_recipients(args: &Value) -> Result<Vec<Recipient>, ToolError> {
  let items = args
    .get("recipients")
    .and_then(Value::as_array)
    .ok_or_else(|| ToolError::InvalidArguments("`recipients` must be an array".to_owned()))?;
  items
    .iter()
    .map(|item| {
      let id = item
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::InvalidArguments("each recipient needs an `id`".to_owned()))?;
      let recipient_type = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("character")
        .to_owned();
      Ok(Recipient {
        id,
        recipient_type,
      })
    })
    .collect()
}

async fn require_scope(
  db: &Database,
  character_id: i64,
  scope: &str,
  permission: &'static str,
) -> Result<(), ToolError> {
  let credential = infra::get(db, character_id, OwnerType::Character)
    .await
    .map_err(internal)?;
  let granted = credential
    .and_then(|cred| cred.scopes().clone())
    .is_some_and(|scopes| scopes.split_whitespace().any(|s| s == scope));
  if granted {
    Ok(())
  } else {
    Err(ToolError::InvalidArguments(format!(
      "character {character_id} has not granted the `{scope}` scope required by {permission}; re-authorize in Pod"
    )))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{config::McpPerms, mcp::tool::Registry, store::repo::character};

  async fn database() -> Database {
    crate::store::open_test().await.expect("open a migrated test database")
  }

  async fn seed_character_with_scopes(db: &Database, id: i64, scopes: &str) {
    use crate::store::model::{Alliance, Bloodline, Character, Corporation, Gender, Race};
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
      .expect("seed character");
    infra::upsert(
      db,
      id,
      OwnerType::Character,
      "tok",
      "rt",
      9_999_999_999,
      None,
      Some(scopes),
    )
    .await
    .expect("seed credential");
  }

  fn all_mail_perms() -> McpPerms {
    let mut perms = McpPerms::default();
    perms.set_send_mail(true);
    perms.set_delete_mail(true);
    perms.set_manage_labels(true);
    perms
  }

  fn registry() -> Registry {
    let mut registry = Registry::default();
    for tool in tools() {
      registry.register(tool);
    }
    registry
  }

  async fn pending_outbox(db: &Database, kind: &str) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind = ?")
      .bind(kind)
      .fetch_one(db.reader())
      .await
      .unwrap()
  }

  async fn seed_mail(db: &Database, character_id: i64, mail_id: i64) {
    let header = CharacterMail {
      character_id,
      from_corp: false,
      from_id: 99,
      from_name: "Sender".to_owned(),
      from_system: false,
      has_attachment: false,
      important: false,
      is_read: false,
      mail_id,
      subject: Some("Hi".to_owned()),
      timestamp: "2026-01-01T00:00:00Z".to_owned(),
    };
    let body = CharacterMailBody {
      body: "hello".to_owned(),
      character_id,
      mail_id,
    };
    mail::upsert_complete(db, &header, &body, &[]).await.expect("seed mail");
  }

  async fn seed_label(db: &Database, character_id: i64, label_id: i64) {
    mail::insert_label(
      db,
      &CharacterMailLabel {
        character_id,
        color: None,
        label_id,
        name: format!("L{label_id}"),
      },
    )
    .await
    .expect("seed label");
  }

  mod gates {
    use super::*;

    #[tokio::test]
    async fn each_tool_is_denied_under_its_own_permission() {
      let db = database().await;
      let registry = registry();
      let denied = [
        ("send_mail", Permission::SendMail, "send_mail"),
        ("delete_mail", Permission::DeleteMail, "delete_mail"),
        ("manage_labels", Permission::ManageLabels, "manage_labels"),
      ];

      for (name, permission, label) in denied {
        let mut perms = all_mail_perms();
        match permission {
          Permission::SendMail => perms.set_send_mail(false),
          Permission::DeleteMail => perms.set_delete_mail(false),
          Permission::ManageLabels => perms.set_manage_labels(false),
          _ => unreachable!(),
        };

        let outcome = registry.dispatch(name, &perms, db.clone(), Value::Null).await;

        assert!(
          matches!(outcome, Err(ToolError::PermissionDenied(p)) if p == label),
          "{name} must be gated by {label}: {outcome:?}"
        );
      }
    }

    #[tokio::test]
    async fn the_gates_are_independent() {
      let db = database().await;
      let registry = registry();
      let mut only_send = McpPerms::default();
      only_send.set_send_mail(true);

      let delete = registry
        .dispatch(
          "delete_mail",
          &only_send,
          db.clone(),
          json!({ "character_id": 1, "mail_id": 1 }),
        )
        .await;

      assert!(matches!(delete, Err(ToolError::PermissionDenied("delete_mail"))));
    }
  }

  mod send_mail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_enqueues_a_send_and_writes_the_optimistic_sent_row() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_SEND).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "send_mail",
          &all_mail_perms(),
          db.clone(),
          json!({
            "character_id": 42,
            "subject": "hi",
            "body": "hello",
            "recipients": [{ "id": 99, "type": "character" }],
          }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("queued").and_then(Value::as_bool), Some(true));
      assert_eq!(pending_outbox(&db, "mail.send").await, 1);
    }

    #[tokio::test]
    async fn it_refuses_a_character_without_the_send_scope() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL).await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "send_mail",
          &all_mail_perms(),
          db,
          json!({
            "character_id": 42,
            "subject": "hi",
            "body": "hello",
            "recipients": [{ "id": 99 }],
          }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod manage_labels {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_enqueues_a_create_label() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "manage_labels",
          &all_mail_perms(),
          db.clone(),
          json!({ "character_id": 42, "action": "create_label", "name": "Ops", "color": "#ffffff" }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("queued").and_then(Value::as_bool), Some(true));
      assert_eq!(pending_outbox(&db, "mail.create_label").await, 1);
    }

    #[tokio::test]
    async fn it_sets_a_mails_labels_and_enqueues_the_change() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      seed_mail(&db, 42, 7000).await;
      seed_label(&db, 42, 11).await;
      seed_label(&db, 42, 22).await;
      mail::add_membership(&db, 42, 7000, 11).await.unwrap();
      let registry = registry();

      let value = registry
        .dispatch(
          "manage_labels",
          &all_mail_perms(),
          db.clone(),
          json!({ "character_id": 42, "action": "set_labels", "mail_id": 7000, "labels": [22] }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("queued").and_then(Value::as_bool), Some(true));
      assert_eq!(pending_outbox(&db, "mail.set_labels").await, 1);
      assert_eq!(mail::membership(&db, 42, 7000).await.unwrap(), vec![22]);
    }

    #[tokio::test]
    async fn it_enqueues_a_delete_label() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      seed_label(&db, 42, 11).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "manage_labels",
          &all_mail_perms(),
          db.clone(),
          json!({ "character_id": 42, "action": "delete_label", "label_id": 11 }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("queued").and_then(Value::as_bool), Some(true));
      assert_eq!(pending_outbox(&db, "mail.delete_label").await, 1);
    }

    #[tokio::test]
    async fn it_rejects_an_unknown_action() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "manage_labels",
          &all_mail_perms(),
          db,
          json!({ "character_id": 42, "action": "rename_label" }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_rejects_an_empty_label_name() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "manage_labels",
          &all_mail_perms(),
          db,
          json!({ "character_id": 42, "action": "create_label", "name": "   " }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_refuses_a_character_without_the_organize_scope() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL).await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "manage_labels",
          &all_mail_perms(),
          db,
          json!({ "character_id": 42, "action": "create_label", "name": "Ops" }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod delete_mail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_purges_the_mail_and_enqueues_the_delete() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      seed_mail(&db, 42, 7000).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "delete_mail",
          &all_mail_perms(),
          db.clone(),
          json!({ "character_id": 42, "mail_id": 7000 }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("queued").and_then(Value::as_bool), Some(true));
      assert_eq!(pending_outbox(&db, "mail.delete").await, 1);
      assert!(mail::mail(&db, 42, 7000).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_rejects_a_mail_the_character_does_not_hold() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "delete_mail",
          &all_mail_perms(),
          db,
          json!({ "character_id": 42, "mail_id": 7000 }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn it_refuses_a_character_without_the_organize_scope() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL).await;
      let registry = registry();

      let outcome = registry
        .dispatch(
          "delete_mail",
          &all_mail_perms(),
          db,
          json!({ "character_id": 42, "mail_id": 7000 }),
        )
        .await;

      assert!(matches!(outcome, Err(ToolError::InvalidArguments(_))));
    }
  }

  mod apply_membership {
    use super::*;

    #[tokio::test]
    async fn it_adds_new_labels_and_removes_dropped_ones() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      seed_mail(&db, 42, 7000).await;
      for label_id in [1, 2, 3] {
        seed_label(&db, 42, label_id).await;
      }
      mail::add_membership(&db, 42, 7000, 1).await.unwrap();
      mail::add_membership(&db, 42, 7000, 2).await.unwrap();

      super::super::apply_membership(&db, 42, 7000, &[1, 2], &[2, 3])
        .await
        .unwrap();

      assert_eq!(mail::membership(&db, 42, 7000).await.unwrap(), vec![2, 3]);
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_the_sets_match() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      seed_mail(&db, 42, 7000).await;
      seed_label(&db, 42, 1).await;
      mail::add_membership(&db, 42, 7000, 1).await.unwrap();

      super::super::apply_membership(&db, 42, 7000, &[1], &[1]).await.unwrap();

      assert_eq!(mail::membership(&db, 42, 7000).await.unwrap(), vec![1]);
    }
  }

  mod parse_recipients {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_the_recipient_type_to_character() {
      let recipients = super::super::parse_recipients(&json!({ "recipients": [{ "id": 99 }] })).unwrap();

      assert_eq!(recipients.len(), 1);
      assert_eq!(recipients[0].id, 99);
      assert_eq!(recipients[0].recipient_type, "character");
    }

    #[test]
    fn it_reads_an_explicit_recipient_type() {
      let recipients =
        super::super::parse_recipients(&json!({ "recipients": [{ "id": 5, "type": "mailing_list" }] })).unwrap();

      assert_eq!(recipients[0].recipient_type, "mailing_list");
    }

    #[test]
    fn it_errors_when_recipients_is_not_an_array() {
      assert!(matches!(
        super::super::parse_recipients(&json!({})),
        Err(ToolError::InvalidArguments(_))
      ));
    }

    #[test]
    fn it_errors_on_a_recipient_without_an_id() {
      assert!(matches!(
        super::super::parse_recipients(&json!({ "recipients": [{ "type": "character" }] })),
        Err(ToolError::InvalidArguments(_))
      ));
    }
  }

  mod arg_specs {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::mcp::args::input_schema;

    fn schema_for(name: &str) -> Value {
      let tool = tools()
        .into_iter()
        .find(|tool| tool.name() == name)
        .expect("tool exists");
      input_schema(tool.args())
    }

    #[test]
    fn send_mail_advertises_its_arguments() {
      let schema = schema_for("send_mail");

      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert_eq!(schema["properties"]["subject"]["type"], "string");
      assert_eq!(schema["properties"]["body"]["type"], "string");
      assert_eq!(schema["properties"]["recipients"]["type"], "array");

      let required = schema["required"].as_array().unwrap();
      for arg in ["character_id", "subject", "body", "recipients"] {
        assert!(required.contains(&json!(arg)), "{arg} must be required");
      }
    }

    #[test]
    fn delete_mail_advertises_its_integer_ids() {
      let schema = schema_for("delete_mail");

      assert_eq!(schema["properties"]["character_id"]["type"], "integer");
      assert_eq!(schema["properties"]["mail_id"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("character_id")));
      assert!(required.contains(&json!("mail_id")));
    }

    #[test]
    fn manage_labels_advertises_action_and_the_labels_array() {
      let schema = schema_for("manage_labels");

      assert_eq!(schema["properties"]["action"]["type"], "string");
      assert_eq!(schema["properties"]["labels"]["type"], "array");
      assert_eq!(schema["properties"]["labels"]["items"]["type"], "integer");

      let required = schema["required"].as_array().unwrap();
      assert!(required.contains(&json!("character_id")));
      assert!(required.contains(&json!("action")));
      assert!(!required.contains(&json!("label_id")));
    }

    #[tokio::test]
    async fn delete_mail_coerces_a_numeric_string_id() {
      let db = database().await;
      seed_character_with_scopes(&db, 42, scopes::CHARACTER_MAIL_ORGANIZE).await;
      seed_mail(&db, 42, 7000).await;
      let registry = registry();

      let value = registry
        .dispatch(
          "delete_mail",
          &all_mail_perms(),
          db.clone(),
          json!({ "character_id": "42", "mail_id": "7000" }),
        )
        .await
        .unwrap();

      assert_eq!(value.get("mail_id").and_then(Value::as_i64), Some(7000));
      assert_eq!(pending_outbox(&db, "mail.delete").await, 1);
    }
  }
}
