use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  border::Radius,
  widget::{Column, Row, Space, container, text},
};

use super::super::{Message, fmt_duration, fmt_eta};
use crate::ui::{
  components::{
    button::{Button, Size},
    eyebrow::eyebrow,
    icon::Icon,
    rule,
  },
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
    color::with_alpha(color::accent(), 0.08)
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
      &t!("skills.queue_footer.total", count => total_n),
      Some(color::text::secondary()),
    ),
    text(t!("skills.queue_footer.select_hint"))
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
    text(t!("skills.queue_footer.finishes", eta => fmt_eta(now, total_secs)))
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
      background: Some(Background::Color(color::accent())),
      border: Border {
        radius: Radius::from(SELECTION_DOT / 2.0),
        ..Border::default()
      },
      ..container::Style::default()
    });

  Row::with_children(vec![
    glow_dot.into(),
    text(t!("skills.queue_footer.selected", count => count))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent()),
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
  Button::secondary(t!("skills.queue_footer.clear"))
    .size(Size::Sm)
    .on_press(Message::SelectionCleared)
    .into()
}

fn create_button<'a>(count: usize) -> Element<'a, Message> {
  let label = format!("{} {}", t!("skills.queue_footer.create_plan"), count);
  Button::primary(label)
    .icon_right(Icon::chevron_right())
    .size(Size::Sm)
    .on_press(Message::CreatePlanFromSelection)
    .into()
}
