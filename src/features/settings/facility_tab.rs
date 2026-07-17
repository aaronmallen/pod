use std::{
  cmp::Ordering,
  collections::{BTreeSet, HashMap},
};

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text, text_editor},
};

use super::{Outcome, facility_intel_fit, facility_intel_import, facility_intel_share};
use crate::{
  config::Settings,
  features::industry::{
    PlannerFacility,
    facility_owner::resolve_facility_owner,
    rig_bonuses::{self, DerivedRigBonuses, RigBonus},
  },
  services::location_search::{LocationRef, LocationTier},
  store::{
    Database,
    model::FacilityIntel,
    repo::{industry, market, sde},
  },
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      button::{Button, Size},
      facility_combobox::{self, FacilityCombobox, FacilityRef, FacilitySearch},
      icon::Icon,
      location_combobox::LocationCombobox,
      modal_overlay::{modal_layers, stable_overlay},
      rig_combobox::{Activity as RigActivity, RigCombobox, RigRef, RigSearch, rigs_for_structure},
      rule, status,
    },
    style::{color, radius, spacing, typography},
  },
};

const COMPOSER_ACTIVITY_ID: i64 = 0;
const DB_MANUFACTURING_ACTIVITY_ID: i64 = industry::MANUFACTURING_ACTIVITY_ID;
const DB_REACTION_ACTIVITY_ID: i64 = industry::REACTION_ACTIVITY_ID;
const EXPORT_LIST_MAX_HEIGHT: f32 = 300.0;
const EXPORT_PANEL_MAX_HEIGHT: f32 = 680.0;
const EXPORT_PANEL_MAX_WIDTH: f32 = 560.0;
const FIT_EDITOR_HEIGHT: f32 = 150.0;
const FIT_PANEL_MAX_WIDTH: f32 = 520.0;
const FIT_PREVIEW_MAX_HEIGHT: f32 = 260.0;
const MANUFACTURING_ACTIVITY_ID: i64 = 1;
const MARKET_SEARCH_MIN_CHARS: usize = 2;
/// Facility ids at or above this are player-owned structures; below are NPC stations.
const MIN_STRUCTURE_ID: i64 = 1_000_000_000_000;
const PANEL_SIDE_PADDING: f32 = 36.0;
const PICKER_MAX_WIDTH: f32 = 600.0;
// Reserves a fixed column for the stacked type/sec pills sized to the widest realistic type badge
// (~9 monospace chars, e.g. "Structure"/"Fortizar") so the facility name wraps at the pill boundary
// rather than colliding with it.
const PILL_COLUMN_WIDTH: f32 = 72.0;
const REACTION_ACTIVITY_ID: i64 = 11;
const RIG_POPOVER_WIDTH: f32 = 320.0;
const RIG_SLOTS: usize = 3;
const GRID_COLUMNS: usize = 3;
const SORT_MENU_WIDTH: f32 = 220.0;

const ACTIVITIES: [Activity; 2] = [
  Activity {
    blurb_key: "settings.facility.manufacturing_blurb",
    id: MANUFACTURING_ACTIVITY_ID,
    name_key: "settings.facility.manufacturing_name",
    placeholder_key: "settings.facility.ask_each_install",
  },
  Activity {
    blurb_key: "settings.facility.reactions_blurb",
    id: REACTION_ACTIVITY_ID,
    name_key: "settings.facility.reactions_name",
    placeholder_key: "settings.facility.ask_each_install",
  },
];

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
  Cleared {
    activity: i64,
  },
  ComposerToggled(bool),
  ExportAllSelected,
  ExportClosed,
  ExportConfirmed,
  ExportFacilityToggled(i64),
  ExportNoneSelected,
  ExportOpened,
  FacilityPicked {
    activity: i64,
    facility: FacilityRef,
  },
  FitApplied,
  FitClosed,
  FitInputChanged(text_editor::Action),
  FitOpened {
    facility_id: i64,
  },
  ImportErrorDismissed,
  ImportFileLoaded(Option<String>),
  ImportFinished(facility_intel_import::ImportSummary),
  ImportOpened,
  ImportResultClosed,
  Loaded(Box<Result<Loaded, String>>),
  MarketCleared,
  MarketPicked(LocationRef),
  MarketPickerToggled,
  MarketQueryChanged(String),
  MarketResults {
    generation: u64,
    results: Vec<LocationRef>,
  },
  PickerToggled {
    activity: i64,
  },
  QueryChanged {
    activity: i64,
    query: String,
  },
  RemoveFacility(i64),
  RigCleared {
    facility_id: i64,
    slot: usize,
  },
  RigDismissed,
  RigPicked {
    facility_id: i64,
    rig: Box<RigRef>,
    slot: usize,
  },
  RigQueryChanged {
    facility_id: i64,
    query: String,
    slot: usize,
  },
  RigSlotToggled {
    facility_id: i64,
    slot: usize,
  },
  Saved(Result<(), String>),
  SearchResults {
    activity: i64,
    generation: u64,
    results: Vec<PlannerFacility>,
  },
  SortChanged(SortBy),
  SortMenuDismissed,
  SortMenuToggled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
  facilities_count: usize,
  intel: Vec<IntelCard>,
  manufacturing: Option<FacilityRef>,
  market: Option<LocationRef>,
  reactions: Option<FacilityRef>,
  rig_catalog: HashMap<i64, RigBonus>,
  rigs: Vec<RigRef>,
}

#[derive(Debug, Default)]
pub struct State {
  clients: Option<crate::features::industry::Clients>,
  composer: Picker,
  db: Option<Database>,
  export: Option<ExportDraft>,
  facilities_count: usize,
  fit: Option<FitDraft>,
  import_error: Option<facility_intel_share::ParseError>,
  import_result: Option<facility_intel_import::ImportSummary>,
  intel: Vec<IntelCard>,
  load_error: Option<String>,
  manufacturing: Picker,
  market: MarketPicker,
  open_rig: Option<OpenRig>,
  reactions: Picker,
  rig_catalog: HashMap<i64, RigBonus>,
  rigs: Vec<RigRef>,
  sort: SortBy,
  sort_open: bool,
}

impl State {
  pub fn new(db: Database) -> Self {
    State {
      db: Some(db),
      ..State::default()
    }
  }

  pub fn set_clients(
    &mut self,
    esi: std::sync::Arc<crate::clients::esi::Client>,
    sso: std::sync::Arc<crate::clients::eve_sso::Client>,
  ) {
    self.clients = Some(crate::features::industry::Clients {
      esi,
      sso,
    });
  }

  fn picker(&self, activity: i64) -> &Picker {
    match activity {
      COMPOSER_ACTIVITY_ID => &self.composer,
      REACTION_ACTIVITY_ID => &self.reactions,
      _ => &self.manufacturing,
    }
  }

  fn picker_mut(&mut self, activity: i64) -> &mut Picker {
    match activity {
      COMPOSER_ACTIVITY_ID => &mut self.composer,
      REACTION_ACTIVITY_ID => &mut self.reactions,
      _ => &mut self.manufacturing,
    }
  }
}

#[derive(Clone, Copy, Debug)]
struct Activity {
  blurb_key: &'static str,
  id: i64,
  name_key: &'static str,
  placeholder_key: &'static str,
}

#[derive(Debug, Default)]
struct ExportDraft {
  selected: BTreeSet<i64>,
}

#[derive(Debug)]
struct FitDraft {
  content: text_editor::Content,
  facility_id: i64,
  facility_name: String,
  structure_name: String,
}

#[derive(Clone, Debug, PartialEq)]
struct IntelCard {
  eft: Option<String>,
  facility: FacilityRef,
  owner: Option<String>,
  rigs: [Option<i64>; RIG_SLOTS],
}

impl IntelCard {
  fn fitted(&self) -> usize {
    self.rigs.iter().flatten().count()
  }

  fn rig_ids(&self) -> Vec<i64> {
    self.rigs.iter().flatten().copied().collect()
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortBy {
  #[default]
  Name,
  System,
  Region,
  Security,
  Rigs,
}

impl SortBy {
  const ALL: [SortBy; 5] = [
    SortBy::Name,
    SortBy::System,
    SortBy::Region,
    SortBy::Security,
    SortBy::Rigs,
  ];

  fn label_key(self) -> &'static str {
    match self {
      SortBy::Name => "settings.facility.sort_name",
      SortBy::System => "settings.facility.sort_system",
      SortBy::Region => "settings.facility.sort_region",
      SortBy::Security => "settings.facility.sort_security",
      SortBy::Rigs => "settings.facility.sort_rigs",
    }
  }
}

fn name_key(card: &IntelCard) -> String {
  strip_system_prefix(&card.facility.name, &card.facility.solar_system).to_lowercase()
}

fn text_key(value: &str) -> (bool, String) {
  let trimmed = value.trim();
  (trimmed.is_empty(), trimmed.to_lowercase())
}

fn region_key(card: &IntelCard) -> (bool, String) {
  match &card.facility.region {
    Some(region) => text_key(region),
    None => (true, String::new()),
  }
}

fn security_key(card: &IntelCard) -> f64 {
  card.facility.security_status.unwrap_or(f64::NEG_INFINITY)
}

fn compare_cards(sort: SortBy, a: &IntelCard, b: &IntelCard) -> Ordering {
  let by_name = || name_key(a).cmp(&name_key(b));
  match sort {
    SortBy::Name => by_name(),
    SortBy::System => text_key(&a.facility.solar_system)
      .cmp(&text_key(&b.facility.solar_system))
      .then_with(by_name),
    SortBy::Region => region_key(a).cmp(&region_key(b)).then_with(by_name),
    SortBy::Security => security_key(b).total_cmp(&security_key(a)).then_with(by_name),
    SortBy::Rigs => b.fitted().cmp(&a.fitted()).then_with(by_name),
  }
}

fn sorted_intel(state: &State) -> Vec<&IntelCard> {
  let mut cards: Vec<&IntelCard> = state.intel.iter().collect();
  cards.sort_by(|a, b| compare_cards(state.sort, a, b));
  cards
}

#[derive(Debug, Default)]
struct OpenRig {
  facility_id: i64,
  search: RigSearch,
  slot: usize,
}

#[derive(Debug, Default)]
struct Picker {
  open: bool,
  search: FacilitySearch,
  selection: Option<FacilityRef>,
}

#[derive(Debug, Default)]
struct MarketPicker {
  generation: u64,
  open: bool,
  query: String,
  results: Vec<LocationRef>,
  searching: bool,
  selection: Option<LocationRef>,
}

fn db_activity(activity: i64) -> i64 {
  if activity == REACTION_ACTIVITY_ID {
    DB_REACTION_ACTIVITY_ID
  } else {
    DB_MANUFACTURING_ACTIVITY_ID
  }
}

fn facility_ref(facility: &PlannerFacility, is_reaction: bool) -> FacilityRef {
  facility.to_ref(is_reaction)
}

/// The name/solar_system_id/type_id to persist as an intel row's snapshot, read back off the display facility. A
/// facility with no recovered identity renders as `#<id>` at solar system 0; that sentinel maps back to a NULL
/// snapshot so a genuinely unidentified row round-trips as NULL rather than storing the `#<id>` placeholder.
fn snapshot_of(facility: &FacilityRef) -> (Option<String>, Option<i64>, Option<i64>) {
  let name = (facility.name != format!("#{}", facility.id)).then(|| facility.name.clone());
  let solar_system_id = (facility.solar_system_id != 0).then_some(facility.solar_system_id);
  (name, solar_system_id, facility.type_id)
}

pub fn load(state: &State) -> iced::Task<Message> {
  let Some(db) = state.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(load_all(db, state.clients.clone()), |result| {
    Message::Loaded(Box::new(result))
  })
}

pub fn reset_to_defaults(state: &State) -> iced::Task<Message> {
  let Some(db) = state.db.clone() else {
    return iced::Task::none();
  };
  let clients = state.clients.clone();
  iced::Task::perform(
    async move {
      let _ = industry::clear_default_facility(&db, DB_MANUFACTURING_ACTIVITY_ID).await;
      let _ = industry::clear_default_facility(&db, DB_REACTION_ACTIVITY_ID).await;
      let _ = market::clear_default_market(&db).await;
      load_all(db, clients).await
    },
    |result| Message::Loaded(Box::new(result)),
  )
}

pub fn update(state: &mut State, message: Message, _settings: &mut Settings) -> (Outcome, iced::Task<Message>) {
  match message {
    Message::Cleared {
      activity,
    } => clear_default(state, activity),
    Message::ComposerToggled(open) => {
      state.composer.open = open;
      state.composer.search.clear();
      (Outcome::None, iced::Task::none())
    }
    // Grouped (rather than handled inline) to keep this match's complexity down; route any new
    // Export/Import variant through this arm into update_export/update_import instead of adding logic here.
    Message::ExportAllSelected
    | Message::ExportClosed
    | Message::ExportConfirmed
    | Message::ExportFacilityToggled(_)
    | Message::ExportNoneSelected
    | Message::ExportOpened
    | Message::ImportErrorDismissed
    | Message::ImportFileLoaded(_)
    | Message::ImportFinished(_)
    | Message::ImportOpened
    | Message::ImportResultClosed => update_export(state, message),
    Message::FacilityPicked {
      activity,
      facility,
    } => facility_picked(state, activity, facility),
    // Grouped (rather than handled inline) to keep this match's complexity down; route any new
    // Fit variant through this arm into update_fit instead of adding logic here.
    Message::FitApplied
    | Message::FitClosed
    | Message::FitInputChanged(_)
    | Message::FitOpened {
      ..
    } => update_fit(state, message),
    Message::Loaded(result) => {
      loaded(state, *result);
      (Outcome::None, iced::Task::none())
    }
    // Grouped (rather than handled inline) to keep this match's complexity down; route any new
    // Market variant through this arm into update_market instead of adding logic here.
    Message::MarketCleared
    | Message::MarketPicked(_)
    | Message::MarketPickerToggled
    | Message::MarketQueryChanged(_)
    | Message::MarketResults {
      ..
    } => update_market(state, message),
    Message::PickerToggled {
      activity,
    } => {
      let picker = state.picker_mut(activity);
      picker.open = !picker.open;
      if !picker.open {
        picker.search.clear();
      }
      (Outcome::None, iced::Task::none())
    }
    Message::QueryChanged {
      activity,
      query,
    } => {
      let picker = state.picker_mut(activity);
      picker.open = true;
      let generation = picker.search.set_query(query.clone());
      (
        Outcome::IndustrySearch {
          activity,
          generation,
          query,
        },
        iced::Task::none(),
      )
    }
    Message::RemoveFacility(facility_id) => remove_facility(state, facility_id),
    // Grouped (rather than handled inline) to keep this match's complexity down; route any new
    // Rig variant through this arm into update_rig instead of adding logic here.
    Message::RigCleared {
      ..
    }
    | Message::RigDismissed
    | Message::RigPicked {
      ..
    }
    | Message::RigQueryChanged {
      ..
    }
    | Message::RigSlotToggled {
      ..
    } => update_rig(state, message),
    Message::Saved(result) => (Outcome::None, saved(state, result)),
    Message::SearchResults {
      activity,
      generation,
      results,
    } => {
      let is_reaction = activity == REACTION_ACTIVITY_ID;
      let refs = results.iter().map(|f| facility_ref(f, is_reaction)).collect();
      state.picker_mut(activity).search.accept_results(generation, refs);
      (Outcome::None, iced::Task::none())
    }
    Message::SortChanged(sort) => {
      state.sort = sort;
      state.sort_open = false;
      (Outcome::None, iced::Task::none())
    }
    Message::SortMenuDismissed => {
      state.sort_open = false;
      (Outcome::None, iced::Task::none())
    }
    Message::SortMenuToggled => {
      state.sort_open = !state.sort_open;
      (Outcome::None, iced::Task::none())
    }
  }
}

fn update_rig(state: &mut State, message: Message) -> (Outcome, iced::Task<Message>) {
  match message {
    Message::RigCleared {
      facility_id,
      slot,
    } => set_rig(state, facility_id, slot, None),
    Message::RigDismissed => {
      state.open_rig = None;
      (Outcome::None, iced::Task::none())
    }
    Message::RigPicked {
      facility_id,
      rig,
      slot,
    } => set_rig(state, facility_id, slot, Some(rig.type_id)),
    Message::RigQueryChanged {
      facility_id,
      query,
      slot,
    } => {
      if state
        .open_rig
        .as_ref()
        .is_some_and(|open| open.facility_id == facility_id && open.slot == slot)
      {
        let results = rig_options(state, facility_id, &query);
        if let Some(open) = state.open_rig.as_mut() {
          let generation = open.search.set_query(query);
          open.search.accept_results(generation, results);
        }
      }
      (Outcome::None, iced::Task::none())
    }
    Message::RigSlotToggled {
      facility_id,
      slot,
    } => {
      let already = state
        .open_rig
        .as_ref()
        .is_some_and(|open| open.facility_id == facility_id && open.slot == slot);
      if already {
        state.open_rig = None;
      } else {
        let results = rig_options(state, facility_id, "");
        let mut search = RigSearch::default();
        let generation = search.set_query(String::new());
        search.accept_results(generation, results);
        state.open_rig = Some(OpenRig {
          facility_id,
          search,
          slot,
        });
      }
      (Outcome::None, iced::Task::none())
    }
    _ => (Outcome::None, iced::Task::none()),
  }
}

fn update_export(state: &mut State, message: Message) -> (Outcome, iced::Task<Message>) {
  match message {
    Message::ExportAllSelected => {
      if let Some(draft) = state.export.as_mut() {
        draft.selected = state.intel.iter().map(|card| card.facility.id).collect();
      }
    }
    Message::ExportClosed => state.export = None,
    Message::ExportConfirmed => return export_confirmed(state),
    Message::ExportFacilityToggled(facility_id) => {
      if let Some(draft) = state.export.as_mut()
        && !draft.selected.remove(&facility_id)
      {
        draft.selected.insert(facility_id);
      }
    }
    Message::ExportNoneSelected => {
      if let Some(draft) = state.export.as_mut() {
        draft.selected.clear();
      }
    }
    Message::ExportOpened if !state.intel.is_empty() => {
      state.export = Some(ExportDraft {
        selected: state.intel.iter().map(|card| card.facility.id).collect(),
      });
    }
    other => return update_import(state, other),
  }
  (Outcome::None, iced::Task::none())
}

fn update_import(state: &mut State, message: Message) -> (Outcome, iced::Task<Message>) {
  match message {
    Message::ImportErrorDismissed => state.import_error = None,
    Message::ImportFileLoaded(Some(content)) => return import_file_loaded(state, &content),
    Message::ImportFinished(summary) => {
      state.import_result = Some(summary);
      return (Outcome::None, reload(state));
    }
    Message::ImportOpened => {
      state.import_error = None;
      return (
        Outcome::None,
        iced::Task::perform(pick_intel_pack(), Message::ImportFileLoaded),
      );
    }
    Message::ImportResultClosed => state.import_result = None,
    _ => {}
  }
  (Outcome::None, iced::Task::none())
}

fn update_fit(state: &mut State, message: Message) -> (Outcome, iced::Task<Message>) {
  match message {
    Message::FitApplied => return fit_applied(state),
    Message::FitClosed => state.fit = None,
    Message::FitInputChanged(action) => {
      if let Some(draft) = state.fit.as_mut() {
        draft.content.perform(action);
      }
    }
    Message::FitOpened {
      facility_id,
    } => open_fit(state, facility_id),
    _ => {}
  }
  (Outcome::None, iced::Task::none())
}

fn open_fit(state: &mut State, facility_id: i64) {
  let Some(card) = state.intel.iter().find(|card| card.facility.id == facility_id) else {
    return;
  };
  state.fit = Some(FitDraft {
    content: text_editor::Content::new(),
    facility_id,
    facility_name: strip_system_prefix(&card.facility.name, &card.facility.solar_system).to_owned(),
    structure_name: card.facility.type_label.clone().unwrap_or_default(),
  });
}

fn fit_applied(state: &mut State) -> (Outcome, iced::Task<Message>) {
  let Some(draft) = state.fit.take() else {
    return (Outcome::None, iced::Task::none());
  };
  let catalog = rig_catalog_pairs(state);
  let parsed = facility_intel_fit::parse_fit(
    &draft.content.text(),
    &draft.structure_name,
    &draft.facility_name,
    catalog.iter().map(|(name, id)| (name.as_str(), *id)),
  );
  if parsed.eft.trim().is_empty() {
    return (Outcome::None, iced::Task::none());
  }
  let facility_id = draft.facility_id;
  let Some(card) = state.intel.iter_mut().find(|card| card.facility.id == facility_id) else {
    return (Outcome::None, iced::Task::none());
  };
  let mut rigs = [None; RIG_SLOTS];
  for (slot, id) in parsed.rigs.iter().take(RIG_SLOTS).enumerate() {
    rigs[slot] = Some(*id);
  }
  card.rigs = rigs;
  card.eft = Some(parsed.eft.clone());
  let (name, solar_system_id, type_id) = snapshot_of(&card.facility);
  let eft = parsed.eft;
  let task = write(&state.db, move |db| async move {
    industry::upsert_facility_intel(
      &db,
      facility_id,
      Some(eft),
      name,
      rigs[0],
      rigs[1],
      rigs[2],
      solar_system_id,
      type_id,
    )
    .await
  });
  (Outcome::None, task)
}

fn rig_catalog_pairs(state: &State) -> Vec<(String, i64)> {
  state.rigs.iter().map(|rig| (rig.name.clone(), rig.type_id)).collect()
}

fn rig_name(state: &State, type_id: i64) -> Option<String> {
  state
    .rigs
    .iter()
    .find(|rig| rig.type_id == type_id)
    .map(|rig| rig.name.clone())
}

fn import_file_loaded(state: &mut State, content: &str) -> (Outcome, iced::Task<Message>) {
  match facility_intel_share::parse_pack(content) {
    Ok(pack) => (
      Outcome::ImportIntel {
        facilities: pack.facilities,
      },
      iced::Task::none(),
    ),
    Err(error) => {
      state.import_error = Some(error);
      (Outcome::None, iced::Task::none())
    }
  }
}

async fn pick_intel_pack() -> Option<String> {
  #[cfg(not(test))]
  {
    let filter = t!("settings.facility.export_file_filter");
    let handle = rfd::AsyncFileDialog::new()
      .set_title(t!("settings.facility.import_dialog_title").into_owned())
      .add_filter(&*filter, &[facility_intel_share::PACK_EXTENSION])
      .pick_file()
      .await?;
    Some(String::from_utf8_lossy(&handle.read().await).into_owned())
  }
  #[cfg(test)]
  {
    None
  }
}

fn clear_default(state: &mut State, activity: i64) -> (Outcome, iced::Task<Message>) {
  let picker = state.picker_mut(activity);
  picker.open = false;
  picker.selection = None;
  picker.search.clear();
  let db_activity = db_activity(activity);
  let task = write(&state.db, move |db| async move {
    industry::clear_default_facility(&db, db_activity).await
  });
  (Outcome::Persist, task)
}

fn facility_picked(state: &mut State, activity: i64, facility: FacilityRef) -> (Outcome, iced::Task<Message>) {
  if activity == COMPOSER_ACTIVITY_ID {
    state.composer.open = false;
    state.composer.search.clear();
    if state.intel.iter().all(|card| card.facility.id != facility.id) {
      state.intel.push(IntelCard {
        eft: None,
        facility: facility.clone(),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
    }
    let facility_id = facility.id;
    let (name, solar_system_id, type_id) = snapshot_of(&facility);
    let task = write(&state.db, move |db| async move {
      industry::upsert_facility_intel(&db, facility_id, None, name, None, None, None, solar_system_id, type_id).await
    });
    return (Outcome::None, task);
  }

  let picker = state.picker_mut(activity);
  picker.open = false;
  picker.selection = Some(facility.clone());
  picker.search.clear();
  let db_activity = db_activity(activity);
  let facility_id = facility.id;
  let task = write(&state.db, move |db| async move {
    industry::set_default_facility(&db, db_activity, facility_id).await
  });
  (Outcome::Persist, task)
}

fn update_market(state: &mut State, message: Message) -> (Outcome, iced::Task<Message>) {
  match message {
    Message::MarketCleared => {
      clear_market_search(state);
      state.market.open = false;
      state.market.selection = None;
      let task = write(&state.db, |db| async move { market::clear_default_market(&db).await });
      (Outcome::Persist, task)
    }
    Message::MarketPicked(place) => {
      clear_market_search(state);
      state.market.open = false;
      let place_id = place.id;
      state.market.selection = Some(place);
      let task = write(&state.db, move |db| async move {
        market::set_default_market(&db, place_id).await
      });
      (Outcome::Persist, task)
    }
    Message::MarketPickerToggled => {
      state.market.open = !state.market.open;
      if !state.market.open {
        clear_market_search(state);
      }
      (Outcome::None, iced::Task::none())
    }
    Message::MarketQueryChanged(query) => market_query_changed(state, query),
    Message::MarketResults {
      generation,
      results,
    } => {
      if generation == state.market.generation {
        state.market.results = results;
        state.market.searching = false;
      }
      (Outcome::None, iced::Task::none())
    }
    _ => (Outcome::None, iced::Task::none()),
  }
}

fn clear_market_search(state: &mut State) {
  state.market.query.clear();
  state.market.results.clear();
  state.market.searching = false;
}

fn market_query_changed(state: &mut State, query: String) -> (Outcome, iced::Task<Message>) {
  state.market.open = true;
  state.market.generation += 1;
  state.market.query = query.clone();
  if query.trim().chars().count() < MARKET_SEARCH_MIN_CHARS {
    state.market.results.clear();
    state.market.searching = false;
    return (Outcome::None, iced::Task::none());
  }
  let (Some(db), Some(clients)) = (state.db.clone(), state.clients.clone()) else {
    return (Outcome::None, iced::Task::none());
  };
  state.market.searching = true;
  let generation = state.market.generation;
  let task = iced::Task::perform(
    async move {
      crate::services::location_search::search_locations_enriched(
        db,
        clients.esi,
        clients.sso,
        query,
        MARKET_SEARCH_MIN_CHARS,
      )
      .await
    },
    move |results| Message::MarketResults {
      generation,
      results,
    },
  );
  (Outcome::None, task)
}

fn export_confirmed(state: &mut State) -> (Outcome, iced::Task<Message>) {
  let Some(draft) = state.export.take() else {
    return (Outcome::None, iced::Task::none());
  };
  let facilities: Vec<facility_intel_share::PortableFacility> = state
    .intel
    .iter()
    .filter(|card| draft.selected.contains(&card.facility.id))
    .map(portable)
    .collect();
  if facilities.is_empty() {
    return (Outcome::None, iced::Task::none());
  }
  (
    Outcome::ExportIntel {
      facilities,
    },
    iced::Task::none(),
  )
}

fn portable(card: &IntelCard) -> facility_intel_share::PortableFacility {
  let (name, solar_system_id, type_id) = snapshot_of(&card.facility);
  let intel = FacilityIntel {
    eft: card.eft.clone(),
    facility_id: card.facility.id,
    name,
    rig_1_type_id: card.rigs[0],
    rig_2_type_id: card.rigs[1],
    rig_3_type_id: card.rigs[2],
    solar_system_id,
    type_id,
  };
  facility_intel_share::portable_facility(&intel)
}

fn remove_facility(state: &mut State, facility_id: i64) -> (Outcome, iced::Task<Message>) {
  state.intel.retain(|card| card.facility.id != facility_id);
  if state
    .open_rig
    .as_ref()
    .is_some_and(|open| open.facility_id == facility_id)
  {
    state.open_rig = None;
  }
  let task = write(&state.db, move |db| async move {
    industry::delete_facility_intel(&db, facility_id).await
  });
  (Outcome::None, task)
}

fn set_rig(state: &mut State, facility_id: i64, slot: usize, rig: Option<i64>) -> (Outcome, iced::Task<Message>) {
  state.open_rig = None;
  let Some(index) = state.intel.iter().position(|card| card.facility.id == facility_id) else {
    return (Outcome::None, iced::Task::none());
  };
  if slot < RIG_SLOTS {
    state.intel[index].rigs[slot] = rig;
  }
  let rigs = state.intel[index].rigs;
  let existing_eft = state.intel[index].eft.clone();
  let structure_name = state.intel[index].facility.type_label.clone().unwrap_or_default();
  let facility_name = strip_system_prefix(
    &state.intel[index].facility.name,
    &state.intel[index].facility.solar_system,
  )
  .to_owned();
  let rig_names: Vec<String> = rigs.iter().flatten().filter_map(|id| rig_name(state, *id)).collect();
  let eft = facility_intel_fit::splice_rigs(existing_eft.as_deref(), &rig_names, &structure_name, &facility_name);
  state.intel[index].eft = Some(eft.clone());
  let (name, solar_system_id, type_id) = snapshot_of(&state.intel[index].facility);
  let task = write(&state.db, move |db| async move {
    industry::upsert_facility_intel(
      &db,
      facility_id,
      Some(eft),
      name,
      rigs[0],
      rigs[1],
      rigs[2],
      solar_system_id,
      type_id,
    )
    .await
  });
  (Outcome::None, task)
}

fn loaded(state: &mut State, result: Result<Loaded, String>) {
  match result {
    Ok(payload) => {
      state.facilities_count = payload.facilities_count;
      state.intel = merge_intel(std::mem::take(&mut state.intel), payload.intel);
      state.manufacturing.selection = payload.manufacturing;
      state.market.selection = payload.market;
      state.reactions.selection = payload.reactions;
      state.rig_catalog = payload.rig_catalog;
      state.rigs = payload.rigs;
      state.load_error = None;
    }
    Err(error) => state.load_error = Some(error),
  }
}

fn merge_intel(previous: Vec<IntelCard>, loaded: Vec<IntelCard>) -> Vec<IntelCard> {
  let loaded_ids: Vec<i64> = loaded.iter().map(|card| card.facility.id).collect();
  let mut merged = loaded;
  for card in previous {
    if !loaded_ids.contains(&card.facility.id) {
      merged.push(card);
    }
  }
  merged
}

fn reload(state: &State) -> iced::Task<Message> {
  load(state)
}

fn saved(state: &State, result: Result<(), String>) -> iced::Task<Message> {
  match result {
    Ok(()) => reload(state),
    Err(_error) => iced::Task::none(),
  }
}

fn write<F, Fut>(db: &Option<Database>, op: F) -> iced::Task<Message>
where
  F: FnOnce(Database) -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Result<(), crate::store::Error>> + Send + 'static,
{
  let Some(db) = db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move { op(db).await.map_err(|err| err.to_string()) },
    Message::Saved,
  )
}

async fn load_all(db: Database, clients: Option<crate::features::industry::Clients>) -> Result<Loaded, String> {
  let facilities = industry::accessible_facilities(&db)
    .await
    .map_err(|err| err.to_string())?;

  let manufacturing = resolve_default(&db, &facilities, DB_MANUFACTURING_ACTIVITY_ID, clients.as_ref()).await;
  let reactions = resolve_default(&db, &facilities, DB_REACTION_ACTIVITY_ID, clients.as_ref()).await;

  let mut intel = Vec::new();
  for row in industry::list_facility_intel(&db)
    .await
    .map_err(|err| err.to_string())?
  {
    let facility = intel_facility_ref(&db, &facilities, &row).await;
    let owner = resolve_facility_owner(&db, row.facility_id)
      .await
      .map(|owner| owner.display());
    intel.push(IntelCard {
      eft: row.eft.clone(),
      facility,
      owner,
      rigs: [row.rig_1_type_id, row.rig_2_type_id, row.rig_3_type_id],
    });
  }

  let (rigs, rig_catalog) = load_rigs(&db).await?;

  let market = resolve_default_market(&db, clients.as_ref()).await;

  Ok(Loaded {
    facilities_count: facilities.len(),
    intel,
    manufacturing,
    market,
    reactions,
    rig_catalog,
    rigs,
  })
}

fn place_ref(id: i64, name: String, tier: LocationTier) -> LocationRef {
  LocationRef {
    context: None,
    id,
    name,
    security_status: None,
    tier: Some(tier),
  }
}

async fn resolve_default_market(
  db: &Database,
  clients: Option<&crate::features::industry::Clients>,
) -> Option<LocationRef> {
  let id = market::default_market(db).await.ok().flatten()?;
  resolve_place(db, clients, id).await
}

async fn resolve_place(
  db: &Database,
  clients: Option<&crate::features::industry::Clients>,
  id: i64,
) -> Option<LocationRef> {
  match LocationTier::from_id(id)? {
    LocationTier::Constellation => sde::get_constellation(db, id)
      .await
      .ok()
      .flatten()
      .map(|constellation| {
        place_ref(
          constellation.id(),
          constellation.name().clone(),
          LocationTier::Constellation,
        )
      }),
    LocationTier::Region => sde::get_region(db, id)
      .await
      .ok()
      .flatten()
      .map(|region| place_ref(region.id(), region.name().clone(), LocationTier::Region)),
    LocationTier::Station => sde::get_station(db, id)
      .await
      .ok()
      .flatten()
      .map(|station| place_ref(station.id(), station.name().clone(), LocationTier::Station)),
    LocationTier::Structure => resolve_structure_place(db, clients, id).await,
    LocationTier::System => sde::get_solar_system(db, id)
      .await
      .ok()
      .flatten()
      .map(|system| place_ref(system.id(), system.name().clone(), LocationTier::System)),
  }
}

async fn resolve_structure_place(
  db: &Database,
  clients: Option<&crate::features::industry::Clients>,
  id: i64,
) -> Option<LocationRef> {
  if let Ok(Some(structure)) = sde::get_structure(db, id).await {
    return Some(place_ref(
      structure.id(),
      structure.name().clone(),
      LocationTier::Structure,
    ));
  }
  let clients = clients?;
  let facility = crate::features::industry::resolve_structure(db, &clients.esi, &clients.sso, id).await?;
  Some(place_ref(facility.id, facility.name, LocationTier::Structure))
}

/// Resolves the saved default facility for an activity into its display shape. A facility still in the
/// accessible list resolves locally; a structure default that is no longer corp-synced (pins are gone, so
/// nothing in the DB can rebuild it) gets the same one-shot ESI resolve the planner uses, so the dropdown
/// keeps showing the user's pick. Unresolvable (no clients, no token, 403/404) degrades to unset.
async fn resolve_default(
  db: &Database,
  facilities: &[crate::store::model::Facility],
  db_activity: i64,
  clients: Option<&crate::features::industry::Clients>,
) -> Option<FacilityRef> {
  let id = industry::default_facility(db, db_activity).await.ok().flatten()?;
  if let Some(facility) = facility_ref_for(db, facilities, id).await {
    return Some(facility);
  }
  if id < MIN_STRUCTURE_ID {
    return None;
  }
  let clients = clients?;
  let facility = crate::features::industry::resolve_structure(db, &clients.esi, &clients.sso, id).await?;
  Some(facility_ref(&facility, db_activity == DB_REACTION_ACTIVITY_ID))
}

/// Builds the display facility for an intel card, always. An accessible facility yields the rich picker
/// facility; anything else (tombstoned, ACL-lost, imported-but-never-dockable) falls back to the row's own
/// snapshot enriched with local SDE geography, so intel is never dropped for being inaccessible.
async fn intel_facility_ref(
  db: &Database,
  facilities: &[crate::store::model::Facility],
  row: &FacilityIntel,
) -> FacilityRef {
  if let Some(facility) = facility_ref_for(db, facilities, row.facility_id).await {
    return facility;
  }
  let (security_status, region, solar_system) = match row.solar_system_id {
    Some(system_id) => industry::system_geo(db, system_id).await.unwrap_or((None, None, None)),
    None => (None, None, None),
  };
  FacilityRef {
    cost_index: None,
    id: row.facility_id,
    name: row.name.clone().unwrap_or_else(|| format!("#{}", row.facility_id)),
    region,
    security_status,
    solar_system: solar_system.unwrap_or_default(),
    solar_system_id: row.solar_system_id.unwrap_or(0),
    type_id: row.type_id,
    type_label: facility_type_label(db, row.facility_id, row.type_id).await,
  }
}

async fn facility_ref_for(db: &Database, facilities: &[crate::store::model::Facility], id: i64) -> Option<FacilityRef> {
  let facility = facilities.iter().find(|facility| facility.id() == id)?;
  Some(FacilityRef {
    cost_index: facility.manufacturing_index(),
    id: facility.id(),
    name: facility.name().clone(),
    region: facility.region().clone(),
    security_status: facility.security_status(),
    solar_system: facility.solar_system().clone().unwrap_or_default(),
    solar_system_id: facility.solar_system_id(),
    type_id: facility.type_id(),
    type_label: facility_type_label(db, id, facility.type_id()).await,
  })
}

async fn facility_type_label(db: &Database, id: i64, type_id: Option<i64>) -> Option<String> {
  if id < MIN_STRUCTURE_ID {
    return Some(super::i18n::tr_static("settings.facility.station").to_owned());
  }
  let type_id = type_id?;
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
}

async fn load_rigs(db: &Database) -> Result<(Vec<RigRef>, HashMap<i64, RigBonus>), String> {
  let rows = sde::structure_rig_bonuses(db).await.map_err(|err| err.to_string())?;

  let mut names: HashMap<i64, String> = HashMap::new();
  for row in &rows {
    names.entry(row.type_id).or_insert_with(|| row.name.clone());
  }

  let catalog = rig_bonuses::build_catalog(rows.into_iter().map(|row| (row.type_id, row.attribute_id, row.value)));

  let mut rigs: Vec<RigRef> = catalog
    .iter()
    .map(|(type_id, bonus)| {
      let name = names.get(type_id).cloned().unwrap_or_default();
      RigRef {
        activity: rig_activity(&name),
        fee: bonus.fee,
        me: bonus.me,
        name,
        te: bonus.te,
        type_id: *type_id,
      }
    })
    .collect();
  rigs.sort_by(|a, b| a.name.cmp(&b.name));

  Ok((rigs, catalog))
}

fn rig_activity(name: &str) -> RigActivity {
  match rig_bonuses::Activity::classify(name) {
    rig_bonuses::Activity::Manufacturing => RigActivity::Manufacturing,
    rig_bonuses::Activity::Reaction => RigActivity::Reaction,
    rig_bonuses::Activity::Science => RigActivity::Science,
  }
}

pub fn badge(state: &State) -> String {
  let set = usize::from(state.manufacturing.selection.is_some()) + usize::from(state.reactions.selection.is_some());
  format!("{set}/{}", ACTIVITIES.len())
}

pub fn view<'a>(state: &'a State, _settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header();
  let body = panel_body(state);

  let base = Column::with_children(vec![header, body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

  let mut layers: Vec<Element<'a, Message>> = Vec::new();
  if let Some(draft) = state.fit.as_ref() {
    layers.extend(modal_layers(Message::FitClosed, fit_modal(state, draft)));
  }
  if let Some(draft) = state.export.as_ref() {
    layers.extend(modal_layers(Message::ExportClosed, export_modal(state, draft)));
  }
  if let Some(summary) = state.import_result.as_ref() {
    layers.extend(modal_layers(Message::ImportResultClosed, import_result_modal(summary)));
  }
  if let Some(error) = state.import_error.as_ref() {
    layers.extend(modal_layers(Message::ImportErrorDismissed, import_error_modal(error)));
  }
  stable_overlay(base, layers)
}

pub fn escape_dismiss(state: &State) -> Option<Message> {
  if state.fit.is_some() {
    return Some(Message::FitClosed);
  }

  if state.import_error.is_some() {
    return Some(Message::ImportErrorDismissed);
  }

  if state.import_result.is_some() {
    return Some(Message::ImportResultClosed);
  }

  if state.export.is_some() {
    return Some(Message::ExportClosed);
  }

  None
}

fn panel_header<'a>() -> Element<'a, Message> {
  let title = text(t!("settings.facility.title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(t!("settings.facility.blurb"))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let band = container(identity).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn panel_body(state: &State) -> Element<'_, Message> {
  let mut sections: Vec<Element<'_, Message>> = Vec::new();
  for activity in ACTIVITIES {
    sections.push(activity_section(state, activity));
  }
  sections.push(market_section(state));
  sections.push(intel_section(state));

  let inner = container(
    Column::with_children(sections)
      .spacing(spacing::SPACE_6)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn section_head<'a>(label: &'a str, detail: &'a str, right: Option<Element<'a, Message>>) -> Element<'a, Message> {
  let micro = text(label)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent()));
  let detail = text(detail)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let copy = Column::with_children(vec![micro.into(), detail.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let mut row = Row::new()
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .push(copy);
  if let Some(right) = right {
    row = row.push(right);
  }
  row.width(Length::Fill).into()
}

fn count_label<'a>(label: String) -> Element<'a, Message> {
  text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn activity_section<'a>(state: &'a State, activity: Activity) -> Element<'a, Message> {
  let picker = state.picker(activity.id);
  let head = section_head(
    super::i18n::tr_static(activity.name_key),
    super::i18n::tr_static(activity.blurb_key),
    Some(count_label(
      t!("settings.facility.structures_count", count => state.facilities_count).into_owned(),
    )),
  );

  let id = activity.id;
  let trigger = FacilityCombobox::new()
    .placeholder(super::i18n::tr_static(activity.placeholder_key))
    .selection(picker.selection.clone())
    .on_toggle(Message::PickerToggled {
      activity: id,
    })
    .trigger();

  let dropdown =
    AnchoredDropdown::new(trigger, picker.open.then(|| popover(picker, id))).on_dismiss(Message::PickerToggled {
      activity: id,
    });

  let capped = container(dropdown).max_width(PICKER_MAX_WIDTH).width(Length::Fill);

  Column::with_children(vec![head, capped.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn popover(picker: &Picker, activity: i64) -> Element<'_, Message> {
  let combobox = FacilityCombobox::new()
    .query(picker.search.query())
    .results(picker.search.results().to_vec())
    .on_input(move |query| Message::QueryChanged {
      activity,
      query,
    })
    .on_pick(move |facility: FacilityRef| Message::FacilityPicked {
      activity,
      facility,
    })
    .highlight(picker.search.highlight())
    .searching(picker.search.searching())
    .selection(picker.selection.clone())
    .on_clear(Message::Cleared {
      activity,
    })
    .popover();

  container(combobox)
    .width(Length::Fill)
    .style(|_| container::Style {
      shadow: crate::ui::style::shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

fn market_section(state: &State) -> Element<'_, Message> {
  let head = section_head(
    super::i18n::tr_static("settings.facility.market_name"),
    super::i18n::tr_static("settings.facility.market_blurb"),
    None,
  );

  let trigger = LocationCombobox::new()
    .placeholder(super::i18n::tr_static("settings.facility.market_placeholder"))
    .selection(state.market.selection.clone())
    .on_toggle(Message::MarketPickerToggled)
    .trigger();

  let dropdown = AnchoredDropdown::new(trigger, state.market.open.then(|| market_popover(state)))
    .on_dismiss(Message::MarketPickerToggled);

  let capped = container(dropdown).max_width(PICKER_MAX_WIDTH).width(Length::Fill);

  Column::with_children(vec![head, capped.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn market_popover(state: &State) -> Element<'_, Message> {
  let combobox = LocationCombobox::new()
    .placeholder(super::i18n::tr_static("settings.facility.market_search_placeholder"))
    .query(&state.market.query)
    .results(state.market.results.clone())
    .searching(state.market.searching)
    .on_input(Message::MarketQueryChanged)
    .on_pick(Message::MarketPicked)
    .selection(state.market.selection.clone())
    .on_clear(Message::MarketCleared)
    .popover();

  container(combobox)
    .width(Length::Fill)
    .style(|_| container::Style {
      shadow: crate::ui::style::shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

fn intel_section(state: &State) -> Element<'_, Message> {
  let count = if state.intel.len() == 1 {
    t!("settings.facility.intel_count_one", count => state.intel.len())
  } else {
    t!("settings.facility.intel_count_other", count => state.intel.len())
  };
  let mut right = Row::new().spacing(spacing::SPACE_3).align_y(Vertical::Center);
  right = right.push(count_label(count.into_owned()));
  right = right.push(
    Button::ghost(t!("settings.facility.import_intel"))
      .icon(Icon::download())
      .size(Size::Sm)
      .on_press(Message::ImportOpened),
  );
  right = right.push(
    Button::ghost(t!("settings.facility.export_intel"))
      .icon(Icon::upload())
      .size(Size::Sm)
      .on_press_maybe((!state.intel.is_empty()).then_some(Message::ExportOpened)),
  );
  if !state.composer.open {
    right = right.push(
      Button::secondary(t!("settings.facility.add_facility"))
        .icon(Icon::plus())
        .size(Size::Sm)
        .on_press(Message::ComposerToggled(true)),
    );
  }

  let head = section_head(
    super::i18n::tr_static("settings.facility.intel_title"),
    super::i18n::tr_static("settings.facility.intel_note"),
    Some(right.into()),
  );

  let mut children: Vec<Element<'_, Message>> = vec![head, warning_line()];
  if state.composer.open {
    children.push(composer(state));
  }
  if state.intel.len() > 1 {
    children.push(sort_control(state));
  }
  if state.intel.is_empty() && !state.composer.open {
    children.push(empty_state());
  } else {
    children.push(intel_grid(state));
  }

  let body = Column::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  Column::with_children(vec![rule::horizontal(), body.into()])
    .spacing(spacing::SPACE_4_5)
    .width(Length::Fill)
    .into()
}

fn warning_line<'a>() -> Element<'a, Message> {
  Row::with_children(vec![
    status::dot_sized(color::status::WARNING, 4.0),
    text(t!("settings.facility.intel_warning"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn sort_control(state: &State) -> Element<'_, Message> {
  let label = text(t!("settings.facility.sort_label"))
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));

  let trigger: Element<'_, Message> = Button::secondary(super::i18n::tr_static(state.sort.label_key()))
    .icon_right(Icon::chevron_down())
    .size(Size::Sm)
    .on_press(Message::SortMenuToggled)
    .into();

  let dropdown = AnchoredDropdown::new(trigger, state.sort_open.then(sort_menu))
    .on_dismiss(Message::SortMenuDismissed)
    .popover_width(SORT_MENU_WIDTH);

  Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    label.into(),
    dropdown.into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill)
  .into()
}

fn sort_menu<'a>() -> Element<'a, Message> {
  let items: Vec<Element<'a, Message>> = SortBy::ALL.into_iter().map(sort_menu_item).collect();

  container(Column::with_children(items).width(Length::Fill))
    .width(Length::Fill)
    .padding(spacing::UNIT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::NAV_CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn sort_menu_item<'a>(sort: SortBy) -> Element<'a, Message> {
  button(
    text(super::i18n::tr_static(sort.label_key()).to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
  })
  .on_press(Message::SortChanged(sort))
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      ..button::Style::default()
    }
  })
  .into()
}

fn intel_grid(state: &State) -> Element<'_, Message> {
  let cards = sorted_intel(state);
  let mut rows: Vec<Element<'_, Message>> = Vec::new();
  for chunk in cards.chunks(GRID_COLUMNS) {
    let mut row = Row::new().spacing(spacing::SPACE_3).width(Length::Fill);
    for &card in chunk {
      row = row.push(intel_card(state, card));
    }
    for _ in chunk.len()..GRID_COLUMNS {
      row = row.push(Space::new().width(Length::Fill));
    }
    rows.push(row.into());
  }

  Column::with_children(rows)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn composer(state: &State) -> Element<'_, Message> {
  let combobox = FacilityCombobox::new()
    .placeholder(super::i18n::tr_static("settings.facility.add_search_placeholder"))
    .query(state.composer.search.query())
    .results(state.composer.search.results().to_vec())
    .on_input(|query| Message::QueryChanged {
      activity: COMPOSER_ACTIVITY_ID,
      query,
    })
    .on_pick(|facility: FacilityRef| Message::FacilityPicked {
      activity: COMPOSER_ACTIVITY_ID,
      facility,
    })
    .highlight(state.composer.search.highlight())
    .searching(state.composer.search.searching())
    .popover();

  let micro = text(t!("settings.facility.add_panel_title"))
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent()));
  let label = text(t!("settings.facility.add_panel_label"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let capped = container(combobox).max_width(PICKER_MAX_WIDTH).width(Length::Fill);

  let footer = Row::with_children(vec![
    text(t!("settings.facility.add_hint"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .width(Length::Fill)
      .into(),
    Button::ghost(t!("settings.facility.cancel"))
      .size(Size::Sm)
      .on_press(Message::ComposerToggled(false))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let inner = Column::with_children(vec![micro.into(), label.into(), capped.into(), footer.into()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill);

  container(inner)
    .width(Length::Fill)
    .padding(spacing::SPACE_4_5)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::RAISED)),
      border: iced::Border {
        color: color::accent(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  let glyph = Icon::facilities().size(28.0).color(color::text::tertiary()).render();
  let title = text(t!("settings.facility.intel_empty_title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let body = container(
    text(t!("settings.facility.intel_empty_body"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(380.0);
  let action = Button::primary(t!("settings.facility.add_first_facility"))
    .icon(Icon::plus())
    .size(Size::Sm)
    .on_press(Message::ComposerToggled(true));

  let inner = Column::with_children(vec![glyph, title.into(), body.into(), action.into()])
    .spacing(spacing::SPACE_3)
    .align_x(iced::alignment::Horizontal::Center)
    .width(Length::Fill);

  container(inner)
    .width(Length::Fill)
    .padding(36.0)
    .align_x(iced::alignment::Horizontal::Center)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::SUNKEN)),
      border: iced::Border {
        color: color::rule_strong(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn intel_card<'a>(state: &'a State, card: &'a IntelCard) -> Element<'a, Message> {
  let header = card_header(card);
  let rigs = rig_row(state, card);
  let derived = derived_row(state, card);

  let inner = Column::with_children(vec![header, rule::horizontal(), rigs, derived])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  container(inner)
    .width(Length::Fill)
    .padding(spacing::SPACE_4_5)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::RAISED)),
      border: iced::Border {
        color: color::rule(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn strip_system_prefix<'a>(name: &'a str, system: &str) -> &'a str {
  let system = system.trim();
  if !system.is_empty()
    && let Some(rest) = name.strip_prefix(system)
    && let Some(rest) = rest.trim_start().strip_prefix('-')
  {
    let stripped = rest.trim_start();
    if !stripped.is_empty() {
      return stripped;
    }
  }
  name
}

fn card_header<'a>(card: &'a IntelCard) -> Element<'a, Message> {
  let name = text(strip_system_prefix(&card.facility.name, &card.facility.solar_system).to_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY))
    .width(Length::Fill);

  let pills = container(
    Column::new()
      .spacing(spacing::UNIT)
      .align_x(Horizontal::Right)
      .push(facility_combobox::type_badge(&card.facility))
      .push(facility_combobox::sec_pill(card.facility.security_status)),
  )
  .width(Length::Fixed(PILL_COLUMN_WIDTH))
  .align_x(Horizontal::Right)
  .clip(true);

  let title_row = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Top)
    .push(name)
    .push(pills);

  let mut meta = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
  let mut has_meta = false;
  if let Some(region) = &card.facility.region {
    meta = meta.push(
      text(region.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
    has_meta = true;
  }
  if !card.facility.solar_system.trim().is_empty() {
    if has_meta {
      meta = meta.push(status::dot_sized(color::text::tertiary(), 2.0));
    }
    meta = meta.push(
      text(card.facility.solar_system.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary())),
    );
    has_meta = true;
  }
  if let Some(owner) = &card.owner {
    if has_meta {
      meta = meta.push(status::dot_sized(color::text::tertiary(), 2.0));
    }
    meta = meta.push(
      text(owner.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }

  let identity = Column::with_children(vec![title_row.into(), meta.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let populate = Button::ghost_icon(Icon::fitting())
    .size(Size::Sm)
    .on_press(Message::FitOpened {
      facility_id: card.facility.id,
    });
  let remove = Button::ghost_icon(Icon::trash())
    .size(Size::Sm)
    .on_press(Message::RemoveFacility(card.facility.id));

  let actions = Row::with_children(vec![populate.into(), remove.into()])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center);

  Row::with_children(vec![identity.into(), actions.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn rig_row<'a>(state: &'a State, card: &'a IntelCard) -> Element<'a, Message> {
  let head = Row::with_children(vec![
    text(t!("settings.facility.rig_slots"))
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .width(Length::Fill)
      .into(),
    text(t!("settings.facility.rig_fitted", fitted => card.fitted()))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .align_y(Vertical::Center);

  let mut slots = Row::new().spacing(spacing::SPACE_2).width(Length::Fill);
  for slot in 0..RIG_SLOTS {
    slots = slots.push(rig_slot(state, card, slot));
  }

  Column::with_children(vec![head.into(), slots.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn rig_slot<'a>(state: &'a State, card: &'a IntelCard, slot: usize) -> Element<'a, Message> {
  let facility_id = card.facility.id;
  let selection = card.rigs[slot].and_then(|type_id| state.rigs.iter().find(|rig| rig.type_id == type_id).cloned());

  let trigger = RigCombobox::new()
    .empty_label(super::i18n::tr_static("settings.facility.add_rig"))
    .me_label(super::i18n::tr_static("settings.facility.rig_me"))
    .te_label(super::i18n::tr_static("settings.facility.rig_te"))
    .fee_label(super::i18n::tr_static("settings.facility.rig_fee"))
    .selection(selection.clone())
    .on_toggle(Message::RigSlotToggled {
      facility_id,
      slot,
    })
    .trigger();

  let open = state
    .open_rig
    .as_ref()
    .filter(|open| open.facility_id == facility_id && open.slot == slot);

  let dropdown = AnchoredDropdown::new(trigger, open.map(|open| rig_popover(card, open, selection)))
    .on_dismiss(Message::RigDismissed)
    .popover_width(RIG_POPOVER_WIDTH);

  container(dropdown).width(Length::Fill).into()
}

fn rig_options(state: &State, facility_id: i64, query: &str) -> Vec<RigRef> {
  let Some(card) = state.intel.iter().find(|card| card.facility.id == facility_id) else {
    return Vec::new();
  };
  let needle = query.trim().to_lowercase();
  rigs_for_structure(state.rigs.iter().cloned(), card.facility.type_id.unwrap_or(0))
    .into_iter()
    .filter(|rig| needle.is_empty() || rig.name.to_lowercase().contains(&needle))
    .collect()
}

fn rig_popover<'a>(card: &'a IntelCard, open: &'a OpenRig, selection: Option<RigRef>) -> Element<'a, Message> {
  let facility_id = card.facility.id;
  let slot = open.slot;

  let combobox = RigCombobox::new()
    .placeholder(super::i18n::tr_static("settings.facility.rig_search_placeholder"))
    .empty_label(super::i18n::tr_static("settings.facility.rig_none"))
    .searching_label(super::i18n::tr_static("settings.facility.rig_searching"))
    .me_label(super::i18n::tr_static("settings.facility.rig_me"))
    .te_label(super::i18n::tr_static("settings.facility.rig_te"))
    .fee_label(super::i18n::tr_static("settings.facility.rig_fee"))
    .clear_label(super::i18n::tr_static("settings.facility.rig_clear"))
    .query(open.search.query())
    .results(open.search.results().to_vec())
    .highlight(open.search.highlight())
    .searching(open.search.searching())
    .selection(selection)
    .width(Length::Fill)
    .on_input(move |query| Message::RigQueryChanged {
      facility_id,
      query,
      slot,
    })
    .on_pick(move |rig: RigRef| Message::RigPicked {
      facility_id,
      rig: Box::new(rig),
      slot,
    })
    .on_clear(Message::RigCleared {
      facility_id,
      slot,
    })
    .popover();

  container(combobox)
    .width(Length::Fill)
    .style(|_| container::Style {
      shadow: crate::ui::style::shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

fn derived_row<'a>(state: &'a State, card: &'a IntelCard) -> Element<'a, Message> {
  let security = card.facility.security_status.unwrap_or(1.0);
  let derived: DerivedRigBonuses = rig_bonuses::derive_rig_bonuses(&card.rig_ids(), &state.rig_catalog, security);

  let head = Row::with_children(vec![
    text(t!("settings.facility.derived_bonuses"))
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text(t!("settings.facility.derived_bonuses_sub"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let chips = Row::with_children(vec![
    derived_chip(
      super::i18n::tr_static("settings.facility.chip_me"),
      derived.me,
      1,
      color::accent(),
      if derived.me != 0.0 {
        super::i18n::tr_static("settings.facility.chip_me_sub_on")
      } else {
        super::i18n::tr_static("settings.facility.chip_me_sub_off")
      },
    ),
    derived_chip(
      super::i18n::tr_static("settings.facility.chip_te"),
      derived.te,
      0,
      color::status::ONLINE,
      if derived.te != 0.0 {
        super::i18n::tr_static("settings.facility.chip_te_sub_on")
      } else {
        super::i18n::tr_static("settings.facility.chip_te_sub_off")
      },
    ),
  ])
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  Column::with_children(vec![head.into(), chips.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn derived_chip<'a>(
  label: &'a str,
  value: f64,
  decimals: usize,
  band: iced::Color,
  sub: &'a str,
) -> Element<'a, Message> {
  let value_text = if value == 0.0 {
    "\u{2014}".to_owned()
  } else {
    format!("{value:.decimals$}%")
  };
  let value_color = if value == 0.0 { color::text::secondary() } else { band };

  let inner = Column::with_children(vec![
    text(label.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(value_text)
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(value_color))
      .into(),
    text(sub.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  container(inner)
    .width(Length::Fill)
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::SUNKEN)),
      border: iced::Border {
        color: color::rule(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn export_modal<'a>(state: &'a State, draft: &'a ExportDraft) -> Element<'a, Message> {
  let content = Column::with_children(vec![
    export_header(),
    rule::horizontal(),
    export_body(state, draft),
    rule::horizontal(),
    export_footer(state, draft),
  ])
  .width(Length::Fill);

  let panel = container(content)
    .width(Length::Fill)
    .max_width(EXPORT_PANEL_MAX_WIDTH)
    .max_height(EXPORT_PANEL_MAX_HEIGHT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  panel.into()
}

fn export_header<'a>() -> Element<'a, Message> {
  let glyph = Icon::upload().size(18.0).color(color::accent()).render();
  let eyebrow = text(t!("settings.facility.export_eyebrow"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));
  let title = text(t!("settings.facility.export_title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let copy = Column::with_children(vec![eyebrow.into(), title.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);
  let close = Button::ghost_icon(Icon::close())
    .size(Size::Sm)
    .on_press(Message::ExportClosed);

  let row = Row::with_children(vec![glyph, copy.into(), close.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn export_body<'a>(state: &'a State, draft: &'a ExportDraft) -> Element<'a, Message> {
  let blurb = text(t!("settings.facility.export_blurb"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let all_on = draft.selected.len() == state.intel.len();
  let (toggle_label, toggle_message) = if all_on {
    (t!("settings.facility.export_clear_all"), Message::ExportNoneSelected)
  } else {
    (t!("settings.facility.export_select_all"), Message::ExportAllSelected)
  };
  let selector_head = Row::with_children(vec![
    text(t!("settings.facility.export_include"))
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .width(Length::Fill)
      .into(),
    export_toggle_link(toggle_label.into_owned(), toggle_message),
  ])
  .align_y(Vertical::Center);

  let rows: Vec<Element<'a, Message>> = state
    .intel
    .iter()
    .map(|card| export_row(card, draft.selected.contains(&card.facility.id)))
    .collect();
  let list = container(
    scrollable(Column::with_children(rows).spacing(6.0).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill),
  )
  .max_height(EXPORT_LIST_MAX_HEIGHT)
  .width(Length::Fill);

  Column::with_children(vec![blurb.into(), selector_head.into(), list.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .padding(Padding {
      top: 16.0,
      right: spacing::SPACE_4_5,
      bottom: 16.0,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn export_toggle_link(label: String, message: Message) -> Element<'static, Message> {
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::accent())),
  )
  .padding(Padding {
    top: 2.0,
    right: 4.0,
    bottom: 2.0,
    left: 4.0,
  })
  .on_press(message)
  .style(|_, _| button::Style {
    background: Some(Background::Color(iced::Color::TRANSPARENT)),
    ..button::Style::default()
  })
  .into()
}

fn export_row<'a>(card: &'a IntelCard, on: bool) -> Element<'a, Message> {
  let mut row = Row::new().spacing(spacing::SPACE_2_5).align_y(Vertical::Center);
  row = row.push(export_checkbox(on));
  row = row.push(
    text(card.facility.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY)),
  );
  row = row.push(facility_combobox::type_badge(&card.facility));
  let location = export_row_location(card);
  if !location.is_empty() {
    row = row.push(
      text(location)
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary())),
    );
  }
  row = row.push(Space::new().width(Length::Fill));
  row = row.push(
    text(t!("settings.facility.export_rigs", fitted => card.fitted()))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  );

  button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 9.0,
      right: 11.0,
      bottom: 9.0,
      left: 11.0,
    })
    .on_press(Message::ExportFacilityToggled(card.facility.id))
    .style(move |_, _| button::Style {
      background: Some(Background::Color(if on {
        color::with_alpha(color::accent(), 0.07)
      } else {
        color::surface::SUNKEN
      })),
      border: Border {
        color: if on {
          color::with_alpha(color::accent(), 0.4)
        } else {
          color::rule()
        },
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn export_row_location(card: &IntelCard) -> String {
  let mut parts: Vec<String> = Vec::new();
  if !card.facility.solar_system.trim().is_empty() {
    parts.push(card.facility.solar_system.clone());
  }
  if let Some(region) = &card.facility.region {
    parts.push(region.clone());
  }
  parts.join(" \u{b7} ")
}

fn export_checkbox<'a>(on: bool) -> Element<'a, Message> {
  let inner: Element<'a, Message> = if on {
    Icon::check().size(12.0).color(color::surface::BASE).render()
  } else {
    Space::new().into()
  };
  container(inner)
    .width(Length::Fixed(17.0))
    .height(Length::Fixed(17.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(if on {
        color::accent()
      } else {
        iced::Color::TRANSPARENT
      })),
      border: Border {
        color: if on { color::accent() } else { color::rule_strong() },
        width: 1.5,
        radius: 5.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn export_footer<'a>(state: &'a State, draft: &'a ExportDraft) -> Element<'a, Message> {
  let rigs: usize = state
    .intel
    .iter()
    .filter(|card| draft.selected.contains(&card.facility.id))
    .map(IntelCard::fitted)
    .sum();
  let summary = t!(
    "settings.facility.export_selected",
    selected => draft.selected.len(),
    total => state.intel.len(),
    rigs => rigs
  );

  let row = Row::with_children(vec![
    text(summary.into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .width(Length::Fill)
      .into(),
    Button::ghost(t!("settings.facility.cancel"))
      .size(Size::Sm)
      .on_press(Message::ExportClosed)
      .into(),
    Button::primary(t!("settings.facility.export_confirm", count => draft.selected.len()))
      .icon(Icon::arrow_out())
      .size(Size::Sm)
      .on_press_maybe((!draft.selected.is_empty()).then_some(Message::ExportConfirmed))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn import_error_modal(error: &facility_intel_share::ParseError) -> Element<'_, Message> {
  let body = container(
    text(import_error_text(error))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 16.0,
    right: spacing::SPACE_4_5,
    bottom: 16.0,
    left: spacing::SPACE_4_5,
  });

  let content = Column::with_children(vec![
    import_modal_header(
      color::status::WARNING,
      t!("settings.facility.import_error_title").into_owned(),
      Message::ImportErrorDismissed,
    ),
    rule::horizontal(),
    body.into(),
    rule::horizontal(),
    import_modal_footer(
      t!("settings.facility.import_error_close").into_owned(),
      Message::ImportErrorDismissed,
    ),
  ])
  .width(Length::Fill);

  import_modal_panel(content)
}

fn import_result_modal(summary: &facility_intel_import::ImportSummary) -> Element<'_, Message> {
  let content = Column::with_children(vec![
    import_modal_header(
      color::accent(),
      t!("settings.facility.import_result_title").into_owned(),
      Message::ImportResultClosed,
    ),
    rule::horizontal(),
    import_result_body(summary),
    rule::horizontal(),
    import_modal_footer(
      t!("settings.facility.import_result_close").into_owned(),
      Message::ImportResultClosed,
    ),
  ])
  .width(Length::Fill);

  import_modal_panel(content)
}

fn import_result_body(summary: &facility_intel_import::ImportSummary) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![
    text(import_summary_line(summary))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if !summary.skipped.is_empty() {
    children.push(
      text(t!("settings.facility.import_result_skipped"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
    let rows: Vec<Element<'_, Message>> = summary
      .skipped
      .iter()
      .map(|name| {
        text(name.clone())
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(typography::colored(color::text::PRIMARY))
          .into()
      })
      .collect();
    children.push(
      container(
        scrollable(Column::with_children(rows).spacing(6.0).width(Length::Fill))
          .style(crate::ui::style::control::scrollbar)
          .width(Length::Fill),
      )
      .max_height(EXPORT_LIST_MAX_HEIGHT)
      .width(Length::Fill)
      .into(),
    );
    children.push(
      text(t!("settings.facility.import_result_skipped_note"))
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .padding(Padding {
      top: 16.0,
      right: spacing::SPACE_4_5,
      bottom: 16.0,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn import_modal_header<'a>(glyph_color: iced::Color, title: String, close: Message) -> Element<'a, Message> {
  let glyph = Icon::download().size(18.0).color(glyph_color).render();
  let eyebrow = text(t!("settings.facility.export_eyebrow"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));
  let title = text(title)
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let copy = Column::with_children(vec![eyebrow.into(), title.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);
  let close = Button::ghost_icon(Icon::close()).size(Size::Sm).on_press(close);

  let row = Row::with_children(vec![glyph, copy.into(), close.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn import_modal_footer<'a>(label: String, message: Message) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    Space::new().width(Length::Fill).into(),
    Button::primary(label).size(Size::Sm).on_press(message).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn import_modal_panel(content: Column<'_, Message>) -> Element<'_, Message> {
  let panel = container(content)
    .width(Length::Fill)
    .max_width(EXPORT_PANEL_MAX_WIDTH)
    .max_height(EXPORT_PANEL_MAX_HEIGHT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  panel.into()
}

fn import_error_text(error: &facility_intel_share::ParseError) -> String {
  match error {
    facility_intel_share::ParseError::Empty => t!("settings.facility.import_error_empty").into_owned(),
    facility_intel_share::ParseError::NotAPack => t!("settings.facility.import_error_not_a_pack").into_owned(),
    facility_intel_share::ParseError::UnsupportedVersion => {
      t!("settings.facility.import_error_unsupported_version").into_owned()
    }
    facility_intel_share::ParseError::WrongFormat => t!("settings.facility.import_error_wrong_format").into_owned(),
  }
}

fn import_summary_line(summary: &facility_intel_import::ImportSummary) -> String {
  if summary.imported == 1 {
    t!("settings.facility.import_result_imported_one", count => summary.imported).into_owned()
  } else {
    t!("settings.facility.import_result_imported_other", count => summary.imported).into_owned()
  }
}

fn fit_modal<'a>(state: &'a State, draft: &'a FitDraft) -> Element<'a, Message> {
  let text = draft.content.text();
  let parsed = (!text.trim().is_empty()).then(|| {
    let catalog = rig_catalog_pairs(state);
    facility_intel_fit::parse_fit(
      &text,
      &draft.structure_name,
      &draft.facility_name,
      catalog.iter().map(|(name, id)| (name.as_str(), *id)),
    )
  });
  let has_current = state
    .intel
    .iter()
    .find(|card| card.facility.id == draft.facility_id)
    .is_some_and(|card| card.fitted() > 0);
  let rig_count = parsed.as_ref().map_or(0, |parsed| parsed.rigs.len());
  let has_fit = parsed.as_ref().is_some_and(|parsed| !parsed.eft.trim().is_empty());

  let content = Column::with_children(vec![
    fit_header(),
    rule::horizontal(),
    fit_editor(draft),
    fit_preview(state, draft, parsed, has_current),
    rule::horizontal(),
    fit_footer(rig_count, has_fit, has_current),
  ])
  .width(Length::Fill);

  container(content)
    .width(Length::Fill)
    .max_width(FIT_PANEL_MAX_WIDTH)
    .max_height(EXPORT_PANEL_MAX_HEIGHT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn fit_header<'a>() -> Element<'a, Message> {
  let glyph = Icon::fitting().size(18.0).color(color::accent()).render();
  let eyebrow = text(t!("settings.facility.fit_eyebrow"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));
  let title = text(t!("settings.facility.fit_title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let copy = Column::with_children(vec![eyebrow.into(), title.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);
  let close = Button::ghost_icon(Icon::close())
    .size(Size::Sm)
    .on_press(Message::FitClosed);

  let row = Row::with_children(vec![glyph, copy.into(), close.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn fit_editor<'a>(draft: &'a FitDraft) -> Element<'a, Message> {
  let blurb = text(t!("settings.facility.fit_blurb"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let editor = text_editor(&draft.content)
    .placeholder(super::i18n::tr_static("settings.facility.fit_placeholder"))
    .on_action(Message::FitInputChanged)
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .padding(spacing::SPACE_2_5)
    .height(Length::Fixed(FIT_EDITOR_HEIGHT))
    .style(fit_editor_style);

  Column::with_children(vec![blurb.into(), editor.into()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .padding(Padding {
      top: 16.0,
      right: spacing::SPACE_4_5,
      bottom: 0.0,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn fit_editor_style(_theme: &iced::Theme, status: text_editor::Status) -> text_editor::Style {
  let focused = matches!(status, text_editor::Status::Focused { .. });
  text_editor::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: if focused { color::accent() } else { color::rule() },
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent(), 0.3),
  }
}

fn fit_preview<'a>(
  state: &State,
  draft: &FitDraft,
  parsed: Option<facility_intel_fit::ParsedFit>,
  has_current: bool,
) -> Element<'a, Message> {
  let inner: Element<'a, Message> = match parsed {
    None => text(t!("settings.facility.fit_awaiting").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Some(parsed) => fit_result(state, draft, parsed, has_current),
  };

  container(
    scrollable(inner)
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill),
  )
  .max_height(FIT_PREVIEW_MAX_HEIGHT)
  .width(Length::Fill)
  .padding(Padding {
    top: 14.0,
    right: spacing::SPACE_4_5,
    bottom: 4.0,
    left: spacing::SPACE_4_5,
  })
  .into()
}

fn fit_result<'a>(
  state: &State,
  draft: &FitDraft,
  parsed: facility_intel_fit::ParsedFit,
  has_current: bool,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![fit_found_row(&parsed)];
  if parsed.rigs.is_empty() {
    children.push(fit_none_recognised());
  } else {
    children.push(fit_rig_list(state, &parsed));
  }
  let notes = fit_notes(&parsed, draft, has_current);
  if !notes.is_empty() {
    children.push(
      Column::with_children(notes)
        .spacing(spacing::SPACE_2)
        .width(Length::Fill)
        .into(),
    );
  }
  Column::with_children(children)
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn fit_found_row<'a>(parsed: &facility_intel_fit::ParsedFit) -> Element<'a, Message> {
  let count = parsed.rigs.len();
  let (count_label, count_color) = if count == 0 {
    (
      t!("settings.facility.fit_found_none").into_owned(),
      color::text::secondary(),
    )
  } else {
    (rig_count_label(count), color::status::ONLINE)
  };

  let mut row = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
  row = row.push(
    text(t!("settings.facility.fit_found").into_owned())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary())),
  );
  row = row.push(
    text(count_label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(count_color)),
  );
  if let Some(hull) = &parsed.hull {
    row = row.push(Space::new().width(Length::Fill));
    row = row.push(
      text(t!("settings.facility.fit_from", ship => hull.clone()).into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }
  row.width(Length::Fill).into()
}

fn fit_none_recognised<'a>() -> Element<'a, Message> {
  container(
    text(t!("settings.facility.fit_none_recognised"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3)
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

fn fit_rig_list<'a>(state: &State, parsed: &facility_intel_fit::ParsedFit) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  for (index, id) in parsed.rigs.iter().enumerate() {
    rows.push(fit_rig_row(state, index, *id));
  }
  Column::with_children(rows)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn fit_rig_row<'a>(state: &State, index: usize, type_id: i64) -> Element<'a, Message> {
  let is_me = rig_is_me(state, type_id);
  let tone = if is_me { color::accent() } else { color::status::ONLINE };
  let kind = if is_me {
    super::i18n::tr_static("settings.facility.rig_me")
  } else {
    super::i18n::tr_static("settings.facility.rig_te")
  };
  let name = rig_name(state, type_id).unwrap_or_else(|| format!("#{type_id}"));

  let ordinal = text(format!("{}", index + 1))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let label = text(name)
    .font(typography::body::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::text::PRIMARY))
    .width(Length::Fill);

  let row = Row::with_children(vec![
    ordinal.into(),
    status::dot_sized(tone, 6.0),
    label.into(),
    fit_kind_pill(kind, tone),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 8.0,
      right: 11.0,
      bottom: 8.0,
      left: 11.0,
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

fn fit_kind_pill<'a>(label: &'a str, tone: Color) -> Element<'a, Message> {
  container(
    text(label)
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(tone)),
  )
  .padding(Padding {
    top: 1.0,
    right: 6.0,
    bottom: 1.0,
    left: 6.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tone, 0.12))),
    border: Border {
      color: color::with_alpha(tone, 0.3),
      width: 1.0,
      radius: 4.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn fit_notes<'a>(
  parsed: &facility_intel_fit::ParsedFit,
  draft: &FitDraft,
  has_current: bool,
) -> Vec<Element<'a, Message>> {
  let mut notes: Vec<Element<'a, Message>> = Vec::new();
  if !parsed.rigs.is_empty() && has_current {
    notes.push(fit_note(
      color::accent(),
      t!("settings.facility.fit_note_replace").into_owned(),
    ));
  }
  if parsed.overflow > 0 {
    notes.push(fit_note(
      color::status::WARNING,
      t!("settings.facility.fit_note_overflow", count => parsed.overflow).into_owned(),
    ));
  }
  if let Some(hull) = &parsed.hull
    && !draft.structure_name.trim().is_empty()
    && norm_loose(hull) != norm_loose(&draft.structure_name)
  {
    notes.push(fit_note(
      color::status::WARNING,
      t!(
        "settings.facility.fit_note_mismatch",
        ship => hull.clone(),
        structure => draft.structure_name.clone()
      )
      .into_owned(),
    ));
  }
  if !parsed.unknown.is_empty() {
    let joined = parsed.unknown.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    let names = if parsed.unknown.len() > 3 {
      format!("{joined}\u{2026}")
    } else {
      joined
    };
    notes.push(fit_note(
      color::text::secondary(),
      t!(
        "settings.facility.fit_note_skipped",
        count => parsed.unknown.len(),
        names => names
      )
      .into_owned(),
    ));
  }
  notes
}

fn fit_note<'a>(tone: Color, message: String) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    status::dot_sized(tone, 5.0),
    text(message)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .width(Length::Fill)
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Top);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 8.0,
      right: 10.0,
      bottom: 8.0,
      left: 10.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tone, 0.07))),
      border: Border {
        color: color::with_alpha(tone, 0.26),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn fit_footer<'a>(rig_count: usize, has_fit: bool, has_current: bool) -> Element<'a, Message> {
  let hint = if rig_count > 0 {
    t!("settings.facility.fit_footer_slots", label => rig_count_label(rig_count)).into_owned()
  } else {
    t!("settings.facility.fit_footer_empty").into_owned()
  };
  let label = if has_current {
    t!("settings.facility.fit_replace")
  } else {
    t!("settings.facility.fit_apply")
  };

  let row = Row::with_children(vec![
    text(hint)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .width(Length::Fill)
      .into(),
    Button::ghost(t!("settings.facility.cancel"))
      .size(Size::Sm)
      .on_press(Message::FitClosed)
      .into(),
    Button::primary(label)
      .icon(Icon::check())
      .size(Size::Sm)
      .on_press_maybe(has_fit.then_some(Message::FitApplied))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn rig_is_me(state: &State, type_id: i64) -> bool {
  state
    .rigs
    .iter()
    .find(|rig| rig.type_id == type_id)
    .is_none_or(|rig| rig.me != 0.0)
}

fn rig_count_label(count: usize) -> String {
  if count == 1 {
    t!("settings.facility.fit_rig_count_one", count => count).into_owned()
  } else {
    t!("settings.facility.fit_rig_count_other", count => count).into_owned()
  }
}

fn norm_loose(value: &str) -> String {
  value
    .chars()
    .filter(|ch| ch.is_ascii_alphanumeric())
    .map(|ch| ch.to_ascii_lowercase())
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  fn facility(id: i64) -> FacilityRef {
    FacilityRef {
      cost_index: Some(0.05),
      id,
      name: "Sotiyo".to_owned(),
      region: Some("The Forge".to_owned()),
      security_status: Some(0.9),
      solar_system: "Jita".to_owned(),
      solar_system_id: 30_000_142,
      type_id: Some(35_827),
      type_label: Some("Sotiyo".to_owned()),
    }
  }

  async fn state_with_db() -> (State, Settings) {
    let db = store::open_test().await.unwrap();
    (State::new(db), Settings::default())
  }

  mod strip_system_prefix {
    use super::super::strip_system_prefix;

    #[test]
    fn it_drops_the_leading_system_and_separator() {
      assert_eq!(
        strip_system_prefix("Nourvukaiken - The R&D Space Party", "Nourvukaiken"),
        "The R&D Space Party"
      );
    }

    #[test]
    fn it_keeps_a_name_that_does_not_start_with_the_system() {
      assert_eq!(strip_system_prefix("Free Indu Port", "Purjola"), "Free Indu Port");
    }

    #[test]
    fn it_keeps_the_name_when_the_system_is_blank_or_would_leave_nothing() {
      assert_eq!(strip_system_prefix("Jita IV - Moon 4", ""), "Jita IV - Moon 4");
      assert_eq!(strip_system_prefix("Purjola -", "Purjola"), "Purjola -");
    }
  }

  mod name_key {
    use std::cmp::Ordering;

    use super::{
      super::{IntelCard, RIG_SLOTS, SortBy, compare_cards, name_key},
      facility,
    };

    fn labelled(name: &str, system: &str) -> IntelCard {
      let mut facility = facility(1);
      facility.name = name.to_owned();
      facility.solar_system = system.to_owned();
      IntelCard {
        eft: None,
        facility,
        owner: None,
        rigs: [None; RIG_SLOTS],
      }
    }

    #[test]
    fn it_orders_by_the_stripped_display_name_not_the_full_name() {
      let zulu = labelled("Amarr - Zulu Base", "Amarr");
      let alpha = labelled("Zeta - Alpha Base", "Zeta");

      assert_eq!(compare_cards(SortBy::Name, &alpha, &zulu), Ordering::Less);
      assert_eq!(compare_cards(SortBy::Name, &zulu, &alpha), Ordering::Greater);
    }

    #[test]
    fn it_sorts_an_unresolved_system_row_by_its_displayed_full_name() {
      let card = labelled("Jita - Trade Hub", "");

      assert_eq!(name_key(&card), "jita - trade hub");
    }
  }

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_both_defaults_when_each_is_resolved() {
      let mut state = State::default();
      state.manufacturing.selection = Some(facility(60_003_760));
      state.reactions.selection = Some(facility(1_021_000_000_001));

      assert_eq!(badge(&state), "2/2");
    }

    #[test]
    fn it_counts_zero_defaults_for_a_fresh_state() {
      assert_eq!(badge(&State::default()), "0/2");
    }
  }

  mod resolve_default_market {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{Constellation, Region, SolarSystem, Structure};

    fn make_constellation(id: i64, name: &str) -> Constellation {
      Constellation {
        id,
        name: name.to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        region_id: 10_000_002,
      }
    }

    fn make_region(id: i64, name: &str) -> Region {
      Region {
        description: None,
        id,
        name: name.to_owned(),
      }
    }

    fn make_solar_system(id: i64, name: &str) -> SolarSystem {
      SolarSystem {
        constellation_id: 20_000_020,
        id,
        name: name.to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        security_class: None,
        security_status: 0.9,
        star_id: None,
      }
    }

    fn make_structure(id: i64, name: &str) -> Structure {
      Structure {
        id,
        name: name.to_owned(),
        owner_id: 98_000_001,
        position_x: None,
        position_y: None,
        position_z: None,
        solar_system_id: 30_000_142,
        type_id: None,
      }
    }

    async fn seed_structure_parents(db: &Database) {
      sde::upsert_region(db, &make_region(10_000_002, "The Forge"))
        .await
        .unwrap();
      sde::upsert_constellation(db, &make_constellation(20_000_020, "Kimotoro"))
        .await
        .unwrap();
      sde::upsert_solar_system(db, &make_solar_system(30_000_142, "Jita"))
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
        VALUES (98000001, 1, 1, 1, 'Owner Corp', 0.0, 'OWN')",
      )
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_returns_none_when_no_default_is_set() {
      let db = store::open_test().await.unwrap();

      assert!(resolve_default_market(&db, None).await.is_none());
    }

    #[tokio::test]
    async fn it_resolves_a_region_default() {
      let db = store::open_test().await.unwrap();
      sde::upsert_region(&db, &make_region(10_000_002, "The Forge"))
        .await
        .unwrap();
      market::set_default_market(&db, 10_000_002).await.unwrap();

      let resolved = resolve_default_market(&db, None).await;

      assert_eq!(
        resolved.map(|place| (place.id, place.tier)),
        Some((10_000_002, Some(LocationTier::Region)))
      );
    }

    #[tokio::test]
    async fn it_round_trips_a_cached_structure_default() {
      let db = store::open_test().await.unwrap();
      seed_structure_parents(&db).await;
      sde::upsert_structure(&db, &make_structure(1_035_000_000_001, "Jita Trade Hub"))
        .await
        .unwrap();
      market::set_default_market(&db, 1_035_000_000_001).await.unwrap();

      let persisted = market::default_market(&db).await.unwrap();
      let resolved = resolve_default_market(&db, None).await;

      assert_eq!(
        persisted,
        Some(1_035_000_000_001),
        "the structure id persists as the default"
      );
      assert_eq!(
        resolved.map(|place| (place.id, place.name, place.tier)),
        Some((
          1_035_000_000_001,
          "Jita Trade Hub".to_owned(),
          Some(LocationTier::Structure)
        ))
      );
    }
  }

  mod resolve_place {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{Constellation, Region, SolarSystem, Station};

    const REGION: i64 = 10_000_002;
    const CONSTELLATION: i64 = 20_000_020;
    const SYSTEM: i64 = 30_000_142;
    const STATION: i64 = 60_003_760;
    const STATION_TYPE: i64 = 54_678;

    fn make_region(id: i64, name: &str) -> Region {
      Region {
        description: None,
        id,
        name: name.to_owned(),
      }
    }

    fn make_constellation(id: i64, name: &str) -> Constellation {
      Constellation {
        id,
        name: name.to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        region_id: REGION,
      }
    }

    fn make_solar_system(id: i64, name: &str) -> SolarSystem {
      SolarSystem {
        constellation_id: CONSTELLATION,
        id,
        name: name.to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        security_class: None,
        security_status: 0.9,
        star_id: None,
      }
    }

    fn make_station(id: i64, name: &str) -> Station {
      Station {
        id,
        max_dockable_ship_volume: 0.0,
        name: name.to_owned(),
        office_rental_cost: 0.0,
        owner: None,
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        race_id: None,
        reprocessing_efficiency: 0.0,
        reprocessing_stations_take: 0.0,
        services: String::new(),
        system_id: SYSTEM,
        type_id: STATION_TYPE,
      }
    }

    async fn seed_geo(db: &Database) {
      sde::upsert_region(db, &make_region(REGION, "The Forge")).await.unwrap();
      sde::upsert_constellation(db, &make_constellation(CONSTELLATION, "Kimotoro"))
        .await
        .unwrap();
      sde::upsert_solar_system(db, &make_solar_system(SYSTEM, "Jita"))
        .await
        .unwrap();
    }

    async fn seed_station_type(db: &Database) {
      sqlx::query("INSERT INTO item_categories (id, name, published) VALUES (6, 'Ship', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT INTO item_groups (id, category_id, name, published) VALUES (25, 6, 'Frigate', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO item_types (id, group_id, description, name, published) VALUES (?, 25, '', 'Station Type', 1)",
      )
      .bind(STATION_TYPE)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_is_none_for_an_id_below_every_tier_range() {
      let db = store::open_test().await.unwrap();

      assert!(resolve_place(&db, None, 42).await.is_none());
    }

    #[tokio::test]
    async fn it_resolves_a_constellation() {
      let db = store::open_test().await.unwrap();
      seed_geo(&db).await;

      let resolved = resolve_place(&db, None, CONSTELLATION).await;

      assert_eq!(
        resolved.map(|place| (place.id, place.name, place.tier)),
        Some((CONSTELLATION, "Kimotoro".to_owned(), Some(LocationTier::Constellation)))
      );
    }

    #[tokio::test]
    async fn it_resolves_a_system() {
      let db = store::open_test().await.unwrap();
      seed_geo(&db).await;

      let resolved = resolve_place(&db, None, SYSTEM).await;

      assert_eq!(
        resolved.map(|place| (place.id, place.name, place.tier)),
        Some((SYSTEM, "Jita".to_owned(), Some(LocationTier::System)))
      );
    }

    #[tokio::test]
    async fn it_resolves_a_station() {
      let db = store::open_test().await.unwrap();
      seed_geo(&db).await;
      seed_station_type(&db).await;
      sde::upsert_station(&db, &make_station(STATION, "Jita IV - Moon 4"))
        .await
        .unwrap();

      let resolved = resolve_place(&db, None, STATION).await;

      assert_eq!(
        resolved.map(|place| (place.id, place.name, place.tier)),
        Some((STATION, "Jita IV - Moon 4".to_owned(), Some(LocationTier::Station)))
      );
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_selects_a_station_default() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(
        &mut state,
        Message::FacilityPicked {
          activity: MANUFACTURING_ACTIVITY_ID,
          facility: facility(60_003_760),
        },
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(state.manufacturing.selection.as_ref().map(|f| f.id), Some(60_003_760));
    }

    #[tokio::test]
    async fn it_persists_a_structure_default_without_pinning() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(
        &mut state,
        Message::FacilityPicked {
          activity: REACTION_ACTIVITY_ID,
          facility: facility(1_021_000_000_001),
        },
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(
        state.reactions.selection.as_ref().map(|f| f.id),
        Some(1_021_000_000_001)
      );
    }

    #[tokio::test]
    async fn it_clears_a_default_selection() {
      let (mut state, mut settings) = state_with_db().await;
      state.manufacturing.selection = Some(facility(60_003_760));

      let (outcome, _task) = update(
        &mut state,
        Message::Cleared {
          activity: MANUFACTURING_ACTIVITY_ID,
        },
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert!(state.manufacturing.selection.is_none());
    }

    fn region(id: i64, name: &str) -> LocationRef {
      LocationRef {
        context: None,
        id,
        name: name.to_owned(),
        security_status: None,
        tier: Some(LocationTier::Region),
      }
    }

    fn structure_place(id: i64, name: &str) -> LocationRef {
      LocationRef {
        context: None,
        id,
        name: name.to_owned(),
        security_status: None,
        tier: Some(LocationTier::Structure),
      }
    }

    #[tokio::test]
    async fn it_selects_a_default_market_region() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(
        &mut state,
        Message::MarketPicked(region(10_000_002, "The Forge")),
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(state.market.selection.as_ref().map(|r| r.id), Some(10_000_002));
      assert!(!state.market.open, "picking closes the picker");
    }

    #[tokio::test]
    async fn it_clears_the_default_market_region() {
      let (mut state, mut settings) = state_with_db().await;
      state.market.selection = Some(region(10_000_002, "The Forge"));

      let (outcome, _task) = update(&mut state, Message::MarketCleared, &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(state.market.selection.is_none());
    }

    #[tokio::test]
    async fn it_toggles_the_market_picker_open_and_closed() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(&mut state, Message::MarketPickerToggled, &mut settings);
      assert!(state.market.open);

      state.market.query = "For".to_owned();
      let _ = update(&mut state, Message::MarketPickerToggled, &mut settings);
      assert!(!state.market.open);
      assert!(state.market.query.is_empty(), "closing clears the query");
    }

    #[tokio::test]
    async fn it_selects_a_structure_default_market() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(
        &mut state,
        Message::MarketPicked(structure_place(1_035_000_000_001, "Jita Trade Hub")),
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(
        state.market.selection.as_ref().map(|place| place.id),
        Some(1_035_000_000_001)
      );
      assert!(!state.market.open, "picking a structure closes the picker");
    }

    #[tokio::test]
    async fn it_gates_the_market_search_below_the_min_char_threshold() {
      let (mut state, mut settings) = state_with_db().await;
      state.market.results = vec![region(10_000_002, "The Forge")];
      state.market.searching = true;

      let (outcome, _task) = update(&mut state, Message::MarketQueryChanged("f".to_owned()), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(
        state.market.results.is_empty(),
        "a sub-threshold query clears stale results"
      );
      assert!(!state.market.searching);
      assert!(state.market.open);
    }

    #[tokio::test]
    async fn it_discards_stale_market_results_by_generation() {
      let (mut state, mut settings) = state_with_db().await;
      state.market.generation = 5;
      state.market.searching = true;

      let _ = update(
        &mut state,
        Message::MarketResults {
          generation: 4,
          results: vec![region(10_000_002, "The Forge")],
        },
        &mut settings,
      );
      assert!(state.market.results.is_empty(), "an older generation is ignored");
      assert!(state.market.searching, "a stale response leaves the searching flag set");

      let _ = update(
        &mut state,
        Message::MarketResults {
          generation: 5,
          results: vec![region(10_000_002, "The Forge")],
        },
        &mut settings,
      );
      assert_eq!(state.market.results.len(), 1, "the current generation is accepted");
      assert!(!state.market.searching);
    }

    #[tokio::test]
    async fn it_adds_an_intel_card_when_the_composer_picks_a_facility() {
      let (mut state, mut settings) = state_with_db().await;
      state.composer.open = true;

      let (_outcome, _task) = update(
        &mut state,
        Message::FacilityPicked {
          activity: COMPOSER_ACTIVITY_ID,
          facility: facility(60_003_760),
        },
        &mut settings,
      );

      assert_eq!(state.intel.len(), 1);
      assert_eq!(state.intel[0].facility.id, 60_003_760);
      assert!(!state.composer.open, "picking closes the composer");
    }

    #[tokio::test]
    async fn it_does_not_duplicate_an_already_tracked_facility() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });

      let _ = update(
        &mut state,
        Message::FacilityPicked {
          activity: COMPOSER_ACTIVITY_ID,
          facility: facility(60_003_760),
        },
        &mut settings,
      );

      assert_eq!(state.intel.len(), 1);
    }

    #[tokio::test]
    async fn it_removes_an_intel_card() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });

      let _ = update(&mut state, Message::RemoveFacility(60_003_760), &mut settings);

      assert!(state.intel.is_empty());
    }

    #[tokio::test]
    async fn it_fits_a_rig_into_a_slot() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      state.open_rig = Some(OpenRig {
        facility_id: 1_021_000_000_001,
        search: RigSearch::default(),
        slot: 1,
      });

      let rig = RigRef {
        activity: RigActivity::Manufacturing,
        fee: 0.0,
        me: -2.0,
        name: "Standup M-Set ME I".to_owned(),
        te: 0.0,
        type_id: 37_180,
      };
      let _ = update(
        &mut state,
        Message::RigPicked {
          facility_id: 1_021_000_000_001,
          rig: Box::new(rig),
          slot: 1,
        },
        &mut settings,
      );

      assert_eq!(state.intel[0].rigs[1], Some(37_180));
      assert!(state.open_rig.is_none(), "fitting closes the open picker");
    }

    #[tokio::test]
    async fn it_toggles_a_rig_slot_open_and_closed() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(
        &mut state,
        Message::RigSlotToggled {
          facility_id: 7,
          slot: 0,
        },
        &mut settings,
      );
      assert!(state.open_rig.is_some());

      let _ = update(
        &mut state,
        Message::RigSlotToggled {
          facility_id: 7,
          slot: 0,
        },
        &mut settings,
      );
      assert!(state.open_rig.is_none());
    }

    #[tokio::test]
    async fn it_emits_a_search_outcome_for_the_composer() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(
        &mut state,
        Message::QueryChanged {
          activity: COMPOSER_ACTIVITY_ID,
          query: "Sotiyo".to_owned(),
        },
        &mut settings,
      );

      assert_eq!(
        outcome,
        Outcome::IndustrySearch {
          activity: COMPOSER_ACTIVITY_ID,
          generation: 1,
          query: "Sotiyo".to_owned(),
        }
      );
    }

    #[tokio::test]
    async fn it_toggles_the_composer_and_clears_its_search() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(&mut state, Message::ComposerToggled(true), &mut settings);
      assert!(state.composer.open);

      let _ = update(&mut state, Message::ComposerToggled(false), &mut settings);
      assert!(!state.composer.open);
    }

    #[tokio::test]
    async fn it_toggles_a_default_picker_open_and_closed() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(
        &mut state,
        Message::PickerToggled {
          activity: MANUFACTURING_ACTIVITY_ID,
        },
        &mut settings,
      );
      assert!(state.manufacturing.open);

      let _ = update(
        &mut state,
        Message::PickerToggled {
          activity: MANUFACTURING_ACTIVITY_ID,
        },
        &mut settings,
      );
      assert!(!state.manufacturing.open);
    }

    #[tokio::test]
    async fn it_applies_a_loaded_payload_to_state() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Ok(Loaded {
          facilities_count: 5,
          intel: vec![IntelCard {
            eft: None,
            facility: facility(60_003_760),
            owner: Some("Owner Corp".to_owned()),
            rigs: [None; RIG_SLOTS],
          }],
          manufacturing: Some(facility(60_003_760)),
          market: Some(region(10_000_002, "The Forge")),
          reactions: None,
          rig_catalog: HashMap::new(),
          rigs: Vec::new(),
        }))),
        &mut settings,
      );

      assert_eq!(state.facilities_count, 5);
      assert_eq!(state.manufacturing.selection.as_ref().map(|f| f.id), Some(60_003_760));
      assert_eq!(state.market.selection.as_ref().map(|r| r.id), Some(10_000_002));
      assert_eq!(state.intel.len(), 1);
    }

    #[tokio::test]
    async fn it_records_a_load_error() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Err("boom".to_owned()))),
        &mut settings,
      );

      assert_eq!(state.load_error.as_deref(), Some("boom"));
    }

    #[tokio::test]
    async fn it_clears_a_fitted_rig_slot() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: None,
        rigs: [Some(37_180), None, None],
      });

      let _ = update(
        &mut state,
        Message::RigCleared {
          facility_id: 1_021_000_000_001,
          slot: 0,
        },
        &mut settings,
      );

      assert_eq!(state.intel[0].rigs[0], None);
    }

    #[tokio::test]
    async fn it_dismisses_an_open_rig_picker() {
      let (mut state, mut settings) = state_with_db().await;
      let _ = update(
        &mut state,
        Message::RigSlotToggled {
          facility_id: 7,
          slot: 0,
        },
        &mut settings,
      );
      assert!(state.open_rig.is_some());

      let _ = update(&mut state, Message::RigDismissed, &mut settings);

      assert!(state.open_rig.is_none());
    }

    #[tokio::test]
    async fn it_filters_the_rigs_in_an_open_slot() {
      let (mut state, mut settings) = state_with_db().await;
      let _ = update(
        &mut state,
        Message::RigSlotToggled {
          facility_id: 7,
          slot: 0,
        },
        &mut settings,
      );

      let _ = update(
        &mut state,
        Message::RigQueryChanged {
          facility_id: 7,
          query: "ME".to_owned(),
          slot: 0,
        },
        &mut settings,
      );

      assert_eq!(state.open_rig.as_ref().map(|open| open.search.query()), Some("ME"));
    }

    #[tokio::test]
    async fn it_reloads_after_a_successful_save() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(&mut state, Message::Saved(Ok(())), &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[tokio::test]
    async fn it_ignores_a_failed_save() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(&mut state, Message::Saved(Err("nope".to_owned())), &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[tokio::test]
    async fn it_opens_the_export_modal_preselecting_all_facilities() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: None,
        rigs: [Some(37_180), None, None],
      });

      let (outcome, _task) = update(&mut state, Message::ExportOpened, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(
        state.export.as_ref().map(|draft| draft.selected.clone()),
        Some(BTreeSet::from([60_003_760, 1_021_000_000_001]))
      );
    }

    #[tokio::test]
    async fn it_does_not_open_the_export_modal_without_facilities() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(&mut state, Message::ExportOpened, &mut settings);

      assert!(state.export.is_none());
    }

    #[tokio::test]
    async fn it_toggles_bulk_selects_and_clears_export_facilities() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      let _ = update(&mut state, Message::ExportOpened, &mut settings);

      let _ = update(&mut state, Message::ExportFacilityToggled(60_003_760), &mut settings);
      assert_eq!(
        state.export.as_ref().map(|draft| draft.selected.clone()),
        Some(BTreeSet::from([1_021_000_000_001]))
      );

      let _ = update(&mut state, Message::ExportNoneSelected, &mut settings);
      assert!(state.export.as_ref().is_some_and(|draft| draft.selected.is_empty()));

      let _ = update(&mut state, Message::ExportAllSelected, &mut settings);
      assert_eq!(
        state.export.as_ref().map(|draft| draft.selected.clone()),
        Some(BTreeSet::from([60_003_760, 1_021_000_000_001]))
      );
    }

    #[tokio::test]
    async fn it_closes_the_export_modal_without_exporting() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      let _ = update(&mut state, Message::ExportOpened, &mut settings);

      let (outcome, _task) = update(&mut state, Message::ExportClosed, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.export.is_none());
    }

    #[tokio::test]
    async fn it_confirms_an_export_with_only_the_selected_facilities() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: None,
        rigs: [Some(37_180), None, Some(43_704)],
      });
      let _ = update(&mut state, Message::ExportOpened, &mut settings);
      let _ = update(&mut state, Message::ExportFacilityToggled(60_003_760), &mut settings);

      let (outcome, _task) = update(&mut state, Message::ExportConfirmed, &mut settings);

      let Outcome::ExportIntel {
        facilities,
      } = outcome
      else {
        panic!("expected an ExportIntel outcome");
      };
      assert_eq!(facilities.len(), 1);
      assert_eq!(facilities[0].facility_id, 1_021_000_000_001);
      assert_eq!(facilities[0].rigs, [Some(37_180), None, Some(43_704)]);
      assert_eq!(facilities[0].name.as_deref(), Some("Sotiyo"));
      assert_eq!(facilities[0].solar_system_id, Some(30_000_142));
      assert_eq!(facilities[0].type_id, Some(35_827));

      assert!(state.export.is_none(), "confirming closes the modal");
    }

    #[tokio::test]
    async fn it_ignores_a_confirm_with_nothing_selected() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      let _ = update(&mut state, Message::ExportOpened, &mut settings);
      let _ = update(&mut state, Message::ExportNoneSelected, &mut settings);

      let (outcome, _task) = update(&mut state, Message::ExportConfirmed, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.export.is_none());
    }

    #[tokio::test]
    async fn it_requests_an_import_resolution_for_a_valid_pack() {
      let (mut state, mut settings) = state_with_db().await;
      let pack = facility_intel_share::build_pack(vec![facility_intel_share::portable_facility(&FacilityIntel {
        eft: None,
        facility_id: 60_003_760,
        name: Some("Hub".to_owned()),
        rig_1_type_id: Some(37_180),
        rig_2_type_id: None,
        rig_3_type_id: None,
        solar_system_id: Some(30_000_142),
        type_id: None,
      })]);
      let encoded = facility_intel_share::encode_pack(&pack).unwrap();

      let (outcome, _task) = update(&mut state, Message::ImportFileLoaded(Some(encoded)), &mut settings);

      let Outcome::ImportIntel {
        facilities,
      } = outcome
      else {
        panic!("expected an ImportIntel outcome");
      };
      assert_eq!(facilities.len(), 1);
      assert_eq!(facilities[0].facility_id, 60_003_760);
      assert_eq!(facilities[0].rigs, [Some(37_180), None, None]);
      assert!(state.import_error.is_none());
    }

    #[tokio::test]
    async fn it_surfaces_the_failure_dialog_for_a_corrupt_pack() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(
        &mut state,
        Message::ImportFileLoaded(Some("definitely not a pack".to_owned())),
        &mut settings,
      );

      assert_eq!(outcome, Outcome::None);
      assert_eq!(state.import_error, Some(facility_intel_share::ParseError::NotAPack));
    }

    #[tokio::test]
    async fn it_surfaces_the_failure_dialog_for_a_future_pack_version() {
      let (mut state, mut settings) = state_with_db().await;
      let pack = facility_intel_share::build_pack(vec![facility_intel_share::portable_facility(&FacilityIntel {
        eft: None,
        facility_id: 60_003_760,
        name: None,
        rig_1_type_id: None,
        rig_2_type_id: None,
        rig_3_type_id: None,
        solar_system_id: None,
        type_id: None,
      })]);
      let encoded = crate::services::pod_pack::encode(
        crate::services::pod_pack::TAG_FACILITY_INTEL,
        facility_intel_share::PACK_VERSION + 1,
        &pack,
      )
      .unwrap();

      let (outcome, _task) = update(&mut state, Message::ImportFileLoaded(Some(encoded)), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(
        state.import_error,
        Some(facility_intel_share::ParseError::UnsupportedVersion)
      );
    }

    #[tokio::test]
    async fn it_ignores_a_cancelled_import_dialog() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(&mut state, Message::ImportFileLoaded(None), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.import_error.is_none());
      assert!(state.import_result.is_none());
    }

    #[tokio::test]
    async fn it_dismisses_the_import_failure_dialog() {
      let (mut state, mut settings) = state_with_db().await;
      state.import_error = Some(facility_intel_share::ParseError::NotAPack);

      let (outcome, _task) = update(&mut state, Message::ImportErrorDismissed, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.import_error.is_none());
    }

    #[tokio::test]
    async fn it_clears_a_stale_import_error_when_reopening_the_dialog() {
      let (mut state, mut settings) = state_with_db().await;
      state.import_error = Some(facility_intel_share::ParseError::NotAPack);

      let (outcome, _task) = update(&mut state, Message::ImportOpened, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.import_error.is_none());
    }

    #[tokio::test]
    async fn it_shows_the_result_modal_when_an_import_finishes() {
      let (mut state, mut settings) = state_with_db().await;
      let summary = facility_intel_import::ImportSummary {
        imported: 2,
        skipped: vec!["Allied Fortizar \u{b7} Jita".to_owned()],
      };

      let (outcome, _task) = update(&mut state, Message::ImportFinished(summary.clone()), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(state.import_result, Some(summary));
    }

    #[tokio::test]
    async fn it_closes_the_import_result_modal() {
      let (mut state, mut settings) = state_with_db().await;
      state.import_result = Some(facility_intel_import::ImportSummary::default());

      let (outcome, _task) = update(&mut state, Message::ImportResultClosed, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.import_result.is_none());
    }

    #[tokio::test]
    async fn it_accepts_search_results_into_a_picker() {
      let (mut state, mut settings) = state_with_db().await;
      let _ = update(
        &mut state,
        Message::QueryChanged {
          activity: MANUFACTURING_ACTIVITY_ID,
          query: "Sot".to_owned(),
        },
        &mut settings,
      );

      let (outcome, _task) = update(
        &mut state,
        Message::SearchResults {
          activity: MANUFACTURING_ACTIVITY_ID,
          generation: 1,
          results: Vec::new(),
        },
        &mut settings,
      );

      assert_eq!(outcome, Outcome::None);
    }
  }

  mod sort {
    use std::cmp::Ordering;

    use pretty_assertions::assert_eq;

    use super::*;

    fn card(name: &str, system: &str, region: Option<&str>, security: Option<f64>, fitted: usize) -> IntelCard {
      let mut rigs = [None; RIG_SLOTS];
      for slot in rigs.iter_mut().take(fitted) {
        *slot = Some(37_180);
      }
      IntelCard {
        eft: None,
        facility: FacilityRef {
          cost_index: None,
          id: 0,
          name: name.to_owned(),
          region: region.map(str::to_owned),
          security_status: security,
          solar_system: system.to_owned(),
          solar_system_id: 0,
          type_id: None,
          type_label: None,
        },
        owner: None,
        rigs,
      }
    }

    fn names(state: &State) -> Vec<String> {
      sorted_intel(state).iter().map(|c| c.facility.name.clone()).collect()
    }

    #[test]
    fn it_orders_by_name_case_insensitively() {
      let a = card("azbel", "Amarr", None, None, 0);
      let b = card("Sotiyo", "Jita", None, None, 0);

      assert_eq!(compare_cards(SortBy::Name, &a, &b), Ordering::Less);
    }

    #[test]
    fn it_orders_by_system_then_tie_breaks_by_name() {
      let a = card("Zzz", "Jita", None, None, 0);
      let b = card("Aaa", "Jita", None, None, 0);

      assert_eq!(compare_cards(SortBy::System, &a, &b), Ordering::Greater);
    }

    #[test]
    fn it_sorts_a_missing_system_last() {
      let known = card("Alpha", "Jita", None, None, 0);
      let missing = card("Beta", "  ", None, None, 0);

      assert_eq!(compare_cards(SortBy::System, &known, &missing), Ordering::Less);
    }

    #[test]
    fn it_orders_by_region_and_sorts_none_last() {
      let known = card("Alpha", "Jita", Some("The Forge"), None, 0);
      let missing = card("Beta", "Jita", None, None, 0);

      assert_eq!(compare_cards(SortBy::Region, &known, &missing), Ordering::Less);
    }

    #[test]
    fn it_orders_by_security_high_to_low_with_missing_lowest() {
      let high = card("Alpha", "Jita", None, Some(0.9), 0);
      let low = card("Beta", "Jita", None, Some(0.4), 0);
      let missing = card("Gamma", "Jita", None, None, 0);

      assert_eq!(compare_cards(SortBy::Security, &high, &low), Ordering::Less);
      assert_eq!(compare_cards(SortBy::Security, &low, &missing), Ordering::Less);
    }

    #[test]
    fn it_tie_breaks_equal_security_by_name() {
      let a = card("Bbb", "Jita", None, Some(0.9), 0);
      let b = card("Aaa", "Jita", None, Some(0.9), 0);

      assert_eq!(compare_cards(SortBy::Security, &a, &b), Ordering::Greater);
    }

    #[test]
    fn it_orders_by_rigs_fitted_most_first_then_name() {
      let many = card("Zzz", "Jita", None, None, 3);
      let few = card("Aaa", "Jita", None, None, 1);

      assert_eq!(compare_cards(SortBy::Rigs, &many, &few), Ordering::Less);

      let a = card("Bbb", "Jita", None, None, 2);
      let b = card("Aaa", "Jita", None, None, 2);
      assert_eq!(compare_cards(SortBy::Rigs, &a, &b), Ordering::Greater);
    }

    #[test]
    fn it_sorts_the_intel_list_by_the_active_key() {
      let mut state = State {
        intel: vec![
          card("Charlie", "Amarr", Some("Domain"), Some(0.5), 1),
          card("Alpha", "Jita", Some("The Forge"), Some(0.9), 3),
          card("Bravo", "Rens", None, None, 0),
        ],
        ..Default::default()
      };

      state.sort = SortBy::Name;
      assert_eq!(names(&state), vec!["Alpha", "Bravo", "Charlie"]);

      state.sort = SortBy::Security;
      assert_eq!(names(&state), vec!["Alpha", "Charlie", "Bravo"]);

      state.sort = SortBy::Rigs;
      assert_eq!(names(&state), vec!["Alpha", "Charlie", "Bravo"]);

      state.sort = SortBy::Region;
      assert_eq!(names(&state), vec!["Charlie", "Alpha", "Bravo"]);
    }

    #[tokio::test]
    async fn it_changes_the_sort_key_and_closes_the_menu() {
      let (mut state, mut settings) = state_with_db().await;
      state.sort_open = true;

      let (outcome, _task) = update(&mut state, Message::SortChanged(SortBy::Security), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(state.sort, SortBy::Security);
      assert!(!state.sort_open, "changing the sort closes the menu");
    }

    #[tokio::test]
    async fn it_toggles_and_dismisses_the_sort_menu() {
      let (mut state, mut settings) = state_with_db().await;

      let _ = update(&mut state, Message::SortMenuToggled, &mut settings);
      assert!(state.sort_open);

      let _ = update(&mut state, Message::SortMenuToggled, &mut settings);
      assert!(!state.sort_open);

      state.sort_open = true;
      let _ = update(&mut state, Message::SortMenuDismissed, &mut settings);
      assert!(!state.sort_open);
    }
  }

  mod loaders {
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::{
      clients::{esi, eve_sso, http},
      store::{
        model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
        repo::{character, infra},
      },
    };

    const STRUCTURE_ID: i64 = 1_021_000_000_001;

    async fn clients_for(db: &Database, base_url: &str) -> crate::features::industry::Clients {
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      crate::features::industry::Clients {
        esi: std::sync::Arc::new(esi::Client::with_base_url(http.clone(), base_url)),
        sso: std::sync::Arc::new(eve_sso::Client::new(http, "test-client")),
      }
    }

    async fn seed_owned_character(db: &Database) {
      let char_id = 42;
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, char_id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(char_id);
      corp.set_creator_id(char_id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(char_id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      let far_future = chrono::Utc::now().timestamp() + 86_400;
      infra::upsert(db, char_id, OwnerType::Character, "tok", "rt", far_future, None, None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_loads_an_empty_database_without_error() {
      let db = store::open_test().await.unwrap();

      let loaded = load_all(db, None)
        .await
        .expect("load_all succeeds on a migrated database");

      assert_eq!(loaded.facilities_count, 0);
      assert!(loaded.intel.is_empty());
      assert!(loaded.manufacturing.is_none());
      assert!(loaded.reactions.is_none());
      assert!(loaded.rigs.is_empty());
      assert!(loaded.rig_catalog.is_empty());
    }

    #[tokio::test]
    async fn it_resolves_a_structure_default_through_esi_when_it_is_not_corp_synced() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"name":"Jita Keepstar","owner_id":98000001,"solar_system_id":30000142,"type_id":35834}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let clients = clients_for(&db, &server.uri()).await;
      seed_owned_character(&db).await;
      industry::set_default_facility(&db, DB_MANUFACTURING_ACTIVITY_ID, STRUCTURE_ID)
        .await
        .unwrap();

      let loaded = load_all(db, Some(clients)).await.unwrap();

      let manufacturing = loaded.manufacturing.expect("structure default resolves via ESI");
      assert_eq!(manufacturing.id, STRUCTURE_ID);
      assert_eq!(manufacturing.name, "Jita Keepstar");
      assert_eq!(manufacturing.solar_system_id, 30_000_142);
    }

    #[tokio::test]
    async fn it_degrades_a_structure_default_to_unset_when_esi_denies_it() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path(format!("/universe/structures/{STRUCTURE_ID}/")))
        .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":"Forbidden"}"#, "application/json"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let clients = clients_for(&db, &server.uri()).await;
      seed_owned_character(&db).await;
      industry::set_default_facility(&db, DB_MANUFACTURING_ACTIVITY_ID, STRUCTURE_ID)
        .await
        .unwrap();

      let loaded = load_all(db, Some(clients)).await.unwrap();

      assert!(loaded.manufacturing.is_none());
    }
  }

  mod merge_intel {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_an_optimistic_card_the_reload_could_not_resolve() {
      let optimistic = vec![IntelCard {
        eft: None,
        facility: facility(1_021_000_000_002),
        owner: None,
        rigs: [None; RIG_SLOTS],
      }];

      let merged = merge_intel(optimistic, Vec::new());

      assert_eq!(merged.len(), 1);
      assert_eq!(merged[0].facility.id, 1_021_000_000_002);
    }

    #[test]
    fn it_prefers_the_reloaded_card_for_a_resolved_facility() {
      let optimistic = vec![IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      }];
      let reloaded = vec![IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: Some("Owner Corp".to_owned()),
        rigs: [Some(37_180), None, None],
      }];

      let merged = merge_intel(optimistic, reloaded);

      assert_eq!(merged.len(), 1);
      assert_eq!(merged[0].owner.as_deref(), Some("Owner Corp"));
    }
  }

  mod import_error_text {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_every_parse_error_to_a_distinct_human_message() {
      let errors = [
        facility_intel_share::ParseError::Empty,
        facility_intel_share::ParseError::NotAPack,
        facility_intel_share::ParseError::UnsupportedVersion,
        facility_intel_share::ParseError::WrongFormat,
      ];

      let messages: Vec<String> = errors.iter().map(import_error_text).collect();

      for message in &messages {
        assert!(!message.is_empty());
        assert!(!message.contains("settings.facility"), "unresolved i18n key: {message}");
      }
      let unique: BTreeSet<&String> = messages.iter().collect();
      assert_eq!(unique.len(), errors.len());
    }
  }

  mod import_summary_line {
    use super::*;

    #[test]
    fn it_picks_the_singular_and_plural_forms() {
      let one = import_summary_line(&facility_intel_import::ImportSummary {
        imported: 1,
        skipped: Vec::new(),
      });
      let many = import_summary_line(&facility_intel_import::ImportSummary {
        imported: 3,
        skipped: Vec::new(),
      });

      assert!(one.contains('1'));
      assert!(many.contains('3'));
    }
  }

  mod fit {
    use pretty_assertions::assert_eq;

    use super::*;

    const SCAN_SAMPLE: &str = "High Power Slots\nStandup Heavy Energy Neutralizer I\nStandup Multirole Missile Launcher I\nMedium Power Slots\nStandup Target Painter I\nRig Slots\nStandup M-Set Moon Drilling Stability I\nStandup M-Set Moon Ore Grading Processor I\nService Slots\nStandup Cloning Center I\nStandup Reprocessing Facility I";

    fn rig(name: &str, type_id: i64, me: f64, te: f64) -> RigRef {
      RigRef {
        activity: RigActivity::Manufacturing,
        fee: 0.0,
        me,
        name: name.to_owned(),
        te,
        type_id,
      }
    }

    fn drilling_and_grading() -> Vec<RigRef> {
      vec![
        rig("Standup M-Set Moon Drilling Stability I", 1001, -2.0, 0.0),
        rig("Standup M-Set Moon Ore Grading Processor I", 1002, -2.0, 0.0),
      ]
    }

    fn anchored_card() -> IntelCard {
      let mut facility = facility(1_021_000_000_001);
      facility.type_label = Some("Raitaru".to_owned());
      facility.name = "Build Wing".to_owned();
      facility.solar_system = String::new();
      IntelCard {
        eft: None,
        facility,
        owner: None,
        rigs: [None; RIG_SLOTS],
      }
    }

    #[tokio::test]
    async fn it_applies_a_pasted_scan_and_stores_a_facility_anchored_eft() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(anchored_card());
      state.rigs = drilling_and_grading();
      state.fit = Some(FitDraft {
        content: text_editor::Content::with_text(SCAN_SAMPLE),
        facility_id: 1_021_000_000_001,
        facility_name: "Build Wing".to_owned(),
        structure_name: "Raitaru".to_owned(),
      });

      let _ = update(&mut state, Message::FitApplied, &mut settings);

      assert_eq!(state.intel[0].rigs, [Some(1001), Some(1002), None]);
      let eft = state.intel[0].eft.clone().expect("an eft is stored");
      assert!(eft.starts_with("[Raitaru, Build Wing]"), "eft was {eft}");
      assert!(eft.contains("Standup Heavy Energy Neutralizer I"));
      assert!(eft.contains("Standup Cloning Center I"));
      assert!(!eft.contains("Slots"));
      assert!(state.fit.is_none(), "applying closes the modal");
    }

    #[tokio::test]
    async fn it_stores_a_pasted_fit_even_when_no_rig_is_recognized() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(anchored_card());
      state.rigs = drilling_and_grading();
      state.fit = Some(FitDraft {
        content: text_editor::Content::with_text("Standup Cloning Center I"),
        facility_id: 1_021_000_000_001,
        facility_name: "Build Wing".to_owned(),
        structure_name: "Raitaru".to_owned(),
      });

      let _ = update(&mut state, Message::FitApplied, &mut settings);

      assert_eq!(state.intel[0].rigs, [None; RIG_SLOTS]);
      let eft = state.intel[0]
        .eft
        .clone()
        .expect("the pasted fit is stored even with no recognized rigs");
      assert!(eft.starts_with("[Raitaru, Build Wing]"), "eft was {eft}");
      assert!(eft.contains("Standup Cloning Center I"));
      assert!(state.fit.is_none(), "applying closes the modal");
    }

    #[tokio::test]
    async fn it_splices_a_manual_rig_edit_into_the_stored_eft() {
      let (mut state, mut settings) = state_with_db().await;
      let mut facility = facility(1_021_000_000_001);
      facility.type_label = Some("Sotiyo".to_owned());
      facility.name = "My Refinery".to_owned();
      facility.solar_system = String::new();
      state.intel.push(IntelCard {
        eft: Some(
          "[Sotiyo, My Refinery]\nStandup Heavy Energy Neutralizer I\n\nStandup M-Set Moon Drilling Stability I\n\nStandup Cloning Center I"
            .to_owned(),
        ),
        facility,
        owner: None,
        rigs: [Some(1001), None, None],
      });
      state.rigs = vec![
        rig("Standup M-Set Moon Drilling Stability I", 1001, -2.0, 0.0),
        rig("Standup L-Set Reaction Efficiency I", 1004, 0.0, -5.0),
      ];
      state.open_rig = Some(OpenRig {
        facility_id: 1_021_000_000_001,
        search: RigSearch::default(),
        slot: 0,
      });

      let _ = update(
        &mut state,
        Message::RigPicked {
          facility_id: 1_021_000_000_001,
          rig: Box::new(rig("Standup L-Set Reaction Efficiency I", 1004, 0.0, -5.0)),
          slot: 0,
        },
        &mut settings,
      );

      let eft = state.intel[0].eft.clone().expect("an eft is stored");
      assert!(eft.contains("Standup L-Set Reaction Efficiency I"));
      assert!(!eft.contains("Standup M-Set Moon Drilling Stability I"));
      assert!(
        eft.contains("Standup Heavy Energy Neutralizer I"),
        "service/weapon lines survive"
      );
      assert!(eft.contains("Standup Cloning Center I"));
      assert!(eft.starts_with("[Sotiyo, My Refinery]"));
    }

    #[tokio::test]
    async fn it_synthesizes_a_rigs_only_eft_on_a_never_pasted_facility() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(anchored_card());
      state.rigs = drilling_and_grading();
      state.open_rig = Some(OpenRig {
        facility_id: 1_021_000_000_001,
        search: RigSearch::default(),
        slot: 0,
      });

      let _ = update(
        &mut state,
        Message::RigPicked {
          facility_id: 1_021_000_000_001,
          rig: Box::new(rig("Standup M-Set Moon Drilling Stability I", 1001, -2.0, 0.0)),
          slot: 0,
        },
        &mut settings,
      );

      let eft = state.intel[0].eft.clone().expect("an eft is synthesized");
      assert!(eft.starts_with("[Raitaru, Build Wing]"));
      assert!(eft.contains("Standup M-Set Moon Drilling Stability I"));
    }

    #[tokio::test]
    async fn it_opens_and_closes_the_fit_modal() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(anchored_card());

      let _ = update(
        &mut state,
        Message::FitOpened {
          facility_id: 1_021_000_000_001,
        },
        &mut settings,
      );
      assert!(state.fit.is_some());
      assert_eq!(escape_dismiss(&state), Some(Message::FitClosed));

      let _ = update(&mut state, Message::FitClosed, &mut settings);
      assert!(state.fit.is_none());
    }

    #[test]
    fn it_counts_every_note_condition() {
      let parsed = facility_intel_fit::ParsedFit {
        eft: String::new(),
        hull: Some("Astrahus".to_owned()),
        overflow: 1,
        rigs: vec![1001],
        unknown: vec!["Standup M-Set Widget I".to_owned()],
      };
      let draft = FitDraft {
        content: text_editor::Content::new(),
        facility_id: 1,
        facility_name: "Post".to_owned(),
        structure_name: "Raitaru".to_owned(),
      };

      assert_eq!(fit_notes(&parsed, &draft, true).len(), 4);
    }

    #[test]
    fn it_renders_no_notes_for_a_clean_matching_fit() {
      let parsed = facility_intel_fit::ParsedFit {
        eft: String::new(),
        hull: Some("Raitaru".to_owned()),
        overflow: 0,
        rigs: vec![1001],
        unknown: Vec::new(),
      };
      let draft = FitDraft {
        content: text_editor::Content::new(),
        facility_id: 1,
        facility_name: "Post".to_owned(),
        structure_name: "Raitaru".to_owned(),
      };

      assert!(fit_notes(&parsed, &draft, false).is_empty());
    }

    #[test]
    fn it_labels_the_rig_count_singular_and_plural() {
      assert!(rig_count_label(1).contains('1'));
      assert!(rig_count_label(2).contains('2'));
    }

    #[test]
    fn it_loosely_normalises_hull_and_structure_names() {
      assert_eq!(norm_loose("M-Set"), "mset");
      assert_eq!(norm_loose("Raitaru "), norm_loose("raitaru"));
    }

    #[test]
    fn it_flags_me_and_te_rigs_from_the_catalog() {
      let state = State {
        rigs: vec![
          rig("Standup M-Set ME I", 1, -2.0, 0.0),
          rig("Standup M-Set TE I", 2, 0.0, -5.0),
        ],
        ..Default::default()
      };

      assert!(rig_is_me(&state, 1));
      assert!(!rig_is_me(&state, 2));
    }

    #[tokio::test]
    async fn it_renders_the_open_fit_modal_with_a_preview() {
      let (mut state, settings) = state_with_db().await;
      state.intel.push(anchored_card());
      state.rigs = drilling_and_grading();
      state.fit = Some(FitDraft {
        content: text_editor::Content::with_text(SCAN_SAMPLE),
        facility_id: 1_021_000_000_001,
        facility_name: "Build Wing".to_owned(),
        structure_name: "Raitaru".to_owned(),
      });

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }

  mod view {
    use super::*;

    #[tokio::test]
    async fn it_renders_the_panel_with_an_intel_card() {
      let (mut state, settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: Some("Owner Corp".to_owned()),
        rigs: [Some(37_180), None, None],
      });
      state.rigs.push(RigRef {
        activity: RigActivity::Manufacturing,
        fee: 0.0,
        me: -2.0,
        name: "Standup M-Set ME I".to_owned(),
        te: 0.0,
        type_id: 37_180,
      });

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_grid_and_sort_control_with_several_facilities() {
      let (mut state, settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: None,
        rigs: [Some(37_180), None, None],
      });
      state.sort_open = true;

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_empty_intel_state() {
      let (state, settings) = state_with_db().await;

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_open_export_modal() {
      let (mut state, settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(1_021_000_000_001),
        owner: None,
        rigs: [Some(37_180), None, None],
      });
      state.export = Some(ExportDraft {
        selected: BTreeSet::from([1_021_000_000_001]),
      });

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_export_modal_with_nothing_selected() {
      let (mut state, settings) = state_with_db().await;
      state.intel.push(IntelCard {
        eft: None,
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
      state.export = Some(ExportDraft::default());

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_open_composer() {
      let (mut state, settings) = state_with_db().await;
      state.composer.open = true;

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_import_failure_dialog() {
      let (mut state, settings) = state_with_db().await;
      state.import_error = Some(facility_intel_share::ParseError::NotAPack);

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_import_result_modal_with_skipped_structures() {
      let (mut state, settings) = state_with_db().await;
      state.import_result = Some(facility_intel_import::ImportSummary {
        imported: 2,
        skipped: vec!["Allied Fortizar \u{b7} Jita".to_owned()],
      });

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_import_result_modal_with_nothing_skipped() {
      let (mut state, settings) = state_with_db().await;
      state.import_result = Some(facility_intel_import::ImportSummary {
        imported: 1,
        skipped: Vec::new(),
      });

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
