//! Tags settings panel: tag management with drag/drop, colors, and sorting.

mod tag_color_swatch;
pub mod tag_empty_state;
pub mod tag_list_row;
pub mod tag_panel_header;

use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, mouse_area, row, scrollable},
};
pub use tag_color_swatch::TagColorSwatch;
pub use tag_empty_state::Component as TagEmptyState;
pub use tag_list_row::TagListRow;

use crate::style::{color, spacing};

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

  let preview = TagColorSwatch::new(name, color_hex).render();
fn render_tags_panel(state: &State) -> Element<'_, Message> {
  let header = tag_panel_header::TagPanelHeader::new(state).render();
  let border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::border::SUBTLE)),
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
    TagSortMode::Color => filtered.sort_by(|(_, a_name, a_color), (_, b_name, b_color)| {
      match (a_color, b_color) {
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(ca), Some(cb)) => ca.cmp(cb).then(a_name.cmp(b_name)),
        (None, None) => a_name.cmp(b_name),
      }
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

    let mut row_builder = TagListRow::new(*id, name)
      .draggable(draggable)
      .editing(editing)
      .draft(&state.draft)
      .color_open(color_open)
      .is_dragging(is_dragging_this)
      .is_drop_above(is_drop_above);

    if let Some(hex) = color_hex.as_deref() {
      row_builder = row_builder.color_hex(hex);
    }

    let row_elements = row_builder.render();

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
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}
