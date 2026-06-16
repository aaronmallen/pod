mod tabs;

use std::time::Duration;

use iced::{
  Element, Length, Task,
  alignment::Vertical,
  widget::{Column, Row, container, operation, text},
};

pub use self::tabs::Tab;
pub use crate::store::repo::standings::CatalogKind as StandingKind;
use crate::{
  store::{
    Database, images,
    repo::{character, org, sde, standings},
  },
  ui::{
    components::{
      avatar::Avatar,
      header::{header as header_band, header_divider, stat_block},
    },
    style::{color, radius, spacing, typography},
  },
};

pub(crate) const STANDINGS_SEARCH_INPUT_ID: &str = "corp-standings-search-input";

const LOGO_SIZE: f32 = 44.0;
const PLACEHOLDER: &str = "\u{2014}";
const SEARCH_DEBOUNCE_MS: u64 = 200;
const STANDINGS_PAGE_SIZE: i64 = 100;

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
  Loaded(Option<CorpHead>),
  StandingsAgentsPageLoaded(Box<StandingsAgentsPage>),
  StandingsClearSearch,
  StandingsFilterChanged(tabs::standings::StandingsFilter),
  StandingsResults(Box<StandingsResult>),
  StandingsScrolled { absolute: f32, relative: f32 },
  StandingsSearchChanged(String),
  TabChanged(Tab),
}

#[derive(Debug)]
pub struct State {
  active: i64,
  active_tab: Tab,
  head: Option<CorpHead>,
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
      head: None,
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
    if let LoadState::Loaded(rows) = &self.standings {
      keys.extend(rows.iter().filter_map(|row| row.image.stale_key()));
    }
    keys
  }

  pub(super) fn active_tab(&self) -> Tab {
    self.active_tab
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
  Task::perform(async move { load_head(&db, corporation_id).await }, Message::Loaded)
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Loaded(head) => {
      state.head = head;
      trigger_standings_search(state, db)
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
    Message::TabChanged(tab) => {
      state.active_tab = tab;
      Task::none()
    }
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

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
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

async fn load_head(db: &Database, corporation_id: i64) -> Option<CorpHead> {
  let corp = match org::get_corporation(db, corporation_id).await {
    Ok(Some(corp)) => corp,
    Ok(None) => return None,
    Err(error) => {
      tracing::warn!(corporation_id, %error, "failed to load corporation for detail view");
      return None;
    }
  };

  let alliance = match corp.alliance_id() {
    Some(alliance_id) => org::get_alliance(db, alliance_id)
      .await
      .ok()
      .flatten()
      .map(|alliance| alliance.name().to_owned()),
    None => None,
  };
  let ceo = match corp.ceo_id() {
    Some(ceo_id) => character::get(db, ceo_id)
      .await
      .ok()
      .flatten()
      .map(|character| character.name().to_owned()),
    None => None,
  };
  let hq = match corp.home_station_id() {
    Some(station_id) => sde::get_station(db, station_id)
      .await
      .ok()
      .flatten()
      .map(|station| station.name().to_owned()),
    None => None,
  };
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

  mod format_members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_millions_thin_space_thousands_and_raw_figures() {
      assert_eq!(format_members(Some(2_400_000)), "2.4M");
      assert_eq!(format_members(Some(12_400)), "12\u{2009}400");
      assert_eq!(format_members(Some(89)), "89");
    }

    #[test]
    fn it_returns_the_placeholder_for_an_unknown_count() {
      assert_eq!(format_members(None), PLACEHOLDER);
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

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_on_the_first_tab() {
      let state = State::new(98_000_001);

      assert_eq!(state.active(), 98_000_001);
      assert_eq!(state.active_tab, Tab::Contacts);
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::TabChanged(Tab::Standings), &db);

      assert_eq!(state.active_tab, Tab::Standings);
    }

    #[tokio::test]
    async fn it_stores_the_loaded_head() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();

      let _task = update(&mut state, Message::Loaded(Some(head())), &db);

      assert!(state.head.is_some());
    }

    #[tokio::test]
    async fn it_reports_a_stale_logo_only_once_loaded() {
      let mut state = State::new(98_000_001);
      let db = crate::store::open_test().await.unwrap();
      assert!(state.stale_images().is_empty());

      let _task = update(&mut state, Message::Loaded(Some(head())), &db);

      assert_eq!(
        state.stale_images(),
        vec![(images::ImageKind::CorporationLogo, 98_000_001)]
      );
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

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_loading_header_before_data_arrives() {
      let state = State::new(98_000_001);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_loaded_header_and_each_tab_body() {
      let mut state = State::new(98_000_001);
      state.head = Some(head());

      for tab in Tab::ORDER {
        state.active_tab = tab;
        let _el: Element<'_, Message> = view(&state);
      }
    }
  }
}
