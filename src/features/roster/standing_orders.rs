mod board;
mod detail;
mod modal;
mod ui;

use iced::{Element, Length, Task, widget::scrollable};

use crate::{
  store::{
    Database,
    model::{DossierObjectiveOrder, NewObjective, Objective, ObjectiveStatus, ObjectiveThreadEntry},
    repo::{character, dossier, objective},
  },
  ui::style::control,
};

#[derive(Clone, Debug)]
pub enum Message {
  Loaded(Box<Snapshot>),
  OpenObjective(i64),
  BackToBoard,
  TabSelected(ObjectiveStatus),
  Scrolled(f32),
  NewPressed,
  EditPressed(i64),
  TitleChanged(String),
  WhyChanged(String),
  TargetChanged(String),
  HorizonChanged(String),
  AccentSelected(String),
  AccentToggle,
  AccentHexChanged(String),
  AccentHexSubmitted,
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
  pub orders: Vec<DossierObjectiveOrder>,
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
  accent_open: bool,
  accent_hex: String,
  accent_hex_invalid: bool,
  pilots: Vec<i64>,
}

impl Draft {
  fn create() -> Self {
    let accent = default_accent();
    Draft {
      editing: None,
      title: String::new(),
      why: String::new(),
      target: String::new(),
      horizon: String::new(),
      accent_hex: accent.clone(),
      accent,
      accent_open: false,
      accent_hex_invalid: false,
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
      accent_hex: view.model.accent.clone(),
      accent: view.model.accent.clone(),
      accent_open: false,
      accent_hex_invalid: false,
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
  scroll_offset: f32,
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
      scroll_offset: 0.0,
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
  let message = match update_board(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match update_modal(state, db, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match update_draft(state, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  let message = match update_status(state, db, message) {
    Ok(task) => return task,
    Err(message) => message,
  };
  update_delete(state, db, message).unwrap_or_else(|_| Task::none())
}

fn update_board(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  Ok(match message {
    Message::Loaded(snapshot) => install_snapshot(state, *snapshot),
    Message::OpenObjective(id) => open_objective(state, id),
    Message::BackToBoard => back_to_board(state),
    Message::TabSelected(status) => set_tab(state, status),
    Message::Scrolled(offset) => {
      state.scroll_offset = offset.max(0.0);
      Task::none()
    }
    other => return Err(other),
  })
}

fn update_modal(state: &mut State, db: &Database, message: Message) -> Result<Task<Message>, Message> {
  Ok(match message {
    Message::NewPressed => open_create(state),
    Message::EditPressed(id) => open_edit(state, id),
    Message::ModalCancelled => close_modal(state),
    Message::ModalSubmitted => submit_modal(state, db),
    other => return Err(other),
  })
}

fn update_draft(state: &mut State, message: Message) -> Result<Task<Message>, Message> {
  Ok(match message {
    Message::TitleChanged(value) => edit_draft(state, |draft| draft.title = value),
    Message::WhyChanged(value) => edit_draft(state, |draft| draft.why = value),
    Message::TargetChanged(value) => edit_draft(state, |draft| draft.target = value),
    Message::HorizonChanged(value) => edit_draft(state, |draft| draft.horizon = value),
    Message::AccentSelected(hex) => edit_draft(state, |draft| {
      draft.accent = hex.clone();
      draft.accent_hex = hex;
      draft.accent_open = false;
      draft.accent_hex_invalid = false;
    }),
    Message::AccentToggle => edit_draft(state, |draft| {
      draft.accent_open = !draft.accent_open;
      if draft.accent_open {
        draft.accent_hex = draft.accent.clone();
        draft.accent_hex_invalid = false;
      }
    }),
    Message::AccentHexChanged(value) => edit_draft(state, |draft| {
      draft.accent_hex = value;
      draft.accent_hex_invalid = false;
    }),
    Message::AccentHexSubmitted => edit_draft(state, apply_accent_hex),
    Message::PilotToggled(id) => toggle_pilot(state, id),
    other => return Err(other),
  })
}

fn update_status(state: &mut State, db: &Database, message: Message) -> Result<Task<Message>, Message> {
  Ok(match message {
    Message::Complete(id) => apply(state, db, Mutation::Complete(id)),
    Message::Cancel(id) => apply(state, db, Mutation::Cancel(id)),
    Message::Reopen(id) => apply(state, db, Mutation::Reopen(id)),
    other => return Err(other),
  })
}

fn update_delete(state: &mut State, db: &Database, message: Message) -> Result<Task<Message>, Message> {
  Ok(match message {
    Message::DeleteRequested => request_delete(state),
    Message::DeleteCancelled => cancel_delete(state),
    Message::DeleteConfirmed(id) => confirm_delete(state, db, id),
    other => return Err(other),
  })
}

pub fn view(state: &State) -> Element<'_, Message> {
  match state.mode {
    Mode::Detail(id) => match state.objective(id) {
      Some(objective) => scrollable(detail::view(objective, &state.roster, state.confirm_delete))
        .style(control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
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
  state.scroll_offset = 0.0;
  Task::none()
}

fn back_to_board(state: &mut State) -> Task<Message> {
  state.mode = Mode::Board;
  state.confirm_delete = false;
  state.scroll_offset = 0.0;
  Task::none()
}

fn set_tab(state: &mut State, status: ObjectiveStatus) -> Task<Message> {
  state.tab = status;
  state.scroll_offset = 0.0;
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

fn apply_accent_hex(draft: &mut Draft) {
  match crate::ui::components::color_picker::normalize_hex(&draft.accent_hex) {
    Some(hex) => {
      draft.accent = hex.clone();
      draft.accent_hex = hex;
      draft.accent_hex_invalid = false;
      draft.accent_open = false;
    }
    None => draft.accent_hex_invalid = !draft.accent_hex.trim().is_empty(),
  }
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
    let orders = dossier::orders_for_objective(db, model.id).await.unwrap_or_default();
    objectives.push(ObjectiveView {
      model,
      pilots,
      thread,
      orders,
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

  async fn seed_pilot(db: &Database, id: i64, name: &str) {
    use crate::store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
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
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
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

    #[tokio::test]
    async fn it_loads_marching_orders_linked_to_each_objective_across_the_roster() {
      let db = crate::store::open_test().await.unwrap();
      seed_pilot(&db, 90_000_001, "Alpha").await;
      seed_pilot(&db, 90_000_002, "Bravo").await;
      let target = objective::create(&db, &objective("Fund a Nyx")).await.unwrap();
      let bare = objective::create(&db, &objective("Bare")).await.unwrap();

      let alpha = dossier::add_order(&db, 90_000_001, "Alpha saves").await.unwrap();
      let bravo = dossier::add_order(&db, 90_000_002, "Bravo saves").await.unwrap();
      dossier::set_objective(&db, alpha.id, target.id).await.unwrap();
      dossier::set_objective(&db, bravo.id, target.id).await.unwrap();

      let snapshot = build_snapshot(&db).await;

      let linked = snapshot
        .objectives
        .iter()
        .find(|view| view.model.id == target.id)
        .unwrap();
      assert_eq!(linked.orders.len(), 2);
      assert_eq!(linked.orders[0].character_name, "Alpha");
      assert_eq!(linked.orders[0].text, "Alpha saves");
      assert_eq!(linked.orders[1].character_name, "Bravo");

      let empty = snapshot
        .objectives
        .iter()
        .find(|view| view.model.id == bare.id)
        .unwrap();
      assert!(empty.orders.is_empty());
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
    async fn it_edits_every_draft_field_through_the_edit_modal() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &objective("Editable")).await.unwrap();
      let mut state = loaded(&db).await;

      let _ = update(&mut state, Message::EditPressed(created.id), &db);
      assert_eq!(state.draft.as_ref().unwrap().editing, Some(created.id));

      let _ = update(&mut state, Message::WhyChanged("Because".to_owned()), &db);
      let _ = update(&mut state, Message::TargetChanged("Ten kills".to_owned()), &db);
      let _ = update(&mut state, Message::HorizonChanged("This week".to_owned()), &db);
      let _ = update(&mut state, Message::AccentSelected("#C07AD9".to_owned()), &db);

      let draft = state.draft.as_ref().unwrap();
      assert_eq!(draft.why, "Because");
      assert_eq!(draft.target, "Ten kills");
      assert_eq!(draft.horizon, "This week");
      assert_eq!(draft.accent, "#C07AD9");
    }

    #[tokio::test]
    async fn it_installs_a_loaded_snapshot_and_selects_a_tab() {
      let db = crate::store::open_test().await.unwrap();
      objective::create(&db, &objective("Loaded")).await.unwrap();
      let mut state = State::new();

      let snapshot = build_snapshot(&db).await;
      let _ = update(&mut state, Message::Loaded(Box::new(snapshot)), &db);
      assert_eq!(state.total_count(), 1);

      let _ = update(&mut state, Message::TabSelected(ObjectiveStatus::Complete), &db);
      assert_eq!(state.tab, ObjectiveStatus::Complete);
    }

    #[tokio::test]
    async fn it_dispatches_status_mutations_and_the_delete_flow() {
      let db = crate::store::open_test().await.unwrap();
      let created = objective::create(&db, &objective("Cycle")).await.unwrap();
      let mut state = loaded(&db).await;

      let _ = update(&mut state, Message::Complete(created.id), &db);
      let _ = update(&mut state, Message::Cancel(created.id), &db);
      let _ = update(&mut state, Message::Reopen(created.id), &db);

      let _ = update(&mut state, Message::OpenObjective(created.id), &db);
      let _ = update(&mut state, Message::DeleteRequested, &db);
      assert!(state.confirm_delete);
      let _ = update(&mut state, Message::DeleteCancelled, &db);
      assert!(!state.confirm_delete);
      let _ = update(&mut state, Message::DeleteConfirmed(created.id), &db);
      assert_eq!(state.mode, Mode::Board);
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

    #[tokio::test]
    async fn it_renders_the_detail_marching_orders_for_a_linked_objective() {
      let db = crate::store::open_test().await.unwrap();
      seed_pilot(&db, 90_000_001, "Alpha").await;
      let target = objective::create(&db, &objective("Fund a Nyx")).await.unwrap();
      let order = dossier::add_order(&db, 90_000_001, "Alpha saves").await.unwrap();
      dossier::set_objective(&db, order.id, target.id).await.unwrap();
      dossier::complete_order(&db, order.id).await.unwrap();
      let mut state = loaded(&db).await;

      let _ = update(&mut state, Message::OpenObjective(target.id), &db);
      let linked = state.objective(target.id).unwrap();
      assert_eq!(linked.orders.len(), 1);
      assert_eq!(linked.orders[0].status, "complete");
      drop(view(&state));
    }

    #[tokio::test]
    async fn it_renders_the_detail_when_no_marching_orders_link() {
      let db = crate::store::open_test().await.unwrap();
      let bare = objective::create(&db, &objective("Bare")).await.unwrap();
      let mut state = loaded(&db).await;

      let _ = update(&mut state, Message::OpenObjective(bare.id), &db);
      assert!(state.objective(bare.id).unwrap().orders.is_empty());
      drop(view(&state));
    }
  }
}
