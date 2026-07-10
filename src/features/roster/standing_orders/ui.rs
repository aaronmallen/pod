use iced::{
  Background, Border, Color, Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Row, container, text},
};

use super::PilotRef;
use crate::{
  store::{images, model::ObjectiveStatus},
  ui::{
    components::{avatar::avatar, icon::Icon},
    style::{color, radius, spacing, typography},
  },
};

const STAMP_ICON: f32 = 11.0;
const STAMP_RADIUS: f32 = 5.0;

pub(super) fn identity() -> Color {
  color::accent()
}

pub(super) fn accent_color(hex: &str) -> Color {
  color::from_hex(hex).unwrap_or_else(identity)
}

pub(super) fn status_color(status: ObjectiveStatus) -> Color {
  match status {
    ObjectiveStatus::Active => identity(),
    ObjectiveStatus::Complete => color::status::ONLINE,
    ObjectiveStatus::Cancelled => color::text::tertiary(),
  }
}

pub(super) fn status_label(status: ObjectiveStatus) -> String {
  match status {
    ObjectiveStatus::Active => t!("standing_orders.status.active"),
    ObjectiveStatus::Complete => t!("standing_orders.status.complete"),
    ObjectiveStatus::Cancelled => t!("standing_orders.status.cancelled"),
  }
  .into_owned()
}

pub(super) fn status_stamp<'a, M: 'static>(status: ObjectiveStatus) -> Element<'a, M> {
  let tint = status_color(status);

  let mut children: Vec<Element<'a, M>> = Vec::new();
  match status {
    ObjectiveStatus::Complete => children.push(Icon::check().size(STAMP_ICON).color(tint).render()),
    ObjectiveStatus::Cancelled => children.push(Icon::block().size(STAMP_ICON).color(tint).render()),
    ObjectiveStatus::Active => {}
  }
  children.push(
    text(status_label(status).to_uppercase())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
  );

  container(
    Row::with_children(children)
      .spacing(spacing::UNIT + 2.0)
      .align_y(Vertical::Center),
  )
  .padding([4.0, 9.0])
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.1))),
    border: Border {
      color: tint,
      width: 1.5,
      radius: STAMP_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

pub(super) fn target_tile<'a, M: 'static>(accent: Color, size: f32, icon: f32) -> Element<'a, M> {
  container(Icon::tracker().size(icon).color(accent).render())
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.12))),
      border: Border {
        color: color::with_alpha(accent, 0.4),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

pub(super) fn pilot_name(roster: &[PilotRef], id: i64) -> &str {
  roster
    .iter()
    .find(|pilot| pilot.id == id)
    .map(|pilot| pilot.name.as_str())
    .unwrap_or("?")
}

pub(super) fn pilot_face<'a, M: 'static>(roster: &[PilotRef], id: i64, size: f32) -> Element<'a, M> {
  let name = pilot_name(roster, id);
  let portrait = images::default_store().character_portrait_path(id);
  avatar(id, name, Length::Fixed(size), size, Some(portrait))
}

pub(super) fn pilot_chip<'a, M: 'static>(roster: &[PilotRef], id: i64, size: f32) -> Element<'a, M> {
  let name = pilot_name(roster, id).to_owned();
  Row::with_children(vec![
    pilot_face(roster, id, size),
    text(name)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::UNIT + 3.0)
  .align_y(Vertical::Center)
  .into()
}

pub(super) fn human_date(iso: &str) -> String {
  chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d")
    .map(|date| date.format("%a \u{b7} %-d %b").to_string())
    .unwrap_or_else(|_| iso.to_owned())
}

pub(super) fn source_label(kind: &str) -> String {
  match kind {
    "log_answer" => t!("standing_orders.thread.source.log_answer"),
    "field_note" => t!("standing_orders.thread.source.field_note"),
    "killmail" => t!("standing_orders.thread.source.killmail"),
    "industry" => t!("standing_orders.thread.source.industry"),
    "skill" => t!("standing_orders.thread.source.skill"),
    other => return other.to_owned(),
  }
  .into_owned()
}

pub(super) fn checkin_label(count: usize) -> String {
  if count == 1 {
    t!("standing_orders.card.checkins_one", count => count)
  } else {
    t!("standing_orders.card.checkins_other", count => count)
  }
  .into_owned()
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn it_labels_every_known_thread_source_and_falls_back_to_the_raw_kind() {
    for kind in ["log_answer", "field_note", "killmail", "industry", "skill"] {
      assert!(!source_label(kind).is_empty(), "{kind} should resolve to a label");
    }
    assert_eq!(source_label("mystery"), "mystery");
  }
}
