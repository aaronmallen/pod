use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{column, container, image, stack, text},
};
use pod_model::Character;

use crate::style::{color, radius, spacing, typography};

pub struct Component<'a> {
  character: &'a Character,
  portrait_handle: Option<&'a image::Handle>,
}

impl<'a> Component<'a> {
  pub fn new(character: &'a Character) -> Self {
    Self {
      character,
      portrait_handle: None,
    }
  }

  pub fn portrait_handle(mut self, handle: Option<&'a image::Handle>) -> Self {
    self.portrait_handle = handle;
    self
  }

  pub fn render<MSG: 'static>(self) -> Element<'a, MSG> {
    let hue = *self.character.portrait_tone() as f32;
    let portrait_layer = portrait_content(self.character, hue, self.portrait_handle);

    let status_label = match *self.character.location_docked() {
      Some(true) => Some("Docked"),
      Some(false) => Some("In space"),
      None => None,
    };

    if let Some(label) = status_label {
      let pill = status_pill(label);
      let pill_overlay = container(pill)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Top)
        .padding(Padding::new(spacing::SPACE_1));

      stack(vec![portrait_layer, pill_overlay.into()])
        .width(Length::Fill)
        .height(spacing::layout::CHARACTER_PORTRAIT_HEIGHT)
        .into()
    } else {
      portrait_layer
    }
  }
}

fn status_pill<'a, MSG: 'a>(label: &'a str) -> Element<'a, MSG> {
  container(text(label).font(typography::mono::REGULAR).size(9.0))
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      border: Border {
        radius: radius::CHIP.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn portrait_content<'a, MSG: 'static>(
  character: &'a Character,
  hue: f32,
  portrait_handle: Option<&'a image::Handle>,
) -> Element<'a, MSG> {
  if let Some(handle) = portrait_handle {
    container(
      image(handle.clone())
        .width(Length::Fill)
        .height(spacing::layout::CHARACTER_PORTRAIT_HEIGHT)
        .content_fit(iced::ContentFit::Cover),
    )
    .width(Length::Fill)
    .height(spacing::layout::CHARACTER_PORTRAIT_HEIGHT)
    .style(|_| container::Style {
      border: Border {
        radius: iced::border::top(radius::PANEL),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  } else {
    portrait_placeholder(character, hue)
  }
}

fn portrait_placeholder<'a, MSG: 'static>(character: &'a Character, hue: f32) -> Element<'a, MSG> {
  let l = 0.25 + (hue / 360.0) * 0.15;
  let bg_color = hsl_to_color(hue, 0.4, l);
  let initials = character_initials(character.name());

  container(
    column([text(initials)
      .font(typography::body::MEDIUM)
      .size(56.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::with_alpha(color::surface::SUNKEN, 0.32)),
      })
      .into()])
    .align_x(iced::alignment::Horizontal::Center),
  )
  .width(Length::Fill)
  .height(spacing::layout::CHARACTER_PORTRAIT_HEIGHT)
  .center_x(Length::Fill)
  .center_y(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_color)),
    border: Border {
      radius: iced::border::top(radius::PANEL),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn character_initials(name: &str) -> String {
  let words: Vec<&str> = name.split_whitespace().collect();
  match words.as_slice() {
    [] => String::new(),
    [only] => only
      .chars()
      .next()
      .map(|c| c.to_uppercase().to_string())
      .unwrap_or_default(),
    [first, .., last] => {
      let f = first
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
      let l = last
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
      format!("{f}{l}")
    }
  }
}

fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
  let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
  let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
  let m = l - c / 2.0;
  let (r, g, b) = if h < 60.0 {
    (c, x, 0.0)
  } else if h < 120.0 {
    (x, c, 0.0)
  } else if h < 180.0 {
    (0.0, c, x)
  } else if h < 240.0 {
    (0.0, x, c)
  } else if h < 300.0 {
    (x, 0.0, c)
  } else {
    (c, 0.0, x)
  };
  Color::from_rgb(r + m, g + m, b + m)
}
