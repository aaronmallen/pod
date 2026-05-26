//! Module-type picker modal for the abyssals tab.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, scrollable, text},
};

use super::{MODAL_LAYOUT, MODAL_SOURCE_PATTERNS, Message, filter_sidebar::section_divider};
use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::assets::State,
};

/// A single selectable entry within a family row.
pub struct ModalEntry {
  /// Display label for this variant.
  pub label: &'static str,
  /// EVE type ID for this variant.
  pub type_id: i32,
}

/// A row within a modal section, either a single item or a named family.
pub enum ModalRow {
  /// A group of related size variants under one name.
  Family {
    /// Family name displayed as a heading.
    name: &'static str,
    /// Size variants belonging to this family.
    variants: &'static [ModalEntry],
  },
  /// A standalone module type with a single selectable row.
  Single {
    /// Display label for the module.
    label: &'static str,
    /// EVE type ID for this module.
    type_id: i32,
  },
}

/// A titled group of rows within one column of the picker modal.
pub struct ModalSection {
  /// Rows contained within this section.
  pub rows: &'static [ModalRow],
  /// Section heading text.
  pub title: &'static str,
}

/// Builder for the module-type picker modal overlay.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new module-type picker component for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the modal overlay into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let selected_id = state.abyssals.selected_source_type_id;

    let build_col = |sections: &'static [ModalSection]| -> Vec<Element<'static, Message>> {
      sections
        .iter()
        .map(|s| {
          container(modal_section_el(s, selected_id))
            .padding(Padding {
              bottom: 24.0,
              ..Padding::ZERO
            })
            .into()
        })
        .collect()
    };

    let col0 = build_col(MODAL_LAYOUT[0]);
    let col1 = build_col(MODAL_LAYOUT[1]);
    let col2 = build_col(MODAL_LAYOUT[2]);

    let subtitle = selected_id
      .and_then(modal_selected_label)
      .unwrap_or_else(|| "Pick a module type".to_string());

    let mut header_row_items: Vec<Element<'_, Message>> = vec![
      column([
        text("Filter by module type")
          .font(body::MEDIUM)
          .size(14.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        Space::new().height(2.0).into(),
        text(subtitle)
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .width(Length::Fill)
      .into(),
    ];

    if selected_id.is_some() {
      header_row_items.push(
        button(
          text("Clear")
            .font(body::REGULAR)
            .size(11.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::SECONDARY),
            }),
        )
        .padding(Padding {
          top: 5.0,
          bottom: 5.0,
          left: 10.0,
          right: 10.0,
        })
        .on_press(Message::TypeSelected(None))
        .style(|_, _| button::Style {
          background: None,
          border: Border {
            color: color::border::SUBTLE,
            radius: 5.0.into(),
            width: 1.0,
          },
          text_color: color::text::SECONDARY,
          ..button::Style::default()
        })
        .into(),
      );
      header_row_items.push(Space::new().width(8.0).into());
    }

    header_row_items.push(
      button(
        text("\u{00d7}")
          .font(mono::REGULAR)
          .size(18.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .width(28.0)
      .height(28.0)
      .on_press(Message::CloseTypeModal)
      .style(|_, _| button::Style {
        background: None,
        border: Border::default(),
        text_color: color::text::SECONDARY,
        ..button::Style::default()
      })
      .into(),
    );

    let panel_header = container(row(header_row_items).align_y(iced::alignment::Vertical::Center))
      .padding(Padding {
        top: 16.0,
        bottom: 16.0,
        left: 24.0,
        right: 24.0,
      })
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        ..container::Style::default()
      });

    let panel_body = scrollable(
      container(row([
        column(col0).width(Length::Fill).into(),
        Space::new().width(44.0).into(),
        column(col1).width(Length::Fill).into(),
        Space::new().width(44.0).into(),
        column(col2).width(Length::Fill).into(),
      ]))
      .padding(Padding {
        top: 24.0,
        bottom: 28.0,
        left: 32.0,
        right: 32.0,
      })
      .width(Length::Fill),
    )
    .height(Length::Fill);

    let panel_footer = container(
      row([
        text("esc \u{00b7} close")
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          })
          .width(Length::Fill)
          .into(),
        button(
          text("Done")
            .font(body::MEDIUM)
            .size(12.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::surface::BASE),
            }),
        )
        .padding(Padding {
          top: 8.0,
          bottom: 8.0,
          left: 18.0,
          right: 18.0,
        })
        .on_press(Message::CloseTypeModal)
        .style(|_, _| button::Style {
          background: Some(Background::Color(color::text::ACCENT)),
          border: Border {
            radius: 6.0.into(),
            ..Border::default()
          },
          text_color: color::surface::BASE,
          ..button::Style::default()
        })
        .into(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: 24.0,
      right: 24.0,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

    let panel = container(column([
      panel_header.into(),
      section_divider(),
      panel_body.into(),
      section_divider(),
      panel_footer.into(),
    ]))
    .max_width(1180.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 12.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

    container(panel)
      .center(Length::Fill)
      .padding(32.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::state::OVERLAY_DARKER)),
        ..container::Style::default()
      })
      .into()
  }
}

/// Returns the display label for the given type ID, or `None` if not found.
pub fn modal_selected_label(type_id: i32) -> Option<String> {
  for col in MODAL_LAYOUT {
    for section in *col {
      for row in section.rows {
        match row {
          ModalRow::Single {
            label,
            type_id: tid,
          } if *tid == type_id => {
            return Some((*label).to_string());
          }
          ModalRow::Family {
            name,
            variants,
            ..
          } => {
            for e in *variants {
              if e.type_id == type_id {
                return Some(format!("{} ({})", name, e.label));
              }
            }
          }
          _ => {}
        }
      }
    }
  }
  None
}

/// Returns the source-pattern string for the given type ID, or `None` if not mapped.
pub fn modal_source_pattern(type_id: i32) -> Option<&'static str> {
  MODAL_SOURCE_PATTERNS
    .iter()
    .find(|&&(tid, _)| tid == type_id)
    .map(|&(_, p)| p)
}

fn modal_type_chip(label: &str, type_id: i32, selected: bool) -> Element<'static, Message> {
  let (bg, border_col, text_col) = if selected {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.14))),
      color::text::ACCENT,
      color::text::ACCENT,
    )
  } else {
    (
      Some(Background::Color(color::surface::BASE)),
      color::border::SUBTLE,
      color::text::SECONDARY,
    )
  };
  let label = label.to_string();
  button(
    text(label)
      .font(body::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_col),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::TypeSelected(Some(type_id)))
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      color: border_col,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: text_col,
    ..button::Style::default()
  })
  .into()
}

fn modal_single_row(label: &'static str, type_id: i32, selected: bool) -> Element<'static, Message> {
  let (bg, text_col, border_col) = if selected {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.10))),
      color::text::ACCENT,
      color::text::ACCENT,
    )
  } else {
    (None, color::text::PRIMARY, Color::TRANSPARENT)
  };
  button(
    text(label)
      .font(body::REGULAR)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_col),
      })
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(Message::TypeSelected(Some(type_id)))
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      color: border_col,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: text_col,
    ..button::Style::default()
  })
  .into()
}

fn modal_family_row(
  name: &'static str,
  variants: &'static [ModalEntry],
  selected_id: Option<i32>,
) -> Element<'static, Message> {
  let some_selected = variants.iter().any(|e| selected_id == Some(e.type_id));
  let (bg, border_col) = if some_selected {
    (
      Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.06))),
      color::with_alpha(color::text::ACCENT, 0.30),
    )
  } else {
    (None, Color::TRANSPARENT)
  };
  let name_el = text(name)
    .font(body::MEDIUM)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });
  let chips: Vec<Element<'static, Message>> = variants
    .iter()
    .map(|e| modal_type_chip(e.label, e.type_id, selected_id == Some(e.type_id)))
    .collect();
  container(column([
    name_el.into(),
    Space::new().height(8.0).into(),
    row(chips).spacing(4.0).wrap().into(),
  ]))
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .style(move |_| container::Style {
    background: bg,
    border: Border {
      color: border_col,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn modal_section_el(section: &'static ModalSection, selected_id: Option<i32>) -> Element<'static, Message> {
  let title = column([
    text(section.title)
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::ACCENT),
      })
      .into(),
    Space::new().height(6.0).into(),
    section_divider(),
    Space::new().height(8.0).into(),
  ])
  .width(Length::Fill);

  let rows: Vec<Element<'static, Message>> = section
    .rows
    .iter()
    .map(|row| match row {
      ModalRow::Single {
        label,
        type_id,
      } => modal_single_row(label, *type_id, selected_id == Some(*type_id)),
      ModalRow::Family {
        name,
        variants,
        ..
      } => modal_family_row(name, variants, selected_id),
    })
    .collect();

  column([title.into(), column(rows).spacing(2.0).width(Length::Fill).into()])
    .width(Length::Fill)
    .into()
}
