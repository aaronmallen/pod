mod header;
mod matrix;
mod model;
mod summary;

use std::collections::{HashMap, HashSet};

use iced::{
  Background, Color, Element, Length, Padding, Task,
  alignment::Horizontal,
  widget::{Column, Stack, container, scrollable},
};
pub(super) use model::CompareModel;

use crate::{
  features::{character_manager::OwnedPilot, skills::browse::SkillCatalog},
  store::{
    Database, images,
    repo::{character, skills},
  },
  ui::{
    components::backdrop,
    style::{color, spacing},
  },
};

pub(super) const CONTENT_MAX_WIDTH: f32 = 1180.0;
pub(super) const HEADER_HEIGHT: f32 = 72.0;
pub(super) const LABEL_COLUMN_WIDTH: f32 = 200.0;

#[derive(Clone, Debug)]
pub struct Loaded {
  pub catalog: SkillCatalog,
  pub models: Vec<(i64, CompareModel)>,
}

#[derive(Clone, Debug)]
pub enum Message {
  #[allow(dead_code)]
  CloseRequested,
  DataLoaded(Box<Loaded>),
  GroupToggled(i64),
  PickerQueryChanged(String),
  PickerToggled,
  PilotAdded(i64),
  PilotRemoved(i64),
}

#[derive(Debug)]
pub struct State {
  catalog: SkillCatalog,
  expanded: HashSet<i64>,
  loading: bool,
  models: HashMap<i64, CompareModel>,
  picker_open: bool,
  picker_query: String,
  roster: Vec<OwnedPilot>,
  selected_ids: Vec<i64>,
}

impl State {
  pub fn new(seed_ids: Vec<i64>, roster: Vec<OwnedPilot>) -> Self {
    let selected_ids = seed_ids
      .into_iter()
      .filter(|id| roster.iter().any(|pilot| pilot.id == *id))
      .collect();

    State {
      catalog: SkillCatalog {
        groups: Vec::new(),
      },
      expanded: HashSet::new(),
      loading: true,
      models: HashMap::new(),
      picker_open: false,
      picker_query: String::new(),
      roster,
      selected_ids,
    }
  }

  pub(super) fn available_pilots(&self) -> Vec<&OwnedPilot> {
    let query = self.picker_query.trim().to_lowercase();
    self
      .roster
      .iter()
      .filter(|pilot| {
        !self.selected_ids.contains(&pilot.id) && (query.is_empty() || pilot.name.to_lowercase().contains(&query))
      })
      .collect()
  }

  pub(super) fn can_remove(&self) -> bool {
    self.selected_ids.len() > 2
  }

  pub(super) fn is_expanded(&self, group_id: i64) -> bool {
    self.expanded.contains(&group_id)
  }

  #[allow(dead_code)]
  pub(super) fn is_loading(&self) -> bool {
    self.loading
  }

  pub(super) fn model(&self, pilot_id: i64) -> Option<&CompareModel> {
    self.models.get(&pilot_id)
  }

  pub(super) fn pilot_accent(&self, pilot_id: i64) -> Color {
    self
      .roster
      .iter()
      .find(|pilot| pilot.id == pilot_id)
      .map(|pilot| pilot.color)
      .unwrap_or(Color::WHITE)
  }

  pub(super) fn pilot_count(&self) -> usize {
    self.selected_ids.len()
  }

  pub(super) fn pilot_name(&self, pilot_id: i64) -> &str {
    self
      .roster
      .iter()
      .find(|pilot| pilot.id == pilot_id)
      .map(|pilot| pilot.name.as_str())
      .unwrap_or("")
  }

  pub(super) fn picker_open(&self) -> bool {
    self.picker_open
  }

  pub(super) fn picker_query(&self) -> &str {
    &self.picker_query
  }

  pub(crate) fn selected_ids(&self) -> &[i64] {
    &self.selected_ids
  }

  pub(super) fn skill_catalog(&self) -> &SkillCatalog {
    &self.catalog
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    Vec::new()
  }
}

pub fn load(db: &Database, ids: Vec<i64>) -> Task<Message> {
  Task::perform(async_load(db.clone(), ids), |loaded| {
    Message::DataLoaded(Box::new(loaded))
  })
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::CloseRequested => Task::none(),
    Message::DataLoaded(loaded) => {
      state.catalog = loaded.catalog;
      state.models.extend(loaded.models);
      state.loading = false;
      Task::none()
    }
    Message::GroupToggled(group_id) => {
      if !state.expanded.remove(&group_id) {
        state.expanded.insert(group_id);
      }
      Task::none()
    }
    Message::PickerQueryChanged(query) => {
      state.picker_query = query;
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      if !state.picker_open {
        state.picker_query.clear();
      }
      Task::none()
    }
    Message::PilotAdded(id) => {
      state.picker_open = false;
      state.picker_query.clear();

      let is_new = !state.selected_ids.contains(&id) && state.roster.iter().any(|pilot| pilot.id == id);
      if !is_new {
        return Task::none();
      }
      state.selected_ids.push(id);

      if state.models.contains_key(&id) {
        Task::none()
      } else {
        load(db, vec![id])
      }
    }
    Message::PilotRemoved(id) => {
      if state.can_remove() {
        state.selected_ids.retain(|selected| *selected != id);
      }
      Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let content = container(
    Column::with_children(vec![summary::summary(state), matrix::matrix(state)])
      .spacing(spacing::SPACE_6)
      .width(Length::Fill)
      .max_width(CONTENT_MAX_WIDTH),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Center)
  .padding(spacing::SPACE_6);

  let body = scrollable(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::ui::style::control::scrollbar);

  let base = container(
    Column::with_children(vec![header::header(state), body.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  });

  if state.picker_open() {
    let dropdown = container(header::dropdown(state))
      .width(Length::Fill)
      .align_x(Horizontal::Right)
      .padding(Padding {
        top: HEADER_HEIGHT + spacing::SPACE_2,
        right: spacing::SPACE_6,
        bottom: 0.0,
        left: spacing::SPACE_6,
      });

    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::PickerToggled),
      dropdown.into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  base.into()
}

async fn async_load(db: Database, ids: Vec<i64>) -> Loaded {
  let catalog = skills::skill_catalog(&db).await.unwrap_or(SkillCatalog {
    groups: Vec::new(),
  });

  let models =
    iced::futures::future::join_all(ids.into_iter().map(|id| load_pilot(db.clone(), catalog.clone(), id))).await;

  Loaded {
    catalog,
    models,
  }
}

async fn load_pilot(db: Database, catalog: SkillCatalog, id: i64) -> (i64, CompareModel) {
  let synced_skills = character::skills(&db, id).await.unwrap_or_default();
  let levels: HashMap<i64, u8> = synced_skills
    .iter()
    .map(|skill| {
      (
        skill.skill_id(),
        skill.trained_skill_level().clamp(0, i64::from(u8::MAX)) as u8,
      )
    })
    .collect();

  let total_sp = character::state(&db, id)
    .await
    .ok()
    .flatten()
    .and_then(|state| state.total_sp)
    .map(|sp| sp.max(0) as u64)
    .unwrap_or(0);

  (id, CompareModel::build(&catalog, levels, total_sp))
}
