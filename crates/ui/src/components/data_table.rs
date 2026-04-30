use iced::{
  Background, Border, Color, Element, Length, Theme,
  widget::{column, container, scrollable, text},
};

use crate::style::{color, typography::body};

/// Scrollable data table: optional header, row iteration, and empty-state.
///
/// `row_fn` receives `(item, row_index, total_rows)` so callers that need
/// per-row context (e.g. "is this the last row?") can compute it inline.
pub struct DataTable<'a, T, M> {
  rows: Vec<&'a T>,
  row_fn: Box<dyn Fn(&'a T, usize, usize) -> Element<'a, M> + 'a>,
  empty_message: &'a str,
  header: Option<Element<'a, M>>,
}

impl<'a, T, M: 'a> DataTable<'a, T, M> {
  pub fn new(
    rows: impl IntoIterator<Item = &'a T>,
    row_fn: impl Fn(&'a T, usize, usize) -> Element<'a, M> + 'a,
  ) -> Self {
    Self {
      rows: rows.into_iter().collect(),
      row_fn: Box::new(row_fn),
      empty_message: "No data to display.",
      header: None,
    }
  }

  pub fn empty_message(mut self, msg: &'a str) -> Self {
    self.empty_message = msg;
    self
  }

  pub fn header(mut self, header: Element<'a, M>) -> Self {
    self.header = Some(header);
    self
  }

  pub fn render(self) -> Element<'a, M> {
    let total = self.rows.len();
    let mut col_children: Vec<Element<'a, M>> = Vec::new();

    if let Some(h) = self.header {
      col_children.push(h);
    }

    if total == 0 {
      col_children.push(empty_state(self.empty_message));
      return column(col_children).width(Length::Fill).into();
    }

    let row_fn = self.row_fn;
    let data_rows: Vec<Element<'a, M>> = self
      .rows
      .into_iter()
      .enumerate()
      .map(|(i, row)| row_fn(row, i, total))
      .collect();

    col_children.push(
      scrollable(column(data_rows).width(Length::Fill))
        .height(Length::Fill)
        .into(),
    );

    column(col_children).width(Length::Fill).into()
  }
}

fn empty_state<'a, M: 'a>(msg: &'a str) -> Element<'a, M> {
  container(
    text(msg)
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(32.0)
  .width(Length::Fill)
  .center_x(Length::Fill)
  .height(Length::Fill)
  .center_y(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    border: Border::default(),
    ..container::Style::default()
  })
  .into()
}
