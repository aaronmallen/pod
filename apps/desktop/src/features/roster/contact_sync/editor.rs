use iced::{
  Background, Border, Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{
  ESTIMATED_ROW_HEIGHT, ListModel, Message, Pilot, State, VIEWPORT_SLACK,
  character_detail::{
    LoadState,
    tabs::{contacts, shared},
  },
  pilot_avatar,
};
use crate::ui::{
  components::{icon::Icon, text_input::TextInput},
  style::{color, radius, spacing, typography},
};

const CHECKBOX_SIZE: f32 = 20.0;
const GRID_COLUMNS: usize = 3;
const NAME_FONT_SIZE: f32 = 18.0;
const SECTION_GAP: f32 = 30.0;
const STEP_BADGE_SIZE: f32 = 20.0;
const TARGET_AVATAR: f32 = 34.0;

pub(super) fn screen<'a>(state: &'a State, list: &'a ListModel) -> Element<'a, Message> {
  Column::with_children(vec![
    name_field(list),
    contacts_section(state, list),
    targets_section(state, list),
  ])
  .spacing(SECTION_GAP)
  .width(Length::Fill)
  .into()
}

fn contacts_section<'a>(state: &'a State, list: &'a ListModel) -> Element<'a, Message> {
  let in_list = t!("contact_sync.in_list", count => list.contacts.len()).into_owned();
  let row_count = match &state.contacts {
    LoadState::Loaded(page) => page.rows().len(),
    _ => 0,
  };

  let header = contacts::header(&state.contacts, state.contact_filter, &state.contacts_query, true)
    .map(|message| Message::Contacts(Box::new(message)));
  // The table isn't independently scrollable here, so size the "viewport" to fit every row
  // (rather than the actual visible height) to keep the virtual list from windowing rows out.
  let table = contacts::body(
    &state.contacts,
    state.contact_sort,
    true,
    row_count as f32 * ESTIMATED_ROW_HEIGHT + VIEWPORT_SLACK,
    0.0,
    contacts::ContactColumns::standings_only(),
  )
  .map(|message| Message::Contacts(Box::new(message)));

  Column::with_children(vec![
    step_eyebrow(1, t!("contact_sync.step_contacts").into_owned(), in_list),
    header,
    table,
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill)
  .into()
}

fn name_field(list: &ListModel) -> Element<'_, Message> {
  let label = text(t!("contact_sync.list_name").to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let placeholder = shared::static_text(t!("contact_sync.name_placeholder"));
  let input = TextInput::new(placeholder, &list.name, Message::NameChanged)
    .font_size(NAME_FONT_SIZE)
    .width(Length::Fill);

  Column::with_children(vec![label.into(), input.render()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn step_eyebrow<'a>(step: usize, label: String, right: String) -> Element<'a, Message> {
  let badge = container(
    text(step.to_string())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::accent()),
      }),
  )
  .width(Length::Fixed(STEP_BADGE_SIZE))
  .height(Length::Fixed(STEP_BADGE_SIZE))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent(), 0.14))),
    border: Border {
      color: color::with_alpha(color::accent(), 0.4),
      radius: 999.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  let heading = text(label.to_uppercase())
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let rule = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    });
  let meta = text(right.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  Row::with_children(vec![badge.into(), heading.into(), rule.into(), meta.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn target_card<'a>(pilot: &'a Pilot, selected: bool) -> Element<'a, Message> {
  let avatar = pilot_avatar(pilot, TARGET_AVATAR, selected.then_some(color::accent()));

  let identity = Column::with_children(vec![
    text(pilot.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD + 1.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(pilot.subtitle.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let mark: Element<'a, Message> = if selected {
    Icon::check().size(12.0).color(color::surface::BASE).render()
  } else {
    Space::new().into()
  };
  let checkbox = container(mark)
    .width(Length::Fixed(CHECKBOX_SIZE))
    .height(Length::Fixed(CHECKBOX_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: selected.then_some(Background::Color(color::accent())),
      border: Border {
        color: if selected {
          color::accent()
        } else {
          color::rule_strong()
        },
        radius: (radius::CONTROL - 3.0).into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let row = Row::with_children(vec![avatar, identity.into(), checkbox.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  button(container(row).width(Length::Fill).padding(iced::Padding {
    top: spacing::SPACE_2_5 + 1.0,
    right: spacing::SPACE_3 + 1.0,
    bottom: spacing::SPACE_2_5 + 1.0,
    left: spacing::SPACE_3 + 1.0,
  }))
  .padding(0)
  .width(Length::Fill)
  .on_press(Message::TargetToggled(pilot.character_id))
  .style(move |_, _| button::Style {
    background: Some(Background::Color(if selected {
      color::with_alpha(color::accent(), 0.07)
    } else {
      color::surface::RAISED
    })),
    border: Border {
      color: if selected {
        color::with_alpha(color::accent(), 0.45)
      } else {
        color::rule()
      },
      radius: radius::NAV_CARD.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn target_grid<'a>(state: &'a State, list: &'a ListModel) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  for chunk in state.pilots.chunks(GRID_COLUMNS) {
    let mut cells: Vec<Element<'a, Message>> = chunk
      .iter()
      .map(|pilot| target_card(pilot, list.target_ids.contains(&pilot.character_id)))
      .collect();
    for _ in chunk.len()..GRID_COLUMNS {
      cells.push(Space::new().width(Length::Fill).into());
    }
    rows.push(
      Row::with_children(cells)
        .spacing(spacing::SPACE_2_5)
        .width(Length::Fill)
        .into(),
    );
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn targets_section<'a>(state: &'a State, list: &'a ListModel) -> Element<'a, Message> {
  let ratio = format!("{} / {}", list.target_ids.len(), state.pilots.len());
  let hint = text(t!("contact_sync.targets_hint"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM + 1.5)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  Column::with_children(vec![
    step_eyebrow(2, t!("contact_sync.step_targets").into_owned(), ratio),
    hint.into(),
    target_grid(state, list),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fill)
  .into()
}
