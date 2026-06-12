use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Column, Row, container, text},
};

use super::super::{Message, fmt_eta, queue::Attr};
use crate::ui::style::{color, radius, spacing, typography};

const HERO_DOT: f32 = 6.0;
const HERO_READOUT_WIDTH: f32 = 240.0;
const SECS_PER_DAY: f64 = 86_400.0;
const SECS_PER_MIN: f64 = 60.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn right_col<'a>(
  pct: i64,
  sp_now: u64,
  sp_to: u64,
  primary: Attr,
  secondary: Attr,
  sp_rate: f64,
  remaining_secs: i64,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let sp_per_min = (sp_rate * SECS_PER_MIN).round() as i64;
  let sp_per_day = (sp_rate * SECS_PER_DAY).round() as i64;

  let attrs = Row::with_children(vec![attr_chip(primary, true), attr_chip(secondary, false)]).spacing(spacing::SPACE_2);

  Column::with_children(vec![
    readout(
      "Progress",
      text(format!("{pct}%"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::LG)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Some(format!("{} / {} SP", group_thousands(sp_now), group_thousands(sp_to))),
    ),
    readout(
      "Attributes",
      attrs.into(),
      Some(format!("{sp_per_min} SP/min \u{b7} {}K/day", sp_per_day / 1_000)),
    ),
    readout(
      "Completes at",
      text(fmt_eta(now, remaining_secs))
        .font(typography::mono::MEDIUM)
        .size(typography::size::LG)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Some("EVE Time".to_owned()),
    ),
  ])
  .spacing(spacing::SPACE_3_5)
  .width(Length::Fixed(HERO_READOUT_WIDTH))
  .into()
}

pub(in crate::features::skills) fn readout<'a>(
  label: &'static str,
  primary: Element<'a, Message>,
  secondary: Option<String>,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    primary,
  ];
  if let Some(secondary) = secondary {
    children.push(
      text(secondary)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    );
  }

  Column::with_children(children).spacing(spacing::UNIT).into()
}

pub(in crate::features::skills) fn attr_chip<'a>(attr: Attr, primary: bool) -> Element<'a, Message> {
  let (fill, fg, border) = if primary {
    (
      color::with_alpha(color::accent::PLASMA, 0.12),
      color::accent::PLASMA,
      color::with_alpha(color::accent::PLASMA, 0.35),
    )
  } else {
    (
      color::with_alpha(color::text::PRIMARY, 0.05),
      color::text::secondary(),
      color::rule(),
    )
  };

  container(
    text(attr.short().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 2.0,
    right: 7.0,
    bottom: 2.0,
    left: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(fill)),
    border: Border {
      color: border,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

pub(in crate::features::skills) fn rank_badge<'a>(rank: u8) -> Element<'a, Message> {
  container(
    text(format!("\u{d7}{rank}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 3.0,
    right: 8.0,
    bottom: 3.0,
    left: 8.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

pub(super) fn pulse_dot<'a>() -> Element<'a, Message> {
  use iced::widget::Space;
  container(Space::new())
    .width(Length::Fixed(HERO_DOT))
    .height(Length::Fixed(HERO_DOT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      border: Border {
        radius: (HERO_DOT / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

pub(in crate::features::skills) fn group_thousands(value: u64) -> String {
  let digits = value.to_string();
  let mut out = String::with_capacity(digits.len() + digits.len() / 3);
  let bytes = digits.as_bytes();
  for (index, byte) in bytes.iter().enumerate() {
    if index > 0 && (bytes.len() - index).is_multiple_of(3) {
      out.push(',');
    }
    out.push(*byte as char);
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  mod group_thousands {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_digits_in_threes() {
      assert_eq!(group_thousands(0), "0");
      assert_eq!(group_thousands(999), "999");
      assert_eq!(group_thousands(1_000), "1,000");
      assert_eq!(group_thousands(1_234_567), "1,234,567");
    }
  }
}
