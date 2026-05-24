use iced::{
  Background, Border, Color, ContentFit, Element, Gradient, Length, Radians,
  alignment::{Horizontal, Vertical},
  gradient,
  widget::{container, image, text},
};

use crate::style::{color, typography as font};

/// The visual style of an avatar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarKind {
  /// Circular portrait used for individual characters.
  #[default]
  Person,
  /// Rounded-square portrait used for corporations.
  Corp,
  /// Gray box showing "SYS" — used for system/automated senders.
  System,
}

/// Builder for an avatar widget.
pub struct Component {
  name: String,
  tone: u16,
  size: f32,
  kind: AvatarKind,
  portrait_handle: Option<image::Handle>,
}

impl Component {
  pub fn new(name: impl Into<String>, tone: u16, size: f32, kind: AvatarKind) -> Self {
    Self {
      name: name.into(),
      tone,
      size,
      kind,
      portrait_handle: None,
    }
  }

  pub fn portrait(mut self, handle: Option<image::Handle>) -> Self {
    self.portrait_handle = handle;
    self
  }

  pub fn render<'a, MSG: 'a>(self) -> Element<'a, MSG> {
    if let Some(handle) = self.portrait_handle {
      let radius = match self.kind {
        AvatarKind::Person => self.size / 2.0,
        AvatarKind::Corp => self.size * 0.12,
        AvatarKind::System => self.size * 0.12,
      };
      return portrait_image(handle, self.size, radius);
    }
    match self.kind {
      AvatarKind::System => system_avatar(self.size),
      AvatarKind::Person => gradient_avatar(&self.name, self.tone, self.size, self.size / 2.0),
      AvatarKind::Corp => gradient_avatar(&self.name, self.tone, self.size, self.size * 0.12),
    }
  }
}

fn portrait_image<'a, MSG: 'a>(handle: image::Handle, size: f32, radius: f32) -> Element<'a, MSG> {
  container(
    image(handle)
      .width(Length::Fill)
      .height(Length::Fill)
      .content_fit(ContentFit::Cover),
  )
  .width(Length::Fixed(size))
  .height(Length::Fixed(size))
  .clip(true)
  .style(move |_| container::Style {
    border: Border {
      radius: radius.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn system_avatar<'a, MSG: 'a>(size: f32) -> Element<'a, MSG> {
  container(
    text("SYS")
      .font(font::mono::REGULAR)
      .size(size * 0.34)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fixed(size))
  .height(Length::Fixed(size))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::state::SUBTLE_FILL)),
    border: Border {
      color: color::border::SUBTLE,
      radius: (size * 0.12).into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn gradient_avatar<'a, MSG: 'a>(name: &str, tone: u16, size: f32, radius: f32) -> Element<'a, MSG> {
  let initials = extract_initials(name);
  let c0 = hsl_to_color(tone, 0.30, 0.22);
  let c1 = hsl_to_color(tone, 0.20, 0.14);

  let angle = Radians(std::f32::consts::FRAC_PI_4 * 3.0);
  let grad = gradient::Linear::new(angle).add_stop(0.0, c0).add_stop(1.0, c1);

  container(
    text(initials)
      .font(font::body::MEDIUM)
      .size(size * 0.40)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::MEDIUM),
      }),
  )
  .width(Length::Fixed(size))
  .height(Length::Fixed(size))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Gradient(Gradient::Linear(grad))),
    border: Border {
      radius: radius.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn extract_initials(name: &str) -> String {
  name
    .split_whitespace()
    .filter_map(|w| w.chars().next())
    .take(2)
    .collect::<String>()
    .to_uppercase()
}

fn hsl_to_color(hue: u16, saturation: f32, lightness: f32) -> Color {
  let h = hue as f32 / 360.0;
  let s = saturation;
  let l = lightness;

  if s == 0.0 {
    return Color::from_rgb(l, l, l);
  }

  let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
  let p = 2.0 * l - q;

  let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
  let g = hue_to_rgb(p, q, h);
  let b = hue_to_rgb(p, q, h - 1.0 / 3.0);

  Color::from_rgb(r, g, b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
  if t < 0.0 {
    t += 1.0;
  }
  if t > 1.0 {
    t -= 1.0;
  }
  if t < 1.0 / 6.0 {
    return p + (q - p) * 6.0 * t;
  }
  if t < 1.0 / 2.0 {
    return q;
  }
  if t < 2.0 / 3.0 {
    return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
  }
  p
}
