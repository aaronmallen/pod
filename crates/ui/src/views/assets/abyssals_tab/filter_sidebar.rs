//! Filter sidebar — type picker, stat-range sliders, and reset control.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, mouse_area, row, scrollable, text},
};

use super::{Message, module_type_picker::modal_selected_label, stat_ranges_panel};
use crate::{
  components::icon,
  style::{
    color,
    typography::{body, mono},
  },
  views::assets::State,
};

pub fn section_divider() -> Element<'static, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

pub fn filter_section<'a>(label: &str, content: Element<'a, Message>) -> Element<'a, Message> {
  let label_el = text(label.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });

  column([
    container(column([label_el.into(), Space::new().height(10.0).into(), content]))
      .padding(Padding {
        top: 14.0,
        bottom: 14.0,
        left: 16.0,
        right: 16.0,
      })
      .width(Length::Fill)
      .into(),
    section_divider(),
  ])
  .width(Length::Fill)
  .into()
}

fn sidebar_reset_button(visible: bool) -> Element<'static, Message> {
  if !visible {
    return Space::new().width(0.0).into();
  }
  button(
    text("Reset")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 4.0,
    right: 4.0,
  })
  .on_press(Message::FilterReset)
  .style(|_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: color::text::TERTIARY,
    ..button::Style::default()
  })
  .into()
}

fn module_type_filter_button(has_filter: bool) -> Element<'static, Message> {
  let (bg, border_col, label_col) = if has_filter {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.08))),
      color::with_alpha(color::text::ACCENT, 0.35),
      color::text::PRIMARY,
    )
  } else {
    (None, color::border::SUBTLE, color::text::SECONDARY)
  };
  let icon_col = if has_filter {
    color::text::ACCENT
  } else {
    color::text::TERTIARY
  };
  let label_text = if has_filter {
    "Edit module filter"
  } else {
    "Filter by module type"
  };
  let mut row_items: Vec<Element<'static, Message>> = vec![
    icon::Component::filter().size(14.0).color(icon_col).render::<Message>(),
    Space::new().width(10.0).into(),
    text(label_text)
      .font(body::REGULAR)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(label_col),
      })
      .width(Length::Fill)
      .into(),
  ];
  if has_filter {
    row_items.push(
      container(
        text("1")
          .font(mono::MEDIUM)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::surface::BASE),
          }),
      )
      .padding(Padding {
        top: 2.0,
        bottom: 2.0,
        left: 7.0,
        right: 7.0,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::text::ACCENT)),
        border: Border {
          radius: 999.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    );
  }
  button(row(row_items).align_y(iced::alignment::Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 12.0,
      right: 12.0,
    })
    .on_press(Message::OpenTypeModal)
    .style(move |_, _| button::Style {
      background: bg,
      border: Border {
        color: border_col,
        radius: 6.0.into(),
        width: 1.0,
      },
      text_color: label_col,
      ..button::Style::default()
    })
    .into()
}

fn selected_type_chip(type_name: &str) -> Element<'static, Message> {
  let name_owned = type_name.to_string();
  container(
    row([
      text(name_owned)
        .font(body::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      Space::new().width(4.0).into(),
      button(
        text("\u{00d7}")
          .font(mono::REGULAR)
          .size(12.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .padding(Padding {
        top: 1.0,
        bottom: 1.0,
        left: 2.0,
        right: 2.0,
      })
      .on_press(Message::TypeSelected(None))
      .style(|_, _| button::Style {
        background: None,
        border: Border::default(),
        text_color: color::text::SECONDARY,
        ..button::Style::default()
      })
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 4.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn filter_pane_drag_handle() -> Element<'static, Message> {
  mouse_area(
    container(Space::new().width(4.0).height(Length::Fill))
      .width(4.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      }),
  )
  .on_press(Message::PaneDragStart)
  .interaction(iced::mouse::Interaction::ResizingHorizontally)
  .into()
}

fn filter_sidebar<'a>(state: &'a State) -> Element<'a, Message> {
  let abyssals_state = &state.abyssals;
  let has_filter = abyssals_state.selected_source_type_id.is_some() || !abyssals_state.stat_range_filters.is_empty();

  let header = container(
    row([
      text("FILTERS")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(Length::Fill)
        .into(),
      sidebar_reset_button(has_filter),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 10.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill);

  let header_with_border = column([header.into(), section_divider()]).width(Length::Fill);

  let filter_btn = module_type_filter_button(abyssals_state.selected_source_type_id.is_some());
  let mut module_type_items: Vec<Element<'_, Message>> = vec![filter_btn];
  if let Some(type_id) = abyssals_state.selected_source_type_id {
    let type_name = modal_selected_label(type_id).unwrap_or_else(|| "Unknown".to_string());
    module_type_items.push(Space::new().height(10.0).into());
    module_type_items.push(selected_type_chip(&type_name));
  }
  let module_section = filter_section("Module Type", column(module_type_items).width(Length::Fill).into());

  let stat_el = match abyssals_state.selected_source_type_id {
    Some(src_id) => stat_ranges_panel::Component::new(abyssals_state, src_id).render(),
    None => stat_ranges_panel::placeholder(),
  };

  let body = scrollable(column([module_section, stat_el]).width(Length::Fill)).height(Length::Fill);

  let pane_width = abyssals_state.filter_pane_width.clamp(160.0, 450.0);

  container(column([header_with_border.into(), body.into()]))
    .width(Length::Fixed(pane_width))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

/// Builder for the filter sidebar.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new filter sidebar for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the filter sidebar into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    filter_sidebar(self.state)
  }
}

/// Renders the drag handle for resizing the filter pane.
pub fn drag_handle() -> Element<'static, Message> {
  filter_pane_drag_handle()
}
