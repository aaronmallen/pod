use chrono::{DateTime, Datelike, Timelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{
  CalendarEvent, Detail, Message, State,
  palette::{OwnerType, Response},
};
use crate::{
  store::model::AttendeeTally,
  ui::{
    components::icon::Icon,
    style::{color, radius, spacing, typography},
  },
};

const MODAL_WIDTH: f32 = 580.0;

pub(super) fn modal<'a>(state: &'a State, detail: &'a Detail) -> Option<Element<'a, Message>> {
  let event = state
    .visible_events()
    .into_iter()
    .find(|event| event.character_id == detail.character_id && event.event_id == detail.event_id)?;
  let owner = event.owner_kind();
  let tint = owner.color();

  let mut sections: Vec<Element<'a, Message>> = vec![
    accent_bar(tint),
    header(event, owner, tint),
    meta_grid(state, event, owner),
  ];

  if let Some(body) = event.body.as_deref().filter(|body| !body.is_empty()) {
    sections.push(body_text(body));
  }

  if owner.respondable() {
    sections.push(respond_section(event));
  }

  if let Some(att) = detail.attendees.as_ref() {
    sections.push(attendees_section(att));
  }

  sections.push(provenance(event, owner));

  let card = container(Column::with_children(sections).spacing(spacing::SPACE_3))
    .width(Length::Fixed(MODAL_WIDTH))
    .padding(spacing::SPACE_6)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  Some(card.into())
}

fn accent_bar<'a>(tint: iced::Color) -> Element<'a, Message> {
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

fn attendees_section<'a>(att: &AttendeeTally) -> Element<'a, Message> {
  let replied = att.accepted + att.tentative + att.declined;

  let summary = Row::with_children(vec![
    tally_cell(Icon::check(), att.accepted, color::status::ONLINE),
    tally_cell(Icon::tilde(), att.tentative, color::status::WARNING),
    tally_cell(Icon::cross(), att.declined, color::status::DANGER),
    Space::new().width(Length::Fill).into(),
    text(format!("{replied} / {} replied", att.invited))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  Column::with_children(vec![eyebrow("Attendees"), rsvp_bar(att), summary.into()])
    .spacing(spacing::SPACE_2)
    .into()
}

fn badge<'a>(label: &str, tint: iced::Color) -> Element<'a, Message> {
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

fn body_text<'a>(body: &str) -> Element<'a, Message> {
  text(strip_html(body))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn close_button<'a>() -> Element<'a, Message> {
  button(
    Icon::close()
      .color(color::text::secondary())
      .size(16.0)
      .render::<Message>(),
  )
  .padding(spacing::SPACE_2)
  .on_press(Message::DetailClosed)
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn date_label(start: DateTime<Utc>) -> String {
  format!("{}, {} {}", long_weekday(start), long_month(start), start.day())
}

fn eyebrow<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn header<'a>(event: &'a CalendarEvent, owner: OwnerType, tint: iced::Color) -> Element<'a, Message> {
  let glyph = container(owner.icon().color(tint).size(20.0).render::<Message>())
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

  let mut badges: Vec<Element<'a, Message>> = vec![
    text(owner.label().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(tint))
      .into(),
  ];
  if event.importance >= 1 {
    badges.push(badge("Important", color::status::DANGER));
  }
  if event.owner_type == "pod" {
    badges.push(badge("Pod", color::status::ONLINE));
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
    close_button(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Top)
  .into()
}

fn hhmm(dt: DateTime<Utc>) -> String {
  format!("{:02}:{:02}", dt.hour(), dt.minute())
}

fn long_month(day: DateTime<Utc>) -> &'static str {
  const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ];
  MONTHS[(day.month0() as usize).min(11)]
}

fn long_weekday(day: DateTime<Utc>) -> &'static str {
  const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
  ];
  DAYS[day.weekday().num_days_from_monday() as usize]
}

fn meta_grid<'a>(state: &'a State, event: &'a CalendarEvent, owner: OwnerType) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = Vec::new();

  let when = when_value(event, state.tweaks().local_time());
  rows.push(meta_row(Icon::clock(), "When", when));

  if let Some(start) = event.start() {
    rows.push(meta_row(Icon::calendar(), "Date", date_label(start)));
  }

  rows.push(meta_row(
    owner.icon(),
    "Owner",
    format!("{} \u{00B7} {}", event.owner_name, owner.label()),
  ));

  if let Some(pilot) = state.pilot(event.character_id) {
    rows.push(meta_row(Icon::characters(), "Calendar", pilot.name.clone()));
  }

  Column::with_children(rows).spacing(spacing::SPACE_2).into()
}

fn meta_row<'a>(icon: Icon, label: &str, value: String) -> Element<'a, Message> {
  Row::with_children(vec![
    container(
      Row::with_children(vec![
        icon.color(color::text::secondary()).size(14.0).render::<Message>(),
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

fn provenance<'a>(event: &'a CalendarEvent, owner: OwnerType) -> Element<'a, Message> {
  let line = if owner == OwnerType::Pod {
    match event.source.as_deref() {
      Some(source) => format!("Pod-derived overlay \u{00B7} {source} \u{2014} not an ESI calendar event."),
      None => "Pod-derived overlay \u{2014} not an ESI calendar event.".to_owned(),
    }
  } else {
    format!(
      "ESI \u{00B7} GET /characters/{}/calendar/{}",
      event.character_id, event.event_id
    )
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

fn respond_button<'a>(event: &'a CalendarEvent, response: Response) -> Element<'a, Message> {
  let active = event.response_kind() == response;
  let tint = response.color();

  let label = Row::with_children(vec![
    response_icon(response)
      .color(if active { tint } else { color::text::secondary() })
      .size(14.0)
      .render::<Message>(),
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
    .on_press(Message::Responded(event.character_id, event.event_id, response))
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

fn respond_section<'a>(event: &'a CalendarEvent) -> Element<'a, Message> {
  let buttons = Row::with_children(vec![
    respond_button(event, Response::Accepted),
    respond_button(event, Response::Tentative),
    respond_button(event, Response::Declined),
  ])
  .spacing(spacing::SPACE_2);

  Column::with_children(vec![eyebrow("Your response"), buttons.into()])
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

fn rsvp_bar<'a>(att: &AttendeeTally) -> Element<'a, Message> {
  let mut segments: Vec<Element<'a, Message>> = Vec::new();
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

fn tally_cell<'a>(icon: Icon, count: i64, tint: iced::Color) -> Element<'a, Message> {
  Row::with_children(vec![
    icon.color(tint).size(12.0).render::<Message>(),
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

fn when_value(event: &CalendarEvent, local_time: bool) -> String {
  if event.is_all_day() {
    return "All day \u{00B7} EVE".to_owned();
  }
  let Some(start) = event.start() else {
    return "Unknown".to_owned();
  };
  let base = match event.end() {
    Some(end) if end != start => format!("{} \u{2013} {} EVE", hhmm(start), hhmm(end)),
    _ => format!("{} EVE", hhmm(start)),
  };
  if local_time {
    format!("{base} \u{00B7} {} LT", hhmm(start))
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
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_an_all_day_event() {
      assert_eq!(when_value(&event(1440, "accepted"), false), "All day \u{00B7} EVE");
    }

    #[test]
    fn it_renders_a_timed_span_in_eve() {
      assert_eq!(when_value(&event(90, "accepted"), false), "19:00 \u{2013} 20:30 EVE");
    }

    #[test]
    fn it_appends_local_time_when_enabled() {
      let value = when_value(&event(0, "accepted"), true);

      assert!(value.contains("LT"));
    }
  }
}
