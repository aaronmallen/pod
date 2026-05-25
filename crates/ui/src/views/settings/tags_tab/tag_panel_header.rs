//! Header section of the tags settings panel: create-tag input,
//! sort-mode control, filter input, and stats row.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  border::Radius,
  widget::{Space, button, column, container, row, text, text_input},
};

use super::{Message, State, TagSortMode};
use crate::style::{color, radius, spacing, typography};

/// Builder for the tag-panel header element.
pub struct TagPanelHeader<'a> {
  state: &'a State,
}

impl<'a> TagPanelHeader<'a> {
  /// Create a new header builder bound to the given panel state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    tag_panel_header(self.state)
  }
}

fn sort_mode_button(label: &'static str, is_active: bool, msg: Message) -> Element<'static, Message> {
  let text_color = if is_active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  let bg = if is_active {
    Some(Background::Color(color::accent::PLASMA_SUBTLE))
  } else {
    None
  };
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(msg)
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      radius: Radius::from(4.0),
      ..Border::default()
    },
    snap: false,
    text_color,
    shadow: iced::Shadow::default(),
  })
  .into()
}

fn create_input_pill<'a>(state: &'a State) -> Element<'a, Message> {
  let plus = text("+")
    .size(14.0)
    .font(typography::body::MEDIUM)
    .style(|_| iced::widget::text::Style {
      color: Some(color::accent::PLASMA),
    });

  let input = text_input("Create a tag\u{2026}", &state.new_name)
    .on_input(Message::NewNameChanged)
    .on_submit(Message::Create)
    .font(typography::body::REGULAR)
    .size(13.0)
    .style(|_, _| text_input::Style {
      background: Background::Color(iced::Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::state::SELECTION,
    })
    .padding(Padding::ZERO);

  container(
    row([plus.into(), input.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .max_width(360.0)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn add_button_style(can_create: bool) -> button::Style {
  let (bg, border_color, text_color) = if can_create {
    (
      Some(Background::Color(color::accent::PLASMA)),
      color::accent::PLASMA,
      color::surface::SUNKEN,
    )
  } else {
    (
      Some(Background::Color(color::state::HOVER_OVERLAY)),
      color::border::SUBTLE,
      color::text::TERTIARY,
    )
  };
  button::Style {
    background: bg,
    border: Border {
      color: border_color,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    snap: false,
    text_color,
    shadow: iced::Shadow::default(),
  }
}

fn add_button(can_create: bool) -> Element<'static, Message> {
  let label_color = if can_create {
    Some(color::surface::SUNKEN)
  } else {
    Some(color::text::TERTIARY)
  };
  let b = button(
    text("Add")
      .font(typography::body::MEDIUM)
      .size(13.0)
      .style(move |_| iced::widget::text::Style {
        color: label_color,
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  })
  .style(move |_, _| add_button_style(can_create));
  if can_create { b.on_press(Message::Create) } else { b }.into()
}

fn sort_control_row(state: &State) -> Element<'_, Message> {
  container(row([
    sort_mode_button(
      "Manual",
      state.sort_mode == TagSortMode::Manual,
      Message::SortModeChanged(TagSortMode::Manual),
    ),
    sort_mode_button(
      "A–Z",
      state.sort_mode == TagSortMode::Name,
      Message::SortModeChanged(TagSortMode::Name),
    ),
    sort_mode_button(
      "Color",
      state.sort_mode == TagSortMode::Color,
      Message::SortModeChanged(TagSortMode::Color),
    ),
  ]))
  .padding(2.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn filter_input<'a>(state: &'a State) -> Element<'a, Message> {
  let search_icon = crate::components::Icon::search()
    .size(14.0)
    .color(color::text::SECONDARY)
    .render::<Message>();

  container(
    row([
      search_icon,
      text_input("Filter\u{2026}", &state.search)
        .on_input(Message::SearchChanged)
        .size(13.0)
        .style(|_, _| text_input::Style {
          background: Background::Color(iced::Color::TRANSPARENT),
          border: Border::default(),
          icon: color::text::SECONDARY,
          placeholder: color::text::TERTIARY,
          selection: color::accent::PLASMA_SUBTLE,
          value: color::text::PRIMARY,
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .max_width(200.0)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn dot_separator<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(3.0)
    .height(3.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::text::GHOST)),
      border: Border {
        radius: radius::FULL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn stats_row<'a>(state: &'a State) -> Element<'a, Message> {
  let colored = state.colored_count();
  let draggable = state.sort_mode == TagSortMode::Manual && state.search.trim().is_empty();

  let mut parts: Vec<Element<'a, Message>> = vec![
    text(format!("{}", state.tags.len()))
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text(" tags")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::GHOST),
      })
      .into(),
    Space::new().width(10.0).into(),
    dot_separator(),
    Space::new().width(10.0).into(),
    text(format!("{colored}"))
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    text(" colored")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::GHOST),
      })
      .into(),
  ];

  if !draggable {
    let warning = reorder_warning_text(state);
    parts.extend([
      Space::new().width(10.0).into(),
      dot_separator(),
      Space::new().width(10.0).into(),
      text(warning)
        .font(typography::mono::REGULAR)
        .size(10.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::WARNING),
        })
        .into(),
    ]);
  }

  row(parts).align_y(Vertical::Center).into()
}

fn reorder_warning_text(state: &State) -> &'static str {
  if state.search.trim().is_empty() {
    "Reorder disabled in sorted view"
  } else {
    "Reorder disabled while filtering"
  }
}

fn tag_panel_header(state: &State) -> Element<'_, Message> {
  let title = text("Tags").size(18.0).color(color::text::PRIMARY);
  let desc = text(
    "Assign a color to any tag and it will render that way everywhere it appears \
    on a character card. Drag rows to reorder; tags follow their manual order on cards.",
  )
  .size(13.0)
  .color(color::text::SECONDARY);

  let can_create = !state.new_name.trim().is_empty();

  let create_row: Element<'_, Message> = row([
    create_input_pill(state),
    add_button(can_create),
    Space::new().width(Length::Fill).into(),
    sort_control_row(state),
    filter_input(state),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into();

  column([
    row([title.into(), Space::new().width(Length::Fill).into()])
      .align_y(Vertical::Center)
      .into(),
    Space::new().height(4.0).into(),
    desc.into(),
    Space::new().height(spacing::SPACE_3_5).into(),
    create_row,
    Space::new().height(8.0).into(),
    stats_row(state),
  ])
  .padding(Padding {
    top: 24.0,
    bottom: spacing::SPACE_3_5,
    left: 36.0,
    right: 36.0,
  })
  .into()
}
