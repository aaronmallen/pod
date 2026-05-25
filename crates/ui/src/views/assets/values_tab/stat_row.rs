//! Row builders for the character × location value matrix.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, container, row, text},
};

use super::{
  super::CharacterStructureCell,
  Message,
  stat_cell::{ValuesStatCell, ValuesStatCellKind},
};
use crate::style::{
  color,
  typography::{body, mono},
};

/// Variant controlling the content and style of a `ValuesStatRow`.
pub enum ValuesStatRowKind<'a> {
  /// Column header row showing structure names.
  Header {
    /// Ordered list of structure display names.
    structures: &'a [String],
  },
  /// Data row for a single character showing per-structure values.
  CharData {
    /// The character's numeric ID.
    char_id: i64,
    /// The character's display name.
    char_name: &'a str,
    /// All character/structure cells for the matrix.
    cells: &'a [CharacterStructureCell],
    /// Ordered list of structure names for column alignment.
    structures: &'a [String],
  },
  /// Totals row showing per-column sums and the grand total.
  Totals {
    /// All character/structure cells for the matrix.
    cells: &'a [CharacterStructureCell],
    /// Ordered list of structure names for column alignment.
    structures: &'a [String],
    /// The pre-computed grand total value.
    total_value: f64,
  },
  /// Title row showing the matrix heading and summary counts.
  Title {
    /// Number of characters in the matrix.
    n_chars: usize,
    /// Number of structures/locations in the matrix.
    n_structs: usize,
  },
}

/// Builder for a single row in the character × location value matrix.
pub struct ValuesStatRow<'a> {
  /// The kind of row to render.
  kind: ValuesStatRowKind<'a>,
}

impl<'a> ValuesStatRow<'a> {
  /// Creates a new row builder with the given kind.
  pub fn new(kind: ValuesStatRowKind<'a>) -> Self {
    Self {
      kind,
    }
  }

  /// Renders the row into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    match self.kind {
      ValuesStatRowKind::Header {
        structures,
      } => render_header_row(structures),
      ValuesStatRowKind::CharData {
        char_id,
        char_name,
        cells,
        structures,
      } => render_char_data_row(cells, char_id, char_name, structures),
      ValuesStatRowKind::Totals {
        cells,
        structures,
        total_value,
      } => render_totals_row(cells, structures, total_value),
      ValuesStatRowKind::Title {
        n_chars,
        n_structs,
      } => render_title_row(n_chars, n_structs),
    }
  }
}

fn render_header_row(structures: &[String]) -> Element<'static, Message> {
  let mut header_cells: Vec<Element<'static, Message>> = vec![
    ValuesStatCell::new(ValuesStatCellKind::Header {
      label: "CHARACTER".to_string(),
      width: 160.0.into(),
    })
    .render(),
  ];
  for s in structures {
    let short = s.split(" · ").next().unwrap_or(s.as_str()).to_string();
    header_cells.push(
      ValuesStatCell::new(ValuesStatCellKind::Header {
        label: short,
        width: Length::Fixed(120.0),
      })
      .render(),
    );
  }
  header_cells.push(ValuesStatCell::new(ValuesStatCellKind::TotalHeader).render());

  container(
    row(header_cells)
      .width(Length::Fill)
      .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
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

fn char_row_total(cells: &[CharacterStructureCell], char_id: i64) -> f64 {
  cells
    .iter()
    .filter(|c| c.character_id == char_id)
    .map(|c| c.value)
    .sum()
}

fn char_struct_value(cells: &[CharacterStructureCell], char_id: i64, struct_name: &str) -> f64 {
  cells
    .iter()
    .filter(|c| c.character_id == char_id && c.structure_name == struct_name)
    .map(|c| c.value)
    .sum()
}

fn build_char_row_cells(
  cells: &[CharacterStructureCell],
  char_id: i64,
  char_name: &str,
  structures: &[String],
  row_total: f64,
) -> Vec<Element<'static, Message>> {
  let mut row_cells: Vec<Element<'static, Message>> = vec![
    ValuesStatCell::new(ValuesStatCellKind::CharName {
      name: char_name.to_string(),
    })
    .render(),
  ];
  for struct_name in structures {
    let v = char_struct_value(cells, char_id, struct_name);
    row_cells.push(
      ValuesStatCell::new(ValuesStatCellKind::Value {
        value: v,
        row_total,
      })
      .render(),
    );
  }
  row_cells.push(
    ValuesStatCell::new(ValuesStatCellKind::RowTotal {
      value: row_total,
    })
    .render(),
  );
  row_cells
}

fn render_char_data_row(
  cells: &[CharacterStructureCell],
  char_id: i64,
  char_name: &str,
  structures: &[String],
) -> Element<'static, Message> {
  let row_total = char_row_total(cells, char_id);
  let row_cells = build_char_row_cells(cells, char_id, char_name, structures, row_total);

  container(
    row(row_cells)
      .width(Length::Fill)
      .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn render_totals_row(
  cells: &[CharacterStructureCell],
  structures: &[String],
  total_value: f64,
) -> Element<'static, Message> {
  let mut totals_cells: Vec<Element<'static, Message>> =
    vec![ValuesStatCell::new(ValuesStatCellKind::TotalsLabel).render()];

  for struct_name in structures {
    let col_total: f64 = cells
      .iter()
      .filter(|c| &c.structure_name == struct_name)
      .map(|c| c.value)
      .sum();
    totals_cells.push(
      ValuesStatCell::new(ValuesStatCellKind::ColTotal {
        value: col_total,
      })
      .render(),
    );
  }
  totals_cells.push(
    ValuesStatCell::new(ValuesStatCellKind::GrandTotal {
      value: total_value,
    })
    .render(),
  );

  container(
    row(totals_cells)
      .width(Length::Fill)
      .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::DEFAULT,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn render_title_row(n_chars: usize, n_structs: usize) -> Element<'static, Message> {
  container(
    row([
      text("Value · character × location")
        .font(body::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().width(12.0).into(),
      text(format!("{n_chars} char · {n_structs} loc"))
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 14.0,
    left: 18.0,
    right: 18.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}
