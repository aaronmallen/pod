//! Grid row layout helpers for the corporation pane.

use std::collections::HashMap;

use iced::{Element, Length, widget::row};
use pod_model::{Character, Corporation};

use super::{CorporationCard, Message};
use crate::style::spacing;

fn corp_cell<'a>(
  corp: &'a Corporation,
  icon_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  characters: &'a [Character],
) -> Element<'a, Message> {
  let id = *corp.id();
  let ceo_id = *corp.ceo_character_id();
  let ceo_name = characters.iter().find(|c| *c.id() == ceo_id).map(|c| c.name().clone());
  CorporationCard::new(corp)
    .icon_handle(icon_handles.get(&id))
    .ceo_name(ceo_name)
    .render()
    .map(move |msg| Message::Card(id, msg))
}

fn pad_cells_to_cols<'a>(cells: &mut Vec<Element<'a, Message>>, cols: usize) {
  while cells.len() < cols {
    cells.push(iced::widget::Space::new().width(Length::Fill).into());
  }
}

/// Builds a list of grid row elements from a flat corporation slice.
pub(super) fn build_corp_grid_rows<'a>(
  corporations: Vec<&'a Corporation>,
  cols: usize,
  icon_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  characters: &'a [Character],
) -> Vec<Element<'a, Message>> {
  let mut grid_rows: Vec<Element<'a, Message>> = Vec::new();

  for chunk in corporations.chunks(cols) {
    let mut cells: Vec<Element<'a, Message>> = chunk
      .iter()
      .map(|corp| corp_cell(corp, icon_handles, characters))
      .collect();
    pad_cells_to_cols(&mut cells, cols);
    grid_rows.push(row(cells).spacing(spacing::SPACE_4).into());
  }

  grid_rows
}

/// Returns the number of grid columns based on the current window width.
pub(super) fn corp_grid_cols(window_width: f32) -> usize {
  if window_width >= 1000.0 {
    3
  } else if window_width >= 700.0 {
    2
  } else {
    1
  }
}
