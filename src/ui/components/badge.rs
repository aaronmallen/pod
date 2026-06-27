use iced::{
  Background, Border, Color, Element, Padding,
  widget::{container, text},
};

use crate::ui::style::{color, radius, typography};

const BADGE_RADIUS: f32 = radius::SUBTLE;
const BG_ALPHA: f32 = 0.12;
const BORDER_ALPHA: f32 = 0.45;
const HAIRLINE: f32 = 1.0;

pub struct Badge<'a, M> {
  bordered: bool,
  color: Option<Color>,
  label: String,
  _marker: std::marker::PhantomData<&'a M>,
}

impl<'a, M> Badge<'a, M>
where
  M: 'a,
{
  pub fn new(label: impl Into<String>, color: Option<Color>) -> Self {
    Self {
      bordered: true,
      color,
      label: label.into(),
      _marker: std::marker::PhantomData,
    }
  }

  #[cfg(test)]
  pub fn bordered(mut self, bordered: bool) -> Self {
    self.bordered = bordered;
    self
  }

  pub fn view(self) -> Element<'a, M> {
    let fill = self.color.unwrap_or(color::text::secondary());
    let background = self.color.map_or_else(
      || color::with_alpha(color::text::PRIMARY, 0.06),
      |c| color::with_alpha(c, BG_ALPHA),
    );
    let border = if self.bordered {
      Border {
        color: self.color.map_or_else(
          || color::with_alpha(color::text::PRIMARY, 0.12),
          |c| color::with_alpha(c, BORDER_ALPHA),
        ),
        width: HAIRLINE,
        radius: BADGE_RADIUS.into(),
      }
    } else {
      Border {
        radius: BADGE_RADIUS.into(),
        ..Border::default()
      }
    };

    container(
      text(self.label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(move |_| text::Style {
          color: Some(fill),
        }),
    )
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(background)),
      border,
      ..container::Style::default()
    })
    .into()
  }
}

pub fn badge<'a, M>(label: impl Into<String>, color: Option<Color>) -> Element<'a, M>
where
  M: 'a,
{
  Badge::new(label, color).view()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod badge {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_renders_a_plain_badge() {
      let _el: Element<'_, ()> = badge("T2", None);
    }

    #[test]
    fn it_renders_with_a_color() {
      let _el: Element<'_, ()> = badge("Unstable", Some(color::status::DANGER));
    }
  }

  mod badge_struct {
    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_renders_without_a_border() {
      let _el: Element<'_, ()> = Badge::new("Primary", Some(color::accent::PLASMA))
        .bordered(false)
        .view();
    }
  }
}
