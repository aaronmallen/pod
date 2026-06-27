use chrono::{DateTime, Datelike, TimeZone, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use super::{
  CalendarEvent, EventMessage, EventWindow,
  palette::{OwnerType, Response},
  time::{fmt_eve, fmt_local},
};
use crate::{
  store::model::AttendeeTally,
  ui::{
    components::icon::Icon,
    datefmt,
    style::{color, radius, spacing, typography},
  },
};

const BODY_WIDTH: f32 = 580.0;

/// The scrollable card body for a detached calendar-event window. The native frame and OS title bar
/// replace the old modal chrome and close button, so this renders only the event content (accent bar,
/// header block, meta grid, body, respond controls, attendees, provenance) on the base surface.
pub(super) fn body(window: &EventWindow) -> Element<'_, EventMessage> {
  let event = &window.event;
  let owner = event.owner_kind();
  let tint = owner.color();

  let mut sections: Vec<Element<'_, EventMessage>> =
    vec![accent_bar(tint), header(event, owner, tint), meta_grid(window)];

  if let Some(text) = event.body.as_deref().filter(|body| !body.is_empty()) {
    sections.push(body_text(text));
  }

  if owner.respondable() {
    sections.push(respond_section(event));
  }

  if let Some(att) = window.attendees.as_ref() {
    sections.push(attendees_section(att));
  }

  sections.push(provenance(event, owner));

  let card = container(Column::with_children(sections).spacing(spacing::SPACE_3))
    .width(Length::Fixed(BODY_WIDTH))
    .padding(spacing::SPACE_6);

  let centered = container(card)
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .padding(spacing::SPACE_3);

  scrollable(centered)
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn accent_bar<'a>(tint: iced::Color) -> Element<'a, EventMessage> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(4.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(tint)),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn attendees_section<'a>(att: &AttendeeTally) -> Element<'a, EventMessage> {
  let replied = att.accepted + att.tentative + att.declined;

  let summary = Row::with_children(vec![
    tally_cell(Icon::check(), att.accepted, color::status::ONLINE),
    tally_cell(Icon::tilde(), att.tentative, color::status::WARNING),
    tally_cell(Icon::cross(), att.declined, color::status::DANGER),
    Space::new().width(Length::Fill).into(),
    text(t!("calendar.detail.replied", replied => replied, invited => att.invited))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  Column::with_children(vec![
    eyebrow(t!("calendar.detail.attendees").into_owned()),
    rsvp_bar(att),
    summary.into(),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn badge<'a>(label: String, tint: iced::Color) -> Element<'a, EventMessage> {
  container(
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 1.0,
    bottom: 1.0,
    left: spacing::UNIT,
    right: spacing::UNIT,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.1))),
    border: Border {
      color: color::with_alpha(tint, 0.35),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn body_text<'a>(body: &str) -> Element<'a, EventMessage> {
  text(strip_html(body))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn date_label(start: DateTime<Utc>) -> String {
  format!(
    "{}, {} {}",
    datefmt::weekday_long(start.weekday()),
    datefmt::month_long(start.month()),
    start.day()
  )
}

fn eyebrow<'a>(label: String) -> Element<'a, EventMessage> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn header<'a>(event: &'a CalendarEvent, owner: OwnerType, tint: iced::Color) -> Element<'a, EventMessage> {
  let glyph = container(owner.icon().color(tint).size(20.0).render::<EventMessage>())
    .width(Length::Fixed(40.0))
    .height(Length::Fixed(40.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tint, 0.16))),
      border: Border {
        color: color::with_alpha(tint, 0.35),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let mut badges: Vec<Element<'a, EventMessage>> = vec![
    text(owner.label().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(tint))
      .into(),
  ];
  if event.importance >= 1 {
    badges.push(badge(
      t!("calendar.detail.important").into_owned(),
      color::status::DANGER,
    ));
  }
  if event.owner_type == "pod" {
    badges.push(badge(t!("calendar.detail.pod").into_owned(), color::status::ONLINE));
  }

  let title_block = Column::with_children(vec![
    Row::with_children(badges)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    text(event.title.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2);

  Row::with_children(vec![
    glyph.into(),
    title_block.into(),
    Space::new().width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Top)
  .into()
}

fn meta_grid(window: &EventWindow) -> Element<'_, EventMessage> {
  let event = &window.event;
  let owner = event.owner_kind();
  let mut rows: Vec<Element<'_, EventMessage>> = Vec::new();

  let when = when_value(event, window.local_time, &chrono::Local);
  rows.push(meta_row(Icon::clock(), t!("calendar.detail.when").into_owned(), when));

  if let Some(start) = event.start() {
    rows.push(meta_row(
      Icon::calendar(),
      t!("calendar.detail.date").into_owned(),
      date_label(start),
    ));
  }

  rows.push(meta_row(
    owner.icon(),
    t!("calendar.detail.owner").into_owned(),
    t!("calendar.detail.owner_value", owner => event.owner_name, kind => owner.label()).into_owned(),
  ));

  if let Some(pilot) = window.pilot_name.as_deref() {
    rows.push(meta_row(
      Icon::characters(),
      t!("calendar.detail.pilot").into_owned(),
      pilot.to_owned(),
    ));
  }

  Column::with_children(rows).spacing(spacing::SPACE_2).into()
}

fn meta_row<'a>(icon: Icon, label: String, value: String) -> Element<'a, EventMessage> {
  Row::with_children(vec![
    container(
      Row::with_children(vec![
        icon.color(color::text::secondary()).size(14.0).render::<EventMessage>(),
        text(label.to_uppercase())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::secondary()))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .width(Length::Fixed(130.0))
    .into(),
    text(value)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .into()
}

fn provenance<'a>(event: &'a CalendarEvent, owner: OwnerType) -> Element<'a, EventMessage> {
  let line = if owner == OwnerType::Pod {
    match event.source.as_deref() {
      Some(source) => t!("calendar.detail.provenance_pod_source", source => source).into_owned(),
      None => t!("calendar.detail.provenance_pod").into_owned(),
    }
  } else {
    t!(
      "calendar.detail.provenance_esi",
      character => event.character_id,
      event => event.event_id
    )
    .into_owned()
  };

  container(
    text(line)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    ..Padding::ZERO
  })
  .into()
}

fn respond_button<'a>(event: &'a CalendarEvent, response: Response) -> Element<'a, EventMessage> {
  let active = event.response_kind() == response;
  let tint = response.color();

  let label = Row::with_children(vec![
    response_icon(response)
      .color(if active { tint } else { color::text::secondary() })
      .size(14.0)
      .render::<EventMessage>(),
    text(response.label())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(if active {
        tint
      } else {
        color::text::secondary()
      }))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(container(label).width(Length::Fill).align_x(Horizontal::Center))
    .padding(spacing::SPACE_2_5)
    .width(Length::Fill)
    .on_press(EventMessage::Responded(response))
    .style(move |_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: if active {
          Some(Background::Color(color::with_alpha(tint, 0.18)))
        } else if hovered {
          Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
        } else {
          None
        },
        border: Border {
          color: if active { tint } else { color::rule() },
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: if active { tint } else { color::text::secondary() },
        ..button::Style::default()
      }
    })
    .into()
}

fn respond_section<'a>(event: &'a CalendarEvent) -> Element<'a, EventMessage> {
  let buttons = Row::with_children(vec![
    respond_button(event, Response::Accepted),
    respond_button(event, Response::Tentative),
    respond_button(event, Response::Declined),
  ])
  .spacing(spacing::SPACE_2);

  Column::with_children(vec![
    eyebrow(t!("calendar.detail.your_response").into_owned()),
    buttons.into(),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn response_icon(response: Response) -> Icon {
  match response {
    Response::Accepted => Icon::check(),
    Response::Declined => Icon::cross(),
    Response::NotResponded => Icon::tilde(),
    Response::Tentative => Icon::tilde(),
  }
}

fn rsvp_bar<'a>(att: &AttendeeTally) -> Element<'a, EventMessage> {
  let mut segments: Vec<Element<'a, EventMessage>> = Vec::new();
  for (count, tint) in [
    (att.accepted, color::status::ONLINE),
    (att.tentative, color::status::WARNING),
    (att.declined, color::status::DANGER),
  ] {
    if count > 0 {
      segments.push(
        container(Space::new())
          .width(Length::FillPortion((count as u16).max(1)))
          .height(Length::Fill)
          .style(move |_| container::Style {
            background: Some(Background::Color(tint)),
            ..container::Style::default()
          })
          .into(),
      );
    }
  }

  container(Row::with_children(segments))
    .width(Length::Fill)
    .height(Length::Fixed(8.0))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn strip_html(html: &str) -> String {
  let mut out = String::with_capacity(html.len());
  let mut in_tag = false;
  for ch in html.chars() {
    match ch {
      '<' => in_tag = true,
      '>' => in_tag = false,
      _ if !in_tag => out.push(ch),
      _ => {}
    }
  }
  out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tally_cell<'a>(icon: Icon, count: i64, tint: iced::Color) -> Element<'a, EventMessage> {
  Row::with_children(vec![
    icon.color(tint).size(12.0).render::<EventMessage>(),
    text(format!("{count}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(tint))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center)
  .into()
}

fn when_value<Tz>(event: &CalendarEvent, local_time: bool, tz: &Tz) -> String
where
  Tz: TimeZone,
  Tz::Offset: std::fmt::Display,
{
  if event.is_all_day() {
    return t!("calendar.detail.when_all_day").into_owned();
  }
  let Some(start) = event.start() else {
    return t!("calendar.detail.when_unknown").into_owned();
  };
  let base = match event.end() {
    Some(end) if end != start => {
      t!("calendar.detail.when_range", start => fmt_eve(start), end => fmt_eve(end)).into_owned()
    }
    _ => t!("calendar.detail.when_single", start => fmt_eve(start)).into_owned(),
  };
  if local_time {
    t!("calendar.detail.when_local", base => base, local => fmt_local(start, tz)).into_owned()
  } else {
    base
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn at(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp).unwrap().with_timezone(&Utc)
  }

  fn event(duration_minutes: i64, response: &str) -> CalendarEvent {
    CalendarEvent {
      body: Some("<p>Form up.</p>".to_owned()),
      character_id: 1,
      duration_minutes,
      event_id: 7,
      importance: 0,
      owner_name: "Corp".to_owned(),
      owner_type: "corporation".to_owned(),
      response: response.to_owned(),
      source: None,
      timestamp: "2026-06-20T19:00:00Z".to_owned(),
      title: "Op".to_owned(),
    }
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_a_respondable_event_with_attendees() {
      let window = EventWindow::new(
        event(90, "accepted"),
        Some("Pilot 1".to_owned()),
        true,
        Response::NotResponded.as_esi().to_owned(),
      )
      .with_attendees(Some(AttendeeTally {
        accepted: 3,
        declined: 1,
        invited: 6,
        tentative: 2,
      }));

      let _el: Element<'_, EventMessage> = body(&window);
    }

    #[test]
    fn it_renders_a_pod_overlay_without_respond_controls() {
      let mut overlay = event(30, "not_responded");
      overlay.owner_type = "pod".to_owned();
      overlay.source = Some("skill".to_owned());
      let window = EventWindow::new(overlay, None, false, Response::NotResponded.as_esi().to_owned());

      let _el: Element<'_, EventMessage> = body(&window);
    }
  }

  mod date_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_the_long_date() {
      assert_eq!(date_label(at("2026-06-20T19:00:00Z")), "Saturday, June 20");
    }
  }

  mod strip_html {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_removes_tags_and_collapses_whitespace() {
      assert_eq!(strip_html("<p>Form  up.</p>"), "Form up.");
    }
  }

  mod when_value {
    use chrono::FixedOffset;
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;

    #[test]
    fn it_appends_the_converted_local_time_when_enabled() {
      let west = FixedOffset::west_opt(5 * 3_600).unwrap();

      assert_eq!(
        when_value(&event(0, "accepted"), true, &west),
        "19:00 EVE \u{00B7} 14:00 -05:00"
      );
    }

    #[test]
    fn it_keeps_the_eve_line_distinct_from_local() {
      let east = FixedOffset::east_opt(3 * 3_600).unwrap();
      let value = when_value(&event(0, "accepted"), true, &east);

      assert_ne!(value, "19:00 EVE");
      assert!(value.starts_with("19:00 EVE \u{00B7} 22:00"));
    }

    #[test]
    fn it_falls_back_to_the_numeric_offset() {
      let west = FixedOffset::west_opt(4 * 3_600).unwrap();

      assert!(when_value(&event(0, "accepted"), true, &west).ends_with("-04:00"));
    }

    #[test]
    fn it_renders_a_timed_span_in_eve() {
      let utc = FixedOffset::east_opt(0).unwrap();

      assert_eq!(
        when_value(&event(90, "accepted"), false, &utc),
        "19:00 \u{2013} 20:30 EVE"
      );
    }

    #[test]
    fn it_renders_an_all_day_event() {
      let utc = FixedOffset::east_opt(0).unwrap();

      assert_eq!(
        when_value(&event(1440, "accepted"), false, &utc),
        "All day \u{00B7} EVE"
      );
    }
  }
}
