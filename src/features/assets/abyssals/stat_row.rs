use iced::{
  Background, Border, Color, Element, Length, Theme,
  alignment::Vertical,
  widget::{Row, Space, container, text},
};

use super::{AbyssalStat, format_stat_value};
use crate::{
  features::assets::Message,
  ui::style::{color, radius, spacing, typography},
};

const BAR_WIDTH: f32 = 110.0;
const BORDER_HEIGHT: f32 = 28.0;
const EPSILON: f64 = 1e-9;

pub(super) fn view(stat: &AbyssalStat) -> Element<'_, Message> {
  let delta = stat.rolled - stat.base_value;
  let direction = roll_direction(stat);
  let stat_color = direction_color(direction);
  let intensity = delta_intensity(stat);
  let border_color = if delta.abs() < EPSILON {
    Color::TRANSPARENT
  } else {
    stat_color
  };

  let name = text(stat.display_name.clone())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_: &Theme| text::Style {
      color: Some(color::text::secondary()),
    });

  let delta_line = text(format_delta_line(delta, stat.base_value, &stat.unit_suffix))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(move |_: &Theme| text::Style {
      color: Some(stat_color),
    });

  let left = iced::widget::Column::with_children(vec![name.into(), delta_line.into()]).width(Length::Fill);

  let value = Row::with_children(vec![
    text(format_stat_value(stat.rolled, ""))
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(|_: &Theme| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(stat.unit_suffix.trim().to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_: &Theme| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .align_y(Vertical::Bottom);

  let content = Row::with_children(vec![
    left.into(),
    Space::new().width(spacing::SPACE_3_5).into(),
    value.into(),
    Space::new().width(spacing::SPACE_3_5).into(),
    intensity_bar(intensity, stat_color),
  ])
  .align_y(Vertical::Center)
  .width(Length::Fill);

  Row::with_children(vec![
    container(Space::new())
      .width(Length::Fixed(2.0))
      .height(Length::Fixed(BORDER_HEIGHT))
      .style(move |_| container::Style {
        background: Some(Background::Color(border_color)),
        ..container::Style::default()
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    content.into(),
  ])
  .align_y(Vertical::Center)
  .width(Length::Fill)
  .into()
}

fn delta_intensity(stat: &AbyssalStat) -> f32 {
  let reach = (stat.bound_hi - stat.base_value)
    .abs()
    .max((stat.base_value - stat.bound_lo).abs());
  if reach < EPSILON {
    return 0.0;
  }
  (((stat.rolled - stat.base_value).abs()) / reach).clamp(0.0, 1.0) as f32
}

fn direction_color(direction: Option<bool>) -> Color {
  match direction {
    Some(true) => color::status::ONLINE,
    Some(false) => color::status::DANGER,
    None => color::text::tertiary(),
  }
}

fn format_delta_line(delta: f64, base: f64, unit_suffix: &str) -> String {
  let sign = if delta >= 0.0 { "+" } else { "" };
  let magnitude = format_stat_value(delta.abs(), unit_suffix);
  let pct = if base.abs() > EPSILON {
    delta / base * 100.0
  } else {
    0.0
  };
  let pct_sign = if pct >= 0.0 { "+" } else { "" };
  format!("{sign}{magnitude} \u{00b7} {pct_sign}{pct:.1}%")
}

fn intensity_bar(intensity: f32, fill: Color) -> Element<'static, Message> {
  container(
    container(
      Space::new()
        .width(Length::Fixed(intensity * BAR_WIDTH))
        .height(Length::Fixed(4.0)),
    )
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(fill, 0.9))),
      ..container::Style::default()
    }),
  )
  .width(Length::Fixed(BAR_WIDTH))
  .height(Length::Fixed(4.0))
  .clip(true)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn roll_direction(stat: &AbyssalStat) -> Option<bool> {
  let delta = stat.rolled - stat.base_value;
  if delta.abs() < EPSILON {
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

  fn stat(base: f64, rolled: f64, bounds: (f64, f64), high_is_good: bool) -> AbyssalStat {
    AbyssalStat {
      attribute_id: 1,
      base_value: base,
      bound_hi: bounds.1,
      bound_lo: bounds.0,
      display_name: "Stat".to_owned(),
      high_is_good,
      rolled,
      unit_suffix: " tf".to_owned(),
    }
  }

  mod delta_intensity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_full_at_the_far_bound() {
      assert_eq!(delta_intensity(&stat(100.0, 140.0, (60.0, 140.0), true)), 1.0);
    }

    #[test]
    fn it_is_zero_when_unrolled() {
      assert_eq!(delta_intensity(&stat(100.0, 100.0, (60.0, 140.0), true)), 0.0);
    }
  }

  mod format_delta_line {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_a_negative_delta_without_plus_sign() {
      let result = format_delta_line(-5.0, 50.0, " tf");

      assert_eq!(result, "5 tf \u{00b7} -10.0%");
    }

    #[test]
    fn it_formats_a_positive_delta_with_plus_sign() {
      let result = format_delta_line(10.0, 100.0, " HP");

      assert_eq!(result, "+10 HP \u{00b7} +10.0%");
    }

    #[test]
    fn it_formats_zero_pct_when_base_is_near_zero() {
      let result = format_delta_line(1.0, 0.0, "%");

      assert_eq!(result, "+1% \u{00b7} +0.0%");
    }
  }

  mod roll_direction {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_a_bad_roll_when_high_is_good_and_value_drops() {
      assert_eq!(roll_direction(&stat(100.0, 80.0, (60.0, 140.0), true)), Some(false));
    }

    #[test]
    fn it_flags_a_good_roll_when_low_is_good_and_value_drops() {
      assert_eq!(roll_direction(&stat(100.0, 80.0, (60.0, 140.0), false)), Some(true));
    }

    #[test]
    fn it_is_none_when_unrolled() {
      assert_eq!(roll_direction(&stat(100.0, 100.0, (60.0, 140.0), true)), None);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_stat_row() {
      let stat = stat(100.0, 120.0, (60.0, 140.0), true);

      let _el: Element<'_, Message> = view(&stat);
    }
  }
}
