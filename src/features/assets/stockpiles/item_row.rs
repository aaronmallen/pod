use iced::{
  Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, text},
};

use super::{StockpileItemLine, type_icon};
use crate::{
  features::assets::{Message, fmt_count},
  ui::{
    components::{progress_bar::progress_bar, rule},
    style::{color, spacing, typography},
  },
};

const BAR_HEIGHT: f32 = 2.0;

pub(super) fn view(item: &StockpileItemLine) -> Element<'_, Message> {
  let ok = item.have >= item.target;
  let bar_color = if ok {
    color::status::ONLINE
  } else if item.pct > 0.5 {
    color::status::WARNING
  } else {
    color::status::DANGER
  };

  let name_and_bar = Column::with_children(vec![
    text(item.type_name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    progress_bar(item.pct as f32, bar_color, BAR_HEIGHT),
  ])
  .spacing(spacing::UNIT + 1.0)
  .width(Length::Fill);

  let content = Row::with_children(vec![type_icon(item.type_id), name_and_bar.into(), readout(item, ok)])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  Column::with_children(vec![
    container(content)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        ..Padding::ZERO
      })
      .into(),
    rule::horizontal_alpha(0.08),
  ])
  .width(Length::Fill)
  .into()
}

fn readout(item: &StockpileItemLine, ok: bool) -> Element<'_, Message> {
  let got_color = if ok {
    color::status::ONLINE
  } else {
    color::text::PRIMARY
  };

  let mut figure_parts: Vec<Element<'_, Message>> = Vec::with_capacity(3);
  if ok {
    figure_parts.push(
      text("\u{2713} ")
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::status::ONLINE),
        })
        .into(),
    );
  }
  figure_parts.push(
    text(fmt_count(item.have))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(got_color),
      })
      .into(),
  );
  figure_parts.push(
    text(format!(" / {}", fmt_count(item.target)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  );
  let figures = Row::with_children(figure_parts);

  let mut children: Vec<Element<'_, Message>> = vec![figures.into()];
  if !ok {
    children.push(
      text(format!("need {}", fmt_count(item.target - item.have)))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
    );
  }

  container(
    Column::with_children(children)
      .spacing(spacing::UNIT)
      .align_x(iced::alignment::Horizontal::Right),
  )
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn line(have: i64, target: i64, pct: f64) -> StockpileItemLine {
    StockpileItemLine {
      have,
      pct,
      target,
      type_id: 34,
      type_name: "Tritanium".to_owned(),
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_short_item_row_with_a_need_caption() {
      let model = line(400, 1000, 0.4);

      let _el: Element<'_, Message> = view(&model);
    }

    #[test]
    fn it_renders_a_satisfied_item_row() {
      let model = line(1000, 1000, 1.0);

      let _el: Element<'_, Message> = view(&model);
    }
  }
}
