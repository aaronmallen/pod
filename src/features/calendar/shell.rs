use chrono::{DateTime, Datelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, Stack, button, container, text},
};

use super::{
  EventMessage, EventWindow, Message, Scope, State, View, agenda, day, detail, grid, month, palette,
  palette::OwnerType, switcher, week, year,
};
use crate::{
  config::Feature,
  ui::{
    components::{
      backdrop, forbidden, header::header as content_header, icon::Icon, positioned_dropdown::positioned_dropdown,
      rule, segmented::segment_button,
    },
    datefmt,
    style::{color, radius, spacing, typography},
  },
};

const HEADER_SIDE_PADDING: f32 = 28.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;

pub(super) fn shell(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let body = Column::with_children(vec![header(state, now), content(state, now)])
    .width(Length::Fill)
    .height(Length::Fill);

  let base = container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  if state.picker_open() {
    let dropdown = positioned_dropdown(switcher::dropdown(state), PICKER_OVERLAY_TOP, PICKER_OVERLAY_LEFT);
    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::PickerToggled),
      dropdown,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  base.into()
}

/// The body of a detached calendar-event window: an in-content header carrying the event subject above
/// the scrollable event card. The native frame and OS title bar supply the chrome, so there is no
/// custom title bar or modal backdrop here.
pub(super) fn event_window(window: &EventWindow) -> Element<'_, EventMessage> {
  let owner = window.owner_kind();
  let tint = owner.color();

  let title = Row::with_children(vec![
    owner.icon().color(tint).size(18.0).render::<EventMessage>(),
    text(window.title().to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let body = Column::with_children(vec![
    content_header(vec![title.into()], Vec::new()),
    detail::body(window),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn auth_banner<'a>(state: &'a State) -> Option<Element<'a, Message>> {
  let unauthorized = state.unauthorized_pilots();
  if unauthorized.is_empty() {
    return None;
  }

  let names = unauthorized
    .iter()
    .map(|pilot| pilot.name.as_str())
    .collect::<Vec<_>>()
    .join(", ");
  let message = if unauthorized.len() == 1 {
    t!("calendar.shell.unauthorized_one", name => names).into_owned()
  } else {
    t!("calendar.shell.unauthorized_many", count => unauthorized.len(), names => names).into_owned()
  };
  let target = unauthorized[0].id;

  let banner = Row::with_children(vec![
    Icon::lock()
      .color(color::status::WARNING)
      .size(15.0)
      .render::<Message>(),
    text(message)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
    Space::new().width(Length::Fill).into(),
    reauth_button(target),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2_5,
    left: HEADER_SIDE_PADDING,
    right: HEADER_SIDE_PADDING,
  });

  Some(
    container(banner)
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.07))),
        border: Border {
          color: color::with_alpha(color::status::WARNING, 0.2),
          width: 1.0,
          radius: 0.0.into(),
        },
        ..container::Style::default()
      })
      .into(),
  )
}

fn content<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  if let Some((id, name, missing)) = state.scope_gate() {
    return forbidden::forbidden(Feature::Calendar.noun(), name, &missing, Message::ReauthRequested(id));
  }

  let mut children: Vec<Element<'a, Message>> = Vec::new();
  if matches!(state.active(), Scope::All)
    && let Some(banner) = auth_banner(state)
  {
    children.push(banner);
  }
  children.push(legend_bar(state));
  children.push(view_body(state, now));

  Column::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The sunken band beneath the header: owner/pilot legend on the left, an event count + timezone
/// summary on the right. Mirrors the design's black status strip (kept out of the header itself).
fn legend_bar<'a>(state: &'a State) -> Element<'a, Message> {
  let scope_label = if state.tweaks().color_by_pilot() {
    t!("calendar.legend.pilots").into_owned()
  } else {
    t!("calendar.legend.owner").into_owned()
  };
  let count = state.visible_events().len();
  let events = if count == 1 {
    t!("calendar.legend.event_count_one", count => count)
  } else {
    t!("calendar.legend.event_count_other", count => count)
  };
  let summary = if state.tweaks().local_time() {
    t!("calendar.legend.summary_local", events => events).into_owned()
  } else {
    t!("calendar.legend.summary_eve", events => events).into_owned()
  };

  let band = Row::with_children(vec![
    text(scope_label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    type_legend(state),
    Space::new().width(Length::Fill).into(),
    text(summary)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(band)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: HEADER_SIDE_PADDING,
      right: HEADER_SIDE_PADDING,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn date_nav<'a>(state: &'a State) -> Element<'a, Message> {
  let nav_button = |icon: Icon, message: Message| {
    button(icon.color(color::text::secondary()).size(16.0).render::<Message>())
      .padding(spacing::SPACE_2)
      .on_press(message)
      .style(|_, status| nav_button_style(status))
  };

  let today = button(
    text(t!("calendar.nav.today"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::CursorToday)
  .style(|_, status| nav_button_style(status));

  let (title_text, sub_text) = nav_period(state);
  let mut title_lines: Vec<Element<'a, Message>> = vec![
    text(title_text)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if let Some(sub) = sub_text {
    title_lines.push(
      text(sub.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }
  let title = Column::with_children(title_lines).spacing(spacing::UNIT);

  Row::with_children(vec![
    nav_button(Icon::chevron_left(), Message::CursorPrev).into(),
    today.into(),
    nav_button(Icon::chevron_right(), Message::CursorNext).into(),
    title.into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn header<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let _ = now;
  let band = Row::with_children(vec![
    switcher::trigger(state),
    rule::vertical(44.0),
    date_nav(state),
    Space::new().width(Length::Fill).into(),
    view_segmented(state),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(band)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: HEADER_SIDE_PADDING,
      right: HEADER_SIDE_PADDING,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn legend_item<'a>(fill: iced::Color, label: &str) -> Element<'a, Message> {
  Row::with_children(vec![
    container(Space::new())
      .width(Length::Fixed(8.0))
      .height(Length::Fixed(8.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn nav_button_style(status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
    border: Border {
      color: color::rule(),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    text_color: color::text::secondary(),
    ..button::Style::default()
  }
}

/// The header date label, which differs per view: a "from" hint for Agenda, the full day for Day, a
/// date range for Week, month + year for Month, and just the year for Year. Returns (title, optional
/// subtitle) — the subtitle is rendered as a small uppercase mono line beneath.
fn nav_period(state: &State) -> (String, Option<String>) {
  let cursor = state.cursor();
  match state.view() {
    View::Agenda => (
      t!("calendar.nav.agenda_title").into_owned(),
      Some(
        t!("calendar.nav.agenda_subtitle", month => datefmt::month_short(cursor.month()), day => cursor.day())
          .into_owned(),
      ),
    ),
    View::Day => (
      format!(
        "{}, {} {}",
        datefmt::weekday_long(cursor.weekday()),
        datefmt::month_long(cursor.month()),
        cursor.day()
      ),
      Some(t!("calendar.nav.day_subtitle", year => cursor.year()).into_owned()),
    ),
    View::Week => {
      let dates = grid::week_dates(cursor, state.tweaks().week_start(), state.tweaks().show_weekends());
      let first = dates.first().copied().unwrap_or(cursor);
      let last = dates.last().copied().unwrap_or(cursor);
      let span = if first.month0() == last.month0() {
        format!(
          "{} {} \u{2013} {}",
          datefmt::month_short(first.month()),
          first.day(),
          last.day()
        )
      } else {
        format!(
          "{} {} \u{2013} {} {}",
          datefmt::month_short(first.month()),
          first.day(),
          datefmt::month_short(last.month()),
          last.day()
        )
      };
      (
        span,
        Some(t!("calendar.nav.week_subtitle", year => first.year()).into_owned()),
      )
    }
    View::Month => (
      format!("{} {}", datefmt::month_long(cursor.month()), cursor.year()),
      None,
    ),
    View::Year => (
      cursor.year().to_string(),
      Some(t!("calendar.nav.year_subtitle").into_owned()),
    ),
  }
}

fn reauth_button<'a>(target: i64) -> Element<'a, Message> {
  button(
    text(t!("calendar.shell.reauthenticate"))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::status::WARNING)),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::ReauthRequested(target))
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::status::WARNING, 0.12))),
      border: Border {
        color: color::with_alpha(color::status::WARNING, 0.45),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::status::WARNING,
      ..button::Style::default()
    }
  })
  .into()
}

fn type_legend<'a>(state: &'a State) -> Element<'a, Message> {
  let items: Vec<Element<'a, Message>> = if state.tweaks().color_by_pilot() {
    state
      .roster()
      .iter()
      .enumerate()
      .map(|(index, pilot)| {
        let first = pilot.name.split_whitespace().next().unwrap_or(&pilot.name).to_owned();
        legend_item(palette::pilot_color(index), &first)
      })
      .collect()
  } else {
    palette::TYPE_LEGEND_ORDER
      .into_iter()
      .map(|owner: OwnerType| legend_item(owner.color(), &owner.short_label()))
      .collect()
  };

  Row::with_children(items)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .into()
}

fn view_body<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  match state.view() {
    View::Agenda => agenda::view(state, now),
    View::Day => day::view(state, now),
    View::Month => month::view(state, now),
    View::Week => week::view(state, now),
    View::Year => year::view(state, now),
  }
}

fn view_segmented<'a>(state: &'a State) -> Element<'a, Message> {
  let segments: Vec<Element<'a, Message>> = View::ALL
    .into_iter()
    .map(|view| {
      segment_button(
        view.label(),
        state.view() == view,
        Padding {
          top: spacing::SPACE_2,
          bottom: spacing::SPACE_2,
          left: spacing::SPACE_3,
          right: spacing::SPACE_3,
        },
        Message::ViewSelected(view),
      )
    })
    .collect();

  container(Row::with_children(segments).spacing(2.0))
    .padding(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}
