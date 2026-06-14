mod header;
mod tabs;

use std::time::Duration;

use iced::{
  Element, Length, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Stack, container, operation, text},
};

pub use self::tabs::Tab;
use self::tabs::{
  killlog::{KillLogEntry, KilllogFilter},
  notifications::NotificationsFilter,
};
pub use crate::store::repo::standings::CatalogKind as StandingKind;
use crate::{
  config::Feature,
  store::{
    Database, images,
    model::{
      CharacterNotification, CharacterState, OwnerType, character_clone_view::CharacterClones,
      character_contacts_view::CharacterContacts,
    },
    repo::{character, infra, org, sde, standings},
  },
  sync::JobKind,
  ui::{
    components::{
      backdrop,
      positioned_dropdown::{positioned_dropdown, positioned_dropdown_right},
    },
    style::{color, spacing, typography},
  },
};

pub(crate) const STANDINGS_SEARCH_INPUT_ID: &str = "standings-search-input";

const CONTACTS_PAGE_SIZE: usize = 100;
const HEADER_SIDE_PADDING: f32 = 28.0;
const KILLLOG_PAGE_SIZE: i64 = 100;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const SCROLL_THRESHOLD: f32 = 0.85;
const SEARCH_DEBOUNCE_MS: u64 = 200;
const STANDINGS_PAGE_SIZE: i64 = 100;
const STANDINGS_HELP_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + TAB_STRIP_OVERLAY_OFFSET;
const TAB_STRIP_OVERLAY_OFFSET: f32 = 96.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
      | JobKind::CharacterCalendar
      | JobKind::CharacterContracts
      | JobKind::CharacterIndustryJobs
      | JobKind::CharacterMail
      | JobKind::CharacterMarketOrders
      | JobKind::CharacterProfile
      | JobKind::CharacterSkills
      | JobKind::CharacterTelemetry
      | JobKind::CharacterWallet
      | JobKind::CorporationIndustryJobs
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
  ContactsScrolled(f32),
  KilllogFilterChanged(KilllogFilter),
  KilllogPageLoaded(Vec<KillLogEntry>),
  KilllogScrolled(f32),
  Loaded(Box<Loaded>),
  NotificationRead(i64),
  NotificationsFilterChanged(NotificationsFilter),
  PickerToggled,
  #[allow(dead_code)]
  ReauthRequested(i64),
  Reloaded(Box<Reloaded>),
  StandingsAgentsPageLoaded(Box<StandingsAgentsPage>),
  StandingsClearSearch,
  StandingsFilterChanged(tabs::standings::StandingsFilter),
  StandingsInsertQuery(String),
  StandingsResults(Box<StandingsResult>),
  StandingsScrolled(f32),
  StandingsSearchChanged(String),
  StandingsToggleHelp,
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
  /// Payload-less, unlike the other variants: a standings reload re-runs the catalog query (preserving the active
  /// search) rather than carrying rows.
  Standings,
}

#[derive(Clone, Debug)]
pub struct StandingsRow {
  pub accessible: Option<bool>,
  pub agent_type: Option<String>,
  pub division: Option<String>,
  pub effective: f64,
  pub faction_id: Option<i64>,
  pub id: i64,
  pub image: images::ImageState,
  pub kind: StandingKind,
  pub level: Option<i64>,
  pub name: String,
  pub raw: f64,
  pub region: Option<String>,
  pub system: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StandingsAgentsPage {
  generation: u64,
  next_cursor: Option<(String, i64)>,
  rows: Vec<StandingsRow>,
}

#[derive(Clone, Debug)]
pub struct StandingsResult {
  /// Snapshot of `State::standings_generation` at dispatch; results whose generation no longer matches are stale
  /// (superseded by a newer debounced search) and discarded.
  generation: u64,
  result: Result<StandingsCatalog, String>,
}

#[derive(Clone, Debug)]
pub struct StandingsCatalog {
  /// Keyset cursor for the next agent page, or `None` when the first agent page exhausted them.
  agent_cursor: Option<(String, i64)>,
  rows: Vec<StandingsRow>,
}

#[derive(Debug)]
pub struct State {
  active: i64,
  active_tab: Tab,
  clones: LoadState<Option<CharacterClones>>,
  contacts: LoadState<CharacterContacts>,
  contact_filter: tabs::contacts::ContactFilter,
  contact_sort: tabs::contacts::ContactSort,
  contacts_visible: usize,
  enabled_tabs: Vec<Tab>,
  granted_scopes: Option<String>,
  head: HeadStats,
  killlog: LoadState<Vec<KillLogEntry>>,
  killlog_cursor: Option<(String, i64)>,
  killlog_filter: KilllogFilter,
  killlog_has_more: bool,
  killlog_loading_more: bool,
  notifications: LoadState<Vec<CharacterNotification>>,
  notifications_filter: NotificationsFilter,
  picker_open: bool,
  roster: Vec<PickerPilot>,
  standings: LoadState<Vec<StandingsRow>>,
  standings_agent_cursor: Option<(String, i64)>,
  standings_filter: tabs::standings::StandingsFilter,
  standings_generation: u64,
  standings_has_more: bool,
  standings_help_open: bool,
  standings_loading_more: bool,
  standings_query: String,
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
      contacts_visible: CONTACTS_PAGE_SIZE,
      enabled_tabs,
      granted_scopes: None,
      head: HeadStats::default(),
      killlog: LoadState::Loading,
      killlog_cursor: None,
      killlog_filter: KilllogFilter::All,
      killlog_has_more: false,
      killlog_loading_more: false,
      notifications: LoadState::Loading,
      notifications_filter: NotificationsFilter::All,
      picker_open: false,
      roster: Vec::new(),
      standings: LoadState::Loading,
      standings_agent_cursor: None,
      standings_filter: tabs::standings::StandingsFilter::All,
      standings_generation: 0,
      standings_has_more: false,
      standings_help_open: false,
      standings_loading_more: false,
      standings_query: String::new(),
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
    let mut keys: Vec<(images::ImageKind, i64)> = self
      .roster
      .iter()
      .filter_map(|pilot| pilot.portrait.stale_key())
      .collect();
    if let LoadState::Loaded(rows) = &self.standings {
      keys.extend(rows.iter().filter_map(|row| row.image.stale_key()));
    }
    if let LoadState::Loaded(contacts) = &self.contacts {
      keys.extend(contacts.images.values().filter_map(images::ImageState::stale_key));
    }
    keys
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

  pub(super) fn contacts_visible(&self) -> usize {
    self.contacts_visible
  }

  pub(super) fn granted_scopes(&self) -> Option<&str> {
    self.granted_scopes.as_deref()
  }

  fn has_loaded_agents(&self) -> bool {
    matches!(&self.standings, LoadState::Loaded(rows) if rows.iter().any(|row| row.kind == StandingKind::Agent))
  }

  pub(super) fn standings_has_filters(&self) -> bool {
    !self.standings_query.trim().is_empty()
  }

  pub(super) fn standings_query(&self) -> &str {
    &self.standings_query
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
      state.contacts_visible = CONTACTS_PAGE_SIZE;
      Task::none()
    }
    Message::ContactSortChanged(sort) => {
      state.contact_sort = sort;
      Task::none()
    }
    Message::ContactsScrolled(_)
    | Message::KilllogPageLoaded(_)
    | Message::KilllogScrolled(_)
    | Message::StandingsAgentsPageLoaded(_)
    | Message::StandingsScrolled(_) => update_pagination(state, message, db),
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
      } = *loaded;
      state.clones = clones;
      state.contacts = contacts;
      state.contacts_visible = CONTACTS_PAGE_SIZE;
      state.granted_scopes = granted_scopes;
      state.head = head;
      state.killlog = killlog;
      reset_killlog_pagination(state);
      state.notifications = notifications;
      state.roster = roster;
      trigger_standings_search(state, db)
    }
    Message::Reloaded(reloaded) => match *reloaded {
      Reloaded::Clones(clones) => {
        state.clones = clones;
        Task::none()
      }
      Reloaded::Contacts(contacts) => {
        state.contacts = contacts;
        Task::none()
      }
      Reloaded::Killlog(killlog) => {
        state.killlog = killlog;
        reset_killlog_pagination(state);
        Task::none()
      }
      Reloaded::Notifications(notifications) => {
        state.notifications = notifications;
        Task::none()
      }
      Reloaded::Standings => trigger_standings_search(state, db),
    },
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::ReauthRequested(_) => Task::none(),
    Message::StandingsClearSearch => {
      state.standings_query.clear();
      Task::batch([
        trigger_standings_search(state, db),
        operation::focus(STANDINGS_SEARCH_INPUT_ID),
      ])
    }
    Message::StandingsFilterChanged(filter) => change_standings_filter(state, filter, db),
    Message::StandingsInsertQuery(fragment) => {
      append_standings_query(state, &fragment);
      state.standings_help_open = false;
      Task::batch([
        trigger_standings_search(state, db),
        operation::focus(STANDINGS_SEARCH_INPUT_ID),
      ])
    }
    Message::StandingsResults(results) => {
      let StandingsResult {
        generation,
        result,
      } = *results;
      if generation == state.standings_generation {
        state.standings = match result {
          Ok(catalog) => {
            state.standings_has_more = catalog.agent_cursor.is_some();
            state.standings_agent_cursor = catalog.agent_cursor;
            LoadState::Loaded(catalog.rows)
          }
          Err(error) => {
            state.standings_has_more = false;
            state.standings_agent_cursor = None;
            LoadState::Error(error)
          }
        };
      }
      Task::none()
    }
    Message::StandingsSearchChanged(query) => {
      state.standings_query = query;
      trigger_standings_search(state, db)
    }
    Message::StandingsToggleHelp => {
      state.standings_help_open = !state.standings_help_open;
      Task::none()
    }
    Message::TabChanged(tab) => {
      state.active_tab = tab;
      Task::none()
    }
  }
}

fn update_pagination(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ContactsScrolled(offset) => {
      if offset >= SCROLL_THRESHOLD && state.active_tab == Tab::Contacts {
        state.contacts_visible = state.contacts_visible.saturating_add(CONTACTS_PAGE_SIZE);
      }
      Task::none()
    }
    Message::KilllogPageLoaded(entries) => {
      state.killlog_loading_more = false;
      state.killlog_has_more = entries.len() as i64 == KILLLOG_PAGE_SIZE;
      state.killlog_cursor = entries.last().map(killlog_cursor);
      if let LoadState::Loaded(existing) = &mut state.killlog {
        existing.extend(entries);
      }
      Task::none()
    }
    Message::KilllogScrolled(offset) => {
      if offset < SCROLL_THRESHOLD || !state.killlog_has_more || state.killlog_loading_more {
        return Task::none();
      }
      let Some(cursor) = state.killlog_cursor.clone() else {
        return Task::none();
      };
      state.killlog_loading_more = true;
      Task::perform(
        load_killlog_page(db.clone(), state.active, Some(cursor)),
        Message::KilllogPageLoaded,
      )
    }
    Message::StandingsAgentsPageLoaded(page) => {
      let StandingsAgentsPage {
        generation,
        next_cursor,
        rows,
      } = *page;
      state.standings_loading_more = false;
      if generation != state.standings_generation {
        return Task::none();
      }
      state.standings_has_more = next_cursor.is_some();
      state.standings_agent_cursor = next_cursor;
      if let LoadState::Loaded(existing) = &mut state.standings {
        existing.extend(rows);
      }
      Task::none()
    }
    Message::StandingsScrolled(offset) => {
      // Only the agent-surfacing filters paginate agents; under Factions/Corps/Other a forced-false page
      // would come back empty and clobber `standings_has_more`, so skip the fetch entirely.
      if offset < SCROLL_THRESHOLD
        || !state.standings_has_more
        || state.standings_loading_more
        || !state.standings_filter.surfaces_agents()
      {
        return Task::none();
      }
      let Some(cursor) = state.standings_agent_cursor.clone() else {
        return Task::none();
      };
      state.standings_loading_more = true;
      run_standings_agent_page(
        db.clone(),
        state.active,
        state.standings_query.clone(),
        state.standings_filter.surfaces_agents(),
        cursor,
        state.standings_generation,
      )
    }
    _ => Task::none(),
  }
}

fn killlog_cursor(entry: &KillLogEntry) -> (String, i64) {
  (entry.kill_time.clone(), entry.killmail_id)
}

fn reset_killlog_pagination(state: &mut State) {
  state.killlog_loading_more = false;
  state.killlog_cursor = match &state.killlog {
    LoadState::Loaded(entries) => entries.last().map(killlog_cursor),
    _ => None,
  };
  state.killlog_has_more = match &state.killlog {
    LoadState::Loaded(entries) => entries.len() as i64 == KILLLOG_PAGE_SIZE,
    _ => false,
  };
}

fn append_standings_query(state: &mut State, fragment: &str) {
  let trimmed = state.standings_query.trim_end();
  state.standings_query = if trimmed.is_empty() {
    fragment.to_owned()
  } else {
    format!("{trimmed} {fragment}")
  };
}

fn change_standings_filter(
  state: &mut State,
  filter: tabs::standings::StandingsFilter,
  db: &Database,
) -> Task<Message> {
  state.standings_filter = filter;
  // Filtering is in-memory: agents are already loaded from the default All initial load. Reload only as a safety net
  // when switching to an agent-surfacing filter that has no agent rows loaded (e.g. agents were never fetched under a
  // non-agent filter) and a load is not already in flight.
  if filter.surfaces_agents() && !state.has_loaded_agents() && !matches!(state.standings, LoadState::Loading) {
    trigger_standings_search(state, db)
  } else {
    Task::none()
  }
}

fn trigger_standings_search(state: &mut State, db: &Database) -> Task<Message> {
  state.standings_generation = state.standings_generation.wrapping_add(1);
  state.standings = LoadState::Loading;
  state.standings_has_more = false;
  state.standings_agent_cursor = None;
  state.standings_loading_more = false;
  run_standings_search(
    db.clone(),
    state.active,
    state.standings_query.clone(),
    state.standings_filter.surfaces_agents(),
    state.standings_generation,
  )
}

fn run_standings_search(
  db: Database,
  character_id: i64,
  query: String,
  force_agents: bool,
  generation: u64,
) -> Task<Message> {
  Task::perform(
    async move {
      tokio::time::sleep(Duration::from_millis(SEARCH_DEBOUNCE_MS)).await;
      load_standings_catalog(&db, character_id, &query, force_agents).await
    },
    move |result| {
      Message::StandingsResults(Box::new(StandingsResult {
        generation,
        result,
      }))
    },
  )
}

fn run_standings_agent_page(
  db: Database,
  character_id: i64,
  query: String,
  force_agents: bool,
  cursor: (String, i64),
  generation: u64,
) -> Task<Message> {
  Task::perform(
    async move { load_standings_agent_page(&db, character_id, &query, force_agents, cursor).await },
    move |page| {
      let (next_cursor, rows) = page.unwrap_or((None, Vec::new()));
      Message::StandingsAgentsPageLoaded(Box::new(StandingsAgentsPage {
        generation,
        next_cursor,
        rows,
      }))
    },
  )
}

async fn load_standings_agent_page(
  db: &Database,
  character_id: i64,
  query: &str,
  force_agents: bool,
  cursor: (String, i64),
) -> Result<(Option<(String, i64)>, Vec<StandingsRow>), String> {
  let parsed = standings::parse(query);
  let page = standings::agent_page(
    db,
    character_id,
    &parsed,
    force_agents,
    Some(cursor),
    STANDINGS_PAGE_SIZE,
  )
  .await
  .map_err(|error| error.to_string())?;

  let store = images::default_store();
  let rows = page.rows.into_iter().map(|row| standings_row(&store, row)).collect();
  Ok((page.next_cursor, rows))
}

// Factions and corporations are loaded in full (limit 0 suppresses the catalog's own agent page); agents come from
// the first keyset page so the result carries a cursor for infinite scroll. `force_agents` lets the active segment
// filter surface the agent catalog with no narrowing text facet.
async fn load_standings_catalog(
  db: &Database,
  character_id: i64,
  query: &str,
  force_agents: bool,
) -> Result<StandingsCatalog, String> {
  let parsed = standings::parse(query);
  let context = standings::catalog(db, character_id, &parsed, force_agents, Some(0))
    .await
    .map_err(|error| error.to_string())?;
  let agents = standings::agent_page(db, character_id, &parsed, force_agents, None, STANDINGS_PAGE_SIZE)
    .await
    .map_err(|error| error.to_string())?;

  let store = images::default_store();
  let mut rows: Vec<StandingsRow> = context.into_iter().map(|row| standings_row(&store, row)).collect();
  rows.extend(agents.rows.into_iter().map(|row| standings_row(&store, row)));
  Ok(StandingsCatalog {
    agent_cursor: agents.next_cursor,
    rows,
  })
}

fn standings_row(store: &images::Store, row: standings::CatalogRow) -> StandingsRow {
  let (image_kind, image_id) = match row.kind {
    StandingKind::Agent => (images::ImageKind::CharacterPortrait, row.id),
    StandingKind::Corporation => (images::ImageKind::CorporationLogo, row.id),
    // A faction has no logo of its own; use its corporation's, falling back to the faction id.
    StandingKind::Faction => (images::ImageKind::CorporationLogo, row.corporation_id.unwrap_or(row.id)),
  };

  StandingsRow {
    accessible: row.accessible,
    agent_type: row.agent_type,
    division: row.division,
    effective: row.effective_standing,
    faction_id: row.faction_id,
    id: row.id,
    image: images::resolve(store, image_kind, image_id),
    kind: row.kind,
    level: row.level,
    name: row.name,
    raw: row.raw_standing,
    region: row.region_name,
    system: row.system_name,
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

  if state.standings_help_open && state.active_tab == Tab::Standings {
    let popover = positioned_dropdown_right(
      tabs::standings::help_popover(),
      STANDINGS_HELP_OVERLAY_TOP,
      HEADER_SIDE_PADDING,
    );

    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::StandingsToggleHelp),
      popover,
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
  let granted_scopes = load_granted_scopes(&db, character_id).await;

  Loaded {
    clones,
    contacts,
    granted_scopes,
    head,
    killlog,
    notifications,
    roster,
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
  let rows = match character::killmails_page(db, character_id, None, KILLLOG_PAGE_SIZE).await {
    Ok(rows) => rows,
    Err(error) => return LoadState::Error(error.to_string()),
  };

  LoadState::Loaded(resolve_killlog_entries(db, rows).await)
}

async fn load_killlog_page(db: Database, character_id: i64, after: Option<(String, i64)>) -> Vec<KillLogEntry> {
  let rows = match character::killmails_page(&db, character_id, after, KILLLOG_PAGE_SIZE).await {
    Ok(rows) => rows,
    Err(_) => return Vec::new(),
  };

  resolve_killlog_entries(&db, rows).await
}

async fn resolve_killlog_entries(
  db: &Database,
  rows: Vec<crate::store::model::CharacterKillEntry>,
) -> Vec<KillLogEntry> {
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

  entries
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
    DetailDataType::Standings => Reloaded::Standings,
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

  fn standings_row_fixture(id: i64, kind: StandingKind, value: f64) -> StandingsRow {
    StandingsRow {
      accessible: None,
      agent_type: None,
      division: None,
      effective: value,
      faction_id: Some(500_001),
      id,
      image: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CorporationLogo,
      },
      kind,
      level: None,
      name: format!("Entity {id}"),
      raw: value,
      region: None,
      system: None,
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
      standings_row_fixture(500_001, StandingKind::Faction, 5.0),
      standings_row_fixture(1_000_125, StandingKind::Corporation, -2.5),
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

  mod pagination {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{CharacterContact, character_contacts_view::CharacterContacts};

    fn killlog_entry(killmail_id: i64, kill_time: &str) -> KillLogEntry {
      KillLogEntry {
        attacker_count: 1,
        final_blow: true,
        is_kill: true,
        kill_time: kill_time.to_owned(),
        killmail_id,
        ship_name: "Rifter".to_owned(),
        ship_type_id: 587,
        system_name: Some("Jita".to_owned()),
        system_security: 0.9,
        value_destroyed_isk: 0.0,
        value_isk: 0.0,
        victim_corp: String::new(),
        victim_name: "Unknown".to_owned(),
      }
    }

    fn contact(contact_id: i64) -> CharacterContact {
      CharacterContact {
        character_id: 42,
        contact_id,
        contact_name: format!("Contact {contact_id}"),
        contact_type: "character".to_owned(),
        is_blocked: false,
        is_watched: false,
        label_ids: String::new(),
        standing: 0.0,
      }
    }

    fn killlog_page(count: i64) -> Vec<KillLogEntry> {
      (0..count)
        .map(|n| killlog_entry(1000 + n, "2024-01-01T00:00:00Z"))
        .collect()
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(&mut state, Message::StandingsScrolled(0.5), &db);

      assert!(!state.standings_loading_more, "a sub-threshold scroll is a no-op");
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_with_no_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = false;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(&mut state, Message::StandingsScrolled(0.95), &db);

      assert!(!state.standings_loading_more, "exhausted agents do not fetch");
    }

    #[tokio::test]
    async fn it_marks_loading_and_clears_the_cursor_guard_on_a_standings_page_fetch() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(&mut state, Message::StandingsScrolled(0.95), &db);

      assert!(state.standings_loading_more, "a qualifying scroll starts a load");
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_while_already_loading() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = true;
      state.standings_loading_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(&mut state, Message::StandingsScrolled(0.95), &db);

      assert!(state.standings_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_when_the_filter_does_not_surface_agents() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_filter = tabs::standings::StandingsFilter::Factions;
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(&mut state, Message::StandingsScrolled(0.95), &db);

      assert!(
        !state.standings_loading_more,
        "a non-agent filter does not paginate agents"
      );
      assert!(state.standings_has_more, "the cursor and has_more are left untouched");
      assert_eq!(state.standings_agent_cursor, Some(("Agent".to_owned(), 1)));
    }

    #[tokio::test]
    async fn it_appends_a_standings_page_and_recomputes_has_more() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      let before = match &state.standings {
        LoadState::Loaded(rows) => rows.len(),
        _ => 0,
      };
      state.standings_loading_more = true;

      let page = StandingsAgentsPage {
        generation: state.standings_generation,
        next_cursor: Some(("Next".to_owned(), 9)),
        rows: vec![standings_row_fixture(3_000_001, StandingKind::Agent, 1.0)],
      };
      let _ = update(&mut state, Message::StandingsAgentsPageLoaded(Box::new(page)), &db);

      assert!(!state.standings_loading_more);
      assert!(state.standings_has_more, "a carried cursor means more pages remain");
      assert_eq!(state.standings_agent_cursor, Some(("Next".to_owned(), 9)));
      assert!(matches!(state.standings, LoadState::Loaded(ref rows) if rows.len() == before + 1));
    }

    #[tokio::test]
    async fn it_exhausts_standings_when_a_page_carries_no_cursor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = true;
      state.standings_loading_more = true;

      let page = StandingsAgentsPage {
        generation: state.standings_generation,
        next_cursor: None,
        rows: Vec::new(),
      };
      let _ = update(&mut state, Message::StandingsAgentsPageLoaded(Box::new(page)), &db);

      assert!(!state.standings_has_more);
      assert_eq!(state.standings_agent_cursor, None);
    }

    #[tokio::test]
    async fn it_drops_a_stale_standings_page_from_an_old_generation() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_generation = 5;
      state.standings_loading_more = true;
      let before = match &state.standings {
        LoadState::Loaded(rows) => rows.len(),
        _ => 0,
      };

      let page = StandingsAgentsPage {
        generation: 4,
        next_cursor: Some(("Next".to_owned(), 9)),
        rows: vec![standings_row_fixture(3_000_001, StandingKind::Agent, 1.0)],
      };
      let _ = update(&mut state, Message::StandingsAgentsPageLoaded(Box::new(page)), &db);

      assert!(!state.standings_loading_more, "loading clears even for a stale page");
      assert!(
        matches!(state.standings, LoadState::Loaded(ref rows) if rows.len() == before),
        "a stale generation does not append rows"
      );
      assert!(
        state.standings_agent_cursor.is_none(),
        "the stale cursor is not adopted"
      );
    }

    #[tokio::test]
    async fn it_ignores_a_killlog_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.killlog = LoadState::Loaded(killlog_page(KILLLOG_PAGE_SIZE));
      reset_killlog_pagination(&mut state);

      let _ = update(&mut state, Message::KilllogScrolled(0.5), &db);

      assert!(!state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_killlog_scroll_with_no_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.killlog = LoadState::Loaded(killlog_page(3));
      reset_killlog_pagination(&mut state);

      let _ = update(&mut state, Message::KilllogScrolled(0.95), &db);

      assert!(!state.killlog_has_more, "a short page is the last page");
      assert!(!state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_marks_loading_on_a_qualifying_killlog_scroll() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.killlog = LoadState::Loaded(killlog_page(KILLLOG_PAGE_SIZE));
      reset_killlog_pagination(&mut state);

      let _ = update(&mut state, Message::KilllogScrolled(0.95), &db);

      assert!(state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_appends_a_killlog_page_and_recomputes_has_more() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.killlog = LoadState::Loaded(killlog_page(KILLLOG_PAGE_SIZE));
      state.killlog_loading_more = true;

      let _ = update(&mut state, Message::KilllogPageLoaded(killlog_page(3)), &db);

      assert!(!state.killlog_loading_more);
      assert!(!state.killlog_has_more, "a short appended page exhausts the log");
      assert!(matches!(state.killlog, LoadState::Loaded(ref rows) if rows.len() == KILLLOG_PAGE_SIZE as usize + 3));
    }

    #[tokio::test]
    async fn it_grows_the_contacts_window_on_scroll_and_resets_on_a_filter_change() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contacts = LoadState::Loaded(CharacterContacts::resolved(
        &images::Store::new(std::path::PathBuf::from("/data/images")),
        (0..CONTACTS_PAGE_SIZE as i64 * 3).map(contact).collect(),
        Vec::new(),
      ));

      assert_eq!(state.contacts_visible(), CONTACTS_PAGE_SIZE);

      let _ = update(&mut state, Message::ContactsScrolled(0.9), &db);
      assert_eq!(state.contacts_visible(), CONTACTS_PAGE_SIZE * 2);

      let _ = update(
        &mut state,
        Message::ContactFilterChanged(tabs::contacts::ContactFilter::Character),
        &db,
      );
      assert_eq!(
        state.contacts_visible(),
        CONTACTS_PAGE_SIZE,
        "switching filters resets the virtual window"
      );
    }

    #[tokio::test]
    async fn it_ignores_a_contacts_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;

      let _ = update(&mut state, Message::ContactsScrolled(0.5), &db);

      assert_eq!(state.contacts_visible(), CONTACTS_PAGE_SIZE);
    }
  }

  mod standings_search {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_records_the_query_and_advances_the_generation() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(
        &mut state,
        Message::StandingsSearchChanged("faction:caldari".to_owned()),
        &db,
      );

      assert_eq!(state.standings_query, "faction:caldari");
      assert_eq!(state.standings_generation, 1);
      assert!(matches!(state.standings, LoadState::Loading));
    }

    #[tokio::test]
    async fn it_clears_the_query_and_re_runs_the_default_catalog() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);
      let _ = update(&mut state, Message::StandingsSearchChanged("corp:navy".to_owned()), &db);

      let _ = update(&mut state, Message::StandingsClearSearch, &db);

      assert!(state.standings_query.is_empty());
      assert!(matches!(state.standings, LoadState::Loading));
    }

    #[tokio::test]
    async fn it_appends_an_inserted_fragment_and_closes_the_help() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);
      state.standings_help_open = true;
      state.standings_query = "faction:caldari".to_owned();

      let _ = update(&mut state, Message::StandingsInsertQuery("corp:navy".to_owned()), &db);

      assert_eq!(state.standings_query, "faction:caldari corp:navy");
      assert!(!state.standings_help_open);
    }

    #[tokio::test]
    async fn it_records_the_facet_filter_without_reloading() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);
      let before = state.standings_generation;

      let _ = update(
        &mut state,
        Message::StandingsFilterChanged(tabs::standings::StandingsFilter::Corps),
        &db,
      );

      assert_eq!(state.standings_filter, tabs::standings::StandingsFilter::Corps);
      assert_eq!(state.standings_generation, before, "filtering is in-memory only");
    }

    #[tokio::test]
    async fn it_keeps_an_agent_filter_in_memory_when_agents_are_already_loaded() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_filter = tabs::standings::StandingsFilter::Factions;
      state.standings = LoadState::Loaded(vec![standings_row_fixture(3_000_001, StandingKind::Agent, 1.0)]);
      let before = state.standings_generation;

      let _ = update(
        &mut state,
        Message::StandingsFilterChanged(tabs::standings::StandingsFilter::Agents),
        &db,
      );

      assert_eq!(state.standings_filter, tabs::standings::StandingsFilter::Agents);
      assert_eq!(
        state.standings_generation, before,
        "agents already loaded means no reload"
      );
    }

    #[tokio::test]
    async fn it_reloads_an_agent_filter_when_no_agents_are_loaded() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_filter = tabs::standings::StandingsFilter::Factions;
      state.standings = LoadState::Loaded(vec![standings_row_fixture(500_001, StandingKind::Faction, 5.0)]);
      let before = state.standings_generation;

      let _ = update(
        &mut state,
        Message::StandingsFilterChanged(tabs::standings::StandingsFilter::Agents),
        &db,
      );

      assert_eq!(
        state.standings_generation,
        before + 1,
        "switching to agents with none loaded triggers a reload"
      );
      assert!(matches!(state.standings, LoadState::Loading));
    }

    #[tokio::test]
    async fn it_toggles_the_help_popover() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::StandingsToggleHelp, &db);
      assert!(state.standings_help_open);

      let _ = update(&mut state, Message::StandingsToggleHelp, &db);
      assert!(!state.standings_help_open);
    }

    #[tokio::test]
    async fn it_applies_results_only_for_the_current_generation() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);
      state.standings_generation = 5;

      let stale = StandingsResult {
        generation: 4,
        result: Ok(StandingsCatalog {
          agent_cursor: None,
          rows: Vec::new(),
        }),
      };
      let _ = update(&mut state, Message::StandingsResults(Box::new(stale)), &db);
      assert!(matches!(state.standings, LoadState::Loading), "stale result is dropped");

      let fresh = StandingsResult {
        generation: 5,
        result: Ok(StandingsCatalog {
          agent_cursor: None,
          rows: vec![standings_row_fixture(500_001, StandingKind::Faction, 5.0)],
        }),
      };
      let _ = update(&mut state, Message::StandingsResults(Box::new(fresh)), &db);

      assert!(matches!(state.standings, LoadState::Loaded(ref rows) if rows.len() == 1));
    }
  }

  mod load_standings_catalog {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_faction(db: &Database) {
      sqlx::query(
        "INSERT INTO factions (id, corporation_id, description, is_unique, name, size_factor, station_count, \
        station_system_count) VALUES (500001, 1000099, '', 1, 'Caldari State', 1.0, 0, 0)",
      )
      .execute(&db.0)
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_maps_a_faction_to_a_corporation_logo_of_its_corporation_id() {
      let db = crate::store::open_test().await.unwrap();
      seed_faction(&db).await;

      let catalog = load_standings_catalog(&db, 42, "", false).await.unwrap();
      let faction = catalog.rows.iter().find(|row| row.name == "Caldari State").unwrap();

      assert_eq!(faction.kind, StandingKind::Faction);
      let resolved = images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, 1_000_099);
      assert_eq!(faction.image.stale_key(), resolved.stale_key());
      assert_eq!(faction.image.path(), resolved.path());
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
      CharacterContacts::resolved(
        &images::Store::new(std::path::PathBuf::from("/data/images")),
        Vec::new(),
        Vec::new(),
      )
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
    async fn it_retriggers_the_standings_catalog_on_a_standings_reload() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      let before = state.standings_generation;

      let _ = update(&mut state, Message::Reloaded(Box::new(Reloaded::Standings)), &db);

      assert!(
        matches!(state.standings, LoadState::Loading),
        "a standings reload re-runs the catalog query"
      );
      assert_eq!(state.standings_generation, before + 1);
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
        contacts: LoadState::Loaded(CharacterContacts::resolved(
          &images::Store::new(std::path::PathBuf::from("/data/images")),
          Vec::new(),
          Vec::new(),
        )),
        granted_scopes: None,
        head: HeadStats {
          total_sp: Some(1_000),
          ..HeadStats::default()
        },
        killlog: LoadState::Loaded(Vec::new()),
        notifications: LoadState::Loaded(Vec::new()),
        roster: vec![pilot(42, "Pilot")],
      };

      let _ = update(&mut state, Message::Loaded(Box::new(loaded)), &db);

      assert_eq!(state.roster.len(), 1);
      assert_eq!(state.head.total_sp, Some(1_000));
      assert!(matches!(state.clones, LoadState::Loaded(None)));
      assert!(
        matches!(state.standings, LoadState::Loading),
        "a full load kicks off the standings catalog query"
      );
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
        Reloaded::Standings
      ));
    }
  }
}
