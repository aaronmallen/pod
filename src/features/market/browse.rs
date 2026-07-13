use iced::{
  Background, Border, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, text},
};

use super::{Message, shell};
use crate::ui::{
  components::icon::Icon,
  style::{
    color,
    control::{bordered_pane, sunken_pane},
    radius, spacing, typography,
  },
};

const TREE_WIDTH: f32 = 286.0;

pub(super) fn surface<'a>() -> iced::Element<'a, Message> {
  Row::with_children(vec![tree_pane(), detail_pane()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn tree_pane<'a>() -> iced::Element<'a, Message> {
  let filter = container(
    Row::with_children(vec![
      Icon::search()
        .size(typography::size::MD)
        .color(color::text::secondary())
        .render(),
      text(t!("market.filter_placeholder").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  });

  let catalog = container(
    text(t!("market.tree_empty").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_4_5);

  let column = Column::with_children(vec![
    container(filter)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2_5,
        right: spacing::SPACE_3,
        bottom: spacing::SPACE_2_5,
        left: spacing::SPACE_3,
      })
      .into(),
    catalog.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  container(column)
    .width(Length::Fixed(TREE_WIDTH))
    .height(Length::Fill)
    .style(sunken_pane)
    .into()
}

fn detail_pane<'a>() -> iced::Element<'a, Message> {
  container(shell::empty_state(
    Icon::contracts(),
    "market.browse_empty_title",
    "market.browse_empty_body",
  ))
  .width(Length::Fill)
  .height(Length::Fill)
  .style(bordered_pane)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_renders_the_two_pane_browse_shell() {
    let _el: iced::Element<'_, Message> = surface();
  }
}
