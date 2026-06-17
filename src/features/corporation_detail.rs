mod killmail_loader;
mod tabs;

use std::time::Duration;

use iced::{
  Element, Length, Task,
  alignment::Vertical,
  widget::{Column, Row, container, operation, text},
};

pub use self::tabs::Tab;
use self::tabs::{
  contacts::ContactRow,
  killlog::{KillLogEntry, KilllogFilter},
};
pub use crate::store::repo::standings::CatalogKind as StandingKind;
use crate::{
  clients::eve_image::Size,
  features::killmail_detail::{self, KillmailDetail},
  store::{
    Database, images,
    model::{CorporationContactLabel, character_contacts_view::image_kind},
    repo::{
      character,
      character::{ContactCursor, ContactSortColumn, ContactSortDir},
      org, sde, standings,
    },
  },
  ui::{
    components::{
      avatar::Avatar,
      header::{header as header_band, header_divider, stat_block},
    },
    style::{color, radius, spacing, typography},
  },
};

const CONTACTS_PAGE_SIZE: i64 = 100;
const KILLLOG_PAGE_SIZE: i64 = 100;
const KILLLOG_SHIP_ICON_SIZE: Size = Size::S64;

pub(crate) const STANDINGS_SEARCH_INPUT_ID: &str = "corp-standings-search-input";

const LOGO_SIZE: f32 = 44.0;
const PLACEHOLDER: &str = "\u{2014}";
const SEARCH_DEBOUNCE_MS: u64 = 200;
const STANDINGS_PAGE_SIZE: i64 = 100;

/// One keyset page of render-ready contact rows plus the per-corporation label lookup. The labels travel with the
/// first page so the address-book notes can resolve label ids without a second query per page.
#[derive(Clone, Debug)]
pub struct ContactsPage {
  cursor: Option<ContactCursor>,
  has_more: bool,
  labels: Vec<CorporationContactLabel>,
  rows: Vec<ContactRow>,
}

impl ContactsPage {
  /// Builds a page directly from render-ready rows and labels. Used by the tab's view tests, which assert on
  /// layout rather than the keyset cursor (so the cursor is derived as `None`).
  #[cfg(test)]
  pub(in crate::features::corporation_detail) fn for_test(
    rows: Vec<ContactRow>,
    labels: Vec<CorporationContactLabel>,
    has_more: bool,
  ) -> Self {
    ContactsPage {
      cursor: None,
      has_more,
      labels,
      rows,
    }
  }

  pub(super) fn has_more(&self) -> bool {
    self.has_more
  }

  pub(super) fn labels(&self) -> &[CorporationContactLabel] {
    &self.labels
  }

  pub(super) fn rows(&self) -> &[ContactRow] {
    &self.rows
  }
}

#[derive(Clone, Debug)]
pub struct CorpDetail {
  pub contacts: ContactsPage,
  pub head: Option<CorpHead>,
  pub killlog: LoadState<Vec<KillLogEntry>>,
}

#[derive(Clone, Debug)]
pub struct CorpHead {
  pub alliance: Option<String>,
  pub ceo: Option<String>,
  pub corporation_id: i64,
  pub hq: Option<String>,
  pub logo: images::ImageState,
  pub members: Option<i64>,
  pub name: String,
  pub tax_rate: Option<f64>,
  pub ticker: String,
}

#[derive(Clone, Debug)]
pub enum LoadState<T> {
  Error(String),
  Loaded(T),
  Loading,
}

#[derive(Clone, Debug)]
pub enum Message {
  CloseKillmailDetail,
  ContactFilterChanged(tabs::contacts::ContactFilter),
  ContactSortChanged(tabs::contacts::ContactSort),
  ContactsPageLoaded(Box<ContactsPage>),
  ContactsScrolled { absolute: f32, relative: f32 },
  ContactsSearchChanged(String),
  ContactsSearchCleared,
  KilllogFilterChanged(KilllogFilter),
  KilllogPageLoaded(Vec<KillLogEntry>),
  KilllogScrolled { absolute: f32, relative: f32 },
  KillmailDetailLoaded(Box<Option<KillmailDetail>>),
  KillmailSelected(i64),
  Loaded(Box<CorpDetail>),
  StandingsAgentsPageLoaded(Box<StandingsAgentsPage>),
  StandingsClearSearch,
  StandingsFilterChanged(tabs::standings::StandingsFilter),
  StandingsResults(Box<StandingsResult>),
  StandingsScrolled { absolute: f32, relative: f32 },
  StandingsSearchChanged(String),
  TabChanged(Tab),
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows (logos, standings/contact avatars, killmail
  /// detail), so the shell should recheck for stale images. Interaction-only messages return `false` to keep the
  /// staleness scan off the per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(
      self,
      Message::ContactsPageLoaded(_)
        | Message::KillmailDetailLoaded(_)
        | Message::Loaded(_)
        | Message::StandingsAgentsPageLoaded(_)
        | Message::StandingsResults(_)
    )
  }
}

#[derive(Debug)]
pub struct State {
  active: i64,
  active_tab: Tab,
  contact_filter: tabs::contacts::ContactFilter,
  contact_sort: tabs::contacts::ContactSort,
  contacts: LoadState<ContactsPage>,
  contacts_cursor: Option<ContactCursor>,
  contacts_has_more: bool,
  contacts_loading_more: bool,
  contacts_query: String,
  contacts_scroll_offset: f32,
  head: Option<CorpHead>,
  killlog: LoadState<Vec<KillLogEntry>>,
  killlog_cursor: Option<(String, i64)>,
  killlog_filter: KilllogFilter,
  killlog_has_more: bool,
  killlog_loading_more: bool,
  killlog_scroll_offset: f32,
  selected_killmail: Option<KillmailDetail>,
  standings: LoadState<Vec<StandingsRow>>,
  standings_agent_cursor: Option<(String, i64)>,
  standings_filter: tabs::standings::StandingsFilter,
  standings_generation: u64,
  standings_has_more: bool,
  standings_loading_more: bool,
  standings_query: String,
  standings_scroll_offset: f32,
}

impl State {
  pub fn new(active: i64) -> Self {
    State {
      active,
      active_tab: Tab::ORDER[0],
      contact_filter: tabs::contacts::ContactFilter::All,
      contact_sort: tabs::contacts::ContactSort::default(),
      contacts: LoadState::Loading,
      contacts_cursor: None,
      contacts_has_more: false,
      contacts_loading_more: false,
      contacts_query: String::new(),
      contacts_scroll_offset: 0.0,
      head: None,
      killlog: LoadState::Loading,
      killlog_cursor: None,
      killlog_filter: KilllogFilter::All,
      killlog_has_more: false,
      killlog_loading_more: false,
      killlog_scroll_offset: 0.0,
      selected_killmail: None,
      standings: LoadState::Loading,
      standings_agent_cursor: None,
      standings_filter: tabs::standings::StandingsFilter::All,
      standings_generation: 0,
      standings_has_more: false,
      standings_loading_more: false,
      standings_query: String::new(),
      standings_scroll_offset: 0.0,
    }
  }

  pub fn active(&self) -> i64 {
    self.active
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    let mut keys: Vec<(images::ImageKind, i64)> = self
      .head
      .as_ref()
      .and_then(|head| head.logo.stale_key())
      .into_iter()
      .collect();
    if let LoadState::Loaded(page) = &self.contacts {
      keys.extend(page.rows.iter().filter_map(|row| row.image.stale_key()));
    }
    if let LoadState::Loaded(rows) = &self.standings {
      keys.extend(rows.iter().filter_map(|row| row.image.stale_key()));
    }
    if let Some(detail) = &self.selected_killmail {
      keys.extend(detail.stale_images());
    }
    keys
  }

  pub(super) fn active_tab(&self) -> Tab {
    self.active_tab
  }

  pub(super) fn contact_filter(&self) -> tabs::contacts::ContactFilter {
    self.contact_filter
  }

  pub(super) fn contact_sort(&self) -> tabs::contacts::ContactSort {
    self.contact_sort
  }

  pub(super) fn contacts(&self) -> &LoadState<ContactsPage> {
    &self.contacts
  }

  pub(super) fn contacts_query(&self) -> &str {
    &self.contacts_query
  }

  pub(super) fn contacts_scroll_offset(&self) -> f32 {
    self.contacts_scroll_offset
  }

  pub(super) fn killlog(&self) -> &LoadState<Vec<KillLogEntry>> {
    &self.killlog
  }

  pub(super) fn killlog_filter(&self) -> KilllogFilter {
    self.killlog_filter
  }

  pub(super) fn killlog_scroll_offset(&self) -> f32 {
    self.killlog_scroll_offset
  }

  pub(super) fn standings(&self) -> &LoadState<Vec<StandingsRow>> {
    &self.standings
  }

  pub(super) fn standings_filter(&self) -> tabs::standings::StandingsFilter {
    self.standings_filter
  }

  pub(super) fn standings_has_filters(&self) -> bool {
    !self.standings_query.trim().is_empty()
  }

  pub(super) fn standings_query(&self) -> &str {
    &self.standings_query
  }

  pub(super) fn standings_scroll_offset(&self) -> f32 {
    self.standings_scroll_offset
  }

  fn has_loaded_agents(&self) -> bool {
    matches!(&self.standings, LoadState::Loaded(rows) if rows.iter().any(|row| row.kind == StandingKind::Agent))
  }
}

#[derive(Clone, Debug)]
pub struct StandingsAgentsPage {
  generation: u64,
  next_cursor: Option<(String, i64)>,
  rows: Vec<StandingsRow>,
}

#[derive(Clone, Debug)]
pub struct StandingsCatalog {
  /// Keyset cursor for the next agent page, or `None` when the first agent page exhausted them.
  agent_cursor: Option<(String, i64)>,
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

pub fn load(db: &Database, corporation_id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { load_detail(&db, corporation_id).await }, |detail| {
    Message::Loaded(Box::new(detail))
  })
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ContactFilterChanged(_)
    | Message::ContactSortChanged(_)
    | Message::ContactsPageLoaded(_)
    | Message::ContactsScrolled {
      ..
    }
    | Message::ContactsSearchChanged(_)
    | Message::ContactsSearchCleared => update_contacts(state, message, db),
    Message::CloseKillmailDetail
    | Message::KilllogFilterChanged(_)
    | Message::KilllogPageLoaded(_)
    | Message::KilllogScrolled {
      ..
    }
    | Message::KillmailDetailLoaded(_)
    | Message::KillmailSelected(_) => update_killlog(state, message, db),
    Message::StandingsAgentsPageLoaded(_)
    | Message::StandingsClearSearch
    | Message::StandingsFilterChanged(_)
    | Message::StandingsResults(_)
    | Message::StandingsScrolled {
      ..
    }
    | Message::StandingsSearchChanged(_) => update_standings(state, message, db),
    Message::Loaded(_) | Message::TabChanged(_) => update_head(state, message, db),
  }
}

fn update_contacts(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ContactFilterChanged(filter) => {
      state.contact_filter = filter;
      restart_contacts(state, db)
    }
    Message::ContactSortChanged(sort) => {
      state.contact_sort = sort;
      restart_contacts(state, db)
    }
    Message::ContactsPageLoaded(page) => {
      apply_contacts_page(state, *page);
      Task::none()
    }
    Message::ContactsScrolled {
      absolute,
      relative,
    } => {
      state.contacts_scroll_offset = absolute;
      if relative < tabs::SCROLL_THRESHOLD || !state.contacts_has_more || state.contacts_loading_more {
        return Task::none();
      }
      let Some(cursor) = state.contacts_cursor.clone() else {
        return Task::none();
      };
      state.contacts_loading_more = true;
      let (contact_type, query, sort, dir) = contact_query_params(state);
      Task::perform(
        load_contacts_page(db.clone(), state.active, contact_type, query, sort, dir, Some(cursor)),
        |page| Message::ContactsPageLoaded(Box::new(page)),
      )
    }
    Message::ContactsSearchChanged(query) => {
      if state.contacts_query == query {
        return Task::none();
      }
      state.contacts_query = query;
      restart_contacts(state, db)
    }
    Message::ContactsSearchCleared => {
      if state.contacts_query.is_empty() {
        return Task::none();
      }
      state.contacts_query.clear();
      restart_contacts(state, db)
    }
    _ => Task::none(),
  }
}

fn apply_contacts_page(state: &mut State, page: ContactsPage) {
  let ContactsPage {
    cursor,
    has_more,
    labels,
    rows,
  } = page;
  state.contacts_loading_more = false;
  state.contacts_has_more = has_more;
  state.contacts_cursor = cursor.clone();
  // Extend the existing page on a scroll-driven fetch; replace it outright when the prior state was Loading
  // (a fresh first page from initial load or a sort/filter restart).
  match &mut state.contacts {
    LoadState::Loaded(existing) => {
      existing.cursor = cursor;
      existing.has_more = has_more;
      existing.rows.extend(rows);
      if existing.labels.is_empty() {
        existing.labels = labels;
      }
    }
    _ => {
      state.contacts = LoadState::Loaded(ContactsPage {
        cursor,
        has_more,
        labels,
        rows,
      });
    }
  }
}

fn update_killlog(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::CloseKillmailDetail => {
      state.selected_killmail = None;
      Task::none()
    }
    Message::KilllogFilterChanged(filter) => {
      state.killlog_filter = filter;
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
    Message::KilllogScrolled {
      absolute,
      relative,
    } => {
      state.killlog_scroll_offset = absolute;
      if relative < tabs::SCROLL_THRESHOLD || !state.killlog_has_more || state.killlog_loading_more {
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
    Message::KillmailDetailLoaded(detail) => {
      state.selected_killmail = *detail;
      Task::none()
    }
    Message::KillmailSelected(killmail_id) => {
      Task::perform(load_killmail_detail(db.clone(), state.active, killmail_id), |detail| {
        Message::KillmailDetailLoaded(Box::new(detail))
      })
    }
    _ => Task::none(),
  }
}

fn update_standings(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::StandingsAgentsPageLoaded(page) => {
      apply_standings_agents_page(state, *page);
      Task::none()
    }
    Message::StandingsClearSearch => {
      state.standings_query.clear();
      Task::batch([
        trigger_standings_search(state, db),
        operation::focus(STANDINGS_SEARCH_INPUT_ID),
      ])
    }
    Message::StandingsFilterChanged(filter) => {
      state.standings_filter = filter;
      // Filtering is in-memory: agents are already loaded from the default All initial load. Reload only as a safety
      // net when switching to an agent-surfacing filter that has no agent rows loaded and a load is not in flight.
      if filter.surfaces_agents() && !state.has_loaded_agents() && !matches!(state.standings, LoadState::Loading) {
        trigger_standings_search(state, db)
      } else {
        Task::none()
      }
    }
    Message::StandingsResults(results) => {
      apply_standings_results(state, *results);
      Task::none()
    }
    Message::StandingsScrolled {
      absolute,
      relative,
    } => {
      state.standings_scroll_offset = absolute;
      // Only the agent-surfacing filters paginate agents; under Factions/Corps/Other a forced-false page
      // would come back empty and clobber `standings_has_more`, so skip the fetch entirely.
      if relative < tabs::SCROLL_THRESHOLD
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
    Message::StandingsSearchChanged(query) => {
      state.standings_query = query;
      trigger_standings_search(state, db)
    }
    _ => Task::none(),
  }
}

fn apply_standings_agents_page(state: &mut State, page: StandingsAgentsPage) {
  let StandingsAgentsPage {
    generation,
    next_cursor,
    rows,
  } = page;
  state.standings_loading_more = false;
  if generation != state.standings_generation {
    return;
  }
  state.standings_has_more = next_cursor.is_some();
  state.standings_agent_cursor = next_cursor;
  if let LoadState::Loaded(existing) = &mut state.standings {
    existing.extend(rows);
  }
}

fn apply_standings_results(state: &mut State, results: StandingsResult) {
  let StandingsResult {
    generation,
    result,
  } = results;
  if generation != state.standings_generation {
    return;
  }
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

fn update_head(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Loaded(detail) => {
      let CorpDetail {
        contacts,
        head,
        killlog,
      } = *detail;
      state.contacts = LoadState::Loaded(contacts);
      state.head = head;
      state.killlog = killlog;
      reset_contacts_pagination(state);
      reset_killlog_pagination(state);
      trigger_standings_search(state, db)
    }
    Message::TabChanged(tab) => {
      state.active_tab = tab;
      Task::none()
    }
    _ => Task::none(),
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let body = Column::with_children(vec![
    header(state),
    tabs::tab_strip(state.active_tab),
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

  if let Some(detail) = state.selected_killmail.as_ref() {
    return killmail_detail::overlay(base.into(), detail, Message::CloseKillmailDetail);
  }

  base.into()
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if state.selected_killmail.is_none() {
    return iced::Subscription::none();
  }
  iced::event::listen_with(|event, _status, _id| {
    matches!(
      event,
      iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        ..
      })
    )
    .then_some(Message::CloseKillmailDetail)
  })
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

fn run_standings_agent_page(
  db: Database,
  corporation_id: i64,
  query: String,
  force_agents: bool,
  cursor: (String, i64),
  generation: u64,
) -> Task<Message> {
  Task::perform(
    async move { load_standings_agent_page(&db, corporation_id, &query, force_agents, cursor).await },
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

fn run_standings_search(
  db: Database,
  corporation_id: i64,
  query: String,
  force_agents: bool,
  generation: u64,
) -> Task<Message> {
  Task::perform(
    async move {
      tokio::time::sleep(Duration::from_millis(SEARCH_DEBOUNCE_MS)).await;
      load_standings_catalog(&db, corporation_id, &query, force_agents).await
    },
    move |result| {
      Message::StandingsResults(Box::new(StandingsResult {
        generation,
        result,
      }))
    },
  )
}

// Factions and corporations are loaded in full (limit 0 suppresses the catalog's own agent page); agents come from
// the first keyset page so the result carries a cursor for infinite scroll. `force_agents` lets the active segment
// filter surface the agent catalog with no narrowing text facet.
async fn load_standings_catalog(
  db: &Database,
  corporation_id: i64,
  query: &str,
  force_agents: bool,
) -> Result<StandingsCatalog, String> {
  let parsed = standings::parse(query);
  let context = standings::corporation_catalog(db, corporation_id, &parsed, force_agents, Some(0))
    .await
    .map_err(|error| error.to_string())?;
  let agents = standings::corporation_agent_page(db, corporation_id, &parsed, force_agents, None, STANDINGS_PAGE_SIZE)
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

async fn load_standings_agent_page(
  db: &Database,
  corporation_id: i64,
  query: &str,
  force_agents: bool,
  cursor: (String, i64),
) -> Result<(Option<(String, i64)>, Vec<StandingsRow>), String> {
  let parsed = standings::parse(query);
  let page = standings::corporation_agent_page(
    db,
    corporation_id,
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

async fn load_detail(db: &Database, corporation_id: i64) -> CorpDetail {
  let contacts = load_contacts(db, corporation_id).await;
  let head = load_head(db, corporation_id).await;
  let killlog = load_killlog(db, corporation_id).await;

  CorpDetail {
    contacts,
    head,
    killlog,
  }
}

async fn load_contacts(db: &Database, corporation_id: i64) -> ContactsPage {
  load_contacts_page(
    db.clone(),
    corporation_id,
    None,
    None,
    ContactSortColumn::Standing,
    ContactSortDir::Desc,
    None,
  )
  .await
}

/// Fetches and image-resolves one keyset page of contacts. The returned page carries the next cursor (when the page
/// filled to the page size). A first page (`after` is `None`) also carries the per-corporation labels, so the initial
/// load and every sort/filter restart bring the address-book labels along; follow-on pages return an empty label set
/// and the caller keeps the labels it already holds.
async fn load_contacts_page(
  db: Database,
  corporation_id: i64,
  contact_type: Option<&'static str>,
  query: Option<String>,
  sort: ContactSortColumn,
  dir: ContactSortDir,
  after: Option<ContactCursor>,
) -> ContactsPage {
  let labels = if after.is_none() {
    org::corporation_contact_labels(&db, corporation_id)
      .await
      .unwrap_or_default()
  } else {
    Vec::new()
  };

  let rows = org::corporation_contacts_page(
    &db,
    corporation_id,
    contact_type,
    query.as_deref(),
    sort,
    dir,
    after.as_ref(),
    CONTACTS_PAGE_SIZE,
  )
  .await
  .unwrap_or_default();
  let has_more = rows.len() as i64 == CONTACTS_PAGE_SIZE;

  let store = images::default_store();
  let rows: Vec<ContactRow> = rows
    .into_iter()
    .map(|contact| {
      let kind = image_kind(contact.contact_type());
      let image = images::resolve(&store, kind, contact.contact_id());
      ContactRow {
        contact,
        image,
      }
    })
    .collect();
  let cursor = rows.last().map(|row| contact_cursor(sort, row));

  ContactsPage {
    cursor,
    has_more,
    labels,
    rows,
  }
}

async fn load_killlog(db: &Database, corporation_id: i64) -> LoadState<Vec<KillLogEntry>> {
  let rows = match org::corporation_killmails_page(db, corporation_id, None, KILLLOG_PAGE_SIZE).await {
    Ok(rows) => rows,
    Err(error) => return LoadState::Error(error.to_string()),
  };

  LoadState::Loaded(resolve_killlog_entries(db, rows).await)
}

async fn load_killlog_page(db: Database, corporation_id: i64, after: Option<(String, i64)>) -> Vec<KillLogEntry> {
  let rows = match org::corporation_killmails_page(&db, corporation_id, after, KILLLOG_PAGE_SIZE).await {
    Ok(rows) => rows,
    Err(_) => return Vec::new(),
  };

  resolve_killlog_entries(&db, rows).await
}

async fn load_killmail_detail(db: Database, corporation_id: i64, killmail_id: i64) -> Option<KillmailDetail> {
  killmail_loader::load(&db, corporation_id, killmail_id).await
}

async fn resolve_killlog_entries(
  db: &Database,
  rows: Vec<crate::store::model::CorporationKillEntry>,
) -> Vec<KillLogEntry> {
  let mut entries = Vec::with_capacity(rows.len());
  for row in rows {
    entries.push(killlog_entry(db, row).await);
  }
  entries
}

async fn killlog_entry(db: &Database, row: crate::store::model::CorporationKillEntry) -> KillLogEntry {
  let ship_name = killlog_ship_name(db, row.ship_type_id()).await;
  let ship_icon = images::default_store().resolve_type_icon(row.ship_type_id(), None, KILLLOG_SHIP_ICON_SIZE);
  let (system_name, system_security) = killlog_system(db, row.system_id()).await;
  let victim_name = killlog_victim_name(db, row.victim_id()).await;
  let victim_corp = killlog_victim_corp(db, row.victim_corp_id()).await;

  KillLogEntry {
    attacker_count: row.attacker_count(),
    final_blow: row.final_blow(),
    is_kill: row.is_kill(),
    kill_time: row.kill_time().clone(),
    killmail_id: row.killmail_id(),
    ship_icon,
    ship_name,
    ship_type_id: row.ship_type_id(),
    system_name,
    system_security,
    value_destroyed_isk: row.value_destroyed_isk(),
    value_isk: row.value_isk(),
    victim_corp,
    victim_name,
  }
}

async fn killlog_ship_name(db: &Database, ship_type_id: i64) -> String {
  sde::get_item_type(db, ship_type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
    .unwrap_or_else(|| format!("Type {ship_type_id}"))
}

async fn killlog_system(db: &Database, system_id: i64) -> (Option<String>, f64) {
  match sde::get_solar_system(db, system_id).await.ok().flatten() {
    Some(system) => (Some(system.name().clone()), system.security_status()),
    None => (None, 0.0),
  }
}

async fn killlog_victim_name(db: &Database, victim_id: Option<i64>) -> String {
  match victim_id {
    Some(id) => character::get(db, id)
      .await
      .ok()
      .flatten()
      .map(|c| c.name().to_owned())
      .unwrap_or_else(|| format!("Pilot {id}")),
    None => "Unknown".to_owned(),
  }
}

async fn killlog_victim_corp(db: &Database, victim_corp_id: Option<i64>) -> String {
  match victim_corp_id {
    Some(id) => org::get_corporation(db, id)
      .await
      .ok()
      .flatten()
      .map(|c| c.name().to_owned())
      .unwrap_or_else(|| format!("Corp {id}")),
    None => String::new(),
  }
}

async fn load_head(db: &Database, corporation_id: i64) -> Option<CorpHead> {
  let corp = match org::get_corporation(db, corporation_id).await {
    Ok(Some(corp)) => corp,
    Ok(None) => return None,
    Err(error) => {
      tracing::warn!(corporation_id, %error, "failed to load corporation for detail view");
      return None;
    }
  };

  let alliance = head_alliance(db, corp.alliance_id()).await;
  let ceo = head_ceo(db, corp.ceo_id()).await;
  let hq = head_hq(db, corp.home_station_id()).await;
  let store = images::default_store();

  Some(CorpHead {
    alliance,
    ceo,
    corporation_id,
    hq,
    logo: images::resolve(&store, images::ImageKind::CorporationLogo, corporation_id),
    members: corp.member_count().map(i64::from),
    name: corp.name().to_owned(),
    tax_rate: corp.tax_rate(),
    ticker: corp.ticker().to_owned(),
  })
}

async fn head_alliance(db: &Database, alliance_id: Option<i64>) -> Option<String> {
  let alliance_id = alliance_id?;
  org::get_alliance(db, alliance_id)
    .await
    .ok()
    .flatten()
    .map(|alliance| alliance.name().to_owned())
}

async fn head_ceo(db: &Database, ceo_id: Option<i64>) -> Option<String> {
  let ceo_id = ceo_id?;
  character::get(db, ceo_id)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().to_owned())
}

async fn head_hq(db: &Database, station_id: Option<i64>) -> Option<String> {
  let station_id = station_id?;
  sde::get_station(db, station_id)
    .await
    .ok()
    .flatten()
    .map(|station| station.name().to_owned())
}

fn header(state: &State) -> Element<'_, Message> {
  match &state.head {
    Some(head) => loaded_header(head),
    None => header_band(vec![loading_identity()], Vec::new()),
  }
}

fn loaded_header(head: &CorpHead) -> Element<'_, Message> {
  let left: Vec<Element<'_, Message>> = vec![
    identity(head),
    header_divider(),
    stat_block("Members", format_members(head.members), color::text::PRIMARY, None),
    header_divider(),
    stat_block("Tax Rate", format_tax(head.tax_rate), color::text::PRIMARY, None),
    header_divider(),
    stat_block(
      "Alliance",
      head.alliance.clone().unwrap_or_else(placeholder),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      "CEO",
      head.ceo.clone().unwrap_or_else(placeholder),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      "HQ",
      head.hq.clone().unwrap_or_else(placeholder),
      color::text::PRIMARY,
      None,
    ),
  ];

  header_band(left, Vec::new())
}

fn identity(head: &CorpHead) -> Element<'_, Message> {
  let logo = Avatar::new(
    head.corporation_id,
    &head.ticker,
    Length::Fixed(LOGO_SIZE),
    LOGO_SIZE,
    head.logo.path(),
  )
  .border(color::with_alpha(color::text::PRIMARY, 0.1), 1.0)
  .radius(radius::SUBTLE)
  .view::<Message>();

  let name = text(head.name.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));

  let ticker = text(head.ticker.clone())
    .font(typography::mono::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::accent::PLASMA));

  Row::with_children(vec![
    logo,
    Column::with_children(vec![name.into(), ticker.into()])
      .spacing(spacing::UNIT)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .into()
}

fn loading_identity<'a>() -> Element<'a, Message> {
  text("Loading\u{2026}")
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn placeholder() -> String {
  PLACEHOLDER.to_owned()
}

/// Derives the next keyset cursor from the last row of a page, matching the active sort column.
fn contact_cursor(sort: ContactSortColumn, row: &ContactRow) -> ContactCursor {
  let id = row.contact.contact_id();
  match sort {
    ContactSortColumn::Name => ContactCursor::Text(row.contact.contact_name().clone(), id),
    ContactSortColumn::Type => ContactCursor::Text(row.contact.contact_type().clone(), id),
    ContactSortColumn::Standing => ContactCursor::Number(row.contact.standing(), id),
  }
}

fn contact_query_params(state: &State) -> (Option<&'static str>, Option<String>, ContactSortColumn, ContactSortDir) {
  use tabs::contacts::{SortColumn, SortDirection};

  let contact_type = state.contact_filter.contact_type();
  let query = {
    let trimmed = state.contacts_query.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
  };
  let sort = match state.contact_sort.column {
    SortColumn::Entity => ContactSortColumn::Name,
    SortColumn::Standing => ContactSortColumn::Standing,
    SortColumn::Type => ContactSortColumn::Type,
  };
  let dir = match state.contact_sort.direction {
    SortDirection::Ascending => ContactSortDir::Asc,
    SortDirection::Descending => ContactSortDir::Desc,
  };
  (contact_type, query, sort, dir)
}

fn killlog_cursor(entry: &KillLogEntry) -> (String, i64) {
  (entry.kill_time.clone(), entry.killmail_id)
}

/// Re-derives the contacts pagination guards from whatever page is currently loaded. A short page (fewer rows than
/// the page size) is the last page, so `has_more` is false and no further fetch is attempted.
fn reset_contacts_pagination(state: &mut State) {
  let (_, _, sort, _) = contact_query_params(state);
  state.contacts_loading_more = false;
  state.contacts_scroll_offset = 0.0;
  match &state.contacts {
    LoadState::Loaded(page) => {
      state.contacts_has_more = page.has_more;
      state.contacts_cursor = page.rows.last().map(|row| contact_cursor(sort, row));
    }
    _ => {
      state.contacts_has_more = false;
      state.contacts_cursor = None;
    }
  }
}

/// Re-runs the first contacts page after a sort, filter, or search change so the new ordering/facet is applied in
/// SQL (rather than holding the whole address book in memory) and the virtual window snaps back to the top.
fn restart_contacts(state: &mut State, db: &Database) -> Task<Message> {
  state.contacts = LoadState::Loading;
  state.contacts_cursor = None;
  state.contacts_has_more = false;
  state.contacts_loading_more = true;
  state.contacts_scroll_offset = 0.0;
  let (contact_type, query, sort, dir) = contact_query_params(state);
  Task::perform(
    load_contacts_page(db.clone(), state.active, contact_type, query, sort, dir, None),
    |page| Message::ContactsPageLoaded(Box::new(page)),
  )
}

fn reset_killlog_pagination(state: &mut State) {
  state.killlog_loading_more = false;
  state.killlog_scroll_offset = 0.0;
  state.killlog_cursor = match &state.killlog {
    LoadState::Loaded(entries) => entries.last().map(killlog_cursor),
    _ => None,
  };
  state.killlog_has_more = match &state.killlog {
    LoadState::Loaded(entries) => entries.len() as i64 == KILLLOG_PAGE_SIZE,
    _ => false,
  };
}

fn format_members(members: Option<i64>) -> String {
  let Some(value) = members else {
    return PLACEHOLDER.to_owned();
  };
  if value >= 1_000_000 {
    format!("{:.1}M", value as f64 / 1e6)
  } else if value >= 1_000 {
    group_thousands(value)
  } else {
    value.to_string()
  }
}

fn group_thousands(value: i64) -> String {
  let digits = value.to_string();
  let mut grouped = String::new();
  let len = digits.len();
  for (index, ch) in digits.chars().enumerate() {
    if index > 0 && (len - index).is_multiple_of(3) {
      grouped.push('\u{2009}'); // thin space (U+2009) as thousands separator
    }
    grouped.push(ch);
  }
  grouped
}

fn format_tax(tax_rate: Option<f64>) -> String {
  match tax_rate {
    Some(rate) => format!("{:.1}%", rate * 100.0),
    None => PLACEHOLDER.to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn detail() -> CorpDetail {
    CorpDetail {
      contacts: ContactsPage::for_test(Vec::new(), Vec::new(), false),
      head: Some(head()),
      killlog: LoadState::Loaded(Vec::new()),
    }
  }

  fn killmail_detail_fixture() -> KillmailDetail {
    KillmailDetail {
      attackers: Vec::new(),
      damage_taken: 0,
      dropped_isk: 0.0,
      is_kill: true,
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id: 100,
      ship_icon: images::IconResolution::Missing,
      ship_name: "Rifter".to_owned(),
      slots: Vec::new(),
      system_name: Some("Jita".to_owned()),
      system_security: 0.9,
      value_destroyed_isk: 0.0,
      value_isk: 1234.5,
      victim_alliance: None,
      victim_corp: None,
      victim_name: "Target".to_owned(),
      victim_portrait: images::ImageState::Stale {
        id: 3,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn head() -> CorpHead {
    CorpHead {
      alliance: Some("Iron Helix Pact".to_owned()),
      ceo: Some("Vex Voronova".to_owned()),
      corporation_id: 98_000_001,
      hq: Some("Jita IV \u{2014} Moon 4".to_owned()),
      logo: images::ImageState::Stale {
        id: 98_000_001,
        kind: images::ImageKind::CorporationLogo,
      },
      members: Some(1247),
      name: "Cobalt Syndicate".to_owned(),
      tax_rate: Some(0.10),
      ticker: "COBSY".to_owned(),
    }
  }

  fn contacts_page(has_more: bool) -> ContactsPage {
    ContactsPage {
      cursor: Some(ContactCursor::Number(1.0, 5)),
      has_more,
      labels: Vec::new(),
      rows: Vec::new(),
    }
  }

  fn standings_catalog(agent_cursor: Option<(String, i64)>) -> StandingsCatalog {
    StandingsCatalog {
      agent_cursor,
      rows: Vec::new(),
    }
  }

  fn killlog_entry_fixture(killmail_id: i64) -> KillLogEntry {
    KillLogEntry {
      attacker_count: 1,
      final_blow: true,
      is_kill: true,
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id,
      ship_icon: images::IconResolution::Missing,
      ship_name: "Rifter".to_owned(),
      ship_type_id: 587,
      system_name: Some("Jita".to_owned()),
      system_security: 0.9,
      value_destroyed_isk: 0.0,
      value_isk: 1.0,
      victim_corp: String::new(),
      victim_name: "Unknown".to_owned(),
    }
  }

  mod format_members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_count() {
      assert_eq!(format_members(None), PLACEHOLDER);
    }

    #[test]
    fn it_uses_millions_thin_space_thousands_and_raw_figures() {
      assert_eq!(format_members(Some(2_400_000)), "2.4M");
      assert_eq!(format_members(Some(12_400)), "12\u{2009}400");
      assert_eq!(format_members(Some(89)), "89");
    }
  }

  mod format_tax {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_a_one_decimal_percentage() {
      assert_eq!(format_tax(Some(0.10)), "10.0%");
      assert_eq!(format_tax(Some(0.025)), "2.5%");
    }

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_rate() {
      assert_eq!(format_tax(None), PLACEHOLDER);
    }
  }

  mod load_killlog {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{Alliance, Bloodline, Character, Corporation, CorporationKillEntry, Gender, Race};

    const CEO_ID: i64 = 7001;

    const CORP_ID: i64 = 98_000_001;

    async fn seed_corporation(db: &Database) {
      let alliance = Alliance::new(CORP_ID, CORP_ID, CEO_ID, "2020-01-01", "Test Alliance", "TST");
      let mut corp = Corporation::new(CORP_ID, "Cobalt Syndicate", "COBSY");
      corp.set_ceo_id(CEO_ID);
      corp.set_creator_id(CEO_ID);
      corp.set_member_count(100);
      corp.set_tax_rate(0.05);
      let race = Race::new(1, CORP_ID, "A race.", "Test Race");
      let bloodline = Bloodline::new(1, CORP_ID, 1, 3, "A bloodline.", 7, 5, "Test", 6, 4);
      let char = Character::new(CEO_ID, 1, CORP_ID, 1, "1990-01-01", Gender::Male, "Test CEO");
      character::insert_with_org(db, &char, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    fn kill(killmail_id: i64, victim_id: Option<i64>, victim_corp_id: Option<i64>) -> CorporationKillEntry {
      CorporationKillEntry {
        attacker_count: 2,
        corporation_id: CORP_ID,
        final_blow: true,
        is_kill: true,
        kill_hash: format!("hash{killmail_id}"),
        kill_time: "2024-01-01T00:00:00Z".to_owned(),
        killmail_id,
        ship_type_id: 587,
        synced_at: "2024-01-02T00:00:00Z".to_owned(),
        system_id: 30_000_142,
        value_destroyed_isk: 0.0,
        value_final: false,
        value_isk: 1.0,
        value_recheck_count: 0,
        value_source: "local".to_owned(),
        victim_alliance_id: None,
        victim_corp_id,
        victim_damage_taken: 0,
        victim_id,
      }
    }

    #[tokio::test]
    async fn it_loads_the_corporation_head_with_resolved_fields() {
      let db = crate::store::open_test().await.unwrap();
      seed_corporation(&db).await;

      let head = load_head(&db, CORP_ID).await.unwrap();

      assert_eq!(head.name, "Cobalt Syndicate");
      assert_eq!(head.ceo.as_deref(), Some("Test CEO"));
      assert_eq!(head.members, Some(100));
    }

    #[tokio::test]
    async fn it_resolves_each_killmail_into_a_render_ready_entry() {
      let db = crate::store::open_test().await.unwrap();
      seed_corporation(&db).await;
      org::upsert_corporation_killmail(&db, &kill(100, Some(2002), Some(3003)))
        .await
        .unwrap();

      let LoadState::Loaded(entries) = load_killlog(&db, CORP_ID).await else {
        panic!("expected a loaded killlog");
      };

      assert_eq!(entries.len(), 1);
      assert_eq!(entries[0].ship_name, "Type 587");
      assert_eq!(entries[0].system_name, None);
      assert_eq!(entries[0].victim_name, "Pilot 2002");
      assert_eq!(entries[0].victim_corp, "Corp 3003");
    }

    #[tokio::test]
    async fn it_returns_no_head_for_an_unknown_corporation() {
      let db = crate::store::open_test().await.unwrap();

      let head = load_head(&db, CORP_ID).await;

      assert!(head.is_none());
    }

    #[tokio::test]
    async fn it_uses_unknown_placeholders_when_victim_ids_are_absent() {
      let db = crate::store::open_test().await.unwrap();
      seed_corporation(&db).await;
      org::upsert_corporation_killmail(&db, &kill(200, None, None))
        .await
        .unwrap();

      let LoadState::Loaded(entries) = load_killlog(&db, CORP_ID).await else {
        panic!("expected a loaded killlog");
      };

      assert_eq!(entries[0].victim_name, "Unknown");
      assert_eq!(entries[0].victim_corp, "");
    }
  }

  mod load_standings_catalog {
    #[tokio::test]
    async fn it_returns_an_empty_catalog_for_an_unseeded_corporation() {
      let db = crate::store::open_test().await.unwrap();

      let catalog = super::super::load_standings_catalog(&db, 98_000_001, "", false)
        .await
        .unwrap();

      assert!(catalog.rows.is_empty());
      assert!(catalog.agent_cursor.is_none());
    }
  }

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_the_killlog_filter() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::KilllogFilterChanged(KilllogFilter::Kills), &db);

      assert_eq!(state.killlog_filter(), KilllogFilter::Kills);
    }

    #[tokio::test]
    async fn it_opens_and_closes_the_killmail_detail_modal() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(
        &mut state,
        Message::KillmailDetailLoaded(Box::new(Some(killmail_detail_fixture()))),
        &db,
      );
      assert!(state.selected_killmail.is_some());

      let _task = update(&mut state, Message::CloseKillmailDetail, &db);
      assert!(state.selected_killmail.is_none());
    }

    #[test]
    fn it_opens_on_the_first_tab() {
      let state = State::new(98_000_001);

      assert_eq!(state.active(), 98_000_001);
      assert_eq!(state.active_tab, Tab::Contacts);
    }

    #[tokio::test]
    async fn it_reports_a_stale_logo_only_once_loaded() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      assert!(state.stale_images().is_empty());

      let _task = update(&mut state, Message::Loaded(Box::new(detail())), &db);

      assert_eq!(
        state.stale_images(),
        vec![(images::ImageKind::CorporationLogo, 98_000_001)]
      );
    }

    #[tokio::test]
    async fn it_stores_the_loaded_head() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::Loaded(Box::new(detail())), &db);

      assert!(state.head.is_some());
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::TabChanged(Tab::Standings), &db);

      assert_eq!(state.active_tab, Tab::Standings);
    }

    #[tokio::test]
    async fn it_tracks_the_standings_search_query() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(
        &mut state,
        Message::StandingsSearchChanged("faction:caldari".to_owned()),
        &db,
      );

      assert_eq!(state.standings_query(), "faction:caldari");
      assert!(state.standings_has_filters());
    }
  }

  mod update_contacts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_clears_the_search_only_when_a_query_is_present() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::ContactsSearchCleared, &db);
      assert!(matches!(state.contacts, LoadState::Loading) || matches!(state.contacts, LoadState::Loaded(_)));

      let _task = update(&mut state, Message::ContactsSearchChanged("foo".to_owned()), &db);
      state.contacts = LoadState::Loaded(ContactsPage::for_test(Vec::new(), Vec::new(), false));
      let _task = update(&mut state, Message::ContactsSearchCleared, &db);

      assert!(state.contacts_query().is_empty());
      assert!(matches!(state.contacts, LoadState::Loading));
    }

    #[tokio::test]
    async fn it_extends_an_already_loaded_page() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.contacts = LoadState::Loaded(ContactsPage::for_test(Vec::new(), Vec::new(), true));

      let _task = update(
        &mut state,
        Message::ContactsPageLoaded(Box::new(contacts_page(false))),
        &db,
      );

      assert!(!state.contacts_has_more);
    }

    #[tokio::test]
    async fn it_fetches_the_next_contacts_page_on_a_deep_scroll() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.contacts_has_more = true;
      state.contacts_cursor = Some(ContactCursor::Number(1.0, 5));

      let _task = update(
        &mut state,
        Message::ContactsScrolled {
          absolute: 99.0,
          relative: 1.0,
        },
        &db,
      );

      assert!(state.contacts_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_short_scroll_with_no_more_pages() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(
        &mut state,
        Message::ContactsScrolled {
          absolute: 12.0,
          relative: 0.99,
        },
        &db,
      );

      assert_eq!(state.contacts_scroll_offset(), 12.0);
      assert!(!state.contacts_loading_more);
    }

    #[tokio::test]
    async fn it_replaces_the_page_when_the_prior_state_was_loading() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(
        &mut state,
        Message::ContactsPageLoaded(Box::new(contacts_page(true))),
        &db,
      );

      assert!(matches!(state.contacts, LoadState::Loaded(_)));
      assert!(state.contacts_has_more);
      assert!(!state.contacts_loading_more);
    }

    #[tokio::test]
    async fn it_restarts_only_when_the_search_query_actually_changes() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::ContactsSearchChanged(String::new()), &db);
      assert!(matches!(state.contacts, LoadState::Loading));

      state.contacts = LoadState::Loaded(ContactsPage::for_test(Vec::new(), Vec::new(), false));
      let _task = update(&mut state, Message::ContactsSearchChanged(String::new()), &db);
      assert!(matches!(state.contacts, LoadState::Loaded(_)));
    }

    #[tokio::test]
    async fn it_restarts_the_page_on_a_filter_change() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(
        &mut state,
        Message::ContactFilterChanged(tabs::contacts::ContactFilter::Corp),
        &db,
      );

      assert_eq!(state.contact_filter(), tabs::contacts::ContactFilter::Corp);
      assert!(matches!(state.contacts, LoadState::Loading));
      assert!(state.contacts_loading_more);
    }

    #[tokio::test]
    async fn it_restarts_the_page_on_a_sort_change() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      let sort = state.contact_sort().toggled(tabs::contacts::SortColumn::Entity);

      let _task = update(&mut state, Message::ContactSortChanged(sort), &db);

      assert_eq!(state.contact_sort(), sort);
      assert!(matches!(state.contacts, LoadState::Loading));
    }
  }

  mod update_head {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_loads_the_detail_and_resets_pagination() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::Loaded(Box::new(detail())), &db);

      assert!(matches!(state.contacts, LoadState::Loaded(_)));
      assert!(state.head.is_some());
      assert!(!state.contacts_has_more);
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::TabChanged(Tab::Standings), &db);

      assert_eq!(state.active_tab, Tab::Standings);
    }
  }

  mod update_killlog {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_a_loaded_killlog_page() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.killlog = LoadState::Loaded(Vec::new());

      let _task = update(
        &mut state,
        Message::KilllogPageLoaded(vec![killlog_entry_fixture(7)]),
        &db,
      );

      let LoadState::Loaded(entries) = &state.killlog else {
        panic!("expected a loaded killlog");
      };
      assert_eq!(entries.len(), 1);
      assert!(!state.killlog_has_more);
      assert!(!state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_fetches_the_next_killlog_page_on_a_deep_scroll() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.killlog_has_more = true;
      state.killlog_cursor = Some(("2024-01-01T00:00:00Z".to_owned(), 7));

      let _task = update(
        &mut state,
        Message::KilllogScrolled {
          absolute: 80.0,
          relative: 1.0,
        },
        &db,
      );

      assert!(state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_short_killlog_scroll() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(
        &mut state,
        Message::KilllogScrolled {
          absolute: 5.0,
          relative: 0.1,
        },
        &db,
      );

      assert_eq!(state.killlog_scroll_offset(), 5.0);
      assert!(!state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_selects_a_killmail_for_detail() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::KillmailSelected(42), &db);

      assert!(state.selected_killmail.is_none());
    }
  }

  mod update_standings {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_an_agent_page_for_the_current_generation() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.standings = LoadState::Loaded(Vec::new());
      let page = StandingsAgentsPage {
        generation: state.standings_generation,
        next_cursor: Some(("a".to_owned(), 1)),
        rows: Vec::new(),
      };

      let _task = update(&mut state, Message::StandingsAgentsPageLoaded(Box::new(page)), &db);

      assert!(state.standings_has_more);
      assert!(!state.standings_loading_more);
    }

    #[tokio::test]
    async fn it_clears_the_search_and_reruns() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.standings_query = "caldari".to_owned();
      let before = state.standings_generation;

      let _task = update(&mut state, Message::StandingsClearSearch, &db);

      assert!(state.standings_query().is_empty());
      assert_eq!(state.standings_generation, before.wrapping_add(1));
    }

    #[tokio::test]
    async fn it_discards_a_stale_generation_agent_page() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.standings_has_more = false;
      let page = StandingsAgentsPage {
        generation: state.standings_generation.wrapping_add(9),
        next_cursor: Some(("a".to_owned(), 1)),
        rows: Vec::new(),
      };

      let _task = update(&mut state, Message::StandingsAgentsPageLoaded(Box::new(page)), &db);

      assert!(!state.standings_has_more);
    }

    #[tokio::test]
    async fn it_fetches_the_next_agent_page_on_a_deep_scroll() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.standings_filter = tabs::standings::StandingsFilter::Agents;
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("a".to_owned(), 1));

      let _task = update(
        &mut state,
        Message::StandingsScrolled {
          absolute: 90.0,
          relative: 1.0,
        },
        &db,
      );

      assert!(state.standings_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_an_in_memory_filter_change_with_agents_loaded() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      let before = state.standings_generation;

      let _task = update(
        &mut state,
        Message::StandingsFilterChanged(tabs::standings::StandingsFilter::Factions),
        &db,
      );

      assert_eq!(state.standings_filter(), tabs::standings::StandingsFilter::Factions);
      assert_eq!(state.standings_generation, before);
    }

    #[tokio::test]
    async fn it_loads_results_for_the_current_generation() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      let results = StandingsResult {
        generation: state.standings_generation,
        result: Ok(standings_catalog(Some(("a".to_owned(), 1)))),
      };

      let _task = update(&mut state, Message::StandingsResults(Box::new(results)), &db);

      assert!(matches!(state.standings, LoadState::Loaded(_)));
      assert!(state.standings_has_more);
    }

    #[tokio::test]
    async fn it_records_an_error_result() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      let results = StandingsResult {
        generation: state.standings_generation,
        result: Err("boom".to_owned()),
      };

      let _task = update(&mut state, Message::StandingsResults(Box::new(results)), &db);

      assert!(matches!(state.standings, LoadState::Error(_)));
      assert!(!state.standings_has_more);
    }

    #[tokio::test]
    async fn it_skips_a_standings_scroll_under_a_non_agent_filter() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      state.standings_filter = tabs::standings::StandingsFilter::Factions;
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("a".to_owned(), 1));

      let _task = update(
        &mut state,
        Message::StandingsScrolled {
          absolute: 90.0,
          relative: 1.0,
        },
        &db,
      );

      assert!(!state.standings_loading_more);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_loaded_header_and_each_tab_body() {
      let mut state = State::new(98_000_001);
      state.head = Some(head());

      for tab in Tab::ORDER {
        state.active_tab = tab;
        let _el: Element<'_, Message> = view(&state);
      }
    }

    #[test]
    fn it_renders_the_loading_header_before_data_arrives() {
      let state = State::new(98_000_001);

      let _el: Element<'_, Message> = view(&state);
    }
  }
}
