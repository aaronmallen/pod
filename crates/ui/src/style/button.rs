use iced::{Background, Border, Color, widget::button};

use super::color;

/// Button style for a sidebar tree-row list item.
///
/// Active: plasma-cyan subtle fill with a 2 px left-accent border.
/// Hover/pressed: faint warm tint. Rest: transparent.
pub fn list_item_active(is_active: bool, status: button::Status) -> button::Style {
  button::Style {
    background: if is_active {
      Some(Background::Color(color::accent::PLASMA_SUBTLE))
    } else {
      match status {
        button::Status::Hovered | button::Status::Pressed => {
          Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.04)))
        }
        _ => None,
      }
    },
    border: Border {
      color: if is_active {
        color::accent::PLASMA
      } else {
        Color::TRANSPARENT
      },
      width: if is_active { 2.0 } else { 0.0 },
      radius: 0.0.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}
