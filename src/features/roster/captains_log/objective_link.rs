use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text, text_input},
};

use super::Message as Parent;
use crate::{
  store::{
    Database,
    model::{LinkSource, NewObjective, Objective, ObjectiveLink, ObjectiveStatus},
    repo::objective,
  },
  ui::{
    components::{
      button::{Button, Size},
      eyebrow::eyebrow_text,
      icon::Icon,
    },
    style::{color, radius, spacing, typography},
  },
};

const CARD_MAX_WIDTH: f32 = 360.0;
const PILL_RADIUS: f32 = 999.0;
const ROW_TILE: f32 = 24.0;
const PANEL_TILE: f32 = 30.0;

#[derive(Clone, Debug)]
pub enum Message {
  Toggle {
    date: String,
    source: LinkSource,
  },
  Pick {
    date: String,
    source: LinkSource,
    objective: i64,
  },
  Clear {
    date: String,
    source: LinkSource,
  },
  StartCreate,
  DraftChanged(String),
  CancelCreate,
  Create {
    date: String,
    source: LinkSource,
  },
  OpenBoard(Option<i64>),
  Reloaded(Box<Data>),
}

#[derive(Clone, Debug)]
pub struct ObjectiveOption {
  pub id: i64,
  pub title: String,
  pub accent: String,
  pub status: ObjectiveStatus,
}

pub fn options(models: &[Objective]) -> Vec<ObjectiveOption> {
  models.iter().map(to_option).collect()
}

fn to_option(model: &Objective) -> ObjectiveOption {
  ObjectiveOption {
    id: model.id,
    title: model.title.clone(),
    accent: model.accent.clone(),
    status: ObjectiveStatus::parse(&model.status).unwrap_or_default(),
  }
}

#[derive(Clone, Debug, Default)]
pub struct Data {
  pub objectives: Vec<ObjectiveOption>,
  pub links: Vec<(String, Vec<ObjectiveLink>)>,
}

#[derive(Clone, Debug)]
struct Picker {
  date: String,
  source: LinkSource,
  creating: bool,
  draft: String,
}

#[derive(Debug, Default)]
pub struct State {
  objectives: Vec<ObjectiveOption>,
  links: HashMap<String, Vec<ObjectiveLink>>,
  picker: std::option::Option<Picker>,
}

impl State {
  pub fn set_objectives(&mut self, objectives: Vec<ObjectiveOption>) {
    self.objectives = objectives;
  }

  pub fn set_day_links(&mut self, date: String, links: Vec<ObjectiveLink>) {
    self.links.insert(date, links);
  }

  pub fn apply(&mut self, data: Data) {
    self.objectives = data.objectives;
    for (date, links) in data.links {
      self.links.insert(date, links);
    }
  }

  fn day_links(&self, date: &str) -> &[ObjectiveLink] {
    self.links.get(date).map(Vec::as_slice).unwrap_or(&[])
  }

  fn option(&self, id: i64) -> std::option::Option<&ObjectiveOption> {
    self.objectives.iter().find(|objective| objective.id == id)
  }

  fn active(&self) -> impl Iterator<Item = &ObjectiveOption> {
    self
      .objectives
      .iter()
      .filter(|objective| objective.status == ObjectiveStatus::Active)
  }

  fn linked_id(&self, date: &str, source: &LinkSource) -> std::option::Option<i64> {
    self
      .day_links(date)
      .iter()
      .find(|link| link.source_kind == source.source_kind() && link.source_ref == source.source_ref())
      .map(|link| link.objective_id)
  }

  fn linked_objective(&self, date: &str, source: &LinkSource) -> std::option::Option<&ObjectiveOption> {
    self.linked_id(date, source).and_then(|id| self.option(id))
  }

  fn is_open(&self, date: &str, source: &LinkSource) -> bool {
    self
      .picker
      .as_ref()
      .is_some_and(|picker| picker.date == date && &picker.source == source)
  }

  fn is_creating(&self, date: &str, source: &LinkSource) -> bool {
    self
      .picker
      .as_ref()
      .is_some_and(|picker| picker.creating && picker.date == date && &picker.source == source)
  }

  fn draft(&self) -> &str {
    self.picker.as_ref().map(|picker| picker.draft.as_str()).unwrap_or("")
  }

  fn toggle(&mut self, date: String, source: LinkSource) {
    if self.is_open(&date, &source) {
      self.picker = None;
    } else {
      self.picker = Some(Picker {
        date,
        source,
        creating: false,
        draft: String::new(),
      });
    }
  }

  fn close(&mut self) {
    self.picker = None;
  }

  fn start_create(&mut self) {
    if let Some(picker) = self.picker.as_mut() {
      picker.creating = true;
    }
  }

  fn cancel_create(&mut self) {
    if let Some(picker) = self.picker.as_mut() {
      picker.creating = false;
      picker.draft.clear();
    }
  }

  fn set_draft(&mut self, value: String) {
    if let Some(picker) = self.picker.as_mut() {
      picker.draft = value;
    }
  }

  #[cfg(test)]
  pub(super) fn is_open_for(&self, date: &str, source: &LinkSource) -> bool {
    self.is_open(date, source)
  }

  #[cfg(test)]
  pub(super) fn linked(&self, date: &str, source: &LinkSource) -> std::option::Option<i64> {
    self.linked_id(date, source)
  }
}

pub fn update(state: &mut State, db: &Database, message: Message) -> Task<Parent> {
  match message {
    Message::Toggle {
      date,
      source,
    } => {
      state.toggle(date, source);
      Task::none()
    }
    Message::StartCreate => {
      state.start_create();
      Task::none()
    }
    Message::DraftChanged(value) => {
      state.set_draft(value);
      Task::none()
    }
    Message::CancelCreate => {
      state.cancel_create();
      Task::none()
    }
    Message::Pick {
      date,
      source,
      objective,
    } => apply_link(state, db, date, source, Some(objective)),
    Message::Clear {
      date,
      source,
    } => apply_link(state, db, date, source, None),
    Message::Create {
      date,
      source,
    } => create_and_link(state, db, date, source),
    Message::Reloaded(data) => {
      state.apply(*data);
      Task::none()
    }
    Message::OpenBoard(_) => Task::none(),
  }
}

fn apply_link(
  state: &mut State,
  db: &Database,
  date: String,
  source: LinkSource,
  target: std::option::Option<i64>,
) -> Task<Parent> {
  let existing = state.linked_id(&date, &source);
  state.close();
  let worker = db.clone();
  let reload = Task::perform(
    async move {
      if let Some(id) = existing {
        let _ = objective::clear_link(&worker, id, &date, &source).await;
      }
      if let Some(id) = target {
        let _ = objective::set_link(&worker, id, &date, &source).await;
      }
      Box::new(reload_data(&worker, &date).await)
    },
    |data| Parent::ObjectiveLink(Message::Reloaded(data)),
  );

  Task::batch([reload, board_reload(db.clone())])
}

fn create_and_link(state: &mut State, db: &Database, date: String, source: LinkSource) -> Task<Parent> {
  let title = state
    .picker
    .as_ref()
    .map(|picker| picker.draft.trim().to_owned())
    .unwrap_or_default();
  if title.is_empty() {
    return Task::none();
  }
  state.close();
  let worker = db.clone();
  let create = Task::perform(
    async move {
      let input = NewObjective {
        accent: default_accent(),
        horizon: None,
        target: None,
        title,
        why: None,
      };
      if let Ok(objective) = objective::create(&worker, &input).await {
        let _ = objective::set_link(&worker, objective.id, &date, &source).await;
      }
      Box::new(reload_data(&worker, &date).await)
    },
    |data| Parent::ObjectiveLink(Message::Reloaded(data)),
  );

  Task::batch([create, board_reload(db.clone())])
}

fn board_reload(db: Database) -> Task<Parent> {
  super::standing_orders::load(&db).map(Parent::StandingOrders)
}

pub(super) async fn reload_data(db: &Database, date: &str) -> Data {
  let objectives = objective::list(db, None)
    .await
    .unwrap_or_default()
    .iter()
    .map(to_option)
    .collect();
  let links = objective::links_for_day(db, date).await.unwrap_or_default();
  Data {
    objectives,
    links: vec![(date.to_owned(), links)],
  }
}

fn default_accent() -> String {
  crate::ui::components::color_picker::PALETTE
    .first()
    .map(|preset| preset.hex.to_owned())
    .unwrap_or_else(|| "#3FB8DB".to_owned())
}

// ── View ─────────────────────────────────────────────────────────────────────

pub fn picker<'a>(state: &'a State, date: &str, source: LinkSource, compact: bool) -> Element<'a, Parent> {
  let mut column: Vec<Element<'a, Parent>> = vec![picker_head(state, date, &source, compact)];
  if state.is_open(date, &source) {
    column.push(disclosure(state, date, &source));
  }

  Column::with_children(column)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn picker_head<'a>(state: &State, date: &str, source: &LinkSource, compact: bool) -> Element<'a, Parent> {
  let mut row: Vec<Element<'a, Parent>> = Vec::new();
  if !compact {
    row.push(standing_order_eyebrow());
  }
  match state.linked_objective(date, source) {
    Some(objective) => row.push(linked_pill(date, source, objective)),
    None => row.push(link_button(date, source)),
  }
  if !compact {
    row.push(
      eyebrow_text(
        &t!("captains_log.objective_link.optional"),
        Some(color::text::tertiary()),
      )
      .into(),
    );
  }

  Row::with_children(row)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .into()
}

fn standing_order_eyebrow<'a>() -> Element<'a, Parent> {
  Row::with_children(vec![
    Icon::chevrons_up().size(13.0).color(color::accent()).render(),
    eyebrow_text(&t!("captains_log.objective_link.eyebrow"), Some(color::accent())).into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_y(Vertical::Center)
  .into()
}

fn link_button<'a>(date: &str, source: &LinkSource) -> Element<'a, Parent> {
  Button::ghost(t!("captains_log.objective_link.link_cta").into_owned())
    .size(Size::Sm)
    .icon(Icon::plus())
    .on_press(Parent::ObjectiveLink(Message::Toggle {
      date: date.to_owned(),
      source: source.clone(),
    }))
    .into()
}

fn linked_pill<'a>(date: &str, source: &LinkSource, objective: &ObjectiveOption) -> Element<'a, Parent> {
  let accent = accent_color(&objective.accent);
  let row = Row::with_children(vec![
    Icon::tracker().size(13.0).color(accent).render(),
    text(objective.title.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    icon_button(
      Icon::pencil(),
      Parent::ObjectiveLink(Message::Toggle {
        date: date.to_owned(),
        source: source.clone(),
      }),
    ),
    icon_button(
      Icon::close(),
      Parent::ObjectiveLink(Message::Clear {
        date: date.to_owned(),
        source: source.clone(),
      }),
    ),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(row)
    .padding(Padding {
      top: 4.0,
      right: 8.0,
      bottom: 4.0,
      left: 11.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.12))),
      border: Border {
        color: color::with_alpha(accent, 0.45),
        width: 1.0,
        radius: PILL_RADIUS.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn icon_button<'a>(icon: Icon, message: Parent) -> Element<'a, Parent> {
  button(icon.size(12.0).color(color::text::secondary()).render::<Parent>())
    .padding(4.0)
    .on_press(message)
    .style(|_, _| button::Style {
      background: None,
      ..button::Style::default()
    })
    .into()
}

fn disclosure<'a>(state: &State, date: &str, source: &LinkSource) -> Element<'a, Parent> {
  let mut items: Vec<Element<'a, Parent>> =
    vec![eyebrow_text(&t!("captains_log.objective_link.active"), Some(color::accent())).into()];

  let current = state.linked_id(date, source);
  let active: Vec<&ObjectiveOption> = state.active().collect();
  if active.is_empty() && !state.is_creating(date, source) {
    items.push(
      text(t!("captains_log.objective_link.none").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }
  for objective in active {
    items.push(objective_row(date, source, objective, current == Some(objective.id)));
  }

  if state.is_creating(date, source) {
    items.push(create_input(state, date, source));
  } else {
    items.push(new_objective_row());
  }

  container(Column::with_children(items).spacing(spacing::SPACE_2))
    .width(Length::Fixed(CARD_MAX_WIDTH))
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::NAV_CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn objective_row<'a>(
  date: &str,
  source: &LinkSource,
  objective: &ObjectiveOption,
  current: bool,
) -> Element<'a, Parent> {
  let accent = accent_color(&objective.accent);
  let mut row: Vec<Element<'a, Parent>> = vec![
    target_tile(accent, ROW_TILE, 13.0),
    text(objective.title.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .width(Length::Fill)
      .into(),
  ];
  if current {
    row.push(Icon::check().size(14.0).color(color::accent()).render());
  }

  button(
    Row::with_children(row)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding([7.0, 8.0])
  .on_press(Parent::ObjectiveLink(Message::Pick {
    date: date.to_owned(),
    source: source.clone(),
    objective: objective.id,
  }))
  .style(row_button_style)
  .into()
}

fn new_objective_row<'a>() -> Element<'a, Parent> {
  let row = Row::with_children(vec![
    Icon::plus().size(13.0).color(color::accent()).render(),
    text(t!("captains_log.objective_link.new").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::accent()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  button(row)
    .width(Length::Fill)
    .padding([7.0, 8.0])
    .on_press(Parent::ObjectiveLink(Message::StartCreate))
    .style(row_button_style)
    .into()
}

fn create_input<'a>(state: &State, date: &str, source: &LinkSource) -> Element<'a, Parent> {
  let submit = Parent::ObjectiveLink(Message::Create {
    date: date.to_owned(),
    source: source.clone(),
  });
  let input = text_input(&t!("captains_log.objective_link.new_placeholder"), state.draft())
    .size(typography::size::MD)
    .padding([9.0, 11.0])
    .on_input(|value| Parent::ObjectiveLink(Message::DraftChanged(value)))
    .on_submit(submit.clone())
    .style(input_style);

  let actions = Row::with_children(vec![
    Button::ghost(t!("captains_log.objective_link.cancel").into_owned())
      .size(Size::Sm)
      .on_press(Parent::ObjectiveLink(Message::CancelCreate))
      .into(),
    Button::primary(t!("captains_log.objective_link.create").into_owned())
      .size(Size::Sm)
      .on_press_maybe((!state.draft().trim().is_empty()).then_some(submit))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  Column::with_children(vec![input.into(), actions.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

pub fn day_panel<'a>(state: &'a State, date: &str) -> Element<'a, Parent> {
  let rows: Vec<(&ObjectiveLink, &ObjectiveOption)> = state
    .day_links(date)
    .iter()
    .filter_map(|link| state.option(link.objective_id).map(|objective| (link, objective)))
    .collect();

  let mut children: Vec<Element<'a, Parent>> = vec![panel_header(rows.len())];
  if rows.is_empty() {
    children.push(panel_empty());
  } else {
    for (link, objective) in rows {
      children.push(panel_row(link, objective));
    }
  }

  container(Column::with_children(children))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn panel_header<'a>(count: usize) -> Element<'a, Parent> {
  let row = Row::with_children(vec![
    Icon::chevrons_up().size(16.0).color(color::accent()).render(),
    text(t!("captains_log.objective_link.panel_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("captains_log.objective_link.linked_count", count => count).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Space::new().width(Length::Fill).into(),
    Button::secondary(t!("captains_log.objective_link.board").into_owned())
      .size(Size::Sm)
      .icon_right(Icon::arrow_right())
      .on_press(Parent::ObjectiveLink(Message::OpenBoard(None)))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 12.0,
      right: 16.0,
      bottom: 12.0,
      left: 16.0,
    })
    .into()
}

fn panel_empty<'a>() -> Element<'a, Parent> {
  let row = Row::with_children(vec![
    Icon::tracker().size(15.0).color(color::text::tertiary()).render(),
    text(t!("captains_log.objective_link.panel_empty").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .width(Length::Fill)
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: 16.0,
      bottom: 13.0,
      left: 16.0,
    })
    .into()
}

fn panel_row<'a>(link: &ObjectiveLink, objective: &ObjectiveOption) -> Element<'a, Parent> {
  let accent = accent_color(&objective.accent);
  let title = text(objective.title.clone())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let meta =
    text(t!("captains_log.objective_link.linked_from", source => source_label(&link.source_kind)).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()));
  let column = Column::with_children(vec![title.into(), meta.into()])
    .spacing(2.0)
    .width(Length::Fill);

  let mut row: Vec<Element<'a, Parent>> = vec![target_tile(accent, PANEL_TILE, 16.0), column.into()];
  if objective.status != ObjectiveStatus::Active {
    row.push(status_tag(objective.status));
  }
  row.push(Icon::arrow_right().size(14.0).color(color::text::tertiary()).render());

  button(
    Row::with_children(row)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 11.0,
    right: 16.0,
    bottom: 11.0,
    left: 16.0,
  })
  .on_press(Parent::ObjectiveLink(Message::OpenBoard(Some(objective.id))))
  .style(row_button_style)
  .into()
}

pub fn chip_if_linked<'a>(state: &State, date: &str, source: &LinkSource) -> std::option::Option<Element<'a, Parent>> {
  state.linked_objective(date, source).map(chip)
}

fn chip<'a>(objective: &ObjectiveOption) -> Element<'a, Parent> {
  let accent = accent_color(&objective.accent);
  let row = Row::with_children(vec![
    Icon::tracker().size(11.0).color(accent).render(),
    text(objective.title.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_y(Vertical::Center);

  container(row)
    .padding(Padding {
      top: 3.0,
      right: 9.0,
      bottom: 3.0,
      left: 7.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.12))),
      border: Border {
        color: color::with_alpha(accent, 0.5),
        width: 1.0,
        radius: PILL_RADIUS.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn status_tag<'a>(status: ObjectiveStatus) -> Element<'a, Parent> {
  let (label, tint) = match status {
    ObjectiveStatus::Complete => (t!("standing_orders.status.complete"), color::status::ONLINE),
    ObjectiveStatus::Cancelled => (t!("standing_orders.status.cancelled"), color::text::tertiary()),
    ObjectiveStatus::Active => (t!("standing_orders.status.active"), color::accent()),
  };

  container(
    text(label.into_owned().to_uppercase())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(tint)),
  )
  .padding([3.0, 8.0])
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.1))),
    border: Border {
      color: tint,
      width: 1.0,
      radius: 5.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn target_tile<'a>(accent: iced::Color, size: f32, icon: f32) -> Element<'a, Parent> {
  container(Icon::tracker().size(icon).color(accent).render::<Parent>())
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .align_x(iced::alignment::Horizontal::Center)
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

fn accent_color(hex: &str) -> iced::Color {
  color::from_hex(hex).unwrap_or_else(color::accent)
}

fn source_label(kind: &str) -> String {
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

fn row_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
    }
    _ => None,
  };
  button::Style {
    background,
    border: Border {
      color: color::rule(),
      width: 0.0,
      radius: radius::CONTROL.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
  let border_alpha = match status {
    text_input::Status::Focused {
      ..
    } => 0.18,
    _ => 0.1,
  };
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, border_alpha),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent(), 0.4),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  fn objective(id: i64, title: &str) -> ObjectiveOption {
    ObjectiveOption {
      id,
      title: title.to_owned(),
      accent: "#5BB97E".to_owned(),
      status: ObjectiveStatus::Active,
    }
  }

  fn log_answer(question: &str) -> LinkSource {
    LinkSource::LogAnswer {
      question_id: question.to_owned(),
    }
  }

  fn link(objective_id: i64, date: &str, source: &LinkSource) -> ObjectiveLink {
    ObjectiveLink {
      date: date.to_owned(),
      objective_id,
      source_kind: source.source_kind().to_owned(),
      source_ref: source.source_ref(),
    }
  }

  mod lookup {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_the_objective_a_source_is_linked_to() {
      let mut state = State::default();
      state.set_objectives(vec![objective(1, "Fund a Nyx"), objective(2, "Break the doctrine")]);
      let source = log_answer("goal");
      state.set_day_links("2026-07-05".to_owned(), vec![link(2, "2026-07-05", &source)]);

      assert_eq!(state.linked_id("2026-07-05", &source), Some(2));
      assert_eq!(state.linked_objective("2026-07-05", &source).map(|o| o.id), Some(2));
      assert_eq!(state.linked_id("2026-07-04", &source), None);
    }

    #[test]
    fn it_lists_only_active_objectives_for_the_picker() {
      let mut state = State::default();
      let mut done = objective(2, "Done");
      done.status = ObjectiveStatus::Complete;
      state.set_objectives(vec![objective(1, "Live"), done]);

      let active: Vec<i64> = state.active().map(|objective| objective.id).collect();
      assert_eq!(active, vec![1]);
    }
  }

  mod picker_state {
    use super::*;

    #[test]
    fn it_toggles_a_picker_open_and_closed_for_a_source() {
      let mut state = State::default();
      let source = log_answer("goal");

      state.toggle("2026-07-05".to_owned(), source.clone());
      assert!(state.is_open("2026-07-05", &source));

      state.toggle("2026-07-05".to_owned(), source.clone());
      assert!(!state.is_open("2026-07-05", &source));
    }

    #[test]
    fn it_tracks_the_create_draft_within_an_open_picker() {
      let mut state = State::default();
      let source = log_answer("goal");
      state.toggle("2026-07-05".to_owned(), source.clone());

      state.start_create();
      state.set_draft("Fund a Nyx".to_owned());
      assert!(state.is_creating("2026-07-05", &source));
      assert_eq!(state.draft(), "Fund a Nyx");

      state.cancel_create();
      assert!(!state.is_creating("2026-07-05", &source));
      assert_eq!(state.draft(), "");
    }
  }

  mod reload {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_reloads_objectives_and_a_days_links_from_the_store() {
      let db = store::open_test().await.unwrap();
      let created = objective::create(
        &db,
        &NewObjective {
          accent: "#5BB97E".to_owned(),
          horizon: None,
          target: None,
          title: "Thread".to_owned(),
          why: None,
        },
      )
      .await
      .unwrap();
      let source = log_answer("goal");
      objective::set_link(&db, created.id, "2026-07-05", &source)
        .await
        .unwrap();

      let data = reload_data(&db, "2026-07-05").await;
      let mut state = State::default();
      state.apply(data);

      assert_eq!(state.linked_id("2026-07-05", &source), Some(created.id));
      assert_eq!(state.active().count(), 1);
    }
  }

  mod dispatch {
    use pretty_assertions::assert_eq;

    use super::*;

    fn new_objective(title: &str) -> NewObjective {
      NewObjective {
        accent: "#5BB97E".to_owned(),
        horizon: None,
        target: None,
        title: title.to_owned(),
        why: None,
      }
    }

    #[tokio::test]
    async fn it_dispatches_every_picker_message() {
      let db = store::open_test().await.unwrap();
      let created = objective::create(&db, &new_objective("Anchor")).await.unwrap();
      let mut state = State::default();
      state.set_objectives(vec![objective(created.id, "Anchor")]);
      let source = log_answer("goal");
      let date = "2026-07-05".to_owned();

      let _ = update(
        &mut state,
        &db,
        Message::Toggle {
          date: date.clone(),
          source: source.clone(),
        },
      );
      assert!(state.is_open_for(&date, &source));

      let _ = update(&mut state, &db, Message::StartCreate);
      let _ = update(&mut state, &db, Message::DraftChanged("Fresh".to_owned()));
      assert!(state.is_creating(&date, &source));

      let _ = update(&mut state, &db, Message::CancelCreate);
      assert!(!state.is_creating(&date, &source));

      let _ = update(
        &mut state,
        &db,
        Message::Create {
          date: date.clone(),
          source: source.clone(),
        },
      );
      assert!(
        state.is_open_for(&date, &source),
        "an empty draft leaves the picker open"
      );

      let _ = update(
        &mut state,
        &db,
        Message::Pick {
          date: date.clone(),
          source: source.clone(),
          objective: created.id,
        },
      );
      assert!(!state.is_open_for(&date, &source));

      let _ = update(
        &mut state,
        &db,
        Message::Clear {
          date: date.clone(),
          source: source.clone(),
        },
      );

      let _ = update(
        &mut state,
        &db,
        Message::Toggle {
          date: date.clone(),
          source: source.clone(),
        },
      );
      let _ = update(&mut state, &db, Message::StartCreate);
      let _ = update(&mut state, &db, Message::DraftChanged("Break the doctrine".to_owned()));
      let _ = update(
        &mut state,
        &db,
        Message::Create {
          date: date.clone(),
          source: source.clone(),
        },
      );
      assert!(!state.is_open_for(&date, &source), "a real create closes the picker");

      let _ = update(&mut state, &db, Message::OpenBoard(Some(created.id)));

      let data = reload_data(&db, &date).await;
      let _ = update(&mut state, &db, Message::Reloaded(Box::new(data)));
      assert_eq!(state.active().count(), 1);
    }
  }

  mod render {
    use super::*;
    use crate::store::model::PromptConfig;

    #[test]
    fn it_renders_the_picker_panel_and_chip() {
      let _ = PromptConfig::default();
      let mut state = State::default();
      state.set_objectives(vec![objective(1, "Fund a Nyx")]);
      let source = log_answer("goal");
      state.set_day_links("2026-07-05".to_owned(), vec![link(1, "2026-07-05", &source)]);

      {
        let _picker: Element<'_, Parent> = picker(&state, "2026-07-05", source.clone(), false);
        let _panel: Element<'_, Parent> = day_panel(&state, "2026-07-05");
        assert!(chip_if_linked(&state, "2026-07-05", &source).is_some());
      }

      // Open + creating variants render too.
      state.toggle("2026-07-05".to_owned(), log_answer("next"));
      state.start_create();
      let _open: Element<'_, Parent> = picker(&state, "2026-07-05", log_answer("next"), true);
    }

    #[test]
    fn it_renders_the_empty_day_panel() {
      let state = State::default();
      let _panel: Element<'_, Parent> = day_panel(&state, "2026-07-05");
    }
  }
}
