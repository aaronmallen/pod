use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text},
};
use serde::Serialize;

use super::{Message, loaders::FolderLabel};
use crate::{
  store::{
    Database,
    model::{CharacterMailLabel, OwnerType},
    repo::{infra, mail},
  },
  ui::{
    components::{color_picker, icon::Icon, rule, text_input::TextInput},
    style::{color, control, radius, shadow, spacing, typography},
  },
};

pub const NAME_MAX_CHARS: usize = 40;
const DEFAULT_COLOR: &str = "#ffff01";
const MENU_WIDTH: f32 = 234.0;
const MODAL_BODY_PAD: f32 = 20.0;
const MODAL_SIDE_PAD: f32 = 18.0;
const MODAL_WIDTH: f32 = 440.0;
const PICKER_MAX_HEIGHT: f32 = 240.0;
const SWATCH_RADIUS: f32 = 3.0;
const SWATCH_SIZE: f32 = 11.0;

/// EVE system label ids (Inbox/Sent/Corp/Alliance). These are synthesized into
/// `character_mail_labels` during sync to satisfy the membership FK, but must
/// never be shown as user labels in the folder pane or as row/reading chips.
pub(crate) const SYSTEM_LABEL_IDS: [i64; 4] = [1, 2, 4, 8];

/// Whether `id` is an EVE system label that should be hidden from the label UI.
pub(crate) fn is_system_label(id: i64) -> bool {
  SYSTEM_LABEL_IDS.contains(&id)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LabelDraft {
  pub color: String,
  pub name: String,
}

impl LabelDraft {
  pub fn blank() -> Self {
    LabelDraft {
      color: DEFAULT_COLOR.to_owned(),
      name: String::new(),
    }
  }

  pub fn can_create(&self) -> bool {
    !self.name.trim().is_empty()
  }
}

#[derive(Debug, Serialize)]
struct CreatePayload {
  character_id: i64,
  color: Option<String>,
  label_id: i64,
  name: String,
}

#[derive(Debug, Serialize)]
struct DeletePayload {
  character_id: i64,
  label_id: i64,
}

#[derive(Debug, Serialize)]
struct SetPayload {
  character_id: i64,
  labels: Vec<i64>,
  mail_id: i64,
  previous: Vec<i64>,
}

/// Returns a negative, millisecond-epoch-derived id used as an optimistic sentinel for a label
/// that has not yet been created on the server. The negativity is load-bearing: sync preserves
/// rows with `label_id < 0`, and the `mail.create_label` outbox handler remaps this id to the
/// real server-assigned value on execute.
pub(super) fn temp_label_id() -> i64 {
  let millis = chrono::Utc::now().timestamp_millis();
  -millis.max(1)
}

/// Writes the optimistic local mirror first, then appends the outbox row. The outbox drainer does
/// not call a handler `apply()`; the feature layer is solely responsible for the optimistic write.
pub(super) async fn enqueue_create(db: Database, character_id: i64, label_id: i64, draft: LabelDraft) {
  let name = draft.name.trim().to_owned();
  let color = Some(draft.color.clone());
  let optimistic = CharacterMailLabel {
    character_id,
    color: color.clone(),
    label_id,
    name: name.clone(),
  };
  let _ = mail::insert_label(&db, &optimistic).await;

  let payload = CreatePayload {
    character_id,
    color,
    label_id,
    name,
  };
  enqueue(&db, character_id, "mail.create_label", &payload, None).await;
}

pub(super) async fn enqueue_delete(db: Database, character_id: i64, label_id: i64) {
  let _ = mail::delete_label(&db, character_id, label_id).await;

  let payload = DeletePayload {
    character_id,
    label_id,
  };
  let dedupe = format!("delete_label:{label_id}");
  enqueue(&db, character_id, "mail.delete_label", &payload, Some(&dedupe)).await;
}

pub(super) async fn enqueue_toggle(db: Database, character_id: i64, mail_id: i64, label_id: i64) {
  let previous = mail::membership(&db, character_id, mail_id).await.unwrap_or_default();
  let labels = toggled_set(&previous, label_id);

  apply_membership(&db, character_id, mail_id, &previous, &labels).await;

  let payload = SetPayload {
    character_id,
    labels,
    mail_id,
    previous,
  };
  let dedupe = format!("set_labels:{mail_id}");
  enqueue(&db, character_id, "mail.set_labels", &payload, Some(&dedupe)).await;
}

pub(super) async fn enqueue_assign(db: Database, character_id: i64, mail_id: i64, label_id: i64) {
  let previous = mail::membership(&db, character_id, mail_id).await.unwrap_or_default();
  if previous.contains(&label_id) {
    return;
  }
  let mut labels = previous.clone();
  labels.push(label_id);

  apply_membership(&db, character_id, mail_id, &previous, &labels).await;

  let payload = SetPayload {
    character_id,
    labels,
    mail_id,
    previous,
  };
  let dedupe = format!("set_labels:{mail_id}");
  enqueue(&db, character_id, "mail.set_labels", &payload, Some(&dedupe)).await;
}

async fn apply_membership(db: &Database, character_id: i64, mail_id: i64, previous: &[i64], labels: &[i64]) {
  for label_id in previous {
    if !labels.contains(label_id) {
      let _ = mail::remove_membership(db, character_id, mail_id, *label_id).await;
    }
  }
  for label_id in labels {
    if !previous.contains(label_id) {
      let _ = mail::add_membership(db, character_id, mail_id, *label_id).await;
    }
  }
}

async fn enqueue(db: &Database, character_id: i64, kind: &str, payload: &impl Serialize, dedupe_key: Option<&str>) {
  let Ok(json) = serde_json::to_string(payload) else {
    return;
  };
  let _ = infra::append(db, OwnerType::Character, character_id, kind, &json, dedupe_key).await;
}

fn toggled_set(current: &[i64], label_id: i64) -> Vec<i64> {
  if current.contains(&label_id) {
    current.iter().copied().filter(|id| *id != label_id).collect()
  } else {
    let mut next = current.to_vec();
    next.push(label_id);
    next
  }
}

pub(super) fn create_modal(draft: &LabelDraft) -> Element<'_, Message> {
  let eyebrow = text("Mail label")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });
  let title = text("New label")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let close = button(
    Icon::close()
      .size(16.0)
      .color(color::text::secondary())
      .render::<Message>(),
  )
  .padding(spacing::UNIT + 2.0)
  .on_press(Message::LabelModalClosed)
  .style(|_, status| ghost_icon_style(status));
  let header = container(
    Row::with_children(vec![
      Column::with_children(vec![eyebrow.into(), title.into()])
        .spacing(spacing::UNIT)
        .width(Length::Fill)
        .into(),
      close.into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3 + 2.0,
    bottom: spacing::SPACE_3 + 2.0,
    left: MODAL_SIDE_PAD,
    right: MODAL_SIDE_PAD,
  });

  let name_label = field_label("Name");
  let name_input = TextInput::new("e.g. Coalition", &draft.name, Message::LabelNameChanged)
    .background(color::surface::SUNKEN)
    .font_size(typography::size::MD)
    .on_submit(Message::LabelModalSubmitted)
    .width(Length::Fill)
    .render();
  let color_label = Row::with_children(vec![
    field_label("Color"),
    Space::new().width(Length::Fill).into(),
    text(draft.color.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .align_y(Vertical::Bottom);
  let grid = color_picker::label_color_grid(&draft.color, Message::LabelColorPicked);

  let body = container(
    Column::with_children(vec![
      name_label,
      name_input,
      container(color_label)
        .width(Length::Fill)
        .padding(Padding {
          top: spacing::SPACE_3_5,
          bottom: spacing::SPACE_2,
          left: 0.0,
          right: 0.0,
        })
        .into(),
      grid,
    ])
    .spacing(spacing::SPACE_2),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: MODAL_BODY_PAD,
    bottom: MODAL_BODY_PAD,
    left: MODAL_SIDE_PAD,
    right: MODAL_SIDE_PAD,
  });

  let footer = container(footer_row(draft))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: MODAL_SIDE_PAD,
      right: MODAL_SIDE_PAD,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  let card = container(
    Column::with_children(vec![
      header.into(),
      rule::horizontal(),
      body.into(),
      rule::horizontal(),
      footer.into(),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fixed(MODAL_WIDTH))
  .clip(true)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    shadow: shadow::CARD,
    ..container::Style::default()
  });

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn footer_row(draft: &LabelDraft) -> Element<'_, Message> {
  let preview_name = draft.name.trim();
  let preview = Row::with_children(vec![
    swatch(Some(draft.color.as_str())),
    text(if preview_name.is_empty() {
      "Label preview".to_owned()
    } else {
      preview_name.to_owned()
    })
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(if preview_name.is_empty() {
        color::text::tertiary()
      } else {
        color::text::PRIMARY
      }),
    })
    .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let cancel = button(text("Cancel").font(typography::body::MEDIUM).size(typography::size::MD))
    .padding(control::padding())
    .on_press(Message::LabelModalClosed)
    .style(control::ghost_button);

  let mut create = button(
    text("Create label")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .style(control::primary_button);
  if draft.can_create() {
    create = create.on_press(Message::LabelModalSubmitted);
  }

  Row::with_children(vec![preview.into(), cancel.into(), create.into()])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
}

pub(super) fn toggle_picker<'a>(mail_id: i64, labels: &'a [FolderLabel], applied: &[i64]) -> Element<'a, Message> {
  let mut list = Column::new().width(Length::Fill);
  if labels.is_empty() {
    list = list.push(
      container(
        text("No labels yet.")
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          }),
      )
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_2_5,
        right: spacing::SPACE_2_5,
      }),
    );
  } else {
    for label in labels {
      list = list.push(picker_row(mail_id, label, applied.contains(&label.label_id)));
    }
  }

  let scroll = iced::widget::scrollable(list)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Shrink);

  let new_label = button(
    Row::with_children(vec![
      Icon::plus()
        .size(13.0)
        .color(color::text::secondary())
        .render::<Message>(),
      text("New label\u{2026}")
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  })
  .on_press(Message::LabelModalOpened)
  .style(|_, status| picker_row_style(false, status));

  let body = Column::with_children(vec![
    picker_header("Apply labels"),
    container(scroll)
      .max_height(PICKER_MAX_HEIGHT)
      .width(Length::Fill)
      .into(),
    rule::horizontal(),
    new_label.into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  container(body)
    .width(Length::Fixed(MENU_WIDTH))
    .padding(spacing::UNIT + 2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

fn picker_header<'a>(label: &str) -> Element<'a, Message> {
  container(
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2 - 2.0,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  })
  .into()
}

fn picker_row(mail_id: i64, label: &FolderLabel, applied: bool) -> Element<'_, Message> {
  let check: Element<'_, Message> = if applied {
    Icon::check()
      .size(14.0)
      .color(color::accent::PLASMA)
      .render::<Message>()
  } else {
    Space::new().width(Length::Fixed(14.0)).into()
  };

  let row = Row::with_children(vec![
    swatch(label.color.as_deref()),
    text(label.name.clone())
      .size(typography::size::MD)
      .width(Length::Fill)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    check,
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .on_press(Message::LabelToggled(mail_id, label.label_id))
    .style(move |_, status| picker_row_style(applied, status))
    .into()
}

fn picker_row_style(applied: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let background = if applied {
    Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.08)))
  } else if hovered {
    Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05)))
  } else {
    None
  };
  button::Style {
    background,
    text_color: color::text::PRIMARY,
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn ghost_icon_style(status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn field_label<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    })
    .into()
}

fn swatch<'a>(hex: Option<&str>) -> Element<'a, Message> {
  let fill = hex
    .and_then(color::from_hex)
    .unwrap_or_else(|| color::with_alpha(color::text::PRIMARY, 0.3));
  container(Space::new())
    .width(Length::Fixed(SWATCH_SIZE))
    .height(Length::Fixed(SWATCH_SIZE))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: color::with_alpha(iced::Color::BLACK, 0.35),
        radius: SWATCH_RADIUS.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod label_draft {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_starts_blank_with_the_default_color_and_cannot_create() {
      let draft = LabelDraft::blank();

      assert_eq!(draft.color, DEFAULT_COLOR);
      assert!(draft.name.is_empty());
      assert!(!draft.can_create());
    }

    #[test]
    fn it_can_create_once_a_non_blank_name_is_present() {
      let mut draft = LabelDraft::blank();

      draft.name = "  ".to_owned();
      assert!(!draft.can_create());

      draft.name = "Coalition".to_owned();
      assert!(draft.can_create());
    }
  }

  mod temp_label_id {
    use super::*;

    #[test]
    fn it_is_always_negative() {
      assert!(temp_label_id() < 0);
    }
  }

  mod is_system_label {
    use super::*;

    #[test]
    fn it_flags_the_four_eve_system_label_ids() {
      for id in SYSTEM_LABEL_IDS {
        assert!(is_system_label(id));
      }
    }

    #[test]
    fn it_treats_user_label_ids_as_non_system() {
      assert!(!is_system_label(7000));
      assert!(!is_system_label(0));
      assert!(!is_system_label(-1));
      // EVE system label bits are exactly 1/2/4/8 — other small ids are user labels.
      assert!(!is_system_label(3));
      assert!(!is_system_label(16));
    }
  }

  mod toggled_set {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_an_absent_label_and_removes_a_present_one() {
      assert_eq!(toggled_set(&[1, 2], 3), vec![1, 2, 3]);
      assert_eq!(toggled_set(&[1, 2, 3], 2), vec![1, 3]);
    }
  }

  mod payloads {
    use super::*;

    #[test]
    fn it_serializes_a_create_payload_the_handler_can_parse() {
      let payload = CreatePayload {
        character_id: 42,
        color: Some("#ff6600".to_owned()),
        label_id: -7,
        name: "Fleet".to_owned(),
      };

      let json = serde_json::to_string(&payload).unwrap();

      assert!(json.contains("\"character_id\":42"));
      assert!(json.contains("\"label_id\":-7"));
      assert!(json.contains("\"name\":\"Fleet\""));
      assert!(json.contains("\"color\":\"#ff6600\""));
    }

    #[test]
    fn it_serializes_a_set_payload_with_the_full_and_previous_sets() {
      let payload = SetPayload {
        character_id: 42,
        labels: vec![1, 2],
        mail_id: 7,
        previous: vec![1],
      };

      let json = serde_json::to_string(&payload).unwrap();

      assert!(json.contains("\"labels\":[1,2]"));
      assert!(json.contains("\"previous\":[1]"));
      assert!(json.contains("\"mail_id\":7"));
    }
  }

  mod render {
    use super::*;

    fn labels() -> Vec<FolderLabel> {
      vec![
        FolderLabel {
          color: Some("#ff6600".to_owned()),
          label_id: 1,
          name: "Fleet".to_owned(),
          unread: 0,
        },
        FolderLabel {
          color: None,
          label_id: 2,
          name: "Intel".to_owned(),
          unread: 0,
        },
      ]
    }

    #[test]
    fn it_renders_the_create_modal_with_a_named_draft() {
      let mut draft = LabelDraft::blank();
      draft.name = "Coalition".to_owned();

      let _el: Element<'_, Message> = create_modal(&draft);
    }

    #[test]
    fn it_renders_the_create_modal_with_a_blank_draft() {
      let draft = LabelDraft::blank();
      let _el: Element<'_, Message> = create_modal(&draft);
    }

    #[test]
    fn it_renders_the_toggle_picker_with_an_applied_label() {
      let catalog = labels();

      let _el: Element<'_, Message> = toggle_picker(7, &catalog, &[1]);
    }

    #[test]
    fn it_renders_the_empty_toggle_picker() {
      let _el: Element<'_, Message> = toggle_picker(7, &[], &[]);
    }
  }
}
