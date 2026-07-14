use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, Stack, button, container, text},
};

use super::{
  IndustryJob, Message, Owner, Scope, State, Tab, blueprints, colonies, extractions, jobs, planner, switcher,
};
use crate::ui::{
  components::{
    backdrop, forbidden,
    icon::Icon,
    positioned_dropdown::positioned_dropdown,
    rule,
    tab_select::{self, TabLayout},
  },
  format::fmt_isk_opt,
  style::{color, control, radius, spacing, typography},
};

const HEADER_SIDE_PADDING: f32 = 28.0;

const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;

const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;

const TAB_STRIP_HEIGHT: f32 = 48.0;

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
    t!("industry.shell.unauthorized_one", name => names).into_owned()
  } else {
    t!("industry.shell.unauthorized_many", count => unauthorized.len(), names => names).into_owned()
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
  if let Some((id, name, missing)) = state.tab_scope_gate() {
    return forbidden::forbidden(tab_noun(state.tab()), name, &missing, Message::ReauthRequested(id));
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
      &t!("industry.header.jobs"),
      stats.active.to_string(),
      ready_accent(stats.ready),
      &t!("industry.header.jobs_sub"),
    ),
    rule::vertical(44.0),
    stat(
      &t!("industry.header.slots"),
      format!("{}/{}", stats.slots_used, stats.slots_max),
      None,
      &t!("industry.header.slots_sub"),
    ),
    rule::vertical(44.0),
    stat(
      &t!("industry.header.in_production"),
      fmt_isk_opt((stats.output_value != 0.0).then_some(stats.output_value)),
      None,
      &t!("industry.header.in_production_sub"),
    ),
    rule::vertical(44.0),
    stat(
      &t!("industry.header.fees"),
      fmt_isk_opt((stats.fees != 0.0).then_some(stats.fees)),
      None,
      &t!("industry.header.fees_sub"),
    ),
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

fn reauth_button<'a>(target: i64) -> Element<'a, Message> {
  button(
    text(t!("industry.shell.reauth"))
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
  let tabs = state
    .enabled_tabs()
    .iter()
    .map(|&tab| {
      let selected = state.tab() == tab;
      tab_select::Tab {
        count_danger: false,
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
    Tab::Blueprints => state.visible_blueprints().len().to_string(),
    Tab::Colonies => String::new(),
    Tab::Extractions => state.visible_extractions().len().to_string(),
    Tab::Jobs => state.visible_jobs().len().to_string(),
    Tab::Planner => state
      .planner()
      .plan()
      .map(|plan| plan.node_count().to_string())
      .unwrap_or_default(),
  }
}

fn tab_icon(tab: Tab) -> Icon {
  match tab {
    Tab::Blueprints => Icon::doc(),
    Tab::Colonies => Icon::planet(),
    Tab::Extractions => Icon::moon(),
    Tab::Jobs => Icon::industry(),
    Tab::Planner => Icon::flask(),
  }
}

fn tab_noun(tab: Tab) -> &'static str {
  match tab {
    Tab::Blueprints => "Blueprints",
    Tab::Colonies => "Colonies",
    Tab::Extractions => "Extractions",
    Tab::Jobs => "Industry jobs",
    Tab::Planner => "Planner",
  }
}

fn tab_body<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  match state.tab() {
    Tab::Blueprints => blueprints::tab(state),
    Tab::Colonies => colonies::tab(state),
    Tab::Extractions => extractions::tab(state, now),
    Tab::Jobs => jobs::tab(state, now),
    Tab::Planner => planner::view(state.planner(), state.active()).map(Message::Planner),
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
