use chrono::{DateTime, Datelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, text},
};

use super::{Message, Scope, State, View, agenda, detail, palette, palette::OwnerType, switcher, tweaks};
use crate::{
  config::Feature,
  ui::{
    components::{
      backdrop, forbidden,
      icon::Icon,
      modal_overlay::modal_overlay,
      positioned_dropdown::{positioned_dropdown, positioned_dropdown_right},
      rule,
      segmented::segment_button,
    },
    style::{color, radius, spacing, typography},
  },
};

const HEADER_SIDE_PADDING: f32 = 28.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const TWEAKS_OVERLAY_RIGHT: f32 = HEADER_SIDE_PADDING;
const TWEAKS_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;

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

  if state.tweaks_open() {
    let panel = positioned_dropdown_right(tweaks::panel(state.tweaks()), TWEAKS_OVERLAY_TOP, TWEAKS_OVERLAY_RIGHT);
    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::TweaksToggled),
      panel,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  if let Some(detail_state) = state.detail()
    && let Some(card) = detail::modal(state, detail_state)
  {
    return modal_overlay(base.into(), Some(Message::DetailClosed), card);
  }

  base.into()
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
    format!("{names}'s calendar isn't authorized and is hidden from the combined view.")
  } else {
    format!("{} pilots' calendars aren't authorized: {names}.", unauthorized.len())
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
  children.push(view_body(state, now));

  Column::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
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
    text("TODAY")
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

  let title = Column::with_children(vec![
    text(nav_title(state))
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(state.view().label().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT);

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
    type_legend(state),
    view_segmented(state),
    tweaks_button(),
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

fn nav_title(state: &State) -> String {
  let cursor = state.cursor();
  format!("{} {}", long_month(cursor), cursor.year())
}

fn placeholder<'a>(view: View) -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      text(format!("{} view", view.label()))
        .font(typography::body::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      text("Coming soon.")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

fn reauth_button<'a>(target: i64) -> Element<'a, Message> {
  button(
    text("Re-authenticate")
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

fn tweaks_button<'a>() -> Element<'a, Message> {
  button(
    Icon::settings()
      .color(color::text::secondary())
      .size(16.0)
      .render::<Message>(),
  )
  .padding(spacing::SPACE_2)
  .on_press(Message::TweaksToggled)
  .style(|_, status| nav_button_style(status))
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
      .map(|owner: OwnerType| legend_item(owner.color(), owner.short_label()))
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
    other => placeholder(other),
  }
}

fn view_segmented<'a>(state: &'a State) -> Element<'a, Message> {
  let segments: Vec<Element<'a, Message>> = View::ALL
    .into_iter()
    .map(|view| {
      segment_button(
        view.label().to_owned(),
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
