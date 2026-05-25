//! Individual cell builders for the character × location value matrix.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{container, text},
};

use super::Message;
use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

/// Variant controlling the visual style and content of a `ValuesStatCell`.
#[derive(Debug)]
pub enum ValuesStatCellKind {
  /// Column header cell with a text label.
  Header {
    /// Label text for the header.
    label: String,
    /// Width of the cell.
    width: Length,
  },
  /// Total column header cell (always fixed 120px with a border).
  TotalHeader,
  /// Data value cell showing an ISK amount heat-mapped by row share.
  Value {
    /// The cell value in ISK.
    value: f64,
    /// The row total used to compute heat-map intensity.
    row_total: f64,
  },
  /// Row total cell at the right end of a data row.
  RowTotal {
    /// The row total value in ISK.
    value: f64,
  },
  /// Character name cell at the left end of a data row.
  CharName {
    /// The character's display name.
    name: String,
  },
  /// "COLUMN TOTAL" label cell at the left of the totals row.
  TotalsLabel,
  /// Per-column total cell in the totals row.
  ColTotal {
    /// The column total value in ISK.
    value: f64,
  },
  /// Grand total cell at the bottom-right of the matrix.
  GrandTotal {
    /// The grand total value in ISK.
    value: f64,
  },
}

/// Builder for a single cell in the character × location value matrix.
pub struct ValuesStatCell {
  /// The kind of cell to render.
  kind: ValuesStatCellKind,
}

impl ValuesStatCell {
  /// Creates a new cell builder with the given kind.
  pub fn new(kind: ValuesStatCellKind) -> Self {
    Self {
      kind,
    }
  }

  /// Renders the cell into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    match self.kind {
      ValuesStatCellKind::Header {
        label,
        width,
      } => render_header(label, width),
      ValuesStatCellKind::TotalHeader => render_total_header(),
      ValuesStatCellKind::CharName {
        name,
      } => render_char_name(name),
      kind => render_data_cell(kind),
    }
  }
}

fn render_data_cell(kind: ValuesStatCellKind) -> Element<'static, Message> {
  match kind {
    ValuesStatCellKind::Value {
      value,
      row_total,
    } => render_value(value, row_total),
    ValuesStatCellKind::RowTotal {
      value,
    } => render_row_total(value),
    ValuesStatCellKind::TotalsLabel => render_totals_label(),
    ValuesStatCellKind::ColTotal {
      value,
    } => render_col_total(value),
    ValuesStatCellKind::GrandTotal {
      value,
    } => render_grand_total(value),
    _ => render_totals_label(),
  }
}

fn render_header(label: String, width: Length) -> Element<'static, Message> {
  container(
    text(label)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(width)
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn render_total_header() -> Element<'static, Message> {
  container(
    text("TOTAL")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
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

fn render_value(v: f64, row_total: f64) -> Element<'static, Message> {
  let intensity = if row_total > 0.0 { (v / row_total) as f32 } else { 0.0 };
  let bg = if v > 0.0 {
    Some(Background::Color(color::plasma_heat(intensity)))
  } else {
    None
  };
  let label = if v == 0.0 {
    "\u{2014}".to_string()
  } else {
    format::fmt_isk(v)
  };
  let text_color = if v == 0.0 {
    color::text::TERTIARY
  } else {
    color::text::PRIMARY
  };

  container(
    text(label)
      .font(mono::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(move |_| container::Style {
    background: bg,
    ..container::Style::default()
  })
  .into()
}

fn render_row_total(row_total: f64) -> Element<'static, Message> {
  container(
    text(format::fmt_isk(row_total))
      .font(mono::MEDIUM)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
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

fn render_char_name(char_name: String) -> Element<'static, Message> {
  container(
    text(char_name)
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(160.0)
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .into()
}

fn render_totals_label() -> Element<'static, Message> {
  container(
    text("COLUMN TOTAL")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(160.0)
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn render_col_total(col_total: f64) -> Element<'static, Message> {
  container(
    text(format::fmt_isk(col_total))
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn render_grand_total(total_value: f64) -> Element<'static, Message> {
  container(
    text(format::fmt_isk(total_value))
      .font(mono::MEDIUM)
      .size(14.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
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
