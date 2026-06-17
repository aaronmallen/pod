use iced::{
  Background, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use crate::ui::{
  components::rule,
  style::{color, spacing, typography},
};

const SIDE_PADDING: f32 = 28.0;
const STAT_DIVIDER_HEIGHT: f32 = 44.0;

pub fn header<'a, M>(left: Vec<Element<'a, M>>, right: Vec<Element<'a, M>>) -> Element<'a, M>
where
  M: 'a,
{
  let left_row = Row::with_children(left)
    .spacing(spacing::SPACE_2)
    .height(Length::Fill)
    .align_y(Vertical::Center);

  let right_row = Row::with_children(right)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  let band = container(
    Row::with_children(vec![
      left_row.into(),
      Space::new().width(Length::Fill).into(),
      right_row.into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    right: SIDE_PADDING,
    bottom: 0.0,
    left: SIDE_PADDING,
  });

  let rule = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    });

  Column::with_children(vec![band.into(), rule.into()])
    .width(Length::Fill)
    .into()
}

pub fn header_divider<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  rule::vertical(STAT_DIVIDER_HEIGHT)
}

pub fn stat_block<'a, M>(label: &str, value: String, value_color: Color, sub: Option<&'a str>) -> Element<'a, M>
where
  M: 'a,
{
  let mut value_row: Vec<Element<'a, M>> = vec![
    text(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(move |_| text::Style {
        color: Some(value_color),
      })
      .into(),
  ];
  if let Some(sub) = sub {
    value_row.push(
      text(sub.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    );
  }

  Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Row::with_children(value_row)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Bottom)
      .into(),
  ])
  .spacing(spacing::UNIT)
  .into()
}

#[cfg(test)]
mod tests {
  use iced::widget::text;

  use super::*;

  mod header {
    use super::*;

    #[test]
    fn it_renders_left_and_right_elements_together() {
      let _el: Element<'_, ()> = header(vec![text("Characters").into()], vec![text("Add").into()]);
    }

    #[test]
    fn it_renders_with_no_elements() {
      let _el: Element<'_, ()> = header(vec![], vec![]);
    }

    #[test]
    fn it_renders_with_only_left_elements() {
      let _el: Element<'_, ()> = header(vec![text("Characters").into()], vec![]);
    }
  }

  mod header_divider {
    use super::*;

    #[test]
    fn it_builds_a_vertical_hairline() {
      let _el: Element<'_, ()> = header_divider();
    }
  }

  mod stat_block {
    use super::*;

    #[test]
    fn it_renders_a_label_over_value_without_a_sub() {
      let _el: Element<'_, ()> = stat_block("Liquid ISK", "1,000 ISK".to_owned(), color::text::PRIMARY, None);
    }

    #[test]
    fn it_renders_a_sub_label_when_provided() {
      let _el: Element<'_, ()> = stat_block("Location", "Jita".to_owned(), color::text::PRIMARY, Some("docked"));
    }
  }
}
