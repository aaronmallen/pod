use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text},
};

use super::{Draft, Message, PilotRef, ui};
use crate::ui::{
  components::{button::Button, color_picker::PALETTE, eyebrow::eyebrow, rule, text_input::TextInput},
  style::{color, radius, spacing, typography},
};

const MODAL_WIDTH: f32 = 520.0;
const HEADER_ICON_TILE: f32 = 34.0;
const PILOT_CHIP_SIZE: f32 = 20.0;
const SWATCH_SIZE: f32 = 30.0;

pub(super) fn view<'a>(draft: &'a Draft, roster: &'a [PilotRef]) -> Element<'a, Message> {
  let body = Column::with_children(vec![
    header(draft),
    rule::horizontal(),
    fields(draft, roster),
    rule::horizontal(),
    footer(draft),
  ])
  .width(Length::Fixed(MODAL_WIDTH));

  container(body)
    .width(Length::Fixed(MODAL_WIDTH))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn header<'a>(draft: &Draft) -> Element<'a, Message> {
  let accent = ui::accent_color(&draft.accent);
  let kicker = if draft.editing.is_some() {
    t!("standing_orders.modal.edit")
  } else {
    t!("standing_orders.modal.new")
  };
  let preview = if draft.title.trim().is_empty() {
    t!("standing_orders.modal.untitled").into_owned()
  } else {
    draft.title.trim().to_owned()
  };

  let heading = Column::with_children(vec![
    eyebrow(&kicker, Some(ui::identity())),
    text(preview)
      .font(typography::body::MEDIUM)
      .size(16.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  container(
    Row::with_children(vec![ui::target_tile(accent, HEADER_ICON_TILE, 19.0), heading.into()])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 17.0,
    right: 20.0,
    bottom: 17.0,
    left: 20.0,
  })
  .into()
}

fn fields<'a>(draft: &'a Draft, roster: &'a [PilotRef]) -> Element<'a, Message> {
  let title_field = labelled(
    &t!("standing_orders.modal.field.title"),
    TextInput::new(
      intern("standing_orders.modal.placeholder.title"),
      &draft.title,
      Message::TitleChanged,
    )
    .background(color::surface::SUNKEN)
    .font_size(14.0)
    .on_submit(Message::ModalSubmitted)
    .render(),
  );

  let why_field = labelled(
    &t!("standing_orders.modal.field.why"),
    TextInput::new(
      intern("standing_orders.modal.placeholder.why"),
      &draft.why,
      Message::WhyChanged,
    )
    .background(color::surface::SUNKEN)
    .font_size(14.0)
    .render(),
  );

  let target_field = labelled(
    &t!("standing_orders.modal.field.target"),
    TextInput::new(
      intern("standing_orders.modal.placeholder.target"),
      &draft.target,
      Message::TargetChanged,
    )
    .background(color::surface::SUNKEN)
    .font_size(14.0)
    .on_submit(Message::ModalSubmitted)
    .render(),
  );

  let horizon_field = labelled(
    &t!("standing_orders.modal.field.horizon"),
    TextInput::new(
      intern("standing_orders.modal.placeholder.horizon"),
      &draft.horizon,
      Message::HorizonChanged,
    )
    .background(color::surface::SUNKEN)
    .font_size(14.0)
    .on_submit(Message::ModalSubmitted)
    .render(),
  );

  let split = Row::with_children(vec![target_field, horizon_field])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  Column::with_children(vec![
    title_field,
    why_field,
    split.into(),
    labelled(&t!("standing_orders.modal.field.pilots"), pilots(draft, roster)),
    labelled(&t!("standing_orders.modal.field.accent"), swatches(draft)),
  ])
  .spacing(spacing::SPACE_4_5)
  .width(Length::Fill)
  .padding(Padding {
    top: 18.0,
    right: 20.0,
    bottom: 18.0,
    left: 20.0,
  })
  .into()
}

fn labelled<'a>(label: &str, control: Element<'a, Message>) -> Element<'a, Message> {
  Column::with_children(vec![eyebrow(label, None), control])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn pilots<'a>(draft: &'a Draft, roster: &'a [PilotRef]) -> Element<'a, Message> {
  let chips: Vec<Element<'a, Message>> = roster
    .iter()
    .map(|pilot| pilot_toggle(pilot, draft.pilots.contains(&pilot.id)))
    .collect();

  wrap(chips)
}

fn pilot_toggle<'a>(pilot: &PilotRef, selected: bool) -> Element<'a, Message> {
  let face = ui::pilot_face::<Message>(std::slice::from_ref(pilot), pilot.id, PILOT_CHIP_SIZE);
  let content = Row::with_children(vec![
    face,
    text(pilot.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .into(),
  ])
  .spacing(spacing::UNIT + 3.0)
  .align_y(Vertical::Center);

  button(content)
    .padding([5.0, 11.0])
    .on_press(Message::PilotToggled(pilot.id))
    .style(move |_, _| pilot_style(selected))
    .into()
}

fn pilot_style(selected: bool) -> button::Style {
  let identity = ui::identity();
  button::Style {
    background: selected.then(|| Background::Color(color::with_alpha(identity, 0.14))),
    border: Border {
      color: if selected {
        color::with_alpha(identity, 0.45)
      } else {
        color::rule()
      },
      width: 1.0,
      radius: 999.0.into(),
    },
    text_color: if selected {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    },
    ..button::Style::default()
  }
}

fn swatches<'a>(draft: &Draft) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = PALETTE.iter().map(|preset| swatch(preset.hex, &draft.accent)).collect();
  wrap(cells)
}

fn swatch<'a>(hex: &str, current: &str) -> Element<'a, Message> {
  let fill = color::from_hex(hex).unwrap_or_else(ui::identity);
  let selected = current.eq_ignore_ascii_case(hex);
  let hex_owned = hex.to_owned();

  button(
    Space::new()
      .width(Length::Fixed(SWATCH_SIZE))
      .height(Length::Fixed(SWATCH_SIZE)),
  )
  .padding(Padding::ZERO)
  .on_press(Message::AccentSelected(hex_owned))
  .style(move |_, _| swatch_style(fill, selected))
  .into()
}

fn swatch_style(fill: Color, selected: bool) -> button::Style {
  button::Style {
    background: Some(Background::Color(fill)),
    border: Border {
      color: if selected {
        color::text::PRIMARY
      } else {
        color::with_alpha(fill, 0.5)
      },
      width: if selected { 2.0 } else { 1.0 },
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  }
}

fn footer<'a>(draft: &Draft) -> Element<'a, Message> {
  let can_save = !draft.title.trim().is_empty();
  let submit_label = if draft.editing.is_some() {
    t!("standing_orders.action.save")
  } else {
    t!("standing_orders.action.create")
  };

  Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    Button::ghost(t!("standing_orders.action.cancel"))
      .on_press(Message::ModalCancelled)
      .into(),
    Button::primary(submit_label)
      .on_press_maybe(can_save.then_some(Message::ModalSubmitted))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 13.0,
    right: 18.0,
    bottom: 13.0,
    left: 18.0,
  })
  .into()
}

fn intern(key: &str) -> &'static str {
  use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
  };
  static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));
  if let Some(&value) = cache
    .read()
    .expect("standing orders modal i18n cache poisoned")
    .get(key)
  {
    return value;
  }

  let resolved: &'static str = Box::leak(t!(key).into_owned().into_boxed_str());
  cache
    .write()
    .expect("standing orders modal i18n cache poisoned")
    .entry(key.to_owned())
    .or_insert(resolved)
}

fn wrap<'a>(children: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  Column::with_children(
    children
      .into_iter()
      .fold(Vec::<Vec<Element<'a, Message>>>::new(), |mut acc, child| {
        match acc.last_mut() {
          Some(row) if row.len() < WRAP_COLUMNS => row.push(child),
          _ => acc.push(vec![child]),
        }
        acc
      })
      .into_iter()
      .map(|row| Row::with_children(row).spacing(spacing::UNIT + 3.0).into())
      .collect::<Vec<_>>(),
  )
  .spacing(spacing::UNIT + 3.0)
  .width(Length::Fill)
  .into()
}

const WRAP_COLUMNS: usize = 3;
