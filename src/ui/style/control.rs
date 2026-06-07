use iced::{
  Background, Border, Color, Padding,
  widget::{button, container, scrollable},
};

use crate::ui::style::{color, radius, spacing};

pub fn card(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  }
}

pub fn danger_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => color::with_alpha(color::status::DANGER, 0.85),
    _ => color::status::DANGER,
  };
  button::Style {
    background: Some(Background::Color(background)),
    text_color: color::surface::BASE,
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

pub fn ghost_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let border_alpha = match status {
    button::Status::Hovered | button::Status::Pressed => 0.18,
    _ => 0.1,
  };
  button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color: color::text::PRIMARY,
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, border_alpha),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  }
}

pub fn padding() -> Padding {
  Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3_5,
  }
}

pub fn primary_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Disabled => color::with_alpha(color::accent::PLASMA, 0.4),
    _ => color::accent::PLASMA,
  };
  button::Style {
    background: Some(Background::Color(background)),
    text_color: color::surface::BASE,
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

pub fn scrollbar(theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
  let mut base = scrollable::default(theme, status);
  let focused = matches!(
    status,
    scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
  );
  if focused {
    base.vertical_rail.scroller.background = Background::Color(color::accent::PLASMA);
    base.horizontal_rail.scroller.background = Background::Color(color::accent::PLASMA);
  }
  base
}
