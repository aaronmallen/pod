use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, Stack, button, container, text},
};

use super::{IndustryJob, Message, Owner, Scope, State, Tab, jobs, switcher};
use crate::ui::{
  components::{
    backdrop, forbidden,
    icon::Icon,
    positioned_dropdown::positioned_dropdown,
    rule,
    tab_select::{self, TabLayout},
  },
  style::{color, control, radius, spacing, typography},
};

const HEADER_SIDE_PADDING: f32 = 28.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const TAB_STRIP_HEIGHT: f32 = 48.0;

pub(super) fn shell(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let body = Column::with_children(vec![header(state, now), tab_strip(state), content(state, now)])
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

fn auth_banner<'a>(state: &'a State) -> Option<Element<'a, Message>> {
  let unauthorized = state.unauthorized_characters();
  if unauthorized.is_empty() {
    return None;
  }

  let names = unauthorized
    .iter()
    .map(|owner| owner.name.as_str())
    .collect::<Vec<_>>()
    .join(", ");
  let message = if unauthorized.len() == 1 {
    format!("{names}'s industry isn't authorized and is hidden from the combined view.")
  } else {
    format!("{} pilots' industry isn't authorized: {names}.", unauthorized.len())
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
    return forbidden::forbidden("Industry", name, &missing, Message::ReauthRequested(id));
  }

  let mut children: Vec<Element<'a, Message>> = Vec::new();
  if matches!(state.active(), Scope::All)
    && let Some(banner) = auth_banner(state)
  {
    children.push(banner);
  }
  children.push(tab_body(state, now));

  Column::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn header<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let stats = Stats::derive(state, now);
  let band = Row::with_children(vec![
    switcher::trigger(state),
    rule::vertical(44.0),
    stat(
      "Active jobs",
      stats.active.to_string(),
      ready_accent(stats.ready),
      "ready to deliver",
    ),
    rule::vertical(44.0),
    stat(
      "Job slots",
      format!("{}/{}", stats.slots_used, stats.slots_max),
      None,
      "used / max",
    ),
    rule::vertical(44.0),
    stat("In production", fmt_isk(stats.output_value), None, "est. output value"),
    rule::vertical(44.0),
    stat("Job fees", fmt_isk(stats.fees), None, "active jobs"),
    Space::new().width(Length::Fill).into(),
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

fn fmt_isk(value: f64) -> String {
  if value <= 0.0 {
    return "\u{2014}".to_owned();
  }
  if value >= 1_000_000_000_000.0 {
    format!("{:.2}T", value / 1_000_000_000_000.0)
  } else if value >= 1_000_000_000.0 {
    format!("{:.2}B", value / 1_000_000_000.0)
  } else if value >= 1_000_000.0 {
    format!("{:.1}M", value / 1_000_000.0)
  } else if value >= 1_000.0 {
    format!("{:.1}K", value / 1_000.0)
  } else {
    format!("{value:.0}")
  }
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

fn ready_accent<'a>(ready: usize) -> Option<Element<'a, Message>> {
  (ready > 0).then(|| {
    Row::with_children(vec![
      Icon::check()
        .color(color::status::ONLINE)
        .size(11.0)
        .render::<Message>(),
      text(ready.to_string())
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::status::ONLINE))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center)
    .into()
  })
}

fn stat<'a>(label: &str, value: String, accent: Option<Element<'a, Message>>, sub: &str) -> Element<'a, Message> {
  let mut value_row: Vec<Element<'a, Message>> = vec![
    text(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if let Some(accent) = accent {
    value_row.push(accent);
  }

  Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
    Row::with_children(value_row)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Bottom)
      .into(),
    text(sub.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .into()
}

fn tab_strip(state: &State) -> Element<'_, Message> {
  let tabs = Tab::ALL
    .into_iter()
    .map(|tab| {
      let selected = state.tab() == tab;
      tab_select::Tab {
        count: tab_count(state, tab),
        icon: Some(tab_icon(tab)),
        label: tab.label(),
        on_press: (!selected).then_some(Message::TabSelected(tab)),
        selected,
      }
    })
    .collect::<Vec<_>>();

  container(tab_select::tab_select_with(tabs, TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT))
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: HEADER_SIDE_PADDING,
      right: HEADER_SIDE_PADDING,
    })
    .style(control::bordered_pane)
    .into()
}

fn tab_count(state: &State, tab: Tab) -> String {
  match tab {
    Tab::Jobs => state.visible_jobs().len().to_string(),
  }
}

fn tab_icon(tab: Tab) -> Icon {
  match tab {
    Tab::Jobs => Icon::industry(),
  }
}

fn tab_body<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  match state.tab() {
    Tab::Jobs => jobs::tab(state, now),
  }
}

struct Stats {
  active: usize,
  fees: f64,
  output_value: f64,
  ready: usize,
  slots_max: i64,
  slots_used: i64,
}

impl Stats {
  fn derive(state: &State, now: DateTime<Utc>) -> Self {
    let jobs = state.visible_jobs();
    let ready = jobs.iter().filter(|job| job.is_ready(now)).count();
    let fees = jobs.iter().map(|job| job.cost).sum();
    let output_value = jobs.iter().filter_map(|job| job.value).sum();
    let (slots_used, slots_max) = slot_totals(state, &jobs, now);
    Stats {
      active: jobs.len(),
      fees,
      output_value,
      ready,
      slots_max,
      slots_used,
    }
  }
}

fn slot_totals(state: &State, jobs: &[&IndustryJob], now: DateTime<Utc>) -> (i64, i64) {
  let used = jobs.iter().filter(|job| !job.is_ready(now)).count() as i64;
  let max = match state.active() {
    Scope::All => state
      .roster()
      .iter()
      .map(|owner| owner.slots.manufacturing + owner.slots.reactions + owner.slots.science)
      .sum(),
    Scope::Char(id) => state
      .owner(Owner::Character(id))
      .map(|owner| owner.slots.manufacturing + owner.slots.reactions + owner.slots.science)
      .unwrap_or(0),
    Scope::Corp(id) => state
      .owner(Owner::Corporation(id))
      .map(|owner| owner.slots.manufacturing + owner.slots.reactions + owner.slots.science)
      .unwrap_or(0),
  };
  (used, max)
}
