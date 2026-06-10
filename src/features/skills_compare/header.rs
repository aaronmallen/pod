use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text},
};

use super::{HEADER_HEIGHT, Message, State};
use crate::{
  features::skills::fmt_sp,
  ui::{
    components::{
      avatar::Avatar,
      icon::Icon,
      picker::{PickerGroup, picker_character_row, picker_dropdown},
      text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

const CHIP_PORTRAIT: f32 = 28.0;
const REMOVE_GLYPH: &str = "\u{00d7}";

pub(super) fn header(state: &State) -> Element<'_, Message> {
  let title = Column::with_children(vec![
    text("Compare")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text(format!("{} pilots", state.pilot_count()))
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT - 2.0);

  let chips = Row::with_children(
    state
      .selected_ids()
      .iter()
      .map(|id| chip(state, *id))
      .collect::<Vec<_>>(),
  )
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let bar = Row::with_children(vec![
    container(title).width(Length::Shrink).into(),
    divider(),
    chips.into(),
    add_pilot(state),
  ])
  .spacing(spacing::SPACE_6)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(bar)
    .width(Length::Fill)
    .height(Length::Fixed(HEADER_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_6,
      bottom: 0.0,
      left: spacing::SPACE_6,
    })
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn add_pilot(state: &State) -> Element<'_, Message> {
  let trigger = button(
    Row::with_children(vec![
      text("+")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      text("ADD PILOT")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  })
  .on_press(Message::PickerToggled)
  .style(move |_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let active = state.picker_open() || hover;
    button::Style {
      background: state
        .picker_open()
        .then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if active { 0.25 } else { 0.12 }),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      text_color: color::text::SECONDARY,
      ..button::Style::default()
    }
  });

  container(trigger).width(Length::Shrink).into()
}

fn chip(state: &State, pilot_id: i64) -> Element<'_, Message> {
  let accent = state.pilot_accent(pilot_id);
  let name = state.pilot_name(pilot_id).to_owned();
  let total_sp = state.model(pilot_id).map(|model| model.total_sp).unwrap_or(0);
  let portrait = state.portrait(pilot_id).path();

  let identity = Column::with_children(vec![
    text(name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("{} SP", fmt_sp(total_sp as i64)))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT - 2.0);

  let chip_body = Row::with_children(vec![
    Avatar::new(pilot_id, name, Length::Fixed(CHIP_PORTRAIT), CHIP_PORTRAIT, portrait)
      .radius(radius::SUBTLE)
      .view(),
    identity.into(),
    remove_button(state, pilot_id),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(chip_body)
    .padding(Padding {
      top: 5.0,
      right: 6.0,
      bottom: 5.0,
      left: 5.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      border: Border {
        color: color::with_alpha(accent, 0.6),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fixed(44.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    })
    .into()
}

pub(super) fn dropdown(state: &State) -> Element<'_, Message> {
  let search = TextInput::new(
    "Search pilots\u{2026}",
    state.picker_query(),
    Message::PickerQueryChanged,
  )
  .leading_icon(Icon::search())
  .background(color::surface::SUNKEN)
  .render();

  let mut rows: Vec<Element<'_, Message>> =
    vec![container(search).width(Length::Fill).padding(spacing::SPACE_2_5).into()];

  let available = state.available_pilots();
  if available.is_empty() {
    rows.push(empty_state());
  } else {
    for pilot in available {
      rows.push(pilot_row(state, pilot.id));
    }
  }

  picker_dropdown(vec![PickerGroup {
    title: None,
    items: rows,
  }])
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("No matches")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 20.0,
    right: spacing::SPACE_3_5,
    bottom: 20.0,
    left: spacing::SPACE_3_5,
  })
  .align_x(iced::alignment::Horizontal::Center)
  .into()
}

fn pilot_row(state: &State, pilot_id: i64) -> Element<'_, Message> {
  let name = state.pilot_name(pilot_id).to_owned();

  let trailing: Option<Element<'_, Message>> = state.model(pilot_id).map(|model| {
    text(format!("{} SP", fmt_sp(model.total_sp as i64)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into()
  });

  picker_character_row(
    pilot_id,
    name,
    String::new(),
    state.portrait(pilot_id).path(),
    trailing,
    false,
    Message::PilotAdded(pilot_id),
  )
}

fn remove_button(state: &State, pilot_id: i64) -> Element<'_, Message> {
  let enabled = state.can_remove();

  let glyph = text(REMOVE_GLYPH)
    .font(typography::body::REGULAR)
    .size(typography::size::LG)
    .style(move |_| text::Style {
      color: Some(if enabled {
        color::text::SECONDARY
      } else {
        color::with_alpha(color::text::TERTIARY, 0.4)
      }),
    });

  let mut control = button(
    container(glyph)
      .width(Length::Fixed(20.0))
      .height(Length::Fixed(20.0))
      .align_x(iced::alignment::Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .style(move |_, status| {
    let hover = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then(|| Background::Color(color::with_alpha(color::status::DANGER, 0.14))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      text_color: if hover {
        color::status::DANGER
      } else {
        Color::TRANSPARENT
      },
      ..button::Style::default()
    }
  });

  if enabled {
    control = control.on_press(Message::PilotRemoved(pilot_id));
  }

  control.into()
}
