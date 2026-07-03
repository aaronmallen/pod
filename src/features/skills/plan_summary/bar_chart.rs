use iced::{
  Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::section_label;
use crate::ui::{
  components::progress_bar::progress_bar,
  style::{color, spacing, typography},
};

pub(crate) fn bar_chart_row<'a, M: 'a>(
  label: String,
  time_str: String,
  fraction: f32,
  bar_color: Color,
) -> Element<'a, M> {
  column(vec![
    bar_label_row(label, time_str),
    Space::new().height(4.0).into(),
    progress_bar(fraction, bar_color, 4.0),
  ])
  .width(Length::Fill)
  .into()
}

fn bar_label_row<'a, M: 'a>(label: String, time_str: String) -> Element<'a, M> {
  row(vec![
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(time_str)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

pub(crate) fn time_chart_section<'a, M: 'a>(title: &str, rows: Vec<Element<'a, M>>) -> Element<'a, M> {
  container(
    column(vec![
      container(section_label(title))
        .padding(Padding {
          top: 0.0,
          bottom: spacing::SPACE_3,
          left: 0.0,
          right: 0.0,
        })
        .width(Length::Fill)
        .into(),
      column(rows).width(Length::Fill).into(),
    ])
    .width(Length::Fill),
  )
  .padding(Padding {
    top: spacing::SPACE_3,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  })
  .width(Length::Fill)
  .into()
}
