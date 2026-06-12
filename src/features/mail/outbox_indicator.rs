use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, mouse_area, text},
};

use super::{Message, loaders::OutboxIndicator};
use crate::ui::{
  components::badge::badge,
  style::{color, radius, spacing, typography},
};

pub(super) fn indicator(state: &OutboxIndicator) -> Option<Element<'_, Message>> {
  if state.pending == 0 && state.failed.is_empty() {
    return None;
  }

  let mut counts = Row::new().spacing(spacing::SPACE_2_5).align_y(Vertical::Center);
  if state.pending > 0 {
    counts = counts.push(badge(format!("{} sending", state.pending), None));
  }
  if !state.failed.is_empty() {
    counts = counts.push(badge(
      format!("{} failed", state.failed.len()),
      Some(color::status::DANGER),
    ));
  }

  let mut column = Column::new().spacing(spacing::SPACE_2).push(counts);

  if let Some(first) = state.failed.first() {
    column = column.push(failure_row(first.id, &first.last_error));
  }

  Some(
    container(column)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04))),
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.1),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into(),
  )
}

fn failure_row(id: i64, last_error: &str) -> Element<'_, Message> {
  let error = text(last_error.to_owned())
    .size(typography::size::SM)
    .width(Length::Fill)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  Row::with_children(vec![
    error.into(),
    action("Retry", Message::OutboxRetry(id)),
    action("Dismiss", Message::OutboxDismiss(id)),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn action(label: &str, message: Message) -> Element<'_, Message> {
  let pill = container(
    text(label.to_owned())
      .size(typography::size::SM)
      .font(typography::body::MEDIUM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  mouse_area(pill).on_press(message).into()
}
