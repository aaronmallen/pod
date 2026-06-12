use iced::{Background, Border, Element, Length, Padding, widget::container};

use crate::ui::style::{color, control, radius};

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
