use iced::{
  Background, Border, Color, Element,
  alignment::Vertical,
  widget::{Row, button, container, text},
};

use crate::ui::style::{color, spacing, typography};

const CHIP_RADIUS: f32 = 999.0;
const HAIRLINE: f32 = 1.0;
const REMOVE_GLYPH: &str = "\u{00D7}";

pub struct Chip<'a, M> {
  color: Option<Color>,
  label: String,
  on_press: Option<M>,
  on_remove: Option<M>,
  selected: bool,
  _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, M> Chip<'a, M>
where
  M: Clone + 'a,
{
  pub fn new(label: impl Into<String>, color: Option<Color>) -> Self {
    Self {
      color,
      label: label.into(),
      on_press: None,
      on_remove: None,
      selected: false,
      _marker: std::marker::PhantomData,
    }
  }

  #[allow(dead_code)]
  pub fn on_press(mut self, message: M) -> Self {
    self.on_press = Some(message);
    self
  }

  pub fn on_remove(mut self, message: M) -> Self {
    self.on_remove = Some(message);
    self
  }

  #[allow(dead_code)]
  pub fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  pub fn view(self) -> Element<'a, M> {
    let base = self.color.unwrap_or(color::text::SECONDARY);
    let fill = if self.selected { color::accent::PLASMA } else { base };
    let background = if self.selected {
      color::with_alpha(color::accent::PLASMA, 0.12)
    } else {
      self.color.map_or_else(
        || color::with_alpha(color::text::PRIMARY, 0.06),
        |c| color::with_alpha(c, 0.12),
      )
    };
    let border = if self.selected {
      color::with_alpha(color::accent::PLASMA, 0.45)
    } else {
      self.color.map_or_else(
        || color::with_alpha(color::text::PRIMARY, 0.1),
        |c| color::with_alpha(c, 0.45),
      )
    };

    let label = text(self.label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(fill),
      });

    let mut children: Vec<Element<'a, M>> = vec![label.into()];
    if let Some(message) = self.on_remove {
      children.push(remove_affordance(fill, message));
    }

    let body = container(
      Row::with_children(children)
        .spacing(spacing::UNIT)
        .align_y(Vertical::Center),
    )
    .padding([spacing::UNIT / 2.0, spacing::SPACE_2])
    .style(move |_| container::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border,
        width: HAIRLINE,
        radius: CHIP_RADIUS.into(),
      },
      ..container::Style::default()
    });

    match self.on_press {
      None => body.into(),
      Some(message) => button(body)
        .padding(0)
        .on_press(message)
        .style(|_, _| button::Style {
          background: Some(Background::Color(Color::TRANSPARENT)),
          ..button::Style::default()
        })
        .into(),
    }
  }
}

pub fn chip<'a, M>(label: impl Into<String>, color: Option<Color>) -> Element<'a, M>
where
  M: Clone + 'a,
{
  Chip::new(label, color).view()
}

fn remove_affordance<'a, M>(fill: Color, message: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(
    text(REMOVE_GLYPH)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(fill),
      }),
  )
  .padding(0)
  .on_press(message)
  .style(remove_button)
  .into()
}

fn remove_button(_theme: &iced::Theme, _status: button::Status) -> button::Style {
  button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    ..button::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod chip {
    use super::*;

    #[test]
    fn it_renders_a_plain_pill() {
      let _plain: Element<'_, ()> = chip("Capsuleer", None);
    }

    #[test]
    fn it_renders_with_a_color() {
      let _colored: Element<'_, ()> = chip("Hostile", Some(color::text::PRIMARY));
    }
  }

  mod chip_struct {
    use super::*;

    #[test]
    fn it_renders_a_removable_variant() {
      let _removable: Element<'_, i32> = Chip::new("Wormhole", None).on_remove(7).view();
    }

    #[test]
    fn it_renders_a_colored_removable_variant() {
      let _removable: Element<'_, i32> = Chip::new("Nullsec", Some(color::text::SECONDARY)).on_remove(1).view();
    }

    #[test]
    fn it_renders_a_pressable_variant() {
      let _pressable: Element<'_, i32> = Chip::new("Highsec", None).on_press(2).view();
    }

    #[test]
    fn it_renders_a_selected_variant() {
      let _selected: Element<'_, i32> = Chip::new("Lowsec", None).selected(true).on_press(3).view();
    }
  }
}
