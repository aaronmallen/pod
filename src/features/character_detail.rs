mod header;
mod tabs;

use iced::{
  Element, Length, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Stack, container, text},
};

pub use self::tabs::Tab;
use self::tabs::{
  killlog::{KillLogEntry, KilllogFilter},
  notifications::NotificationsFilter,
};
use crate::{
  config::Feature,
  store::{
    Database, images,
    model::{
      CharacterNotification, CharacterStanding, CharacterState, OwnerType, character_clone_view::CharacterClones,
      character_contacts_view::CharacterContacts,
    },
    repo::{character, infra, org, sde},
  },
  sync::JobKind,
  ui::{
    components::{backdrop, positioned_dropdown::positioned_dropdown},
    style::{color, spacing, typography},
  },
};

const HEADER_SIDE_PADDING: f32 = 28.0;
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetailDataType {
  Clones,
  Contacts,
  Killlog,
  Notifications,
  Standings,
}

impl DetailDataType {
  pub fn for_job_kind(kind: JobKind) -> Option<Self> {
    match kind {
      JobKind::CharacterClones => Some(Self::Clones),
      JobKind::CharacterContacts => Some(Self::Contacts),
      JobKind::CharacterKillmails => Some(Self::Killlog),
      JobKind::CharacterNotifications => Some(Self::Notifications),
      JobKind::CharacterStandings => Some(Self::Standings),
      JobKind::AssetSync
      | JobKind::CharacterAbyssals
      | JobKind::CharacterContracts
      | JobKind::CharacterMail
      | JobKind::CharacterMarketOrders
      | JobKind::CharacterProfile
      | JobKind::CharacterSkills
      | JobKind::CharacterTelemetry
      | JobKind::CharacterWallet
      | JobKind::CorporationProfile
      | JobKind::CorporationWallet
      | JobKind::KillmailReconcile
      | JobKind::MarketPrices
      | JobKind::NetWorthSnapshot => None,
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct HeadStats {
  pub docked: bool,
  pub liquid_isk: Option<f64>,
  pub location: Option<String>,
  pub sec_status: Option<f64>,
  pub total_sp: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Loaded {
  pub clones: LoadState<Option<CharacterClones>>,
  pub contacts: LoadState<CharacterContacts>,
  pub granted_scopes: Option<String>,
  pub head: HeadStats,
  pub killlog: LoadState<Vec<KillLogEntry>>,
  pub notifications: LoadState<Vec<CharacterNotification>>,
  pub roster: Vec<PickerPilot>,
  pub standings: LoadState<Vec<CharacterStanding>>,
}

#[derive(Clone, Debug)]
pub enum LoadState<T> {
  Error(String),
  Loaded(T),
  Loading,
}

#[derive(Clone, Debug)]
pub enum Message {
  CharacterChanged(i64),
  ContactFilterChanged(tabs::contacts::ContactFilter),
  ContactSortChanged(tabs::contacts::ContactSort),
  KilllogFilterChanged(KilllogFilter),
  Loaded(Box<Loaded>),
  NotificationRead(i64),
  NotificationsFilterChanged(NotificationsFilter),
  PickerToggled,
  #[allow(dead_code)]
  ReauthRequested(i64),
  Reloaded(Box<Reloaded>),
  TabChanged(Tab),
}

#[derive(Clone, Debug)]
pub struct PickerPilot {
  pub corp: String,
  #[allow(dead_code)]
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub name: String,
  pub portrait: images::ImageState,
  pub total_sp: i64,
}

#[derive(Clone, Debug)]
pub enum Reloaded {
  Clones(LoadState<Option<CharacterClones>>),
  Contacts(LoadState<CharacterContacts>),
  Killlog(LoadState<Vec<KillLogEntry>>),
  Notifications(LoadState<Vec<CharacterNotification>>),
  Standings(LoadState<Vec<CharacterStanding>>),
}

#[derive(Debug)]
pub struct State {
  active: i64,
  active_tab: Tab,
  clones: LoadState<Option<CharacterClones>>,
  contacts: LoadState<CharacterContacts>,
  contact_filter: tabs::contacts::ContactFilter,
  contact_sort: tabs::contacts::ContactSort,
  enabled_tabs: Vec<Tab>,
  granted_scopes: Option<String>,
  head: HeadStats,
  killlog: LoadState<Vec<KillLogEntry>>,
  killlog_filter: KilllogFilter,
  notifications: LoadState<Vec<CharacterNotification>>,
  notifications_filter: NotificationsFilter,
  picker_open: bool,
  roster: Vec<PickerPilot>,
  standings: LoadState<Vec<CharacterStanding>>,
}

impl State {
  pub fn new(active: i64, features: &[Feature]) -> Self {
    let enabled_tabs = tabs::enabled_tabs(features);
    let active_tab = tabs::resolve_first_tab(&enabled_tabs);
    State {
      active,
      active_tab,
      clones: LoadState::Loading,
      contacts: LoadState::Loading,
      contact_filter: tabs::contacts::ContactFilter::All,
      contact_sort: tabs::contacts::ContactSort::default(),
      enabled_tabs,
      granted_scopes: None,
      head: HeadStats::default(),
      killlog: LoadState::Loading,
      killlog_filter: KilllogFilter::All,
      notifications: LoadState::Loading,
      notifications_filter: NotificationsFilter::All,
      picker_open: false,
      roster: Vec::new(),
      standings: LoadState::Loading,
    }
  }

  pub fn active(&self) -> i64 {
    self.active
  }

  #[cfg(test)]
  pub fn enabled_tabs(&self) -> &[Tab] {
    &self.enabled_tabs
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .roster
      .iter()
      .filter_map(|pilot| pilot.portrait.stale_key())
      .collect()
  }

  pub fn sync_features(&mut self, features: &[Feature]) {
    self.enabled_tabs = tabs::enabled_tabs(features);
    if !self.enabled_tabs.contains(&self.active_tab) {
      self.active_tab = tabs::resolve_first_tab(&self.enabled_tabs);
    }
  }

  pub(super) fn active_name(&self) -> &str {
    self
      .roster
      .iter()
      .find(|pilot| pilot.id == self.active)
      .map_or("", |pilot| pilot.name.as_str())
  }

  pub(super) fn granted_scopes(&self) -> Option<&str> {
    self.granted_scopes.as_deref()
  }
}

pub fn load(db: &Database, character_id: i64, owned: Vec<i64>) -> Task<Message> {
  Task::perform(load_detail(db.clone(), character_id, owned), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

pub fn reload(db: &Database, character_id: i64, data_type: DetailDataType) -> Task<Message> {
  Task::perform(reload_type(db.clone(), character_id, data_type), |reloaded| {
    Message::Reloaded(Box::new(reloaded))
  })
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::CharacterChanged(id) => {
      state.active = id;
      state.picker_open = false;
      Task::none()
    }
    Message::ContactFilterChanged(filter) => {
      state.contact_filter = filter;
      Task::none()
    }
    Message::ContactSortChanged(sort) => {
      state.contact_sort = sort;
      Task::none()
    }
    Message::KilllogFilterChanged(filter) => {
      state.killlog_filter = filter;
      Task::none()
    }
    Message::NotificationsFilterChanged(filter) => {
      state.notifications_filter = filter;
      Task::none()
    }
    Message::NotificationRead(notification_id) => Task::perform(
      mark_notification_read(db.clone(), state.active, notification_id),
      |reloaded| Message::Reloaded(Box::new(reloaded)),
    ),
    Message::Loaded(loaded) => {
      let Loaded {
        clones,
        contacts,
        granted_scopes,
        head,
        killlog,
        notifications,
        roster,
        standings,
      } = *loaded;
      state.clones = clones;
      state.contacts = contacts;
      state.granted_scopes = granted_scopes;
      state.head = head;
      state.killlog = killlog;
      state.notifications = notifications;
      state.roster = roster;
      state.standings = standings;
      Task::none()
    }
    Message::Reloaded(reloaded) => {
      match *reloaded {
        Reloaded::Clones(clones) => state.clones = clones,
        Reloaded::Contacts(contacts) => state.contacts = contacts,
        Reloaded::Killlog(killlog) => state.killlog = killlog,
        Reloaded::Notifications(notifications) => state.notifications = notifications,
        Reloaded::Standings(standings) => state.standings = standings,
      }
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::ReauthRequested(_) => Task::none(),
    Message::TabChanged(tab) => {
      state.active_tab = tab;
      Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  if state.roster.is_empty() {
    return empty_state();
  }

  let body = Column::with_children(vec![
    header::header(state),
    tabs::tab_strip(&state.enabled_tabs, state.active_tab),
    tabs::tab_body(state),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  let base = container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  if state.picker_open {
    let dropdown = positioned_dropdown(header::picker_dropdown(state), PICKER_OVERLAY_TOP, PICKER_OVERLAY_LEFT);

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

async fn load_detail(db: Database, character_id: i64, owned: Vec<i64>) -> Loaded {
  let credentials = infra::all(&db).await.unwrap_or_default();
  let scopes_by_id: std::collections::HashMap<i64, Option<String>> = credentials
    .into_iter()
    .filter(|cred| cred.owner_type() == OwnerType::Character)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();

  let mut roster = Vec::with_capacity(owned.len());
  for id in owned {
    roster.push(picker_pilot(&db, id, scopes_by_id.get(&id).cloned().flatten()).await);
  }

  let head = load_head_stats(&db, character_id).await;

  let clones = match character::clones(&db, character_id).await {
    Ok(clones) => LoadState::Loaded(clones),
    Err(error) => LoadState::Error(error.to_string()),
  };
  let contacts = match character::contacts(&db, character_id).await {
    Ok(contacts) => LoadState::Loaded(contacts),
    Err(error) => LoadState::Error(error.to_string()),
  };
  let killlog = load_killlog(&db, character_id).await;
  let notifications = load_notifications(&db, character_id).await;
  let standings = match character::standings(&db, character_id).await {
    Ok(rows) => LoadState::Loaded(rows),
    Err(error) => LoadState::Error(error.to_string()),
  };
  let granted_scopes = load_granted_scopes(&db, character_id).await;

  Loaded {
    clones,
    contacts,
    granted_scopes,
    head,
    killlog,
    notifications,
    roster,
    standings,
  }
}

async fn load_granted_scopes(db: &Database, character_id: i64) -> Option<String> {
  infra::get(db, character_id, OwnerType::Character)
    .await
    .ok()
    .flatten()
    .and_then(|credential| credential.scopes().clone())
}

async fn load_killlog(db: &Database, character_id: i64) -> LoadState<Vec<KillLogEntry>> {
  let rows = match character::killmails(db, character_id).await {
    Ok(rows) => rows,
    Err(error) => return LoadState::Error(error.to_string()),
  };

  let mut entries = Vec::with_capacity(rows.len());
  for row in rows {
    let ship_name = sde::get_item_type(db, row.ship_type_id())
      .await
      .ok()
      .flatten()
      .map(|item| item.name().clone())
      .unwrap_or_else(|| format!("Type {}", row.ship_type_id()));

    let (system_name, system_security) = match sde::get_solar_system(db, row.system_id()).await.ok().flatten() {
      Some(system) => (Some(system.name().clone()), system.security_status()),
      None => (None, 0.0),
    };

    let victim_name = match row.victim_id() {
      Some(id) => character::get(db, id)
        .await
        .ok()
        .flatten()
        .map(|c| c.name().to_owned())
        .unwrap_or_else(|| format!("Pilot {id}")),
      None => "Unknown".to_owned(),
    };
    let victim_corp = match row.victim_corp_id() {
      Some(id) => org::get_corporation(db, id)
        .await
        .ok()
        .flatten()
        .map(|c| c.name().to_owned())
        .unwrap_or_else(|| format!("Corp {id}")),
      None => String::new(),
    };

    entries.push(KillLogEntry {
      attacker_count: row.attacker_count(),
      final_blow: row.final_blow(),
      is_kill: row.is_kill(),
      kill_time: row.kill_time().clone(),
      killmail_id: row.killmail_id(),
      ship_name,
      ship_type_id: row.ship_type_id(),
      system_name,
      system_security,
      value_destroyed_isk: row.value_destroyed_isk(),
      value_isk: row.value_isk(),
      victim_corp,
      victim_name,
    });
  }

  LoadState::Loaded(entries)
}

async fn load_notifications(db: &Database, character_id: i64) -> LoadState<Vec<CharacterNotification>> {
  match character::notifications(db, character_id).await {
    Ok(rows) => LoadState::Loaded(rows),
    Err(error) => LoadState::Error(error.to_string()),
  }
}

async fn mark_notification_read(db: Database, character_id: i64, notification_id: i64) -> Reloaded {
  if let Err(error) = character::mark_read(&db, character_id, notification_id).await {
    return Reloaded::Notifications(LoadState::Error(error.to_string()));
  }
  Reloaded::Notifications(load_notifications(&db, character_id).await)
}

async fn reload_type(db: Database, character_id: i64, data_type: DetailDataType) -> Reloaded {
  match data_type {
    DetailDataType::Clones => Reloaded::Clones(match character::clones(&db, character_id).await {
      Ok(clones) => LoadState::Loaded(clones),
      Err(error) => LoadState::Error(error.to_string()),
    }),
    DetailDataType::Contacts => Reloaded::Contacts(match character::contacts(&db, character_id).await {
      Ok(contacts) => LoadState::Loaded(contacts),
      Err(error) => LoadState::Error(error.to_string()),
    }),
    DetailDataType::Killlog => Reloaded::Killlog(load_killlog(&db, character_id).await),
    DetailDataType::Notifications => Reloaded::Notifications(load_notifications(&db, character_id).await),
    DetailDataType::Standings => Reloaded::Standings(match character::standings(&db, character_id).await {
      Ok(rows) => LoadState::Loaded(rows),
      Err(error) => LoadState::Error(error.to_string()),
    }),
  }
}

async fn load_head_stats(db: &Database, character_id: i64) -> HeadStats {
  let state: Option<CharacterState> = character::state(db, character_id).await.ok().flatten();
  let sec_status = character::get(db, character_id)
    .await
    .ok()
    .flatten()
    .and_then(|character| character.security_status());

  let Some(state) = state else {
    return HeadStats {
      sec_status,
      ..HeadStats::default()
    };
  };

  let location = match state.solar_system_id {
    Some(system_id) => sde::get_solar_system(db, system_id)
      .await
      .ok()
      .flatten()
      .map(|system| system.name().clone()),
    None => None,
  };
  let docked = state.station_id.is_some() || state.structure_id.is_some();

  HeadStats {
    docked,
    liquid_isk: state.wallet_balance,
    location,
    sec_status,
    total_sp: state.total_sp,
  }
}

async fn picker_pilot(db: &Database, id: i64, granted_scopes: Option<String>) -> PickerPilot {
  let character = character::get(db, id).await.ok().flatten();
  let name = character.as_ref().map(|c| c.name().to_owned()).unwrap_or_default();
  let corp = match character.as_ref().map(|c| c.corporation_id()) {
    Some(corp_id) => org::get_corporation(db, corp_id)
      .await
      .ok()
      .flatten()
      .map(|c| c.ticker().to_owned())
      .unwrap_or_default(),
    None => String::new(),
  };
  let total_sp = character::state(db, id)
    .await
    .ok()
    .flatten()
    .and_then(|state| state.total_sp)
    .unwrap_or(0);
  let portrait = images::resolve(&images::default_store(), images::ImageKind::CharacterPortrait, id);

  PickerPilot {
    corp,
    granted_scopes,
    id,
    name,
    portrait,
    total_sp,
  }
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    text("Select a character to view details")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

pub(super) fn fmt_isk(balance: Option<f64>) -> String {
  let Some(value) = balance else {
    return "\u{2014}".to_owned();
  };
  let magnitude = value.abs();
  if magnitude >= 1e9 {
    format!("{:.2}B", value / 1e9)
  } else if magnitude >= 1e6 {
    format!("{:.1}M", value / 1e6)
  } else if magnitude >= 1e3 {
    format!("{:.1}K", value / 1e3)
  } else {
    format!("{value:.0}")
  }
}

pub(super) fn fmt_sp(total: Option<i64>) -> String {
  match total {
    None | Some(0) => "\u{2014}".to_owned(),
    Some(value) => {
      let n = value as f64;
      if n >= 1e6 {
        format!("{:.1}M", n / 1e6)
      } else if n >= 1e3 {
        format!("{:.0}K", n / 1e3)
      } else {
        value.to_string()
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{
      Alliance, Bloodline, Character, CharacterKillEntry, Constellation, Corporation, Gender, Race, Region, SolarSystem,
    },
  };

  async fn seed_character(db: &Database, id: i64, name: &str, sec_status: Option<f64>) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let mut character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
    if let Some(value) = sec_status {
      character.set_security_status(value);
    }
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_telemetry(db: &Database, character_id: i64, station_id: Option<i64>) {
    sqlx::query(
      "INSERT INTO character_telemetry (character_id, online, solar_system_id, station_id, synced_at) \
      VALUES (?, ?, ?, ?, ?)",
    )
    .bind(character_id)
    .bind(true)
    .bind(30_000_142)
    .bind(station_id)
    .bind(1_700_000_000_i64)
    .execute(&db.0)
    .await
    .unwrap();
  }

  async fn seed_solar_system(db: &Database, system_id: i64, name: &str) {
    let region = Region {
      description: None,
      id: 10_000_001,
      name: "Test Region".to_owned(),
    };
    let constellation = Constellation {
      id: 20_000_001,
      name: "Test Constellation".to_owned(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      region_id: region.id,
    };
    let system = SolarSystem {
      constellation_id: constellation.id,
      id: system_id,
      name: name.to_owned(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      security_class: None,
      security_status: 0.9,
      star_id: None,
    };
    sde::upsert_region(db, &region).await.unwrap();
    sde::upsert_constellation(db, &constellation).await.unwrap();
    sde::upsert_solar_system(db, &system).await.unwrap();
  }

  fn kill_entry(
    character_id: i64,
    killmail_id: i64,
    victim_id: Option<i64>,
    victim_corp_id: Option<i64>,
  ) -> CharacterKillEntry {
    CharacterKillEntry {
      attacker_count: 3,
      character_id,
      final_blow: true,
      is_kill: true,
      kill_hash: "abc123".to_owned(),
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id,
      ship_type_id: 587,
      synced_at: "2024-01-02T00:00:00Z".to_owned(),
      system_id: 30_000_142,
      value_destroyed_isk: 0.0,
      value_final: false,
      value_isk: 1234.5,
      value_recheck_count: 0,
      value_source: "zkill".to_owned(),
      victim_corp_id,
      victim_id,
    }
  }

  fn pilot(id: i64, name: &str) -> PickerPilot {
    PickerPilot {
      corp: "TEST".to_owned(),
      granted_scopes: None,
      id,
      name: name.to_owned(),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
      total_sp: 47_320_400,
    }
  }

  fn standing(from_id: i64, from_type: &str, value: f64) -> CharacterStanding {
    CharacterStanding {
      character_id: 42,
      from_id,
      from_name: format!("Entity {from_id}"),
      from_type: from_type.to_owned(),
      standing: value,
    }
  }

  fn all_character_scopes() -> String {
    use crate::clients::esi::scopes;
    [
      scopes::CHARACTER_CLONES,
      scopes::CHARACTER_CONTACTS,
      scopes::CHARACTER_KILLMAILS,
      scopes::CHARACTER_NOTIFICATIONS,
      scopes::CHARACTER_STANDINGS,
    ]
    .join(" ")
  }

  fn loaded_state(active: i64) -> State {
    let mut state = State::new(active, &Feature::ALL);
    state.granted_scopes = Some(all_character_scopes());
    state.roster = vec![pilot(active, "Test Pilot"), pilot(7, "Wingmate")];
    state.standings = LoadState::Loaded(vec![
      standing(500_001, "faction", 5.0),
      standing(1_000_125, "npc_corp", -2.5),
    ]);
    state.head = HeadStats {
      docked: true,
      liquid_isk: Some(1_234_567_890.0),
      location: Some("Jita".to_owned()),
      sec_status: Some(4.8),
      total_sp: Some(47_320_400),
    };
    state
  }

  mod fmt_isk {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_billions_with_two_decimals() {
      assert_eq!(fmt_isk(Some(1_234_567_890.0)), "1.23B");
    }

    #[test]
    fn it_renders_millions_with_one_decimal() {
      assert_eq!(fmt_isk(Some(2_500_000.0)), "2.5M");
    }

    #[test]
    fn it_renders_an_em_dash_for_none() {
      assert_eq!(fmt_isk(None), "\u{2014}");
    }
  }

  mod fmt_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_millions_with_one_decimal() {
      assert_eq!(fmt_sp(Some(47_320_400)), "47.3M");
    }

    #[test]
    fn it_renders_an_em_dash_for_none_or_zero() {
      assert_eq!(fmt_sp(None), "\u{2014}");
      assert_eq!(fmt_sp(Some(0)), "\u{2014}");
    }
  }

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_selects_the_first_enabled_tab_on_open() {
      let state = State::new(42, &Feature::ALL);

      assert_eq!(state.active_tab, Tab::Clones);
      assert_eq!(state.active(), 42);
    }

    #[test]
    fn it_falls_back_to_clones_when_no_gated_tab_is_enabled() {
      let state = State::new(42, &[]);

      assert_eq!(state.active_tab, Tab::Clones);
    }

    #[test]
    fn sync_features_rebuilds_the_enabled_tab_strip() {
      let mut state = State::new(42, &Feature::ALL);

      state.sync_features(&[Feature::Standings]);

      assert_eq!(state.enabled_tabs, vec![Tab::Standings]);
    }

    #[test]
    fn sync_features_reselects_the_active_tab_when_it_is_disabled() {
      let mut state = State::new(42, &Feature::ALL);
      state.active_tab = Tab::Standings;

      state.sync_features(&[Feature::CombatLog]);

      assert_eq!(
        state.active_tab,
        Tab::Killlog,
        "disabling the active tab's feature re-resolves to the first remaining tab"
      );
    }

    #[test]
    fn sync_features_keeps_a_still_enabled_active_tab() {
      let mut state = State::new(42, &Feature::ALL);
      state.active_tab = Tab::Standings;

      state.sync_features(&[Feature::CloneMonitoring, Feature::Standings]);

      assert_eq!(state.active_tab, Tab::Standings);
    }

    #[test]
    fn it_surfaces_stale_roster_portraits_as_image_keys() {
      let mut state = State::new(42, &[]);
      state.roster = vec![pilot(42, "Test Pilot"), pilot(7, "Wingmate")];

      let stale = state.stale_images();

      assert_eq!(
        stale,
        vec![
          (images::ImageKind::CharacterPortrait, 42),
          (images::ImageKind::CharacterPortrait, 7),
        ]
      );
    }

    #[test]
    fn it_omits_fresh_roster_portraits_from_the_stale_keys() {
      let mut state = State::new(42, &[]);
      let mut fresh = pilot(42, "Test Pilot");
      fresh.portrait = images::ImageState::Fresh(std::path::PathBuf::from("/tmp/42.jpg"));
      state.roster = vec![fresh];

      assert!(state.stale_images().is_empty());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_records_the_active_pilot_and_closes_the_picker_on_a_switch() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);
      state.picker_open = true;

      let _ = update(&mut state, Message::CharacterChanged(7), &db);

      assert_eq!(state.active, 7);
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::TabChanged(Tab::Standings), &db);

      assert_eq!(state.active_tab, Tab::Standings);
    }

    #[tokio::test]
    async fn it_treats_a_reauth_request_as_a_noop_for_the_app_shell_to_intercept() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::ReauthRequested(42), &db);

      assert_eq!(state.active(), 42);
    }

    #[tokio::test]
    async fn it_toggles_the_picker_dropdown() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_empty_state_with_no_roster() {
      let state = State::new(42, &Feature::ALL);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_loaded_detail_with_the_standings_tab() {
      let mut state = loaded_state(42);
      state.active_tab = Tab::Standings;

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_an_empty_tab_body_for_a_not_yet_implemented_tab() {
      let mut state = loaded_state(42);
      state.active_tab = Tab::Clones;

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_with_the_picker_dropdown_open() {
      let mut state = loaded_state(42);
      state.picker_open = true;

      let _el: Element<'_, Message> = view(&state);
    }
  }

  mod data_type_mapping {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_rendered_kind_to_its_detail_type() {
      assert_eq!(
        DetailDataType::for_job_kind(JobKind::CharacterClones),
        Some(DetailDataType::Clones)
      );
      assert_eq!(
        DetailDataType::for_job_kind(JobKind::CharacterContacts),
        Some(DetailDataType::Contacts)
      );
      assert_eq!(
        DetailDataType::for_job_kind(JobKind::CharacterKillmails),
        Some(DetailDataType::Killlog)
      );
      assert_eq!(
        DetailDataType::for_job_kind(JobKind::CharacterNotifications),
        Some(DetailDataType::Notifications)
      );
      assert_eq!(
        DetailDataType::for_job_kind(JobKind::CharacterStandings),
        Some(DetailDataType::Standings)
      );
    }

    #[test]
    fn it_maps_unrendered_kinds_to_none() {
      for kind in [
        JobKind::AssetSync,
        JobKind::CharacterAbyssals,
        JobKind::CharacterMarketOrders,
        JobKind::CharacterProfile,
        JobKind::CharacterSkills,
        JobKind::CharacterTelemetry,
        JobKind::CharacterWallet,
        JobKind::CorporationProfile,
        JobKind::CorporationWallet,
        JobKind::MarketPrices,
        JobKind::NetWorthSnapshot,
      ] {
        assert_eq!(
          DetailDataType::for_job_kind(kind),
          None,
          "{kind:?} should not reload the detail"
        );
      }
    }
  }

  mod reload_arm {
    use super::*;

    fn contacts() -> CharacterContacts {
      CharacterContacts {
        contacts: Vec::new(),
        labels: Vec::new(),
      }
    }

    #[tokio::test]
    async fn it_replaces_only_the_reloaded_field_and_leaves_the_rest() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      assert!(matches!(state.standings, LoadState::Loaded(ref rows) if rows.len() == 2));
      assert!(matches!(state.clones, LoadState::Loading));

      let _ = update(
        &mut state,
        Message::Reloaded(Box::new(Reloaded::Clones(LoadState::Loaded(None)))),
        &db,
      );

      assert!(matches!(state.clones, LoadState::Loaded(None)), "clones replaced");
      assert!(
        matches!(state.standings, LoadState::Loaded(ref rows) if rows.len() == 2),
        "standings left intact"
      );
    }

    #[tokio::test]
    async fn it_replaces_a_contacts_reload_without_touching_standings() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);

      let _ = update(
        &mut state,
        Message::Reloaded(Box::new(Reloaded::Contacts(LoadState::Loaded(contacts())))),
        &db,
      );

      assert!(matches!(state.contacts, LoadState::Loaded(_)), "contacts replaced");
      assert!(
        matches!(state.standings, LoadState::Loaded(ref rows) if rows.len() == 2),
        "standings left intact"
      );
    }

    #[tokio::test]
    async fn it_replaces_a_killlog_reload_without_touching_standings() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);

      let _ = update(
        &mut state,
        Message::Reloaded(Box::new(Reloaded::Killlog(LoadState::Loaded(Vec::new())))),
        &db,
      );

      assert!(matches!(state.killlog, LoadState::Loaded(_)), "killlog replaced");
      assert!(
        matches!(state.standings, LoadState::Loaded(ref rows) if rows.len() == 2),
        "standings left intact"
      );
    }

    #[tokio::test]
    async fn it_replaces_a_notifications_reload() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);

      let _ = update(
        &mut state,
        Message::Reloaded(Box::new(Reloaded::Notifications(LoadState::Loaded(Vec::new())))),
        &db,
      );

      assert!(
        matches!(state.notifications, LoadState::Loaded(_)),
        "notifications replaced"
      );
    }

    #[tokio::test]
    async fn it_replaces_a_standings_reload() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);

      let _ = update(
        &mut state,
        Message::Reloaded(Box::new(Reloaded::Standings(LoadState::Loaded(Vec::new())))),
        &db,
      );

      assert!(
        matches!(state.standings, LoadState::Loaded(ref rows) if rows.is_empty()),
        "standings replaced"
      );
    }
  }

  mod update_filters {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_records_the_contact_filter() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(
        &mut state,
        Message::ContactFilterChanged(tabs::contacts::ContactFilter::Character),
        &db,
      );

      assert_eq!(state.contact_filter, tabs::contacts::ContactFilter::Character);
    }

    #[tokio::test]
    async fn it_records_the_contact_sort() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);
      let sort = tabs::contacts::ContactSort::default().toggled(tabs::contacts::SortColumn::Entity);

      let _ = update(&mut state, Message::ContactSortChanged(sort), &db);

      assert_eq!(state.contact_sort, sort);
    }

    #[tokio::test]
    async fn it_records_the_killlog_filter() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::KilllogFilterChanged(KilllogFilter::Kills), &db);

      assert_eq!(state.killlog_filter, KilllogFilter::Kills);
    }

    #[tokio::test]
    async fn it_records_the_notifications_filter() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(
        &mut state,
        Message::NotificationsFilterChanged(NotificationsFilter::Unread),
        &db,
      );

      assert_eq!(state.notifications_filter, NotificationsFilter::Unread);
    }

    #[tokio::test]
    async fn it_marks_a_notification_read_and_updates_the_unread_count() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      let mut unread = CharacterNotification {
        character_id: 42,
        is_read: false,
        notif_type: "KillReportFinalBlow".to_owned(),
        notification_id: 99,
        sender_id: Some(1001),
        sender_type: Some("character".to_owned()),
        synced_at: "2024-01-02T00:00:00Z".to_owned(),
        text: Some("body".to_owned()),
        timestamp: "2024-01-01T00:00:00Z".to_owned(),
      };
      character::upsert_notification(&db, &unread).await.unwrap();
      unread.notification_id = 100;
      character::upsert_notification(&db, &unread).await.unwrap();

      let mut state = State::new(42, &Feature::ALL);
      state.notifications = load_notifications(&db, 42).await;
      assert_eq!(unread_count_from(&state.notifications), 2);

      let reloaded = mark_notification_read(db.clone(), 42, 99).await;
      let _ = update(&mut state, Message::Reloaded(Box::new(reloaded)), &db);

      assert_eq!(unread_count_from(&state.notifications), 1);
      let read = match &state.notifications {
        LoadState::Loaded(rows) => rows.iter().find(|n| n.notification_id() == 99).unwrap().is_read(),
        _ => panic!("expected loaded notifications"),
      };
      assert!(read);

      let reloaded = load_notifications(&db, 42).await;
      let persisted_unread = match &reloaded {
        LoadState::Loaded(rows) => tabs::notifications::unread_count(rows),
        _ => panic!("expected loaded notifications"),
      };
      assert_eq!(persisted_unread, 1);
    }

    fn unread_count_from(state: &LoadState<Vec<CharacterNotification>>) -> usize {
      match state {
        LoadState::Loaded(rows) => tabs::notifications::unread_count(rows),
        _ => panic!("expected loaded notifications"),
      }
    }

    #[tokio::test]
    async fn it_applies_a_full_load_to_every_field() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);
      let loaded = Loaded {
        clones: LoadState::Loaded(None),
        contacts: LoadState::Loaded(CharacterContacts {
          contacts: Vec::new(),
          labels: Vec::new(),
        }),
        granted_scopes: None,
        head: HeadStats {
          total_sp: Some(1_000),
          ..HeadStats::default()
        },
        killlog: LoadState::Loaded(Vec::new()),
        notifications: LoadState::Loaded(Vec::new()),
        roster: vec![pilot(42, "Pilot")],
        standings: LoadState::Loaded(Vec::new()),
      };

      let _ = update(&mut state, Message::Loaded(Box::new(loaded)), &db);

      assert_eq!(state.roster.len(), 1);
      assert_eq!(state.head.total_sp, Some(1_000));
      assert!(matches!(state.clones, LoadState::Loaded(None)));
    }
  }

  mod load_killlog {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_each_killmail_into_a_render_ready_entry() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      character::upsert_killmail(&db, &kill_entry(42, 100, Some(2002), Some(3003)))
        .await
        .unwrap();

      let LoadState::Loaded(entries) = load_killlog(&db, 42).await else {
        panic!("expected a loaded killlog");
      };

      assert_eq!(entries.len(), 1);
      assert_eq!(entries[0].killmail_id, 100);
      assert_eq!(entries[0].ship_name, "Type 587");
      assert_eq!(entries[0].system_name, None);
      assert_eq!(entries[0].victim_name, "Pilot 2002");
      assert_eq!(entries[0].victim_corp, "Corp 3003");
    }

    #[tokio::test]
    async fn it_resolves_the_real_system_name_when_the_sde_has_it() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      seed_solar_system(&db, 30_000_142, "Jita").await;
      character::upsert_killmail(&db, &kill_entry(42, 100, Some(2002), Some(3003)))
        .await
        .unwrap();

      let LoadState::Loaded(entries) = load_killlog(&db, 42).await else {
        panic!("expected a loaded killlog");
      };

      assert_eq!(entries[0].system_name, Some("Jita".to_owned()));
      assert_eq!(entries[0].system_security, 0.9);
    }

    #[tokio::test]
    async fn it_uses_unknown_placeholders_when_victim_ids_are_absent() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      character::upsert_killmail(&db, &kill_entry(42, 200, None, None))
        .await
        .unwrap();

      let LoadState::Loaded(entries) = load_killlog(&db, 42).await else {
        panic!("expected a loaded killlog");
      };

      assert_eq!(entries[0].victim_name, "Unknown");
      assert_eq!(entries[0].victim_corp, "");
    }

    #[tokio::test]
    async fn it_is_an_empty_loaded_list_when_there_are_no_killmails() {
      let db = store::open_test().await.unwrap();

      let LoadState::Loaded(entries) = load_killlog(&db, 42).await else {
        panic!("expected a loaded killlog");
      };

      assert!(entries.is_empty());
    }
  }

  mod load_head_stats {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_the_sec_status_when_no_state_row_exists() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", Some(4.8)).await;

      let head = load_head_stats(&db, 42).await;

      assert_eq!(head.sec_status, Some(4.8));
      assert!(head.location.is_none());
      assert!(!head.docked);
    }

    #[tokio::test]
    async fn it_infers_docked_from_a_present_station_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", Some(1.0)).await;
      seed_telemetry(&db, 42, Some(60_003_760)).await;

      let head = load_head_stats(&db, 42).await;

      assert!(head.docked, "a present station id means docked");
      assert_eq!(head.sec_status, Some(1.0));
    }

    #[tokio::test]
    async fn it_is_undocked_with_no_station_or_structure() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      seed_telemetry(&db, 42, None).await;

      let head = load_head_stats(&db, 42).await;

      assert!(!head.docked);
    }
  }

  mod picker_pilot {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_identity_and_corp_ticker() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Test Pilot", None).await;

      let pilot = picker_pilot(&db, 42, None).await;

      assert_eq!(pilot.id, 42);
      assert_eq!(pilot.name, "Test Pilot");
      assert_eq!(pilot.corp, "TSC");
      assert_eq!(pilot.total_sp, 0);
    }

    #[tokio::test]
    async fn it_falls_back_to_empty_fields_for_an_unknown_pilot() {
      let db = store::open_test().await.unwrap();

      let pilot = picker_pilot(&db, 999, None).await;

      assert_eq!(pilot.id, 999);
      assert_eq!(pilot.name, "");
      assert_eq!(pilot.corp, "");
      assert_eq!(pilot.total_sp, 0);
      assert!(pilot.portrait.path().is_none());
    }
  }

  mod load_detail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_loads_the_roster_head_and_every_tab() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Test Pilot", Some(2.5)).await;
      seed_telemetry(&db, 42, Some(60_003_760)).await;
      character::upsert_killmail(&db, &kill_entry(42, 100, None, None))
        .await
        .unwrap();

      let loaded = load_detail(db.clone(), 42, vec![42]).await;

      assert_eq!(loaded.roster.len(), 1);
      assert_eq!(loaded.roster[0].name, "Test Pilot");
      assert_eq!(loaded.head.sec_status, Some(2.5));
      assert!(matches!(loaded.clones, LoadState::Loaded(_)));
      assert!(matches!(loaded.contacts, LoadState::Loaded(_)));
      assert!(matches!(loaded.killlog, LoadState::Loaded(ref rows) if rows.len() == 1));
      assert!(matches!(loaded.notifications, LoadState::Loaded(_)));
      assert!(matches!(loaded.standings, LoadState::Loaded(_)));
    }

    #[tokio::test]
    async fn it_loads_with_an_empty_owned_roster() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Test Pilot", None).await;

      let loaded = load_detail(db.clone(), 42, Vec::new()).await;

      assert!(loaded.roster.is_empty());
      assert!(matches!(loaded.clones, LoadState::Loaded(_)));
    }
  }

  mod load_granted_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_credential_scopes_for_the_character() {
      let db = store::open_test().await.unwrap();
      infra::upsert(
        &db,
        42,
        OwnerType::Character,
        "tok",
        "rt",
        9999,
        None,
        Some("esi-clones.read_clones.v1 esi-characters.read_standings.v1"),
      )
      .await
      .unwrap();

      let scopes = load_granted_scopes(&db, 42).await;

      assert_eq!(
        scopes.as_deref(),
        Some("esi-clones.read_clones.v1 esi-characters.read_standings.v1")
      );
    }

    #[tokio::test]
    async fn it_is_none_when_the_character_has_no_credential() {
      let db = store::open_test().await.unwrap();

      assert!(load_granted_scopes(&db, 42).await.is_none());
    }
  }

  mod reload_type {
    use super::*;

    #[tokio::test]
    async fn it_reloads_each_detail_type() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      character::upsert_killmail(&db, &kill_entry(42, 100, None, None))
        .await
        .unwrap();

      assert!(matches!(
        reload_type(db.clone(), 42, DetailDataType::Clones).await,
        Reloaded::Clones(LoadState::Loaded(_))
      ));
      assert!(matches!(
        reload_type(db.clone(), 42, DetailDataType::Contacts).await,
        Reloaded::Contacts(LoadState::Loaded(_))
      ));
      assert!(matches!(
        reload_type(db.clone(), 42, DetailDataType::Killlog).await,
        Reloaded::Killlog(LoadState::Loaded(ref rows)) if rows.len() == 1
      ));
      assert!(matches!(
        reload_type(db.clone(), 42, DetailDataType::Notifications).await,
        Reloaded::Notifications(LoadState::Loaded(_))
      ));
      assert!(matches!(
        reload_type(db.clone(), 42, DetailDataType::Standings).await,
        Reloaded::Standings(LoadState::Loaded(_))
      ));
    }
  }
}
