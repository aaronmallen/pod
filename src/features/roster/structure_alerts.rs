use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use crate::{
  clients::eve_image::Size,
  store::{
    Database,
    images::{self, IconResolution},
    repo::{character, customs_office, industry, org, sde},
  },
  ui::{
    components::{icon::Icon, rule},
    style::{color, control, radius, spacing, typography},
  },
};

const BACK_BUTTON_HEIGHT: f32 = 32.0;
const BANNER_ICON_TILE: f32 = 40.0;
const CARD_ICON_TILE: f32 = 46.0;
const CARD_MIN_WIDTH: f32 = 320.0;
const CONTENT_MAX_WIDTH: f32 = 1180.0;
const CUSTOMS_OFFICE_TYPE_ID: i64 = 2233;
const DETAIL_ICON_TILE: f32 = 64.0;
const FUEL_BAR_HEIGHT: f32 = 6.0;
const FUEL_LOW_DAYS: f64 = 2.0;
const FUEL_WARN_DAYS: f64 = 5.0;
const FUEL_WINDOW_DAYS: f64 = 30.0;
const PILL_RADIUS: f32 = 999.0;
const SCREEN_PADDING: f32 = 28.0;
const STATE_STALE_AFTER_HOURS: i64 = 24;

#[derive(Clone, Debug)]
pub enum Message {
  Back,
  ClearScope,
  CorpFilterSelected(Option<i64>),
  FilterSelected(Filter),
  Loaded(Box<Snapshot>),
  OpenStructure(i64),
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
  corps: Vec<CorpChip>,
  scope_name: Option<String>,
  scope_ticker: Option<String>,
  structures: Vec<StructureRow>,
}

#[derive(Debug, Default)]
pub struct State {
  corp_filter: Option<i64>,
  filter: Filter,
  open: Option<i64>,
  scope: Option<i64>,
  snapshot: Option<Snapshot>,
}

impl State {
  pub fn new(scope: Option<i64>) -> Self {
    Self {
      corp_filter: None,
      filter: Filter::All,
      open: None,
      scope,
      snapshot: None,
    }
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    Vec::new()
  }

  fn scoped_structures(&self) -> Vec<&StructureRow> {
    let Some(snapshot) = self.snapshot.as_ref() else {
      return Vec::new();
    };
    snapshot
      .structures
      .iter()
      .filter(|row| self.corp_filter.is_none_or(|id| row.corp_id == id))
      .collect()
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filter {
  #[default]
  All,
  Alerts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AlertKind {
  Anchoring,
  Fuel,
  Reinforced,
  Service,
}

#[derive(Clone, Debug, PartialEq)]
struct Alert {
  detail: String,
  kind: AlertKind,
  label: String,
  severity: Severity,
  timer_label: Option<String>,
  timer_target: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
struct CorpChip {
  count: usize,
  id: i64,
  ticker: String,
  top_severity: Option<Severity>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Severity {
  Info,
  Warning,
  Critical,
}

impl Severity {
  fn color(self) -> iced::Color {
    match self {
      Severity::Critical => color::status::DANGER,
      Severity::Info => color::accent(),
      Severity::Warning => color::status::WARNING,
    }
  }

  fn label(self) -> String {
    match self {
      Severity::Critical => t!("structure_alerts.severity.critical").into_owned(),
      Severity::Info => t!("structure_alerts.severity.notice").into_owned(),
      Severity::Warning => t!("structure_alerts.severity.warning").into_owned(),
    }
  }

  fn rank(self) -> u8 {
    match self {
      Severity::Critical => 3,
      Severity::Info => 1,
      Severity::Warning => 2,
    }
  }
}

#[derive(Clone, Debug)]
struct StructureRow {
  access_char: String,
  access_role: String,
  alert: Option<Alert>,
  category: String,
  corp_id: i64,
  corp_ticker: String,
  fuel_days: Option<f64>,
  icon: IconResolution,
  id: i64,
  is_poco: bool,
  name: String,
  region: String,
  reinforce_window: Option<String>,
  security: Option<f64>,
  services: Vec<ServiceRow>,
  stale: bool,
  system: String,
  tax_alliance: Option<f64>,
  tax_corp: Option<f64>,
  tax_standing: Option<f64>,
  type_name: String,
}

#[derive(Clone, Debug)]
struct ServiceRow {
  name: String,
  online: bool,
}

pub fn escape_dismiss(state: &State) -> Option<Message> {
  state.open.map(|_| Message::Back)
}

pub fn load(db: &Database, scope: Option<i64>) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move { Box::new(load_snapshot(&db, scope).await) },
    Message::Loaded,
  )
}

pub fn update(state: &mut State, message: Message, _db: &Database) -> Task<Message> {
  match message {
    Message::Back => {
      state.open = None;
      Task::none()
    }
    // ClearScope re-navigates at the app level; nothing to do in the screen state.
    Message::ClearScope => Task::none(),
    Message::CorpFilterSelected(id) => {
      state.corp_filter = id;
      Task::none()
    }
    Message::FilterSelected(filter) => {
      state.filter = filter;
      Task::none()
    }
    Message::Loaded(snapshot) => {
      state.snapshot = Some(*snapshot);
      if let Some(id) = state.corp_filter
        && !state
          .snapshot
          .as_ref()
          .is_some_and(|snapshot| snapshot.corps.iter().any(|corp| corp.id == id))
      {
        state.corp_filter = None;
      }
      Task::none()
    }
    Message::OpenStructure(id) => {
      state.open = Some(id);
      Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let body: Element<'_, Message> = match state.snapshot.as_ref() {
    None => starting_up(),
    Some(_) => match state.open.and_then(|id| find_row(state, id)) {
      Some(row) => detail_view(row),
      None => list_view(state),
    },
  };

  let content = container(
    container(body)
      .width(Length::Fill)
      .max_width(CONTENT_MAX_WIDTH)
      .padding(SCREEN_PADDING),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Center);

  let scroll = scrollable(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(control::scrollbar);

  let base = Column::with_children(vec![header_band(state), rule::horizontal(), scroll.into()])
    .width(Length::Fill)
    .height(Length::Fill);

  container(base)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn find_row(state: &State, id: i64) -> Option<&StructureRow> {
  state
    .snapshot
    .as_ref()
    .and_then(|snapshot| snapshot.structures.iter().find(|row| row.id == id))
}

fn header_band(state: &State) -> Element<'_, Message> {
  let Some(snapshot) = state.snapshot.as_ref() else {
    return container(Space::new())
      .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
      .into();
  };

  let structures = state.scoped_structures();
  let alert_count = structures.iter().filter(|row| row.alert.is_some()).count();
  let top = top_severity(structures.iter().copied());
  let scoped = state.scope.is_some();

  let identity: Element<'_, Message> = if scoped {
    let name = snapshot.scope_name.clone().unwrap_or_default();
    let ticker = snapshot.scope_ticker.clone().unwrap_or_default();
    corp_lock_control(name, ticker)
  } else {
    let tint = top.map_or(color::accent(), Severity::color);
    let tile = icon_tile(
      Icon::facilities().size(22.0).color(tint).render::<Message>(),
      BANNER_ICON_TILE,
      color::with_alpha(tint, 0.13),
      color::with_alpha(tint, 0.4),
    );
    let heading = Column::with_children(vec![
      body_text(t!("structure_alerts.title").into_owned(), 19.0, color::text::PRIMARY).into(),
      mono_caption(
        t!("structure_alerts.subtitle", count => snapshot.corps.len().to_string()).to_uppercase(),
        color::text::secondary(),
      )
      .into(),
    ])
    .spacing(4.0);
    Row::with_children(vec![tile, heading.into()])
      .spacing(spacing::SPACE_3_5)
      .align_y(Vertical::Center)
      .into()
  };

  let (alert_color, alert_sub) = match top {
    Some(severity) => (severity.color(), severity.label()),
    None => (color::status::ONLINE, t!("structure_alerts.clear").into_owned()),
  };

  let stats = Row::with_children(vec![
    head_stat(
      if scoped {
        t!("structure_alerts.stat.structures").into_owned()
      } else {
        t!("structure_alerts.stat.visible").into_owned()
      },
      structures.len().to_string(),
      color::text::PRIMARY,
      None,
    ),
    head_divider(),
    head_stat(
      t!("structure_alerts.stat.alerts").into_owned(),
      alert_count.to_string(),
      alert_color,
      Some(alert_sub),
    ),
    head_divider(),
    head_stat(
      t!("structure_alerts.stat.access_via").into_owned(),
      access_pilot_count(state).to_string(),
      color::text::PRIMARY,
      Some(pilot_word(access_pilot_count(state))),
    ),
  ])
  .spacing(spacing::SPACE_6)
  .align_y(Vertical::Center);

  let row = Row::with_children(vec![
    identity,
    head_divider(),
    stats.into(),
    Space::new().width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_6)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: SCREEN_PADDING,
      bottom: 0.0,
      left: SCREEN_PADDING,
    })
    .align_y(Vertical::Center)
    .into()
}

fn corp_lock_control<'a>(name: String, ticker: String) -> Element<'a, Message> {
  let copy = Column::with_children(vec![
    body_text(name, 17.0, color::text::PRIMARY).into(),
    mono_caption(
      t!("structure_alerts.corp_lock_sub", ticker => ticker).to_uppercase(),
      color::text::secondary(),
    )
    .into(),
  ])
  .spacing(3.0);

  let inner = Row::with_children(vec![
    Icon::chevron_left().size(16.0).color(color::text::secondary()).render(),
    copy.into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  button(container(inner).padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_3,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2_5,
  }))
  .padding(0)
  .on_press(Message::ClearScope)
  .style(outline_button_style)
  .into()
}

fn list_view(state: &State) -> Element<'_, Message> {
  let structures = state.scoped_structures();
  let alerts: Vec<&StructureRow> = structures.iter().copied().filter(|row| row.alert.is_some()).collect();
  let shown: Vec<&StructureRow> = match state.filter {
    Filter::Alerts => alerts.clone(),
    Filter::All => structures.clone(),
  };

  let mut children: Vec<Element<'_, Message>> = vec![list_toolbar(state, structures.len(), alerts.len())];

  if state.scope.is_none()
    && let Some(snapshot) = state.snapshot.as_ref()
    && snapshot.corps.len() > 1
  {
    children.push(corp_filter_chips(state, snapshot));
  }

  if !alerts.is_empty() {
    children.push(alert_banner(&alerts));
  }

  if shown.is_empty() {
    children.push(empty_state(structures.is_empty()));
  } else {
    children.push(card_grid(&shown));
  }

  Column::with_children(children).spacing(spacing::SPACE_4_5).into()
}

fn list_toolbar(state: &State, visible: usize, alerting: usize) -> Element<'_, Message> {
  let title = Column::with_children(vec![
    body_text(
      t!("structure_alerts.list.heading").into_owned(),
      20.0,
      color::text::PRIMARY,
    )
    .into(),
    mono_caption(
      t!("structure_alerts.list.counts", visible => visible.to_string(), alerting => alerting.to_string())
        .to_uppercase(),
      color::text::secondary(),
    )
    .into(),
  ])
  .spacing(4.0);

  let filters = Row::with_children(vec![
    filter_button(
      state.filter,
      Filter::All,
      t!("structure_alerts.filter.all").into_owned(),
      visible,
    ),
    filter_button(
      state.filter,
      Filter::Alerts,
      t!("structure_alerts.filter.alerts").into_owned(),
      alerting,
    ),
  ])
  .spacing(spacing::UNIT);

  Row::with_children(vec![
    title.into(),
    Space::new().width(Length::Fill).into(),
    container(filters)
      .padding(spacing::UNIT)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::rule(),
          radius: radius::NAV_CARD.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn filter_button<'a>(active: Filter, target: Filter, label: String, count: usize) -> Element<'a, Message> {
  let on = active == target;
  let count_color = if on && target == Filter::Alerts && count > 0 {
    color::status::DANGER
  } else if on {
    color::accent()
  } else {
    color::text::tertiary()
  };
  let inner = Row::with_children(vec![
    mono_caption(
      label.to_uppercase(),
      if on {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
    )
    .into(),
    mono_caption(count.to_string(), count_color).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(container(inner).padding(Padding {
    top: spacing::UNIT + 2.0,
    right: spacing::SPACE_3,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_3,
  }))
  .padding(0)
  .on_press(Message::FilterSelected(target))
  .style(move |_, _| button::Style {
    background: on.then_some(Background::Color(color::with_alpha(color::accent(), 0.14))),
    border: Border {
      radius: radius::CONTROL.into(),
      ..Border::default()
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn corp_filter_chips<'a>(state: &State, snapshot: &'a Snapshot) -> Element<'a, Message> {
  let total: usize = snapshot.corps.iter().map(|corp| corp.count).sum();
  let mut chips: Vec<Element<'a, Message>> = vec![corp_chip(
    state.corp_filter.is_none(),
    None,
    t!("structure_alerts.filter.all_corps").into_owned(),
    total,
    None,
  )];
  for corp in &snapshot.corps {
    chips.push(corp_chip(
      state.corp_filter == Some(corp.id),
      Some(corp.id),
      corp.ticker.clone(),
      corp.count,
      corp.top_severity,
    ));
  }
  iced::widget::Row::with_children(chips)
    .spacing(spacing::SPACE_2)
    .wrap()
    .into()
}

fn corp_chip<'a>(
  active: bool,
  id: Option<i64>,
  label: String,
  count: usize,
  severity: Option<Severity>,
) -> Element<'a, Message> {
  let mut inner_children: Vec<Element<'a, Message>> = Vec::new();
  if let Some(severity) = severity {
    inner_children.push(status_dot(severity.color()));
  }
  inner_children.push(
    mono_caption(
      label.to_uppercase(),
      if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
    )
    .into(),
  );
  inner_children.push(
    mono_caption(
      count.to_string(),
      if active {
        color::accent()
      } else {
        color::text::tertiary()
      },
    )
    .into(),
  );

  let inner = Row::with_children(inner_children)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  button(container(inner).padding(Padding {
    top: spacing::UNIT + 1.0,
    right: spacing::SPACE_2_5 + 1.0,
    bottom: spacing::UNIT + 1.0,
    left: spacing::SPACE_2_5 + 1.0,
  }))
  .padding(0)
  .on_press(Message::CorpFilterSelected(id))
  .style(move |_, _| button::Style {
    background: active.then_some(Background::Color(color::with_alpha(color::accent(), 0.14))),
    border: Border {
      color: if active {
        color::with_alpha(color::accent(), 0.5)
      } else {
        color::rule()
      },
      radius: PILL_RADIUS.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn alert_banner<'a>(alerts: &[&'a StructureRow]) -> Element<'a, Message> {
  let top = top_severity(alerts.iter().copied()).unwrap_or(Severity::Warning);
  let color = top.color();

  let mut counts: HashMap<u8, usize> = HashMap::new();
  for row in alerts {
    if let Some(alert) = row.alert.as_ref() {
      *counts.entry(alert.severity.rank()).or_default() += 1;
    }
  }
  let mut tally: Vec<Element<'a, Message>> = Vec::new();
  for severity in [Severity::Critical, Severity::Warning, Severity::Info] {
    if let Some(count) = counts.get(&severity.rank()).copied().filter(|count| *count > 0) {
      tally.push(banner_count(severity, count));
    }
  }

  let head = Row::with_children(vec![
    icon_tile(
      Icon::alert_triangle().size(22.0).color(color).render::<Message>(),
      BANNER_ICON_TILE,
      color::with_alpha(color, 0.16),
      color::with_alpha(color, 0.5),
    ),
    Column::with_children(vec![
      body_text(
        t!("structure_alerts.banner.title", count => alerts.len().to_string()).into_owned(),
        17.0,
        color::text::PRIMARY,
      )
      .into(),
      Row::with_children(tally).spacing(spacing::SPACE_3_5).into(),
    ])
    .spacing(5.0)
    .width(Length::Fill)
    .into(),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center);

  let mut children: Vec<Element<'a, Message>> = vec![
    container(head)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_3 + 1.0,
        right: spacing::SPACE_4_5,
        bottom: spacing::SPACE_3 + 1.0,
        left: spacing::SPACE_4_5,
      })
      .into(),
  ];
  for row in alerts.iter().copied() {
    children.push(alert_row(row));
  }

  container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(color, 0.06))),
      border: Border {
        color: color::with_alpha(color, 0.5),
        radius: radius::NAV_CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn banner_count<'a>(severity: Severity, count: usize) -> Element<'a, Message> {
  Row::with_children(vec![
    status_dot(severity.color()),
    mono_caption(count.to_string(), color::text::PRIMARY).into(),
    mono_caption(severity.label().to_uppercase(), color::text::secondary()).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn alert_row<'a>(row: &'a StructureRow) -> Element<'a, Message> {
  let alert = row.alert.as_ref().expect("alert_row called on a nominal structure");
  let color = alert.severity.color();

  let identity = Column::with_children(vec![
    Row::with_children(vec![
      body_text(row.name.clone(), 14.5, color::text::PRIMARY).into(),
      mono_caption(
        format!("{} \u{b7} {}", row.type_name, row.system).to_uppercase(),
        color::text::tertiary(),
      )
      .into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .into(),
    body_text(alert.label.clone(), 12.5, color).into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = vec![structure_tile(row, 38.0), identity.into()];
  if let Some(timer) = timer_readout(alert) {
    children.push(timer);
  }
  children.push(
    Icon::chevron_right()
      .size(20.0)
      .color(color::text::secondary())
      .render(),
  );

  let inner = Row::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center);

  button(container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_4_5,
  }))
  .padding(0)
  .width(Length::Fill)
  .on_press(Message::OpenStructure(row.id))
  .style(|_, _| button::Style {
    background: None,
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn card_grid<'a>(rows: &[&'a StructureRow]) -> Element<'a, Message> {
  let cards: Vec<Element<'a, Message>> = rows.iter().copied().map(structure_card).collect();
  iced::widget::Row::with_children(cards)
    .spacing(spacing::SPACE_3_5)
    .wrap()
    .into()
}

fn structure_card<'a>(row: &'a StructureRow) -> Element<'a, Message> {
  let meta = row.alert.as_ref().map(|alert| alert.severity);
  let border = meta.map_or(color::rule(), |severity| color::with_alpha(severity.color(), 0.5));

  let mut head_children: Vec<Element<'a, Message>> = vec![
    structure_tile(row, CARD_ICON_TILE),
    Column::with_children(vec![
      body_text(row.name.clone(), 16.0, color::text::PRIMARY).into(),
      Row::with_children(vec![
        mono_caption(
          format!("{} \u{b7} {}", row.type_name, row.category).to_uppercase(),
          color::text::secondary(),
        )
        .into(),
        corp_ticker_pill(row.corp_ticker.clone()),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    ])
    .spacing(4.0)
    .width(Length::Fill)
    .into(),
  ];
  if row.stale {
    head_children.push(Icon::alert_triangle().size(16.0).color(color::status::WARNING).render());
  }
  let head = Row::with_children(head_children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  let location = Row::with_children(vec![
    body_text(row.system.clone(), 12.5, color::text::secondary()).into(),
    Space::new().width(Length::Fill).into(),
    security_pill(row.security),
    mono_caption(row.region.clone(), color::text::tertiary()).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let access = mono_caption(
    format!("{} \u{b7} {}", row.access_char, row.access_role),
    color::text::tertiary(),
  );

  let mut children: Vec<Element<'a, Message>> = vec![head.into(), location.into(), access.into(), card_status(row)];
  if !row.is_poco
    && let Some(days) = row.fuel_days
  {
    children.push(fuel_footer(days));
  }

  let card = container(
    Column::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3_5)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: border,
      radius: radius::NAV_CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  button(card)
    .padding(0)
    .width(Length::Fixed(CARD_MIN_WIDTH))
    .on_press(Message::OpenStructure(row.id))
    .style(|_, _| button::Style {
      background: None,
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into()
}

fn card_status<'a>(row: &'a StructureRow) -> Element<'a, Message> {
  match row.alert.as_ref() {
    Some(alert) => {
      let color = alert.severity.color();
      let mut children: Vec<Element<'a, Message>> = vec![
        severity_pill(alert.severity),
        body_text(alert.label.clone(), 12.5, color).into(),
      ];
      if let Some(timer) = row
        .alert
        .as_ref()
        .and_then(|alert| alert.timer_target)
        .map(fmt_countdown)
      {
        children.push(Space::new().width(Length::Fill).into());
        children.push(mono_value(timer, color));
      }
      container(
        Row::with_children(children)
          .spacing(spacing::SPACE_2_5)
          .align_y(Vertical::Center),
      )
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2_5,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
      })
      .into()
    }
    None => {
      let trailing = if row.is_poco {
        mono_caption(
          t!(
            "structure_alerts.card.poco_tax",
            pct => format_pct(row.tax_corp)
          )
          .to_uppercase(),
          color::text::tertiary(),
        )
      } else {
        let online = row.services.iter().filter(|service| service.online).count();
        mono_caption(
          t!(
            "structure_alerts.card.services",
            online => online.to_string(),
            total => row.services.len().to_string()
          )
          .to_uppercase(),
          color::text::tertiary(),
        )
      };
      Row::with_children(vec![
        status_dot(color::status::ONLINE),
        mono_caption(
          t!("structure_alerts.status.nominal").to_uppercase(),
          color::status::ONLINE,
        )
        .into(),
        Space::new().width(Length::Fill).into(),
        trailing.into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into()
    }
  }
}

fn fuel_footer<'a>(days: f64) -> Element<'a, Message> {
  let color = fuel_color(days);
  let head = Row::with_children(vec![
    mono_caption(
      t!("structure_alerts.detail.fuel").to_uppercase(),
      color::text::secondary(),
    )
    .into(),
    Space::new().width(Length::Fill).into(),
    mono_value(fmt_fuel_days(days), color),
  ])
  .align_y(Vertical::Center);

  Column::with_children(vec![head.into(), fuel_bar(days, color)])
    .spacing(spacing::SPACE_2)
    .into()
}

fn detail_view(row: &StructureRow) -> Element<'_, Message> {
  let back = button(
    container(
      Row::with_children(vec![
        Icon::chevron_left().size(16.0).color(color::text::secondary()).render(),
        mono_caption(
          t!("structure_alerts.detail.back").to_uppercase(),
          color::text::secondary(),
        )
        .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3,
      bottom: 0.0,
      left: spacing::SPACE_2_5,
    })
    .height(Length::Fixed(BACK_BUTTON_HEIGHT))
    .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(Message::Back)
  .style(outline_button_style);

  let mut identity_children: Vec<Element<'_, Message>> = vec![
    structure_tile(row, DETAIL_ICON_TILE),
    Column::with_children(vec![
      body_text(row.name.clone(), 27.0, color::text::PRIMARY).into(),
      Row::with_children(vec![
        mono_caption(
          format!("{} \u{b7} {}", row.type_name, row.category).to_uppercase(),
          color::text::secondary(),
        )
        .into(),
        body_text(row.system.clone(), 13.0, color::text::secondary()).into(),
        security_pill(row.security),
        mono_caption(row.region.clone(), color::text::tertiary()).into(),
      ])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center)
      .into(),
    ])
    .spacing(7.0)
    .width(Length::Fill)
    .into(),
  ];
  if row.stale {
    identity_children.push(stale_badge());
  }
  let identity = Row::with_children(identity_children)
    .spacing(spacing::SPACE_4_5)
    .align_y(Vertical::Center);

  let mut children: Vec<Element<'_, Message>> = vec![back.into(), identity.into()];
  if let Some(alert) = row.alert.as_ref() {
    children.push(alert_hero(alert));
  }
  children.extend(detail_panels(row));

  Column::with_children(children).spacing(spacing::SPACE_4_5).into()
}

fn alert_hero<'a>(alert: &'a Alert) -> Element<'a, Message> {
  let color = alert.severity.color();
  let mut children: Vec<Element<'a, Message>> = vec![
    icon_tile(
      alert_icon(alert.kind).size(24.0).color(color).render::<Message>(),
      44.0,
      color::with_alpha(color, 0.16),
      color::with_alpha(color, 0.5),
    ),
    Column::with_children(vec![
      severity_pill(alert.severity),
      body_text(alert.label.clone(), 19.0, color::text::PRIMARY).into(),
      body_text(alert.detail.clone(), 13.5, color::text::secondary()).into(),
    ])
    .spacing(8.0)
    .width(Length::Fill)
    .into(),
  ];
  if let Some(timer) = timer_readout(alert) {
    children.push(timer);
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_4_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_4_5)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(color, 0.08))),
    border: Border {
      color: color::with_alpha(color, 0.55),
      radius: radius::NAV_CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn detail_panels(row: &StructureRow) -> Vec<Element<'_, Message>> {
  let mut panels: Vec<Element<'_, Message>> = Vec::new();

  if row.is_poco {
    panels.push(panel(
      t!("structure_alerts.detail.tax_rates").into_owned(),
      Row::with_children(vec![
        big_stat(
          t!("structure_alerts.detail.tax_corp").into_owned(),
          format_pct(row.tax_corp),
          color::text::PRIMARY,
        ),
        big_stat(
          t!("structure_alerts.detail.tax_alliance").into_owned(),
          format_pct(row.tax_alliance),
          color::text::PRIMARY,
        ),
        big_stat(
          t!("structure_alerts.detail.tax_standing").into_owned(),
          format_pct(row.tax_standing),
          color::status::WARNING,
        ),
      ])
      .spacing(spacing::SPACE_4_5)
      .into(),
    ));
  } else {
    if let Some(days) = row.fuel_days {
      panels.push(panel(
        t!("structure_alerts.detail.fuel_power").into_owned(),
        fuel_footer(days),
      ));
    }
    if !row.services.is_empty() {
      let online = row.services.iter().filter(|service| service.online).count();
      let mut rows: Vec<Element<'_, Message>> = Vec::new();
      for service in &row.services {
        rows.push(service_row(service));
      }
      panels.push(panel_with_meta(
        t!("structure_alerts.detail.services").into_owned(),
        Some(
          t!(
            "structure_alerts.detail.services_meta",
            online => online.to_string(),
            total => row.services.len().to_string()
          )
          .into_owned(),
        ),
        Column::with_children(rows).into(),
      ));
    }
  }

  if let Some(window) = row.reinforce_window.clone() {
    panels.push(panel(
      t!("structure_alerts.detail.vulnerability").into_owned(),
      key_value(
        t!("structure_alerts.detail.window").into_owned(),
        window,
        color::text::PRIMARY,
      ),
    ));
  }

  panels.push(panel(
    t!("structure_alerts.detail.access").into_owned(),
    Column::with_children(vec![
      key_value(
        t!("structure_alerts.detail.visible_via").into_owned(),
        row.access_char.clone(),
        color::accent(),
      ),
      key_value(
        t!("structure_alerts.detail.your_access").into_owned(),
        row.access_role.clone(),
        color::text::PRIMARY,
      ),
    ])
    .into(),
  ));

  panels
}

fn service_row<'a>(service: &'a ServiceRow) -> Element<'a, Message> {
  let color = if service.online {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let state = if service.online {
    t!("structure_alerts.detail.online").into_owned()
  } else {
    t!("structure_alerts.detail.offline").into_owned()
  };
  container(
    Row::with_children(vec![
      status_dot(color),
      body_text(service.name.clone(), 14.0, color::text::PRIMARY).into(),
      Space::new().width(Length::Fill).into(),
      mono_caption(state.to_uppercase(), color).into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2_5,
    right: 0.0,
    bottom: spacing::SPACE_2_5,
    left: 0.0,
  })
  .into()
}

fn empty_state<'a>(no_structures: bool) -> Element<'a, Message> {
  let (icon, title, sub) = if no_structures {
    (
      Icon::facilities(),
      t!("structure_alerts.empty.none_title").into_owned(),
      t!("structure_alerts.empty.none_sub").into_owned(),
    )
  } else {
    (
      Icon::check(),
      t!("structure_alerts.empty.clear_title").into_owned(),
      t!("structure_alerts.empty.clear_sub").into_owned(),
    )
  };
  let color = if no_structures {
    color::text::secondary()
  } else {
    color::status::ONLINE
  };

  container(
    Column::with_children(vec![
      icon.size(30.0).color(color).render(),
      body_text(title, 15.0, color::text::PRIMARY).into(),
      mono_caption(sub.to_uppercase(), color::text::secondary()).into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 60.0,
    right: spacing::SPACE_4_5,
    bottom: 60.0,
    left: spacing::SPACE_4_5,
  })
  .align_x(Horizontal::Center)
  .into()
}

fn starting_up<'a>() -> Element<'a, Message> {
  container(mono_caption(
    t!("structure_alerts.loading").to_uppercase(),
    color::text::secondary(),
  ))
  .width(Length::Fill)
  .padding(60.0)
  .align_x(Horizontal::Center)
  .into()
}

fn structure_tile<'a>(row: &'a StructureRow, size: f32) -> Element<'a, Message> {
  let tint = row
    .alert
    .as_ref()
    .map_or(color::text::secondary(), |alert| alert.severity.color());
  match &row.icon {
    IconResolution::Found(path) => icon_tile(
      image(image::Handle::from_path(path.clone()))
        .width(Length::Fixed(size * 0.72))
        .height(Length::Fixed(size * 0.72))
        .content_fit(ContentFit::Contain)
        .into(),
      size,
      color::with_alpha(tint, 0.1),
      color::with_alpha(tint, 0.3),
    ),
    IconResolution::Missing => icon_tile(
      Icon::facilities().size(size * 0.52).color(tint).render::<Message>(),
      size,
      color::with_alpha(tint, 0.1),
      color::with_alpha(tint, 0.3),
    ),
  }
}

fn icon_tile<'a>(
  content: Element<'a, Message>,
  size: f32,
  background: iced::Color,
  border: iced::Color,
) -> Element<'a, Message> {
  container(content)
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn severity_pill<'a>(severity: Severity) -> Element<'a, Message> {
  let color = severity.color();
  container(
    Row::with_children(vec![
      status_dot(color),
      mono_caption(severity.label().to_uppercase(), color).into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    right: spacing::SPACE_2_5,
    bottom: 3.0,
    left: spacing::SPACE_2_5,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(color, 0.14))),
    border: Border {
      color: color::with_alpha(color, 0.45),
      radius: PILL_RADIUS.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn security_pill<'a>(security: Option<f64>) -> Element<'a, Message> {
  let (label, color) = match security {
    Some(sec) if sec > 0.45 => (format!("{sec:.1}"), color::status::ONLINE),
    Some(sec) if sec > 0.0 => (format!("{sec:.1}"), color::status::WARNING),
    Some(sec) => (format!("{sec:.1}"), color::status::DANGER),
    None => ("\u{2014}".to_owned(), color::text::tertiary()),
  };
  mono_value(label, color)
}

fn corp_ticker_pill<'a>(ticker: String) -> Element<'a, Message> {
  container(mono_caption(ticker, color::accent()))
    .padding(Padding {
      top: 1.0,
      right: spacing::SPACE_2,
      bottom: 1.0,
      left: spacing::SPACE_2,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent(), 0.1))),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn stale_badge<'a>() -> Element<'a, Message> {
  container(
    Row::with_children(vec![
      Icon::alert_triangle().size(13.0).color(color::status::WARNING).render(),
      mono_caption(
        t!("structure_alerts.detail.stale").to_uppercase(),
        color::status::WARNING,
      )
      .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::UNIT + 2.0,
    right: spacing::SPACE_2_5,
    bottom: spacing::UNIT + 2.0,
    left: spacing::SPACE_2_5,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.12))),
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.4),
      radius: PILL_RADIUS.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn timer_readout<'a>(alert: &'a Alert) -> Option<Element<'a, Message>> {
  let target = alert.timer_target?;
  let label = alert.timer_label.clone().unwrap_or_default();
  Some(
    Column::with_children(vec![
      mono_caption(label.to_uppercase(), color::text::tertiary()).into(),
      mono_value(fmt_countdown(target), alert.severity.color()),
    ])
    .spacing(4.0)
    .align_x(Horizontal::Right)
    .into(),
  )
}

fn panel(title: String, body: Element<'_, Message>) -> Element<'_, Message> {
  panel_with_meta(title, None, body)
}

fn panel_with_meta(title: String, meta: Option<String>, body: Element<'_, Message>) -> Element<'_, Message> {
  let mut head_children: Vec<Element<'_, Message>> =
    vec![mono_caption(title.to_uppercase(), color::text::secondary()).into()];
  if let Some(meta) = meta {
    head_children.push(Space::new().width(Length::Fill).into());
    head_children.push(mono_caption(meta.to_uppercase(), color::text::tertiary()).into());
  }
  let head = Row::with_children(head_children).align_y(Vertical::Center);

  container(
    Column::with_children(vec![head.into(), body])
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_4_5)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule(),
      radius: radius::NAV_CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn key_value(label: String, value: String, value_color: iced::Color) -> Element<'static, Message> {
  container(
    Row::with_children(vec![
      mono_caption(label.to_uppercase(), color::text::secondary()).into(),
      Space::new().width(Length::Fill).into(),
      body_text(value, 13.5, value_color).into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: 0.0,
    bottom: spacing::SPACE_2,
    left: 0.0,
  })
  .into()
}

fn big_stat(label: String, value: String, value_color: iced::Color) -> Element<'static, Message> {
  Column::with_children(vec![
    mono_caption(label.to_uppercase(), color::text::secondary()).into(),
    text(value)
      .font(typography::mono::REGULAR)
      .size(24.0)
      .style(move |_| text::Style {
        color: Some(value_color),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn head_stat<'a>(label: String, value: String, value_color: iced::Color, sub: Option<String>) -> Element<'a, Message> {
  let mut value_children: Vec<Element<'a, Message>> = vec![
    text(value)
      .font(typography::mono::REGULAR)
      .size(16.0)
      .style(move |_| text::Style {
        color: Some(value_color),
      })
      .into(),
  ];
  if let Some(sub) = sub {
    value_children.push(mono_caption(sub.to_uppercase(), color::text::secondary()).into());
  }
  Column::with_children(vec![
    mono_caption(label.to_uppercase(), color::text::secondary()).into(),
    Row::with_children(value_children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Bottom)
      .into(),
  ])
  .spacing(4.0)
  .into()
}

fn head_divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fixed(44.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn status_dot<'a>(color: iced::Color) -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(7.0))
    .height(Length::Fixed(7.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color)),
      border: Border {
        radius: PILL_RADIUS.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn fuel_bar<'a>(days: f64, color: iced::Color) -> Element<'a, Message> {
  let pct = (days / FUEL_WINDOW_DAYS).clamp(0.0, 1.0);
  let fill = container(Space::new())
    .width(Length::FillPortion(((pct * 1000.0) as u16).max(1)))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color)),
      border: Border {
        radius: (FUEL_BAR_HEIGHT / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let rest = Space::new().width(Length::FillPortion((((1.0 - pct) * 1000.0) as u16).max(1)));

  container(Row::with_children(vec![fill.into(), rest.into()]).width(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(FUEL_BAR_HEIGHT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
      border: Border {
        radius: (FUEL_BAR_HEIGHT / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn outline_button_style(_: &iced::Theme, status: button::Status) -> button::Style {
  let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: None,
    border: Border {
      color: if hover { color::rule_strong() } else { color::rule() },
      radius: radius::NAV_CARD.into(),
      width: 1.0,
    },
    text_color: if hover {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    },
    ..button::Style::default()
  }
}

fn body_text<'a>(value: String, size: f32, text_color: iced::Color) -> iced::widget::Text<'a> {
  text(value)
    .font(typography::body::REGULAR)
    .size(size)
    .style(move |_| text::Style {
      color: Some(text_color),
    })
}

fn mono_caption<'a>(value: String, text_color: iced::Color) -> iced::widget::Text<'a> {
  text(value)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(text_color),
    })
}

fn mono_value<'a>(value: String, text_color: iced::Color) -> Element<'a, Message> {
  text(value)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(text_color),
    })
    .into()
}

fn alert_icon(kind: AlertKind) -> Icon {
  match kind {
    AlertKind::Anchoring => Icon::clock(),
    AlertKind::Fuel => Icon::alert_triangle(),
    AlertKind::Reinforced => Icon::alert_triangle(),
    AlertKind::Service => Icon::facilities(),
  }
}

fn access_pilot_count(state: &State) -> usize {
  state
    .scoped_structures()
    .iter()
    .map(|row| row.access_char.as_str())
    .filter(|name| *name != "\u{2014}")
    .collect::<HashSet<&str>>()
    .len()
}

fn pilot_word(count: usize) -> String {
  if count == 1 {
    t!("structure_alerts.stat.pilot").into_owned()
  } else {
    t!("structure_alerts.stat.pilots").into_owned()
  }
}

fn top_severity<'a>(rows: impl Iterator<Item = &'a StructureRow>) -> Option<Severity> {
  rows
    .filter_map(|row| row.alert.as_ref().map(|alert| alert.severity))
    .max()
}

fn fuel_color(days: f64) -> iced::Color {
  if days <= 0.0 {
    color::status::DANGER
  } else if days < FUEL_LOW_DAYS {
    color::status::WARNING
  } else if days < FUEL_WARN_DAYS {
    color::accent()
  } else {
    color::status::ONLINE
  }
}

fn format_pct(rate: Option<f64>) -> String {
  match rate {
    Some(rate) => format!("{}%", (rate * 100.0).round() as i64),
    None => "\u{2014}".to_owned(),
  }
}

fn fmt_countdown(target: DateTime<Utc>) -> String {
  let seconds = (target - Utc::now()).num_seconds();
  let past = seconds < 0;
  let total = seconds.abs();
  let days = total / 86_400;
  let hours = (total % 86_400) / 3_600;
  let minutes = (total % 3_600) / 60;
  let value = if days > 0 {
    format!("{days}d {hours}h {minutes:02}m")
  } else if hours > 0 {
    format!("{hours}h {minutes:02}m")
  } else {
    format!("{minutes}m")
  };
  if past {
    t!("structure_alerts.timer_ago", time => value).into_owned()
  } else {
    value
  }
}

fn fmt_fuel_days(days: f64) -> String {
  if days <= 0.0 {
    t!("structure_alerts.fuel_empty").into_owned()
  } else if days < 1.0 {
    format!("{}h", (days * 24.0).round() as i64)
  } else {
    format!("{days:.1}d")
  }
}

fn parse_time(value: Option<&str>) -> Option<DateTime<Utc>> {
  value.and_then(|raw| {
    DateTime::parse_from_rfc3339(raw)
      .ok()
      .map(|parsed| parsed.with_timezone(&Utc))
  })
}

fn fuel_days_from(fuel_expires: Option<&str>, now: DateTime<Utc>) -> Option<f64> {
  parse_time(fuel_expires).map(|target| (target - now).num_seconds() as f64 / 86_400.0)
}

fn derive_alert(
  state: Option<&str>,
  fuel_days: Option<f64>,
  services_offline: bool,
  timer_target: Option<DateTime<Utc>>,
) -> Option<Alert> {
  let state_lower = state.map(str::to_lowercase);
  let reinforced = state_lower.as_deref().is_some_and(|value| value.contains("reinforce"));
  let anchoring = state_lower
    .as_deref()
    .is_some_and(|value| value.contains("anchoring") || value.contains("unanchoring"));

  if reinforced {
    return Some(Alert {
      detail: t!("structure_alerts.alert.reinforced_detail").into_owned(),
      kind: AlertKind::Reinforced,
      label: t!("structure_alerts.alert.reinforced").into_owned(),
      severity: Severity::Critical,
      timer_label: Some(t!("structure_alerts.alert.timer_exits").into_owned()),
      timer_target,
    });
  }

  if let Some(days) = fuel_days {
    if days <= 0.0 {
      return Some(Alert {
        detail: t!("structure_alerts.alert.fuel_empty_detail").into_owned(),
        kind: AlertKind::Fuel,
        label: t!("structure_alerts.alert.fuel_empty").into_owned(),
        severity: Severity::Critical,
        timer_label: None,
        timer_target: None,
      });
    }
    if days < FUEL_LOW_DAYS {
      return Some(Alert {
        detail: t!("structure_alerts.alert.fuel_low_detail").into_owned(),
        kind: AlertKind::Fuel,
        label: t!("structure_alerts.alert.fuel_low", time => fmt_fuel_days(days)).into_owned(),
        severity: Severity::Warning,
        timer_label: Some(t!("structure_alerts.alert.timer_fuel").into_owned()),
        timer_target,
      });
    }
  }

  if services_offline {
    return Some(Alert {
      detail: t!("structure_alerts.alert.service_detail").into_owned(),
      kind: AlertKind::Service,
      label: t!("structure_alerts.alert.service").into_owned(),
      severity: Severity::Warning,
      timer_label: None,
      timer_target: None,
    });
  }

  if anchoring {
    return Some(Alert {
      detail: t!("structure_alerts.alert.anchoring_detail").into_owned(),
      kind: AlertKind::Anchoring,
      label: t!("structure_alerts.alert.anchoring").into_owned(),
      severity: Severity::Info,
      timer_label: Some(t!("structure_alerts.alert.timer_completes").into_owned()),
      timer_target,
    });
  }

  None
}

fn sort_loudest_first(rows: &mut [StructureRow]) {
  rows.sort_by(|a, b| {
    let ra = a.alert.as_ref().map_or(0, |alert| alert.severity.rank());
    let rb = b.alert.as_ref().map_or(0, |alert| alert.severity.rank());
    rb.cmp(&ra)
      .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
  });
}

fn reinforce_window(hour: Option<i64>) -> Option<String> {
  hour.map(|hour| {
    let end = (hour + 2) % 24;
    format!("{hour:02}:00 \u{2013} {end:02}:00 EVE")
  })
}

async fn load_snapshot(db: &Database, scope: Option<i64>) -> Snapshot {
  let owned = org::all_owned_corporations(db).await.unwrap_or_default();
  let corp_meta: HashMap<i64, (String, String, Option<i64>)> = owned
    .iter()
    .map(|corp| {
      (
        corp.id(),
        (corp.name().clone(), corp.ticker().clone(), corp.authorized_by()),
      )
    })
    .collect();

  let pilot_names: HashMap<i64, String> = character::all_owned(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|character| (character.id(), character.name().clone()))
    .collect();

  let store = images::default_store();
  let icons = store.icon_index();
  let now = Utc::now();

  let mut rows: Vec<StructureRow> = Vec::new();

  let facilities = industry::accessible_facilities(db).await.unwrap_or_default();
  let mut type_ids: HashSet<i64> = facilities.iter().filter_map(|facility| facility.type_id()).collect();
  type_ids.insert(CUSTOMS_OFFICE_TYPE_ID);
  let type_lookup = type_catalog(db, &type_ids).await;

  for facility in facilities {
    let Some(corp_id) = facility.owner_id() else {
      continue;
    };
    if scope.is_some_and(|id| id != corp_id) {
      continue;
    }
    let state = crate::store::repo::structure_state::read(db, facility.id())
      .await
      .ok()
      .flatten();
    let services: Vec<ServiceRow> = state
      .as_ref()
      .map(|state| {
        state
          .service_list()
          .into_iter()
          .map(|service| ServiceRow {
            name: service.name,
            online: service.state.eq_ignore_ascii_case("online"),
          })
          .collect()
      })
      .unwrap_or_default();
    let fuel_days = state
      .as_ref()
      .and_then(|state| fuel_days_from(state.fuel_expires.as_deref(), now));
    let timer_target = state
      .as_ref()
      .and_then(|state| parse_time(state.state_timer_end.as_deref()));
    let services_offline = services.iter().any(|service| !service.online);
    let alert = state
      .as_ref()
      .and_then(|state| derive_alert(state.state.as_deref(), fuel_days, services_offline, timer_target));
    let stale = state.as_ref().is_some_and(|state| is_stale(&state.synced_at, now));
    let reinforce = state.as_ref().and_then(|state| reinforce_window(state.reinforce_hour));

    let (type_name, category) = facility
      .type_id()
      .and_then(|id| type_lookup.get(&id).cloned())
      .unwrap_or_else(|| (t!("structure_alerts.type.upwell").into_owned(), String::new()));
    let (_, corp_ticker, authorized_by) = corp_meta
      .get(&corp_id)
      .cloned()
      .unwrap_or_else(|| (String::new(), String::new(), None));
    let access_char = authorized_by
      .and_then(|id| pilot_names.get(&id).cloned())
      .unwrap_or_else(|| "\u{2014}".to_owned());

    rows.push(StructureRow {
      access_char,
      access_role: t!("structure_alerts.access.role").into_owned(),
      alert,
      category,
      corp_id,
      corp_ticker,
      fuel_days,
      icon: icons.resolve_type_icon(facility.type_id().unwrap_or_default(), None, Size::S64),
      id: facility.id(),
      is_poco: false,
      name: facility.name().clone(),
      region: facility.region().clone().unwrap_or_default(),
      reinforce_window: reinforce,
      security: facility.security_status(),
      services,
      stale,
      system: facility.solar_system().clone().unwrap_or_default(),
      tax_alliance: None,
      tax_corp: None,
      tax_standing: None,
      type_name,
    });
  }

  let offices = match scope {
    Some(id) => customs_office::list_for_corporation(db, id).await.unwrap_or_default(),
    None => customs_office::list(db).await.unwrap_or_default(),
  };
  let poco_icon = icons.resolve_type_icon(CUSTOMS_OFFICE_TYPE_ID, None, Size::S64);
  for office in offices {
    let (security, region, system) = industry::system_geo(db, office.system_id)
      .await
      .unwrap_or((None, None, None));
    let (_, corp_ticker, authorized_by) = corp_meta
      .get(&office.corporation_id)
      .cloned()
      .unwrap_or_else(|| (String::new(), String::new(), None));
    let access_char = authorized_by
      .and_then(|id| pilot_names.get(&id).cloned())
      .unwrap_or_else(|| "\u{2014}".to_owned());
    let stale = is_stale(&office.synced_at, now);

    rows.push(StructureRow {
      access_char,
      access_role: t!("structure_alerts.access.role").into_owned(),
      alert: None,
      category: t!("structure_alerts.category.orbital").into_owned(),
      corp_id: office.corporation_id,
      corp_ticker,
      fuel_days: None,
      icon: poco_icon.clone(),
      id: office.office_id,
      is_poco: true,
      name: system
        .clone()
        .map(|system| t!("structure_alerts.poco_name", system => system).into_owned())
        .unwrap_or_else(|| t!("structure_alerts.type.poco").into_owned()),
      region: region.unwrap_or_default(),
      reinforce_window: reinforce_window(Some(office.reinforce_exit_start)),
      security,
      services: Vec::new(),
      stale,
      system: system.unwrap_or_default(),
      tax_alliance: office.alliance_tax_rate,
      tax_corp: office.corporation_tax_rate,
      tax_standing: office.bad_standing_tax_rate,
      type_name: t!("structure_alerts.type.poco").into_owned(),
    });
  }

  sort_loudest_first(&mut rows);

  let corps = corp_chips(&rows);
  let (scope_name, scope_ticker) = match scope {
    Some(id) => {
      let meta = corp_meta.get(&id);
      (meta.map(|meta| meta.0.clone()), meta.map(|meta| meta.1.clone()))
    }
    None => (None, None),
  };

  Snapshot {
    corps,
    scope_name,
    scope_ticker,
    structures: rows,
  }
}

async fn type_catalog(db: &Database, type_ids: &HashSet<i64>) -> HashMap<i64, (String, String)> {
  let ids: Vec<i64> = type_ids.iter().copied().collect();
  let details = sde::type_details_for(db, &ids).await.unwrap_or_default();
  let group_ids: Vec<i64> = details.iter().map(|(_, _, group_id)| *group_id).collect();
  let groups: HashMap<i64, String> = sde::group_names_for(db, &group_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();
  details
    .into_iter()
    .map(|(id, name, group_id)| (id, (name, groups.get(&group_id).cloned().unwrap_or_default())))
    .collect()
}

fn corp_chips(rows: &[StructureRow]) -> Vec<CorpChip> {
  let mut order: Vec<i64> = Vec::new();
  let mut chips: HashMap<i64, CorpChip> = HashMap::new();
  for row in rows {
    let severity = row.alert.as_ref().map(|alert| alert.severity);
    let entry = chips.entry(row.corp_id).or_insert_with(|| {
      order.push(row.corp_id);
      CorpChip {
        count: 0,
        id: row.corp_id,
        ticker: row.corp_ticker.clone(),
        top_severity: None,
      }
    });
    entry.count += 1;
    entry.top_severity = match (entry.top_severity, severity) {
      (Some(existing), Some(next)) => Some(existing.max(next)),
      (existing, next) => existing.or(next),
    };
  }
  order.into_iter().filter_map(|id| chips.remove(&id)).collect()
}

fn is_stale(synced_at: &str, now: DateTime<Utc>) -> bool {
  parse_time(Some(synced_at)).is_none_or(|target| now - target > ChronoDuration::hours(STATE_STALE_AFTER_HOURS))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn row(id: i64, name: &str, corp_id: i64, alert: Option<Severity>) -> StructureRow {
    StructureRow {
      access_char: "Vex Voronova".to_owned(),
      access_role: "Director".to_owned(),
      alert: alert.map(|severity| Alert {
        detail: "detail".to_owned(),
        kind: AlertKind::Fuel,
        label: "label".to_owned(),
        severity,
        timer_label: None,
        timer_target: None,
      }),
      category: "Citadel".to_owned(),
      corp_id,
      corp_ticker: "COBSY".to_owned(),
      fuel_days: Some(12.0),
      icon: IconResolution::Missing,
      id,
      is_poco: false,
      name: name.to_owned(),
      region: "The Forge".to_owned(),
      reinforce_window: Some("18:00 \u{2013} 20:00 EVE".to_owned()),
      security: Some(0.95),
      services: vec![ServiceRow {
        name: "Market Hub".to_owned(),
        online: true,
      }],
      stale: false,
      system: "Jita".to_owned(),
      tax_alliance: None,
      tax_corp: None,
      tax_standing: None,
      type_name: "Fortizar".to_owned(),
    }
  }

  fn snapshot(structures: Vec<StructureRow>) -> Snapshot {
    let corps = corp_chips(&structures);
    Snapshot {
      corps,
      scope_name: None,
      scope_ticker: None,
      structures,
    }
  }

  fn loaded(structures: Vec<StructureRow>, scope: Option<i64>) -> State {
    let mut state = State::new(scope);
    state.snapshot = Some(snapshot(structures));
    state
  }

  mod derive_alert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_reinforced_state_as_critical() {
      let alert = derive_alert(Some("armor_reinforce"), Some(20.0), false, None).unwrap();

      assert_eq!(alert.severity, Severity::Critical);
      assert_eq!(alert.kind, AlertKind::Reinforced);
    }

    #[test]
    fn it_flags_empty_fuel_as_critical() {
      let alert = derive_alert(Some("shield_vulnerable"), Some(0.0), false, None).unwrap();

      assert_eq!(alert.severity, Severity::Critical);
      assert_eq!(alert.kind, AlertKind::Fuel);
    }

    #[test]
    fn it_flags_low_fuel_as_warning() {
      let alert = derive_alert(Some("shield_vulnerable"), Some(1.5), false, None).unwrap();

      assert_eq!(alert.severity, Severity::Warning);
    }

    #[test]
    fn it_flags_an_offline_service_as_warning() {
      let alert = derive_alert(Some("shield_vulnerable"), Some(20.0), true, None).unwrap();

      assert_eq!(alert.severity, Severity::Warning);
      assert_eq!(alert.kind, AlertKind::Service);
    }

    #[test]
    fn it_flags_anchoring_as_info() {
      let alert = derive_alert(Some("anchoring"), Some(20.0), false, None).unwrap();

      assert_eq!(alert.severity, Severity::Info);
    }

    #[test]
    fn it_returns_none_for_a_nominal_structure() {
      assert_eq!(derive_alert(Some("shield_vulnerable"), Some(20.0), false, None), None);
    }

    #[test]
    fn it_prefers_the_loudest_signal() {
      let alert = derive_alert(Some("hull_reinforce"), Some(0.0), true, None).unwrap();

      assert_eq!(alert.severity, Severity::Critical);
      assert_eq!(alert.kind, AlertKind::Reinforced);
    }
  }

  mod sort_loudest_first {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_orders_by_severity_then_name() {
      let mut rows = vec![
        row(1, "Zeta", 1, None),
        row(2, "Alpha", 1, Some(Severity::Warning)),
        row(3, "Beta", 1, Some(Severity::Critical)),
        row(4, "Gamma", 1, None),
      ];

      sort_loudest_first(&mut rows);

      assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        vec!["Beta", "Alpha", "Gamma", "Zeta"]
      );
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn db() -> Database {
      crate::store::open_test().await.unwrap()
    }

    #[tokio::test]
    async fn it_opens_and_closes_the_detail() {
      let db = db().await;
      let mut state = loaded(vec![row(1, "Cobalt Keep", 1, Some(Severity::Critical))], None);

      let _ = update(&mut state, Message::OpenStructure(1), &db);
      assert_eq!(state.open, Some(1));

      let _ = update(&mut state, Message::Back, &db);
      assert_eq!(state.open, None);
    }

    #[tokio::test]
    async fn it_switches_filters_and_corp_scope() {
      let db = db().await;
      let mut state = loaded(vec![row(1, "Cobalt Keep", 1, Some(Severity::Critical))], None);

      let _ = update(&mut state, Message::FilterSelected(Filter::Alerts), &db);
      assert_eq!(state.filter, Filter::Alerts);

      let _ = update(&mut state, Message::CorpFilterSelected(Some(1)), &db);
      assert_eq!(state.corp_filter, Some(1));
    }

    #[tokio::test]
    async fn it_drops_a_corp_filter_absent_from_a_new_snapshot() {
      let db = db().await;
      let mut state = loaded(vec![row(1, "Cobalt Keep", 1, None)], None);
      state.corp_filter = Some(99);

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(snapshot(vec![row(2, "Hek Roost", 3, None)]))),
        &db,
      );

      assert_eq!(state.corp_filter, None);
    }
  }

  mod escape_dismiss {
    use super::*;

    #[test]
    fn it_dismisses_an_open_detail() {
      let mut state = loaded(vec![row(1, "Cobalt Keep", 1, None)], None);
      state.open = Some(1);

      assert!(matches!(escape_dismiss(&state), Some(Message::Back)));
    }

    #[test]
    fn it_does_nothing_on_the_list() {
      let state = loaded(vec![row(1, "Cobalt Keep", 1, None)], None);

      assert!(escape_dismiss(&state).is_none());
    }
  }

  mod message {
    use super::*;

    #[test]
    fn it_reports_data_loading_messages() {
      assert!(Message::Loaded(Box::new(snapshot(Vec::new()))).loads_data());
      assert!(!Message::Back.loads_data());
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_loading_state() {
      let state = State::new(None);

      let _view: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_cross_corp_list_with_alerts() {
      let state = loaded(
        vec![
          row(1, "Cobalt Keep", 1, Some(Severity::Critical)),
          row(2, "Sakht Drillhead", 1, Some(Severity::Warning)),
          row(3, "Hek Roost", 3, None),
        ],
        None,
      );

      let _view: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_alerts_filter() {
      let mut state = loaded(vec![row(1, "Cobalt Keep", 1, Some(Severity::Critical))], None);
      state.filter = Filter::Alerts;

      let _view: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_detail_view() {
      let mut poco = row(4, "Jita IV \u{b7} Customs", 1, None);
      poco.is_poco = true;
      poco.tax_corp = Some(0.05);
      poco.services = Vec::new();
      let mut state = loaded(vec![row(1, "Cobalt Keep", 1, Some(Severity::Critical)), poco], None);
      state.open = Some(1);

      let _view: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_poco_detail_view() {
      let mut poco = row(4, "Jita IV \u{b7} Customs", 1, None);
      poco.is_poco = true;
      poco.tax_corp = Some(0.05);
      poco.tax_alliance = Some(0.02);
      poco.tax_standing = Some(0.1);
      poco.services = Vec::new();
      let mut state = loaded(vec![poco], None);
      state.open = Some(4);

      let _view: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_corp_locked_header() {
      let mut state = loaded(vec![row(1, "Cobalt Keep", 1, Some(Severity::Warning))], Some(1));
      state.snapshot.as_mut().unwrap().scope_name = Some("Cobalt Syndicate".to_owned());
      state.snapshot.as_mut().unwrap().scope_ticker = Some("COBSY".to_owned());

      let _view: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_empty_state() {
      let state = loaded(Vec::new(), None);

      let _view: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_counts_access_pilots_from_the_scoped_set() {
      let state = loaded(vec![row(1, "Cobalt Keep", 1, None)], None);

      assert_eq!(access_pilot_count(&state), 1);
    }
  }
}
