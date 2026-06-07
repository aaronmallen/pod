use iced::{Background, widget::button};

use crate::ui::style::color;

pub fn segment_button_style(active: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let background = if active {
    Some(color::with_alpha(color::accent::PLASMA, 0.12))
  } else if hovered {
    Some(color::with_alpha(color::text::PRIMARY, 0.04))
  } else {
    None
  };

  button::Style {
    background: background.map(Background::Color),
    text_color: if active {
      color::accent::PLASMA
    } else {
      color::text::PRIMARY
    },
    ..button::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod segment_button_style {
    use super::*;

    #[test]
    fn it_fills_the_active_segment() {
      let style = segment_button_style(true, button::Status::Active);

      assert!(style.background.is_some());
    }

    #[test]
    fn it_leaves_an_inactive_idle_segment_transparent() {
      let style = segment_button_style(false, button::Status::Active);

      assert!(style.background.is_none());
    }

    #[test]
    fn it_tints_an_inactive_hovered_segment() {
      let style = segment_button_style(false, button::Status::Hovered);

      assert!(style.background.is_some());
    }
  }
}
