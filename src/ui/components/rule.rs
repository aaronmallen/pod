use iced::{
  Background, Element, Length,
  widget::{Space, container},
};

use crate::ui::style::color;

const HAIRLINE: f32 = 1.0;
const RULE_ALPHA: f32 = 0.1;

pub fn horizontal<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  horizontal_alpha(RULE_ALPHA)
}

pub fn horizontal_alpha<'a, M>(alpha: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new().width(Length::Fill).height(Length::Fixed(HAIRLINE)))
    .width(Length::Fill)
    .height(Length::Fixed(HAIRLINE))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, alpha))),
      ..container::Style::default()
    })
    .into()
}

pub fn vertical<'a, M>(height: f32) -> Element<'a, M>
where
  M: 'a,
{
  vertical_alpha(height, RULE_ALPHA)
}

pub fn vertical_alpha<'a, M>(height: f32, alpha: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(
    Space::new()
      .width(Length::Fixed(HAIRLINE))
      .height(Length::Fixed(height)),
  )
  .width(Length::Fixed(HAIRLINE))
  .height(Length::Fixed(height))
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, alpha))),
    ..container::Style::default()
  })
  .into()
}

pub fn vertical_fill<'a, M>(alpha: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new().width(Length::Fixed(HAIRLINE)).height(Length::Fill))
    .width(Length::Fixed(HAIRLINE))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, alpha))),
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug)]
  enum Message {}

  mod horizontal {
    use super::*;

    #[test]
    fn it_builds_a_horizontal_hairline() {
      let _el: Element<'_, Message> = horizontal();
    }
  }

  mod horizontal_alpha {
    use super::*;

    #[test]
    fn it_builds_a_horizontal_hairline_at_a_custom_alpha() {
      let _el: Element<'_, Message> = horizontal_alpha(0.06);
    }
  }

  mod vertical {
    use super::*;

    #[test]
    fn it_builds_a_vertical_hairline_of_the_given_height() {
      let _el: Element<'_, Message> = vertical(44.0);
    }
  }

  mod vertical_alpha {
    use super::*;

    #[test]
    fn it_builds_a_vertical_hairline_at_a_custom_alpha() {
      let _el: Element<'_, Message> = vertical_alpha(44.0, 0.08);
    }
  }

  mod vertical_fill {
    use super::*;

    #[test]
    fn it_builds_a_fill_height_vertical_hairline() {
      let _el: Element<'_, Message> = vertical_fill(0.1);
    }
  }
}
