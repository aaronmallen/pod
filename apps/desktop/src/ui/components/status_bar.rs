use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, Space, container},
};

use crate::ui::style::{color, radius, spacing};

const DIVIDER_HEIGHT: f32 = 15.0;

pub fn status_bar<'a, M>(left: Vec<Element<'a, M>>, right: Vec<Element<'a, M>>) -> Element<'a, M>
where
  M: 'a,
{
  let has_both = !left.is_empty() && !right.is_empty();
  let leading = Row::with_children(left)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);
  let trailing = Row::with_children(right)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  let mut segments: Vec<Element<'a, M>> = vec![leading.into()];
  if has_both {
    segments.push(divider());
  }
  segments.push(Space::new().width(Length::Fill).into());
  if has_both {
    segments.push(divider());
  }
  segments.push(trailing.into());

  let body = Row::with_children(segments)
    .spacing(spacing::SPACE_3)
    .height(Length::Fill)
    .align_y(Vertical::Center);

  container(body)
    .width(Length::Fill)
    .height(spacing::layout::STATUS_BAR_HEIGHT)
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3,
      bottom: 0.0,
      left: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::NAVIGATION)),
      border: Border {
        radius: iced::border::bottom(radius::PANEL),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn divider<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  container(
    Space::new()
      .width(Length::Fixed(1.0))
      .height(Length::Fixed(DIVIDER_HEIGHT)),
  )
  .width(Length::Fixed(1.0))
  .height(Length::Fixed(DIVIDER_HEIGHT))
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_renders_with_and_without_each_slot() {
    let _both: Element<'_, ()> = status_bar(
      vec![iced::widget::text("L").into()],
      vec![iced::widget::text("R").into()],
    );
    let _left: Element<'_, ()> = status_bar(vec![iced::widget::text("L").into()], vec![]);
    let _right: Element<'_, ()> = status_bar(vec![], vec![iced::widget::text("R").into()]);
    let _empty: Element<'_, ()> = status_bar(vec![], vec![]);
  }
}
