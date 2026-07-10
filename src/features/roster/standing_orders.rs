mod board;
mod detail;
mod modal;
mod ui;

use iced::{Element, Task};

use crate::store::{
  Database,
  model::{NewObjective, Objective, ObjectiveStatus, ObjectiveThreadEntry},
  repo::{character, objective},
};

#[derive(Clone, Debug)]
pub enum Message {
  Loaded(Box<Snapshot>),
  OpenObjective(i64),
  BackToBoard,
  TabSelected(ObjectiveStatus),
  NewPressed,
  EditPressed(i64),
  TitleChanged(String),
  WhyChanged(String),
  TargetChanged(String),
  HorizonChanged(String),
  AccentSelected(String),
  PilotToggled(i64),
  ModalCancelled,
  ModalSubmitted,
  Complete(i64),
  Cancel(i64),
  Reopen(i64),
  DeleteRequested,
  DeleteCancelled,
  DeleteConfirmed(i64),
}

#[derive(Clone, Debug)]
pub struct PilotRef {
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug)]
pub struct ObjectiveView {
  pub model: Objective,
  pub pilots: Vec<i64>,
  pub thread: Vec<ObjectiveThreadEntry>,
}

impl ObjectiveView {
  pub fn status(&self) -> ObjectiveStatus {
    ObjectiveStatus::parse(&self.model.status).unwrap_or_default()
  }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
  objectives: Vec<ObjectiveView>,
  roster: Vec<PilotRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
  Board,
  Detail(i64),
}

#[derive(Clone, Debug)]
struct Draft {
  editing: Option<i64>,
  title: String,
  why: String,
  target: String,
  horizon: String,
  accent: String,
  pilots: Vec<i64>,
}

impl Draft {
  fn create() -> Self {
    Draft {
      editing: None,
      title: String::new(),
      why: String::new(),
      target: String::new(),
      horizon: String::new(),
      accent: default_accent(),
      pilots: Vec::new(),
    }
  }

  fn edit(view: &ObjectiveView) -> Self {
    Draft {
      editing: Some(view.model.id),
      title: view.model.title.clone(),
      why: view.model.why.clone().unwrap_or_default(),
      target: view.model.target.clone().unwrap_or_default(),
      horizon: view.model.horizon.clone().unwrap_or_default(),
      accent: view.model.accent.clone(),
      pilots: view.pilots.clone(),
    }
  }

  fn new_objective(&self) -> NewObjective {
    NewObjective {
      accent: self.accent.clone(),
      horizon: non_blank(&self.horizon),
      target: non_blank(&self.target),
      title: self.title.trim().to_owned(),
      why: non_blank(&self.why),
    }
  }
}

pub struct State {
  loaded: bool,
  objectives: Vec<ObjectiveView>,
  roster: Vec<PilotRef>,
  tab: ObjectiveStatus,
  mode: Mode,
  draft: Option<Draft>,
  confirm_delete: bool,
}

impl State {
  pub fn new() -> Self {
    State {
      loaded: false,
      objectives: Vec::new(),
      roster: Vec::new(),
      tab: ObjectiveStatus::Active,
      mode: Mode::Board,
      draft: None,
      confirm_delete: false,
    }
  }

  pub fn active_count(&self) -> usize {
    self.count_of(ObjectiveStatus::Active)
  }

  pub fn total_count(&self) -> usize {
    self.objectives.len()
  }

  fn count_of(&self, status: ObjectiveStatus) -> usize {
    self.objectives.iter().filter(|view| view.status() == status).count()
  }

  fn with_status(&self, status: ObjectiveStatus) -> Vec<&ObjectiveView> {
    self.objectives.iter().filter(|view| view.status() == status).collect()
  }

  fn objective(&self, id: i64) -> Option<&ObjectiveView> {
    self.objectives.iter().find(|view| view.model.id == id)
  }
}

enum Mutation {
  Create(NewObjective, Vec<i64>),
  Update(i64, NewObjective, Vec<i64>),
  Complete(i64),
  Cancel(i64),
  Reopen(i64),
  Delete(i64),
}

pub fn load(db: &Database) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { Box::new(build_snapshot(&db).await) }, Message::Loaded)
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Loaded(snapshot) => install_snapshot(state, *snapshot),
    Message::OpenObjective(id) => open_objective(state, id),
    Message::BackToBoard => back_to_board(state),
    Message::TabSelected(status) => set_tab(state, status),
    Message::NewPressed => open_create(state),
    Message::EditPressed(id) => open_edit(state, id),
    Message::TitleChanged(value) => edit_draft(state, |draft| draft.title = value),
    Message::WhyChanged(value) => edit_draft(state, |draft| draft.why = value),
    Message::TargetChanged(value) => edit_draft(state, |draft| draft.target = value),
    Message::HorizonChanged(value) => edit_draft(state, |draft| draft.horizon = value),
    Message::AccentSelected(hex) => edit_draft(state, |draft| draft.accent = hex),
    Message::PilotToggled(id) => toggle_pilot(state, id),
    Message::ModalCancelled => close_modal(state),
    Message::ModalSubmitted => submit_modal(state, db),
    Message::Complete(id) => apply(state, db, Mutation::Complete(id)),
    Message::Cancel(id) => apply(state, db, Mutation::Cancel(id)),
    Message::Reopen(id) => apply(state, db, Mutation::Reopen(id)),
    Message::DeleteRequested => request_delete(state),
    Message::DeleteCancelled => cancel_delete(state),
    Message::DeleteConfirmed(id) => confirm_delete(state, db, id),
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  match state.mode {
    Mode::Detail(id) => match state.objective(id) {
      Some(objective) => detail::view(objective, &state.roster, state.confirm_delete),
      None => board::view(state),
    },
    Mode::Board => board::view(state),
  }
}

pub fn overlay_layers(state: &State) -> Vec<Element<'_, Message>> {
  match &state.draft {
    Some(draft) => {
      crate::ui::components::modal_overlay::modal_layers(Message::ModalCancelled, modal::view(draft, &state.roster))
    }
    None => Vec::new(),
  }
}

pub fn escape_dismiss(state: &State) -> Option<Message> {
  if state.draft.is_some() {
    return Some(Message::ModalCancelled);
  }
  match state.mode {
    Mode::Detail(_) => Some(Message::BackToBoard),
    Mode::Board => None,
  }
}

fn install_snapshot(state: &mut State, snapshot: Snapshot) -> Task<Message> {
  state.objectives = snapshot.objectives;
  state.roster = snapshot.roster;
  state.loaded = true;
  if let Mode::Detail(id) = state.mode
    && state.objective(id).is_none()
  {
    state.mode = Mode::Board;
    state.confirm_delete = false;
  }
  Task::none()
}

fn open_objective(state: &mut State, id: i64) -> Task<Message> {
  state.mode = Mode::Detail(id);
  state.confirm_delete = false;
  Task::none()
}

fn back_to_board(state: &mut State) -> Task<Message> {
  state.mode = Mode::Board;
  state.confirm_delete = false;
  Task::none()
}

fn set_tab(state: &mut State, status: ObjectiveStatus) -> Task<Message> {
  state.tab = status;
  Task::none()
}

fn open_create(state: &mut State) -> Task<Message> {
  state.draft = Some(Draft::create());
  Task::none()
}

fn open_edit(state: &mut State, id: i64) -> Task<Message> {
  if let Some(view) = state.objective(id) {
    state.draft = Some(Draft::edit(view));
  }
  Task::none()
}

fn edit_draft(state: &mut State, apply: impl FnOnce(&mut Draft)) -> Task<Message> {
  if let Some(draft) = state.draft.as_mut() {
    apply(draft);
  }
  Task::none()
}

fn toggle_pilot(state: &mut State, id: i64) -> Task<Message> {
  if let Some(draft) = state.draft.as_mut() {
    if let Some(index) = draft.pilots.iter().position(|pilot| *pilot == id) {
      draft.pilots.remove(index);
    } else {
      draft.pilots.push(id);
    }
  }
  Task::none()
}

fn close_modal(state: &mut State) -> Task<Message> {
  state.draft = None;
  Task::none()
}

fn submit_modal(state: &mut State, db: &Database) -> Task<Message> {
  let Some(draft) = state.draft.take() else {
    return Task::none();
  };
  if draft.title.trim().is_empty() {
    state.draft = Some(draft);
    return Task::none();
  }
  let mutation = match draft.editing {
    Some(id) => Mutation::Update(id, draft.new_objective(), draft.pilots.clone()),
    None => Mutation::Create(draft.new_objective(), draft.pilots.clone()),
  };
  apply(state, db, mutation)
}

fn request_delete(state: &mut State) -> Task<Message> {
  state.confirm_delete = true;
  Task::none()
}

fn cancel_delete(state: &mut State) -> Task<Message> {
  state.confirm_delete = false;
  Task::none()
}

fn confirm_delete(state: &mut State, db: &Database, id: i64) -> Task<Message> {
  state.mode = Mode::Board;
  state.confirm_delete = false;
  apply(state, db, Mutation::Delete(id))
}

fn apply(_state: &State, db: &Database, mutation: Mutation) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      run_mutation(&db, mutation).await;
      Box::new(build_snapshot(&db).await)
    },
    Message::Loaded,
  )
}

async fn run_mutation(db: &Database, mutation: Mutation) {
  match mutation {
    Mutation::Create(input, pilots) => {
      if let Ok(created) = objective::create(db, &input).await {
        for pilot in pilots {
          let _ = objective::assign_pilot(db, created.id, pilot).await;
        }
      }
    }
    Mutation::Update(id, input, pilots) => {
      let _ = objective::update(db, id, &input).await;
      sync_pilots(db, id, &pilots).await;
    }
    Mutation::Complete(id) => {
      let _ = objective::complete(db, id).await;
    }
    Mutation::Cancel(id) => {
      let _ = objective::cancel(db, id).await;
    }
    Mutation::Reopen(id) => {
      let _ = objective::reopen(db, id).await;
    }
    Mutation::Delete(id) => {
      let _ = objective::delete(db, id).await;
    }
  }
}

async fn sync_pilots(db: &Database, id: i64, desired: &[i64]) {
  let current = objective::pilots(db, id).await.unwrap_or_default();
  for pilot in desired {
    if !current.contains(pilot) {
      let _ = objective::assign_pilot(db, id, *pilot).await;
    }
  }
  for pilot in &current {
    if !desired.contains(pilot) {
      let _ = objective::unassign_pilot(db, id, *pilot).await;
    }
  }
}

async fn build_snapshot(db: &Database) -> Snapshot {
  let models = objective::list(db, None).await.unwrap_or_default();
  let mut objectives = Vec::with_capacity(models.len());
  for model in models {
    let pilots = objective::pilots(db, model.id).await.unwrap_or_default();
    let thread = objective::thread(db, model.id)
      .await
      .unwrap_or_default()
      .into_iter()
      .filter(|entry| entry.text.as_deref().is_some_and(|text| !text.trim().is_empty()))
      .collect();
    objectives.push(ObjectiveView {
      model,
      pilots,
      thread,
    });
  }

  let roster = character::all_owned(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|character| PilotRef {
      id: character.id(),
      name: character.name().clone(),
    })
    .collect();

  Snapshot {
    objectives,
    roster,
  }
}

fn default_accent() -> String {
  crate::ui::components::color_picker::PALETTE
    .first()
    .map(|preset| preset.hex.to_owned())
    .unwrap_or_else(|| "#3FB8DB".to_owned())
}

fn non_blank(value: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_owned())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn objective(title: &str) -> NewObjective {
    NewObjective {
      accent: "#5BB97E".to_owned(),
      horizon: Some("This month".to_owned()),
      target: Some("Ten clean kills".to_owned()),
      title: title.to_owned(),
      why: Some("Stay sharp".to_owned()),
    }
  }

  async fn loaded(db: &Database) -> State {
    let mut state = State::new();
    let _ = install_snapshot(&mut state, build_snapshot(db).await);
    state
  }

  mod mutations {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_an_objective_with_assigned_pilots_absent_from_the_roster() {
      let db = crate::store::open_test().await.unwrap();

      run_mutation(&db, Mutation::Create(objective("Fund a Nyx"), Vec::new())).await;

      let rows = objective::list(&db, None).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].title, "Fund a Nyx");
      assert_eq!(rows[0].status, "active");
    }

    #[tokio::test]
    async fn it_updates_the_editable_fields_and_keeps_the_status() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &objective("Draft")).await.unwrap();

      let mut edit = objective("Renamed");
      edit.accent = "#C07AD9".to_owned();
      run_mutation(&db, Mutation::Update(created.id, edit, Vec::new())).await;

      let fetched = objective::get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(fetched.title, "Renamed");
      assert_eq!(fetched.accent, "#C07AD9");
      assert_eq!(fetched.status, "active");
    }

    #[tokio::test]
    async fn it_walks_an_objective_through_complete_cancel_and_reopen() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &objective("Cycle")).await.unwrap();

      run_mutation(&db, Mutation::Complete(created.id)).await;
      assert_eq!(
        objective::get(&db, created.id).await.unwrap().unwrap().status,
        "complete"
      );

      run_mutation(&db, Mutation::Cancel(created.id)).await;
      assert_eq!(
        objective::get(&db, created.id).await.unwrap().unwrap().status,
        "cancelled"
      );

      run_mutation(&db, Mutation::Reopen(created.id)).await;
      assert_eq!(objective::get(&db, created.id).await.unwrap().unwrap().status, "active");
    }

    #[tokio::test]
    async fn it_deletes_an_objective() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &objective("Scrap")).await.unwrap();

      run_mutation(&db, Mutation::Delete(created.id)).await;

      assert_eq!(objective::get(&db, created.id).await.unwrap(), None);
    }
  }

  mod snapshot {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_loads_objectives_newest_first_and_filters_empty_thread_text() {
      let db = crate::store::open_test().await.unwrap();
      let first = objective::create(&db, &objective("One")).await.unwrap();
      objective::create(&db, &objective("Two")).await.unwrap();
      objective::set_link(
        &db,
        first.id,
        "2026-07-04",
        &crate::store::model::LinkSource::LogAnswer {
          question_id: "goal".to_owned(),
        },
      )
      .await
      .unwrap();

      let snapshot = build_snapshot(&db).await;

      assert_eq!(snapshot.objectives.len(), 2);
      assert_eq!(snapshot.objectives[0].model.title, "Two");
      // The orphaned link has no answer text, so it is filtered out of the thread.
      assert!(snapshot.objectives[1].thread.is_empty());
    }
  }

  mod flow {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_opens_and_closes_the_create_modal() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded(&db).await;

      let _ = update(&mut state, Message::NewPressed, &db);
      assert!(state.draft.is_some());

      let _ = update(&mut state, Message::TitleChanged("Break the doctrine".to_owned()), &db);
      assert_eq!(state.draft.as_ref().unwrap().title, "Break the doctrine");

      let _ = update(&mut state, Message::ModalCancelled, &db);
      assert!(state.draft.is_none());
    }

    #[tokio::test]
    async fn it_ignores_a_blank_title_submission() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded(&db).await;
      let _ = update(&mut state, Message::NewPressed, &db);

      let _ = update(&mut state, Message::ModalSubmitted, &db);

      assert!(state.draft.is_some(), "a blank objective keeps the modal open");
    }

    #[tokio::test]
    async fn it_routes_between_the_board_and_a_detail_view() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &objective("Anchor")).await.unwrap();
      let mut state = loaded(&db).await;

      let _ = update(&mut state, Message::OpenObjective(created.id), &db);
      assert_eq!(state.mode, Mode::Detail(created.id));

      let _ = update(&mut state, Message::DeleteRequested, &db);
      assert!(state.confirm_delete);

      let _ = update(&mut state, Message::BackToBoard, &db);
      assert_eq!(state.mode, Mode::Board);
      assert!(!state.confirm_delete);
    }

    #[tokio::test]
    async fn it_toggles_a_pilot_in_the_draft() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded(&db).await;
      let _ = update(&mut state, Message::NewPressed, &db);

      let _ = update(&mut state, Message::PilotToggled(90_000_001), &db);
      assert_eq!(state.draft.as_ref().unwrap().pilots, vec![90_000_001]);

      let _ = update(&mut state, Message::PilotToggled(90_000_001), &db);
      assert!(state.draft.as_ref().unwrap().pilots.is_empty());
    }

    #[tokio::test]
    async fn it_counts_objectives_by_status() {
      let db = crate::store::open_test().await.unwrap();
      let first = objective::create(&db, &objective("Live")).await.unwrap();
      objective::create(&db, &objective("Also live")).await.unwrap();
      objective::complete(&db, first.id).await.unwrap();
      let state = loaded(&db).await;

      assert_eq!(state.total_count(), 2);
      assert_eq!(state.active_count(), 1);
      assert_eq!(state.count_of(ObjectiveStatus::Complete), 1);
    }
  }

  mod render {
    use super::*;

    #[tokio::test]
    async fn it_renders_the_board_and_a_detail_view() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &objective("Render")).await.unwrap();
      let mut state = loaded(&db).await;

      drop(view(&state));

      let _ = update(&mut state, Message::OpenObjective(created.id), &db);
      drop(view(&state));

      let _ = update(&mut state, Message::NewPressed, &db);
      assert_eq!(overlay_layers(&state).len(), 2);
    }
  }
}
