use std::sync::Arc;

use iced::{
  Background, Border, Element, Length, Padding,
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
      icon::Icon,
      picker::TriggerPortrait,
      rule,
    },
    style::{color, radius, spacing, typography},
  },
};

const DOCKED_WIDTH: f32 = 540.0;

const EXPANDED_WIDTH: f32 = 760.0;

const FROM_PORTRAIT_SIZE: f32 = 22.0;

const LINK_POPOVER_WIDTH: f32 = 320.0;

#[derive(Clone, Debug)]
pub struct Draft {
  pub body: text_editor::Content,
  pub cc: Vec<Recipient>,
  pub cc_chips: Vec<EntityRef>,
  pub cc_search: EntitySearch,
  pub error: Option<String>,
  pub expanded: bool,
  pub from_character_id: i64,
  pub from_picker_open: bool,
  /// The `mail_drafts` row id once this compose has been persisted; threaded back so every later
  /// save updates the same row and a successful send deletes it by id.
  pub id: Option<i64>,
  pub kind: Kind,
  pub link: Option<LinkPopover>,
  pub minimized: bool,
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
      expanded: false,
      from_character_id,
      from_picker_open: false,
      id: None,
      kind: Kind::New,
      link: None,
      minimized: false,
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

  pub(super) fn from_persisted(row: &MailDraft) -> Self {
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
  pub(super) fn category(self) -> Option<crate::features::entity_search::EntityCategory> {
    use crate::features::entity_search::EntityCategory;
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

  pub(super) fn header(self) -> &'static str {
    match self {
      Kind::New => "New message",
      Kind::Reply => "Reply",
      Kind::ReplyAll => "Reply all",
      Kind::Forward => "Forward",
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

pub(super) fn panel<'a>(draft: &'a Draft, roster: &'a [RosterPilot]) -> Element<'a, Message> {
  let body: Element<'a, Message> = if draft.minimized {
    header_bar(draft)
  } else {
    Column::with_children(vec![
      header_bar(draft),
      to_field(draft),
      cc_field(draft),
      subject_field(draft),
      body_field(draft),
      error_line(draft),
      footer(draft, roster),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
  };

  let width = if draft.expanded { EXPANDED_WIDTH } else { DOCKED_WIDTH };

  let card = container(body)
    .width(Length::Fixed(width))
    .height(if draft.minimized {
      Length::Shrink
    } else {
      Length::Fixed(560.0)
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.16),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let alignment = if draft.expanded {
    (Horizontal::Center, Vertical::Center)
  } else {
    (Horizontal::Right, Vertical::Bottom)
  };

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment.0)
    .align_y(alignment.1)
    .padding(Padding {
      top: 0.0,
      bottom: spacing::layout::STATUS_BAR_HEIGHT + spacing::SPACE_3_5,
      left: spacing::SPACE_6,
      right: spacing::SPACE_6,
    })
    .into()
}

fn header_bar(draft: &Draft) -> Element<'_, Message> {
  let row = Row::with_children(vec![
    eyebrow_text(draft.kind.header(), None).width(Length::Fill).into(),
    header_button("\u{2013}", Message::ComposeMinimizeToggled),
    header_button(
      if draft.expanded { "\u{2921}" } else { "\u{2922}" },
      Message::ComposeExpandToggled,
    ),
    header_button("\u{2715}", Message::ComposeClosed),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center);

  let bar = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2 + 2.0,
    bottom: spacing::SPACE_2 + 2.0,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_2,
  });

  Column::with_children(vec![bar.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn header_button<'a>(glyph: &str, message: Message) -> Element<'a, Message> {
  mouse_area(
    container(
      text(glyph.to_owned())
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    )
    .width(Length::Fixed(28.0))
    .height(Length::Fixed(28.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center),
  )
  .on_press(message)
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
    text("\u{25be}")
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
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
  }
}
