use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text, text_input},
};

use super::super::Message as Parent;
use crate::{
  features::skills::queue_timing::roman,
  store::{
    Database,
    model::{DossierOrder, Objective, ObjectiveStatus},
    repo::{character, dossier as dossier_repo, objective, sde},
  },
  ui::{
    components::{eyebrow::eyebrow_text, icon::Icon, rule},
    format::skill_label,
    style::{color, radius, spacing, typography},
  },
};

const BODY_MAX_WIDTH: f32 = 760.0;
const CARD_RADIUS: f32 = radius::CARD;
const DISCLOSURE_WIDTH: f32 = 300.0;
const HOT_PROGRESS: f32 = 0.9;
const ORDER_BUTTON: f32 = 26.0;
const PILL_RADIUS: f32 = 999.0;
const TILE: f32 = 22.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditTarget {
  NearTerm,
  Order(i64),
  Purpose,
}

#[derive(Clone, Debug)]
pub enum Message {
  AddOrder,
  CancelEdit,
  ClearObjective(i64),
  CommitEdit,
  DraftChanged(String),
  PickObjective { objective: i64, order: i64 },
  Reloaded(Box<Data>),
  RemoveOrder(i64),
  SetStatus { id: i64, status: OrderStatus },
  StartEdit(EditTarget),
  ToggleArchive,
  TogglePicker(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderStatus {
  Active,
  Cancelled,
  Complete,
}

#[derive(Clone, Debug)]
pub struct ObjectiveOption {
  pub accent: String,
  pub id: i64,
  pub status: ObjectiveStatus,
  pub title: String,
}

fn to_option(model: &Objective) -> ObjectiveOption {
  ObjectiveOption {
    accent: model.accent.clone(),
    id: model.id,
    status: ObjectiveStatus::parse(&model.status).unwrap_or_default(),
    title: model.title.clone(),
  }
}

#[derive(Clone, Debug)]
pub struct NowTraining {
  pub level: i64,
  pub progress: f32,
  pub remaining: String,
  pub skill: String,
}

#[derive(Clone, Debug, Default)]
pub struct Data {
  pub near_term: Option<String>,
  pub objectives: Vec<ObjectiveOption>,
  pub orders: Vec<DossierOrder>,
  pub purpose: Option<String>,
  pub training: Option<NowTraining>,
}

#[derive(Debug, Default)]
pub struct State {
  data: Data,
  draft: String,
  editing: Option<EditTarget>,
  picker: Option<i64>,
  show_archive: bool,
}

impl State {
  pub(in crate::features::roster::character_detail) fn reset(&mut self, data: Data) {
    self.data = data;
    self.draft.clear();
    self.editing = None;
    self.picker = None;
    self.show_archive = false;
  }

  fn option(&self, id: i64) -> Option<&ObjectiveOption> {
    self.data.objectives.iter().find(|objective| objective.id == id)
  }

  fn active_objectives(&self) -> impl Iterator<Item = &ObjectiveOption> {
    self
      .data
      .objectives
      .iter()
      .filter(|objective| objective.status == ObjectiveStatus::Active)
  }

  fn is_editing(&self, target: EditTarget) -> bool {
    self.editing == Some(target)
  }

  #[cfg(test)]
  pub(in crate::features::roster::character_detail) fn draft(&self) -> &str {
    &self.draft
  }

  #[cfg(test)]
  pub(in crate::features::roster::character_detail) fn editing(&self) -> Option<EditTarget> {
    self.editing
  }

  #[cfg(test)]
  pub(in crate::features::roster::character_detail) fn picker(&self) -> Option<i64> {
    self.picker
  }

  #[cfg(test)]
  pub(in crate::features::roster::character_detail) fn show_archive(&self) -> bool {
    self.show_archive
  }
}

#[derive(Clone, Debug)]
enum PersistOp {
  AddOrder,
  Brief {
    near_term: Option<String>,
    purpose: Option<String>,
  },
  ClearObjective(i64),
  EditOrder {
    id: i64,
    text: String,
  },
  RemoveOrder(i64),
  SetObjective {
    id: i64,
    objective: i64,
  },
  SetStatus {
    id: i64,
    status: OrderStatus,
  },
}

pub(in crate::features::roster::character_detail) fn update(
  state: &mut State,
  character_id: i64,
  db: &Database,
  message: Message,
) -> Task<Parent> {
  match message {
    Message::AddOrder => run(character_id, db, PersistOp::AddOrder),
    Message::CancelEdit => {
      state.editing = None;
      state.draft.clear();
      Task::none()
    }
    Message::ClearObjective(id) => {
      state.picker = None;
      run(character_id, db, PersistOp::ClearObjective(id))
    }
    Message::CommitEdit => commit_edit(state, character_id, db),
    Message::DraftChanged(value) => {
      state.draft = value;
      Task::none()
    }
    Message::PickObjective {
      objective,
      order,
    } => {
      state.picker = None;
      run(
        character_id,
        db,
        PersistOp::SetObjective {
          id: order,
          objective,
        },
      )
    }
    Message::Reloaded(data) => {
      state.data = *data;
      Task::none()
    }
    Message::RemoveOrder(id) => run(character_id, db, PersistOp::RemoveOrder(id)),
    Message::SetStatus {
      id,
      status,
    } => run(
      character_id,
      db,
      PersistOp::SetStatus {
        id,
        status,
      },
    ),
    Message::StartEdit(target) => {
      state.draft = current_value(state, target);
      state.editing = Some(target);
      Task::none()
    }
    Message::ToggleArchive => {
      state.show_archive = !state.show_archive;
      Task::none()
    }
    Message::TogglePicker(id) => {
      state.picker = (state.picker != Some(id)).then_some(id);
      Task::none()
    }
  }
}

fn current_value(state: &State, target: EditTarget) -> String {
  match target {
    EditTarget::NearTerm => state.data.near_term.clone().unwrap_or_default(),
    EditTarget::Order(id) => state
      .data
      .orders
      .iter()
      .find(|order| order.id == id)
      .map(|order| order.text.clone())
      .unwrap_or_default(),
    EditTarget::Purpose => state.data.purpose.clone().unwrap_or_default(),
  }
}

fn commit_edit(state: &mut State, character_id: i64, db: &Database) -> Task<Parent> {
  let Some(target) = state.editing.take() else {
    return Task::none();
  };
  let draft = std::mem::take(&mut state.draft);
  let trimmed = draft.trim().to_owned();
  let op = match target {
    EditTarget::NearTerm => PersistOp::Brief {
      near_term: non_empty(&trimmed),
      purpose: state.data.purpose.clone(),
    },
    EditTarget::Order(id) => PersistOp::EditOrder {
      id,
      text: trimmed,
    },
    EditTarget::Purpose => PersistOp::Brief {
      near_term: state.data.near_term.clone(),
      purpose: non_empty(&trimmed),
    },
  };
  run(character_id, db, op)
}

fn non_empty(value: &str) -> Option<String> {
  (!value.is_empty()).then(|| value.to_owned())
}

fn run(character_id: i64, db: &Database, op: PersistOp) -> Task<Parent> {
  let worker = db.clone();
  Task::perform(
    async move { Box::new(persist_and_reload(&worker, character_id, op).await) },
    |data| Parent::Dossier(Message::Reloaded(data)),
  )
}

async fn persist_and_reload(db: &Database, character_id: i64, op: PersistOp) -> Data {
  match op {
    PersistOp::AddOrder => {
      let _ = dossier_repo::add_order(db, character_id, "").await;
    }
    PersistOp::Brief {
      near_term,
      purpose,
    } => {
      let _ = dossier_repo::upsert_brief(db, character_id, purpose.as_deref(), near_term.as_deref()).await;
    }
    PersistOp::ClearObjective(id) => {
      let _ = dossier_repo::clear_objective(db, id).await;
    }
    PersistOp::EditOrder {
      id,
      text,
    } => {
      let _ = dossier_repo::edit_order(db, id, &text).await;
    }
    PersistOp::RemoveOrder(id) => {
      let _ = dossier_repo::remove_order(db, id).await;
    }
    PersistOp::SetObjective {
      id,
      objective,
    } => {
      let _ = dossier_repo::set_objective(db, id, objective).await;
    }
    PersistOp::SetStatus {
      id,
      status,
    } => {
      let _ = match status {
        OrderStatus::Active => dossier_repo::reopen_order(db, id).await,
        OrderStatus::Cancelled => dossier_repo::cancel_order(db, id).await,
        OrderStatus::Complete => dossier_repo::complete_order(db, id).await,
      };
    }
  }
  load_data(db, character_id).await
}

pub(in crate::features::roster::character_detail) async fn load_data(db: &Database, character_id: i64) -> Data {
  let brief = dossier_repo::get_brief(db, character_id).await.ok().flatten();
  let (near_term, purpose) = brief.map(|brief| (brief.near_term, brief.purpose)).unwrap_or_default();
  let orders = dossier_repo::list_orders(db, character_id).await.unwrap_or_default();
  let objectives = objective::list(db, None)
    .await
    .unwrap_or_default()
    .iter()
    .map(to_option)
    .collect();
  let training = load_training(db, character_id).await;
  Data {
    near_term,
    objectives,
    orders,
    purpose,
    training,
  }
}

async fn load_training(db: &Database, character_id: i64) -> Option<NowTraining> {
  let now = Utc::now();
  let entry = character::current_skillqueue(db, character_id, now)
    .await
    .ok()
    .flatten()?;
  let finish = entry.finish_date().as_deref().and_then(parse_timestamp)?;
  if finish <= now {
    return None;
  }
  let start = entry.start_date().as_deref().and_then(parse_timestamp);
  let item_type = sde::get_item_type(db, entry.skill_id()).await.ok().flatten();
  let skill = skill_label(item_type.as_ref().map(|item| item.name().as_str()), entry.skill_id());
  let progress = match start {
    Some(start) if finish > start => {
      let span = (finish - start).num_seconds() as f32;
      let elapsed = (now - start).num_seconds() as f32;
      (elapsed / span).clamp(0.0, 1.0)
    }
    _ => 0.0,
  };
  Some(NowTraining {
    level: entry.finished_level(),
    progress,
    remaining: format_remaining(finish - now),
    skill,
  })
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(value)
    .ok()
    .map(|value| value.with_timezone(&Utc))
}

fn format_remaining(duration: chrono::Duration) -> String {
  let total_minutes = duration.num_minutes();
  if total_minutes <= 0 {
    return t!("roster.card.done").into_owned();
  }
  let days = total_minutes / (24 * 60);
  let hours = (total_minutes % (24 * 60)) / 60;
  let minutes = total_minutes % 60;
  if days > 0 {
    format!("{days}d {hours}h")
  } else if hours > 0 {
    format!("{hours}h {minutes}m")
  } else {
    format!("{minutes}m")
  }
}

pub(in crate::features::roster::character_detail) fn body(state: &State) -> Element<'_, Parent> {
  let sections = Column::with_children(vec![purpose_hero(state), orders_card(state), training_card(state)])
    .spacing(spacing::SPACE_6)
    .width(Length::Fill);

  container(container(sections).max_width(BODY_MAX_WIDTH).width(Length::Fill))
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .into()
}

fn purpose_hero(state: &State) -> Element<'_, Parent> {
  let head = Row::with_children(vec![
    Icon::personal().size(15.0).color(color::accent()).render(),
    eyebrow_text(&t!("roster.dossier.purpose_eyebrow"), Some(color::accent())).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let field = edit_field(
    state,
    EditTarget::Purpose,
    state.data.purpose.as_deref().unwrap_or_default(),
    t!("roster.dossier.purpose_placeholder").into_owned(),
    true,
    false,
  );

  let content = Column::with_children(vec![head.into(), field])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill);

  accent_bar_card(content.into())
}

fn accent_bar_card(content: Element<'_, Parent>) -> Element<'_, Parent> {
  let bar = container(Space::new())
    .width(Length::Fixed(3.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent())),
      ..container::Style::default()
    });

  let padded = container(content).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_4_5,
    right: spacing::SPACE_6,
    bottom: spacing::SPACE_4_5,
    left: spacing::SPACE_6,
  });

  container(Row::with_children(vec![bar.into(), padded.into()]).width(Length::Fill))
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

fn orders_card(state: &State) -> Element<'_, Parent> {
  section_card(
    Icon::chevron_up(),
    t!("roster.dossier.orders_title").into_owned(),
    orders_editor(state),
  )
}

fn training_card(state: &State) -> Element<'_, Parent> {
  let field = edit_field(
    state,
    EditTarget::NearTerm,
    state.data.near_term.as_deref().unwrap_or_default(),
    t!("roster.dossier.near_term_placeholder").into_owned(),
    false,
    false,
  );

  let readout = match &state.data.training {
    Some(training) => now_training(training),
    None => empty_training(),
  };

  let body = Column::with_children(vec![field, readout])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  section_card(
    Icon::skills(),
    t!("roster.dossier.training_toward").into_owned(),
    body.into(),
  )
}

fn now_training(training: &NowTraining) -> Element<'_, Parent> {
  let dot_color = if training.progress > HOT_PROGRESS {
    color::accent()
  } else {
    color::status::ONLINE
  };

  let level = text(format!(" {}", roman(training.level)))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let skill = Row::with_children(vec![
    text(training.skill.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    level.into(),
  ])
  .align_y(Vertical::Center);

  let labelled = Column::with_children(vec![
    eyebrow_text(&t!("roster.dossier.now_training"), Some(color::text::tertiary())).into(),
    skill.into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .width(Length::Fill);

  let remaining = text(training.remaining.clone())
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let row = Row::with_children(vec![status_dot(dot_color), labelled.into(), remaining.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  sunken_row(row.into())
}

fn empty_training() -> Element<'static, Parent> {
  let row = Row::with_children(vec![
    status_dot(color::status::DANGER),
    text(t!("roster.dossier.skill_queue_empty").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::status::DANGER))
      .width(Length::Fill)
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  sunken_row(row.into())
}

fn status_dot(fill: Color) -> Element<'static, Parent> {
  container(Space::new())
    .width(Length::Fixed(6.0))
    .height(Length::Fixed(6.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn sunken_row(content: Element<'_, Parent>) -> Element<'_, Parent> {
  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: 9.0,
      right: spacing::SPACE_3,
      bottom: 9.0,
      left: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn section_card<'a>(icon: Icon, label: String, body: Element<'a, Parent>) -> Element<'a, Parent> {
  let head = container(
    Row::with_children(vec![
      icon.size(16.0).color(color::accent()).render(),
      eyebrow_text(&label, Some(color::text::secondary())).into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_4_5,
  });

  let content = container(body).width(Length::Fill).padding(spacing::SPACE_4_5);

  container(Column::with_children(vec![head.into(), rule::horizontal(), content.into()]).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: CARD_RADIUS.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn orders_editor(state: &State) -> Element<'_, Parent> {
  let active: Vec<&DossierOrder> = state
    .data
    .orders
    .iter()
    .filter(|order| order.status == "active")
    .collect();
  let archived: Vec<&DossierOrder> = state
    .data
    .orders
    .iter()
    .filter(|order| order.status != "active")
    .collect();

  let mut children: Vec<Element<'_, Parent>> = Vec::new();
  if active.is_empty() {
    children.push(
      text(t!("roster.dossier.no_active_orders").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }
  for (index, order) in active.iter().enumerate() {
    children.push(order_row(state, order, Some(index + 1)));
  }

  children.push(order_toolbar(archived.len(), state.show_archive));

  if state.show_archive && !archived.is_empty() {
    children.push(archive_section(state, &archived));
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn order_toolbar<'a>(archived: usize, show_archive: bool) -> Element<'a, Parent> {
  let add = button(
    Row::with_children(vec![
      Icon::plus().size(13.0).color(color::text::secondary()).render(),
      text(t!("roster.dossier.add_order").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding([spacing::SPACE_2, spacing::SPACE_3])
  .on_press(Parent::Dossier(Message::AddOrder))
  .style(dashed_button_style);

  let mut row: Vec<Element<'a, Parent>> = vec![add.into(), Space::new().width(Length::Fill).into()];
  if archived > 0 {
    row.push(archive_toggle(archived, show_archive));
  }

  Row::with_children(row)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn archive_toggle<'a>(archived: usize, show_archive: bool) -> Element<'a, Parent> {
  let chevron = if show_archive {
    Icon::chevron_up()
  } else {
    Icon::chevron_down()
  };
  let label = if show_archive {
    t!("roster.dossier.hide_archive").into_owned()
  } else {
    t!("roster.dossier.view_archive", count => archived).into_owned()
  };

  button(
    Row::with_children(vec![
      chevron.size(13.0).color(color::text::secondary()).render(),
      text(label)
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding([spacing::SPACE_2, spacing::SPACE_3])
  .on_press(Parent::Dossier(Message::ToggleArchive))
  .style(quiet_button_style)
  .into()
}

fn archive_section<'a>(state: &'a State, archived: &[&'a DossierOrder]) -> Element<'a, Parent> {
  let mut children: Vec<Element<'a, Parent>> = vec![
    eyebrow_text(
      &t!("roster.dossier.archive_count", count => archived.len()),
      Some(color::text::tertiary()),
    )
    .into(),
  ];
  for order in archived {
    children.push(order_row(state, order, None));
  }

  container(
    Column::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  })
  .into()
}

fn order_row<'a>(state: &'a State, order: &'a DossierOrder, number: Option<usize>) -> Element<'a, Parent> {
  let field = edit_field(
    state,
    EditTarget::Order(order.id),
    &order.text,
    t!("roster.dossier.order_placeholder").into_owned(),
    false,
    order.status != "active",
  );

  let controls = Row::with_children(vec![
    order_link(state, order),
    Space::new().width(Length::Fill).into(),
    order_actions(order),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let content = Column::with_children(vec![field, controls.into()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill);

  let row = Row::with_children(vec![order_badge(&order.status, number), content.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::NAV_CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn order_badge<'a>(status: &str, number: Option<usize>) -> Element<'a, Parent> {
  let (tint, inner): (Color, Element<'a, Parent>) = match status {
    "complete" => (
      color::status::ONLINE,
      Icon::check().size(13.0).color(color::status::ONLINE).render(),
    ),
    "cancelled" => (
      color::text::tertiary(),
      Icon::block().size(13.0).color(color::text::tertiary()).render(),
    ),
    _ => {
      let label = number.map(|n| n.to_string()).unwrap_or_default();
      (
        color::accent(),
        text(label)
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(color::accent()))
          .into(),
      )
    }
  };

  container(inner)
    .width(Length::Fixed(TILE))
    .height(Length::Fixed(TILE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tint, 0.14))),
      border: Border {
        color: color::with_alpha(tint, 0.4),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn order_actions(order: &DossierOrder) -> Element<'_, Parent> {
  let mut row: Vec<Element<'_, Parent>> = Vec::new();
  if order.status == "active" {
    row.push(order_button(
      Icon::check(),
      Message::SetStatus {
        id: order.id,
        status: OrderStatus::Complete,
      },
      color::status::ONLINE,
    ));
    row.push(order_button(
      Icon::block(),
      Message::SetStatus {
        id: order.id,
        status: OrderStatus::Cancelled,
      },
      color::status::DANGER,
    ));
  } else {
    row.push(status_tag(&order.status));
    row.push(order_button(
      Icon::reset(),
      Message::SetStatus {
        id: order.id,
        status: OrderStatus::Active,
      },
      color::text::secondary(),
    ));
  }
  row.push(order_button(
    Icon::close(),
    Message::RemoveOrder(order.id),
    color::status::DANGER,
  ));

  Row::with_children(row)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
}

fn status_tag<'a>(status: &str) -> Element<'a, Parent> {
  let (label, tint) = if status == "complete" {
    (t!("standing_orders.status.complete"), color::status::ONLINE)
  } else {
    (t!("standing_orders.status.cancelled"), color::text::tertiary())
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
      color: color::with_alpha(tint, 0.4),
      width: 1.0,
      radius: 5.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn order_link<'a>(state: &'a State, order: &'a DossierOrder) -> Element<'a, Parent> {
  let mut column: Vec<Element<'a, Parent>> = vec![link_head(state, order)];
  if state.picker == Some(order.id) {
    column.push(link_disclosure(state, order));
  }
  Column::with_children(column).spacing(spacing::SPACE_2).into()
}

fn link_head<'a>(state: &'a State, order: &'a DossierOrder) -> Element<'a, Parent> {
  let linked = order.objective_id.and_then(|id| state.option(id));
  let (icon, label, tint, solid) = match linked {
    Some(objective) => (
      Icon::tracker(),
      objective.title.clone(),
      accent_color(&objective.accent),
      true,
    ),
    None => (
      Icon::chevron_up(),
      t!("roster.dossier.link_objective").into_owned(),
      color::accent(),
      false,
    ),
  };

  let row = Row::with_children(vec![
    icon.size(13.0).color(tint).render(),
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(if solid {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      }))
      .into(),
    Icon::chevron_down().size(12.0).color(color::text::tertiary()).render(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(row)
    .padding([5.0, 9.0])
    .on_press(Parent::Dossier(Message::TogglePicker(order.id)))
    .style(move |_, status| link_button_style(status, tint, solid))
    .into()
}

fn link_disclosure<'a>(state: &'a State, order: &'a DossierOrder) -> Element<'a, Parent> {
  let mut items: Vec<Element<'a, Parent>> = vec![
    eyebrow_text(&t!("roster.dossier.active_objectives"), Some(color::accent())).into(),
    clear_row(order, order.objective_id.is_none()),
  ];

  let active: Vec<&ObjectiveOption> = state.active_objectives().collect();
  if active.is_empty() {
    items.push(
      text(t!("roster.dossier.no_active_objectives").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }
  for objective in active {
    items.push(objective_row(
      order,
      objective,
      order.objective_id == Some(objective.id),
    ));
  }

  container(Column::with_children(items).spacing(spacing::SPACE_2))
    .width(Length::Fixed(DISCLOSURE_WIDTH))
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::NAV_CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn clear_row<'a>(order: &DossierOrder, current: bool) -> Element<'a, Parent> {
  let mut row: Vec<Element<'a, Parent>> = vec![
    text(t!("roster.dossier.no_objective").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(if current {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      }))
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
  .on_press(Parent::Dossier(Message::ClearObjective(order.id)))
  .style(row_button_style)
  .into()
}

fn objective_row<'a>(order: &DossierOrder, objective: &ObjectiveOption, current: bool) -> Element<'a, Parent> {
  let accent = accent_color(&objective.accent);
  let mut row: Vec<Element<'a, Parent>> = vec![
    target_tile(accent),
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
  .on_press(Parent::Dossier(Message::PickObjective {
    objective: objective.id,
    order: order.id,
  }))
  .style(row_button_style)
  .into()
}

fn target_tile<'a>(accent: Color) -> Element<'a, Parent> {
  container(Icon::tracker().size(12.0).color(accent).render::<Parent>())
    .width(Length::Fixed(TILE))
    .height(Length::Fixed(TILE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.14))),
      border: Border {
        color: color::with_alpha(accent, 0.4),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn edit_field<'a>(
  state: &'a State,
  target: EditTarget,
  value: &str,
  placeholder: String,
  big: bool,
  strike: bool,
) -> Element<'a, Parent> {
  if state.is_editing(target) {
    editing_field(state, placeholder, big)
  } else {
    idle_field(target, value, placeholder, big, strike)
  }
}

fn idle_field<'a>(
  target: EditTarget,
  value: &str,
  placeholder: String,
  big: bool,
  strike: bool,
) -> Element<'a, Parent> {
  let has = !value.trim().is_empty();
  let display = if has { value.to_owned() } else { placeholder };
  let tint = if strike {
    color::text::tertiary()
  } else if has {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };
  let font = if big {
    typography::body::MEDIUM
  } else {
    typography::body::REGULAR
  };
  let size = if big {
    typography::size::LG
  } else {
    typography::size::MD
  };

  let label = text(display)
    .font(font)
    .size(size)
    .style(typography::colored(tint))
    .width(Length::Fill);

  let row = Row::with_children(vec![
    label.into(),
    Icon::pencil().size(13.0).color(color::text::tertiary()).render(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  button(row)
    .width(Length::Fill)
    .padding([spacing::SPACE_2, spacing::SPACE_2_5])
    .on_press(Parent::Dossier(Message::StartEdit(target)))
    .style(quiet_button_style)
    .into()
}

fn editing_field<'a>(state: &'a State, placeholder: String, big: bool) -> Element<'a, Parent> {
  let size = if big {
    typography::size::LG
  } else {
    typography::size::MD
  };
  let input = text_input(&placeholder, &state.draft)
    .size(size)
    .padding([9.0, 12.0])
    .on_input(|value| Parent::Dossier(Message::DraftChanged(value)))
    .on_submit(Parent::Dossier(Message::CommitEdit))
    .style(input_style);

  let row = Row::with_children(vec![
    container(input).width(Length::Fill).into(),
    order_button(Icon::check(), Message::CommitEdit, color::status::ONLINE),
    order_button(Icon::close(), Message::CancelEdit, color::status::DANGER),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  row.into()
}

fn order_button<'a>(icon: Icon, message: Message, hover: Color) -> Element<'a, Parent> {
  let glyph = container(icon.size(13.0).color(color::text::secondary()).render::<Parent>())
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center);

  button(glyph)
    .width(Length::Fixed(ORDER_BUTTON))
    .height(Length::Fixed(ORDER_BUTTON))
    .padding(0)
    .on_press(Parent::Dossier(message))
    .style(move |_, status| square_button_style(status, hover))
    .into()
}

fn accent_color(hex: &str) -> Color {
  color::from_hex(hex).unwrap_or_else(color::accent)
}

fn square_button_style(status: button::Status, hover: Color) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: if hovered {
        color::with_alpha(hover, 0.5)
      } else {
        color::rule()
      },
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    text_color: if hovered { hover } else { color::text::secondary() },
    ..button::Style::default()
  }
}

fn link_button_style(status: button::Status, tint: Color, solid: bool) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let background = if solid {
    Some(Background::Color(color::with_alpha(tint, 0.12)))
  } else if hovered {
    Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
  } else {
    None
  };
  button::Style {
    background,
    border: Border {
      color: if solid {
        color::with_alpha(tint, 0.45)
      } else {
        color::rule_strong()
      },
      width: 1.0,
      radius: PILL_RADIUS.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn dashed_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: None,
    border: Border {
      color: if hovered { color::accent() } else { color::rule_strong() },
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    text_color: if hovered {
      color::accent()
    } else {
      color::text::secondary()
    },
    ..button::Style::default()
  }
}

fn quiet_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
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
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, NewObjective, Race},
    repo::{character, objective},
  };

  const PILOT: i64 = 90_000_001;

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 98_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn new_objective(title: &str) -> NewObjective {
    NewObjective {
      accent: "#FF8800".to_owned(),
      horizon: None,
      target: None,
      title: title.to_owned(),
      why: None,
    }
  }

  mod transient {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_opens_and_cancels_an_inline_edit() {
      let db = store::open_test().await.unwrap();
      let mut state = State::default();

      let _ = update(&mut state, PILOT, &db, Message::StartEdit(EditTarget::Purpose));
      assert_eq!(state.editing(), Some(EditTarget::Purpose));

      let _ = update(&mut state, PILOT, &db, Message::DraftChanged("Anchor".to_owned()));
      assert_eq!(state.draft(), "Anchor");

      let _ = update(&mut state, PILOT, &db, Message::CancelEdit);
      assert_eq!(state.editing(), None);
      assert_eq!(state.draft(), "");
    }

    #[tokio::test]
    async fn it_seeds_the_draft_from_the_current_value_on_edit() {
      let db = store::open_test().await.unwrap();
      let mut state = State::default();
      state.data.purpose = Some("Fleet anchor".to_owned());

      let _ = update(&mut state, PILOT, &db, Message::StartEdit(EditTarget::Purpose));
      assert_eq!(state.draft(), "Fleet anchor");
    }

    #[tokio::test]
    async fn it_toggles_the_picker_and_archive() {
      let db = store::open_test().await.unwrap();
      let mut state = State::default();

      let _ = update(&mut state, PILOT, &db, Message::TogglePicker(7));
      assert_eq!(state.picker(), Some(7));
      let _ = update(&mut state, PILOT, &db, Message::TogglePicker(7));
      assert_eq!(state.picker(), None);

      let _ = update(&mut state, PILOT, &db, Message::ToggleArchive);
      assert!(state.show_archive());
    }
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_a_brief_edit() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;

      let data = persist_and_reload(
        &db,
        PILOT,
        PersistOp::Brief {
          near_term: Some("Caldari Cruiser V".to_owned()),
          purpose: Some("Fleet anchor".to_owned()),
        },
      )
      .await;

      assert_eq!(data.purpose.as_deref(), Some("Fleet anchor"));
      assert_eq!(data.near_term.as_deref(), Some("Caldari Cruiser V"));
    }

    #[tokio::test]
    async fn it_adds_edits_and_removes_an_order() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;

      let added = persist_and_reload(&db, PILOT, PersistOp::AddOrder).await;
      assert_eq!(added.orders.len(), 1);
      let id = added.orders[0].id;
      assert_eq!(added.orders[0].text, "");

      let edited = persist_and_reload(
        &db,
        PILOT,
        PersistOp::EditOrder {
          id,
          text: "Anchor the roam".to_owned(),
        },
      )
      .await;
      assert_eq!(edited.orders[0].text, "Anchor the roam");

      let removed = persist_and_reload(&db, PILOT, PersistOp::RemoveOrder(id)).await;
      assert!(removed.orders.is_empty());
    }

    #[tokio::test]
    async fn it_transitions_order_status() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let id = persist_and_reload(&db, PILOT, PersistOp::AddOrder).await.orders[0].id;

      let done = persist_and_reload(
        &db,
        PILOT,
        PersistOp::SetStatus {
          id,
          status: OrderStatus::Complete,
        },
      )
      .await;
      assert_eq!(done.orders[0].status, "complete");

      let reopened = persist_and_reload(
        &db,
        PILOT,
        PersistOp::SetStatus {
          id,
          status: OrderStatus::Active,
        },
      )
      .await;
      assert_eq!(reopened.orders[0].status, "active");
    }

    #[tokio::test]
    async fn it_links_and_unlinks_an_objective() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let objective = objective::create(&db, &new_objective("Fly the Cerberus"))
        .await
        .unwrap();
      let id = persist_and_reload(&db, PILOT, PersistOp::AddOrder).await.orders[0].id;

      let linked = persist_and_reload(
        &db,
        PILOT,
        PersistOp::SetObjective {
          id,
          objective: objective.id,
        },
      )
      .await;
      assert_eq!(linked.orders[0].objective_id, Some(objective.id));
      assert_eq!(
        linked
          .objectives
          .iter()
          .find(|option| option.id == objective.id)
          .map(|option| option.title.as_str()),
        Some("Fly the Cerberus")
      );

      let cleared = persist_and_reload(&db, PILOT, PersistOp::ClearObjective(id)).await;
      assert_eq!(cleared.orders[0].objective_id, None);
    }
  }

  mod render {
    use super::*;

    fn order(id: i64, status: &str) -> DossierOrder {
      DossierOrder {
        character_id: PILOT,
        created_at: "2026-07-09T00:00:00Z".to_owned(),
        id,
        objective_id: None,
        position: 0,
        status: status.to_owned(),
        text: "Anchor the roam".to_owned(),
        updated_at: "2026-07-09T00:00:00Z".to_owned(),
      }
    }

    #[test]
    fn it_renders_the_body_across_states() {
      let mut state = State::default();
      state.data.purpose = Some("Fleet anchor".to_owned());
      state.data.orders = vec![order(1, "active"), order(2, "complete")];
      state.data.objectives = vec![ObjectiveOption {
        accent: "#FF8800".to_owned(),
        id: 5,
        status: ObjectiveStatus::Active,
        title: "Fly the Cerberus".to_owned(),
      }];
      state.data.training = Some(NowTraining {
        level: 5,
        progress: 0.5,
        remaining: "2d 4h".to_owned(),
        skill: "Caldari Cruiser".to_owned(),
      });

      {
        let _el: Element<'_, Parent> = body(&state);
      }

      state.picker = Some(1);
      state.show_archive = true;
      state.editing = Some(EditTarget::Order(1));
      {
        let _open: Element<'_, Parent> = body(&state);
      }
    }

    #[test]
    fn it_renders_the_empty_training_and_orders_state() {
      let state = State::default();
      let _el: Element<'_, Parent> = body(&state);
    }
  }
}
