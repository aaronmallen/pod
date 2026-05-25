//! Tags settings panel: tag management with drag/drop, colors, and sorting.

pub mod tag_empty_state;
pub mod tag_panel_header;
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, mouse_area, row, scrollable, text, text_input},
};
pub use tag_empty_state::Component as TagEmptyState;

use crate::{
  components::ColorPicker,
  style::{color, radius, spacing, typography},
};

/// Builder for the tags settings panel.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Create a new tags panel builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    render_tags_panel(self.state)
  }
}

/// Messages produced by the tags panel.
#[derive(Clone, Debug)]
pub enum Message {
  /// The color picker for a tag was closed.
  ColorClose,
  /// The color picker for a tag was opened.
  ColorOpen(i32),
  /// The color-set DB operation returned a result.
  ColorSet(Result<(i32, String, Option<String>), String>),
  /// Create a new tag was requested.
  Create,
  /// The tag-create DB operation returned a result.
  Created(Result<(i32, String, Option<String>), String>),
  /// Delete a tag was requested.
  Delete(i32),
  /// The tag-delete DB operation returned a result.
  Deleted(Result<i32, String>),
  /// A drag was released (drop or cancel).
  DragEnd,
  /// A drag was started on the tag with the given id.
  DragStart(i32),
  /// The rename draft for the currently-edited tag changed.
  DraftChanged(String),
  /// The drag was dropped; drop target is in state.drag_over.
  Drop,
  /// Inline editing of a tag name was initiated.
  EditStart(i32),
  /// The full tag list was loaded from the database.
  Loaded(Vec<(i32, String, Option<String>)>),
  /// The new-tag name input changed.
  NewNameChanged(String),
  /// The rename of the currently-edited tag was committed.
  Rename,
  /// The tag-rename DB operation returned a result.
  Renamed(Result<(i32, String, Option<String>), String>),
  /// The tag-reorder DB operation returned a result.
  Reordered(Result<(), String>),
  /// The tag list filter query changed.
  SearchChanged(String),
  /// A color was selected or cleared for a tag.
  SetColor(i32, Option<String>),
  /// The cursor entered a tag row's bounds during a drag.
  SlotEntered(i32),
  /// The sort mode for the tag list changed.
  SortModeChanged(TagSortMode),
}

/// Runtime state for the tags settings panel.
#[derive(Default)]
pub struct State {
  /// Id of the tag whose color picker is currently open, if any.
  pub color_open: Option<i32>,
  /// Current name draft for the inline rename input.
  pub draft: String,
  /// Id of the tag currently acting as the drop target during a drag.
  pub drag_over: Option<i32>,
  /// Id of the tag currently being dragged, if any.
  pub dragging: Option<i32>,
  /// Id of the tag currently being renamed inline, if any.
  pub editing: Option<i32>,
  /// Text in the "Create a tag" input.
  pub new_name: String,
  /// Filter query for the tag list.
  pub search: String,
  /// Current sort mode for the tag list.
  pub sort_mode: TagSortMode,
  /// All tags, in manual sort order.
  pub tags: Vec<(i32, String, Option<String>)>,
}

impl State {
  /// Returns the number of tags that have a color assigned.
  pub fn colored_count(&self) -> usize {
    self.tags.iter().filter(|(_, _, c)| c.is_some()).count()
  }
}

/// Sort mode for the tag list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TagSortMode {
  /// Colored tags first (grouped by hex), then alphabetical.
  Color,
  /// Manual drag-and-drop order.
  #[default]
  Manual,
  /// Alphabetical by name.
  Name,
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

#[allow(clippy::too_many_arguments)]
fn render_tag_row<'a>(
  id: i32,
  name: &'a str,
  color_hex: Option<&'a str>,
  draggable: bool,
  editing: bool,
  draft: &'a str,
  color_open: bool,
  is_dragging: bool,
  is_drop_above: bool,
) -> Vec<Element<'a, Message>> {
  let handle = drag_handle(draggable);

  let on_toggle = if color_open {
    Message::ColorClose
  } else {
    Message::ColorOpen(id)
  };
  let color_picker = ColorPicker::new(
    color_hex.unwrap_or(""),
    color_open,
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

  let name_el: Element<'a, Message> = if editing {
    text_input("", draft)
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
      text(name)
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

  let preview = tag_preview_chip(name, color_hex);

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

  let drag_handle_el: Element<'a, Message> = if draggable {
    mouse_area(handle).on_press(Message::DragStart(id)).into()
  } else {
    handle
  };

  let row_bg = if is_dragging {
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
  if is_drop_above {
    result.push(drop_indicator());
  }
  result.push(tag_row);
  result.push(row_border);
  result
}

fn render_tags_panel(state: &State) -> Element<'_, Message> {
  let header = tag_panel_header::TagPanelHeader::new(state).render();
  let border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  let list = tag_list_body(state);
  let panel: Element<'_, Message> = column([header, border.into(), list])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  if state.dragging.is_some() {
    mouse_area(panel).on_release(Message::Drop).into()
  } else {
    panel
  }
}

fn tag_list_body(state: &State) -> Element<'_, Message> {
  if state.tags.is_empty() {
    return TagEmptyState::new().render();
  }

  let search = state.search.trim().to_lowercase();
  let draggable = state.sort_mode == TagSortMode::Manual && search.is_empty();

  let mut filtered: Vec<&(i32, String, Option<String>)> = state
    .tags
    .iter()
    .filter(|(_, name, _)| search.is_empty() || name.to_lowercase().contains(&search))
    .collect();

  match state.sort_mode {
    TagSortMode::Manual => {}
    TagSortMode::Name => filtered.sort_by(|(_, a, _), (_, b, _)| a.cmp(b)),
    TagSortMode::Color => filtered.sort_by(|(_, a_name, a_color), (_, b_name, b_color)| match (a_color, b_color) {
      (Some(_), None) => std::cmp::Ordering::Less,
      (None, Some(_)) => std::cmp::Ordering::Greater,
      (Some(ca), Some(cb)) => ca.cmp(cb).then(a_name.cmp(b_name)),
      (None, None) => a_name.cmp(b_name),
    }),
  }

  if filtered.is_empty() {
    return TagEmptyState::new().query(&state.search).render();
  }

  let mut items: Vec<Element<'_, Message>> = Vec::new();
  let is_active_drag = state.dragging.is_some();

  for (id, name, color_hex) in &filtered {
    let editing = state.editing == Some(*id);
    let color_open = state.color_open == Some(*id);
    let is_dragging_this = state.dragging == Some(*id);
    let is_drop_above = is_active_drag && !is_dragging_this && state.drag_over == Some(*id);

    let row_elements = render_tag_row(
      *id,
      name,
      color_hex.as_deref(),
      draggable,
      editing,
      &state.draft,
      color_open,
      is_dragging_this,
      is_drop_above,
    );

    if is_active_drag && !is_dragging_this {
      let id_copy = *id;
      items.push(
        mouse_area(column(row_elements).width(Length::Fill))
          .on_enter(Message::SlotEntered(id_copy))
          .into(),
      );
    } else {
      items.extend(row_elements);
    }
  }

  scrollable(column(items).width(Length::Fill).padding(Padding {
    top: 8.0,
    bottom: 60.0,
    left: 36.0,
    right: 36.0,
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
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
