use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, mouse_area, text, text_editor, text_input},
};
use serde::{Deserialize, Serialize};

use super::{Message, loaders::RosterPilot};
use crate::{
  store::{
    Database, images,
    model::{OwnerType, character_mail_view::MailRender},
    repo::infra,
  },
  ui::{
    components::{
      avatar::Avatar,
      entity_search::{EntityKind, EntityRef, EntitySearch, MultiSelect},
      eyebrow::eyebrow_text,
      picker::TriggerPortrait,
      rule,
    },
    style::{color, radius, spacing, typography},
  },
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Kind {
  Forward,
  #[default]
  New,
  Reply,
  ReplyAll,
}

impl Kind {
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
    Recipient::character(entity.name, entity.id)
  }

  pub(super) fn typed(name: impl Into<String>) -> Self {
    Recipient {
      id: None,
      name: name.into(),
      recipient_type: None,
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
  pub expanded: bool,
  pub from_character_id: i64,
  pub from_picker_open: bool,
  pub kind: Kind,
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
      kind: Kind::New,
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

  pub(super) fn can_send(&self) -> bool {
    !self.to.is_empty() && !self.subject.trim().is_empty()
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SendPayload {
  pub body: String,
  pub from_character_id: i64,
  pub recipients: Vec<Recipient>,
  pub subject: String,
}

impl SendPayload {
  fn from_draft(draft: &Draft) -> Self {
    let mut recipients = draft.to.clone();
    recipients.extend(draft.cc.iter().cloned());
    SendPayload {
      body: draft.body.text(),
      from_character_id: draft.from_character_id,
      recipients,
      subject: draft.subject.clone(),
    }
  }
}

pub(super) async fn enqueue_send(db: Database, draft: Draft) -> Result<(), String> {
  let payload = SendPayload::from_draft(&draft);
  let json = serde_json::to_string(&payload).map_err(|e| format!("could not build the send: {e}"))?;
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

const DOCKED_WIDTH: f32 = 540.0;
const EXPANDED_WIDTH: f32 = 760.0;

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
  .placeholder("Add recipient\u{2026}")
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
  .placeholder("Add Cc recipient\u{2026}")
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

  let row = Row::with_children(vec![from_trigger.into(), Space::new().width(Length::Fill).into(), send])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  let footer_bar = container(row)
    .width(Length::Fill)
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  if draft.from_picker_open && roster.len() > 1 {
    Column::with_children(vec![
      from_dropdown(draft, roster),
      rule::horizontal(),
      footer_bar.into(),
    ])
    .into()
  } else {
    Column::with_children(vec![rule::horizontal(), footer_bar.into()]).into()
  }
}

const FROM_PORTRAIT_SIZE: f32 = 22.0;

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
    text("Send")
      .size(typography::size::MD)
      .font(typography::body::MEDIUM)
      .style(move |_| text::Style {
        color: Some(fg),
      }),
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
  fn reply_all_moves_other_participants_to_cc_excluding_sender_and_self() {
    let draft = Draft::from_mail(Kind::ReplyAll, &render());

    assert_eq!(draft.to[0].id, Some(95_000_001));
    assert_eq!(draft.cc.len(), 1);
    assert_eq!(draft.cc[0].id, Some(95_000_009));
    assert!(draft.show_cc);
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
  fn can_send_requires_a_recipient_and_subject() {
    let mut draft = Draft::blank(42);
    assert!(!draft.can_send());
    draft.to.push(Recipient::typed("Pilot"));
    assert!(!draft.can_send());
    draft.subject = "Hello".to_owned();
    assert!(draft.can_send());
  }

  #[test]
  fn send_payload_merges_to_and_cc() {
    let mut draft = Draft::blank(42);
    draft.to.push(Recipient::typed("A"));
    draft.cc.push(Recipient::typed("B"));
    let payload = SendPayload::from_draft(&draft);
    assert_eq!(payload.recipients.len(), 2);
    assert_eq!(payload.from_character_id, 42);
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
}
