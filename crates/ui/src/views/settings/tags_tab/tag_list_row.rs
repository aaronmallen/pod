//! Single tag row component for the tags settings panel.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, mouse_area, row, text, text_input},
};

use crate::{
  components::ColorPicker,
  style::{color, radius, spacing, typography},
  views::settings::tags_tab::Message,
};

/// Builder for a single tag entry row in the tags settings panel.
///
/// Renders: drag handle, color picker, inline rename input or label
/// button, tag preview chip, and delete button. Returns a `Vec` to
/// allow the optional drop-indicator element to be prepended.
pub struct TagListRow<'a> {
  /// The hex color string for the tag (e.g. `"#3FB8DB"`), or `None`.
  pub color_hex: Option<&'a str>,
  /// Whether the color picker popover is currently open for this row.
  pub color_open: bool,
  /// Current rename draft string (used when `editing` is `true`).
  pub draft: &'a str,
  /// Whether this tag is draggable (manual sort mode, no active filter).
  pub draggable: bool,
  /// Whether this tag row is in inline-rename mode.
  pub editing: bool,
  /// The database ID of the tag.
  pub id: i32,
  /// Whether this tag is the one currently being dragged.
  pub is_dragging: bool,
  /// Whether the drop indicator should appear above this row.
  pub is_drop_above: bool,
  /// The display name of the tag.
  pub name: &'a str,
}

impl<'a> TagListRow<'a> {
  /// Create a new `TagListRow` builder for the tag with the given `id` and `name`.
  pub fn new(id: i32, name: &'a str) -> Self {
    Self {
      color_hex: None,
      color_open: false,
      draft: "",
      draggable: false,
      editing: false,
      id,
      is_dragging: false,
      is_drop_above: false,
      name,
    }
  }

  /// Set the hex color string for the tag (e.g. `"#3FB8DB"`).
  pub fn color_hex(mut self, hex: &'a str) -> Self {
    self.color_hex = Some(hex);
    self
  }

  /// Set whether the color picker popover is open for this row.
  pub fn color_open(mut self, open: bool) -> Self {
    self.color_open = open;
    self
  }

  /// Set the rename draft string shown in the inline text input.
  pub fn draft(mut self, draft: &'a str) -> Self {
    self.draft = draft;
    self
  }

  /// Set whether this row is draggable.
  pub fn draggable(mut self, draggable: bool) -> Self {
    self.draggable = draggable;
    self
  }

  /// Set whether this tag is in inline-rename mode.
  pub fn editing(mut self, editing: bool) -> Self {
    self.editing = editing;
    self
  }

  /// Set whether this tag is the one currently being dragged.
  pub fn is_dragging(mut self, dragging: bool) -> Self {
    self.is_dragging = dragging;
    self
  }

  /// Set whether the drop indicator should appear above this row.
  pub fn is_drop_above(mut self, drop_above: bool) -> Self {
    self.is_drop_above = drop_above;
    self
  }

  /// Consume the builder and return the finished row elements.
  ///
  /// Returns a `Vec` so the optional drop-indicator element can be
  /// prepended before the row itself.
  pub fn render(self) -> Vec<Element<'a, Message>> {
    let id = self.id;
    let handle = drag_handle(self.draggable);

    let on_toggle = if self.color_open {
      Message::ColorClose
    } else {
      Message::ColorOpen(id)
    };
    let color_picker = ColorPicker::new(
      self.color_hex.unwrap_or(""),
      self.color_open,
      move |hex: String| {
        if hex.is_empty() {
          Message::SetColor(id, None)
        } else {
          Message::SetColor(id, Some(hex))
        }
      },
      on_toggle,
    )
    .render();

    let name_el: Element<'a, Message> = if self.editing {
      text_input("", self.draft)
        .on_input(Message::DraftChanged)
        .on_submit(Message::Rename)
        .font(typography::body::REGULAR)
        .size(14.0)
        .style(|_, _| text_input::Style {
          background: Background::Color(color::surface::SUNKEN),
          border: Border {
            color: color::accent::PLASMA,
            radius: radius::CHIP.into(),
            width: 1.0,
          },
          icon: color::text::SECONDARY,
          placeholder: color::text::TERTIARY,
          value: color::text::PRIMARY,
          selection: color::state::SELECTION,
        })
        .into()
    } else {
      button(
        text(self.name)
          .font(typography::body::REGULAR)
          .size(14.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          }),
      )
      .padding(Padding::ZERO)
      .on_press(Message::EditStart(id))
      .style(|_, _| button::Style {
        background: None,
        border: Border::default(),
        snap: false,
        text_color: color::text::PRIMARY,
        shadow: iced::Shadow::default(),
      })
      .into()
    };

    let preview = tag_preview_chip(self.name, self.color_hex);

    let delete_btn = button(
      container(text("\u{00D7}").font(typography::body::REGULAR).size(14.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center),
    )
    .width(26.0)
    .height(26.0)
    .padding(Padding::ZERO)
    .on_press(Message::Delete(id))
    .style(|_, status| button::Style {
      background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Some(Background::Color(color::status::DANGER_SUBTLE))
      } else {
        None
      },
      border: Border {
        color: color::border::SUBTLE,
        radius: radius::CHIP.into(),
        width: 1.0,
      },
      snap: false,
      text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        color::status::DANGER
      } else {
        color::text::SECONDARY
      },
      shadow: iced::Shadow::default(),
    });

    let drag_handle_el: Element<'a, Message> = if self.draggable {
      mouse_area(handle).on_press(Message::DragStart(id)).into()
    } else {
      handle
    };

    let row_bg = if self.is_dragging {
      Some(Background::Color(iced::Color {
        a: 0.04,
        ..color::accent::PLASMA
      }))
    } else {
      None
    };

    let tag_row: Element<'a, Message> = container(
      row([
        drag_handle_el,
        color_picker,
        container(name_el)
          .width(Length::Fill)
          .padding(Padding {
            left: spacing::SPACE_3,
            right: spacing::SPACE_3,
            ..Padding::ZERO
          })
          .into(),
        preview,
        delete_btn.into(),
      ])
      .spacing(10.0)
      .align_y(Vertical::Center)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: 4.0,
        right: 4.0,
      }),
    )
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: row_bg,
      ..container::Style::default()
    })
    .into();

    let row_border: Element<'a, Message> = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into();

    let mut result: Vec<Element<'a, Message>> = Vec::new();
    if self.is_drop_above {
      result.push(drop_indicator());
    }
    result.push(tag_row);
    result.push(row_border);
    result
  }
}

fn drag_handle<'a, MSG: 'a>(draggable: bool) -> Element<'a, MSG> {
  let dot_color = if draggable {
    color::text::DIM
  } else {
    color::text::GHOST
  };
  container(column([
    drag_handle_pair(dot_color),
    Space::new().height(5.0).into(),
    drag_handle_pair(dot_color),
    Space::new().height(5.0).into(),
    drag_handle_pair(dot_color),
  ]))
  .width(18.0)
  .height(24.0)
  .center_x(18.0)
  .center_y(24.0)
  .into()
}

fn drag_handle_pair<'a, MSG: 'a>(dot_color: iced::Color) -> Element<'a, MSG> {
  row([
    container(Space::new())
      .width(2.4)
      .height(2.4)
      .style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(6.0).into(),
    container(Space::new())
      .width(2.4)
      .height(2.4)
      .style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
  ])
  .into()
}

fn drop_indicator<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(2.0))
    .width(Length::Fill)
    .height(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      ..container::Style::default()
    })
    .into()
}

fn hex_to_iced_color(hex: &str) -> Option<iced::Color> {
  let hex = hex.trim_start_matches('#');
  if hex.len() != 6 {
    return None;
  }
  let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
  let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
  let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
  Some(iced::Color {
    r,
    g,
    b,
    a: 1.0,
  })
}

fn tag_preview_chip<'a>(name: &'a str, color_hex: Option<&'a str>) -> Element<'a, Message> {
  let (bg, fg, bd) = match color_hex.and_then(hex_to_iced_color) {
    Some(c) => (
      iced::Color {
        a: 0.12,
        ..c
      },
      c,
      iced::Color {
        a: 0.45,
        ..c
      },
    ),
    None => (color::state::TAG_FILL, color::text::SECONDARY, color::border::SUBTLE),
  };
  container(
    text(name)
      .font(typography::body::MEDIUM)
      .size(11.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 8.0,
    right: 8.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: bd,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}
