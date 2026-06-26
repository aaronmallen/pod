use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, opaque, svg, text, text_input},
};

use super::{Message, State};
use crate::{
  store::{model::Tag, search::AVAILABLE_KEYS},
  ui::{
    components::{eyebrow::eyebrow, rule},
    style::{color, control, radius, spacing, typography},
  },
};

static CLOSE_ICON: &[u8] = include_bytes!("../../../assets/images/icons/close.svg");
static HELP_ICON: &[u8] = include_bytes!("../../../assets/images/icons/help.svg");
static SEARCH_ICON: &[u8] = include_bytes!("../../../assets/images/icons/search.svg");

const CHIPS_PER_ROW: usize = 3;

const CLOSE_ICON_SIZE: f32 = 14.0;

const EXAMPLES: &[(&str, &str)] = &[
  ("tag:pvp", "has tag PvP"),
  ("tag:cruiser,frigate", "cruiser OR frigate"),
  ("tag:pvp tag:caldari", "pvp AND caldari"),
  ("-tag:alt", "NOT tagged alt"),
  ("corp:cobalt", "corp contains \"cobalt\""),
  ("loc:jita", "in Jita"),
  ("status:in-space", "undocked"),
  ("training:idle", "queue empty"),
  ("\"black iris\"", "phrase match"),
];

const HELP_ICON_SIZE: f32 = 15.0;

const INPUT_BOX_HEIGHT: f32 = 36.0;

const POPOVER_TOP_OFFSET: f32 = 120.0;

const POPOVER_WIDTH: f32 = 380.0;

const SEARCH_ICON_SIZE: f32 = 14.0;

const SEARCH_ROW_PAD_X: f32 = 32.0;

pub(super) fn popover(tags: &[Tag]) -> Element<'_, Message> {
  let header = Row::with_children(vec![
    section_label("Query syntax"),
    Space::new().width(Length::Fill).into(),
    icon_button(
      icon(CLOSE_ICON, CLOSE_ICON_SIZE, color::text::secondary()),
      Message::ToggleSearchHelp,
    ),
  ])
  .align_y(Vertical::Center);

  let intro = text(
    "Combine plain text with key:value filters. Comma-separate values for OR. Repeat keys to AND. \
    Prefix with - to negate. Click any example to add it.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(muted_text);

  let examples = Column::with_children(
    EXAMPLES
      .iter()
      .map(|&(query, note)| example_row(query, note))
      .collect::<Vec<_>>(),
  )
  .spacing(spacing::SPACE_2);

  let keys = chip_row(AVAILABLE_KEYS.iter().map(|&key| key_chip(key)).collect());
  let your_tags = chip_row(tags.iter().map(tag_chip).collect());

  let content = Column::with_children(vec![
    header.into(),
    intro.into(),
    examples.into(),
    section_label("Available keys"),
    keys,
    section_label(&format!("Your tags ({})", tags.len())),
    your_tags,
  ])
  .spacing(spacing::SPACE_3);

  let card = container(content)
    .width(Length::Fixed(POPOVER_WIDTH))
    .padding(spacing::SPACE_3_5)
    .style(control::card);

  container(opaque(card))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Top)
    .padding(Padding {
      top: POPOVER_TOP_OFFSET,
      right: SEARCH_ROW_PAD_X,
      bottom: 0.0,
      left: 0.0,
    })
    .into()
}

pub(super) fn search_bar(state: &State) -> Element<'_, Message> {
  let input = text_input("Search… try tag:pvp or status:docked", state.search_query())
    .id(crate::features::shell::focus_search::characters_search_id())
    .on_input(Message::SearchChanged)
    .size(typography::size::MD)
    .padding(0)
    .style(input_style)
    .width(Length::Fill);

  let mut cluster: Vec<Element<'_, Message>> = vec![
    icon(SEARCH_ICON, SEARCH_ICON_SIZE, color::text::secondary()),
    input.into(),
  ];

  if !state.search_query().is_empty() {
    cluster.push(icon_button(
      icon(CLOSE_ICON, CLOSE_ICON_SIZE, color::text::secondary()),
      Message::ClearSearch,
    ));
  }
  cluster.push(rule::vertical_alpha(18.0, 0.12));

  let help_color = if state.search_help_open() {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  cluster.push(icon_button(
    icon(HELP_ICON, HELP_ICON_SIZE, help_color),
    Message::ToggleSearchHelp,
  ));

  let input_box = container(
    Row::with_children(cluster)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .height(Length::Fixed(INPUT_BOX_HEIGHT))
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    right: 4.0,
    bottom: 0.0,
    left: spacing::SPACE_3,
  })
  .style(input_box_style);

  container(input_box)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: SEARCH_ROW_PAD_X,
      bottom: spacing::SPACE_3_5,
      left: SEARCH_ROW_PAD_X,
    })
    .style(row_style)
    .into()
}

fn row_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  }
}

fn chip_row<'a>(chips: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  let mut current: Vec<Element<'a, Message>> = Vec::new();
  for chip in chips {
    current.push(chip);
    if current.len() == CHIPS_PER_ROW {
      rows.push(
        Row::with_children(std::mem::take(&mut current))
          .spacing(spacing::SPACE_2)
          .into(),
      );
    }
  }
  if !current.is_empty() {
    rows.push(Row::with_children(current).spacing(spacing::SPACE_2).into());
  }

  Column::with_children(rows).spacing(spacing::SPACE_2).into()
}

fn code_chip<'a>(label: &'a str) -> Element<'a, Message> {
  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(chip_padding())
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.10))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.25),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn chip_padding() -> Padding {
  Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
  }
}

fn example_row<'a>(query: &'a str, note: &'a str) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      code_chip(query),
      text(note)
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(muted_text)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .padding(spacing::SPACE_2)
  .width(Length::Fill)
  .on_press(Message::InsertQuery(query.to_owned()))
  .style(example_button_style)
  .into()
}

fn example_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05)))
    }
    _ => None,
  };

  button::Style {
    background,
    text_color: color::text::secondary(),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn icon<'a>(bytes: &'static [u8], size: f32, tint: Color) -> Element<'a, Message> {
  svg(svg::Handle::from_memory(bytes))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .style(move |_, _| svg::Style {
      color: Some(tint),
    })
    .into()
}

fn icon_button<'a>(content: Element<'a, Message>, message: Message) -> Element<'a, Message> {
  button(content)
    .padding(spacing::SPACE_2)
    .on_press(message)
    .style(icon_button_style)
    .into()
}

fn icon_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
    }
    _ => None,
  };

  button::Style {
    background,
    text_color: color::text::secondary(),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn input_box_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  }
}

fn input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

fn key_chip<'a>(key: &str) -> Element<'a, Message> {
  container(
    text(format!("{key}:"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(muted_text),
  )
  .padding(chip_padding())
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn muted_text(_theme: &iced::Theme) -> text::Style {
  text::Style {
    color: Some(color::text::secondary()),
  }
}

fn section_label<'a>(label: &str) -> Element<'a, Message> {
  eyebrow(label, Some(color::text::tertiary()))
}

fn tag_chip<'a>(tag: &Tag) -> Element<'a, Message> {
  let name = tag.name().to_lowercase();
  let fragment = if name.contains(' ') {
    format!("tag:\"{name}\"")
  } else {
    format!("tag:{name}")
  };

  button(
    text(format!("tag:{name}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM),
  )
  .padding(chip_padding())
  .on_press(Message::InsertQuery(fragment))
  .style(tag_chip_style)
  .into()
}

fn tag_chip_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let (text_color, border_color) = match status {
    button::Status::Hovered | button::Status::Pressed => {
      (color::accent::PLASMA, color::with_alpha(color::accent::PLASMA, 0.4))
    }
    _ => (color::text::secondary(), color::with_alpha(color::text::PRIMARY, 0.12)),
  };

  button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color,
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..button::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod popover {
    use super::*;

    #[test]
    fn it_renders_the_help_popover() {
      let _el: Element<'_, Message> = popover(&[]);
    }
  }

  mod search_bar {
    use super::*;

    #[test]
    fn it_renders_the_clear_control_and_active_help_toggle_with_a_query() {
      let mut state = State::new();
      state.search_query = "corp:cobalt".to_owned();
      state.search_help_open = true;

      let _el: Element<'_, Message> = search_bar(&state);
    }

    #[test]
    fn it_renders_without_a_clear_control_when_the_query_is_empty() {
      let state = State::new();

      let _el: Element<'_, Message> = search_bar(&state);
    }
  }
}
