//! Card grid — scrollable multi-column grid of filtered abyssal module cards.

use std::collections::HashMap;

use iced::{
  Element, Length, Padding, Theme,
  widget::{column, container, row, scrollable, text},
};
use pod_model::AbyssalViewModel;

use super::Message;
use crate::{
  style::{color, spacing, typography::body},
  views::assets::State,
};

const CARD_MAX_WIDTH: f32 = 500.0;
const GRID_PADDING_H: f32 = 28.0;

fn grid_cols(window_width: f32) -> usize {
  let available = (window_width - GRID_PADDING_H * 2.0).max(0.0);
  let cols = (available / CARD_MAX_WIDTH).floor() as usize;
  cols.max(1)
}

fn item_passes_filter(
  item: &AbyssalViewModel,
  selected_source_type_id: Option<i32>,
  stat_range_filters: &HashMap<i32, (f64, f64)>,
) -> bool {
  if selected_source_type_id.is_some_and(|id| item.type_id != id) {
    return false;
  }
  for (attr_id, (min_val, max_val)) in stat_range_filters {
    if let Some(stat) = item.stats.iter().find(|s| s.attribute_id == *attr_id)
      && (stat.rolled_value < *min_val || stat.rolled_value > *max_val)
    {
      return false;
    }
  }
  true
}

fn empty_grid_message(msg: &str) -> Element<'static, Message> {
  container(
    text(msg.to_string())
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .center(Length::Fill)
  .into()
}

fn build_card<'a>(
  item: &'a AbyssalViewModel,
  char_name: &'a str,
  type_icons: &'a HashMap<i32, iced::widget::image::Handle>,
  portrait: Option<iced::widget::image::Handle>,
) -> Element<'a, Message> {
  super::abyssal_card::Component::new(item, char_name, type_icons, portrait).render()
}

fn char_name_for(state: &State, character_id: i64) -> &str {
  state
    .characters
    .iter()
    .find(|c| *c.id() == character_id)
    .map(|c| c.name().as_str())
    .unwrap_or("")
}

fn card_grid<'a>(state: &'a State, window_width: f32) -> Element<'a, Message> {
  if state.abyssals.abyssals.is_empty() {
    return empty_grid_message("No abyssal modules synced yet.\nSync your characters to load abyssal data.");
  }

  let type_icons = &state.abyssals.type_icons;

  let items: Vec<&AbyssalViewModel> = state
    .abyssals
    .abyssals
    .iter()
    .filter(|item| {
      item_passes_filter(
        item,
        state.abyssals.selected_source_type_id,
        &state.abyssals.stat_range_filters,
      )
    })
    .collect();

  if items.is_empty() {
    return empty_grid_message("No abyssal modules match the current filters.");
  }

  let visible = state.abyssals.visible_count.min(items.len());
  let visible_items = &items[..visible];
  let cols = grid_cols(window_width);

  let mut grid_rows: Vec<Element<'_, Message>> = Vec::new();
  for chunk in visible_items.chunks(cols) {
    let mut cells: Vec<Element<'_, Message>> = chunk
      .iter()
      .map(|item| {
        let char_name = char_name_for(state, item.character_id);
        let portrait = state.abyssals.portrait_handles.get(&item.character_id).cloned();
        build_card(item, char_name, type_icons, portrait)
      })
      .collect();
    while cells.len() < cols {
      cells.push(iced::widget::Space::new().width(Length::Fill).into());
    }
    grid_rows.push(row(cells).spacing(spacing::SPACE_4).into());
  }

  scrollable(
    container(column(grid_rows).spacing(spacing::SPACE_4))
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: GRID_PADDING_H,
        right: GRID_PADDING_H,
      })
      .width(Length::Fill),
  )
  .height(Length::Fill)
  .on_scroll(|vp| Message::ScrollUpdate(vp.relative_offset().y))
  .into()
}

/// Builder for the abyssal card grid.
pub struct Component<'a> {
  state: &'a State,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new card grid builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_width: 1200.0,
    }
  }

  /// Sets the available window width for computing column count.
  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  /// Renders the card grid into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    card_grid(self.state, self.window_width)
  }
}
