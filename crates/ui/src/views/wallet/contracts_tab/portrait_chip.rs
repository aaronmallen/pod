//! Portrait chip — inline portrait image or initials avatar chip.

use iced::{
  Background, Border, Color, ContentFit, Element, Theme,
  widget::{container, image, text},
};

use super::Message;
use crate::style::{color, typography::mono};

fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
  let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
  let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
  let m = l - c / 2.0;
  let (r, g, b) = hsl_rgb_components(h, c, x);
  Color::from_rgb(r + m, g + m, b + m)
}

fn hsl_rgb_components(h: f32, c: f32, x: f32) -> (f32, f32, f32) {
  if h < 120.0 {
    hsl_rgb_low(h, c, x)
  } else if h < 240.0 {
    hsl_rgb_mid(h, c, x)
  } else if h < 300.0 {
    (x, 0.0, c)
  } else {
    (c, 0.0, x)
  }
}

fn hsl_rgb_low(h: f32, c: f32, x: f32) -> (f32, f32, f32) {
  if h < 60.0 { (c, x, 0.0) } else { (x, c, 0.0) }
}

fn hsl_rgb_mid(h: f32, c: f32, x: f32) -> (f32, f32, f32) {
  if h < 180.0 { (0.0, c, x) } else { (0.0, x, c) }
}

fn first_upper(word: &str) -> String {
  word
    .chars()
    .next()
    .map(|c| c.to_uppercase().to_string())
    .unwrap_or_default()
}

fn char_initials(name: &str) -> String {
  let words: Vec<&str> = name.split_whitespace().collect();
  match words.as_slice() {
    [] => String::new(),
    [only] => first_upper(only),
    [first, .., last] => format!("{}{}", first_upper(first), first_upper(last)),
  }
}

/// Builder for an inline portrait image or initials avatar chip.
pub struct Component<'a> {
  handle: Option<&'a image::Handle>,
  name: String,
  tone: u16,
}

impl<'a> Component<'a> {
  /// Creates a new portrait chip component.
  pub fn new(name: impl Into<String>, tone: u16) -> Self {
    Self {
      handle: None,
      name: name.into(),
      tone,
    }
  }

  /// Sets the portrait image handle.
  pub fn handle(mut self, handle: Option<&'a image::Handle>) -> Self {
    self.handle = handle;
    self
  }

  /// Renders the portrait chip into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    if let Some(h) = self.handle {
      return container(
        image::Image::new(h.clone())
          .width(18.0)
          .height(18.0)
          .content_fit(ContentFit::Cover),
      )
      .width(18.0)
      .height(18.0)
      .clip(true)
      .style(|_| container::Style {
        border: Border {
          radius: 4.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into();
    }
    let hue = self.tone as f32;
    let l = 0.25 + (hue / 360.0) * 0.15;
    let bg = hsl_to_color(hue, 0.35, l);
    let initials = char_initials(&self.name);
    container(
      text(initials)
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::MEDIUM),
        }),
    )
    .width(18.0)
    .height(18.0)
    .center_x(18.0)
    .center_y(18.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        radius: 4.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  }
}
