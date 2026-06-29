use std::collections::HashMap;

use iced::{
  Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, scrollable, text},
};

use super::Outcome;
use crate::{
  config::Settings,
  features::industry::{
    PinnedStructure, PlannerFacility,
    facility_owner::resolve_facility_owner,
    rig_bonuses::{self, DerivedRigBonuses, RigBonus},
  },
  store::{
    Database,
    repo::{industry, sde},
  },
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      button::{Button, Size},
      facility_combobox::{self, FacilityCombobox, FacilityRef, FacilitySearch},
      icon::Icon,
      rig_combobox::{Activity as RigActivity, RigCombobox, RigRef, RigSearch, rigs_for_structure},
      rule, status,
    },
    style::{color, radius, spacing, typography},
  },
};

const COMPOSER_ACTIVITY_ID: i64 = 0;
const DB_MANUFACTURING_ACTIVITY_ID: i64 = industry::MANUFACTURING_ACTIVITY_ID;
const DB_REACTION_ACTIVITY_ID: i64 = industry::REACTION_ACTIVITY_ID;
const MANUFACTURING_ACTIVITY_ID: i64 = 1;
/// Facility ids at or above this are player-owned structures; below are NPC stations.
const MIN_STRUCTURE_ID: i64 = 1_000_000_000_000;
const PANEL_SIDE_PADDING: f32 = 36.0;
const PICKER_MAX_WIDTH: f32 = 600.0;
const REACTION_ACTIVITY_ID: i64 = 11;
const RIG_SLOTS: usize = 3;

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
  FacilityPicked {
    activity: i64,
    facility: FacilityRef,
  },
  Loaded(Box<Result<Loaded, String>>),
  PickerToggled {
    activity: i64,
  },
  QueryChanged {
    activity: i64,
    query: String,
  },
  Reload,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
  facilities_count: usize,
  intel: Vec<IntelCard>,
  manufacturing: Option<FacilityRef>,
  reactions: Option<FacilityRef>,
  rig_catalog: HashMap<i64, RigBonus>,
  rigs: Vec<RigRef>,
}

#[derive(Debug, Default)]
pub struct State {
  composer: Picker,
  db: Option<Database>,
  facilities_count: usize,
  intel: Vec<IntelCard>,
  load_error: Option<String>,
  manufacturing: Picker,
  open_rig: Option<OpenRig>,
  reactions: Picker,
  rig_catalog: HashMap<i64, RigBonus>,
  rigs: Vec<RigRef>,
}

impl State {
  pub fn new(db: Database) -> Self {
    State {
      db: Some(db),
      ..State::default()
    }
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

#[derive(Clone, Debug, PartialEq)]
struct IntelCard {
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

fn pin_for(facility: &FacilityRef) -> Option<PinnedStructure> {
  (facility.id >= MIN_STRUCTURE_ID).then(|| PinnedStructure {
    id: facility.id,
    name: facility.name.clone(),
    solar_system_id: facility.solar_system_id,
    type_id: facility.type_id,
  })
}

fn set_config_default(settings: &mut Settings, activity: i64, value: Option<i64>) {
  if activity == REACTION_ACTIVITY_ID {
    settings.industry_mut().set_reactions(value);
  } else {
    settings.industry_mut().set_manufacturing(value);
  }
}

pub fn load(db: &Database) -> iced::Task<Message> {
  iced::Task::perform(load_all(db.clone()), |result| Message::Loaded(Box::new(result)))
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> (Outcome, iced::Task<Message>) {
  match message {
    Message::Cleared {
      activity,
    } => clear_default(state, settings, activity),
    Message::ComposerToggled(open) => {
      state.composer.open = open;
      state.composer.search.clear();
      (Outcome::None, iced::Task::none())
    }
    Message::FacilityPicked {
      activity,
      facility,
    } => facility_picked(state, settings, activity, facility),
    Message::Loaded(result) => {
      loaded(state, *result);
      (Outcome::None, iced::Task::none())
    }
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
    Message::Reload => (Outcome::None, reload(state)),
    Message::RemoveFacility(facility_id) => remove_facility(state, facility_id),
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
  }
}

fn clear_default(state: &mut State, settings: &mut Settings, activity: i64) -> (Outcome, iced::Task<Message>) {
  let picker = state.picker_mut(activity);
  picker.open = false;
  picker.selection = None;
  picker.search.clear();
  set_config_default(settings, activity, None);
  let db_activity = db_activity(activity);
  let task = write(&state.db, move |db| async move {
    industry::clear_default_facility(&db, db_activity).await
  });
  (Outcome::Persist, task)
}

fn facility_picked(
  state: &mut State,
  settings: &mut Settings,
  activity: i64,
  facility: FacilityRef,
) -> (Outcome, iced::Task<Message>) {
  let pin = pin_for(&facility);
  if activity == COMPOSER_ACTIVITY_ID {
    state.composer.open = false;
    state.composer.search.clear();
    if state.intel.iter().all(|card| card.facility.id != facility.id) {
      state.intel.push(IntelCard {
        facility: facility.clone(),
        owner: None,
        rigs: [None; RIG_SLOTS],
      });
    }
    let facility_id = facility.id;
    let task = write(&state.db, move |db| async move {
      industry::upsert_facility_intel(&db, facility_id, None, None, None).await
    });
    let outcome = pin.map_or(Outcome::None, Outcome::IndustryPin);
    return (outcome, task);
  }

  let picker = state.picker_mut(activity);
  picker.open = false;
  picker.selection = Some(facility.clone());
  picker.search.clear();
  set_config_default(settings, activity, Some(facility.id));
  let db_activity = db_activity(activity);
  let facility_id = facility.id;
  let task = write(&state.db, move |db| async move {
    industry::set_default_facility(&db, db_activity, facility_id).await
  });
  match pin {
    Some(pin) => (Outcome::IndustryPin(pin), task),
    None => (Outcome::Persist, task),
  }
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
  let Some(card) = state.intel.iter_mut().find(|card| card.facility.id == facility_id) else {
    return (Outcome::None, iced::Task::none());
  };
  if slot < RIG_SLOTS {
    card.rigs[slot] = rig;
  }
  let rigs = card.rigs;
  let task = write(&state.db, move |db| async move {
    industry::upsert_facility_intel(&db, facility_id, rigs[0], rigs[1], rigs[2]).await
  });
  (Outcome::None, task)
}

fn loaded(state: &mut State, result: Result<Loaded, String>) {
  match result {
    Ok(payload) => {
      state.facilities_count = payload.facilities_count;
      state.intel = merge_intel(std::mem::take(&mut state.intel), payload.intel);
      state.manufacturing.selection = payload.manufacturing;
      state.reactions.selection = payload.reactions;
      state.rig_catalog = payload.rig_catalog;
      state.rigs = payload.rigs;
      state.load_error = None;
    }
    Err(error) => state.load_error = Some(error),
  }
}

/// Reloaded intel rows resolve their display facility from the accessible-facility list. A structure added
/// this session may not appear there until its pin lands, so a row the reload could not resolve keeps the
/// optimistic card already in memory rather than vanishing.
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
  match state.db.clone() {
    Some(db) => load(&db),
    None => iced::Task::none(),
  }
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

async fn load_all(db: Database) -> Result<Loaded, String> {
  let facilities = industry::accessible_facilities(&db)
    .await
    .map_err(|err| err.to_string())?;

  let manufacturing = resolve_default(&db, &facilities, DB_MANUFACTURING_ACTIVITY_ID).await;
  let reactions = resolve_default(&db, &facilities, DB_REACTION_ACTIVITY_ID).await;

  let mut intel = Vec::new();
  for row in industry::list_facility_intel(&db)
    .await
    .map_err(|err| err.to_string())?
  {
    if let Some(facility) = facility_ref_for(&db, &facilities, row.facility_id).await {
      let owner = resolve_facility_owner(&db, row.facility_id)
        .await
        .map(|owner| owner.display());
      intel.push(IntelCard {
        facility,
        owner,
        rigs: [row.rig_1_type_id, row.rig_2_type_id, row.rig_3_type_id],
      });
    }
  }

  let (rigs, rig_catalog) = load_rigs(&db).await?;

  Ok(Loaded {
    facilities_count: facilities.len(),
    intel,
    manufacturing,
    reactions,
    rig_catalog,
    rigs,
  })
}

async fn resolve_default(
  db: &Database,
  facilities: &[crate::store::model::Facility],
  db_activity: i64,
) -> Option<FacilityRef> {
  let id = industry::default_facility(db, db_activity).await.ok().flatten()?;
  facility_ref_for(db, facilities, id).await
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

  Column::with_children(vec![header, body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
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
    .style(typography::colored(color::accent::PLASMA));
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

fn intel_section(state: &State) -> Element<'_, Message> {
  let count = if state.intel.len() == 1 {
    t!("settings.facility.intel_count_one", count => state.intel.len())
  } else {
    t!("settings.facility.intel_count_other", count => state.intel.len())
  };
  let mut right = Row::new().spacing(spacing::SPACE_3).align_y(Vertical::Center);
  right = right.push(count_label(count.into_owned()));
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
  if state.intel.is_empty() && !state.composer.open {
    children.push(empty_state());
  } else {
    for card in &state.intel {
      children.push(intel_card(state, card));
    }
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
    .style(typography::colored(color::accent::PLASMA));
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
        color: color::accent::PLASMA,
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

fn card_header<'a>(card: &'a IntelCard) -> Element<'a, Message> {
  let mut title_row = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
  title_row = title_row.push(
    text(card.facility.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY)),
  );
  title_row = title_row.push(facility_combobox::type_badge(&card.facility));
  title_row = title_row.push(facility_combobox::sec_pill(card.facility.security_status));
  if !card.facility.solar_system.trim().is_empty() {
    title_row = title_row.push(
      text(card.facility.solar_system.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary())),
    );
  }

  let mut meta = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
  if let Some(region) = &card.facility.region {
    meta = meta.push(
      text(region.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }
  if let Some(owner) = &card.owner {
    meta = meta.push(status::dot_sized(color::text::tertiary(), 2.0));
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

  let remove = Button::ghost_icon(Icon::trash())
    .size(Size::Sm)
    .on_press(Message::RemoveFacility(card.facility.id));

  Row::with_children(vec![identity.into(), remove.into()])
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
    .on_dismiss(Message::RigDismissed);

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
      color::accent::PLASMA,
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
    derived_chip(
      super::i18n::tr_static("settings.facility.chip_fee"),
      derived.fee,
      1,
      color::status::WARNING,
      if derived.fee != 0.0 {
        super::i18n::tr_static("settings.facility.chip_fee_sub_on")
      } else {
        super::i18n::tr_static("settings.facility.chip_fee_sub_off")
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

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_selects_a_station_default_and_mirrors_it_to_config() {
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
      assert_eq!(*settings.industry().manufacturing(), Some(60_003_760));
    }

    #[tokio::test]
    async fn it_requests_a_pin_for_a_structure_default() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(
        &mut state,
        Message::FacilityPicked {
          activity: REACTION_ACTIVITY_ID,
          facility: facility(1_021_000_000_001),
        },
        &mut settings,
      );

      assert!(matches!(outcome, Outcome::IndustryPin(_)));
    }

    #[tokio::test]
    async fn it_clears_a_default_selection_and_config() {
      let (mut state, mut settings) = state_with_db().await;
      state.manufacturing.selection = Some(facility(60_003_760));
      settings.industry_mut().set_manufacturing(Some(60_003_760));

      let (outcome, _task) = update(
        &mut state,
        Message::Cleared {
          activity: MANUFACTURING_ACTIVITY_ID,
        },
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert!(state.manufacturing.selection.is_none());
      assert_eq!(*settings.industry().manufacturing(), None);
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
            facility: facility(60_003_760),
            owner: Some("Owner Corp".to_owned()),
            rigs: [None; RIG_SLOTS],
          }],
          manufacturing: Some(facility(60_003_760)),
          reactions: None,
          rig_catalog: HashMap::new(),
          rigs: Vec::new(),
        }))),
        &mut settings,
      );

      assert_eq!(state.facilities_count, 5);
      assert_eq!(state.manufacturing.selection.as_ref().map(|f| f.id), Some(60_003_760));
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
    async fn it_reloads_from_the_database() {
      let (mut state, mut settings) = state_with_db().await;

      let (outcome, _task) = update(&mut state, Message::Reload, &mut settings);

      assert_eq!(outcome, Outcome::None);
    }

    #[tokio::test]
    async fn it_clears_a_fitted_rig_slot() {
      let (mut state, mut settings) = state_with_db().await;
      state.intel.push(IntelCard {
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

  mod loaders {
    use super::*;

    #[tokio::test]
    async fn it_loads_an_empty_database_without_error() {
      let db = store::open_test().await.unwrap();

      let loaded = load_all(db).await.expect("load_all succeeds on a migrated database");

      assert_eq!(loaded.facilities_count, 0);
      assert!(loaded.intel.is_empty());
      assert!(loaded.manufacturing.is_none());
      assert!(loaded.reactions.is_none());
      assert!(loaded.rigs.is_empty());
      assert!(loaded.rig_catalog.is_empty());
    }
  }

  mod merge_intel {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_an_optimistic_card_the_reload_could_not_resolve() {
      let optimistic = vec![IntelCard {
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
        facility: facility(60_003_760),
        owner: None,
        rigs: [None; RIG_SLOTS],
      }];
      let reloaded = vec![IntelCard {
        facility: facility(60_003_760),
        owner: Some("Owner Corp".to_owned()),
        rigs: [Some(37_180), None, None],
      }];

      let merged = merge_intel(optimistic, reloaded);

      assert_eq!(merged.len(), 1);
      assert_eq!(merged[0].owner.as_deref(), Some("Owner Corp"));
    }
  }

  mod view {
    use super::*;

    #[tokio::test]
    async fn it_renders_the_panel_with_an_intel_card() {
      let (mut state, settings) = state_with_db().await;
      state.intel.push(IntelCard {
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
    async fn it_renders_the_empty_intel_state() {
      let (state, settings) = state_with_db().await;

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[tokio::test]
    async fn it_renders_the_open_composer() {
      let (mut state, settings) = state_with_db().await;
      state.composer.open = true;

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
