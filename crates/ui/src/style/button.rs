use iced::{Background, Border, Color, widget::button};

use super::color;

fn list_item_bg(is_active: bool, status: button::Status) -> Option<Background> {
  if is_active {
    Some(Background::Color(color::accent::PLASMA_SUBTLE))
  } else {
    list_item_hover_bg(status)
  }
}

fn list_item_hover_bg(status: button::Status) -> Option<Background> {
  match status {
    button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
    _ => None,
  }
}

fn list_item_border(is_active: bool) -> Border {
  Border {
    color: if is_active {
      color::accent::PLASMA
    } else {
      Color::TRANSPARENT
    },
    width: if is_active { 2.0 } else { 0.0 },
    radius: 0.0.into(),
  }
}

/// Button style for a sidebar tree-row list item.
///
/// Active: plasma-cyan subtle fill with a 2 px left-accent border.
/// Hover/pressed: faint warm tint. Rest: transparent.
pub fn list_item_active(is_active: bool, status: button::Status) -> button::Style {
  button::Style {
    background: list_item_bg(is_active, status),
    border: list_item_border(is_active),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}
