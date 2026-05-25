//! Character × location value matrix panel for the values tab.

use iced::{
  Background, Border, Element, Length,
  widget::{column, container, scrollable},
};

use super::{
  super::CharacterStructureCell,
  Message,
  stat_row::{ValuesStatRow, ValuesStatRowKind},
};
use crate::style::color;

/// Builder for the character × location value matrix panel.
pub struct Component<'a> {
  /// The character/structure cell data.
  cells: &'a [CharacterStructureCell],
  /// The pre-computed grand total value.
  total_value: f64,
}

impl<'a> Component<'a> {
  /// Creates a new matrix panel builder.
  pub fn new(cells: &'a [CharacterStructureCell], total_value: f64) -> Self {
    Self {
      cells,
      total_value,
    }
  }

  /// Renders the matrix panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let structures = sorted_structures(self.cells);
    let characters = sorted_characters(self.cells);

    let n_chars = characters.len();
    let n_structs = structures.len();

    let header_row = ValuesStatRow::new(ValuesStatRowKind::Header {
      structures: &structures,
    })
    .render();

    let data_rows: Vec<Element<'static, Message>> = characters
      .iter()
      .map(|(char_id, char_name)| {
        ValuesStatRow::new(ValuesStatRowKind::CharData {
          char_id: *char_id,
          char_name,
          cells: self.cells,
          structures: &structures,
        })
        .render()
      })
      .collect();

    let totals_row = ValuesStatRow::new(ValuesStatRowKind::Totals {
      cells: self.cells,
      structures: &structures,
      total_value: self.total_value,
    })
    .render();

    let title_row = ValuesStatRow::new(ValuesStatRowKind::Title {
      n_chars,
      n_structs,
    })
    .render();

    let mut body_rows: Vec<Element<'static, Message>> = vec![header_row];
    body_rows.extend(data_rows);
    body_rows.push(totals_row);

    container(column([
      title_row,
      scrollable(column(body_rows).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    ]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}

fn structure_total_value(cells: &[CharacterStructureCell], structure_name: &str) -> f64 {
  cells
    .iter()
    .filter(|c| c.structure_name == structure_name)
    .map(|c| c.value)
    .sum()
}

fn unique_structure_names(cells: &[CharacterStructureCell]) -> Vec<String> {
  cells
    .iter()
    .map(|c| c.structure_name.clone())
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect()
}

fn sorted_structures(cells: &[CharacterStructureCell]) -> Vec<String> {
  let mut structures = unique_structure_names(cells);
  structures.sort_by(|a, b| {
    let a_total = structure_total_value(cells, a);
    let b_total = structure_total_value(cells, b);
    b_total.partial_cmp(&a_total).unwrap_or(std::cmp::Ordering::Equal)
  });
  structures
}

fn sorted_characters(cells: &[CharacterStructureCell]) -> Vec<(i64, String)> {
  let mut characters: Vec<(i64, String)> = cells
    .iter()
    .map(|c| (c.character_id, c.character_name.clone()))
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();
  characters.sort_by_key(|(id, _)| *id);
  characters
}
