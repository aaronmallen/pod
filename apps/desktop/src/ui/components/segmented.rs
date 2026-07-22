use iced::{
  Background, Element, Padding,
  widget::{button, text},
};

use crate::ui::style::{color, typography};

pub fn segment_button<'a, M>(label: impl Into<String>, active: bool, padding: Padding, on_press: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let fill = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  button(
    text(label.into())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(fill)),
  )
  .padding(padding)
  .on_press(on_press)
  .style(move |_, status| segment_button_style(active, status))
  .into()
}

pub fn segment_button_style(active: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let background = if active {
    Some(color::with_alpha(color::accent(), 0.12))
  } else if hovered {
    Some(color::with_alpha(color::text::PRIMARY, 0.04))
  } else {
    None
  };

  button::Style {
    background: background.map(Background::Color),
    text_color: if active { color::accent() } else { color::text::PRIMARY },
    ..button::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod segment_button {
    use super::*;

    #[test]
    fn it_renders_an_active_segment() {
      let _el: Element<'_, ()> = segment_button("All", true, Padding::ZERO, ());
    }

    #[test]
    fn it_renders_an_inactive_segment() {
      let _el: Element<'_, ()> = segment_button("Out", false, Padding::ZERO, ());
    }
  }

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
