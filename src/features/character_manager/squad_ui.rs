use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, button, container, svg, text, text_input},
};

use super::{Message, SquadCreator};
use crate::ui::{
  components::{color_picker, rule},
  style::{color, control, radius, shadow, spacing, typography},
};

static SQUADS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/squads.svg");

const CARD_WIDTH: f32 = 440.0;
const ICON_TILE: f32 = 34.0;
const ICON_GLYPH: f32 = 20.0;
const ICON_RADIUS: f32 = 8.0;
const SECTION_PAD_X: f32 = 20.0;
const SECTION_PAD_Y: f32 = 18.0;
const HEADER_PAD_BOTTOM: f32 = 16.0;
const FIELD_GAP: f32 = 16.0;
const FOOTER_PAD_X: f32 = 16.0;
const FOOTER_PAD_Y: f32 = 12.0;
const INPUT_PAD_X: f32 = 12.0;
const INPUT_PAD_Y: f32 = 10.0;
const UNTITLED: &str = "Untitled squad";

pub(super) fn new_squad_button<'a>() -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      text("+").size(typography::size::MD).into(),
      text("New squad")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(control::padding())
  .on_press(Message::OpenSquadCreator)
  .style(control::ghost_button)
  .into()
}

pub(super) fn modal_view<'a>(creator: &'a SquadCreator) -> Element<'a, Message> {
  let card = container(
    Column::with_children(vec![
      header(creator),
      rule::horizontal(),
      body(creator),
      rule::horizontal(),
      footer(creator),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fixed(CARD_WIDTH))
  .clip(true)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.18),
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
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn header<'a>(creator: &'a SquadCreator) -> Element<'a, Message> {
  let icon_tile = container(
    svg(svg::Handle::from_memory(SQUADS_ICON))
      .width(Length::Fixed(ICON_GLYPH))
      .height(Length::Fixed(ICON_GLYPH))
      .style(|_, _| svg::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(Length::Fixed(ICON_TILE))
  .height(Length::Fixed(ICON_TILE))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.14))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.4),
      width: 1.0,
      radius: ICON_RADIUS.into(),
    },
    ..container::Style::default()
  });

  let trimmed = creator.name.trim();
  let title = if trimmed.is_empty() { UNTITLED } else { trimmed };
  let eyebrow = if creator.editing.is_some() {
    "Edit squad"
  } else {
    "New squad"
  };

  let titles = Column::with_children(vec![
    text(eyebrow)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    text(title.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let row = Row::with_children(vec![icon_tile.into(), titles.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: SECTION_PAD_Y,
      right: SECTION_PAD_X,
      bottom: HEADER_PAD_BOTTOM,
      left: SECTION_PAD_X,
    })
    .into()
}

fn body<'a>(creator: &'a SquadCreator) -> Element<'a, Message> {
  let fields = Column::with_children(vec![
    field(
      "Name",
      "e.g. Home Defense Fleet",
      &creator.name,
      Message::SquadCreatorNameChanged,
    ),
    field(
      "Description",
      "What's this squad for?",
      &creator.description,
      Message::SquadCreatorDescriptionChanged,
    ),
    color_field(creator),
  ])
  .spacing(FIELD_GAP)
  .width(Length::Fill);

  container(fields)
    .width(Length::Fill)
    .padding(Padding {
      top: SECTION_PAD_Y,
      right: SECTION_PAD_X,
      bottom: SECTION_PAD_Y,
      left: SECTION_PAD_X,
    })
    .into()
}

fn field<'a>(
  label: &'a str,
  placeholder: &'a str,
  value: &'a str,
  on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
  let input = text_input(placeholder, value)
    .size(typography::size::MD)
    .padding(Padding {
      top: INPUT_PAD_Y,
      right: INPUT_PAD_X,
      bottom: INPUT_PAD_Y,
      left: INPUT_PAD_X,
    })
    .on_input(on_input)
    .on_submit(Message::CreateSquad)
    .style(field_input_style);

  Column::with_children(vec![
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    input.into(),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill)
  .into()
}

fn color_field<'a>(creator: &'a SquadCreator) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    text("Color")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    color_picker::color_swatch(Some(&creator.color), Message::SquadColorPickerToggled),
  ];

  if creator.color_popover_open {
    children.push(color_picker::color_popover(
      Some(&creator.color),
      &creator.hex_draft,
      creator.hex_invalid,
      Message::SquadColorSelected,
      Message::SquadColorHexChanged,
      Message::SquadColorHexSubmitted,
    ));
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn footer<'a>(creator: &'a SquadCreator) -> Element<'a, Message> {
  let can_save = !creator.name.trim().is_empty();

  let cancel = button(text("Cancel").font(typography::body::MEDIUM).size(typography::size::SM))
    .padding(control::padding())
    .on_press(Message::CloseSquadCreator)
    .style(control::ghost_button);

  let primary_label = if creator.editing.is_some() {
    "Save changes"
  } else {
    "Create squad"
  };
  let create = button(
    text(primary_label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM),
  )
  .padding(control::padding())
  .on_press_maybe(can_save.then_some(Message::CreateSquad))
  .style(control::primary_button);

  let row = Row::with_children(vec![cancel.into(), create.into()])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  container(container(row).width(Length::Fill).align_x(Horizontal::Right))
    .width(Length::Fill)
    .padding(Padding {
      top: FOOTER_PAD_Y,
      right: FOOTER_PAD_X,
      bottom: FOOTER_PAD_Y,
      left: FOOTER_PAD_X,
    })
    .into()
}

fn field_input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
  let border_alpha = match status {
    text_input::Status::Focused {
      ..
    } => 0.18,
    _ => 0.1,
  };
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, border_alpha),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    icon: color::text::SECONDARY,
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}
