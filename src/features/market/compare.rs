use std::sync::Arc;

use iced::{
  Element, Length, Padding, Task,
  widget::{Column, container, text},
};

use super::{BookAccess, Message, State, StructureBook, book};
use crate::{
  clients::{esi, eve_sso},
  services::location_search::{LocationRef, LocationTier},
  store::{
    Database,
    repo::{market_comparison, sde},
  },
  ui::{
    format::fmt_isk_opt,
    style::{color, spacing, typography},
  },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arbitrage {
  Margin {
    buy_at: usize,
    margin: f64,
    margin_pct: f64,
    sell_at: usize,
  },
  None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompareColumn {
  pub access: BookAccess,
  pub book: Option<book::OrderBook>,
  pub place: LocationRef,
}

impl CompareColumn {
  fn new(place: LocationRef) -> Self {
    CompareColumn {
      access: BookAccess::default(),
      book: None,
      place,
    }
  }

  pub fn best_buy(&self) -> Option<f64> {
    self.book.as_ref().and_then(|book| book.best_buy)
  }

  pub fn best_sell(&self) -> Option<f64> {
    self.book.as_ref().and_then(|book| book.best_sell)
  }
}

pub fn arbitrage(columns: &[CompareColumn]) -> Arbitrage {
  if priced_column_count(columns) < 2 {
    return Arbitrage::None;
  }
  let (Some(buy_at), Some(sell_at)) = (cheapest_sell(columns), richest_buy(columns)) else {
    return Arbitrage::None;
  };
  if buy_at == sell_at {
    return Arbitrage::None;
  }
  let (Some(ask), Some(bid)) = (columns[buy_at].best_sell(), columns[sell_at].best_buy()) else {
    return Arbitrage::None;
  };
  let margin = bid - ask;
  if margin <= 0.0 {
    return Arbitrage::None;
  }
  let margin_pct = if ask > 0.0 { margin / ask * 100.0 } else { 0.0 };
  Arbitrage::Margin {
    buy_at,
    margin,
    margin_pct,
    sell_at,
  }
}

pub fn cheapest_sell(columns: &[CompareColumn]) -> Option<usize> {
  columns
    .iter()
    .enumerate()
    .filter_map(|(index, column)| column.best_sell().map(|price| (index, price)))
    .min_by(|left, right| left.1.total_cmp(&right.1))
    .map(|(index, _)| index)
}

pub fn richest_buy(columns: &[CompareColumn]) -> Option<usize> {
  columns
    .iter()
    .enumerate()
    .filter_map(|(index, column)| column.best_buy().map(|price| (index, price)))
    .max_by(|left, right| left.1.total_cmp(&right.1))
    .map(|(index, _)| index)
}

fn priced_column_count(columns: &[CompareColumn]) -> usize {
  columns
    .iter()
    .filter(|column| column.best_sell().is_some() || column.best_buy().is_some())
    .count()
}

pub(super) fn try_dispatch(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  match &message {
    Message::CompareMarketsLoaded(_)
    | Message::CompareBookLoaded(..)
    | Message::CompareStructureBookLoaded(..)
    | Message::CompareAddPickerToggled
    | Message::CompareAddSearchChanged(_)
    | Message::CompareAddResultsLoaded(..)
    | Message::CompareMarketPicked(_)
    | Message::CompareMarketRemoved(_) => {}
    _ => return Err(message),
  }
  Ok(apply(state, message, db))
}

pub(super) fn reduce(state: &mut State, message: Message) {
  match message {
    Message::CompareMarketsLoaded(places) => {
      state.compare = places.into_iter().map(CompareColumn::new).collect();
    }
    Message::CompareBookLoaded(place_id, book) => apply_book(state, place_id, *book),
    Message::CompareStructureBookLoaded(place_id, result) => apply_structure_book(state, place_id, result),
    Message::CompareAddPickerToggled => {
      state.compare_add_open = !state.compare_add_open;
      if !state.compare_add_open {
        state.compare_search.clear();
      }
    }
    Message::CompareAddSearchChanged(query) => {
      state.compare_search.set_query(query);
    }
    Message::CompareAddResultsLoaded(generation, results) => {
      state.compare_search.accept_results(generation, results);
    }
    Message::CompareMarketPicked(_) => {
      state.compare_add_open = false;
      state.compare_search.clear();
    }
    _ => {}
  }
}

pub(super) fn structure_fetches<'a>(places: impl Iterator<Item = &'a LocationRef>, type_id: i64) -> Vec<(i64, i64)> {
  places
    .filter(|place| place.tier == Some(LocationTier::Structure))
    .map(|place| (place.id, type_id))
    .collect()
}

pub(super) fn location_search(
  state: &State,
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Task<Message> {
  if !state.compare_search.searchable() {
    return Task::none();
  }
  let generation = state.compare_search.generation();
  Task::perform(
    crate::services::location_search::search_locations_enriched(
      db.clone(),
      esi,
      sso,
      query,
      super::LOCATION_SEARCH_MIN_CHARS,
    ),
    move |results| Message::CompareAddResultsLoaded(generation, results),
  )
}

pub(super) fn load_markets_task(db: &Database) -> Task<Message> {
  Task::perform(load_markets(db.clone()), Message::CompareMarketsLoaded)
}

// A minimal placeholder that exercises the compare state; the real columns / arbitrage strip / add
// modal render in the follow-up view task (tvkvtxpv), which consumes the accessors and messages here.
pub(super) fn surface(state: &State) -> Element<'_, Message> {
  let columns = state.compare_columns();
  let margin = match arbitrage(columns) {
    Arbitrage::Margin {
      margin, ..
    } => Some(margin),
    Arbitrage::None => None,
  };

  let mut children: Vec<Element<'_, Message>> = vec![
    text(t!("market.compare_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    placeholder_line(columns.len().to_string()),
    placeholder_line(fmt_isk_opt(margin)),
  ];

  if state.compare_add_open() {
    let searching = if state.compare_searching() { "searching" } else { "idle" };
    let highlight = state
      .compare_highlight()
      .map_or_else(|| "-".to_owned(), |index| index.to_string());
    children.push(placeholder_line(format!(
      "{} · {} · {} · {}",
      state.compare_query(),
      state.compare_results().len(),
      searching,
      highlight,
    )));
  }

  container(Column::with_children(children).spacing(spacing::SPACE_2))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: 20.0,
      right: 28.0,
      bottom: 36.0,
      left: 28.0,
    })
    .into()
}

fn placeholder_line<'a>(value: String) -> Element<'a, Message> {
  text(value)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn apply(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  let follow = plan(&message);
  reduce(state, message);
  execute(state, db, follow)
}

fn plan(message: &Message) -> Follow {
  match message {
    Message::CompareMarketsLoaded(_) => Follow::Fanout,
    Message::CompareMarketPicked(place) => Follow::Add(place.clone()),
    Message::CompareMarketRemoved(place_id) => Follow::Remove(*place_id),
    _ => Follow::None,
  }
}

fn execute(state: &State, db: &Database, follow: Follow) -> Task<Message> {
  match follow {
    Follow::Add(place) => persist_add_task(db, place),
    Follow::Fanout => super::compare_region_fetch_tasks(state, db),
    Follow::None => Task::none(),
    Follow::Remove(place_id) => persist_remove_task(db, place_id),
  }
}

fn apply_book(state: &mut State, place_id: i64, book: book::OrderBook) {
  if let Some(column) = column_mut(state, place_id) {
    column.book = Some(book);
    column.access = BookAccess::Ok;
  }
}

fn apply_structure_book(state: &mut State, place_id: i64, result: StructureBook) {
  let Some(column) = column_mut(state, place_id) else {
    return;
  };
  match result {
    StructureBook::Loaded(book) => {
      column.book = Some(*book);
      column.access = BookAccess::Ok;
    }
    StructureBook::NoAccess => column.access = BookAccess::NoAccess,
    StructureBook::Error => column.access = BookAccess::Error,
  }
}

fn column_mut(state: &mut State, place_id: i64) -> Option<&mut CompareColumn> {
  state.compare.iter_mut().find(|column| column.place.id == place_id)
}

fn persist_add_task(db: &Database, place: LocationRef) -> Task<Message> {
  let db = db.clone();
  let place_id = place.id;
  let tier = place
    .tier
    .or_else(|| LocationTier::from_id(place_id))
    .unwrap_or(LocationTier::Region);
  Task::perform(
    async move {
      let _ = market_comparison::add(&db, place_id, tier).await;
      load_markets(db).await
    },
    Message::CompareMarketsLoaded,
  )
}

fn persist_remove_task(db: &Database, place_id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = market_comparison::remove(&db, place_id).await;
      load_markets(db).await
    },
    Message::CompareMarketsLoaded,
  )
}

async fn load_markets(db: Database) -> Vec<LocationRef> {
  let markets = market_comparison::list(&db).await.unwrap_or_default();
  let mut places = Vec::with_capacity(markets.len());
  for market in markets {
    places.push(resolve_place(&db, market.place_id, market.tier).await);
  }
  places
}

async fn resolve_place(db: &Database, place_id: i64, tier: LocationTier) -> LocationRef {
  LocationRef {
    context: None,
    id: place_id,
    name: place_name(db, place_id, tier).await,
    security_status: None,
    tier: Some(tier),
  }
}

async fn place_name(db: &Database, place_id: i64, tier: LocationTier) -> String {
  match tier {
    LocationTier::Region => super::region_ref(db, place_id).await.name,
    LocationTier::Constellation => super::named_or_fallback(
      sde::get_constellation(db, place_id)
        .await
        .ok()
        .flatten()
        .map(|constellation| constellation.name().clone()),
      place_id,
    ),
    LocationTier::Station => super::named_or_fallback(
      sde::get_station(db, place_id)
        .await
        .ok()
        .flatten()
        .map(|station| station.name().clone()),
      place_id,
    ),
    LocationTier::Structure => super::named_or_fallback(
      sde::get_structure(db, place_id)
        .await
        .ok()
        .flatten()
        .map(|structure| structure.name().clone()),
      place_id,
    ),
    LocationTier::System => super::named_or_fallback(
      sde::get_solar_system(db, place_id)
        .await
        .ok()
        .flatten()
        .map(|system| system.name().clone()),
      place_id,
    ),
  }
}

enum Follow {
  Add(LocationRef),
  Fanout,
  None,
  Remove(i64),
}

#[cfg(test)]
mod tests {
  use super::*;

  fn place(id: i64, tier: LocationTier) -> LocationRef {
    LocationRef {
      context: None,
      id,
      name: String::new(),
      security_status: None,
      tier: Some(tier),
    }
  }

  fn column(id: i64, tier: LocationTier, best_sell: Option<f64>, best_buy: Option<f64>) -> CompareColumn {
    let book = (best_sell.is_some() || best_buy.is_some()).then(|| book::OrderBook {
      best_buy,
      best_sell,
      ..book::OrderBook::default()
    });
    CompareColumn {
      access: BookAccess::Ok,
      book,
      place: place(id, tier),
    }
  }

  mod arbitrage {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_finds_the_best_buy_cheap_sell_richer_buy_pair() {
      let columns = vec![
        column(60_003_760, LocationTier::Station, Some(5.0), Some(4.0)),
        column(60_008_494, LocationTier::Station, Some(9.0), Some(12.0)),
      ];

      match arbitrage(&columns) {
        Arbitrage::Margin {
          buy_at,
          margin,
          margin_pct,
          sell_at,
        } => {
          assert_eq!(buy_at, 0);
          assert_eq!(sell_at, 1);
          assert_eq!(margin, 7.0);
          assert_eq!(margin_pct, 7.0 / 5.0 * 100.0);
        }
        Arbitrage::None => panic!("expected a positive margin"),
      }
    }

    #[test]
    fn it_reports_no_margin_when_the_cheapest_ask_sits_above_every_bid() {
      let columns = vec![
        column(60_003_760, LocationTier::Station, Some(10.0), Some(4.0)),
        column(60_008_494, LocationTier::Station, Some(12.0), Some(6.0)),
      ];

      assert_eq!(arbitrage(&columns), Arbitrage::None);
    }

    #[test]
    fn it_reports_no_margin_for_a_single_priced_column() {
      let columns = vec![
        column(60_003_760, LocationTier::Station, Some(5.0), Some(9.0)),
        column(60_008_494, LocationTier::Station, None, None),
      ];

      assert_eq!(arbitrage(&columns), Arbitrage::None);
    }

    #[test]
    fn it_reports_no_margin_when_the_cheapest_ask_and_richest_bid_share_a_column() {
      let columns = vec![
        column(60_003_760, LocationTier::Station, Some(5.0), Some(12.0)),
        column(60_008_494, LocationTier::Station, Some(9.0), Some(4.0)),
      ];

      assert_eq!(arbitrage(&columns), Arbitrage::None);
    }

    #[test]
    fn it_ignores_columns_with_no_priced_ask_when_buying() {
      let columns = vec![
        column(60_003_760, LocationTier::Station, None, Some(20.0)),
        column(60_008_494, LocationTier::Station, Some(6.0), Some(7.0)),
        column(60_004_588, LocationTier::Station, Some(4.0), None),
      ];

      match arbitrage(&columns) {
        Arbitrage::Margin {
          buy_at,
          sell_at,
          ..
        } => {
          assert_eq!(buy_at, 2);
          assert_eq!(sell_at, 0);
        }
        Arbitrage::None => panic!("expected a positive margin"),
      }
    }
  }

  mod badges {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_picks_the_cheapest_sell_and_the_richest_buy_columns() {
      let columns = vec![
        column(60_003_760, LocationTier::Station, Some(9.0), Some(4.0)),
        column(60_008_494, LocationTier::Station, Some(5.0), Some(3.0)),
        column(60_004_588, LocationTier::Station, Some(7.0), Some(11.0)),
      ];

      assert_eq!(cheapest_sell(&columns), Some(1));
      assert_eq!(richest_buy(&columns), Some(2));
    }

    #[test]
    fn it_reports_no_badge_when_no_side_is_priced() {
      let columns = vec![column(60_003_760, LocationTier::Station, None, None)];

      assert_eq!(cheapest_sell(&columns), None);
      assert_eq!(richest_buy(&columns), None);
    }
  }

  mod structure_fetches {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_selects_only_structure_tier_places() {
      let places = [
        place(60_003_760, LocationTier::Station),
        place(1_035_000_000_001, LocationTier::Structure),
        place(10_000_002, LocationTier::Region),
      ];

      let fetches = structure_fetches(places.iter(), 34);

      assert_eq!(fetches, vec![(1_035_000_000_001, 34)]);
    }
  }

  mod reduce {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rebuilds_the_columns_from_a_loaded_set() {
      let mut state = State::new();

      reduce(
        &mut state,
        Message::CompareMarketsLoaded(vec![
          place(60_003_760, LocationTier::Station),
          place(60_008_494, LocationTier::Station),
        ]),
      );

      assert_eq!(state.compare_columns().len(), 2);
      assert_eq!(state.compare_reference_place().map(|place| place.id), Some(60_003_760));
    }

    #[test]
    fn it_stores_a_loaded_book_on_the_matching_column() {
      let mut state = State::new();
      reduce(
        &mut state,
        Message::CompareMarketsLoaded(vec![place(60_003_760, LocationTier::Station)]),
      );

      let book = book::build_order_book(vec![
        crate::clients::esi::models::market::RegionOrder {
          is_buy_order: false,
          price: 5.0,
          ..Default::default()
        },
        crate::clients::esi::models::market::RegionOrder {
          is_buy_order: true,
          price: 4.0,
          ..Default::default()
        },
      ]);

      reduce(&mut state, Message::CompareBookLoaded(60_003_760, Box::new(book)));

      assert_eq!(state.compare_columns()[0].best_sell(), Some(5.0));
      assert_eq!(state.compare_columns()[0].best_buy(), Some(4.0));
    }

    #[test]
    fn it_marks_a_column_no_access_from_a_structure_miss() {
      let mut state = State::new();
      reduce(
        &mut state,
        Message::CompareMarketsLoaded(vec![place(1_035_000_000_001, LocationTier::Structure)]),
      );

      reduce(
        &mut state,
        Message::CompareStructureBookLoaded(1_035_000_000_001, StructureBook::NoAccess),
      );

      assert_eq!(state.compare_columns()[0].access, BookAccess::NoAccess);
    }

    #[test]
    fn it_drives_the_add_picker_open_and_closed() {
      let mut state = State::new();

      reduce(&mut state, Message::CompareAddPickerToggled);
      assert!(state.compare_add_open());

      reduce(&mut state, Message::CompareAddPickerToggled);
      assert!(!state.compare_add_open());
    }

    #[test]
    fn it_closes_the_add_picker_on_a_pick() {
      let mut state = State::new();
      reduce(&mut state, Message::CompareAddPickerToggled);

      reduce(
        &mut state,
        Message::CompareMarketPicked(place(60_008_494, LocationTier::Station)),
      );

      assert!(!state.compare_add_open());
    }

    #[test]
    fn it_stores_the_add_search_query() {
      let mut state = State::new();

      reduce(&mut state, Message::CompareAddSearchChanged("jita".to_owned()));

      assert_eq!(state.compare_query(), "jita");
    }

    #[test]
    fn it_leaves_the_columns_untouched_on_a_remove_message() {
      let mut state = State::new();
      reduce(
        &mut state,
        Message::CompareMarketsLoaded(vec![place(60_003_760, LocationTier::Station)]),
      );

      reduce(&mut state, Message::CompareMarketRemoved(60_003_760));

      assert_eq!(state.compare_columns().len(), 1);
    }
  }

  mod persistence {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_seeds_and_resolves_the_default_set() {
      let db = store::open_test().await.unwrap();

      let places = load_markets(db).await;

      assert_eq!(places.len(), 3);
      assert!(places.iter().all(|place| place.tier == Some(LocationTier::Station)));
    }

    #[tokio::test]
    async fn it_reflects_an_added_market_on_the_next_load() {
      let db = store::open_test().await.unwrap();
      market_comparison::add(&db, 10_000_002, LocationTier::Region)
        .await
        .unwrap();
      market_comparison::add(&db, 60_003_760, LocationTier::Station)
        .await
        .unwrap();

      let ids: Vec<i64> = load_markets(db).await.into_iter().map(|place| place.id).collect();

      assert_eq!(ids, vec![10_000_002, 60_003_760]);
    }

    #[tokio::test]
    async fn it_keeps_at_least_one_market_on_remove() {
      let db = store::open_test().await.unwrap();
      market_comparison::add(&db, 60_003_760, LocationTier::Station)
        .await
        .unwrap();

      let removed = market_comparison::remove(&db, 60_003_760).await.unwrap();

      assert!(!removed);
      assert_eq!(load_markets(db).await.len(), 1);
    }
  }
}
