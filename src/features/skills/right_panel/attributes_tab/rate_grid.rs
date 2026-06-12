use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Column, Row, container, text},
};

use super::{PAIR_LABELS, attribute_short, card, group_thousands_u64};
use crate::{
  features::skills::attributes::PairRate,
  ui::{
    components::eyebrow::eyebrow,
    style::{color, radius, spacing, typography},
  },
};

pub fn rate_grid<'a, Message: 'a>(matrix: &[PairRate; 6]) -> Element<'a, Message> {
  let mut grid_rows: Vec<Element<'a, Message>> = Vec::with_capacity(3);
  for pair in (0..matrix.len()).step_by(2) {
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(2);
    for index in pair..(pair + 2).min(matrix.len()) {
      cells.push(rate_cell(matrix[index], PAIR_LABELS[index]));
    }
    grid_rows.push(
      Row::with_children(cells)
        .spacing(spacing::SPACE_2)
        .width(Length::Fill)
        .into(),
    );
  }

  let body = Column::with_children({
    let mut children: Vec<Element<'a, Message>> = vec![eyebrow(
      "Training rate by attribute pair",
      Some(color::text::secondary()),
    )];
    children.extend(grid_rows);
    children
  })
  .spacing(spacing::SPACE_2_5)
  .width(Length::Fill);

  card(
    body.into(),
    color::surface::SUNKEN,
    color::with_alpha(color::text::PRIMARY, 0.1),
  )
}

fn rate_cell<'a, Message: 'a>(cell: PairRate, label: &str) -> Element<'a, Message> {
  let (label_color, fill, border) = if cell.active {
    (
      color::accent::PLASMA,
      color::with_alpha(color::accent::PLASMA, 0.08),
      color::with_alpha(color::accent::PLASMA, 0.3),
    )
  } else {
    (
      color::text::secondary(),
      color::surface::RAISED,
      color::with_alpha(color::text::PRIMARY, 0.1),
    )
  };

  let inner = Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
    text(group_thousands_u64(cell.sp_per_hr))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!(
      "SP/hr · {}+{}",
      attribute_short(cell.primary),
      attribute_short(cell.secondary)
    ))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    })
    .into(),
  ])
  .spacing(spacing::UNIT);

  container(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: border,
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}
