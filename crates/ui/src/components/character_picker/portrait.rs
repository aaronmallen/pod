use iced::{
  Background, Border, Color, Element, Length, gradient,
  widget::{container, image, text},
};

use super::{CharacterEntry, CorporationEntry, PickerSelection};
use crate::style::{color, typography as font};

pub fn portrait_image_swatch<MSG: 'static>(handle: image::Handle, size: f32, radius: f32) -> Element<'static, MSG> {
  container(
    image(handle)
      .width(Length::Fixed(size))
      .height(Length::Fixed(size))
      .content_fit(iced::ContentFit::Cover),
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

pub fn initials_swatch<MSG: 'static>(name: &str, tone: u16, size: f32, radius: f32) -> Element<'static, MSG> {
  let initials = name
    .split_whitespace()
    .filter_map(|w| w.chars().next())
    .take(2)
    .map(|c| c.to_uppercase().next().unwrap_or(c))
    .collect::<String>();
  let h = tone as f32 / 360.0;
  let (r0, g0, b0) = hsl_to_rgb(h, 0.28, 0.28);
  let (r1, g1, b1) = hsl_to_rgb(h, 0.18, 0.16);
  let grad = gradient::Linear::new(std::f32::consts::PI * 0.75)
    .add_stop(0.0, Color::from_rgb(r0, g0, b0))
    .add_stop(1.0, Color::from_rgb(r1, g1, b1));
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
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(iced::alignment::Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Gradient(grad.into())),
    border: Border {
      radius: radius.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

pub fn portrait_swatch<MSG: 'static>(
  name: &str,
  tone: u16,
  size: f32,
  radius: f32,
  portrait_handle: Option<image::Handle>,
) -> Element<'static, MSG> {
  if let Some(handle) = portrait_handle {
    return portrait_image_swatch(handle, size, radius);
  }
  initials_swatch(name, tone, size, radius)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
  if s == 0.0 {
    return (l, l, l);
  }
  let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
  let p = 2.0 * l - q;
  (
    hue_to_channel(p, q, h + 1.0 / 3.0),
    hue_to_channel(p, q, h),
    hue_to_channel(p, q, h - 1.0 / 3.0),
  )
}

pub fn hue_to_channel(p: f32, q: f32, t: f32) -> f32 {
  let t = clamp_hue_t(t);
  hue_channel_value(p, q, t)
}

fn clamp_hue_t(mut t: f32) -> f32 {
  if t < 0.0 {
    t += 1.0;
  }
  if t > 1.0 {
    t -= 1.0;
  }
  t
}

fn hue_channel_value(p: f32, q: f32, t: f32) -> f32 {
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

pub fn trigger_label_col(name: String, subtitle: String) -> Element<'static, super::Message> {
  use iced::widget::column;

  column([
    text(name)
      .font(font::body::MEDIUM)
      .size(17.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(subtitle.to_uppercase())
      .font(font::mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .spacing(3.0)
  .into()
}

pub fn display_for_all(entries: &[CharacterEntry]) -> (String, String, u16, Option<image::Handle>) {
  let e = entries.iter().find(|e| e.id.is_none()).or_else(|| entries.first());
  e.map(|e| (e.name.clone(), e.corp_name.clone(), e.tone, e.portrait_handle.clone()))
    .unwrap_or_else(|| ("—".to_string(), String::new(), 220, None))
}

pub fn display_for_character(entries: &[CharacterEntry], id: i64) -> (String, String, u16, Option<image::Handle>) {
  let e = entries.iter().find(|e| e.id == Some(id)).or_else(|| entries.first());
  e.map(|e| (e.name.clone(), e.corp_name.clone(), e.tone, e.portrait_handle.clone()))
    .unwrap_or_else(|| ("—".to_string(), String::new(), 220, None))
}

pub fn display_for_corporation(
  corp_entries: &[CorporationEntry],
  id: i64,
) -> (String, String, u16, Option<image::Handle>) {
  let e = corp_entries.iter().find(|e| e.id == id);
  e.map(|e| (e.name.clone(), e.ticker.clone(), 220, e.icon_handle.clone()))
    .unwrap_or_else(|| ("—".to_string(), String::new(), 220, None))
}

pub fn selected_display(
  entries: &[CharacterEntry],
  corp_entries: &[CorporationEntry],
  selected: &PickerSelection,
) -> (String, String, u16, Option<image::Handle>) {
  match selected {
    PickerSelection::All => display_for_all(entries),
    PickerSelection::Character(id) => display_for_character(entries, *id),
    PickerSelection::Corporation(id) => display_for_corporation(corp_entries, *id),
  }
}
