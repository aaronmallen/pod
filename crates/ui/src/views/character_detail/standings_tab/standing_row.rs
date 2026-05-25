//! Row component for a single standing entry.

use iced::{
  Border, Color, Element, Length, Padding, Theme,
  widget::{container, row, text},
};
use pod_model::CharacterStanding;

use super::standing_bar::StandingBar;
use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::Message,
};

/// Builder for a standing row in the standings table.
pub struct StandingRow<'a> {
  is_last: bool,
  standing: &'a CharacterStanding,
}

impl<'a> StandingRow<'a> {
  /// Creates a new standing row for the given standing entry.
  pub fn new(standing: &'a CharacterStanding, is_last: bool) -> Self {
    Self {
      is_last,
      standing,
    }
  }

  /// Renders the standing row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    standing_row(self.standing, self.is_last)
  }
}

fn standing_color(v: f64) -> Color {
  if v >= 5.0 {
    color::status::ONLINE
  } else if v > 0.0 {
    color::status::ONLINE_STRONG
  } else if v >= -0.01 {
    color::text::SECONDARY
  } else if v > -5.0 {
    color::status::DANGER_STRONG
  } else {
    color::status::DANGER
  }
}

fn standing_eff_col<'a>(v: f64, effective_color: Color) -> Element<'a, Message> {
  let eff_label = format!("{}{:.2}", if v >= 0.0 { "+" } else { "" }, v);
  container(
    text(eff_label)
      .font(mono::MEDIUM)
      .size(14.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(effective_color),
      }),
  )
  .width(60.0)
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn standing_name_col<'a>(standing: &'a CharacterStanding) -> Element<'a, Message> {
  text(standing.from_name.clone())
    .font(body::REGULAR)
    .size(13.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .width(Length::Fill)
    .into()
}

fn standing_raw_col<'a>(standing: &'a CharacterStanding) -> Element<'a, Message> {
  let raw_label = format!(
    "{}{:.2} raw",
    if standing.standing >= 0.0 { "+" } else { "" },
    standing.standing
  );
  container(
    text(raw_label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(90.0)
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn standing_row<'a>(standing: &'a CharacterStanding, is_last: bool) -> Element<'a, Message> {
  let v = standing.standing;
  let effective_color = standing_color(v);
  let inner = row([
    standing_name_col(standing),
    standing_raw_col(standing),
    standing_eff_col(v, effective_color),
    StandingBar::new(v).render(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .spacing(16.0)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 16.0,
    right: 16.0,
  });
  container(inner)
    .width(Length::Fill)
    .style(move |_| container::Style {
      border: Border {
        color: if is_last {
          Color::TRANSPARENT
        } else {
          color::border::SUBTLE
        },
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}
