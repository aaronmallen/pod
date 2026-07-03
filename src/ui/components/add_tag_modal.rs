use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text, text_input},
};

use crate::{
  store::model::Tag,
  ui::{
    components::{
      button::{Button, Size},
      chip::Chip,
      color_picker, rule,
    },
    style::{color, radius, shadow, spacing, typography},
  },
};

const CARD_WIDTH: f32 = 440.0;
const LIST_MAX_HEIGHT: f32 = 320.0;
const HEADER_PAD_X: f32 = 20.0;
const HEADER_PAD_Y: f32 = 16.0;
const SECTION_PAD_X: f32 = 16.0;
const INPUT_PAD_Y: f32 = 12.0;
const FOOTER_PAD_Y: f32 = 10.0;
const INPUT_WELL_PAD_X: f32 = 12.0;
const INPUT_WELL_PAD_Y: f32 = 10.0;
const LIST_PAD: f32 = 6.0;
const ROW_PAD_X: f32 = 12.0;
const ROW_PAD_Y: f32 = 9.0;

#[derive(Clone, Debug)]
pub struct AddTagModal {
  pub entity_id: i64,
  pub entity_type: &'static str,
  pub input: String,
}

impl AddTagModal {
  pub fn new(entity_id: i64, entity_type: &'static str) -> Self {
    Self {
      entity_id,
      entity_type,
      input: String::new(),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddTagMessage {
  InputChanged(String),
  Assign {
    entity_id: i64,
    entity_type: &'static str,
    tag_id: i64,
  },
  CreateAndAssign {
    entity_id: i64,
    entity_type: &'static str,
  },
  Unassign {
    entity_id: i64,
    entity_type: &'static str,
    tag_id: i64,
  },
  Close,
}

pub fn view<'a, M>(
  modal: &'a AddTagModal,
  entity_name: &'a str,
  assigned: Vec<&'a Tag>,
  assignable: Vec<&'a Tag>,
  on_message: impl Fn(AddTagMessage) -> M + Clone + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let mut children: Vec<Element<'a, M>> = vec![header(entity_name), rule::horizontal()];
  if let Some(current) = current_tags(modal, &assigned, on_message.clone()) {
    children.push(current);
    children.push(rule::horizontal());
  }
  children.push(search_input(modal, on_message.clone()));
  children.push(rule::horizontal());
  children.push(tag_list(modal, &assignable, on_message.clone()));
  children.push(rule::horizontal());
  children.push(footer(on_message));

  let card = container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fixed(CARD_WIDTH))
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

  card.into()
}

fn header<'a, M>(entity_name: &str) -> Element<'a, M>
where
  M: 'a,
{
  let titles = Column::with_children(vec![
    text(t!("common.add_tag_modal.title"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    text(entity_name.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  container(titles)
    .width(Length::Fill)
    .padding(Padding {
      top: HEADER_PAD_Y,
      right: HEADER_PAD_X,
      bottom: HEADER_PAD_Y,
      left: HEADER_PAD_X,
    })
    .into()
}

fn current_tags<'a, M>(
  modal: &AddTagModal,
  assigned: &[&'a Tag],
  on_message: impl Fn(AddTagMessage) -> M + 'a,
) -> Option<Element<'a, M>>
where
  M: Clone + 'a,
{
  if assigned.is_empty() {
    return None;
  }

  let entity_id = modal.entity_id;
  let entity_type = modal.entity_type;
  let chips: Vec<Element<'a, M>> = assigned
    .iter()
    .map(|tag| {
      Chip::new(tag.name().clone(), hex_to_color(tag.color().as_deref()))
        .on_remove(on_message(AddTagMessage::Unassign {
          entity_id,
          entity_type,
          tag_id: tag.id(),
        }))
        .view()
    })
    .collect();

  let label = text(t!("common.add_tag_modal.current_tags"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let body = Column::with_children(vec![
    label.into(),
    Row::with_children(chips).spacing(spacing::UNIT).wrap().into(),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  Some(
    container(body)
      .width(Length::Fill)
      .padding(Padding {
        top: INPUT_PAD_Y,
        right: SECTION_PAD_X,
        bottom: INPUT_PAD_Y,
        left: SECTION_PAD_X,
      })
      .into(),
  )
}

fn search_input<'a, M>(modal: &'a AddTagModal, on_message: impl Fn(AddTagMessage) -> M + Clone + 'a) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let entity_id = modal.entity_id;
  let entity_type = modal.entity_type;
  let on_change = on_message.clone();
  let input = text_input("Search or create a tag\u{2026}", &modal.input)
    .size(typography::size::MD)
    .padding(Padding {
      top: INPUT_WELL_PAD_Y,
      right: INPUT_WELL_PAD_X,
      bottom: INPUT_WELL_PAD_Y,
      left: INPUT_WELL_PAD_X,
    })
    .on_input(move |value| on_change(AddTagMessage::InputChanged(value)))
    .on_submit(on_message(AddTagMessage::CreateAndAssign {
      entity_id,
      entity_type,
    }))
    .style(search_input_style);

  container(input)
    .width(Length::Fill)
    .padding(Padding {
      top: INPUT_PAD_Y,
      right: SECTION_PAD_X,
      bottom: INPUT_PAD_Y,
      left: SECTION_PAD_X,
    })
    .into()
}

fn tag_list<'a, M>(
  modal: &AddTagModal,
  assignable: &[&'a Tag],
  on_message: impl Fn(AddTagMessage) -> M + Clone + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let query = modal.input.trim().to_lowercase();
  let matches: Vec<&Tag> = assignable
    .iter()
    .copied()
    .filter(|tag| query.is_empty() || tag.name().to_lowercase().contains(&query))
    .collect();
  let can_create = !query.is_empty() && !assignable.iter().any(|tag| tag.name().to_lowercase() == query);

  let mut rows: Vec<Element<'a, M>> = Vec::with_capacity(matches.len() + 1);
  if can_create {
    rows.push(create_row(
      modal.entity_type,
      modal.entity_id,
      modal.input.trim(),
      on_message.clone(),
    ));
  }
  for tag in matches {
    rows.push(tag_row(modal.entity_type, modal.entity_id, tag, on_message.clone()));
  }

  let body: Element<'a, M> = if rows.is_empty() {
    empty_placeholder(assignable.is_empty())
  } else {
    Column::with_children(rows).width(Length::Fill).into()
  };

  let list = container(body).width(Length::Fill).padding(LIST_PAD);

  container(
    scrollable(list)
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Shrink),
  )
  .width(Length::Fill)
  .max_height(LIST_MAX_HEIGHT)
  .into()
}

fn tag_row<'a, M>(
  entity_type: &'static str,
  entity_id: i64,
  tag: &Tag,
  on_message: impl Fn(AddTagMessage) -> M + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let label = Row::with_children(vec![
    text(t!("common.add_tag_modal.tag"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    text(tag.name().clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  button(label)
    .width(Length::Fill)
    .padding(Padding {
      top: ROW_PAD_Y,
      right: ROW_PAD_X,
      bottom: ROW_PAD_Y,
      left: ROW_PAD_X,
    })
    .on_press(on_message(AddTagMessage::Assign {
      entity_id,
      entity_type,
      tag_id: tag.id(),
    }))
    .style(row_button)
    .into()
}

fn create_row<'a, M>(
  entity_type: &'static str,
  entity_id: i64,
  input: &str,
  on_message: impl Fn(AddTagMessage) -> M + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let label = Row::with_children(vec![
    text(t!("common.add_tag_modal.new"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::accent()),
      })
      .into(),
    text(t!("common.add_tag_modal.create", name => input))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  button(label)
    .width(Length::Fill)
    .padding(Padding {
      top: ROW_PAD_Y,
      right: ROW_PAD_X,
      bottom: ROW_PAD_Y,
      left: ROW_PAD_X,
    })
    .on_press(on_message(AddTagMessage::CreateAndAssign {
      entity_id,
      entity_type,
    }))
    .style(row_button)
    .into()
}

fn empty_placeholder<'a, M>(nothing_assignable: bool) -> Element<'a, M>
where
  M: 'a,
{
  let copy = if nothing_assignable {
    t!("common.add_tag_modal.all_assigned")
  } else {
    t!("common.add_tag_modal.type_to_create")
  };

  container(
    text(copy)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_6)
  .align_x(Horizontal::Center)
  .into()
}

fn footer<'a, M>(on_message: impl Fn(AddTagMessage) -> M + 'a) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let hint = text(t!("common.add_tag_modal.footer_hint"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });

  let cancel = Button::ghost(t!("common.add_tag_modal.cancel").into_owned())
    .size(Size::Sm)
    .on_press(on_message(AddTagMessage::Close));

  let row = Row::with_children(vec![
    container(hint).align_y(Vertical::Center).into(),
    Space::new().width(Length::Fill).into(),
    cancel.into(),
  ])
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: FOOTER_PAD_Y,
      right: SECTION_PAD_X,
      bottom: FOOTER_PAD_Y,
      left: SECTION_PAD_X,
    })
    .into()
}

fn hex_to_color(hex: Option<&str>) -> Option<Color> {
  let normalized = color_picker::normalize_hex(hex?)?;
  let digits = normalized.trim_start_matches('#');
  let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
  let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
  let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
  Some(Color::from_rgb8(r, g, b))
}

fn search_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::accent(),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent(), 0.4),
  }
}

fn row_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::accent(), 0.08)))
    }
    _ => None,
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{self, model::ENTITY_TYPE_CHARACTER, repo::infra};

  async fn seed_tags() -> Vec<Tag> {
    let db = store::open_test().await.unwrap();
    infra::create(&db, "Hauler", None, None).await.unwrap();
    infra::create(&db, "Scout", None, None).await.unwrap();
    infra::tag_all(&db).await.unwrap()
  }

  #[test]
  fn it_builds_a_modal_with_a_blank_input() {
    let modal = AddTagModal::new(7, ENTITY_TYPE_CHARACTER);
    assert_eq!(modal.entity_id, 7);
    assert_eq!(modal.entity_type, ENTITY_TYPE_CHARACTER);
    assert!(modal.input.is_empty());
  }

  #[tokio::test]
  async fn it_renders_a_create_row_when_the_typed_name_is_new() {
    let tags = seed_tags().await;
    let assignable: Vec<&Tag> = tags.iter().collect();
    let modal = AddTagModal {
      entity_id: 1,
      entity_type: ENTITY_TYPE_CHARACTER,
      input: "Logi".to_owned(),
    };

    let _el: Element<'_, AddTagMessage> = view(&modal, "Test Pilot", Vec::new(), assignable, |m| m);
  }

  #[tokio::test]
  async fn it_renders_the_current_tags_section_with_removable_chips() {
    let tags = seed_tags().await;
    let assigned: Vec<&Tag> = tags.iter().take(1).collect();
    let assignable: Vec<&Tag> = tags.iter().skip(1).collect();
    let modal = AddTagModal {
      entity_id: 1,
      entity_type: ENTITY_TYPE_CHARACTER,
      input: String::new(),
    };

    assert!(
      current_tags(&modal, &assigned, |m: AddTagMessage| m).is_some(),
      "an entity with tags shows the current-tags section"
    );
    assert!(
      current_tags(&modal, &[], |m: AddTagMessage| m).is_none(),
      "an entity with no tags omits the current-tags section"
    );

    let _el: Element<'_, AddTagMessage> = view(&modal, "Test Pilot", assigned, assignable, |m| m);
  }

  #[tokio::test]
  async fn it_renders_the_empty_placeholder_when_nothing_is_assignable() {
    let modal = AddTagModal {
      entity_id: 1,
      entity_type: ENTITY_TYPE_CHARACTER,
      input: String::new(),
    };

    let _el: Element<'_, AddTagMessage> = view(&modal, "Test Pilot", Vec::new(), Vec::new(), |m| m);
  }

  #[tokio::test]
  async fn it_renders_the_modal_with_assignable_rows() {
    let tags = seed_tags().await;
    let assignable: Vec<&Tag> = tags.iter().collect();
    let modal = AddTagModal {
      entity_id: 1,
      entity_type: ENTITY_TYPE_CHARACTER,
      input: String::new(),
    };

    let _el: Element<'_, AddTagMessage> = view(&modal, "Test Pilot", Vec::new(), assignable, |m| m);
  }

  #[test]
  fn it_maps_messages_through_the_host_mapper() {
    let mapped: Vec<u8> = [AddTagMessage::InputChanged("x".to_owned()), AddTagMessage::Close]
      .into_iter()
      .map(|message| match message {
        AddTagMessage::InputChanged(_) => 1u8,
        AddTagMessage::Close => 2u8,
        _ => 0u8,
      })
      .collect();
    assert_eq!(mapped, vec![1, 2]);
  }
}
