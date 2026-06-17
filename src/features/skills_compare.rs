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
    components::{backdrop, modal_overlay::modal_overlay},
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

impl Message {
  /// Whether handling this message can surface new image-bearing rows (selected pilot portraits), so the shell
  /// should recheck for stale images. Interaction-only messages return `false` to keep the staleness scan off the
  /// per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::DataLoaded(_) | Message::PilotAdded(_))
  }
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

  pub(super) fn portrait(&self, pilot_id: i64) -> images::ImageState {
    images::resolve(&images::default_store(), images::ImageKind::CharacterPortrait, pilot_id)
  }

  pub(crate) fn selected_ids(&self) -> &[i64] {
    &self.selected_ids
  }

  pub(super) fn skill_catalog(&self) -> &SkillCatalog {
    &self.catalog
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .selected_ids
      .iter()
      .filter_map(|id| self.portrait(*id).stale_key())
      .collect()
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

    let overlay = Stack::with_children(vec![backdrop::click_catcher(Message::PickerToggled), dropdown.into()])
      .width(Length::Fill)
      .height(Length::Fill)
      .into();
    return modal_overlay(base.into(), None, overlay);
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

#[cfg(test)]
mod tests {
  use iced::Color;

  use super::*;
  use crate::features::skills::browse::{AttrKey, SkillCatalogEntry, SkillCatalogGroup};

  type PilotSeed<'a> = (i64, &'a [(i64, u8)], u64);

  fn pilot(id: i64) -> OwnedPilot {
    OwnedPilot {
      color: Color::WHITE,
      granted_scopes: None,
      id,
      name: format!("Pilot {id}"),
    }
  }

  fn entry(type_id: i64, group_id: i64, rank: u8) -> SkillCatalogEntry {
    SkillCatalogEntry {
      group_id,
      group_name: "Group".to_owned(),
      name: format!("Skill {type_id}"),
      primary_attr: AttrKey::Intelligence,
      prereqs: Vec::new(),
      rank,
      secondary_attr: AttrKey::Memory,
      type_id,
    }
  }

  fn catalog() -> SkillCatalog {
    SkillCatalog {
      groups: vec![
        SkillCatalogGroup {
          id: 1,
          name: "Gunnery".to_owned(),
          skills: vec![entry(10, 1, 1), entry(11, 1, 2)],
        },
        SkillCatalogGroup {
          id: 2,
          name: "Missiles".to_owned(),
          skills: vec![entry(20, 2, 4)],
        },
      ],
    }
  }

  fn compare_model(levels: &[(i64, u8)], total_sp: u64) -> CompareModel {
    CompareModel::build(&catalog(), levels.iter().copied().collect(), total_sp)
  }

  fn loaded(ids: &[PilotSeed]) -> Loaded {
    Loaded {
      catalog: catalog(),
      models: ids
        .iter()
        .map(|(id, levels, total_sp)| (*id, compare_model(levels, *total_sp)))
        .collect(),
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_a_stale_key_for_each_uncached_pilot() {
      let roster = vec![pilot(900_000_001), pilot(900_000_002)];
      let state = State::new(vec![900_000_001, 900_000_002], roster);

      let stale = state.stale_images();

      assert_eq!(
        stale,
        vec![
          (images::ImageKind::CharacterPortrait, 900_000_001),
          (images::ImageKind::CharacterPortrait, 900_000_002),
        ]
      );
    }

    #[test]
    fn it_is_empty_with_no_selected_pilots() {
      let state = State::new(Vec::new(), Vec::new());

      assert_eq!(state.stale_images(), Vec::new());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stores_the_loaded_catalog_and_models_and_clears_loading() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2)]);

      let _ = update(
        &mut state,
        Message::DataLoaded(Box::new(loaded(&[
          (1, &[(10, 5)], 9_000_000),
          (2, &[(10, 1)], 1_000_000),
        ]))),
        &db,
      );

      assert!(!state.loading);
      assert_eq!(state.skill_catalog().groups.len(), 2);
      assert!(state.model(1).is_some());
      assert!(state.model(2).is_some());
    }

    #[tokio::test]
    async fn it_toggles_a_group_open_then_closed() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2)]);

      let _ = update(&mut state, Message::GroupToggled(1), &db);
      assert!(state.is_expanded(1));

      let _ = update(&mut state, Message::GroupToggled(1), &db);
      assert!(!state.is_expanded(1));
    }

    #[tokio::test]
    async fn it_records_the_picker_query() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2)]);

      let _ = update(&mut state, Message::PickerQueryChanged("alt".to_owned()), &db);

      assert_eq!(state.picker_query(), "alt");
    }

    #[tokio::test]
    async fn it_clears_the_query_when_the_picker_closes() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2)]);
      let _ = update(&mut state, Message::PickerQueryChanged("alt".to_owned()), &db);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open());
      assert_eq!(state.picker_query(), "alt");

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open());
      assert_eq!(state.picker_query(), "");
    }

    #[tokio::test]
    async fn it_adds_a_rostered_pilot_and_closes_the_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2), pilot(3)]);
      let _ = update(&mut state, Message::PickerToggled, &db);

      let _ = update(&mut state, Message::PilotAdded(3), &db);

      assert_eq!(state.selected_ids(), &[1, 2, 3]);
      assert!(!state.picker_open());
    }

    #[tokio::test]
    async fn it_ignores_adding_an_already_selected_pilot() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2)]);

      let _ = update(&mut state, Message::PilotAdded(2), &db);

      assert_eq!(state.selected_ids(), &[1, 2]);
    }

    #[tokio::test]
    async fn it_ignores_adding_a_pilot_outside_the_roster() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2)]);

      let _ = update(&mut state, Message::PilotAdded(99), &db);

      assert_eq!(state.selected_ids(), &[1, 2]);
    }

    #[tokio::test]
    async fn it_removes_a_pilot_only_when_more_than_two_remain() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2, 3], vec![pilot(1), pilot(2), pilot(3)]);

      let _ = update(&mut state, Message::PilotRemoved(3), &db);
      assert_eq!(state.selected_ids(), &[1, 2]);

      let _ = update(&mut state, Message::PilotRemoved(2), &db);
      assert_eq!(state.selected_ids(), &[1, 2]);
    }

    #[tokio::test]
    async fn it_treats_a_close_request_as_a_no_op() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(vec![1, 2], vec![pilot(1), pilot(2)]);

      let _ = update(&mut state, Message::CloseRequested, &db);

      assert_eq!(state.selected_ids(), &[1, 2]);
    }
  }

  mod view {
    use super::*;

    async fn populated() -> (State, Database) {
      let db = crate::store::open_test().await.unwrap();
      let roster = vec![pilot(1), pilot(2), pilot(3), pilot(4)];
      let mut state = State::new(vec![1, 2, 3], roster);
      let _ = update(
        &mut state,
        Message::DataLoaded(Box::new(loaded(&[
          (1, &[(10, 5), (11, 5), (20, 5)], 9_000_000),
          (2, &[(10, 1), (11, 0), (20, 0)], 1_000_000),
          (3, &[(10, 4), (11, 4), (20, 4)], 5_000_000),
        ]))),
        &db,
      );
      (state, db)
    }

    #[tokio::test]
    async fn it_renders_the_collapsed_matrix() {
      let (state, _db) = populated().await;

      let _el: Element<'_, Message> = view(&state);
    }

    #[tokio::test]
    async fn it_renders_an_expanded_group() {
      let (mut state, db) = populated().await;
      let _ = update(&mut state, Message::GroupToggled(1), &db);

      let _el: Element<'_, Message> = view(&state);
    }

    #[tokio::test]
    async fn it_renders_the_picker_with_an_available_pilot() {
      let (mut state, db) = populated().await;
      let _ = update(&mut state, Message::PickerToggled, &db);

      let _el: Element<'_, Message> = view(&state);
    }

    #[tokio::test]
    async fn it_renders_the_picker_empty_state_when_no_pilot_matches() {
      let (mut state, db) = populated().await;
      let _ = update(&mut state, Message::PickerToggled, &db);
      let _ = update(&mut state, Message::PickerQueryChanged("zzzz".to_owned()), &db);

      let _el: Element<'_, Message> = view(&state);
    }
  }
}
