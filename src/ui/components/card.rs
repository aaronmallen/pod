use iced::{
  Background, Border, Element, Length, Padding, Shadow, Vector,
  widget::{button, container},
};

use crate::ui::style::{color, control, radius};

const SELECTED_RING_ALPHA: f32 = 0.15;
/// Blur radius (logical px) of the selected glow ring. Iced has no shadow
/// spread, so a soft blur stands in for the design's crisp `0 0 0 3px` ring.
const SELECTED_RING_BLUR: f32 = 4.0;

pub fn card<'a, M>(content: impl Into<Element<'a, M>>) -> Element<'a, M>
where
  M: 'a,
{
  container(content).style(control::card).into()
}

pub fn card_padded<'a, M>(content: impl Into<Element<'a, M>>, padding: impl Into<Padding>) -> Element<'a, M>
where
  M: 'a,
{
  container(content).padding(padding).style(control::card).into()
}

pub fn selectable_card<'a, M>(content: impl Into<Element<'a, M>>, selected: bool, on_press: M) -> button::Button<'a, M>
where
  M: Clone + 'a,
{
  button(content).padding(0).on_press(on_press).style(move |_, status| {
    let border_color = if selected {
      color::accent::PLASMA
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
      color::rule_strong()
    } else {
      color::with_alpha(color::text::PRIMARY, 0.1)
    };
    let shadow = if selected {
      Shadow {
        color: color::with_alpha(color::accent::PLASMA, SELECTED_RING_ALPHA),
        offset: Vector::ZERO,
        blur_radius: SELECTED_RING_BLUR,
      }
    } else {
      Shadow::default()
    };

    button::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: radius::NAV_CARD.into(),
      },
      shadow,
      ..button::Style::default()
    }
  })
}

pub fn panel<'a, M>(content: impl Into<Element<'a, M>>, accent: bool) -> Element<'a, M>
where
  M: 'a,
{
  let border_color = if accent {
    color::with_alpha(color::accent::PLASMA, 0.30)
  } else {
    color::rule()
  };

  container(content)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use iced::widget::text;

  use super::*;

  mod card {
    use super::*;

    #[test]
    fn it_wraps_content_in_the_card_surface() {
      let _el: Element<'_, ()> = card(text("body"));
    }
  }

  mod card_padded {
    use super::*;

    #[test]
    fn it_wraps_content_with_padding() {
      let _el: Element<'_, ()> = card_padded(text("body"), control::padding());
    }
  }

  mod selectable_card {
    use super::*;

    #[test]
    fn it_builds_an_unselected_selectable_card() {
      let _el: Element<'_, ()> = selectable_card(text("body"), false, ()).into();
    }

    #[test]
    fn it_builds_a_selected_selectable_card() {
      let _el: Element<'_, ()> = selectable_card(text("body"), true, ()).into();
    }
  }

  mod panel {
    use super::*;

    #[test]
    fn it_wraps_content_in_a_plain_panel() {
      let _el: Element<'_, ()> = panel(text("body"), false);
    }

    #[test]
    fn it_wraps_content_in_an_accent_panel() {
      let _el: Element<'_, ()> = panel(text("body"), true);
    }
  }
}
