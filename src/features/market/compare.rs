mod view;

use std::{collections::HashSet, sync::Arc};

use iced::{Point, Task};
pub(super) use view::{mount, surface};

use super::{BookAccess, Message, State, StructureBook, book};
use crate::{
  clients::{esi, eve_sso},
  services::location_search::{LocationRef, LocationTier},
  store::{
    Database,
    model::MarketComparisonPin,
    repo::{market_comparison_pin, sde},
  },
};

pub(super) const DEFAULT_STATIONS: [i64; 3] = [60_003_760, 60_008_494, 60_011_866];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlockId {
  Pin(i64),
  Transient,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompareBlock {
  pub columns: Vec<CompareColumn>,
  pub id: BlockId,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompareColumn {
  pub access: BookAccess,
  pub book: Option<book::OrderBook>,
  pub place: LocationRef,
  pub row: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CompareMenu {
  pub(super) anchor: Point,
  pub(super) block: BlockId,
  pub(super) place_id: i64,
}

enum Follow {
  AddPinMarket(i64, LocationRef),
  CreatePin(CompareBlock),
  DeletePin(i64),
  FetchMissing,
  FetchOne(i64, LocationRef),
  None,
  PersistOrder,
  RemovePinMarket(i64),
}

impl CompareBlock {
  pub fn contains(&self, place_id: i64) -> bool {
    self.columns.iter().any(|column| column.place.id == place_id)
  }

  pub fn pin_id(&self) -> Option<i64> {
    match self.id {
      BlockId::Pin(id) => Some(id),
      BlockId::Transient => None,
    }
  }
}

impl CompareColumn {
  fn new(place: LocationRef, row: Option<i64>) -> Self {
    CompareColumn {
      access: BookAccess::default(),
      book: None,
      place,
      row,
    }
  }

  pub fn best_buy(&self) -> Option<f64> {
    self.book.as_ref().and_then(|book| book.best_buy)
  }

  pub fn best_sell(&self) -> Option<f64> {
    self.book.as_ref().and_then(|book| book.best_sell)
  }

  pub fn book_volume(&self) -> Option<i64> {
    self
      .book
      .as_ref()
      .map(|book| book.sell.iter().chain(&book.buy).map(|row| row.volume_remain).sum())
  }

  pub fn spread_pct(&self) -> Option<f64> {
    self.book.as_ref().and_then(|book| book.spread_pct)
  }
}

pub fn block_badges(block: &CompareBlock) -> (Option<usize>, Option<usize>) {
  if block.columns.len() < 2 {
    return (None, None);
  }
  (cheapest_sell(&block.columns), richest_buy(&block.columns))
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

pub(super) fn try_dispatch(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  match &message {
    Message::CompareAddPickerDismissed
    | Message::CompareAddPickerOpened(_)
    | Message::CompareAddResultsLoaded(..)
    | Message::CompareAddSearchChanged(_)
    | Message::CompareBookLoaded(..)
    | Message::CompareCursorMoved(_)
    | Message::CompareDragStarted(_)
    | Message::CompareDropEntered(_)
    | Message::CompareDropExited(_)
    | Message::CompareDropReleased
    | Message::CompareGripEntered(_)
    | Message::CompareGripExited(_)
    | Message::CompareMarketPicked(_)
    | Message::CompareMarketRemoved(..)
    | Message::CompareMenuDismissed
    | Message::CompareMenuOpened(..)
    | Message::ComparePinRequested
    | Message::ComparePinsLoaded(_)
    | Message::CompareStructureBookLoaded(..)
    | Message::CompareTransientLoaded(_)
    | Message::CompareUnpinRequested(_) => {}
    _ => return Err(message),
  }
  Ok(apply(state, message, db))
}

pub(super) fn reduce(state: &mut State, message: Message) {
  match message {
    Message::ComparePinsLoaded(blocks) => {
      state.compare_pins = merge_known_books(state, blocks);
    }
    Message::CompareTransientLoaded(block) => accept_transient(state, *block),
    Message::CompareBookLoaded(type_id, place_id, book) => apply_book(state, type_id, place_id, *book),
    Message::CompareStructureBookLoaded(place_id, type_id, result) => {
      apply_structure_book(state, place_id, type_id, result);
    }
    Message::CompareAddPickerOpened(block_id) => {
      state.compare_add_target = Some(block_id);
      state.compare_search.clear();
    }
    Message::CompareAddPickerDismissed => close_add_picker(state),
    Message::CompareAddSearchChanged(query) => {
      state.compare_search.set_query(query);
    }
    Message::CompareAddResultsLoaded(generation, results) => {
      state.compare_search.accept_results(generation, results);
    }
    Message::CompareMarketPicked(place) => adopt_pick(state, place),
    Message::CompareMarketRemoved(block_id, place_id) => {
      remove_column(state, block_id, place_id);
      state.compare_menu = None;
    }
    Message::ComparePinRequested => state.compare_transient = None,
    Message::CompareUnpinRequested(pin_id) => {
      state.compare_pins.retain(|block| block.id != BlockId::Pin(pin_id));
    }
    Message::CompareCursorMoved(point) => state.compare_cursor = Some(point),
    Message::CompareMenuOpened(block_id, place_id) => open_menu(state, block_id, place_id),
    Message::CompareMenuDismissed => state.compare_menu = None,
    Message::CompareDragStarted(_)
    | Message::CompareDropEntered(_)
    | Message::CompareDropExited(_)
    | Message::CompareDropReleased
    | Message::CompareGripEntered(_)
    | Message::CompareGripExited(_) => reduce_drag(state, message),
    _ => {}
  }
}

fn reduce_drag(state: &mut State, message: Message) {
  match message {
    Message::CompareDragStarted(pin_id) => {
      state.compare_dragging = Some(pin_id);
      state.compare_drop_target = None;
    }
    Message::CompareDropEntered(pin_id)
      if state.compare_dragging.is_some() && state.compare_dragging != Some(pin_id) =>
    {
      state.compare_drop_target = Some(pin_id);
    }
    Message::CompareDropExited(pin_id) if state.compare_drop_target == Some(pin_id) => {
      state.compare_drop_target = None;
    }
    Message::CompareDropReleased => release_drop(state),
    Message::CompareGripEntered(pin_id) => state.compare_grip_hover = Some(pin_id),
    Message::CompareGripExited(pin_id) if state.compare_grip_hover == Some(pin_id) => {
      state.compare_grip_hover = None;
    }
    _ => {}
  }
}

fn release_drop(state: &mut State) {
  let drop = state.compare_dragging.take().zip(state.compare_drop_target.take());
  if let Some((dragged, target)) = drop {
    splice_pins(&mut state.compare_pins, dragged, target);
  }
}

fn splice_pins(pins: &mut Vec<CompareBlock>, dragged: i64, target: i64) {
  let from = pins.iter().position(|block| block.id == BlockId::Pin(dragged));
  let to = pins.iter().position(|block| block.id == BlockId::Pin(target));
  if let Some((from, to)) = from.zip(to)
    && from != to
  {
    let moved = pins.remove(from);
    pins.insert(to, moved);
  }
}

pub(super) fn find_block(state: &State, id: BlockId) -> Option<&CompareBlock> {
  match id {
    BlockId::Pin(_) => state.compare_pins.iter().find(|block| block.id == id),
    BlockId::Transient => state.compare_transient.as_ref(),
  }
}

pub(super) fn load_pins_task(db: &Database) -> Task<Message> {
  Task::perform(load_pins(db.clone()), Message::ComparePinsLoaded)
}

pub(super) fn load_transient_task(db: &Database, type_id: i64) -> Task<Message> {
  Task::perform(load_transient(db.clone(), type_id), |block| {
    Message::CompareTransientLoaded(Box::new(block))
  })
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

pub(super) fn structure_fetches<'a>(places: impl Iterator<Item = &'a LocationRef>, type_id: i64) -> Vec<(i64, i64)> {
  places
    .filter(|place| place.tier == Some(LocationTier::Structure))
    .map(|place| (place.id, type_id))
    .collect()
}

fn accept_transient(state: &mut State, block: CompareBlock) {
  if state.selected != Some(block.type_id) {
    return;
  }
  let merged = merge_block_books(state, block);
  state.compare_transient = Some(merged);
}

fn add_pin_market_task(db: &Database, pin_id: i64, place: LocationRef) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = market_comparison_pin::add_market(&db, pin_id, place.id, place_tier(&place)).await;
      load_pins(db).await
    },
    Message::ComparePinsLoaded,
  )
}

fn adopt_pick(state: &mut State, place: LocationRef) {
  if let Some(block_id) = state.compare_add_target {
    push_column(state, block_id, place);
  }
  close_add_picker(state);
}

fn apply(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  let follow = plan(state, &message);
  reduce(state, message);
  execute(state, db, follow)
}

fn apply_book(state: &mut State, type_id: i64, place_id: i64, book: book::OrderBook) {
  for column in columns_mut(state, type_id, place_id) {
    column.book = Some(book.clone());
    column.access = BookAccess::Ok;
  }
}

fn apply_structure_book(state: &mut State, place_id: i64, type_id: i64, result: StructureBook) {
  for column in columns_mut(state, type_id, place_id) {
    match &result {
      StructureBook::Loaded(book) => {
        column.book = Some((**book).clone());
        column.access = BookAccess::Ok;
      }
      StructureBook::NoAccess => column.access = BookAccess::NoAccess,
      StructureBook::Error => column.access = BookAccess::Error,
    }
  }
}

fn blocks_of(state: &State) -> impl Iterator<Item = &CompareBlock> {
  state.compare_pins.iter().chain(state.compare_transient.as_ref())
}

fn close_add_picker(state: &mut State) {
  state.compare_add_target = None;
  state.compare_search.clear();
}

fn column_fetchable(column: &CompareColumn) -> bool {
  column.book.is_none() && column.place.tier != Some(LocationTier::Structure)
}

fn columns_mut(state: &mut State, type_id: i64, place_id: i64) -> impl Iterator<Item = &mut CompareColumn> {
  state
    .compare_pins
    .iter_mut()
    .chain(state.compare_transient.as_mut())
    .filter(move |block| block.type_id == type_id)
    .flat_map(|block| block.columns.iter_mut())
    .filter(move |column| column.place.id == place_id)
}

async fn constellation_name(db: &Database, place_id: i64) -> Option<String> {
  sde::get_constellation(db, place_id)
    .await
    .ok()
    .flatten()
    .map(|constellation| constellation.name().clone())
}

fn create_pin_task(db: &Database, block: CompareBlock) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = persist_pin(&db, &block).await;
      load_pins(db).await
    },
    Message::ComparePinsLoaded,
  )
}

fn delete_pin_task(db: &Database, pin_id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = market_comparison_pin::delete(&db, pin_id).await;
      load_pins(db).await
    },
    Message::ComparePinsLoaded,
  )
}

fn execute(state: &State, db: &Database, follow: Follow) -> Task<Message> {
  match follow {
    Follow::AddPinMarket(pin_id, place) => add_pin_market_task(db, pin_id, place),
    Follow::CreatePin(block) => create_pin_task(db, block),
    Follow::DeletePin(pin_id) => delete_pin_task(db, pin_id),
    Follow::FetchMissing => fetch_missing_books(state, db),
    Follow::FetchOne(type_id, place) => super::load_compare_book(db, &place, type_id),
    Follow::None => Task::none(),
    Follow::PersistOrder => persist_order_task(state, db),
    Follow::RemovePinMarket(row_id) => remove_pin_market_task(db, row_id),
  }
}

fn persist_order_task(state: &State, db: &Database) -> Task<Message> {
  let ids: Vec<i64> = state.compare_pins.iter().filter_map(CompareBlock::pin_id).collect();
  let db = db.clone();
  Task::perform(
    async move {
      let _ = market_comparison_pin::reorder(&db, &ids).await;
      load_pins(db).await
    },
    Message::ComparePinsLoaded,
  )
}

fn fetch_missing_books(state: &State, db: &Database) -> Task<Message> {
  let mut seen = HashSet::new();
  let mut tasks = Vec::new();
  for block in blocks_of(state) {
    for column in &block.columns {
      if !column_fetchable(column) || !seen.insert((block.type_id, column.place.id)) {
        continue;
      }
      tasks.push(super::load_compare_book(db, &column.place, block.type_id));
    }
  }
  Task::batch(tasks)
}

fn find_block_mut(state: &mut State, id: BlockId) -> Option<&mut CompareBlock> {
  match id {
    BlockId::Pin(_) => state.compare_pins.iter_mut().find(|block| block.id == id),
    BlockId::Transient => state.compare_transient.as_mut(),
  }
}

async fn load_pin_block(db: &Database, pin: &MarketComparisonPin) -> CompareBlock {
  let markets = market_comparison_pin::markets(db, pin.id).await.unwrap_or_default();
  let mut columns = Vec::with_capacity(markets.len());
  for market in markets {
    let tier = market_tier(&market.tier, market.place_id);
    let place = resolve_place(db, market.place_id, tier).await;
    columns.push(CompareColumn::new(place, Some(market.id)));
  }
  CompareBlock {
    columns,
    id: BlockId::Pin(pin.id),
    type_id: pin.type_id,
  }
}

async fn load_pins(db: Database) -> Vec<CompareBlock> {
  let pins = market_comparison_pin::list(&db).await.unwrap_or_default();
  let mut blocks = Vec::with_capacity(pins.len());
  for pin in pins {
    blocks.push(load_pin_block(&db, &pin).await);
  }
  blocks
}

async fn load_transient(db: Database, type_id: i64) -> CompareBlock {
  let mut columns = Vec::with_capacity(DEFAULT_STATIONS.len());
  for place_id in DEFAULT_STATIONS {
    let place = resolve_place(&db, place_id, LocationTier::Station).await;
    columns.push(CompareColumn::new(place, None));
  }
  CompareBlock {
    columns,
    id: BlockId::Transient,
    type_id,
  }
}

fn market_tier(value: &str, place_id: i64) -> LocationTier {
  LocationTier::parse(value)
    .or_else(|| LocationTier::from_id(place_id))
    .unwrap_or(LocationTier::Station)
}

fn merge_block_books(state: &State, mut block: CompareBlock) -> CompareBlock {
  for column in &mut block.columns {
    if column.book.is_some() {
      continue;
    }
    if let Some(known) = known_column(state, block.type_id, column.place.id) {
      column.access = known.access;
      column.book = known.book.clone();
    }
  }
  block
}

fn merge_known_books(state: &State, blocks: Vec<CompareBlock>) -> Vec<CompareBlock> {
  blocks
    .into_iter()
    .map(|block| merge_block_books(state, block))
    .collect()
}

fn known_column(state: &State, type_id: i64, place_id: i64) -> Option<&CompareColumn> {
  blocks_of(state)
    .filter(|block| block.type_id == type_id)
    .flat_map(|block| block.columns.iter())
    .find(|column| column.place.id == place_id && column.book.is_some())
}

fn open_menu(state: &mut State, block_id: BlockId, place_id: i64) {
  if !find_block(state, block_id).is_some_and(|block| block.contains(place_id)) {
    return;
  }
  let anchor = state.compare_cursor.unwrap_or(Point::ORIGIN);
  state.compare_menu = Some(CompareMenu {
    anchor,
    block: block_id,
    place_id,
  });
}

async fn persist_pin(db: &Database, block: &CompareBlock) -> Result<(), crate::store::Error> {
  let pin = market_comparison_pin::create(db, block.type_id).await?;
  for column in &block.columns {
    market_comparison_pin::add_market(db, pin.id, column.place.id, place_tier(&column.place)).await?;
  }
  Ok(())
}

fn place_tier(place: &LocationRef) -> LocationTier {
  place
    .tier
    .or_else(|| LocationTier::from_id(place.id))
    .unwrap_or(LocationTier::Station)
}

async fn place_name(db: &Database, place_id: i64, tier: LocationTier) -> String {
  match tier {
    LocationTier::Region => super::region_ref(db, place_id).await.name,
    LocationTier::Constellation => super::named_or_fallback(constellation_name(db, place_id).await, place_id),
    LocationTier::Station => super::named_or_fallback(station_name(db, place_id).await, place_id),
    LocationTier::Structure => super::named_or_fallback(structure_name(db, place_id).await, place_id),
    LocationTier::System => super::named_or_fallback(system_name(db, place_id).await, place_id),
  }
}

fn plan(state: &State, message: &Message) -> Follow {
  match message {
    Message::ComparePinsLoaded(_) | Message::CompareTransientLoaded(_) => Follow::FetchMissing,
    Message::CompareMarketPicked(place) => plan_pick(state, place),
    Message::CompareMarketRemoved(block_id, place_id) => plan_remove(state, *block_id, *place_id),
    Message::ComparePinRequested => match state.compare_transient.clone() {
      Some(block) => Follow::CreatePin(block),
      None => Follow::None,
    },
    Message::CompareUnpinRequested(pin_id) => Follow::DeletePin(*pin_id),
    Message::CompareDropReleased => release_follow(state),
    _ => Follow::None,
  }
}

fn release_follow(state: &State) -> Follow {
  match state.compare_dragging.zip(state.compare_drop_target) {
    Some((dragged, target)) if dragged != target => Follow::PersistOrder,
    _ => Follow::None,
  }
}

fn plan_pick(state: &State, place: &LocationRef) -> Follow {
  let Some(block) = state.compare_add_target.and_then(|id| find_block(state, id)) else {
    return Follow::None;
  };
  if block.contains(place.id) {
    return Follow::None;
  }
  match block.id {
    BlockId::Pin(pin_id) => Follow::AddPinMarket(pin_id, place.clone()),
    BlockId::Transient if place.tier == Some(LocationTier::Structure) => Follow::None,
    BlockId::Transient => Follow::FetchOne(block.type_id, place.clone()),
  }
}

fn plan_remove(state: &State, block_id: BlockId, place_id: i64) -> Follow {
  let Some(block) = find_block(state, block_id) else {
    return Follow::None;
  };
  if block.columns.len() <= 1 || block.pin_id().is_none() {
    return Follow::None;
  }
  match block
    .columns
    .iter()
    .find(|column| column.place.id == place_id)
    .and_then(|column| column.row)
  {
    Some(row_id) => Follow::RemovePinMarket(row_id),
    None => Follow::None,
  }
}

fn push_column(state: &mut State, block_id: BlockId, place: LocationRef) {
  if let Some(block) = find_block_mut(state, block_id)
    && !block.contains(place.id)
  {
    block.columns.push(CompareColumn::new(place, None));
  }
}

fn remove_column(state: &mut State, block_id: BlockId, place_id: i64) {
  if let Some(block) = find_block_mut(state, block_id)
    && block.columns.len() > 1
  {
    block.columns.retain(|column| column.place.id != place_id);
  }
}

fn remove_pin_market_task(db: &Database, row_id: i64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let _ = market_comparison_pin::remove_market(&db, row_id).await;
      load_pins(db).await
    },
    Message::ComparePinsLoaded,
  )
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

async fn station_name(db: &Database, place_id: i64) -> Option<String> {
  sde::get_station(db, place_id)
    .await
    .ok()
    .flatten()
    .map(|station| station.name().clone())
}

async fn structure_name(db: &Database, place_id: i64) -> Option<String> {
  sde::get_structure(db, place_id)
    .await
    .ok()
    .flatten()
    .map(|structure| structure.name().clone())
}

async fn system_name(db: &Database, place_id: i64) -> Option<String> {
  sde::get_solar_system(db, place_id)
    .await
    .ok()
    .flatten()
    .map(|system| system.name().clone())
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
      row: None,
    }
  }

  fn block(id: BlockId, type_id: i64, columns: Vec<CompareColumn>) -> CompareBlock {
    CompareBlock {
      columns,
      id,
      type_id,
    }
  }

  fn transient_default(type_id: i64) -> CompareBlock {
    let columns = DEFAULT_STATIONS
      .iter()
      .map(|&id| column(id, LocationTier::Station, None, None))
      .collect();
    block(BlockId::Transient, type_id, columns)
  }

  mod badges {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_picks_the_cheapest_sell_and_the_richest_buy_columns() {
      let columns = vec![
        column(60_003_760, LocationTier::Station, Some(9.0), Some(4.0)),
        column(60_008_494, LocationTier::Station, Some(5.0), Some(3.0)),
        column(60_011_866, LocationTier::Station, Some(7.0), Some(11.0)),
      ];

      let badges = block_badges(&block(BlockId::Transient, 34, columns));

      assert_eq!(badges, (Some(1), Some(2)));
    }

    #[test]
    fn it_gates_the_badges_behind_a_second_column() {
      let columns = vec![column(60_003_760, LocationTier::Station, Some(9.0), Some(4.0))];

      let badges = block_badges(&block(BlockId::Transient, 34, columns));

      assert_eq!(badges, (None, None));
    }

    #[test]
    fn it_reports_no_badge_when_no_side_is_priced() {
      let columns = vec![column(60_003_760, LocationTier::Station, None, None)];

      assert_eq!(cheapest_sell(&columns), None);
      assert_eq!(richest_buy(&columns), None);
    }
  }

  mod reduce {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_loaded_pins_and_preserves_known_books() {
      let mut state = State::new();
      state.selected = Some(34);
      state.compare_transient = Some(block(
        BlockId::Transient,
        34,
        vec![column(60_003_760, LocationTier::Station, Some(5.0), Some(4.0))],
      ));

      reduce(
        &mut state,
        Message::ComparePinsLoaded(vec![block(
          BlockId::Pin(1),
          34,
          vec![
            column(60_003_760, LocationTier::Station, None, None),
            column(60_008_494, LocationTier::Station, None, None),
          ],
        )]),
      );

      assert_eq!(state.compare_pins.len(), 1);
      assert_eq!(state.compare_pins[0].columns[0].best_sell(), Some(5.0));
      assert_eq!(state.compare_pins[0].columns[1].book, None);
    }

    #[test]
    fn it_applies_a_loaded_book_to_every_matching_column() {
      let mut state = State::new();
      state.compare_pins = vec![
        block(
          BlockId::Pin(1),
          34,
          vec![column(60_003_760, LocationTier::Station, None, None)],
        ),
        block(
          BlockId::Pin(2),
          34,
          vec![column(60_003_760, LocationTier::Station, None, None)],
        ),
        block(
          BlockId::Pin(3),
          35,
          vec![column(60_003_760, LocationTier::Station, None, None)],
        ),
      ];

      let book = book::OrderBook {
        best_sell: Some(5.0),
        ..book::OrderBook::default()
      };
      reduce(&mut state, Message::CompareBookLoaded(34, 60_003_760, Box::new(book)));

      assert_eq!(state.compare_pins[0].columns[0].best_sell(), Some(5.0));
      assert_eq!(state.compare_pins[1].columns[0].best_sell(), Some(5.0));
      assert_eq!(state.compare_pins[2].columns[0].book, None);
    }

    #[test]
    fn it_ignores_a_transient_for_a_stale_selection() {
      let mut state = State::new();
      state.selected = Some(35);

      reduce(
        &mut state,
        Message::CompareTransientLoaded(Box::new(transient_default(34))),
      );

      assert_eq!(state.compare_transient, None);
    }

    #[test]
    fn it_builds_a_fresh_transient_after_a_pin() {
      let mut state = State::new();
      state.selected = Some(34);
      state.compare_transient = Some(block(
        BlockId::Transient,
        34,
        vec![column(10_000_002, LocationTier::Region, None, None)],
      ));

      reduce(&mut state, Message::ComparePinRequested);
      assert_eq!(state.compare_transient, None);

      reduce(
        &mut state,
        Message::CompareTransientLoaded(Box::new(transient_default(34))),
      );

      let transient = state.compare_transient.expect("expected a fresh transient block");
      let ids: Vec<i64> = transient.columns.iter().map(|column| column.place.id).collect();
      assert_eq!(ids, DEFAULT_STATIONS.to_vec());
    }

    #[test]
    fn it_drops_a_pin_on_unpin_without_a_transient_drop_back() {
      let mut state = State::new();
      state.compare_pins = vec![block(
        BlockId::Pin(7),
        34,
        vec![column(60_003_760, LocationTier::Station, None, None)],
      )];

      reduce(&mut state, Message::CompareUnpinRequested(7));

      assert!(state.compare_pins.is_empty());
      assert_eq!(state.compare_transient, None);
    }

    #[test]
    fn it_keeps_the_last_column_on_a_remove() {
      let mut state = State::new();
      state.compare_transient = Some(block(
        BlockId::Transient,
        34,
        vec![column(60_003_760, LocationTier::Station, None, None)],
      ));

      reduce(
        &mut state,
        Message::CompareMarketRemoved(BlockId::Transient, 60_003_760),
      );

      assert_eq!(
        state.compare_transient.as_ref().map(|block| block.columns.len()),
        Some(1)
      );
    }

    #[test]
    fn it_removes_a_column_and_dismisses_the_menu() {
      let mut state = State::new();
      state.compare_transient = Some(block(
        BlockId::Transient,
        34,
        vec![
          column(60_003_760, LocationTier::Station, None, None),
          column(60_008_494, LocationTier::Station, None, None),
        ],
      ));
      reduce(&mut state, Message::CompareMenuOpened(BlockId::Transient, 60_003_760));

      reduce(
        &mut state,
        Message::CompareMarketRemoved(BlockId::Transient, 60_003_760),
      );

      let ids: Vec<i64> = state
        .compare_transient
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .map(|column| column.place.id)
        .collect();
      assert_eq!(ids, vec![60_008_494]);
      assert_eq!(state.compare_menu, None);
    }

    #[test]
    fn it_appends_a_pick_to_the_open_block_and_closes_the_modal() {
      let mut state = State::new();
      state.compare_transient = Some(block(
        BlockId::Transient,
        34,
        vec![column(60_003_760, LocationTier::Station, None, None)],
      ));
      reduce(&mut state, Message::CompareAddPickerOpened(BlockId::Transient));

      reduce(
        &mut state,
        Message::CompareMarketPicked(place(10_000_002, LocationTier::Region)),
      );

      let ids: Vec<i64> = state
        .compare_transient
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .map(|column| column.place.id)
        .collect();
      assert_eq!(ids, vec![60_003_760, 10_000_002]);
      assert_eq!(state.compare_add_target, None);
    }

    #[test]
    fn it_marks_a_column_no_access_from_a_structure_miss() {
      let mut state = State::new();
      state.compare_transient = Some(block(
        BlockId::Transient,
        34,
        vec![column(1_035_000_000_001, LocationTier::Structure, None, None)],
      ));

      reduce(
        &mut state,
        Message::CompareStructureBookLoaded(1_035_000_000_001, 34, StructureBook::NoAccess),
      );

      assert_eq!(
        state.compare_transient.as_ref().unwrap().columns[0].access,
        BookAccess::NoAccess
      );
    }
  }

  mod plan {
    use super::*;

    #[test]
    fn it_promotes_the_transient_into_a_pin() {
      let mut state = State::new();
      state.compare_transient = Some(transient_default(34));

      match plan(&state, &Message::ComparePinRequested) {
        Follow::CreatePin(block) => assert_eq!(block.type_id, 34),
        _ => panic!("expected a create-pin follow"),
      }
    }

    #[test]
    fn it_plans_nothing_for_a_pin_without_a_transient() {
      let state = State::new();

      assert!(matches!(plan(&state, &Message::ComparePinRequested), Follow::None));
    }

    #[test]
    fn it_persists_a_pick_into_the_target_pin() {
      let mut state = State::new();
      state.compare_pins = vec![block(
        BlockId::Pin(7),
        34,
        vec![column(60_003_760, LocationTier::Station, None, None)],
      )];
      state.compare_add_target = Some(BlockId::Pin(7));

      let follow = plan(
        &state,
        &Message::CompareMarketPicked(place(10_000_002, LocationTier::Region)),
      );

      match follow {
        Follow::AddPinMarket(pin_id, place) => {
          assert_eq!(pin_id, 7);
          assert_eq!(place.id, 10_000_002);
        }
        _ => panic!("expected an add-pin-market follow"),
      }
    }

    #[test]
    fn it_fetches_a_transient_pick_without_persisting() {
      let mut state = State::new();
      state.compare_transient = Some(transient_default(34));
      state.compare_add_target = Some(BlockId::Transient);

      let follow = plan(
        &state,
        &Message::CompareMarketPicked(place(10_000_002, LocationTier::Region)),
      );

      match follow {
        Follow::FetchOne(type_id, place) => {
          assert_eq!(type_id, 34);
          assert_eq!(place.id, 10_000_002);
        }
        _ => panic!("expected a fetch-one follow"),
      }
    }

    #[test]
    fn it_floors_pin_market_removal_at_one_column() {
      let mut state = State::new();
      let mut only = column(60_003_760, LocationTier::Station, None, None);
      only.row = Some(11);
      state.compare_pins = vec![block(BlockId::Pin(7), 34, vec![only])];

      let follow = plan(&state, &Message::CompareMarketRemoved(BlockId::Pin(7), 60_003_760));

      assert!(matches!(follow, Follow::None));
    }

    #[test]
    fn it_removes_a_pin_market_by_its_row() {
      let mut state = State::new();
      let mut first = column(60_003_760, LocationTier::Station, None, None);
      first.row = Some(11);
      let mut second = column(60_008_494, LocationTier::Station, None, None);
      second.row = Some(12);
      state.compare_pins = vec![block(BlockId::Pin(7), 34, vec![first, second])];

      let follow = plan(&state, &Message::CompareMarketRemoved(BlockId::Pin(7), 60_008_494));

      assert!(matches!(follow, Follow::RemovePinMarket(12)));
    }
  }

  mod drag {
    use pretty_assertions::assert_eq;

    use super::*;

    fn pin(id: i64) -> CompareBlock {
      block(
        BlockId::Pin(id),
        34,
        vec![column(60_003_760, LocationTier::Station, None, None)],
      )
    }

    fn seeded(ids: &[i64]) -> State {
      let mut state = State::new();
      state.compare_pins = ids.iter().copied().map(pin).collect();
      state
    }

    fn ids(state: &State) -> Vec<i64> {
      state.compare_pins.iter().filter_map(CompareBlock::pin_id).collect()
    }

    #[test]
    fn it_arms_the_drag_from_a_grip_press() {
      let mut state = seeded(&[1, 2, 3]);

      reduce(&mut state, Message::CompareDragStarted(2));

      assert_eq!(state.compare_dragging, Some(2));
      assert_eq!(state.compare_drop_target, None);
    }

    #[test]
    fn it_tracks_a_drop_target_only_while_dragging() {
      let mut state = seeded(&[1, 2, 3]);

      reduce(&mut state, Message::CompareDropEntered(3));
      assert_eq!(state.compare_drop_target, None);

      reduce(&mut state, Message::CompareDragStarted(1));
      reduce(&mut state, Message::CompareDropEntered(3));
      assert_eq!(state.compare_drop_target, Some(3));
    }

    #[test]
    fn it_never_targets_the_dragged_block_itself() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::CompareDragStarted(1));

      reduce(&mut state, Message::CompareDropEntered(1));

      assert_eq!(state.compare_drop_target, None);
    }

    #[test]
    fn it_clears_only_a_matching_target_on_exit() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::CompareDragStarted(1));
      reduce(&mut state, Message::CompareDropEntered(3));

      reduce(&mut state, Message::CompareDropExited(2));
      assert_eq!(state.compare_drop_target, Some(3));

      reduce(&mut state, Message::CompareDropExited(3));
      assert_eq!(state.compare_drop_target, None);
    }

    #[test]
    fn it_splices_the_dragged_block_to_the_target_index_on_release() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::CompareDragStarted(1));
      reduce(&mut state, Message::CompareDropEntered(3));

      reduce(&mut state, Message::CompareDropReleased);

      assert_eq!(ids(&state), vec![2, 3, 1]);
      assert_eq!(state.compare_dragging, None);
      assert_eq!(state.compare_drop_target, None);
    }

    #[test]
    fn it_keeps_the_order_on_a_release_without_a_target() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::CompareDragStarted(2));
      reduce(&mut state, Message::CompareDropEntered(2));

      reduce(&mut state, Message::CompareDropReleased);

      assert_eq!(ids(&state), vec![1, 2, 3]);
      assert_eq!(state.compare_dragging, None);
    }

    #[test]
    fn it_plans_a_persist_only_for_a_real_move() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::CompareDragStarted(1));
      assert!(matches!(plan(&state, &Message::CompareDropReleased), Follow::None));

      reduce(&mut state, Message::CompareDropEntered(3));

      assert!(matches!(
        plan(&state, &Message::CompareDropReleased),
        Follow::PersistOrder
      ));
    }

    #[test]
    fn it_tracks_grip_hover_per_block() {
      let mut state = seeded(&[1, 2]);

      reduce(&mut state, Message::CompareGripEntered(1));
      assert_eq!(state.compare_grip_hover, Some(1));

      reduce(&mut state, Message::CompareGripExited(2));
      assert_eq!(state.compare_grip_hover, Some(1));

      reduce(&mut state, Message::CompareGripExited(1));
      assert_eq!(state.compare_grip_hover, None);
    }
  }

  mod menu {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_a_menu_for_a_known_place_at_the_cursor() {
      let mut state = State::new();
      state.compare_transient = Some(transient_default(34));
      reduce(&mut state, Message::CompareCursorMoved(Point::new(40.0, 60.0)));

      reduce(&mut state, Message::CompareMenuOpened(BlockId::Transient, 60_003_760));

      assert_eq!(
        state.compare_menu,
        Some(CompareMenu {
          anchor: Point::new(40.0, 60.0),
          block: BlockId::Transient,
          place_id: 60_003_760,
        })
      );
    }

    #[test]
    fn it_ignores_a_menu_request_for_an_unknown_place() {
      let mut state = State::new();

      reduce(&mut state, Message::CompareMenuOpened(BlockId::Transient, 60_003_760));

      assert_eq!(state.compare_menu, None);
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

  mod persistence {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_persists_and_restores_a_pinned_block() {
      let db = store::open_test().await.unwrap();
      let transient = block(
        BlockId::Transient,
        34,
        vec![
          column(60_003_760, LocationTier::Station, None, None),
          column(10_000_002, LocationTier::Region, None, None),
        ],
      );

      persist_pin(&db, &transient).await.unwrap();
      let blocks = load_pins(db).await;

      assert_eq!(blocks.len(), 1);
      assert_eq!(blocks[0].type_id, 34);
      let ids: Vec<i64> = blocks[0].columns.iter().map(|column| column.place.id).collect();
      assert_eq!(ids, vec![60_003_760, 10_000_002]);
      assert!(blocks[0].columns.iter().all(|column| column.row.is_some()));
      assert_eq!(blocks[0].columns[1].place.tier, Some(LocationTier::Region));
    }

    #[tokio::test]
    async fn it_pins_the_same_item_twice_with_different_sets() {
      let db = store::open_test().await.unwrap();
      let first = block(
        BlockId::Transient,
        34,
        vec![column(60_003_760, LocationTier::Station, None, None)],
      );
      let second = block(
        BlockId::Transient,
        34,
        vec![column(60_008_494, LocationTier::Station, None, None)],
      );

      persist_pin(&db, &first).await.unwrap();
      persist_pin(&db, &second).await.unwrap();
      let blocks = load_pins(db).await;

      assert_eq!(blocks.len(), 2);
      assert_ne!(blocks[0].id, blocks[1].id);
      assert!(blocks.iter().all(|block| block.type_id == 34));
    }

    #[tokio::test]
    async fn it_restores_a_persisted_reorder() {
      let db = store::open_test().await.unwrap();
      persist_pin(&db, &transient_default(34)).await.unwrap();
      persist_pin(&db, &transient_default(35)).await.unwrap();
      persist_pin(&db, &transient_default(36)).await.unwrap();
      let before: Vec<i64> = load_pins(db.clone())
        .await
        .iter()
        .filter_map(CompareBlock::pin_id)
        .collect();

      let reordered = vec![before[2], before[0], before[1]];
      market_comparison_pin::reorder(&db, &reordered).await.unwrap();
      let after: Vec<i64> = load_pins(db).await.iter().filter_map(CompareBlock::pin_id).collect();

      assert_eq!(after, reordered);
    }

    #[tokio::test]
    async fn it_deletes_a_pin_and_its_markets_on_unpin() {
      let db = store::open_test().await.unwrap();
      persist_pin(&db, &transient_default(34)).await.unwrap();
      let pinned = load_pins(db.clone()).await;
      let pin_id = pinned[0].pin_id().unwrap();

      market_comparison_pin::delete(&db, pin_id).await.unwrap();

      assert!(load_pins(db).await.is_empty());
    }
  }
}
