use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  border::Radius,
  widget::{Column, Row, Space, button, container, text},
};

use super::super::{Message, fmt_duration, fmt_eta};
use crate::ui::{
  components::{eyebrow::eyebrow, rule},
  style::{color, radius, spacing, typography},
};

const SELECTION_DOT: f32 = 7.0;

pub(super) fn footer<'a>(
  total_n: usize,
  total_secs: f64,
  selection_count: usize,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let body = if selection_count > 0 {
    selection_bar(selection_count)
  } else {
    totals_bar(total_n, total_secs, now)
  };

  let background = if selection_count > 0 {
    color::with_alpha(color::accent::PLASMA, 0.08)
  } else {
    color::surface::SUNKEN
  };

  let bar = container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(background)),
      border: Border {
        radius: Radius {
          top_left: 0.0,
          top_right: 0.0,
          bottom_right: radius::CONTROL,
          bottom_left: radius::CONTROL,
        },
        ..Border::default()
      },
      ..container::Style::default()
    });

  Column::with_children(vec![rule::horizontal(), bar.into()])
    .width(Length::Fill)
    .into()
}

fn totals_bar<'a>(total_n: usize, total_secs: f64, now: DateTime<Utc>) -> Row<'a, Message> {
  let total_secs = total_secs.round() as i64;

  Row::with_children(vec![
    eyebrow(
      &format!("Total \u{b7} {total_n} skills"),
      Some(color::text::secondary()),
    ),
    text("\u{21e7} / \u{2318}-click to select")
      .font(typography::body::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    text(fmt_duration(total_secs))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("finishes {} EVE", fmt_eta(now, total_secs)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
}

fn selection_bar<'a>(count: usize) -> Row<'a, Message> {
  let glow_dot = container(text(""))
    .width(Length::Fixed(SELECTION_DOT))
    .height(Length::Fixed(SELECTION_DOT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      border: Border {
        radius: Radius::from(SELECTION_DOT / 2.0),
        ..Border::default()
      },
      ..container::Style::default()
    });

  Row::with_children(vec![
    glow_dot.into(),
    text(format!("{count} selected"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    clear_button(),
    create_button(count),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
}

fn clear_button<'a>() -> Element<'a, Message> {
  button(
    text("Clear")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::SelectionCleared)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if hover { 0.25 } else { 0.1 }),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if hover {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn create_button<'a>(count: usize) -> Element<'a, Message> {
  button(
    text(format!("Create plan \u{25b8} {count}"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::CreatePlanFromSelection)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(color::with_alpha(
        color::accent::PLASMA,
        if hover { 0.22 } else { 0.14 },
      ))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, if hover { 0.6 } else { 0.4 }),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  })
  .into()
}
