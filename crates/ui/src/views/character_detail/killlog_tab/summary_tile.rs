//! Summary tile component: stat display for kills, losses, ISK, and efficiency.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, text},
};

use crate::{
  style::{color, typography::mono},
  views::character_detail::Message,
};

/// Builder for the kill log summary tiles row.
pub struct Component {
  efficiency: (f64, f64),
  kills: (usize, f64),
  losses: (usize, f64),
}

impl Component {
  /// Creates a new summary tile row from kill and loss counts with ISK values.
  pub fn new(kill_count: usize, kill_isk: f64, loss_count: usize, loss_isk: f64) -> Self {
    Self {
      efficiency: (kill_isk, kill_isk + loss_isk),
      kills: (kill_count, kill_isk),
      losses: (loss_count, loss_isk),
    }
  }

  /// Renders the summary tiles row.
  pub fn render<'a>(self) -> Element<'a, Message> {
    let (kill_isk, total_isk) = self.efficiency;
    let eff_label = efficiency_label(kill_isk, total_isk);
    let eff_color = efficiency_color(kill_isk, total_isk);
    row([
      summary_tile("Kills", self.kills.0.to_string(), color::status::ONLINE),
      summary_tile("Losses", self.losses.0.to_string(), color::status::DANGER),
      summary_tile(
        "ISK Destroyed",
        format!("{} ISK", crate::format::fmt_isk(kill_isk)),
        color::status::ONLINE,
      ),
      summary_tile("Efficiency", eff_label, eff_color),
    ])
    .spacing(12.0)
    .width(Length::Fill)
    .into()
  }
}

fn efficiency_color(kill_isk: f64, total_isk: f64) -> Color {
  if total_isk <= 0.0 {
    color::text::SECONDARY
  } else if kill_isk / total_isk * 100.0 >= 50.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  }
}

fn efficiency_label(kill_isk: f64, total_isk: f64) -> String {
  if total_isk <= 0.0 {
    "\u{2014}".to_string()
  } else {
    format!("{:.1}%", kill_isk / total_isk * 100.0)
  }
}

fn summary_tile(label: &str, value: String, accent: Color) -> Element<'static, Message> {
  let label_el = text(label.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });
  let value_el = text(value)
    .font(mono::MEDIUM)
    .size(22.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(accent),
    });
  container(
    column([label_el.into(), Space::new().height(6.0).into(), value_el.into()]).padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::DEFAULT,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}
