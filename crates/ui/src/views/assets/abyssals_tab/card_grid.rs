//! Card grid — scrollable list of filtered abyssal module cards.

use std::collections::HashMap;

use iced::{
  Element, Length, Padding, Theme,
  widget::{column, container, scrollable, text},
};
use pod_model::AbyssalViewModel;

use super::Message;
use crate::{
  style::{color, typography::body},
  views::assets::State,
};

fn item_passes_filter(
  item: &AbyssalViewModel,
  selected_source_type_id: Option<i32>,
  stat_range_filters: &HashMap<i32, (f64, f64)>,
) -> bool {
  if selected_source_type_id.is_some_and(|id| item.type_id != id) {
    return false;
  }
  for (attr_id, (min_val, max_val)) in stat_range_filters {
    if let Some(stat) = item.stats.iter().find(|s| s.attribute_id == *attr_id) {
      if stat.rolled_value < *min_val || stat.rolled_value > *max_val {
        return false;
      }
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

fn card_grid<'a>(state: &'a State) -> Element<'a, Message> {
  if state.abyssals.abyssals.is_empty() {
    return empty_grid_message("No abyssal modules synced yet.\nSync your characters to load abyssal data.");
  }

  let char_name_map: HashMap<i64, &str> = state.characters.iter().map(|c| (*c.id(), c.name().as_str())).collect();
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
  let cards: Vec<Element<'_, Message>> = items[..visible]
    .iter()
    .map(|item| {
      let char_name = char_name_map.get(&item.character_id).copied().unwrap_or("");
      let portrait = state.abyssals.portrait_handles.get(&item.character_id).cloned();
      container(super::abyssal_card::Component::new(item, char_name, type_icons, portrait).render())
        .padding(Padding {
          top: 0.0,
          bottom: 16.0,
          left: 0.0,
          right: 0.0,
        })
        .max_width(500.0)
        .width(Length::Fill)
        .into()
    })
    .collect();

  scrollable(
    container(column(cards).spacing(0.0))
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: 28.0,
        right: 28.0,
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
}

impl<'a> Component<'a> {
  /// Creates a new card grid builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the card grid into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    card_grid(self.state)
  }
}
