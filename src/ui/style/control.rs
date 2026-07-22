use iced::{
  Background, Border, Color, Padding,
  widget::{container, scrollable, slider},
};

use crate::ui::style::{color, radius, spacing};

pub fn bordered_pane(_theme: &iced::Theme) -> container::Style {
  container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  }
}

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

pub fn padding() -> Padding {
  Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3_5,
  }
}

pub fn scrollbar(theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
  let mut base = scrollable::default(theme, status);
  let focused = matches!(
    status,
    scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
  );
  if focused {
    base.vertical_rail.scroller.background = Background::Color(color::accent());
    base.horizontal_rail.scroller.background = Background::Color(color::accent());
  }
  base
}

pub fn slider_track(_theme: &iced::Theme, _status: slider::Status) -> slider::Style {
  slider::Style {
    rail: slider::Rail {
      backgrounds: (
        Background::Color(color::accent()),
        Background::Color(color::with_alpha(color::text::PRIMARY, 0.12)),
      ),
      width: 6.0,
      border: Border {
        radius: radius::SUBTLE.into(),
        width: 0.0,
        color: Color::TRANSPARENT,
      },
    },
    handle: slider::Handle {
      shape: slider::HandleShape::Circle {
        radius: 10.0,
      },
      background: Background::Color(color::accent()),
      border_color: color::surface::BASE,
      border_width: 3.0,
    },
  }
}

pub fn sunken_pane(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod slider_track {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_a_circular_handle_over_a_neutral_rail() {
      let style = slider_track(&iced::Theme::Dark, slider::Status::Active);

      assert!(matches!(style.handle.shape, slider::HandleShape::Circle { .. }));
      assert_eq!(style.rail.backgrounds.0, Background::Color(color::accent()));
      assert_eq!(
        style.rail.backgrounds.1,
        Background::Color(color::with_alpha(color::text::PRIMARY, 0.12))
      );
    }
  }
}
