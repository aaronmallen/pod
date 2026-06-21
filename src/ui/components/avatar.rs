use std::path::PathBuf;

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Radians,
  alignment::{Horizontal, Vertical},
  gradient::Linear,
  widget::{Stack, container, image, text},
};

use crate::ui::style::{color, typography};

const CORP_BADGE_RATIO: f32 = 0.5;
const GRADIENT_ANGLE: f32 = std::f32::consts::FRAC_PI_4 * 3.0;
const HUE_SHIFT: f32 = 40.0;
const HUE_STEP: i64 = 47;
const INITIALS_RATIO: f32 = 0.4;
const STATUS_DOT_INSET: f32 = 2.0;
const STATUS_DOT_SIZE: f32 = 8.0;

pub struct Avatar {
  border: Option<(Color, f32)>,
  corp_badge: Option<PathBuf>,
  height: f32,
  id: i64,
  name: String,
  portrait: Option<PathBuf>,
  radius: f32,
  status_dot: Option<Color>,
  width: Length,
}

impl Avatar {
  pub fn new(id: i64, name: impl Into<String>, width: Length, height: f32, portrait: Option<PathBuf>) -> Self {
    Self {
      border: None,
      corp_badge: None,
      height,
      id,
      name: name.into(),
      portrait,
      radius: 0.0,
      status_dot: None,
      width,
    }
  }

  pub fn border(mut self, color: Color, width: f32) -> Self {
    self.border = Some((color, width));
    self
  }

  /// Overlays a corporation logo as a corner badge, signalling that a market
  /// trade exists in both this character's and the corp's wallet.
  pub fn corp_badge(mut self, logo: Option<PathBuf>) -> Self {
    self.corp_badge = logo;
    self
  }

  pub fn radius(mut self, radius: f32) -> Self {
    self.radius = radius;
    self
  }

  #[allow(dead_code)] // component API surface; exercised only by unit tests
  pub fn status_dot(mut self, color: Color) -> Self {
    self.status_dot = Some(color);
    self
  }

  pub fn view<'a, M>(self) -> Element<'a, M>
  where
    M: 'a,
  {
    let inner = match self.portrait {
      Some(path) => portrait_image(path, self.width, self.height),
      None => tonal_placeholder(self.id, &self.name, self.width, self.height),
    };

    let (border_color, border_width) = self.border.unwrap_or((Color::TRANSPARENT, 0.0));
    let radius = self.radius;

    let framed: Element<'a, M> = container(inner)
      .clip(true)
      .style(move |_| container::Style {
        border: Border {
          color: border_color,
          width: border_width,
          radius: radius.into(),
        },
        ..container::Style::default()
      })
      .into();

    let mut layers: Vec<Element<'a, M>> = vec![framed];
    if let Some(logo) = self.corp_badge {
      layers.push(corp_badge_overlay(logo, self.height));
    }
    if let Some(dot) = self.status_dot {
      layers.push(
        container(super::status::dot_sized::<M>(dot, STATUS_DOT_SIZE))
          .width(Length::Fill)
          .height(Length::Fixed(self.height))
          .align_x(Horizontal::Right)
          .align_y(Vertical::Bottom)
          .padding(STATUS_DOT_INSET)
          .into(),
      );
    }

    if layers.len() == 1 {
      layers.pop().expect("the framed avatar layer is always present")
    } else {
      Stack::with_children(layers).into()
    }
  }
}

fn corp_badge_overlay<'a, M>(logo: PathBuf, height: f32) -> Element<'a, M>
where
  M: 'a,
{
  let badge = height * CORP_BADGE_RATIO;

  container(
    container(super::clip::clip_layer(
      image(image::Handle::from_path(logo))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
      Length::Fill,
      Length::Fill,
    ))
    .width(Length::Fixed(badge))
    .height(Length::Fixed(badge))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::surface::BASE,
        width: 1.0,
        radius: 2.0.into(),
      },
      ..container::Style::default()
    }),
  )
  .width(Length::Fill)
  .height(Length::Fixed(height))
  .align_x(Horizontal::Right)
  .align_y(Vertical::Bottom)
  .into()
}

pub fn avatar<'a, M>(id: i64, name: &str, width: Length, height: f32, portrait: Option<PathBuf>) -> Element<'a, M>
where
  M: 'a,
{
  match portrait {
    Some(path) => portrait_image(path, width, height),
    None => tonal_placeholder(id, name, width, height),
  }
}

fn hsl(hue: f32, saturation: f32, lightness: f32) -> Color {
  let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
  let hue_prime = hue.rem_euclid(360.0) / 60.0;
  let second = chroma * (1.0 - (hue_prime % 2.0 - 1.0).abs());
  let (r, g, b) = match hue_prime as u8 {
    0 => (chroma, second, 0.0),
    1 => (second, chroma, 0.0),
    2 => (0.0, chroma, second),
    3 => (0.0, second, chroma),
    4 => (second, 0.0, chroma),
    _ => (chroma, 0.0, second),
  };
  let lightness_match = lightness - chroma / 2.0;

  Color::from_rgb(r + lightness_match, g + lightness_match, b + lightness_match)
}

fn initials(name: &str) -> String {
  name
    .split_whitespace()
    .filter_map(|word| word.chars().next())
    .take(2)
    .collect::<String>()
    .to_uppercase()
}

fn portrait_image<'a, M>(path: PathBuf, width: Length, height: f32) -> Element<'a, M>
where
  M: 'a,
{
  container(super::clip::clip_layer(
    image(image::Handle::from_path(path))
      .width(Length::Fill)
      .height(Length::Fill)
      .content_fit(ContentFit::Cover),
    Length::Fill,
    Length::Fill,
  ))
  .width(width)
  .height(Length::Fixed(height))
  .clip(true)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn tonal_background(id: i64) -> Background {
  let hue = (id.rem_euclid(360) * HUE_STEP).rem_euclid(360) as f32;

  Background::Gradient(
    Linear::new(Radians(GRADIENT_ANGLE))
      .add_stop(0.0, hsl(hue, 0.35, 0.22))
      .add_stop(1.0, hsl((hue + HUE_SHIFT).rem_euclid(360.0), 0.30, 0.14))
      .into(),
  )
}

fn tonal_placeholder<'a, M>(id: i64, name: &str, width: Length, height: f32) -> Element<'a, M>
where
  M: 'a,
{
  let background = tonal_background(id);

  container(
    text(initials(name))
      .size(height * INITIALS_RATIO)
      .font(typography::body::REGULAR)
      .style(|_| text::Style {
        color: Some(color::text::dim()),
      }),
  )
  .width(width)
  .height(Length::Fixed(height))
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(background),
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod avatar_struct {
    use super::*;
    use crate::ui::style::color;

    const HEIGHT: f32 = 48.0;

    const PILOT_ID: i64 = 12_345_678;

    #[test]
    fn it_renders_a_plain_framed_avatar() {
      let _el: Element<'_, ()> = Avatar::new(PILOT_ID, "Test Pilot", Length::Fixed(HEIGHT), HEIGHT, None).view();
    }

    #[test]
    fn it_renders_with_a_border_and_radius() {
      let _el: Element<'_, ()> = Avatar::new(PILOT_ID, "Test Pilot", Length::Fixed(HEIGHT), HEIGHT, None)
        .border(color::accent::PLASMA, 1.0)
        .radius(8.0)
        .view();
    }

    #[test]
    fn it_renders_with_a_status_dot() {
      let _el: Element<'_, ()> = Avatar::new(PILOT_ID, "Test Pilot", Length::Fixed(HEIGHT), HEIGHT, None)
        .status_dot(color::status::ONLINE)
        .view();
    }

    #[test]
    fn it_renders_with_a_corp_badge() {
      let logo = Some(PathBuf::from("/tmp/corp-logo.png"));

      let _el: Element<'_, ()> = Avatar::new(PILOT_ID, "Test Pilot", Length::Fixed(HEIGHT), HEIGHT, None)
        .corp_badge(logo)
        .view();
    }

    #[test]
    fn it_renders_without_a_corp_badge_when_none() {
      let _el: Element<'_, ()> = Avatar::new(PILOT_ID, "Test Pilot", Length::Fixed(HEIGHT), HEIGHT, None)
        .corp_badge(None)
        .view();
    }
  }

  mod hsl {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_converts_primary_hues_to_rgb() {
      assert_eq!(hsl(0.0, 1.0, 0.5), Color::from_rgb(1.0, 0.0, 0.0));
      assert_eq!(hsl(120.0, 1.0, 0.5), Color::from_rgb(0.0, 1.0, 0.0));
      assert_eq!(hsl(240.0, 1.0, 0.5), Color::from_rgb(0.0, 0.0, 1.0));
    }

    #[test]
    fn it_returns_gray_at_zero_saturation() {
      assert_eq!(hsl(200.0, 0.0, 0.5), Color::from_rgb(0.5, 0.5, 0.5));
    }
  }

  mod initials {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_takes_the_first_letter_of_up_to_two_words_uppercased() {
      assert_eq!(initials("Test Pilot"), "TP");
      assert_eq!(initials("cmdr jane doe smith"), "CJ");
      assert_eq!(initials("Solo"), "S");
      assert_eq!(initials(""), "");
    }
  }

  mod render {
    use super::*;

    const HEIGHT: f32 = 140.0;

    const PILOT_ID: i64 = 12_345_678;

    #[test]
    fn it_renders_with_and_without_a_portrait() {
      let portrait = Some(PathBuf::from("/tmp/portrait.png"));

      let _with_portrait: Element<'_, ()> = avatar(PILOT_ID, "Test Pilot", Length::Fill, HEIGHT, portrait);
      let _no_portrait: Element<'_, ()> = avatar(PILOT_ID, "Test Pilot", Length::Fill, HEIGHT, None);
    }
  }
}
