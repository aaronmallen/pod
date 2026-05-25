//! Grid row construction helpers for the characters tab.

use std::collections::HashMap;

use iced::{
  Border, Element, Length,
  widget::{container, mouse_area, row},
};
use pod_model::Character;

use super::{Message, character_card};
use crate::style::{color, radius, spacing};

/// Builder for a single character cell in the grid.
pub struct CharacterCell<'a> {
  /// The character to render.
  character: &'a Character,
  /// Whether drag-and-drop is currently active.
  dragging_id: Option<i64>,
  /// The slot index that the cursor is hovering over during a drag.
  drag_hover: Option<i32>,
  /// Whether skill monitoring is enabled.
  feat_skill_monitoring: bool,
  /// Whether wallet display is enabled.
  feat_wallet: bool,
  /// Portrait image handles keyed by character id.
  portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  /// Grid slot index for this cell.
  slot: i32,
}

impl<'a> CharacterCell<'a> {
  /// Creates a new `CharacterCell` builder for the given character and slot.
  pub fn new(
    character: &'a Character,
    portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
    slot: i32,
  ) -> Self {
    Self {
      character,
      dragging_id: None,
      drag_hover: None,
      feat_skill_monitoring: true,
      feat_wallet: true,
      portrait_handles,
      slot,
    }
  }

  /// Sets the id of the character currently being dragged.
  pub fn dragging_id(mut self, v: Option<i64>) -> Self {
    self.dragging_id = v;
    self
  }

  /// Sets the slot index that the drag cursor is hovering over.
  pub fn drag_hover(mut self, v: Option<i32>) -> Self {
    self.drag_hover = v;
    self
  }

  /// Enables or disables skill monitoring display.
  pub fn feat_skill_monitoring(mut self, v: bool) -> Self {
    self.feat_skill_monitoring = v;
    self
  }

  /// Enables or disables wallet display.
  pub fn feat_wallet(mut self, v: bool) -> Self {
    self.feat_wallet = v;
    self
  }

  /// Renders the cell into an `Element`.
  pub fn render(self) -> Element<'a, Message> {
    let id = *self.character.id();
    let slot = self.slot;
    let is_dragging = self.dragging_id.is_some();
    let is_hover_target = is_hover_target(is_dragging, self.drag_hover, self.dragging_id, slot, id);
    character_card::Component::new(self.character)
      .portrait_handle(self.portrait_handles.get(&id))
      .feat_skill_monitoring(self.feat_skill_monitoring)
      .feat_wallet(self.feat_wallet)
      .is_dragging(self.dragging_id == Some(id))
      .is_hover_target(is_hover_target)
      .render()
      .map(move |msg| map_card_msg(msg, slot, id))
  }
}

/// Builder for an empty grid slot placeholder.
pub struct EmptySlot {
  /// Whether drag-and-drop is currently active.
  is_dragging: bool,
  /// Grid slot index for this placeholder.
  slot: i32,
}

impl EmptySlot {
  /// Creates a new `EmptySlot` builder for the given slot index.
  pub fn new(slot: i32) -> Self {
    Self {
      is_dragging: false,
      slot,
    }
  }

  /// Sets whether a drag is currently in progress.
  pub fn is_dragging(mut self, v: bool) -> Self {
    self.is_dragging = v;
    self
  }

  /// Renders the placeholder into an `Element`.
  pub fn render<'a>(self) -> Element<'a, Message> {
    let content = container(iced::widget::Space::new().width(Length::Fill))
      .width(Length::Fill)
      .height(spacing::layout::CHARACTER_CARD_HEIGHT)
      .style(move |_| {
        if self.is_dragging {
          container::Style {
            border: Border {
              color: color::border::SUBTLE,
              radius: radius::PANEL.into(),
              width: 1.0,
            },
            ..container::Style::default()
          }
        } else {
          container::Style::default()
        }
      });

    if self.is_dragging {
      mouse_area(content).on_enter(Message::SlotEntered(self.slot)).into()
    } else {
      content.into()
    }
  }
}

fn map_card_msg(msg: character_card::Message, slot: i32, id: i64) -> Message {
  match msg {
    character_card::Message::CardEntered(_) => Message::SlotEntered(slot),
    other => Message::Card(id, other),
  }
}

fn is_hover_target(is_dragging: bool, drag_hover: Option<i32>, dragging_id: Option<i64>, slot: i32, id: i64) -> bool {
  is_dragging && drag_hover == Some(slot) && dragging_id != Some(id)
}

/// Returns the number of grid columns for the given window width.
pub(super) fn grid_cols(window_width: f32) -> usize {
  if window_width >= 1000.0 {
    3
  } else if window_width >= 700.0 {
    2
  } else {
    1
  }
}

fn build_slot_map<'a>(characters: &[&'a Character]) -> HashMap<i32, &'a Character> {
  characters.iter().map(|c| (*c.sort_order(), *c)).collect()
}

fn build_fixed_grid_rows<'a>(
  characters: Vec<&'a Character>,
  portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  dragging_id: Option<i64>,
  drag_hover: Option<i32>,
  feat_skill_monitoring: bool,
  feat_wallet: bool,
) -> Vec<Element<'a, Message>> {
  let max_slot = characters.iter().map(|c| *c.sort_order()).max().unwrap_or(0);
  let row_count = (max_slot / 3 + 2) as usize;
  let slot_map = build_slot_map(&characters);
  (0..row_count)
    .map(|row_idx| {
      build_grid_row(
        row_idx,
        &slot_map,
        portrait_handles,
        dragging_id,
        drag_hover,
        feat_skill_monitoring,
        feat_wallet,
      )
    })
    .collect()
}

pub(super) fn build_grid_rows<'a>(
  characters: Vec<&'a Character>,
  cols: usize,
  portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  dragging_id: Option<i64>,
  drag_hover: Option<i32>,
  feat_skill_monitoring: bool,
  feat_wallet: bool,
) -> Vec<Element<'a, Message>> {
  if cols < 3 || characters.is_empty() {
    return build_grid_rows_responsive(
      characters,
      cols,
      portrait_handles,
      dragging_id,
      drag_hover,
      feat_skill_monitoring,
      feat_wallet,
    );
  }
  build_fixed_grid_rows(
    characters,
    portrait_handles,
    dragging_id,
    drag_hover,
    feat_skill_monitoring,
    feat_wallet,
  )
}

fn build_grid_row<'a>(
  row_idx: usize,
  slot_map: &HashMap<i32, &'a Character>,
  portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  dragging_id: Option<i64>,
  drag_hover: Option<i32>,
  feat_skill_monitoring: bool,
  feat_wallet: bool,
) -> Element<'a, Message> {
  let is_dragging = dragging_id.is_some();
  let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(3);
  for col_idx in 0i32..3 {
    let slot = row_idx as i32 * 3 + col_idx;
    if let Some(c) = slot_map.get(&slot) {
      cells.push(
        CharacterCell::new(c, portrait_handles, slot)
          .dragging_id(dragging_id)
          .drag_hover(drag_hover)
          .feat_skill_monitoring(feat_skill_monitoring)
          .feat_wallet(feat_wallet)
          .render(),
      );
    } else {
      cells.push(EmptySlot::new(slot).is_dragging(is_dragging).render());
    }
  }
  row(cells).spacing(spacing::SPACE_4).into()
}

fn build_grid_rows_responsive<'a>(
  characters: Vec<&'a Character>,
  cols: usize,
  portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  dragging_id: Option<i64>,
  drag_hover: Option<i32>,
  feat_skill_monitoring: bool,
  feat_wallet: bool,
) -> Vec<Element<'a, Message>> {
  let mut grid_rows: Vec<Element<'a, Message>> = Vec::new();
  for chunk in characters.chunks(cols) {
    let mut cells: Vec<Element<'a, Message>> = chunk
      .iter()
      .map(|c| {
        let sort_order = *c.sort_order();
        CharacterCell::new(c, portrait_handles, sort_order)
          .dragging_id(dragging_id)
          .drag_hover(drag_hover)
          .feat_skill_monitoring(feat_skill_monitoring)
          .feat_wallet(feat_wallet)
          .render()
      })
      .collect();
    while cells.len() < cols {
      cells.push(iced::widget::Space::new().width(Length::Fill).into());
    }
    grid_rows.push(row(cells).spacing(spacing::SPACE_4).into());
  }
  grid_rows
}
