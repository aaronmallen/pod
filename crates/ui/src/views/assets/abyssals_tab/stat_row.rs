//! Stat row component for the abyssals tab card view.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, text},
};
use pod_model::AbyssalStatViewModel;

use super::{Message, format_stat_value};
use crate::style::{
  color,
  typography::{body, mono},
};

/// Builder for a single stat row in an abyssal module card.
pub struct Component<'a> {
  stat: &'a AbyssalStatViewModel,
}

impl<'a> Component<'a> {
  /// Creates a new stat row for the given abyssal stat view model.
  pub fn new(stat: &'a AbyssalStatViewModel) -> Self {
    Self {
      stat,
    }
  }

  /// Renders the stat row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let stat = self.stat;
    let delta = stat.rolled_value - stat.base_value;
    let stat_color = stat_direction_color(stat_roll_direction(stat));
    let intensity = stat_delta_intensity(stat, delta);
    let border_color = if delta.abs() < 1e-9 {
      Color::TRANSPARENT
    } else {
      stat_color
    };

    let name_el = text(stat.display_name.clone())
      .font(body::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      });

    let delta_line = format_delta_line(delta, stat.base_value, &stat.unit_suffix);
    let delta_el = text(delta_line)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(stat_color),
      });

    let left_col = column([name_el.into(), delta_el.into()]).width(Length::Fill);

    let value_num = format_stat_value(stat.rolled_value, "");
    let unit_str = stat.unit_suffix.trim().to_string();
    let value_el = row([
      text(value_num)
        .font(mono::MEDIUM)
        .size(16.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(unit_str)
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Bottom);

    let bar_el = container(stat_intensity_bar(intensity, stat_color)).width(110.0);

    let content = row([
      left_col.into(),
      Space::new().width(14.0).into(),
      value_el.into(),
      Space::new().width(14.0).into(),
      bar_el.into(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    container(row([
      container(Space::new())
        .width(2.0)
        .height(Length::Fill)
        .style(move |_| container::Style {
          background: Some(Background::Color(border_color)),
          ..container::Style::default()
        })
        .into(),
      Space::new().width(8.0).into(),
      content.into(),
    ]))
    .width(Length::Fill)
    .padding(Padding {
      top: 5.0,
      bottom: 5.0,
      left: 0.0,
      right: 0.0,
    })
    .into()
  }
}

fn format_delta_line(delta: f64, base: f64, unit_suffix: &str) -> String {
  let sign = if delta >= 0.0 { "+" } else { "" };
  let abs_str = format_stat_value(delta.abs(), unit_suffix);
  let pct = if base.abs() > 1e-9 { delta / base * 100.0 } else { 0.0 };
  let pct_sign = if pct >= 0.0 { "+" } else { "" };
  format!("{}{} \u{00b7} {}{:.1}%", sign, abs_str, pct_sign, pct)
}

fn stat_delta_intensity(stat: &AbyssalStatViewModel, delta: f64) -> f32 {
  let range_span = (stat.max_mult - 1.0).abs().max(1e-9);
  let delta_pct = if stat.base_value.abs() > 1e-9 {
    (delta / stat.base_value).abs()
  } else {
    0.0
  };
  (delta_pct / range_span).clamp(0.0, 1.0) as f32
}

fn stat_direction_color(dir: Option<bool>) -> Color {
  match dir {
    Some(true) => color::text::SUCCESS,
    Some(false) => color::text::DANGER,
    None => color::text::TERTIARY,
  }
}

fn stat_intensity_bar(intensity: f32, fill_col: Color) -> Element<'static, Message> {
  let bg_col = color::border::SUBTLE;
  container(
    container(Space::new().width(Length::Fixed(intensity * 110.0)).height(4.0)).style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(fill_col, 0.9))),
      ..container::Style::default()
    }),
  )
  .width(110.0)
  .height(4.0)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_col)),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .clip(true)
  .into()
}

fn stat_roll_direction(stat: &AbyssalStatViewModel) -> Option<bool> {
  let delta = stat.rolled_value - stat.base_value;
  if delta.abs() < 1e-9 {
    None
  } else if stat.high_is_good {
    Some(delta > 0.0)
  } else {
    Some(delta < 0.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod format_delta_line {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_a_positive_delta_with_plus_sign() {
      let result = format_delta_line(10.0, 100.0, " HP");

      assert_eq!(result, "+10 HP \u{00b7} +10.0%");
    }

    #[test]
    fn it_formats_a_negative_delta_without_plus_sign() {
      let result = format_delta_line(-5.0, 50.0, " tf");

      assert_eq!(result, "5 tf \u{00b7} -10.0%");
    }

    #[test]
    fn it_formats_zero_pct_when_base_is_near_zero() {
      let result = format_delta_line(1.0, 0.0, "%");

      assert_eq!(result, "+1% \u{00b7} +0.0%");
    }
  }
}
