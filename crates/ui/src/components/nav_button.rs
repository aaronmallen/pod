use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Space, container, stack},
};

use crate::{
  components::{Button, Icon},
  style::{color, radius, spacing},
};

/// A navigation icon button with active, hover, and optional badge states.
pub struct NavButton<Message> {
  has_badge: bool,
  icon: Icon,
  is_active: bool,
  is_hovered: bool,
  on_press: Message,
}

impl<Message: Clone + 'static> NavButton<Message> {
  pub fn new(icon: Icon, is_active: bool, is_hovered: bool, has_badge: bool, on_press: Message) -> Self {
    Self {
      has_badge,
      icon,
      is_active,
      is_hovered,
      on_press,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let icon_color = if self.is_active {
      color::text::PRIMARY
    } else {
      color::text::SECONDARY
    };
    let icon_element = self.icon.size(22.0).color(icon_color).render::<Message>();
    let is_active = self.is_active;

    let is_hovered = self.is_hovered;
    let btn = Button::nav(
      container(icon_element).center_x(Length::Fill).center_y(Length::Fill),
      is_active,
    )
    .width(spacing::layout::NAV_ITEM_HEIGHT)
    .height(spacing::layout::NAV_ITEM_HEIGHT)
    .on_press(self.on_press);

    let mut layers: Vec<Element<'static, Message>> = vec![
      container(btn).center_x(Length::Fill).center_y(Length::Fill).into(),
      nav_active_indicator::<Message>(is_active, is_hovered),
    ];

    if self.has_badge {
      layers.push(nav_badge::<Message>());
    }

    stack(layers)
      .width(spacing::layout::RAIL_WIDTH)
      .height(spacing::layout::NAV_ITEM_HEIGHT)
      .into()
  }
}

fn nav_active_indicator<Message: 'static>(is_active: bool, is_hovered: bool) -> Element<'static, Message> {
  container(
    container(Space::new())
      .width(2.0)
      .height(24.0)
      .style(move |_| container::Style {
        background: match (is_active, is_hovered) {
          (true, _) => Some(Background::Color(color::accent::PLASMA)),
          (false, true) => Some(Background::Color(color::text::PRIMARY)),
          (false, false) => None,
        },
        border: Border {
          radius: Radius {
            top_left: 0.0,
            top_right: 2.0,
            bottom_right: 2.0,
            bottom_left: 0.0,
          },
          ..Border::default()
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Left)
  .align_y(Vertical::Center)
  .into()
}

fn nav_badge<Message: 'static>() -> Element<'static, Message> {
  container(
    container(Space::new())
      .width(6.0)
      .height(6.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: radius::FULL.into(),
          ..Border::default()
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Right)
  .align_y(Vertical::Top)
  .padding(Padding::new(8.0))
  .into()
}
