//! Clone card component: header row plus implant slot grid.

use std::collections::HashMap;

use iced::{
  Background, Border, Color, Element, Length,
  widget::{Space, column, container},
};
use pod_model::CharacterClone;

use super::implant_slot_row;
use crate::{style::color, views::character_detail::Message};

/// Builder for a single clone card (active or jump clone).
pub struct Component<'a> {
  clone: &'a CharacterClone,
  icons: &'a HashMap<i32, iced::widget::image::Handle>,
  is_active: bool,
}

impl<'a> Component<'a> {
  /// Creates a new clone card for the given clone.
  pub fn new(clone: &'a CharacterClone, icons: &'a HashMap<i32, iced::widget::image::Handle>, is_active: bool) -> Self {
    Self {
      clone,
      icons,
      is_active,
    }
  }

  /// Renders the clone card.
  pub fn render(self) -> Element<'a, Message> {
    let header = card_header(self.clone, self.is_active);
    let cols = if self.is_active { 2 } else { 1 };
    let grid = implant_slot_row::slot_grid(&self.clone.implants, cols, self.icons);
    let border_color = if self.is_active {
      color::accent::PLASMA_MUTED
    } else {
      color::border::DEFAULT
    };
    let mut style = container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: border_color,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    };
    if self.is_active {
      style.shadow = iced::Shadow {
        blur_radius: 0.0,
        color: Color::TRANSPARENT,
        offset: iced::Vector::new(0.0, 0.0),
      };
    }
    container(column([header, grid]))
      .width(Length::Fill)
      .clip(true)
      .style(move |_| style)
      .into()
  }
}

fn card_right_label(clone: &CharacterClone, is_active: bool) -> String {
  if is_active {
    "ACTIVE".to_string()
  } else if clone.implants.is_empty() {
    "EMPTY".to_string()
  } else {
    format!("{} IMPLANTS", clone.implants.len())
  }
}

fn card_display_name(clone: &CharacterClone, is_active: bool) -> String {
  if is_active {
    clone.station_name.clone()
  } else {
    clone.name.clone().unwrap_or_else(|| clone.station_name.clone())
  }
}

fn card_right_color(is_active: bool) -> iced::Color {
  if is_active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  }
}

fn card_header_left_col<'a>(clone: &'a CharacterClone, is_active: bool) -> Element<'a, Message> {
  use iced::{Theme, widget::text};

  use crate::style::typography::{body, mono};

  let name_el = text(card_display_name(clone, is_active))
    .font(body::MEDIUM)
    .size(14.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });
  let location_el = text(clone.station_name.to_uppercase())
    .font(mono::REGULAR)
    .size(10.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });
  column([name_el.into(), Space::new().height(2.0).into(), location_el.into()]).into()
}

fn card_header<'a>(clone: &'a CharacterClone, is_active: bool) -> Element<'a, Message> {
  use iced::{
    Padding, Theme, alignment,
    widget::{row, text},
  };

  use crate::style::typography::mono;

  let right_color = card_right_color(is_active);
  let left_col = card_header_left_col(clone, is_active);
  let right_el = text(card_right_label(clone, is_active))
    .font(mono::REGULAR)
    .size(9.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(right_color),
    });
  let header_row = row([left_col, Space::new().width(Length::Fill).into(), right_el.into()])
    .align_y(alignment::Vertical::Center)
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: 16.0,
      right: 16.0,
    });
  container(header_row)
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}
