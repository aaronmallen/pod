use std::sync::Arc;

use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, mouse_area, text, text_editor, text_input},
};
use serde::{Deserialize, Serialize};

use super::{Message, loaders::RosterPilot, markup::Link};
use crate::{
  store::{
    Database, images,
    model::{
      CharacterMail, CharacterMailBody, CharacterMailRecipient, MailDraft, OwnerType, character_mail_view::MailRender,
    },
    repo::{infra, mail},
  },
  ui::{
    components::{
      avatar::Avatar,
      entity_search::{EntityKind, EntityRef, EntitySearch, MultiSelect},
      eyebrow::eyebrow_text,
      header,
      icon::Icon,
      picker::TriggerPortrait,
      rule,
    },
    style::{color, radius, spacing, typography},
  },
};

pub const COMPOSE_WINDOW_HEIGHT: f32 = 620.0;

pub const COMPOSE_WINDOW_WIDTH: f32 = 600.0;

const FROM_PORTRAIT_SIZE: f32 = 22.0;

const LINK_POPOVER_WIDTH: f32 = 320.0;

/// What a per-window compose needs the app dispatcher (which owns the runtime + window lifetime) to do
/// after a message is applied. State-only edits return [`Effect::None`].
#[derive(Clone, Debug)]
pub enum Effect {
  Discard,
  LinkSearch(String),
  None,
  RecipientSearch { is_to: bool, query: String },
  Send,
}

/// How a freshly-opened compose window is seeded. `Draft` opens blank and then loads the persisted
/// row id into the window once it exists.
#[derive(Clone, Debug)]
pub enum Seed {
  Blank { from_character_id: i64 },
  Draft { draft_id: i64, from_character_id: i64 },
  Reply { kind: Kind, render: Box<MailRender> },
}

impl Seed {
  /// The draft row id to load after the window opens, for a `Draft` seed; `None` for blank/reply.
  pub fn draft_id(&self) -> Option<i64> {
    match self {
      Seed::Draft {
        draft_id, ..
      } => Some(*draft_id),
      _ => None,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Draft {
  pub body: text_editor::Content,
  pub cc: Vec<Recipient>,
  pub cc_chips: Vec<EntityRef>,
  pub cc_search: EntitySearch,
  pub error: Option<String>,
  pub from_character_id: i64,
  pub from_picker_open: bool,
  /// The `mail_drafts` row id once this compose has been persisted; threaded back so every later
  /// save updates the same row and a successful send deletes it by id.
  pub id: Option<i64>,
  pub kind: Kind,
  pub link: Option<LinkPopover>,
  pub quote: Option<String>,
  pub show_cc: bool,
  pub subject: String,
  pub to: Vec<Recipient>,
  /// Parallel to `to`; owned storage for `MultiSelect` which borrows `&[EntityRef]`. Keep in sync via `push_to`/`remove_to`.
  pub to_chips: Vec<EntityRef>,
  pub to_search: EntitySearch,
}

impl Draft {
  pub(super) fn blank(from_character_id: i64) -> Self {
    Draft {
      body: text_editor::Content::new(),
      cc: Vec::new(),
      cc_chips: Vec::new(),
      cc_search: EntitySearch::default(),
      error: None,
      from_character_id,
      from_picker_open: false,
      id: None,
      kind: Kind::New,
      link: None,
      quote: None,
      show_cc: false,
      subject: String::new(),
      to: Vec::new(),
      to_chips: Vec::new(),
      to_search: EntitySearch::default(),
    }
  }

  pub(super) fn from_mail(kind: Kind, mail: &MailRender) -> Self {
    let from_character_id = mail.header.character_id();
    let mut draft = Draft::blank(from_character_id);
    draft.kind = kind;

    let subject = mail.header.subject().clone().unwrap_or_default();
    draft.subject = match kind {
      Kind::Forward => prefixed(&subject, "Fwd: "),
      _ => prefixed(&subject, "Re: "),
    };

    if matches!(kind, Kind::Reply | Kind::ReplyAll) {
      draft.push_to(Recipient::character(
        mail.header.from_name().clone(),
        mail.header.from_id(),
      ));
    }
    if matches!(kind, Kind::ReplyAll) {
      for r in &mail.recipients {
        if r.recipient_id() == mail.header.from_id() || r.recipient_id() == from_character_id {
          continue;
        }
        draft.push_cc(Recipient {
          id: Some(r.recipient_id()),
          name: r.recipient_name().to_owned(),
          recipient_type: Some(r.recipient_type().to_owned()),
        });
      }
      draft.show_cc = !draft.cc.is_empty();
    }

    let quote_body = strip_quote(mail.body.body());
    if !quote_body.is_empty() {
      draft.quote = Some(format!("From {}:\n{}", mail.header.from_name(), quote_body));
    }
    draft
  }

  pub fn from_persisted(row: &MailDraft) -> Self {
    let mut draft = Draft::blank(row.character_id());
    draft.id = Some(row.id());
    draft.kind = Kind::from_storage(row.kind());
    draft.subject = row.subject().clone();
    draft.body = text_editor::Content::with_text(row.body());
    draft.quote = row.quote().clone();

    for recipient in deserialize_recipients(row.recipients_to()) {
      draft.push_to(recipient);
    }
    for recipient in deserialize_recipients(row.recipients_cc()) {
      draft.push_cc(recipient);
    }
    draft.show_cc = !draft.cc.is_empty();
    draft
  }

  pub(super) fn can_send(&self) -> bool {
    !self.to.is_empty() && !self.subject.trim().is_empty()
  }

  /// A draft worth persisting: anything typed into the subject, body, or recipients. A blank new
  /// compose closed without input is discarded rather than saved.
  pub(super) fn is_empty(&self) -> bool {
    self.subject.trim().is_empty() && self.body.text().trim().is_empty() && self.to.is_empty() && self.cc.is_empty()
  }

  pub(super) fn persist_input(&self) -> mail::DraftInput {
    mail::DraftInput {
      body: self.body.text(),
      character_id: self.from_character_id,
      kind: self.kind.as_storage().to_owned(),
      quote: self.quote.clone(),
      recipients_cc: serialize_recipients(&self.cc),
      recipients_to: serialize_recipients(&self.to),
      subject: self.subject.clone(),
    }
  }

  pub(super) fn push_cc(&mut self, recipient: Recipient) {
    self.cc_chips.push(recipient_entity(&recipient));
    self.cc.push(recipient);
  }

  pub(super) fn push_to(&mut self, recipient: Recipient) {
    self.to_chips.push(recipient_entity(&recipient));
    self.to.push(recipient);
  }

  pub(super) fn remove_cc(&mut self, index: usize) {
    if index < self.cc.len() {
      self.cc.remove(index);
      self.cc_chips.remove(index);
    }
  }

  pub(super) fn remove_to(&mut self, index: usize) {
    if index < self.to.len() {
      self.to.remove(index);
      self.to_chips.remove(index);
    }
  }

  /// Wraps the current body selection in `<b>`/`<i>` (per `kind`), or — when nothing is selected —
  /// inserts an empty tag pair at the cursor. The editor content is the wire format, so the tags go
  /// out verbatim.
  pub(super) fn wrap_emphasis(&mut self, kind: EmphasisKind) {
    let selection = self.body.selection().unwrap_or_default();
    let wrapped = match kind {
      EmphasisKind::Bold => super::markup::bold(&selection),
      EmphasisKind::Italic => super::markup::italic(&selection),
    };
    self.insert_text(&wrapped);
  }

  /// Replaces the current body selection with `text` (or inserts at the cursor when nothing is
  /// selected). A paste edit overwrites any active selection in iced's editor.
  pub(super) fn insert_text(&mut self, text: &str) {
    self
      .body
      .perform(text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(
        text.to_owned(),
      ))));
  }

  pub fn from_seed(seed: Seed) -> Self {
    match seed {
      Seed::Blank {
        from_character_id,
      }
      | Seed::Draft {
        from_character_id, ..
      } => Draft::blank(from_character_id),
      Seed::Reply {
        kind,
        render,
      } => Draft::from_mail(kind, &render),
    }
  }

  pub fn set_id(&mut self, id: Option<i64>) {
    self.id = id;
  }

  /// The sent draft's persisted row id (when it had one), so the app can delete it on a successful
  /// send.
  pub fn sent_draft_id(&self) -> Option<i64> {
    self.id
  }

  /// The persist input paired with the existing row id, when the compose is non-empty and worth
  /// saving. `None` for a blank compose, which is discarded rather than saved.
  pub fn pending_save(&self) -> Option<(Option<i64>, mail::DraftInput)> {
    if self.is_empty() {
      return None;
    }
    Some((self.id, self.persist_input()))
  }

  pub fn recipient_search_generation(&self, is_to: bool) -> u64 {
    if is_to {
      self.to_search.generation()
    } else {
      self.cc_search.generation()
    }
  }

  pub fn link_search(&self) -> Option<(u64, crate::features::roster::entity_search::EntityCategory)> {
    let popover = self.link.as_ref()?;
    Some((popover.search.generation(), popover.kind.category()?))
  }

  fn snapshot_clone(&self) -> Draft {
    self.clone()
  }
}

/// The window title for a compose: distinguishes a reply/forward from a new mail by subject so two
/// open composes are tellable apart.
pub fn window_title(draft: &Draft) -> String {
  let subject = draft.subject.trim();
  if subject.is_empty() {
    "New message".to_owned()
  } else {
    subject.to_owned()
  }
}

/// Loads a persisted draft row for an open compose window, routed back as a `DraftLoaded`.
pub fn load_draft(db: &Database, draft_id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { mail::draft(&db, draft_id).await.ok().flatten() }, |row| {
    Message::DraftLoaded(Box::new(row))
  })
}

/// Applies a per-window compose message to a single window's [`Draft`] and reports the follow-up the
/// app dispatcher must run. Mirrors the former single-instance compose handlers, minus the holder
/// bookkeeping now owned per-window by the app.
pub fn update(draft: &mut Draft, message: Message) -> Effect {
  // Recipient + link field edits are pure state mutations; the only ones needing async are the search
  // inputs, surfaced via the dedicated branches below.
  match &message {
    Message::ComposeToInput(value) => {
      let value = value.clone();
      draft.to_search.set_query(value.clone());
      return Effect::RecipientSearch {
        is_to: true,
        query: value,
      };
    }
    Message::ComposeCcInput(value) => {
      let value = value.clone();
      draft.cc_search.set_query(value.clone());
      return Effect::RecipientSearch {
        is_to: false,
        query: value,
      };
    }
    Message::ComposeLinkSearchInput(value) => {
      let value = value.clone();
      if let Some(popover) = draft.link.as_mut() {
        popover.search.set_query(value.clone());
      }
      return Effect::LinkSearch(value);
    }
    _ => {}
  }
  let message = match apply_recipients(draft, message) {
    Ok(()) => return Effect::None,
    Err(message) => message,
  };
  let message = match apply_link(draft, message) {
    Ok(()) => return Effect::None,
    Err(message) => message,
  };
  apply_fields(draft, message)
}

fn apply_fields(draft: &mut Draft, message: Message) -> Effect {
  match message {
    Message::ComposeCcShown => draft.show_cc = true,
    Message::ComposeSubjectChanged(value) => draft.subject = value,
    Message::ComposeBodyChanged(action) => draft.body.perform(action),
    Message::ComposeBold => draft.wrap_emphasis(EmphasisKind::Bold),
    Message::ComposeItalic => draft.wrap_emphasis(EmphasisKind::Italic),
    Message::ComposeFromToggled => draft.from_picker_open = !draft.from_picker_open,
    Message::ComposeFromChanged(character_id) => {
      draft.from_character_id = character_id;
      draft.from_picker_open = false;
    }
    Message::ComposeDiscarded => return Effect::Discard,
    Message::ComposeSend if draft.can_send() => {
      return Effect::Send;
    }
    Message::ComposeSent(Err(error)) => draft.error = Some(error),
    _ => {}
  }
  Effect::None
}

fn apply_link(draft: &mut Draft, message: Message) -> Result<(), Message> {
  match message {
    Message::ComposeLinkToggled => {
      draft.link = match draft.link {
        Some(_) => None,
        None => Some(LinkPopover::default()),
      };
    }
    Message::ComposeLinkKindSelected(kind) => {
      if let Some(popover) = draft.link.as_mut() {
        popover.select(kind);
      }
    }
    Message::ComposeLinkUrlChanged(value) => {
      if let Some(popover) = draft.link.as_mut() {
        popover.url = value;
      }
    }
    Message::ComposeLinkSearched {
      generation,
      results,
    } => {
      if let Some(popover) = draft.link.as_mut() {
        popover.search.accept_results(generation, results.clone());
        popover.results = results;
      }
    }
    Message::ComposeLinkPicked(entity) => {
      let markup = draft
        .link
        .as_ref()
        .and_then(|popover| popover.kind.link_for(entity.id, entity.name))
        .map(|link| link.to_markup());
      if let Some(markup) = markup {
        draft.insert_text(&markup);
        draft.link = None;
      }
    }
    Message::ComposeLinkInsert => {
      let markup = draft
        .link
        .as_ref()
        .and_then(LinkPopover::http_link)
        .map(|link| link.to_markup());
      if let Some(markup) = markup {
        draft.insert_text(&markup);
        draft.link = None;
      }
    }
    other => return Err(other),
  }
  Ok(())
}

fn apply_recipients(draft: &mut Draft, message: Message) -> Result<(), Message> {
  match message {
    Message::ComposeToSearched {
      generation,
      results,
    } => {
      draft.to_search.accept_results(generation, results);
    }
    Message::ComposeCcSearched {
      generation,
      results,
    } => {
      draft.cc_search.accept_results(generation, results);
    }
    Message::ComposeToCommitted => {
      let name = draft.to_search.query().trim().to_owned();
      if !name.is_empty() {
        draft.push_to(Recipient::typed(name));
        draft.to_search.clear();
      }
    }
    Message::ComposeCcCommitted => {
      let name = draft.cc_search.query().trim().to_owned();
      if !name.is_empty() {
        draft.push_cc(Recipient::typed(name));
        draft.cc_search.clear();
      }
    }
    Message::ComposeToPicked(entity) => {
      draft.push_to(Recipient::from_entity(entity));
      draft.to_search.clear();
    }
    Message::ComposeCcPicked(entity) => {
      draft.push_cc(Recipient::from_entity(entity));
      draft.cc_search.clear();
    }
    Message::ComposeToRemoved(index) => draft.remove_to(index),
    Message::ComposeCcRemoved(index) => draft.remove_cc(index),
    other => return Err(other),
  }
  Ok(())
}

/// Clones the draft for the async send (the editor content must outlive the borrow).
pub fn send(db: &Database, draft: &Draft) -> Task<Message> {
  let db = db.clone();
  let draft = draft.snapshot_clone();
  Task::perform(enqueue_send(db, draft), Message::ComposeSent)
}

/// Renders the compose as the body of a native-chrome window: an in-content header (matching the
/// Compare/SkillPlanEditor convention) stacked above the compose form. The OS frame supplies the
/// title bar; the kind-aware OS title reuses [`window_title`].
pub fn view<'a>(draft: &'a Draft, roster: &'a [RosterPilot]) -> Element<'a, Message> {
  let title = text(window_title(draft))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG + 2.0)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let header = header::header(vec![title.into()], Vec::new());

  container(
    Column::with_children(vec![header, window_body(draft, roster)])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  })
  .into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmphasisKind {
  Bold,
  Italic,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LinkKind {
  Character,
  Corporation,
  #[default]
  Http,
  SolarSystem,
  Station,
}

impl LinkKind {
  pub(super) const ALL: [LinkKind; 5] = [
    LinkKind::Http,
    LinkKind::Character,
    LinkKind::Corporation,
    LinkKind::SolarSystem,
    LinkKind::Station,
  ];

  /// The entity-search category for searchable kinds; `None` for the plain `http` kind, which takes
  /// a typed URL rather than an entity search.
  pub(super) fn category(self) -> Option<crate::features::roster::entity_search::EntityCategory> {
    use crate::features::roster::entity_search::EntityCategory;
    match self {
      LinkKind::Character => Some(EntityCategory::Character),
      LinkKind::Corporation => Some(EntityCategory::Corporation),
      LinkKind::Http => None,
      LinkKind::SolarSystem => Some(EntityCategory::SolarSystem),
      LinkKind::Station => Some(EntityCategory::Station),
    }
  }

  pub(super) fn label(self) -> &'static str {
    match self {
      LinkKind::Character => "Character",
      LinkKind::Corporation => "Corporation",
      LinkKind::Http => "http://",
      LinkKind::SolarSystem => "Solar System",
      LinkKind::Station => "Station",
    }
  }

  pub(super) fn placeholder(self) -> &'static str {
    match self {
      LinkKind::Character => "Search characters\u{2026}",
      LinkKind::Corporation => "Search corporations\u{2026}",
      LinkKind::Http => "example.com/path",
      LinkKind::SolarSystem => "Search solar systems\u{2026}",
      LinkKind::Station => "Search stations\u{2026}",
    }
  }

  /// Builds the markup link for an entity result of this kind. Returns `None` for `http`, which is
  /// built from the typed URL instead of a picked entity.
  pub(super) fn link_for(self, id: i64, name: String) -> Option<Link> {
    match self {
      LinkKind::Character => Some(Link::character(id, name)),
      LinkKind::Corporation => Some(Link::corporation(id, name)),
      LinkKind::Http => None,
      LinkKind::SolarSystem => Some(Link::solar_system(id, name)),
      // The SDE per-station type-id is resolved by the search loader and folded into the picked
      // entity; absent it, the station degrades to a system-level link.
      LinkKind::Station => Some(Link::solar_system(id, name)),
    }
  }
}

/// The toolbar "Generate Link" popover state: the selected kind, the typed query, and the live
/// entity-search results for searchable kinds.
#[derive(Clone, Debug, Default)]
pub struct LinkPopover {
  pub kind: LinkKind,
  pub results: Vec<EntityRef>,
  pub search: EntitySearch,
  pub url: String,
}

impl LinkPopover {
  pub(super) fn can_insert(&self) -> bool {
    matches!(self.kind, LinkKind::Http) && !self.url.trim().is_empty()
  }

  /// The markup for an http link from the typed URL, normalising a bare host to an `http://` URL so
  /// the emitted href is always absolute.
  pub(super) fn http_link(&self) -> Option<Link> {
    let raw = self.url.trim();
    if raw.is_empty() {
      return None;
    }
    let url = if raw.contains("://") {
      raw.to_owned()
    } else {
      format!("http://{raw}")
    };
    Some(Link::http(url.clone(), url))
  }

  pub(super) fn select(&mut self, kind: LinkKind) {
    self.kind = kind;
    self.url.clear();
    self.search.clear();
    self.results.clear();
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Kind {
  Forward,
  #[default]
  New,
  Reply,
  ReplyAll,
}

impl Kind {
  pub(super) fn as_storage(self) -> &'static str {
    match self {
      Kind::Forward => "Forward",
      Kind::New => "New",
      Kind::Reply => "Reply",
      Kind::ReplyAll => "ReplyAll",
    }
  }

  fn from_storage(raw: &str) -> Self {
    match raw {
      "Forward" => Kind::Forward,
      "Reply" => Kind::Reply,
      "ReplyAll" => Kind::ReplyAll,
      _ => Kind::New,
    }
  }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Recipient {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub id: Option<i64>,
  pub name: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub recipient_type: Option<String>,
}

impl Recipient {
  pub(super) fn character(name: impl Into<String>, id: i64) -> Self {
    Recipient {
      id: Some(id),
      name: name.into(),
      recipient_type: Some("character".to_owned()),
    }
  }

  pub(super) fn from_entity(entity: EntityRef) -> Self {
    let recipient_type = match entity.kind {
      EntityKind::Alliance => "alliance",
      EntityKind::Corporation => "corporation",
      EntityKind::Character | EntityKind::SolarSystem | EntityKind::Station => "character",
    };
    Recipient {
      id: Some(entity.id),
      name: entity.name,
      recipient_type: Some(recipient_type.to_owned()),
    }
  }

  pub(super) fn typed(name: impl Into<String>) -> Self {
    Recipient {
      id: None,
      name: name.into(),
      recipient_type: None,
    }
  }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SendPayload {
  pub body: String,
  pub from_character_id: i64,
  /// The synthetic, negative mail id of the optimistic Sent-folder row written at enqueue time. The
  /// `mail.send` handler purges this row when the ESI send permanently fails (compensate).
  pub optimistic_mail_id: i64,
  pub recipients: Vec<Recipient>,
  pub subject: String,
}

impl SendPayload {
  fn from_draft(draft: &Draft, optimistic_mail_id: i64) -> Self {
    let mut recipients = draft.to.clone();
    recipients.extend(draft.cc.iter().cloned());
    SendPayload {
      body: draft.body.text(),
      from_character_id: draft.from_character_id,
      optimistic_mail_id,
      recipients,
      subject: draft.subject.clone(),
    }
  }
}

/// A negative, millisecond-epoch-derived mail id for the optimistic Sent-folder row. The negativity
/// is load-bearing: mail sync upserts the real sent mail under ESI's own positive id and never
/// deletes this placeholder, so the sync job sweeps negative self-sent rows on the next pass.
fn optimistic_mail_id() -> i64 {
  let millis = chrono::Utc::now().timestamp_millis();
  -millis.max(1)
}

fn deserialize_recipients(json: &str) -> Vec<Recipient> {
  serde_json::from_str(json).unwrap_or_default()
}

fn serialize_recipients(recipients: &[Recipient]) -> String {
  serde_json::to_string(recipients).unwrap_or_else(|_| "[]".to_owned())
}

fn prefixed(subject: &str, prefix: &str) -> String {
  if subject.starts_with(prefix) {
    subject.to_owned()
  } else {
    format!("{prefix}{subject}")
  }
}

fn strip_quote(html: &str) -> String {
  super::loaders::strip_html_snippet(html)
}

/// Writes the optimistic Sent-folder mail first, then appends the `mail.send` outbox row. Mirrors
/// the label handlers: the outbox drainer never calls a handler `apply()`, so the feature layer
/// owns the optimistic write. The synthetic row carries `from_id == character_id`, which is what
/// puts it in the Sent folder; it is reconciled away by the next sync and compensated (purged) by
/// the handler on permanent failure.
pub(super) async fn enqueue_send(db: Database, draft: Draft) -> Result<(), String> {
  let mail_id = optimistic_mail_id();
  let payload = SendPayload::from_draft(&draft, mail_id);
  let json = serde_json::to_string(&payload).map_err(|e| format!("could not build the send: {e}"))?;

  write_optimistic_sent(&db, &payload, mail_id).await;

  infra::append(
    &db,
    OwnerType::Character,
    payload.from_character_id,
    "mail.send",
    &json,
    None,
  )
  .await
  .map(|_| ())
  .map_err(|e| format!("could not queue the send: {e}"))
}

async fn write_optimistic_sent(db: &Database, payload: &SendPayload, mail_id: i64) {
  let character_id = payload.from_character_id;
  let from_name = crate::store::repo::character::get(db, character_id)
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
    subject: Some(payload.subject.clone()),
    timestamp: chrono::Utc::now().to_rfc3339(),
  };
  let body = CharacterMailBody {
    body: payload.body.clone(),
    character_id,
    mail_id,
  };
  let recipients: Vec<CharacterMailRecipient> = payload
    .recipients
    .iter()
    .filter_map(|recipient| {
      recipient.id.map(|recipient_id| CharacterMailRecipient {
        character_id,
        mail_id,
        recipient_id,
        recipient_name: recipient.name.clone(),
        recipient_type: recipient
          .recipient_type
          .clone()
          .unwrap_or_else(|| "character".to_owned()),
      })
    })
    .collect();

  let _ = mail::upsert_complete(db, &header, &body, &recipients).await;
}

fn window_body<'a>(draft: &'a Draft, roster: &'a [RosterPilot]) -> Element<'a, Message> {
  let body = Column::with_children(vec![
    to_field(draft),
    cc_field(draft),
    subject_field(draft),
    body_field(draft),
    error_line(draft),
    footer(draft, roster),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      ..container::Style::default()
    })
    .into()
}

fn to_field<'a>(draft: &'a Draft) -> Element<'a, Message> {
  let picker = MultiSelect::new(
    draft.to_search.query(),
    &draft.to_chips,
    draft.to_search.results(),
    Message::ComposeToInput,
    Message::ComposeToPicked,
    Message::ComposeToRemoved,
  )
  .inline(true)
  .placeholder("Search characters or corporations\u{2026}")
  .searching(draft.to_search.searching())
  .on_submit(Message::ComposeToCommitted)
  .view();

  let content: Element<'a, Message> = if draft.show_cc {
    picker
  } else {
    Row::with_children(vec![
      container(picker).width(Length::Fill).into(),
      mouse_area(text("Cc").size(typography::size::SM).style(|_| text::Style {
        color: Some(color::text::secondary()),
      }))
      .on_press(Message::ComposeCcShown)
      .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
  };

  field_row("To", content)
}

fn cc_field<'a>(draft: &'a Draft) -> Element<'a, Message> {
  if !draft.show_cc {
    return Space::new().width(Length::Shrink).height(Length::Shrink).into();
  }
  let picker = MultiSelect::new(
    draft.cc_search.query(),
    &draft.cc_chips,
    draft.cc_search.results(),
    Message::ComposeCcInput,
    Message::ComposeCcPicked,
    Message::ComposeCcRemoved,
  )
  .inline(true)
  .placeholder("Search characters or corporations\u{2026}")
  .searching(draft.cc_search.searching())
  .on_submit(Message::ComposeCcCommitted)
  .view();

  field_row("Cc", picker)
}

fn recipient_entity(recipient: &Recipient) -> EntityRef {
  let id = recipient.id.unwrap_or_default();
  EntityRef {
    id,
    kind: EntityKind::Character,
    name: recipient.name.clone(),
    portrait: Some(images::default_store().image_path(images::ImageKind::CharacterPortrait, id)),
  }
}

fn subject_field(draft: &Draft) -> Element<'_, Message> {
  let input = text_input("—", &draft.subject)
    .on_input(Message::ComposeSubjectChanged)
    .padding(0.0)
    .size(typography::size::LG)
    .font(typography::body::MEDIUM)
    .width(Length::Fill)
    .style(transparent_input);
  field_row("Subject", input.into())
}

fn body_field(draft: &Draft) -> Element<'_, Message> {
  let editor = text_editor(&draft.body)
    .placeholder("Write your message…")
    .on_action(Message::ComposeBodyChanged)
    .padding(0.0)
    .size(typography::size::MD)
    .height(Length::Fill)
    .style(transparent_editor);

  let mut column = Column::new().spacing(spacing::SPACE_3).push(editor);
  if let Some(quote) = &draft.quote {
    column = column.push(
      container(text(quote.clone()).size(typography::size::SM).style(|_| text::Style {
        color: Some(color::text::secondary()),
      }))
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_3,
        bottom: 0.0,
        left: spacing::SPACE_3,
        right: 0.0,
      }),
    );
  }

  container(column)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_6 - 4.0,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .into()
}

fn error_line(draft: &Draft) -> Element<'_, Message> {
  match &draft.error {
    Some(message) => container(text(message.clone()).size(typography::size::SM).style(|_| text::Style {
      color: Some(color::status::DANGER),
    }))
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .into(),
    None => Space::new().width(Length::Shrink).height(Length::Shrink).into(),
  }
}

fn footer<'a>(draft: &'a Draft, roster: &'a [RosterPilot]) -> Element<'a, Message> {
  let from_pilot = roster.iter().find(|p| p.id == draft.from_character_id);
  let from_name = from_pilot
    .map(|p| p.name.clone())
    .unwrap_or_else(|| "Unknown".to_owned());

  let mut trigger_cells: Vec<Element<'a, Message>> = vec![eyebrow_text("FROM", Some(color::text::tertiary())).into()];
  if let Some(pilot) = from_pilot {
    let portrait = TriggerPortrait {
      id: pilot.id,
      name: pilot.name.clone(),
      path: pilot.portrait.path(),
    };
    trigger_cells.push(from_portrait(portrait));
  }
  trigger_cells.push(
    text(from_name)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  );
  trigger_cells.push(
    Icon::chevron_down()
      .size(typography::size::SM)
      .color(color::text::secondary())
      .render::<Message>(),
  );

  let from_trigger = mouse_area(
    container(
      Row::with_children(trigger_cells)
        .spacing(spacing::UNIT + 2.0)
        .align_y(Vertical::Center),
    )
    .padding(Padding {
      top: spacing::UNIT,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2,
      right: spacing::SPACE_2,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    }),
  )
  .on_press(Message::ComposeFromToggled);

  let send = send_button(draft.can_send());

  let row = Row::with_children(vec![
    toolbar_button(Icon::bold(), false, Message::ComposeBold),
    toolbar_button(Icon::italic(), false, Message::ComposeItalic),
    toolbar_button(Icon::link(), draft.link.is_some(), Message::ComposeLinkToggled),
    Space::new().width(Length::Fill).into(),
    discard_button(),
    from_trigger.into(),
    send,
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let footer_bar = container(row)
    .width(Length::Fill)
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  let mut children: Vec<Element<'a, Message>> = Vec::new();
  if let Some(popover) = &draft.link {
    children.push(link_popover(popover));
  }
  if draft.from_picker_open && roster.len() > 1 {
    children.push(from_dropdown(draft, roster));
  }
  children.push(rule::horizontal());
  children.push(footer_bar.into());

  Column::with_children(children).into()
}

fn toolbar_button<'a>(icon: Icon, active: bool, message: Message) -> Element<'a, Message> {
  let tint = if active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  mouse_area(
    container(icon.size(14.0).color(tint).render::<Message>())
      .width(Length::Fixed(28.0))
      .height(Length::Fixed(28.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .on_press(message)
  .into()
}

fn link_popover(popover: &LinkPopover) -> Element<'_, Message> {
  let header = container(eyebrow_text("GENERATE LINK", Some(color::text::tertiary())))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    });

  let mut kinds = Column::new().spacing(spacing::UNIT).width(Length::Fill);
  for kind in LinkKind::ALL {
    kinds = kinds.push(link_radio(kind, kind == popover.kind));
  }

  let input: Element<'_, Message> = match popover.kind {
    LinkKind::Http => {
      let field = text_input(LinkKind::Http.placeholder(), &popover.url)
        .on_input(Message::ComposeLinkUrlChanged)
        .on_submit(Message::ComposeLinkInsert)
        .padding(spacing::SPACE_2)
        .size(typography::size::MD)
        .width(Length::Fill)
        .style(link_input_style);
      Row::with_children(vec![
        container(field).width(Length::Fill).into(),
        link_insert_button(popover.can_insert()),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into()
    }
    _ => MultiSelect::new(
      popover.search.query(),
      &[],
      &popover.results,
      Message::ComposeLinkSearchInput,
      Message::ComposeLinkPicked,
      |_| Message::ComposeLinkToggled,
    )
    .placeholder(popover.kind.placeholder())
    .searching(popover.search.searching())
    .view(),
  };

  let body = Column::with_children(vec![
    header.into(),
    rule::horizontal(),
    container(kinds)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_2,
        right: spacing::SPACE_2,
      })
      .into(),
    container(input)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_3,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .into(),
  ])
  .width(Length::Fill);

  container(body)
    .width(Length::Fixed(LINK_POPOVER_WIDTH))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn link_radio<'a>(kind: LinkKind, selected: bool) -> Element<'a, Message> {
  let dot_color = if selected {
    color::accent::PLASMA
  } else {
    color::rule_strong()
  };
  let inner: Element<'a, Message> = if selected {
    container(Space::new())
      .width(Length::Fixed(7.0))
      .height(Length::Fixed(7.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: 3.5.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  } else {
    Space::new().into()
  };
  let dot = container(inner)
    .width(Length::Fixed(15.0))
    .height(Length::Fixed(15.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      border: Border {
        color: dot_color,
        radius: 7.5.into(),
        width: 1.5,
      },
      ..container::Style::default()
    });

  mouse_area(
    container(
      Row::with_children(vec![
        dot.into(),
        text(kind.label())
          .size(typography::size::SM)
          .style(move |_| text::Style {
            color: Some(if selected {
              color::text::PRIMARY
            } else {
              color::text::secondary()
            }),
          })
          .into(),
      ])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT + 1.0,
      bottom: spacing::UNIT + 1.0,
      left: spacing::UNIT + 2.0,
      right: spacing::UNIT + 2.0,
    }),
  )
  .on_press(Message::ComposeLinkKindSelected(kind))
  .into()
}

fn link_insert_button<'a>(enabled: bool) -> Element<'a, Message> {
  let (fg, bg) = if enabled {
    (color::surface::BASE, color::accent::PLASMA)
  } else {
    (color::text::tertiary(), color::with_alpha(color::text::PRIMARY, 0.08))
  };
  let button = container(
    text("Insert")
      .size(typography::size::SM)
      .font(typography::body::MEDIUM)
      .style(move |_| text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  if enabled {
    mouse_area(button).on_press(Message::ComposeLinkInsert).into()
  } else {
    button.into()
  }
}

fn from_portrait<'a>(portrait: TriggerPortrait) -> Element<'a, Message> {
  Avatar::new(
    portrait.id,
    portrait.name,
    Length::Fixed(FROM_PORTRAIT_SIZE),
    FROM_PORTRAIT_SIZE,
    portrait.path,
  )
  .radius(radius::SUBTLE)
  .view::<Message>()
}

fn from_dropdown<'a>(draft: &'a Draft, roster: &'a [RosterPilot]) -> Element<'a, Message> {
  let mut column = Column::new().spacing(spacing::UNIT / 2.0);
  for pilot in roster {
    let selected = pilot.id == draft.from_character_id;
    column = column.push(
      mouse_area(
        container(
          text(format!("{} · {}", pilot.name, pilot.corp))
            .size(typography::size::MD)
            .style(move |_| text::Style {
              color: Some(if selected {
                color::accent::PLASMA
              } else {
                color::text::PRIMARY
              }),
            }),
        )
        .width(Length::Fill)
        .padding(Padding {
          top: spacing::SPACE_2,
          bottom: spacing::SPACE_2,
          left: spacing::SPACE_2_5,
          right: spacing::SPACE_2_5,
        }),
      )
      .on_press(Message::ComposeFromChanged(pilot.id)),
    );
  }
  container(column)
    .width(Length::Fill)
    .padding(spacing::UNIT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.16),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

/// The "Discard" footer affordance: closes the window without saving a draft. The native title-bar
/// close button auto-saves a non-empty draft; this is the explicit throw-away path.
fn discard_button<'a>() -> Element<'a, Message> {
  let button = container(
    text("Discard")
      .size(typography::size::MD)
      .font(typography::body::MEDIUM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  mouse_area(button).on_press(Message::ComposeDiscarded).into()
}

fn send_button<'a>(enabled: bool) -> Element<'a, Message> {
  let (fg, bg) = if enabled {
    (color::surface::BASE, color::accent::PLASMA)
  } else {
    (color::text::tertiary(), color::with_alpha(color::text::PRIMARY, 0.05))
  };
  let button = container(
    Row::with_children(vec![
      text("Send")
        .size(typography::size::MD)
        .font(typography::body::MEDIUM)
        .style(move |_| text::Style {
          color: Some(fg),
        })
        .into(),
      Icon::arrow_out().size(12.0).color(fg).render::<Message>(),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  if enabled {
    mouse_area(button).on_press(Message::ComposeSend).into()
  } else {
    button.into()
  }
}

fn field_row<'a>(label: &str, content: Element<'a, Message>) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    container(eyebrow_text(label, None)).width(Length::Fixed(56.0)).into(),
    container(content).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center);

  let field = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  });

  Column::with_children(vec![field.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn transparent_editor(_theme: &iced::Theme, _status: text_editor::Status) -> text_editor::Style {
  text_editor::Style {
    background: Background::Color(iced::Color::TRANSPARENT),
    border: Border {
      color: iced::Color::TRANSPARENT,
      radius: 0.0.into(),
      width: 0.0,
    },
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::accent::PLASMA_MUTED,
  }
}

fn link_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::rule(),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::accent::PLASMA_MUTED,
  }
}

fn transparent_input(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(iced::Color::TRANSPARENT),
    border: Border {
      color: iced::Color::TRANSPARENT,
      radius: 0.0.into(),
      width: 0.0,
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::accent::PLASMA_MUTED,
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store::{
    self,
    model::{
      Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, CharacterMailRecipient, Corporation, Gender,
      Race,
    },
    repo::character,
  };

  fn entity(id: i64, name: &str) -> EntityRef {
    EntityRef {
      id,
      kind: EntityKind::Character,
      name: name.to_owned(),
      portrait: None,
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_a_picked_to_recipient_and_clears_the_query() {
      let mut draft = Draft::blank(42);
      update(&mut draft, Message::ComposeToInput("Vex".to_owned()));

      update(&mut draft, Message::ComposeToPicked(entity(95_000_001, "Vex Voronova")));

      assert_eq!(draft.to.len(), 1);
      assert_eq!(draft.to[0].id, Some(95_000_001));
      assert!(draft.to_search.query().is_empty());
    }

    #[test]
    fn it_commits_a_typed_recipient_without_an_id() {
      let mut draft = Draft::blank(42);
      update(&mut draft, Message::ComposeToInput("Typed Pilot".to_owned()));

      update(&mut draft, Message::ComposeToCommitted);

      assert_eq!(draft.to.len(), 1);
      assert_eq!(draft.to[0].id, None);
      assert_eq!(draft.to[0].name, "Typed Pilot");
    }

    #[test]
    fn it_discards_recipient_results_from_a_superseded_search() {
      let mut draft = Draft::blank(42);
      update(&mut draft, Message::ComposeToInput("Vex".to_owned()));
      let stale = draft.recipient_search_generation(true);
      update(&mut draft, Message::ComposeToInput("Vexor".to_owned()));

      update(
        &mut draft,
        Message::ComposeToSearched {
          generation: stale,
          results: vec![entity(1, "Stale")],
        },
      );

      assert!(draft.to_search.results().is_empty());
    }

    #[test]
    fn it_reports_a_recipient_search_effect_for_a_to_input() {
      let mut draft = Draft::blank(42);

      let effect = update(&mut draft, Message::ComposeToInput("Vex".to_owned()));

      assert!(matches!(
        effect,
        Effect::RecipientSearch {
          is_to: true,
          ..
        }
      ));
    }

    #[test]
    fn it_wraps_a_bold_tag_into_the_body() {
      let mut draft = Draft::blank(42);

      update(&mut draft, Message::ComposeBold);

      assert_eq!(draft.body.text(), "<b></b>");
    }

    #[test]
    fn it_signals_discard_on_the_discard_message() {
      let mut draft = Draft::blank(42);

      assert!(matches!(update(&mut draft, Message::ComposeDiscarded), Effect::Discard));
    }

    #[test]
    fn it_signals_send_only_for_a_sendable_draft() {
      let mut draft = Draft::blank(42);
      assert!(matches!(update(&mut draft, Message::ComposeSend), Effect::None));

      draft.push_to(Recipient::typed("Vex"));
      draft.subject = "CTA".to_owned();
      assert!(matches!(update(&mut draft, Message::ComposeSend), Effect::Send));
    }

    #[test]
    fn it_records_a_failed_send_error_inline() {
      let mut draft = Draft::blank(42);

      update(&mut draft, Message::ComposeSent(Err("boom".to_owned())));

      assert_eq!(draft.error.as_deref(), Some("boom"));
    }

    #[test]
    fn it_toggles_the_link_popover() {
      let mut draft = Draft::blank(42);

      update(&mut draft, Message::ComposeLinkToggled);
      assert!(draft.link.is_some());

      update(&mut draft, Message::ComposeLinkToggled);
      assert!(draft.link.is_none());
    }
  }

  mod apply_link {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_selects_a_kind_and_resets_the_typed_url() {
      let mut draft = Draft::blank(42);
      apply_link(&mut draft, Message::ComposeLinkToggled).unwrap();
      apply_link(&mut draft, Message::ComposeLinkUrlChanged("example.com".to_owned())).unwrap();

      apply_link(&mut draft, Message::ComposeLinkKindSelected(LinkKind::Character)).unwrap();

      let popover = draft.link.as_ref().unwrap();
      assert_eq!(popover.kind, LinkKind::Character);
      assert!(popover.url.is_empty(), "switching kinds clears a typed http url");
    }

    #[test]
    fn it_records_a_typed_url_into_the_open_popover() {
      let mut draft = Draft::blank(42);
      apply_link(&mut draft, Message::ComposeLinkToggled).unwrap();

      apply_link(&mut draft, Message::ComposeLinkUrlChanged("foo.example".to_owned())).unwrap();

      assert_eq!(draft.link.as_ref().unwrap().url, "foo.example");
    }

    #[test]
    fn it_drops_a_url_change_when_no_popover_is_open() {
      let mut draft = Draft::blank(42);

      apply_link(&mut draft, Message::ComposeLinkUrlChanged("foo.example".to_owned())).unwrap();

      assert!(draft.link.is_none());
    }

    #[test]
    fn it_accepts_link_results_for_the_live_generation_and_drops_a_stale_one() {
      let mut draft = Draft::blank(42);
      apply_link(&mut draft, Message::ComposeLinkToggled).unwrap();
      apply_link(&mut draft, Message::ComposeLinkKindSelected(LinkKind::Character)).unwrap();
      let generation = draft.link_search().unwrap().0;

      apply_link(
        &mut draft,
        Message::ComposeLinkSearched {
          generation,
          results: vec![entity(1, "Vex")],
        },
      )
      .unwrap();
      assert_eq!(draft.link.as_ref().unwrap().results.len(), 1);

      apply_link(
        &mut draft,
        Message::ComposeLinkSearched {
          generation: generation.wrapping_sub(1),
          results: vec![entity(2, "Stale")],
        },
      )
      .unwrap();
      assert_eq!(
        draft.link.as_ref().unwrap().results.len(),
        1,
        "a stale generation does not overwrite the live results"
      );
    }

    #[test]
    fn it_inserts_entity_markup_and_closes_the_popover_on_pick() {
      let mut draft = Draft::blank(42);
      apply_link(&mut draft, Message::ComposeLinkToggled).unwrap();
      apply_link(&mut draft, Message::ComposeLinkKindSelected(LinkKind::Character)).unwrap();

      apply_link(&mut draft, Message::ComposeLinkPicked(entity(95_000_001, "Vex"))).unwrap();

      assert!(draft.body.text().contains("showinfo:1377//95000001"));
      assert!(draft.link.is_none(), "a successful pick closes the popover");
    }

    #[test]
    fn it_keeps_the_popover_open_when_picking_under_the_http_kind() {
      let mut draft = Draft::blank(42);
      apply_link(&mut draft, Message::ComposeLinkToggled).unwrap();

      apply_link(&mut draft, Message::ComposeLinkPicked(entity(95_000_001, "Vex"))).unwrap();

      assert!(draft.body.text().is_empty(), "http has no entity markup to insert");
      assert!(draft.link.is_some());
    }

    #[test]
    fn it_inserts_an_http_link_and_closes_the_popover_on_insert() {
      let mut draft = Draft::blank(42);
      apply_link(&mut draft, Message::ComposeLinkToggled).unwrap();
      apply_link(&mut draft, Message::ComposeLinkUrlChanged("example.com/x".to_owned())).unwrap();

      apply_link(&mut draft, Message::ComposeLinkInsert).unwrap();

      assert!(draft.body.text().contains("http://example.com/x"));
      assert!(draft.link.is_none());
    }

    #[test]
    fn it_does_nothing_on_insert_with_an_empty_url() {
      let mut draft = Draft::blank(42);
      apply_link(&mut draft, Message::ComposeLinkToggled).unwrap();

      apply_link(&mut draft, Message::ComposeLinkInsert).unwrap();

      assert!(draft.body.text().is_empty());
      assert!(draft.link.is_some(), "an empty url leaves the popover open");
    }

    #[test]
    fn it_returns_the_message_back_for_an_unrelated_kind() {
      let mut draft = Draft::blank(42);

      let returned = apply_link(&mut draft, Message::ComposeSubjectChanged("Hi".to_owned()));

      assert!(matches!(returned, Err(Message::ComposeSubjectChanged(_))));
    }
  }

  fn render() -> MailRender {
    MailRender {
      body: CharacterMailBody {
        body: "<p>Form up at <b>Jita</b>.</p>".to_owned(),
        character_id: 42,
        mail_id: 7,
      },
      header: CharacterMail {
        character_id: 42,
        from_id: 95_000_001,
        from_name: "Vex Voronova".to_owned(),
        is_read: false,
        mail_id: 7,
        subject: Some("CTA tonight".to_owned()),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
        ..Default::default()
      },
      label_ids: vec![8],
      recipients: vec![
        CharacterMailRecipient {
          character_id: 42,
          mail_id: 7,
          recipient_id: 42,
          recipient_name: "Me".to_owned(),
          recipient_type: "character".to_owned(),
        },
        CharacterMailRecipient {
          character_id: 42,
          mail_id: 7,
          recipient_id: 95_000_009,
          recipient_name: "Other Pilot".to_owned(),
          recipient_type: "character".to_owned(),
        },
      ],
      recipients_display: "Me, Other Pilot".to_owned(),
    }
  }

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

  #[test]
  fn can_send_requires_a_recipient_and_subject() {
    let mut draft = Draft::blank(42);
    assert!(!draft.can_send());
    draft.to.push(Recipient::typed("Pilot"));
    assert!(!draft.can_send());
    draft.subject = "Hello".to_owned();
    assert!(draft.can_send());
  }

  #[tokio::test]
  async fn enqueue_send_appends_a_mail_send_outbox_row() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    let mut draft = Draft::blank(42);
    draft.to.push(Recipient::typed("Vex Voronova"));
    draft.subject = "Hello".to_owned();
    draft.body = text_editor::Content::with_text("Hi there");

    enqueue_send(db.clone(), draft).await.unwrap();

    let count = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM outbox WHERE kind = 'mail.send' AND subject_id = 42 AND status = 'pending'",
    )
    .fetch_one(&db.0)
    .await
    .unwrap();
    assert_eq!(count, 1);
  }

  #[tokio::test]
  async fn enqueue_send_writes_an_optimistic_self_sent_mail() {
    let db = store::open_test().await.unwrap();
    seed_character(&db, 42).await;
    let mut draft = Draft::blank(42);
    draft.to.push(Recipient::character("Vex Voronova", 95_000_001));
    draft.subject = "Hello".to_owned();
    draft.body = text_editor::Content::with_text("Hi <b>there</b>");

    enqueue_send(db.clone(), draft).await.unwrap();

    let headers = mail::headers(&db, 42).await.unwrap();
    let sent: Vec<_> = headers.iter().filter(|h| h.from_id() == 42).collect();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].mail_id() < 0, "the placeholder carries a negative temp id");
    assert!(sent[0].is_read());
    assert_eq!(sent[0].subject().as_deref(), Some("Hello"));

    let body = mail::body(&db, 42, sent[0].mail_id()).await.unwrap().unwrap();
    assert_eq!(body.body, "Hi <b>there</b>");
  }

  #[test]
  fn forward_uses_fwd_subject_and_no_recipients() {
    let draft = Draft::from_mail(Kind::Forward, &render());

    assert_eq!(draft.subject, "Fwd: CTA tonight");
    assert!(draft.to.is_empty());
    assert!(draft.quote.as_deref().unwrap().contains("Form up at Jita"));
  }

  #[test]
  fn re_prefix_is_idempotent() {
    assert_eq!(prefixed("Re: hi", "Re: "), "Re: hi");
    assert_eq!(prefixed("hi", "Re: "), "Re: hi");
  }

  #[test]
  fn reply_all_moves_other_participants_to_cc_excluding_sender_and_self() {
    let draft = Draft::from_mail(Kind::ReplyAll, &render());

    assert_eq!(draft.to[0].id, Some(95_000_001));
    assert_eq!(draft.cc.len(), 1);
    assert_eq!(draft.cc[0].id, Some(95_000_009));
    assert!(draft.show_cc);
  }

  #[test]
  fn from_persisted_round_trips_recipients_subject_body_kind_and_quote() {
    let row = MailDraft {
      body: "<b>Form up</b>".to_owned(),
      character_id: 42,
      created_at: "2026-06-18T10:00:00Z".to_owned(),
      id: 7,
      kind: "ReplyAll".to_owned(),
      quote: Some("From Vex:\nhi".to_owned()),
      recipients_cc: r#"[{"id":95000009,"name":"Alt","recipient_type":"character"}]"#.to_owned(),
      recipients_to: r#"[{"id":95000001,"name":"Vex","recipient_type":"character"}]"#.to_owned(),
      subject: "Re: CTA".to_owned(),
      updated_at: "2026-06-18T10:05:00Z".to_owned(),
    };

    let draft = Draft::from_persisted(&row);

    assert_eq!(draft.id, Some(7));
    assert_eq!(draft.kind, Kind::ReplyAll);
    assert_eq!(draft.subject, "Re: CTA");
    assert_eq!(draft.body.text(), "<b>Form up</b>");
    assert_eq!(draft.quote.as_deref(), Some("From Vex:\nhi"));
    assert_eq!(draft.to[0].id, Some(95_000_001));
    assert_eq!(draft.cc[0].id, Some(95_000_009));
    assert_eq!(draft.to_chips.len(), 1);
    assert_eq!(draft.cc_chips.len(), 1);
    assert!(draft.show_cc);
  }

  #[test]
  fn is_empty_guards_a_blank_compose_but_not_a_typed_one() {
    let mut draft = Draft::blank(42);
    assert!(draft.is_empty());

    draft.subject = "   ".to_owned();
    assert!(draft.is_empty());

    draft.subject = "CTA".to_owned();
    assert!(!draft.is_empty());
  }

  #[test]
  fn persist_input_serialises_recipients_and_kind() {
    let mut draft = Draft::blank(42);
    draft.kind = Kind::Forward;
    draft.subject = "Fwd".to_owned();
    draft.push_to(Recipient::character("Vex", 95_000_001));

    let input = draft.persist_input();

    assert_eq!(input.character_id, 42);
    assert_eq!(input.kind, "Forward");
    assert_eq!(input.subject, "Fwd");
    assert!(input.recipients_to.contains("95000001"));
    assert_eq!(input.recipients_cc, "[]");
  }

  #[test]
  fn reply_prefills_sender_subject_quote_and_pins_from() {
    let draft = Draft::from_mail(Kind::Reply, &render());

    assert_eq!(draft.kind, Kind::Reply);
    assert_eq!(draft.from_character_id, 42);
    assert_eq!(draft.subject, "Re: CTA tonight");
    assert_eq!(draft.to.len(), 1);
    assert_eq!(draft.to[0].id, Some(95_000_001));
    assert!(draft.quote.as_deref().unwrap().contains("Form up at Jita"));
  }

  #[test]
  fn send_payload_merges_to_and_cc() {
    let mut draft = Draft::blank(42);
    draft.to.push(Recipient::typed("A"));
    draft.cc.push(Recipient::typed("B"));
    let payload = SendPayload::from_draft(&draft, -1);
    assert_eq!(payload.recipients.len(), 2);
    assert_eq!(payload.from_character_id, 42);
  }

  mod from_entity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_entity_kind_to_the_recipient_type() {
      let corp = EntityRef {
        id: 98_000_001,
        kind: EntityKind::Corporation,
        name: "Test Corp".to_owned(),
        portrait: None,
      };

      let recipient = Recipient::from_entity(corp);

      assert_eq!(recipient.id, Some(98_000_001));
      assert_eq!(recipient.recipient_type.as_deref(), Some("corporation"));
    }
  }

  mod wrap_emphasis {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_inserts_an_empty_tag_pair_at_the_cursor_without_a_selection() {
      let mut draft = Draft::blank(42);

      draft.wrap_emphasis(EmphasisKind::Bold);

      assert_eq!(draft.body.text(), "<b></b>");
    }

    #[test]
    fn it_inserts_an_italic_pair() {
      let mut draft = Draft::blank(42);

      draft.wrap_emphasis(EmphasisKind::Italic);

      assert_eq!(draft.body.text(), "<i></i>");
    }

    #[test]
    fn it_wraps_the_current_selection() {
      let mut draft = Draft::blank(42);
      draft.body = text_editor::Content::with_text("Form up");
      draft.body.perform(text_editor::Action::SelectAll);

      draft.wrap_emphasis(EmphasisKind::Bold);

      assert_eq!(draft.body.text(), "<b>Form up</b>");
    }
  }

  mod link_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_entity_markup_for_searchable_kinds() {
      let link = LinkKind::Character
        .link_for(95_000_001, "Pod Pilot".to_owned())
        .unwrap();

      assert_eq!(link.to_markup(), "<a href=\"showinfo:1377//95000001\">Pod Pilot</a>");
    }

    #[test]
    fn it_has_no_entity_markup_for_http() {
      assert!(LinkKind::Http.link_for(1, "x".to_owned()).is_none());
    }

    #[test]
    fn it_has_a_label_for_each_kind() {
      assert_eq!(LinkKind::Character.label(), "Character");
      assert_eq!(LinkKind::Corporation.label(), "Corporation");
      assert_eq!(LinkKind::Http.label(), "http://");
      assert_eq!(LinkKind::SolarSystem.label(), "Solar System");
      assert_eq!(LinkKind::Station.label(), "Station");
    }

    #[test]
    fn it_has_a_placeholder_for_each_kind() {
      assert_eq!(LinkKind::Character.placeholder(), "Search characters\u{2026}");
      assert_eq!(LinkKind::Corporation.placeholder(), "Search corporations\u{2026}");
      assert_eq!(LinkKind::Http.placeholder(), "example.com/path");
      assert_eq!(LinkKind::SolarSystem.placeholder(), "Search solar systems\u{2026}");
      assert_eq!(LinkKind::Station.placeholder(), "Search stations\u{2026}");
    }

    #[test]
    fn it_maps_only_searchable_kinds_to_a_category() {
      assert!(LinkKind::Http.category().is_none());
      assert!(LinkKind::Character.category().is_some());
      assert!(LinkKind::Station.category().is_some());
    }
  }

  mod link_popover {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_normalises_a_bare_host_to_an_http_url() {
      let popover = LinkPopover {
        kind: LinkKind::Http,
        url: "example.com/op".to_owned(),
        ..LinkPopover::default()
      };

      let link = popover.http_link().unwrap();

      assert_eq!(
        link.to_markup(),
        "<a href=\"http://example.com/op\">http://example.com/op</a>"
      );
    }

    #[test]
    fn it_keeps_an_explicit_scheme() {
      let popover = LinkPopover {
        kind: LinkKind::Http,
        url: "https://example.com".to_owned(),
        ..LinkPopover::default()
      };

      let link = popover.http_link().unwrap();

      assert_eq!(
        link.to_markup(),
        "<a href=\"https://example.com\">https://example.com</a>"
      );
    }

    #[test]
    fn it_cannot_insert_an_empty_url() {
      let popover = LinkPopover::default();

      assert!(!popover.can_insert());
      assert!(popover.http_link().is_none());
    }

    #[test]
    fn it_renders_the_http_url_input_variant() {
      let popover = LinkPopover {
        kind: LinkKind::Http,
        url: "example.com".to_owned(),
        ..LinkPopover::default()
      };

      let _: Element<'_, Message> = link_popover(&popover);
    }

    #[test]
    fn it_renders_the_entity_search_variant() {
      let popover = LinkPopover {
        kind: LinkKind::Character,
        ..LinkPopover::default()
      };

      let _: Element<'_, Message> = link_popover(&popover);
    }
  }

  mod link_radio {
    use super::*;

    #[test]
    fn it_renders_a_selected_radio() {
      let _: Element<'_, Message> = link_radio(LinkKind::Character, true);
    }

    #[test]
    fn it_renders_an_unselected_radio() {
      let _: Element<'_, Message> = link_radio(LinkKind::Http, false);
    }
  }
}
