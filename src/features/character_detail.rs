mod header;
mod killmail_loader;
mod tabs;

use std::{collections::HashSet, time::Duration};

use iced::{
  Element, Length, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Stack, container, operation, text},
};

pub use self::tabs::Tab;
use self::tabs::{
  contacts::ContactRow,
  killlog::{KillLogEntry, KilllogFilter},
  notifications::NotificationsFilter,
};
pub use crate::store::repo::standings::CatalogKind as StandingKind;
use crate::{
  clients::eve_image::Size,
  config::Feature,
  features::killmail_detail::{self, KillmailDetail},
  store::{
    Database, images,
    model::{
      CharacterContact, CharacterContactLabel, CharacterNotification, CharacterState, OwnerType,
      character_clone_view::CharacterClones, character_contacts_view::image_kind,
    },
    repo::{
      character::{self, ContactCursor, ContactSortColumn, ContactSortDir},
      infra, org, sde, standings,
    },
  },
  sync::{JobKey, JobKind, Subject},
  ui::{
    components::{
      backdrop,
      positioned_dropdown::{positioned_dropdown, positioned_dropdown_right},
    },
    style::{color, spacing, typography},
  },
};

pub(crate) const STANDINGS_SEARCH_INPUT_ID: &str = "standings-search-input";

const CONTACTS_PAGE_SIZE: i64 = 100;

const HEADER_SIDE_PADDING: f32 = 28.0;

const KILLLOG_PAGE_SIZE: i64 = 100;

const KILLLOG_SHIP_ICON_SIZE: Size = Size::S64;

const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;

const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;

const SCROLL_THRESHOLD: f32 = 0.85;

const SEARCH_DEBOUNCE_MS: u64 = 200;

const STANDINGS_PAGE_SIZE: i64 = 100;

const STANDINGS_HELP_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + TAB_STRIP_OVERLAY_OFFSET;

const TAB_STRIP_OVERLAY_OFFSET: f32 = 96.0;

/// One keyset page of render-ready contact rows plus the per-character label lookup. The labels travel with the
/// first page so the address-book notes can resolve label ids without a second query per page.
#[derive(Clone, Debug)]
pub struct ContactsPage {
  cursor: Option<ContactCursor>,
  has_more: bool,
  labels: Vec<CharacterContactLabel>,
  rows: Vec<ContactRow>,
}

impl ContactsPage {
  /// Builds a page directly from render-ready rows and labels. Used by the tab's view tests, which assert on
  /// layout rather than the keyset cursor (so the cursor is derived as `None`).
  #[cfg(test)]
  pub(in crate::features::character_detail) fn for_test(
    rows: Vec<ContactRow>,
    labels: Vec<CharacterContactLabel>,
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

  pub(super) fn labels(&self) -> &[CharacterContactLabel] {
    &self.labels
  }

  pub(super) fn rows(&self) -> &[ContactRow] {
    &self.rows
  }
}

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
      JobKind::CharacterKillmails | JobKind::KillmailDetailBackfill => Some(Self::Killlog),
      JobKind::CharacterNotifications => Some(Self::Notifications),
      JobKind::CharacterStandings => Some(Self::Standings),
      JobKind::AssetSync
      | JobKind::CharacterAbyssals
      | JobKind::CharacterBlueprints
      | JobKind::CharacterCalendar
      | JobKind::CharacterContracts
      | JobKind::CharacterIndustryJobs
      | JobKind::CharacterMail
      | JobKind::CharacterMarketOrders
      | JobKind::CharacterProfile
      | JobKind::CharacterSkills
      | JobKind::CharacterTelemetry
      | JobKind::CharacterWallet
      | JobKind::CorporationAbyssals
      | JobKind::CorporationBlueprints
      | JobKind::CorporationContacts
      | JobKind::CorporationContracts
      | JobKind::CorporationIndustryJobs
      | JobKind::CorporationKillmails
      | JobKind::CorporationMiningExtractions
      | JobKind::CorporationProfile
      | JobKind::CorporationStandings
      | JobKind::CorporationStructures
      | JobKind::CorporationWallet
      | JobKind::IndustryCostIndices
      | JobKind::KillmailReconcile
      | JobKind::MarketPrices
      | JobKind::NetWorthSnapshot
      | JobKind::TokenAudit => None,
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
pub enum LoadState<T> {
  Error(String),
  Loaded(T),
  Loading,
}

#[derive(Clone, Debug)]
pub struct Loaded {
  pub clones: LoadState<Option<CharacterClones>>,
  pub contacts: LoadState<ContactsPage>,
  pub granted_scopes: Option<String>,
  pub head: HeadStats,
  pub killlog: LoadState<Vec<KillLogEntry>>,
  pub notifications: LoadState<Vec<CharacterNotification>>,
  pub roster: Vec<PickerPilot>,
}

#[derive(Clone, Debug)]
pub enum Message {
  CharacterChanged(i64),
  CloseKillmailDetail,
  ContactAddOpened,
  ContactDeleteCancelled,
  ContactDeleteConfirmed,
  ContactDeleteRequested(Box<CharacterContact>),
  ContactDeleted(Result<(), String>),
  ContactEditOpened(Box<CharacterContact>),
  ContactEntityChanged(Option<crate::ui::components::entity_search::EntityRef>),
  ContactEntityInput(String),
  ContactEntityResults {
    generation: u64,
    results: Vec<crate::ui::components::entity_search::EntityRef>,
  },
  ContactFilterChanged(tabs::contacts::ContactFilter),
  ContactLabelToggled(i64),
  ContactModalClosed,
  ContactModalSubmitted,
  ContactSortChanged(tabs::contacts::ContactSort),
  ContactStandingChanged(f64),
  ContactSubmitted(Result<(), String>),
  ContactWatchToggled,
  ContactsPageLoaded(Box<ContactsPage>),
  ContactsScrolled {
    absolute: f32,
    relative: f32,
  },
  ContactsSearchChanged(String),
  ContactsSearchCleared,
  FeaturesChanged(Vec<Feature>),
  KilllogFilterChanged(KilllogFilter),
  KilllogPageLoaded(Vec<KillLogEntry>),
  KilllogScrolled {
    absolute: f32,
    relative: f32,
  },
  KillmailDetailLoaded(Box<Option<KillmailDetail>>),
  KillmailSelected(i64),
  Loaded(Box<Loaded>),
  NotificationRead(i64),
  NotificationsFilterChanged(NotificationsFilter),
  PickerToggled,
  ReauthRequested(i64),
  Reloaded(Box<Reloaded>),
  StandingsAgentsPageLoaded(Box<StandingsAgentsPage>),
  StandingsClearSearch,
  StandingsFilterChanged(tabs::standings::StandingsFilter),
  StandingsInsertQuery(String),
  StandingsResults(Box<StandingsResult>),
  StandingsScrolled {
    absolute: f32,
    relative: f32,
  },
  StandingsSearchChanged(String),
  StandingsToggleHelp,
  TabChanged(Tab),
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows (portraits, logos, standings/contact
  /// avatars, killmail detail), so the shell should recheck for stale images. Interaction-only messages return
  /// `false` to keep the staleness scan off the per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(
      self,
      Message::ContactAddOpened
        | Message::ContactEditOpened(_)
        | Message::ContactEntityChanged(_)
        | Message::ContactsPageLoaded(_)
        | Message::KillmailDetailLoaded(_)
        | Message::Loaded(_)
        | Message::Reloaded(_)
        | Message::StandingsAgentsPageLoaded(_)
        | Message::StandingsResults(_)
    )
  }
}

#[derive(Clone, Debug)]
pub struct PickerPilot {
  pub corp: String,
  // Plumbed through for the picker re-auth UX; loaded but not yet read by the picker view.
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
  Contacts(LoadState<ContactsPage>),
  Killlog(LoadState<Vec<KillLogEntry>>),
  Notifications(LoadState<Vec<CharacterNotification>>),
  /// Payload-less, unlike the other variants: a standings reload re-runs the catalog query (preserving the active
  /// search) rather than carrying rows.
  Standings,
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

#[derive(Debug)]
pub struct State {
  active: i64,
  active_tab: Tab,
  clones: LoadState<Option<CharacterClones>>,
  contact_delete: Option<tabs::contact_modal::DeleteConfirm>,
  contact_filter: tabs::contacts::ContactFilter,
  contact_modal: Option<tabs::contact_modal::ContactModal>,
  contact_search_generation: u64,
  contact_sort: tabs::contacts::ContactSort,
  contacts: LoadState<ContactsPage>,
  contacts_cursor: Option<ContactCursor>,
  contacts_has_more: bool,
  contacts_loading_more: bool,
  contacts_query: String,
  contacts_scroll_offset: f32,
  dirty: HashSet<DetailDataType>,
  enabled_tabs: Vec<Tab>,
  granted_scopes: Option<String>,
  head: HeadStats,
  killlog: LoadState<Vec<KillLogEntry>>,
  killlog_cursor: Option<(String, i64)>,
  killlog_filter: KilllogFilter,
  killlog_has_more: bool,
  killlog_loading_more: bool,
  killlog_scroll_offset: f32,
  notifications: LoadState<Vec<CharacterNotification>>,
  notifications_filter: NotificationsFilter,
  picker_open: bool,
  roster: Vec<PickerPilot>,
  selected_killmail: Option<KillmailDetail>,
  standings: LoadState<Vec<StandingsRow>>,
  standings_agent_cursor: Option<(String, i64)>,
  standings_filter: tabs::standings::StandingsFilter,
  standings_generation: u64,
  standings_has_more: bool,
  standings_help_open: bool,
  standings_loading_more: bool,
  standings_query: String,
  standings_scroll_offset: f32,
}

impl State {
  pub fn new(active: i64, features: &[Feature]) -> Self {
    let enabled_tabs = tabs::enabled_tabs(features);
    let active_tab = tabs::resolve_first_tab(&enabled_tabs);
    State {
      active,
      active_tab,
      clones: LoadState::Loading,
      contact_delete: None,
      contact_modal: None,
      contact_search_generation: 0,
      contacts: LoadState::Loading,
      contact_filter: tabs::contacts::ContactFilter::All,
      contact_sort: tabs::contacts::ContactSort::default(),
      contacts_cursor: None,
      contacts_has_more: false,
      contacts_loading_more: false,
      contacts_query: String::new(),
      contacts_scroll_offset: 0.0,
      dirty: HashSet::new(),
      enabled_tabs,
      granted_scopes: None,
      head: HeadStats::default(),
      killlog: LoadState::Loading,
      killlog_cursor: None,
      killlog_filter: KilllogFilter::All,
      killlog_has_more: false,
      killlog_loading_more: false,
      killlog_scroll_offset: 0.0,
      notifications: LoadState::Loading,
      notifications_filter: NotificationsFilter::All,
      picker_open: false,
      roster: Vec::new(),
      selected_killmail: None,
      standings: LoadState::Loading,
      standings_agent_cursor: None,
      standings_filter: tabs::standings::StandingsFilter::All,
      standings_generation: 0,
      standings_has_more: false,
      standings_help_open: false,
      standings_loading_more: false,
      standings_query: String::new(),
      standings_scroll_offset: 0.0,
    }
  }

  pub fn active(&self) -> i64 {
    self.active
  }

  pub fn drain_dirty(&mut self, db: &Database) -> Option<Task<Message>> {
    if self.dirty.is_empty() {
      return None;
    }
    let data_types = std::mem::take(&mut self.dirty);
    let active = self.active;
    Some(Task::batch(
      data_types.into_iter().map(|data_type| reload(db, active, data_type)),
    ))
  }

  #[cfg(test)]
  pub fn enabled_tabs(&self) -> &[Tab] {
    &self.enabled_tabs
  }

  #[cfg(test)]
  pub fn is_dirty(&self) -> bool {
    !self.dirty.is_empty()
  }

  pub fn mark_dirty(&mut self, key: JobKey) {
    if key.subject != Subject::Character(self.active) {
      return;
    }
    if let Some(data_type) = DetailDataType::for_job_kind(key.kind) {
      self.dirty.insert(data_type);
    }
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
    if let LoadState::Loaded(page) = &self.contacts {
      keys.extend(page.rows.iter().filter_map(|row| row.image.stale_key()));
    }
    if let Some(detail) = &self.selected_killmail {
      keys.extend(detail.stale_images());
    }
    if let Some(modal) = &self.contact_modal {
      keys.extend(modal.stale_key());
    }
    keys
  }

  pub(super) fn sync_features(&mut self, features: &[Feature]) {
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

  pub(super) fn contact_delete(&self) -> Option<&tabs::contact_modal::DeleteConfirm> {
    self.contact_delete.as_ref()
  }

  /// The character's in-game contact labels, carried alongside the first loaded contacts page. Handed to the
  /// add/edit modal so its label chips reflect the real ESI-synced labels rather than a fixed list.
  pub(super) fn contact_label_catalog(&self) -> Vec<CharacterContactLabel> {
    match &self.contacts {
      LoadState::Loaded(page) => page.labels().to_vec(),
      _ => Vec::new(),
    }
  }

  /// Names of already-loaded contacts, used to dedupe the add picker. Only the current keyset page is visible, so a
  /// duplicate add for an off-page contact still reconciles on the next sync.
  pub(super) fn contact_exclude_names(&self) -> Vec<String> {
    match &self.contacts {
      LoadState::Loaded(page) => page.rows.iter().map(|row| row.contact.contact_name().clone()).collect(),
      _ => Vec::new(),
    }
  }

  pub(super) fn contact_modal(&self) -> Option<&tabs::contact_modal::ContactModal> {
    self.contact_modal.as_ref()
  }

  pub fn contact_search_generation(&self) -> u64 {
    self.contact_search_generation
  }

  pub(super) fn contacts_query(&self) -> &str {
    &self.contacts_query
  }

  pub(super) fn contacts_scroll_offset(&self) -> f32 {
    self.contacts_scroll_offset
  }

  pub(super) fn contacts_write_enabled(&self) -> bool {
    crate::ui::components::forbidden::missing_scopes(
      self.granted_scopes(),
      &[crate::clients::esi::scopes::CHARACTER_CONTACTS_WRITE],
    )
    .is_empty()
  }

  pub(super) fn granted_scopes(&self) -> Option<&str> {
    self.granted_scopes.as_deref()
  }

  pub(super) fn killlog_scroll_offset(&self) -> f32 {
    self.killlog_scroll_offset
  }

  pub(super) fn standings_scroll_offset(&self) -> f32 {
    self.standings_scroll_offset
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

/// JSON shape serialized into the outbox payload (nested under `target`/`previous` by the enqueue fns) and consumed by
/// the sync contact handlers. The field set is a contract: `label_ids` is a flat `Vec<i64>` and `is_blocked` is absent
/// (writes never block).
#[derive(Clone, serde::Serialize)]
struct ContactPayload {
  contact_id: i64,
  contact_name: String,
  contact_type: String,
  label_ids: Vec<i64>,
  standing: f64,
  watched: bool,
}

impl ContactPayload {
  fn from_contact(contact: &CharacterContact) -> Self {
    ContactPayload {
      contact_id: contact.contact_id(),
      contact_name: contact.contact_name().clone(),
      contact_type: contact.contact_type().clone(),
      label_ids: serde_json::from_str(contact.label_ids()).unwrap_or_default(),
      standing: contact.standing(),
      watched: contact.is_watched(),
    }
  }

  fn into_contact(self, character_id: i64) -> Result<CharacterContact, String> {
    Ok(CharacterContact {
      character_id,
      contact_id: self.contact_id,
      contact_name: self.contact_name,
      contact_type: self.contact_type,
      is_blocked: false,
      is_watched: self.watched,
      label_ids: serde_json::to_string(&self.label_ids).map_err(|error| error.to_string())?,
      standing: self.standing,
    })
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
      restart_contacts(state, db)
    }
    Message::ContactSortChanged(sort) => {
      state.contact_sort = sort;
      restart_contacts(state, db)
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
    Message::FeaturesChanged(features) => {
      state.sync_features(&features);
      Task::none()
    }
    Message::ContactAddOpened
    | Message::ContactDeleteCancelled
    | Message::ContactDeleteConfirmed
    | Message::ContactDeleteRequested(_)
    | Message::ContactDeleted(_)
    | Message::ContactEditOpened(_)
    | Message::ContactEntityChanged(_)
    | Message::ContactEntityInput(_)
    | Message::ContactEntityResults {
      ..
    }
    | Message::ContactLabelToggled(_)
    | Message::ContactModalClosed
    | Message::ContactModalSubmitted
    | Message::ContactStandingChanged(_)
    | Message::ContactSubmitted(_)
    | Message::ContactWatchToggled => update_contacts_modal(state, message, db),
    Message::ContactsPageLoaded(_)
    | Message::ContactsScrolled {
      ..
    }
    | Message::KilllogPageLoaded(_)
    | Message::KilllogScrolled {
      ..
    }
    | Message::StandingsAgentsPageLoaded(_)
    | Message::StandingsScrolled {
      ..
    } => update_pagination(state, message, db),
    Message::CloseKillmailDetail
    | Message::KilllogFilterChanged(_)
    | Message::KillmailDetailLoaded(_)
    | Message::KillmailSelected(_)
    | Message::NotificationRead(_)
    | Message::NotificationsFilterChanged(_) => update_killlog(state, message, db),
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
      reset_contacts_pagination(state);
      state.granted_scopes = granted_scopes;
      state.head = head;
      state.killlog = killlog;
      reset_killlog_pagination(state);
      state.notifications = notifications;
      state.roster = roster;
      if let Some(modal) = state.contact_modal.as_mut() {
        modal.refresh_image();
      }
      trigger_standings_search(state, db)
    }
    Message::Reloaded(reloaded) => match *reloaded {
      Reloaded::Clones(clones) => {
        state.clones = clones;
        Task::none()
      }
      Reloaded::Contacts(contacts) => {
        state.contacts = contacts;
        reset_contacts_pagination(state);
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
    Message::StandingsClearSearch
    | Message::StandingsFilterChanged(_)
    | Message::StandingsInsertQuery(_)
    | Message::StandingsResults(_)
    | Message::StandingsSearchChanged(_)
    | Message::StandingsToggleHelp => update_standings(state, message, db),
    Message::TabChanged(tab) => {
      state.active_tab = tab;
      Task::none()
    }
  }
}

/// Kill-log and notification message arms split out of [`update`] to keep its cyclomatic complexity in check.
fn update_killlog(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::KilllogFilterChanged(filter) => {
      state.killlog_filter = filter;
      Task::none()
    }
    Message::KillmailSelected(killmail_id) => {
      let viewing = state.active;
      Task::perform(load_killmail_detail(db.clone(), viewing, killmail_id), |detail| {
        Message::KillmailDetailLoaded(Box::new(detail))
      })
    }
    Message::KillmailDetailLoaded(detail) => {
      state.selected_killmail = *detail;
      Task::none()
    }
    Message::CloseKillmailDetail => {
      state.selected_killmail = None;
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
    _ => Task::none(),
  }
}

/// Standings search/filter/help message arms split out of [`update`] to keep its cyclomatic complexity in check.
fn update_standings(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
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
    _ => Task::none(),
  }
}

fn update_pagination(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ContactsPageLoaded(page) => {
      let ContactsPage {
        cursor,
        has_more,
        labels,
        rows,
      } = *page;
      state.contacts_loading_more = false;
      state.contacts_has_more = has_more;
      state.contacts_cursor = cursor.clone();
      // Extend the existing page on a scroll-driven fetch; replace it outright when the prior state was Loading
      // (a fresh first page from initial load, reload, or a sort/filter restart).
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
      Task::none()
    }
    Message::ContactsScrolled {
      absolute,
      relative,
    } => {
      // The shared scrollbar also routes the short Clones/Notifications tabs here; only track the offset and
      // paginate when Contacts is actually the active tab so their scrolling can't disturb its window.
      if state.active_tab != Tab::Contacts {
        return Task::none();
      }
      state.contacts_scroll_offset = absolute;
      if relative < SCROLL_THRESHOLD || !state.contacts_has_more || state.contacts_loading_more {
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
      if relative < SCROLL_THRESHOLD || !state.killlog_has_more || state.killlog_loading_more {
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
    Message::StandingsScrolled {
      absolute,
      relative,
    } => {
      state.standings_scroll_offset = absolute;
      // Only the agent-surfacing filters paginate agents; under Factions/Corps/Other a forced-false page
      // would come back empty and clobber `standings_has_more`, so skip the fetch entirely.
      if relative < SCROLL_THRESHOLD
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

fn update_contacts_modal(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  use tabs::contact_modal::{ContactModal, DeleteConfirm};

  match message {
    Message::ContactAddOpened => {
      let exclude = state.contact_exclude_names();
      let catalog = state.contact_label_catalog();
      state.contact_modal = Some(ContactModal::add(exclude, catalog));
      Task::none()
    }
    Message::ContactEditOpened(contact) => {
      let catalog = state.contact_label_catalog();
      state.contact_modal = Some(ContactModal::edit(&contact, catalog));
      Task::none()
    }
    Message::ContactModalClosed => {
      state.contact_modal = None;
      Task::none()
    }
    Message::ContactEntityInput(query) => {
      if let Some(modal) = state.contact_modal.as_mut() {
        state.contact_search_generation = modal.set_query(query);
      }
      Task::none()
    }
    Message::ContactEntityResults {
      generation,
      results,
    } => {
      if let Some(modal) = state.contact_modal.as_mut() {
        modal.accept_results(generation, results);
      }
      Task::none()
    }
    Message::ContactEntityChanged(entity) => {
      if let Some(modal) = state.contact_modal.as_mut() {
        modal.set_entity(entity);
      }
      Task::none()
    }
    Message::ContactStandingChanged(standing) => {
      if let Some(modal) = state.contact_modal.as_mut() {
        modal.set_standing(standing);
      }
      Task::none()
    }
    Message::ContactLabelToggled(label_id) => {
      if let Some(modal) = state.contact_modal.as_mut() {
        modal.toggle_label(label_id);
      }
      Task::none()
    }
    Message::ContactWatchToggled => {
      if let Some(modal) = state.contact_modal.as_mut() {
        modal.toggle_watch();
      }
      Task::none()
    }
    Message::ContactModalSubmitted => submit_contact(state, db),
    Message::ContactSubmitted(result) => {
      match result {
        Ok(()) => state.contact_modal = None,
        Err(error) => {
          tracing::warn!(target: "pod::character_detail", %error, "contact submit failed to enqueue")
        }
      }
      Task::none()
    }
    Message::ContactDeleteRequested(contact) => {
      state.contact_delete = Some(DeleteConfirm {
        contact: *contact,
      });
      Task::none()
    }
    Message::ContactDeleteCancelled => {
      state.contact_delete = None;
      Task::none()
    }
    Message::ContactDeleteConfirmed => {
      let Some(confirm) = state.contact_delete.take() else {
        return Task::none();
      };
      let character_id = state.active;
      Task::perform(
        enqueue_contact_remove(db.clone(), character_id, confirm.contact),
        Message::ContactDeleted,
      )
    }
    Message::ContactDeleted(result) => {
      if let Err(error) = result {
        tracing::warn!(target: "pod::character_detail", %error, "contact remove failed to enqueue");
      }
      reload(db, state.active, DetailDataType::Contacts)
    }
    _ => Task::none(),
  }
}

fn submit_contact(state: &mut State, db: &Database) -> Task<Message> {
  let Some(modal) = state.contact_modal.as_ref() else {
    return Task::none();
  };
  let Some(entity) = modal.entity() else {
    return Task::none();
  };

  let character_id = state.active;
  let target = ContactPayload {
    contact_id: entity.id,
    contact_name: entity.name.clone(),
    contact_type: entity_kind_str(entity.kind).to_owned(),
    label_ids: modal.labels().to_vec(),
    standing: modal.standing(),
    watched: modal.watch(),
  };

  // `previous` is derived only from loaded rows; a contact paginated off-page yields None, in which case the edit's
  // compensation falls back to `target` (see enqueue_contact_edit).
  let previous = modal
    .is_edit()
    .then(|| existing_contact(&state.contacts, target.contact_id).map(ContactPayload::from_contact));

  let submit = if modal.is_edit() {
    let previous = previous.flatten();
    Task::perform(
      enqueue_contact_edit(db.clone(), character_id, target, previous),
      Message::ContactSubmitted,
    )
  } else {
    Task::perform(
      enqueue_contact_add(db.clone(), character_id, target),
      Message::ContactSubmitted,
    )
  };

  Task::batch([submit, reload(db, character_id, DetailDataType::Contacts)])
}

fn entity_kind_str(kind: crate::ui::components::entity_search::EntityKind) -> &'static str {
  use crate::ui::components::entity_search::EntityKind;
  match kind {
    EntityKind::Alliance => "alliance",
    EntityKind::Character => "character",
    EntityKind::Corporation => "corporation",
    EntityKind::SolarSystem => "solar_system",
    EntityKind::Station => "station",
  }
}

fn existing_contact(contacts: &LoadState<ContactsPage>, contact_id: i64) -> Option<&CharacterContact> {
  match contacts {
    LoadState::Loaded(page) => page
      .rows
      .iter()
      .map(|row| &row.contact)
      .find(|contact| contact.contact_id() == contact_id),
    _ => None,
  }
}

/// Mirrors the contact into the local store before appending the outbox row, so the new row shows immediately. The
/// `"contact.add"` kind string must match the sync handler registry.
async fn enqueue_contact_add(db: Database, character_id: i64, target: ContactPayload) -> Result<(), String> {
  let json = serde_json::json!({ "character_id": character_id, "target": &target }).to_string();
  character::upsert_contact(&db, &target.clone().into_contact(character_id)?)
    .await
    .map_err(|error| error.to_string())?;
  infra::append(&db, OwnerType::Character, character_id, "contact.add", &json, None)
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

async fn enqueue_contact_edit(
  db: Database,
  character_id: i64,
  target: ContactPayload,
  previous: Option<ContactPayload>,
) -> Result<(), String> {
  let previous = previous.unwrap_or_else(|| target.clone());
  let json = serde_json::json!({ "character_id": character_id, "previous": &previous, "target": &target }).to_string();
  character::upsert_contact(&db, &target.clone().into_contact(character_id)?)
    .await
    .map_err(|error| error.to_string())?;
  infra::append(&db, OwnerType::Character, character_id, "contact.edit", &json, None)
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

async fn enqueue_contact_remove(db: Database, character_id: i64, contact: CharacterContact) -> Result<(), String> {
  let previous = ContactPayload::from_contact(&contact);
  let json = serde_json::json!({ "character_id": character_id, "previous": &previous }).to_string();
  character::delete_contact(&db, character_id, contact.contact_id())
    .await
    .map_err(|error| error.to_string())?;
  infra::append(&db, OwnerType::Character, character_id, "contact.remove", &json, None)
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// Translates the active contact filter, name search, and sort header into the repo's page parameters. The search
/// term is pushed into SQL (rather than filtering an in-memory set) so keyset pagination keeps working over the
/// filtered result.
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

/// Derives the next keyset cursor from the last row of a page, matching the active sort column.
fn contact_cursor(sort: ContactSortColumn, row: &ContactRow) -> ContactCursor {
  let id = row.contact.contact_id();
  match sort {
    ContactSortColumn::Name => ContactCursor::Text(row.contact.contact_name().clone(), id),
    ContactSortColumn::Type => ContactCursor::Text(row.contact.contact_type().clone(), id),
    ContactSortColumn::Standing => ContactCursor::Number(row.contact.standing(), id),
  }
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

/// Re-runs the first contacts page after a sort or filter change so the new ordering/facet is applied in SQL
/// (rather than holding the whole address book in memory) and the virtual window snaps back to the top.
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

  if let Some(detail) = state.selected_killmail.as_ref() {
    return killmail_detail::overlay(base.into(), detail, Message::CloseKillmailDetail);
  }

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

  if let Some(confirm) = state.contact_delete() {
    return Stack::with_children(vec![
      base.into(),
      backdrop::backdrop(Message::ContactDeleteCancelled),
      tabs::contact_modal::delete_confirm(confirm),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  if let Some(modal) = state.contact_modal() {
    return Stack::with_children(vec![
      base.into(),
      backdrop::backdrop(Message::ContactModalClosed),
      tabs::contact_modal::modal(modal),
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
  let contacts = load_contacts(&db, character_id).await;
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

/// Loads the first contacts page under the default sort/filter (the values `State::new` starts with): standing
/// descending, all entity types. The page carries the per-character labels.
async fn load_contacts(db: &Database, character_id: i64) -> LoadState<ContactsPage> {
  LoadState::Loaded(
    load_contacts_page(
      db.clone(),
      character_id,
      None,
      None,
      ContactSortColumn::Standing,
      ContactSortDir::Desc,
      None,
    )
    .await,
  )
}

/// Fetches and image-resolves one keyset page of contacts. The returned page carries the next cursor (when the page
/// filled to the page size). A first page (`after` is `None`) also carries the per-character labels, so the initial
/// load and every sort/filter restart bring the address-book labels along; follow-on pages return an empty label set
/// and the caller keeps the labels it already holds.
async fn load_contacts_page(
  db: Database,
  character_id: i64,
  contact_type: Option<&'static str>,
  query: Option<String>,
  sort: ContactSortColumn,
  dir: ContactSortDir,
  after: Option<ContactCursor>,
) -> ContactsPage {
  let labels = if after.is_none() {
    character::contact_labels(&db, character_id).await.unwrap_or_default()
  } else {
    Vec::new()
  };

  let rows = character::contacts_page(
    &db,
    character_id,
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

async fn load_killmail_detail(db: Database, character_id: i64, killmail_id: i64) -> Option<KillmailDetail> {
  killmail_loader::load(&db, character_id, killmail_id, character_id).await
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
    let ship_icon = images::default_store().resolve_type_icon(row.ship_type_id(), None, KILLLOG_SHIP_ICON_SIZE);

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
      ship_icon,
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
    DetailDataType::Contacts => Reloaded::Contacts(load_contacts(&db, character_id).await),
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
      victim_alliance_id: None,
      victim_corp_id,
      victim_damage_taken: 0,
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
      scopes::CHARACTER_CONTACTS_WRITE,
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

  fn killmail_detail_fixture() -> killmail_detail::KillmailDetail {
    killmail_detail::KillmailDetail {
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

  mod contacts_modal {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ui::components::entity_search::{EntityKind, EntityRef};

    fn entity(id: i64, kind: EntityKind, name: &str) -> EntityRef {
      EntityRef {
        id,
        kind,
        name: name.to_owned(),
        portrait: None,
      }
    }

    fn contact(id: i64, kind: &str, name: &str, standing: f64, watched: bool, label_ids: &str) -> CharacterContact {
      CharacterContact {
        character_id: 42,
        contact_id: id,
        contact_name: name.to_owned(),
        contact_type: kind.to_owned(),
        is_blocked: false,
        is_watched: watched,
        label_ids: label_ids.to_owned(),
        standing,
      }
    }

    async fn outbox_count(db: &Database, kind: &str) -> i64 {
      sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind = ? AND subject_id = 42")
        .bind(kind)
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    async fn contact_present(db: &Database, contact_id: i64) -> bool {
      character::contacts(db, 42)
        .await
        .unwrap()
        .contacts
        .iter()
        .any(|c| c.contact_id() == contact_id)
    }

    #[tokio::test]
    async fn it_builds_a_submit_task_and_clears_the_modal_on_success() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      let _ = update(&mut state, Message::ContactAddOpened, &db);
      let _ = update(
        &mut state,
        Message::ContactEntityChanged(Some(entity(95_010, EntityKind::Character, "New Friend"))),
        &db,
      );

      let task = update(&mut state, Message::ContactModalSubmitted, &db);
      drop(task);
      assert!(
        state.contact_modal().is_some(),
        "the modal stays open until the enqueue resolves"
      );

      let _ = update(&mut state, Message::ContactSubmitted(Ok(())), &db);
      assert!(state.contact_modal().is_none(), "a successful enqueue closes the modal");
    }

    #[tokio::test]
    async fn it_cancels_a_pending_delete() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);

      let row = contact(95_040, "character", "Spared", 0.0, false, "[]");
      let _ = update(&mut state, Message::ContactDeleteRequested(Box::new(row)), &db);
      let _ = update(&mut state, Message::ContactDeleteCancelled, &db);

      assert!(state.contact_delete().is_none());
    }

    #[tokio::test]
    async fn it_confirms_a_delete_and_drops_the_row_immediately() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Owner", None).await;
      character::upsert_contact(&db, &contact(95_030, "character", "Doomed", 0.0, false, "[]"))
        .await
        .unwrap();
      let mut state = loaded_state(42);

      let row = contact(95_030, "character", "Doomed", 0.0, false, "[]");
      let _ = update(&mut state, Message::ContactDeleteRequested(Box::new(row)), &db);
      assert!(state.contact_delete().is_some());

      let task = update(&mut state, Message::ContactDeleteConfirmed, &db);
      drop(task);
      enqueue_contact_remove(db.clone(), 42, contact(95_030, "character", "Doomed", 0.0, false, "[]"))
        .await
        .unwrap();

      assert!(state.contact_delete().is_none(), "the confirm dialog closes");
      assert_eq!(outbox_count(&db, "contact.remove").await, 1);
      assert!(!contact_present(&db, 95_030).await, "the row drops optimistically");
    }

    #[tokio::test]
    async fn it_drops_a_stale_search_response_from_a_superseded_generation() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      let _ = update(&mut state, Message::ContactAddOpened, &db);

      let _ = update(&mut state, Message::ContactEntityInput("Vex".to_owned()), &db);
      let stale = state.contact_search_generation();
      let _ = update(&mut state, Message::ContactEntityInput("Vexor".to_owned()), &db);

      let _ = update(
        &mut state,
        Message::ContactEntityResults {
          generation: stale,
          results: vec![entity(1, EntityKind::Character, "Stale")],
        },
        &db,
      );

      assert!(
        state.contact_modal().unwrap().entity().is_none(),
        "a stale results message does not populate the picker"
      );
    }

    #[tokio::test]
    async fn it_enqueues_an_add_and_mirrors_the_row_optimistically() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Owner", None).await;

      enqueue_contact_add(
        db.clone(),
        42,
        ContactPayload {
          contact_id: 95_010,
          contact_name: "New Friend".to_owned(),
          contact_type: "character".to_owned(),
          label_ids: vec![1, 2],
          standing: 10.0,
          watched: true,
        },
      )
      .await
      .unwrap();

      assert_eq!(outbox_count(&db, "contact.add").await, 1);
      assert!(contact_present(&db, 95_010).await, "the optimistic row is mirrored");
    }

    #[tokio::test]
    async fn it_enqueues_an_edit_and_updates_the_row_immediately() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Owner", None).await;
      character::upsert_contact(&db, &contact(95_020, "character", "Old Friend", 5.0, false, "[1]"))
        .await
        .unwrap();

      enqueue_contact_edit(
        db.clone(),
        42,
        ContactPayload {
          contact_id: 95_020,
          contact_name: "Old Friend".to_owned(),
          contact_type: "character".to_owned(),
          label_ids: vec![1],
          standing: -10.0,
          watched: false,
        },
        Some(ContactPayload {
          contact_id: 95_020,
          contact_name: "Old Friend".to_owned(),
          contact_type: "character".to_owned(),
          label_ids: vec![1],
          standing: 5.0,
          watched: false,
        }),
      )
      .await
      .unwrap();

      assert_eq!(outbox_count(&db, "contact.edit").await, 1);
      let stored = character::contacts(&db, 42)
        .await
        .unwrap()
        .contacts
        .into_iter()
        .find(|c| c.contact_id() == 95_020)
        .unwrap();
      assert_eq!(stored.standing(), -10.0);
    }

    #[tokio::test]
    async fn it_forces_watch_off_for_non_character_entities() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      let _ = update(&mut state, Message::ContactAddOpened, &db);

      let _ = update(
        &mut state,
        Message::ContactEntityChanged(Some(entity(98_001, EntityKind::Corporation, "Test Corp"))),
        &db,
      );
      let _ = update(&mut state, Message::ContactWatchToggled, &db);

      assert!(
        !state.contact_modal().unwrap().watch(),
        "a corporation can never be watchlisted"
      );
    }

    #[tokio::test]
    async fn it_is_a_noop_to_submit_with_no_entity_selected() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      let _ = update(&mut state, Message::ContactAddOpened, &db);

      let task = update(&mut state, Message::ContactModalSubmitted, &db);
      drop(task);

      assert_eq!(outbox_count(&db, "contact.add").await, 0);
      assert!(
        state.contact_modal().is_some(),
        "the modal stays open until an entity is chosen"
      );
    }

    #[tokio::test]
    async fn it_opens_and_closes_the_add_modal() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);

      let _ = update(&mut state, Message::ContactAddOpened, &db);
      assert!(state.contact_modal().is_some());
      assert!(!state.contact_modal().unwrap().is_edit());

      let _ = update(&mut state, Message::ContactModalClosed, &db);
      assert!(state.contact_modal().is_none());
    }

    #[tokio::test]
    async fn it_opens_the_edit_modal_with_the_entity_locked() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);

      let row = contact(95_001, "character", "Test Pilot", 5.0, true, "[1]");
      let _ = update(&mut state, Message::ContactEditOpened(Box::new(row)), &db);

      let modal = state.contact_modal().expect("modal open");
      assert!(modal.is_edit());
      assert_eq!(modal.entity().map(|e| e.id), Some(95_001));
      assert_eq!(modal.standing(), 5.0);
    }

    #[tokio::test]
    async fn it_tracks_field_edits_in_the_open_modal() {
      let db = store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      let _ = update(&mut state, Message::ContactAddOpened, &db);

      let _ = update(
        &mut state,
        Message::ContactEntityChanged(Some(entity(95_002, EntityKind::Character, "New Pilot"))),
        &db,
      );
      let _ = update(&mut state, Message::ContactStandingChanged(-10.0), &db);
      let _ = update(&mut state, Message::ContactLabelToggled(2), &db);
      let _ = update(&mut state, Message::ContactWatchToggled, &db);

      let modal = state.contact_modal().expect("modal open");
      assert_eq!(modal.entity().map(|e| e.id), Some(95_002));
      assert_eq!(modal.standing(), -10.0);
      assert_eq!(modal.labels(), &[2]);
      assert!(modal.watch());
      assert!(modal.can_submit());
    }
  }

  mod contacts_write_enabled {
    use super::*;

    #[test]
    fn it_is_enabled_when_the_write_scope_is_granted() {
      use crate::clients::esi::scopes;
      let mut state = State::new(42, &Feature::ALL);
      state.granted_scopes = Some(format!(
        "{} {}",
        scopes::CHARACTER_CONTACTS,
        scopes::CHARACTER_CONTACTS_WRITE
      ));

      assert!(state.contacts_write_enabled());
    }

    #[test]
    fn it_is_gated_when_no_scopes_are_granted() {
      let mut state = State::new(42, &Feature::ALL);
      state.granted_scopes = None;

      assert!(!state.contacts_write_enabled());
    }

    #[test]
    fn it_is_gated_when_only_the_read_scope_is_granted() {
      use crate::clients::esi::scopes;
      let mut state = State::new(42, &Feature::ALL);
      state.granted_scopes = Some(scopes::CHARACTER_CONTACTS.to_owned());

      assert!(
        !state.contacts_write_enabled(),
        "a pilot authorized before write_contacts existed must be surfaced for re-auth"
      );
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

  mod load_contacts {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_contact(db: &Database, contact_id: i64, kind: &str, name: &str) {
      sqlx::query(
        "INSERT INTO character_contacts \
          (character_id, contact_id, contact_type, standing, is_watched, is_blocked, label_ids, contact_name) \
        VALUES (42, ?, ?, 0.0, 0, 0, '[]', ?)",
      )
      .bind(contact_id)
      .bind(kind)
      .bind(name)
      .execute(&db.0)
      .await
      .unwrap();
    }

    async fn seed_label(db: &Database, label_id: i64, name: &str) {
      sqlx::query("INSERT INTO character_contact_labels (character_id, label_id, label_name) VALUES (42, ?, ?)")
        .bind(label_id)
        .bind(name)
        .execute(&db.0)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_keeps_the_labels_through_a_filter_restart() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      seed_label(&db, 1, "Fleet").await;
      seed_contact(&db, 100, "character", "Wingmate").await;

      let mut state = State::new(42, &Feature::ALL);
      state.contacts = load_contacts(&db, 42).await;
      reset_contacts_pagination(&mut state);

      // A filter change re-runs the first page; its labels must survive so the address-book notes still resolve.
      let task = update(
        &mut state,
        Message::ContactFilterChanged(tabs::contacts::ContactFilter::Character),
        &db,
      );
      // Drive the dispatched first-page load to completion and apply it.
      let page = load_contacts_page(
        db.clone(),
        42,
        Some("character"),
        None,
        ContactSortColumn::Standing,
        ContactSortDir::Desc,
        None,
      )
      .await;
      drop(task);
      let _ = update(&mut state, Message::ContactsPageLoaded(Box::new(page)), &db);

      let LoadState::Loaded(page) = &state.contacts else {
        panic!("expected a loaded contacts page");
      };
      assert_eq!(
        page
          .labels()
          .iter()
          .map(|l| l.label_name().as_str())
          .collect::<Vec<_>>(),
        ["Fleet"],
        "labels survive a sort/filter restart"
      );
    }

    #[tokio::test]
    async fn it_loads_the_first_page_with_resolved_rows_and_labels() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", None).await;
      seed_label(&db, 1, "Fleet").await;
      seed_contact(&db, 100, "character", "Wingmate").await;

      let LoadState::Loaded(page) = load_contacts(&db, 42).await else {
        panic!("expected a loaded contacts page");
      };

      assert_eq!(page.rows().len(), 1);
      assert_eq!(page.rows()[0].contact.contact_name(), "Wingmate");
      assert_eq!(
        page
          .labels()
          .iter()
          .map(|l| l.label_name().as_str())
          .collect::<Vec<_>>(),
        ["Fleet"]
      );
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
    async fn it_is_none_when_the_character_has_no_credential() {
      let db = store::open_test().await.unwrap();

      assert!(load_granted_scopes(&db, 42).await.is_none());
    }

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
  }

  mod load_head_stats {
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[tokio::test]
    async fn it_returns_only_the_sec_status_when_no_state_row_exists() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Pilot", Some(4.8)).await;

      let head = load_head_stats(&db, 42).await;

      assert_eq!(head.sec_status, Some(4.8));
      assert!(head.location.is_none());
      assert!(!head.docked);
    }
  }

  mod load_killlog {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_an_empty_loaded_list_when_there_are_no_killmails() {
      let db = store::open_test().await.unwrap();

      let LoadState::Loaded(entries) = load_killlog(&db, 42).await else {
        panic!("expected a loaded killlog");
      };

      assert!(entries.is_empty());
    }

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
  }

  mod load_killmail_detail {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{KillmailAttacker, KillmailItem};

    fn attacker(killmail_id: i64, ordinal: i64, character_id: i64, damage: i64, final_blow: bool) -> KillmailAttacker {
      KillmailAttacker {
        alliance_id: None,
        attacker_character_id: Some(character_id),
        character_id: 42,
        corporation_id: Some(6006),
        damage_done: damage,
        final_blow,
        killmail_id,
        ordinal,
        ship_type_id: Some(670),
      }
    }

    fn item(killmail_id: i64, ordinal: i64, flag: i64, dropped: bool) -> KillmailItem {
      KillmailItem {
        character_id: 42,
        flag,
        killmail_id,
        ordinal,
        quantity_destroyed: if dropped { 0 } else { 1 },
        quantity_dropped: if dropped { 2 } else { 0 },
        type_id: 2185,
        value_isk: 4242.5,
      }
    }

    #[tokio::test]
    async fn it_flags_the_viewing_character_among_the_attackers() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Owner", None).await;
      character::upsert_killmail(&db, &kill_entry(42, 100, Some(9), None))
        .await
        .unwrap();
      let attackers = vec![attacker(100, 0, 42, 100, true)];
      character::upsert_killmail_detail(&db, 42, 100, &attackers, &[])
        .await
        .unwrap();

      let detail = load_killmail_detail(db.clone(), 42, 100).await.expect("detail loads");

      assert!(detail.attackers[0].is_self);
    }

    #[tokio::test]
    async fn it_groups_items_by_slot_and_sorts_attackers_final_blow_then_share() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42, "Owner", None).await;
      let mut kill = kill_entry(42, 100, Some(9), None);
      kill.value_isk = 100.0;
      kill.value_destroyed_isk = 80.0;
      kill.victim_damage_taken = 5000;
      character::upsert_killmail(&db, &kill).await.unwrap();
      // Two attackers: the higher-damage one (300) is NOT final blow; the lower-damage one (100) is.
      let attackers = vec![attacker(100, 0, 7, 300, false), attacker(100, 1, 9, 100, true)];
      // A high-power module (flag 27) and a cargo-hold item (flag 5).
      let items = vec![item(100, 0, 27, false), item(100, 1, 5, true)];
      character::upsert_killmail_detail(&db, 42, 100, &attackers, &items)
        .await
        .unwrap();

      let detail = load_killmail_detail(db.clone(), 42, 100).await.expect("detail loads");

      assert_eq!(detail.slots.len(), 2);
      assert_eq!(detail.slots[0].label, "High power");
      assert_eq!(detail.slots[1].label, "Cargo hold");
      assert!(detail.slots[1].items[0].dropped);
      assert_eq!(detail.slots[1].items[0].quantity, 2);

      assert_eq!(detail.attackers.len(), 2);
      assert!(detail.attackers[0].final_blow, "final blow sorts first");
      assert_eq!(detail.attackers[0].damage_share, 0.25);
      assert_eq!(detail.attackers[1].damage_share, 0.75);
      assert_eq!(detail.dropped_isk, 20.0);
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_killmail() {
      let db = store::open_test().await.unwrap();

      assert!(load_killmail_detail(db.clone(), 42, 999).await.is_none());
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

  mod mark_dirty {
    use super::*;

    const PILOT: i64 = 42;

    fn finished(kind: JobKind, subject: Subject) -> JobKey {
      JobKey::new(kind, subject)
    }

    #[test]
    fn it_ignores_a_corporation_subject_job() {
      let mut state = State::new(PILOT, &[]);

      state.mark_dirty(finished(JobKind::CharacterClones, Subject::Corporation(PILOT)));

      assert!(!state.is_dirty());
    }

    #[test]
    fn it_ignores_a_finished_job_for_a_different_pilot() {
      let mut state = State::new(PILOT, &[]);

      state.mark_dirty(finished(JobKind::CharacterClones, Subject::Character(PILOT + 1)));

      assert!(!state.is_dirty());
    }

    #[test]
    fn it_ignores_a_kind_this_screen_does_not_render() {
      let mut state = State::new(PILOT, &[]);

      state.mark_dirty(finished(JobKind::CharacterWallet, Subject::Character(PILOT)));
      state.mark_dirty(finished(JobKind::CharacterTelemetry, Subject::Character(PILOT)));

      assert!(!state.is_dirty());
    }

    #[test]
    fn it_marks_the_matching_type_for_the_drilled_in_pilot() {
      let mut state = State::new(PILOT, &[]);

      state.mark_dirty(finished(JobKind::CharacterClones, Subject::Character(PILOT)));

      assert!(state.is_dirty());
    }
  }

  mod pagination {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::CharacterContact;

    fn killlog_entry(killmail_id: i64, kill_time: &str) -> KillLogEntry {
      KillLogEntry {
        attacker_count: 1,
        final_blow: true,
        is_kill: true,
        kill_time: kill_time.to_owned(),
        killmail_id,
        ship_icon: images::IconResolution::Missing,
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

    fn contact_row(contact_id: i64) -> ContactRow {
      ContactRow {
        contact: CharacterContact {
          character_id: 42,
          contact_id,
          contact_name: format!("Contact {contact_id}"),
          contact_type: "character".to_owned(),
          is_blocked: false,
          is_watched: false,
          label_ids: String::new(),
          standing: 0.0,
        },
        image: images::ImageState::Stale {
          id: contact_id,
          kind: images::ImageKind::CharacterPortrait,
        },
      }
    }

    fn contacts_page(count: i64) -> ContactsPage {
      ContactsPage::for_test((0..count).map(|n| contact_row(95_000 + n)).collect(), Vec::new(), false)
    }

    fn killlog_page(count: i64) -> Vec<KillLogEntry> {
      (0..count)
        .map(|n| killlog_entry(1000 + n, "2024-01-01T00:00:00Z"))
        .collect()
    }

    #[tokio::test]
    async fn it_appends_a_contacts_page_and_recomputes_has_more() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contacts = LoadState::Loaded(contacts_page(CONTACTS_PAGE_SIZE));
      state.contacts_loading_more = true;

      let mut page = contacts_page(3);
      page.has_more = false;
      let _ = update(&mut state, Message::ContactsPageLoaded(Box::new(page)), &db);

      assert!(!state.contacts_loading_more);
      assert!(!state.contacts_has_more, "a short appended page exhausts the set");
      assert!(
        matches!(state.contacts, LoadState::Loaded(ref page) if page.rows().len() == CONTACTS_PAGE_SIZE as usize + 3)
      );
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
    async fn it_ignores_a_contacts_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contacts = LoadState::Loaded(contacts_page(CONTACTS_PAGE_SIZE));
      state.contacts_has_more = true;
      state.contacts_cursor = Some(ContactCursor::Number(0.0, 95_099));

      let _ = update(
        &mut state,
        Message::ContactsScrolled {
          absolute: 0.0,
          relative: 0.5,
        },
        &db,
      );

      assert!(!state.contacts_loading_more, "a sub-threshold scroll is a no-op");
    }

    #[tokio::test]
    async fn it_ignores_a_full_contacts_page_with_no_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contacts = LoadState::Loaded(contacts_page(3));
      state.contacts_has_more = false;

      let _ = update(
        &mut state,
        Message::ContactsScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(!state.contacts_loading_more, "an exhausted set does not fetch");
    }

    #[tokio::test]
    async fn it_ignores_a_killlog_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.killlog = LoadState::Loaded(killlog_page(KILLLOG_PAGE_SIZE));
      reset_killlog_pagination(&mut state);

      let _ = update(
        &mut state,
        Message::KilllogScrolled {
          absolute: 0.0,
          relative: 0.5,
        },
        &db,
      );

      assert!(!state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_killlog_scroll_with_no_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.killlog = LoadState::Loaded(killlog_page(3));
      reset_killlog_pagination(&mut state);

      let _ = update(
        &mut state,
        Message::KilllogScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(!state.killlog_has_more, "a short page is the last page");
      assert!(!state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_below_the_threshold() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(
        &mut state,
        Message::StandingsScrolled {
          absolute: 0.0,
          relative: 0.5,
        },
        &db,
      );

      assert!(!state.standings_loading_more, "a sub-threshold scroll is a no-op");
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_when_the_filter_does_not_surface_agents() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_filter = tabs::standings::StandingsFilter::Factions;
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(
        &mut state,
        Message::StandingsScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(
        !state.standings_loading_more,
        "a non-agent filter does not paginate agents"
      );
      assert!(state.standings_has_more, "the cursor and has_more are left untouched");
      assert_eq!(state.standings_agent_cursor, Some(("Agent".to_owned(), 1)));
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_while_already_loading() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = true;
      state.standings_loading_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(
        &mut state,
        Message::StandingsScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(state.standings_loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_standings_scroll_with_no_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = false;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(
        &mut state,
        Message::StandingsScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(!state.standings_loading_more, "exhausted agents do not fetch");
    }

    #[tokio::test]
    async fn it_marks_loading_and_clears_the_cursor_guard_on_a_standings_page_fetch() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.standings_has_more = true;
      state.standings_agent_cursor = Some(("Agent".to_owned(), 1));

      let _ = update(
        &mut state,
        Message::StandingsScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(state.standings_loading_more, "a qualifying scroll starts a load");
    }

    #[tokio::test]
    async fn it_marks_loading_on_a_qualifying_killlog_scroll() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.killlog = LoadState::Loaded(killlog_page(KILLLOG_PAGE_SIZE));
      reset_killlog_pagination(&mut state);

      let _ = update(
        &mut state,
        Message::KilllogScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(state.killlog_loading_more);
    }

    #[tokio::test]
    async fn it_restarts_contacts_pagination_on_a_filter_change() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contacts = LoadState::Loaded(contacts_page(CONTACTS_PAGE_SIZE));
      state.contacts_scroll_offset = 900.0;

      let _ = update(
        &mut state,
        Message::ContactFilterChanged(tabs::contacts::ContactFilter::Character),
        &db,
      );

      assert!(
        matches!(state.contacts, LoadState::Loading),
        "switching filters re-runs the first page from SQL"
      );
      assert_eq!(state.contacts_scroll_offset(), 0.0, "the window snaps back to the top");
      assert!(state.contacts_loading_more, "a fresh first page is in flight");
    }

    #[tokio::test]
    async fn it_starts_a_contacts_fetch_past_the_threshold_with_more_pages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contacts = LoadState::Loaded(contacts_page(CONTACTS_PAGE_SIZE));
      state.contacts_has_more = true;
      state.contacts_cursor = Some(ContactCursor::Number(0.0, 95_099));

      let _ = update(
        &mut state,
        Message::ContactsScrolled {
          absolute: 0.0,
          relative: 0.95,
        },
        &db,
      );

      assert!(state.contacts_loading_more, "a qualifying scroll starts a load");
    }

    #[tokio::test]
    async fn it_tracks_the_contacts_scroll_offset_for_windowing() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contacts = LoadState::Loaded(contacts_page(CONTACTS_PAGE_SIZE));

      let _ = update(
        &mut state,
        Message::ContactsScrolled {
          absolute: 1_234.0,
          relative: 0.1,
        },
        &db,
      );

      assert_eq!(state.contacts_scroll_offset(), 1_234.0);
    }
  }

  mod picker_pilot {
    use pretty_assertions::assert_eq;

    use super::*;

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
  }

  mod reload_arm {
    use super::*;

    fn contacts() -> ContactsPage {
      ContactsPage::for_test(Vec::new(), Vec::new(), false)
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

  mod standings_search {
    use pretty_assertions::assert_eq;

    use super::*;

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
  }

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_clones_when_no_gated_tab_is_enabled() {
      let state = State::new(42, &[]);

      assert_eq!(state.active_tab, Tab::Clones);
    }

    #[test]
    fn it_omits_fresh_roster_portraits_from_the_stale_keys() {
      let mut state = State::new(42, &[]);
      let mut fresh = pilot(42, "Test Pilot");
      fresh.portrait = images::ImageState::Fresh(std::path::PathBuf::from("/tmp/42.jpg"));
      state.roster = vec![fresh];

      assert!(state.stale_images().is_empty());
    }

    #[test]
    fn it_selects_the_first_enabled_tab_on_open() {
      let state = State::new(42, &Feature::ALL);

      assert_eq!(state.active_tab, Tab::Clones);
      assert_eq!(state.active(), 42);
    }

    #[test]
    fn it_surfaces_an_open_contact_modals_stale_portrait_as_an_image_key() {
      let mut state = State::new(42, &[]);
      let contact = CharacterContact {
        character_id: 42,
        contact_id: 98_000_001,
        contact_name: "Test Corp".to_owned(),
        contact_type: "corporation".to_owned(),
        is_blocked: false,
        is_watched: false,
        label_ids: "[]".to_owned(),
        standing: 0.0,
      };
      state.contact_modal = Some(tabs::contact_modal::ContactModal::edit(&contact, Vec::new()));

      let stale = state.stale_images();

      assert!(stale.contains(&(images::ImageKind::CorporationLogo, 98_000_001)));
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
    fn sync_features_keeps_a_still_enabled_active_tab() {
      let mut state = State::new(42, &Feature::ALL);
      state.active_tab = Tab::Standings;

      state.sync_features(&[Feature::CloneMonitoring, Feature::Standings]);

      assert_eq!(state.active_tab, Tab::Standings);
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
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_opens_and_closes_the_killmail_detail_modal() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(
        &mut state,
        Message::KillmailDetailLoaded(Box::new(Some(killmail_detail_fixture()))),
        &db,
      );
      assert!(state.selected_killmail.is_some());

      let _ = update(&mut state, Message::CloseKillmailDetail, &db);
      assert!(state.selected_killmail.is_none());
    }

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
    async fn it_toggles_the_picker_dropdown() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_treats_a_reauth_request_as_a_noop_for_the_app_shell_to_intercept() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::ReauthRequested(42), &db);

      assert_eq!(state.active(), 42);
    }
  }

  mod update_filters {
    use pretty_assertions::assert_eq;

    use super::*;

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
        contacts: LoadState::Loaded(ContactsPage::for_test(Vec::new(), Vec::new(), false)),
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
    async fn it_records_the_contact_search_query_and_clears_it() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new(42, &Feature::ALL);

      let _ = update(&mut state, Message::ContactsSearchChanged("vex".to_owned()), &db);
      assert_eq!(state.contacts_query(), "vex");

      let _ = update(&mut state, Message::ContactsSearchCleared, &db);
      assert_eq!(state.contacts_query(), "");
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
  }

  mod view {
    use super::*;

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
    fn it_renders_the_empty_state_with_no_roster() {
      let state = State::new(42, &Feature::ALL);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_with_the_picker_dropdown_open() {
      let mut state = loaded_state(42);
      state.picker_open = true;

      let _el: Element<'_, Message> = view(&state);
    }
  }

  mod write_gating {
    use super::*;

    #[test]
    fn it_renders_the_contacts_tab_read_only_when_the_write_scope_is_absent() {
      use crate::clients::esi::scopes;
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.granted_scopes = Some(scopes::CHARACTER_CONTACTS.to_owned());

      assert!(!state.contacts_write_enabled());
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_contacts_tab_with_write_actions_when_the_scope_is_granted() {
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;

      assert!(state.contacts_write_enabled());
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_delete_confirm_overlay() {
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contact_delete = Some(tabs::contact_modal::DeleteConfirm {
        contact: CharacterContact {
          character_id: 42,
          contact_id: 95_050,
          contact_name: "Doomed".to_owned(),
          contact_type: "character".to_owned(),
          is_blocked: false,
          is_watched: false,
          label_ids: "[]".to_owned(),
          standing: 0.0,
        },
      });

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_open_add_modal_overlay() {
      let mut state = loaded_state(42);
      state.active_tab = Tab::Contacts;
      state.contact_modal = Some(tabs::contact_modal::ContactModal::add(Vec::new(), Vec::new()));

      let _el: Element<'_, Message> = view(&state);
    }
  }
}
