use std::sync::Arc;

use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Point, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, image, mouse_area, scrollable, svg, text},
};

use super::{
  Message, State, WatchCard, WatchMenu,
  tree::{MarketNode, MarketTree},
  watch_eval,
};
use crate::{
  clients::{esi, eve_image::Size as ImageSize, eve_sso},
  services::location_search::{LocationRef, LocationTier},
  store::{
    Database,
    images::{self, IconResolution},
    model::{MarketWatch, NewWatch, WatchDirection},
    repo::{market_watchlist, sde},
  },
  ui::{
    components::{
      backdrop,
      button::{Button, Size},
      clip::clip_layer,
      context_menu::{self, Item},
      eyebrow::eyebrow_text,
      icon::Icon,
      icon_tile::icon_tile,
      location_combobox::{LocationCombobox, LocationSearch},
      modal_overlay,
      text_input::TextInput,
    },
    format::fmt_isk_opt,
    style::{color, control, radius, shadow, spacing, typography},
  },
};

const CARD_WIDTH: f32 = 460.0;
const BODY_MAX_HEIGHT: f32 = 460.0;
const ITEM_LIST_HEIGHT: f32 = 260.0;
const DIRECTION_PAD_Y: f32 = 9.0;
const FIELD_HEIGHT: f32 = 42.0;
const MAX_ITEM_RESULTS: usize = 50;

const CARD_MIN_WIDTH: f32 = 330.0;
const CARD_GAP: f32 = spacing::SPACE_3_5;
const DRAG_SCRIM_ALPHA: f32 = 0.6;
const DROP_HIGHLIGHT_HEIGHT: f32 = 2.0;
const DROP_HIGHLIGHT_INSET: f32 = 6.0;
const GRIP_CONTAINER_WIDTH: f32 = 16.0;
const GRIP_SVG_HEIGHT: f32 = 16.0;
const GRIP_SVG_WIDTH: f32 = 10.0;
const CARD_ICON_IMAGE: ImageSize = ImageSize::S64;
const CARD_ICON_TILE: f32 = 30.0;
const CARD_ICON_SIZE: f32 = 12.0;
const MET_BORDER_ALPHA: f32 = 0.32;
const EMPTY_COPY_WIDTH: f32 = 360.0;
const EMPTY_VERTICAL_PADDING: f32 = 56.0;
const EMPTY_HORIZONTAL_PADDING: f32 = 32.0;

// ── Item identity ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub(super) struct WatchItem {
  pub name: String,
  pub type_id: i64,
}

struct DragVisual {
  dragging: bool,
  is_over: bool,
}

struct FlatItem {
  group: String,
  name: String,
  type_id: i64,
}

// ── Modal form ────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub(super) struct WatchForm {
  direction: WatchDirection,
  editing: Option<i64>,
  item: Option<WatchItem>,
  item_picker_open: bool,
  item_query: String,
  region: Option<LocationRef>,
  region_picker_open: bool,
  region_search: LocationSearch,
  target: String,
}

impl WatchForm {
  fn new(region: Option<LocationRef>) -> Self {
    Self {
      direction: WatchDirection::Buy,
      editing: None,
      item: None,
      item_picker_open: false,
      item_query: String::new(),
      region,
      region_picker_open: false,
      region_search: LocationSearch::default(),
      target: String::new(),
    }
  }

  fn editing(watch: &MarketWatch, tree: &MarketTree, location_name: String) -> Self {
    let item = find_item(tree, watch.type_id);
    let region = watch.location_id.or(watch.region_id).map(|id| {
      let tier = watch
        .location_tier
        .as_deref()
        .and_then(LocationTier::parse)
        .or_else(|| LocationTier::from_id(id))
        .unwrap_or(LocationTier::Region);
      scope_location(id, location_name.clone(), tier)
    });
    Self {
      direction: WatchDirection::parse(&watch.direction).unwrap_or_default(),
      editing: Some(watch.id),
      item,
      item_picker_open: false,
      item_query: String::new(),
      region,
      region_picker_open: false,
      region_search: LocationSearch::default(),
      target: watch.target_price.map(|price| price.to_string()).unwrap_or_default(),
    }
  }

  fn toggle_item_picker(&mut self) {
    self.item_picker_open = !self.item_picker_open;
    if !self.item_picker_open {
      self.item_query.clear();
    }
  }

  fn set_item_query(&mut self, query: String) {
    self.item_query = query;
  }

  fn pick_item(&mut self, type_id: i64, name: String) {
    self.item = Some(WatchItem {
      name,
      type_id,
    });
    self.item_picker_open = false;
    self.item_query.clear();
  }

  fn set_direction(&mut self, direction: WatchDirection) {
    self.direction = direction;
  }

  fn set_target(&mut self, target: String) {
    self.target = target;
  }

  fn toggle_region_picker(&mut self) {
    self.region_picker_open = !self.region_picker_open;
    if !self.region_picker_open {
      self.region_search.clear();
    }
  }

  fn set_region_query(&mut self, query: String) {
    self.region_search.set_query(query);
  }

  fn accept_region_results(&mut self, generation: u64, results: Vec<LocationRef>) {
    self.region_search.accept_results(generation, results);
  }

  fn pick_region(&mut self, region: LocationRef) {
    self.region = Some(region);
    self.region_picker_open = false;
    self.region_search.clear();
  }

  fn target_value(&self) -> Option<f64> {
    let cleaned: String = self
      .target
      .chars()
      .filter(|c| c.is_ascii_digit() || *c == '.')
      .collect();
    match cleaned.parse::<f64>() {
      Ok(value) if value > 0.0 => Some(value),
      _ => None,
    }
  }

  fn is_valid(&self) -> bool {
    self.item.is_some() && self.target_value().is_some()
  }
}

pub(super) struct WatchSubmit {
  direction: WatchDirection,
  editing: Option<i64>,
  location: Option<LocationRef>,
  target_price: Option<f64>,
  type_id: i64,
}

fn scope_location(id: i64, name: String, tier: LocationTier) -> LocationRef {
  LocationRef {
    context: None,
    id,
    name,
    security_status: None,
    tier: Some(tier),
  }
}

fn to_submit(form: &WatchForm) -> Option<WatchSubmit> {
  let item = form.item.as_ref()?;
  let target_price = form.target_value()?;
  Some(WatchSubmit {
    direction: form.direction,
    editing: form.editing,
    location: form.region.clone(),
    target_price: Some(target_price),
    type_id: item.type_id,
  })
}

fn browse_submit(state: &State) -> Option<WatchSubmit> {
  let type_id = state.selected?;
  let location = state.active_location()?.clone();
  if watched_at(&state.watches, type_id, location.id) {
    return None;
  }
  Some(WatchSubmit {
    direction: WatchDirection::Sell,
    editing: None,
    target_price: state.book.as_ref().and_then(|book| book.best_sell),
    location: Some(location),
    type_id,
  })
}

fn compare_submit(state: &State, block_id: super::compare::BlockId) -> Option<WatchSubmit> {
  let block = super::compare::find_block(state, block_id)?;
  let column = block.columns.first()?;
  if watched_at(&state.watches, block.type_id, column.place.id) {
    return None;
  }
  Some(WatchSubmit {
    direction: WatchDirection::Sell,
    editing: None,
    location: Some(column.place.clone()),
    target_price: column.best_sell(),
    type_id: block.type_id,
  })
}

pub(super) fn is_block_watched(state: &State, block: &super::compare::CompareBlock) -> bool {
  block
    .columns
    .first()
    .is_some_and(|column| watched_at(&state.watches, block.type_id, column.place.id))
}

pub(super) fn is_watched(state: &State, type_id: i64) -> bool {
  state
    .active_location()
    .is_some_and(|location| watched_at(&state.watches, type_id, location.id))
}

fn watched_at(watches: &[WatchCard], type_id: i64, market_id: i64) -> bool {
  watches.iter().any(|card| {
    card.type_id == type_id
      // Watches saved before per-market scoping only recorded a region_id; fall back to matching
      // on region when location_id is unset.
      && (card.watch.location_id == Some(market_id)
        || (card.watch.location_id.is_none() && card.watch.region_id == Some(market_id)))
  })
}

// ── Reducer + follow-ups ──────────────────────────────────────

enum Follow {
  Book,
  None,
  Persist(WatchSubmit),
  PersistOrder,
}

// Returns `Ok(task)` for a watchlist-modal message it fully handled, or `Err(message)` to hand the
// message back to the market reducer untouched.
pub(super) fn try_dispatch(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  match &message {
    Message::WatchNew
    | Message::WatchEdit(_)
    | Message::WatchModalClosed
    | Message::WatchItemPickerToggled
    | Message::WatchItemSearchChanged(_)
    | Message::WatchItemPicked(..)
    | Message::WatchDirectionSelected(_)
    | Message::WatchTargetChanged(_)
    | Message::WatchRegionPickerToggled
    | Message::WatchRegionSearchChanged(_)
    | Message::WatchRegionResultsLoaded(..)
    | Message::WatchRegionPicked(_)
    | Message::WatchSubmitted
    | Message::BrowseWatchSubmitted
    | Message::CompareWatchSubmitted(_)
    | Message::WatchDragStarted(_)
    | Message::WatchDropEntered(_)
    | Message::WatchDropExited(_)
    | Message::WatchDropReleased
    | Message::WatchGripEntered(_)
    | Message::WatchGripExited(_) => {}
    _ => return Err(message),
  }
  Ok(apply(state, message, db))
}

fn apply(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  let follow = plan(state, &message);
  reduce(state, message);
  execute(state, db, follow)
}

fn plan(state: &State, message: &Message) -> Follow {
  match message {
    Message::WatchItemPicked(..) | Message::WatchRegionPicked(_) => Follow::Book,
    Message::WatchSubmitted => state
      .watch_modal
      .as_ref()
      .and_then(to_submit)
      .map_or(Follow::None, Follow::Persist),
    Message::BrowseWatchSubmitted => browse_submit(state).map_or(Follow::None, Follow::Persist),
    Message::CompareWatchSubmitted(block_id) => compare_submit(state, *block_id).map_or(Follow::None, Follow::Persist),
    Message::WatchDropReleased => release_follow(state),
    _ => Follow::None,
  }
}

fn release_follow(state: &State) -> Follow {
  match state.dragging_watch.zip(state.watch_drop_target) {
    Some((dragged, target)) if dragged != target => Follow::PersistOrder,
    _ => Follow::None,
  }
}

pub(super) fn reduce(state: &mut State, message: Message) {
  match message {
    Message::WatchNew => state.watch_modal = Some(WatchForm::new(state.active_region.clone())),
    Message::WatchEdit(watch) => {
      state.watch_menu = None;
      let location_name = card_location_label(state, watch.id);
      state.watch_modal = Some(WatchForm::editing(&watch, &state.tree, location_name));
    }
    Message::WatchCursorMoved(_)
    | Message::WatchMenuOpened(_)
    | Message::WatchMenuDismissed
    | Message::WatchRemoved(_) => {
      reduce_menu(state, message);
    }
    Message::WatchModalClosed | Message::WatchSubmitted => state.watch_modal = None,
    Message::BrowseWatchSubmitted => adopt_browse_watch(state),
    Message::CompareWatchSubmitted(block_id) => adopt_compare_watch(state, block_id),
    Message::WatchItemPickerToggled => with_form(state, WatchForm::toggle_item_picker),
    Message::WatchItemSearchChanged(query) => with_form(state, |form| form.set_item_query(query)),
    Message::WatchItemPicked(type_id, name) => with_form(state, |form| form.pick_item(type_id, name)),
    Message::WatchDirectionSelected(direction) => with_form(state, |form| form.set_direction(direction)),
    Message::WatchTargetChanged(target) => with_form(state, |form| form.set_target(target)),
    Message::WatchDragStarted(_)
    | Message::WatchDropEntered(_)
    | Message::WatchDropExited(_)
    | Message::WatchDropReleased
    | Message::WatchGripEntered(_)
    | Message::WatchGripExited(_) => reduce_drag(state, message),
    _ => reduce_region(state, message),
  }
}

fn reduce_drag(state: &mut State, message: Message) {
  match message {
    Message::WatchDragStarted(id) => {
      state.dragging_watch = Some(id);
      state.watch_drop_target = None;
    }
    Message::WatchDropEntered(id) if state.dragging_watch.is_some() && state.dragging_watch != Some(id) => {
      state.watch_drop_target = Some(id);
    }
    Message::WatchDropExited(id) if state.watch_drop_target == Some(id) => {
      state.watch_drop_target = None;
    }
    Message::WatchDropReleased => release_drop(state),
    Message::WatchGripEntered(id) => state.watch_grip_hover = Some(id),
    Message::WatchGripExited(id) if state.watch_grip_hover == Some(id) => {
      state.watch_grip_hover = None;
    }
    _ => {}
  }
}

fn release_drop(state: &mut State) {
  let drop = state.dragging_watch.take().zip(state.watch_drop_target.take());
  if let Some((dragged, target)) = drop {
    splice_watches(&mut state.watches, dragged, target);
  }
}

fn splice_watches(cards: &mut Vec<WatchCard>, dragged: i64, target: i64) {
  let from = cards.iter().position(|card| card.watch.id == dragged);
  let to = cards.iter().position(|card| card.watch.id == target);
  if let Some((from, to)) = from.zip(to)
    && from != to
  {
    // `to` is the target's index before removal, so this drops the card after the target when
    // dragging downward (from < to) and before it when dragging upward (from > to).
    let moved = cards.remove(from);
    cards.insert(to, moved);
  }
}

fn reduce_region(state: &mut State, message: Message) {
  match message {
    Message::WatchRegionPickerToggled => with_form(state, WatchForm::toggle_region_picker),
    Message::WatchRegionSearchChanged(query) => with_form(state, |form| form.set_region_query(query)),
    Message::WatchRegionResultsLoaded(generation, results) => {
      with_form(state, |form| form.accept_region_results(generation, results));
    }
    Message::WatchRegionPicked(region) => with_form(state, |form| form.pick_region(region)),
    _ => {}
  }
}

fn reduce_menu(state: &mut State, message: Message) {
  match message {
    Message::WatchCursorMoved(point) => state.watch_cursor = Some(point),
    Message::WatchMenuOpened(id) => open_menu(state, id),
    Message::WatchMenuDismissed | Message::WatchRemoved(_) => state.watch_menu = None,
    _ => {}
  }
}

fn open_menu(state: &mut State, id: i64) {
  let Some(watch) = state
    .watches
    .iter()
    .find(|card| card.watch.id == id)
    .map(|card| card.watch.clone())
  else {
    return;
  };
  let anchor = state.watch_cursor.unwrap_or(Point::ORIGIN);
  state.watch_menu = Some(WatchMenu {
    anchor,
    watch,
  });
}

fn adopt_browse_watch(state: &mut State) {
  let Some(submit) = browse_submit(state) else {
    return;
  };
  let card = pending_card(&submit, state.active_region.as_ref());
  state.watches.push(card);
}

fn adopt_compare_watch(state: &mut State, block_id: super::compare::BlockId) {
  let Some(submit) = compare_submit(state, block_id) else {
    return;
  };
  let card = pending_card(&submit, None);
  state.watches.push(card);
}

/// Builds a provisional card (`id: 0`, empty timestamps) so the grid and `is_watched`/`is_block_watched`
/// flip immediately on submit; the follow-up persist-and-fetch task replaces it with the saved row.
fn card_location_label(state: &State, watch_id: i64) -> String {
  state
    .watches
    .iter()
    .find(|card| card.watch.id == watch_id)
    .map(|card| card.location_label.clone())
    .unwrap_or_default()
}

fn pending_card(submit: &WatchSubmit, region: Option<&LocationRef>) -> WatchCard {
  let location = submit.location.as_ref();
  let tier = location.and_then(|place| place.tier.or_else(|| LocationTier::from_id(place.id)));
  WatchCard {
    direction: submit.direction,
    location_label: location.map(|place| place.name.clone()).unwrap_or_default(),
    region_id: region.map(|place| place.id),
    region_label: region.map(|place| place.name.clone()).unwrap_or_default(),
    system_label: String::new(),
    target: submit.target_price,
    type_id: submit.type_id,
    watch: MarketWatch {
      created_at: String::new(),
      direction: submit.direction.as_str().to_owned(),
      id: 0,
      location_id: location.map(|place| place.id),
      location_tier: tier.map(|tier| tier.as_str().to_owned()),
      region_id: region.map(|place| place.id),
      target_price: submit.target_price,
      type_id: submit.type_id,
      updated_at: String::new(),
    },
  }
}

fn with_form(state: &mut State, apply: impl FnOnce(&mut WatchForm)) {
  if let Some(form) = state.watch_modal.as_mut() {
    apply(form);
  }
}

fn execute(state: &State, db: &Database, follow: Follow) -> Task<Message> {
  match follow {
    Follow::None => Task::none(),
    Follow::Book => fetch_book_task(state, db),
    Follow::Persist(submit) => Task::perform(persist_and_fetch(db.clone(), submit), Message::WatchesLoaded),
    Follow::PersistOrder => persist_order_task(state, db),
  }
}

async fn persist_and_fetch(db: Database, submit: WatchSubmit) -> Vec<super::WatchCard> {
  persist(db.clone(), submit).await;
  super::fetch_watches(db).await
}

fn persist_order_task(state: &State, db: &Database) -> Task<Message> {
  let ids: Vec<i64> = state.watches.iter().map(|card| card.watch.id).collect();
  Task::perform(persist_order_and_fetch(db.clone(), ids), Message::WatchesLoaded)
}

async fn persist_order_and_fetch(db: Database, ids: Vec<i64>) -> Vec<super::WatchCard> {
  let _ = market_watchlist::reorder(&db, &ids).await;
  super::fetch_watches(db).await
}

fn fetch_book_task(state: &State, db: &Database) -> Task<Message> {
  let Some(form) = state.watch_modal.as_ref() else {
    return Task::none();
  };
  match (form.region.as_ref(), form.item.as_ref().map(|item| item.type_id)) {
    (Some(location), Some(type_id)) if location.tier == Some(LocationTier::Region) => {
      super::load_book(db, location.id, type_id, None)
    }
    _ => Task::none(),
  }
}

pub(super) fn watch_location_search(
  state: &State,
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
  query: String,
) -> Task<Message> {
  let Some(form) = state.watch_modal.as_ref() else {
    return Task::none();
  };
  if !form.region_search.searchable() {
    return Task::none();
  }
  let generation = form.region_search.generation();
  Task::perform(
    crate::services::location_search::search_locations_enriched(
      db.clone(),
      esi,
      sso,
      query,
      super::LOCATION_SEARCH_MIN_CHARS,
    ),
    move |results| Message::WatchRegionResultsLoaded(generation, results),
  )
}

async fn persist(db: Database, submit: WatchSubmit) {
  let (location_id, location_tier, region_id) = scope_columns(&db, submit.location.as_ref()).await;
  let new = NewWatch {
    direction: submit.direction,
    location_id,
    location_tier,
    region_id,
    target_price: submit.target_price,
    type_id: submit.type_id,
  };

  match submit.editing {
    Some(id) => {
      let _ = market_watchlist::update(&db, id, &new).await;
    }
    None => {
      let _ = market_watchlist::create(&db, &new).await;
    }
  }
}

async fn scope_columns(db: &Database, location: Option<&LocationRef>) -> (Option<i64>, Option<String>, Option<i64>) {
  let Some(location) = location else {
    return (None, None, None);
  };
  let tier = location
    .tier
    .or_else(|| LocationTier::from_id(location.id))
    .unwrap_or(LocationTier::Region);
  let region_id = scope_region(db, location.id, tier).await;
  (Some(location.id), Some(tier.as_str().to_owned()), region_id)
}

async fn scope_region(db: &Database, id: i64, tier: LocationTier) -> Option<i64> {
  match tier {
    LocationTier::Structure => {
      let structure = sde::get_structure(db, id).await.ok().flatten()?;
      super::region_of_system(db, structure.solar_system_id()).await
    }
    _ => super::region_of(db, id).await,
  }
}

// ── Catalog helpers ───────────────────────────────────────────

fn flat_items(tree: &MarketTree) -> Vec<FlatItem> {
  let mut out = Vec::new();
  for node in &tree.roots {
    collect_items(node, &mut out);
  }
  out.sort_by(|left, right| left.name.cmp(&right.name));
  out
}

fn collect_items(node: &MarketNode, out: &mut Vec<FlatItem>) {
  for leaf in &node.items {
    out.push(FlatItem {
      group: node.name.clone(),
      name: leaf.name.clone(),
      type_id: leaf.type_id,
    });
  }
  for child in &node.children {
    collect_items(child, out);
  }
}

fn find_item(tree: &MarketTree, type_id: i64) -> Option<WatchItem> {
  flat_items(tree)
    .into_iter()
    .find(|item| item.type_id == type_id)
    .map(|item| WatchItem {
      name: item.name,
      type_id: item.type_id,
    })
}

// ── Watchlist tab ─────────────────────────────────────────────

pub(super) fn surface(state: &State) -> Element<'_, Message> {
  let cards = state.watches();
  let prices = state.watch_prices();
  let store = images::default_store();

  let body: Element<'_, Message> = if cards.is_empty() {
    empty_card()
  } else {
    grid(state, &store)
  };

  let inner = Column::with_children(vec![targets_header(cards.len(), count_met(cards, prices)), body])
    .spacing(spacing::SPACE_4_5)
    .width(Length::Fill);

  scrollable(container(inner).width(Length::Fill).padding(Padding {
    top: 20.0,
    right: 28.0,
    bottom: 36.0,
    left: 28.0,
  }))
  .style(control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn targets_header<'a>(total: usize, met: usize) -> Element<'a, Message> {
  let new_button: Element<'a, Message> = Button::primary(tr("market.watch.new_button"))
    .icon(Icon::plus())
    .size(Size::Sm)
    .on_press(Message::WatchNew)
    .into();

  Row::with_children(vec![
    text(t!("market.watchlist_targets_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    count_summary(met, total.saturating_sub(met)),
    Space::new().width(Length::Fill).into(),
    new_button,
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center)
  .into()
}

fn count_summary<'a>(met: usize, tracking: usize) -> Element<'a, Message> {
  let met_color = if met > 0 {
    color::status::ONLINE
  } else {
    color::text::tertiary()
  };
  let tracking_color = if tracking > 0 {
    color::status::WARNING
  } else {
    color::text::tertiary()
  };

  Row::with_children(vec![
    count_pill(t!("market.watchlist_count_met", count => met).into_owned(), met_color),
    count_pill("·".to_owned(), color::text::tertiary()),
    count_pill(
      t!("market.watchlist_count_tracking", count => tracking).into_owned(),
      tracking_color,
    ),
  ])
  .spacing(spacing::UNIT + 3.0)
  .align_y(Vertical::Center)
  .into()
}

fn count_pill<'a>(label: String, fill: iced::Color) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(fill))
    .into()
}

fn grid<'a>(state: &'a State, store: &images::Store) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = state
    .watches()
    .iter()
    .map(|card| watch_card(card, state, store))
    .collect();
  Row::with_children(cells).spacing(CARD_GAP).wrap().into()
}

fn count_met(cards: &[WatchCard], prices: &watch_eval::PriceMap) -> usize {
  cards.iter().filter(|card| outcome_of(card, prices).met).count()
}

fn scope_prices(card: &WatchCard, prices: &watch_eval::PriceMap) -> watch_eval::BestPrices {
  card
    .watch
    .location_id
    .or(card.region_id)
    .and_then(|scope_id| prices.get(&(card.type_id, scope_id)).copied())
    .unwrap_or_default()
}

fn outcome_of(card: &WatchCard, prices: &watch_eval::PriceMap) -> watch_eval::WatchOutcome {
  watch_eval::evaluate(card.direction, card.target, &scope_prices(card, prices))
}

// ── Card ──────────────────────────────────────────────────────

fn watch_card<'a>(card: &'a WatchCard, state: &'a State, store: &images::Store) -> Element<'a, Message> {
  let best = scope_prices(card, state.watch_prices());
  let outcome = watch_eval::evaluate(card.direction, card.target, &best);
  let visual = drag_visual(state, card.watch.id);
  let is_over = visual.is_over;

  let content = Column::with_children(vec![
    card_identity(card, state, store),
    price_row(outcome.current, card.target),
    card_footer(outcome, card.target, best.access),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill);

  let panel = container(content)
    .width(Length::Fixed(CARD_MIN_WIDTH))
    .padding(Padding {
      top: 14.0,
      right: 16.0,
      bottom: 14.0,
      left: 14.0,
    })
    .style(move |_| card_style(outcome.met, is_over));

  card_mouse_area(
    drag_layers(panel.into(), &visual),
    card.watch.id,
    state.dragging_watch.is_some(),
  )
}

fn drag_visual(state: &State, id: i64) -> DragVisual {
  DragVisual {
    dragging: state.dragging_watch == Some(id),
    is_over: state.watch_drop_target == Some(id),
  }
}

fn card_mouse_area<'a>(panel: Element<'a, Message>, id: i64, drag_active: bool) -> Element<'a, Message> {
  let area = mouse_area(panel).on_right_press(Message::WatchMenuOpened(id));
  if drag_active {
    area
      .on_enter(Message::WatchDropEntered(id))
      .on_exit(Message::WatchDropExited(id))
      .into()
  } else {
    area.into()
  }
}

fn drag_layers<'a>(panel: Element<'a, Message>, visual: &DragVisual) -> Element<'a, Message> {
  if visual.dragging {
    Stack::new().push(panel).push(drag_scrim()).into()
  } else if visual.is_over {
    Stack::new().push(panel).push(drop_highlight()).into()
  } else {
    panel
  }
}

fn drag_scrim<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(
        color::surface::BASE,
        DRAG_SCRIM_ALPHA,
      ))),
      border: Border {
        radius: radius::CARD.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn drop_highlight<'a>() -> Element<'a, Message> {
  container(
    container(Space::new())
      .width(Length::Fill)
      .height(Length::Fixed(DROP_HIGHLIGHT_HEIGHT))
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent())),
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 1.0,
    right: DROP_HIGHLIGHT_INSET,
    bottom: 0.0,
    left: DROP_HIGHLIGHT_INSET,
  })
  .into()
}

fn card_style(met: bool, is_over: bool) -> container::Style {
  let border_color = if is_over {
    color::accent()
  } else if met {
    color::with_alpha(color::status::ONLINE, MET_BORDER_ALPHA)
  } else {
    color::rule()
  };
  container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  }
}

fn card_identity<'a>(card: &'a WatchCard, state: &'a State, store: &images::Store) -> Element<'a, Message> {
  let identity = Column::with_children(vec![
    text(item_name(&state.tree, card.type_id))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(subtitle(card))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .width(Length::Fill);

  Row::with_children(vec![
    grip_handle(card.watch.id, state.watch_grip_hover == Some(card.watch.id)),
    card_icon(store, card.type_id),
    identity.into(),
    direction_chip(card.direction),
    card_kebab(card.watch.id),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn grip_handle<'a>(id: i64, hovered: bool) -> Element<'a, Message> {
  let tint = if hovered {
    color::text::secondary()
  } else {
    color::text::tertiary()
  };
  let glyph = svg(Icon::grip().handle())
    .width(Length::Fixed(GRIP_SVG_WIDTH))
    .height(Length::Fixed(GRIP_SVG_HEIGHT))
    .style(move |_, _| svg::Style {
      color: Some(tint),
    });

  mouse_area(
    container(glyph)
      .width(Length::Fixed(GRIP_CONTAINER_WIDTH))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .interaction(iced::mouse::Interaction::Grab)
  .on_press(Message::WatchDragStarted(id))
  .on_enter(Message::WatchGripEntered(id))
  .on_exit(Message::WatchGripExited(id))
  .into()
}

fn card_kebab<'a>(id: i64) -> Element<'a, Message> {
  button(
    text("\u{22ef}")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(typography::colored(color::text::tertiary())),
  )
  .padding(Padding {
    top: 0.0,
    right: spacing::UNIT,
    bottom: 0.0,
    left: spacing::UNIT,
  })
  .on_press(Message::WatchMenuOpened(id))
  .style(|_, _| button::Style {
    background: None,
    ..button::Style::default()
  })
  .into()
}

fn card_icon<'a>(store: &images::Store, type_id: i64) -> Element<'a, Message> {
  let content: Element<'a, Message> = match store.resolve_type_icon(type_id, None, CARD_ICON_IMAGE) {
    IconResolution::Found(path) => clip_layer(
      image(image::Handle::from_path(path))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
      Length::Fill,
      Length::Fill,
    ),
    IconResolution::Missing => Space::new().into(),
  };
  icon_tile(content, CARD_ICON_TILE)
}

fn direction_chip<'a>(direction: WatchDirection) -> Element<'a, Message> {
  let (key, tint) = match direction {
    WatchDirection::Buy => ("market.watch.direction_buy", color::status::DANGER),
    WatchDirection::Sell => ("market.watch.direction_sell", color::status::ONLINE),
  };
  eyebrow_text(&t!(key), Some(tint)).into()
}

fn subtitle(card: &WatchCard) -> String {
  match (card.region_label.as_str(), card.system_label.as_str()) {
    ("", "") => String::new(),
    (region, "") => region.to_owned(),
    ("", system) => system.to_owned(),
    (region, system) => format!("{region} \u{b7} {system}"),
  }
}

fn price_row<'a>(current: Option<f64>, target: Option<f64>) -> Element<'a, Message> {
  Row::with_children(vec![
    price_block(
      "market.watchlist_current_label",
      fmt_isk_opt(current),
      typography::size::LG,
      color::text::PRIMARY,
      Horizontal::Left,
    ),
    price_block(
      "market.watchlist_target_label",
      fmt_isk_opt(target),
      typography::size::MD,
      color::text::secondary(),
      Horizontal::Right,
    ),
  ])
  .align_y(Vertical::Bottom)
  .into()
}

fn price_block<'a>(
  label_key: &str,
  value: String,
  value_size: f32,
  value_color: iced::Color,
  align: Horizontal,
) -> Element<'a, Message> {
  let block = Column::with_children(vec![
    eyebrow_text(&t!(label_key), None).into(),
    text(value)
      .font(typography::mono::REGULAR)
      .size(value_size)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(value_color))
      .into(),
  ])
  .spacing(spacing::UNIT - 1.0);

  container(block).width(Length::Fill).align_x(align).into()
}

fn card_footer<'a>(
  outcome: watch_eval::WatchOutcome,
  target: Option<f64>,
  access: watch_eval::PriceAccess,
) -> Element<'a, Message> {
  let content: Element<'a, Message> = if access == watch_eval::PriceAccess::Inaccessible {
    Row::with_children(vec![
      Icon::alert_triangle()
        .size(CARD_ICON_SIZE)
        .color(color::status::WARNING)
        .render(),
      eyebrow_text(&t!("market.watchlist_inaccessible"), Some(color::status::WARNING)).into(),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center)
    .into()
  } else if outcome.met {
    Row::with_children(vec![
      Icon::check().size(CARD_ICON_SIZE).color(color::status::ONLINE).render(),
      eyebrow_text(&t!("market.watchlist_target_met"), Some(color::status::ONLINE)).into(),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center)
    .into()
  } else {
    eyebrow_text(&distance_label(outcome.current, target), Some(color::text::secondary())).into()
  };

  Column::with_children(vec![divider(), container(content).width(Length::Fill).into()])
    .spacing(spacing::SPACE_2_5)
    .into()
}

fn divider<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn distance_label(current: Option<f64>, target: Option<f64>) -> String {
  match (current, target) {
    (Some(current), Some(target)) if target != 0.0 => {
      let pct = ((current - target) / target) * 100.0;
      if pct >= 0.0 {
        t!("market.watchlist_above_target", pct => format!("{pct:.1}")).into_owned()
      } else {
        t!("market.watchlist_below_target", pct => format!("{:.1}", pct.abs())).into_owned()
      }
    }
    _ => t!("market.watchlist_awaiting_data").into_owned(),
  }
}

fn item_name(tree: &MarketTree, type_id: i64) -> String {
  tree
    .roots
    .iter()
    .find_map(|node| find_leaf_name(node, type_id))
    .unwrap_or_else(|| t!("market.book_item_fallback", id => type_id).into_owned())
}

fn find_leaf_name(node: &MarketNode, type_id: i64) -> Option<String> {
  if let Some(leaf) = node.items.iter().find(|leaf| leaf.type_id == type_id) {
    return Some(leaf.name.clone());
  }
  node.children.iter().find_map(|child| find_leaf_name(child, type_id))
}

fn empty_card<'a>() -> Element<'a, Message> {
  let new_button: Element<'a, Message> = Button::primary(tr("market.watch.new_button"))
    .icon(Icon::plus())
    .on_press(Message::WatchNew)
    .into();

  let stack = Column::with_children(vec![
    container(
      Icon::star()
        .size(30.0)
        .color(color::with_alpha(color::text::PRIMARY, 0.24))
        .render(),
    )
    .padding(Padding {
      top: 0.0,
      right: 0.0,
      bottom: spacing::SPACE_2,
      left: 0.0,
    })
    .into(),
    text(t!("market.watchlist_empty_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    container(
      text(t!("market.watchlist_empty_body").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .wrapping(text::Wrapping::Word)
        .style(typography::colored(color::text::secondary())),
    )
    .max_width(EMPTY_COPY_WIDTH)
    .align_x(Horizontal::Center)
    .into(),
    container(new_button)
      .padding(Padding {
        top: spacing::SPACE_4_5,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
      })
      .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_x(Horizontal::Center);

  container(stack)
    .width(Length::Fill)
    .padding(Padding {
      top: EMPTY_VERTICAL_PADDING,
      right: EMPTY_HORIZONTAL_PADDING,
      bottom: EMPTY_VERTICAL_PADDING,
      left: EMPTY_HORIZONTAL_PADDING,
    })
    .align_x(Horizontal::Center)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

pub(super) fn mount<'a>(base: Element<'a, Message>, state: &'a State) -> Element<'a, Message> {
  // Track the pointer over the whole market view so a card right-click or kebab press can anchor the
  // context menu at the cursor: `mouse_area`'s `on_move` reports coordinates relative to its own
  // bounds, which share the overlay `Stack`'s origin the menu is laid out against.
  let base: Element<'a, Message> = if matches!(state.tab, super::Tab::Watchlist) {
    mouse_area(base).on_move(Message::WatchCursorMoved).into()
  } else {
    base
  };

  let layers = if let Some(menu) = state.watch_menu() {
    vec![
      backdrop::click_catcher(Message::WatchMenuDismissed),
      menu_overlay(menu, &state.tree),
    ]
  } else if let Some(form) = state.watch_modal.as_ref() {
    modal_overlay::modal_layers(Message::WatchModalClosed, card(form, state))
  } else {
    Vec::new()
  };
  modal_overlay::stable_overlay(base, layers)
}

// The card context menu: item-name header, Edit, a divider, and a danger Remove. Edit pre-fills the
// Phase 5 modal via `WatchEdit`; Remove deletes the row via `WatchRemoved`.
fn menu_overlay<'a>(menu: &'a WatchMenu, tree: &'a MarketTree) -> Element<'a, Message> {
  let items = vec![
    Item::action(
      tr("market.watch_menu_edit"),
      Message::WatchEdit(Box::new(menu.watch.clone())),
    ),
    Item::separator(),
    Item::danger(tr("market.watch_menu_remove"), Message::WatchRemoved(menu.watch.id)),
  ];
  context_menu::context_menu(&item_name(tree, menu.watch.type_id), items, menu.anchor)
}

// ── Modal card ────────────────────────────────────────────────

fn card<'a>(form: &'a WatchForm, state: &'a State) -> Element<'a, Message> {
  let body = scrollable(
    Column::with_children(vec![
      field(tr("market.watch.item_label"), item_field(form, state)),
      field(tr("market.watch.region_label"), region_field(form)),
      direction_and_target(form),
      readout(form, state),
    ])
    .spacing(spacing::SPACE_4_5)
    .padding(spacing::SPACE_4_5),
  )
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Shrink);

  let content = Column::with_children(vec![
    header(form),
    container(body).max_height(BODY_MAX_HEIGHT).into(),
    footer(form),
  ])
  .width(Length::Fill);

  container(content)
    .width(Length::Fixed(CARD_WIDTH))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

fn header<'a>(form: &'a WatchForm) -> Element<'a, Message> {
  let (title, subtitle) = match &form.item {
    Some(item) if form.editing.is_some() => (tr("market.watch.edit_title"), item.name.clone()),
    _ => (tr("market.watch.new_title"), tr("market.watch.new_subtitle").to_owned()),
  };

  let titles = Column::with_children(vec![
    text(title.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    eyebrow_text(&subtitle, None).into(),
  ])
  .spacing(spacing::UNIT + 1.0)
  .width(Length::Fill);

  let close: Element<'a, Message> = Button::secondary_icon(Icon::close())
    .on_press(Message::WatchModalClosed)
    .into();

  let row = Row::with_children(vec![titles.into(), close])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Top);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 16.0,
      left: 20.0,
    })
    .style(border_bottom)
    .into()
}

fn field<'a>(label: &str, control: Element<'a, Message>) -> Element<'a, Message> {
  Column::with_children(vec![eyebrow_text(label, None).into(), control])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

// ── Item picker ───────────────────────────────────────────────

fn item_field<'a>(form: &'a WatchForm, state: &'a State) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![item_trigger(form)];
  if form.item_picker_open {
    children.push(item_dropdown(form, state));
  }
  Column::with_children(children).spacing(spacing::SPACE_2).into()
}

fn item_trigger<'a>(form: &'a WatchForm) -> Element<'a, Message> {
  let label: Element<'a, Message> = match &form.item {
    Some(item) => text(item.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    None => text(tr("market.watch.item_placeholder").to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  };

  let caret = Icon::chevron_down()
    .color(color::text::secondary())
    .size(14.0)
    .render::<Message>();

  let row = Row::with_children(vec![container(label).width(Length::Fill).into(), caret])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  button(container(row).center_y(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(FIELD_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: 10.0,
      bottom: 0.0,
      left: 12.0,
    })
    .on_press(Message::WatchItemPickerToggled)
    .style(move |_, status| well_style(form.item_picker_open, status))
    .into()
}

fn item_dropdown<'a>(form: &'a WatchForm, state: &'a State) -> Element<'a, Message> {
  let search = TextInput::new(
    tr("market.watch.item_search_placeholder"),
    &form.item_query,
    Message::WatchItemSearchChanged,
  )
  .height(32.0)
  .render();

  let needle = form.item_query.trim().to_lowercase();
  let selected = form.item.as_ref().map(|item| item.type_id);
  let rows: Vec<Element<'a, Message>> = flat_items(&state.tree)
    .into_iter()
    .filter(|item| matches_query(item, &needle))
    .take(MAX_ITEM_RESULTS)
    .map(|item| item_row(item, selected))
    .collect();

  let list: Element<'a, Message> = if rows.is_empty() {
    container(
      text(t!("market.watch.item_empty").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary())),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_4_5)
    .align_x(Horizontal::Center)
    .into()
  } else {
    scrollable(Column::with_children(rows).spacing(spacing::UNIT).padding(4.0))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Shrink)
      .into()
  };

  let inner = Column::with_children(vec![
    container(search).padding(8.0).style(border_bottom).into(),
    container(list).max_height(ITEM_LIST_HEIGHT).into(),
  ])
  .width(Length::Fill);

  container(inner)
    .width(Length::Fill)
    .clip(true)
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

fn matches_query(item: &FlatItem, needle: &str) -> bool {
  needle.is_empty() || item.name.to_lowercase().contains(needle) || item.group.to_lowercase().contains(needle)
}

fn item_row<'a>(item: FlatItem, selected: Option<i64>) -> Element<'a, Message> {
  let on = selected == Some(item.type_id);
  let name_color = if on { color::accent() } else { color::text::PRIMARY };
  let identity = Column::with_children(vec![
    text(item.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(name_color))
      .into(),
    text(item.group)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  button(identity)
    .width(Length::Fill)
    .padding(Padding {
      top: 8.0,
      right: 10.0,
      bottom: 8.0,
      left: 10.0,
    })
    .on_press(Message::WatchItemPicked(item.type_id, item.name))
    .style(move |_, status| row_style(on, status))
    .into()
}

// ── Region field ──────────────────────────────────────────────

fn region_field<'a>(form: &'a WatchForm) -> Element<'a, Message> {
  let trigger = LocationCombobox::new()
    .placeholder(tr("market.watch.region_placeholder"))
    .selection(form.region.clone())
    .on_toggle(Message::WatchRegionPickerToggled)
    .width(Length::Fill)
    .trigger();

  let mut children: Vec<Element<'a, Message>> = vec![trigger];
  if form.region_picker_open {
    children.push(
      LocationCombobox::new()
        .placeholder(tr("market.watch.region_search_placeholder"))
        .query(form.region_search.query())
        .results(form.region_search.results().to_vec())
        .highlight(form.region_search.highlight())
        .searching(form.region_search.searching())
        .selection(form.region.clone())
        .on_input(Message::WatchRegionSearchChanged)
        .on_pick(Message::WatchRegionPicked)
        .width(Length::Fill)
        .popover(),
    );
  }

  Column::with_children(children).spacing(spacing::SPACE_2).into()
}

// ── Direction toggle + target price ───────────────────────────

fn direction_and_target<'a>(form: &'a WatchForm) -> Element<'a, Message> {
  Row::with_children(vec![
    field(tr("market.watch.alert_label"), direction_toggle(form)),
    field(tr("market.watch.target_label"), target_field(form)),
  ])
  .spacing(spacing::SPACE_4_5)
  .into()
}

fn direction_toggle<'a>(form: &'a WatchForm) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    direction_segment(
      WatchDirection::Buy,
      form.direction,
      tr("market.watch.direction_buy"),
      color::status::DANGER,
    ),
    direction_segment(
      WatchDirection::Sell,
      form.direction,
      tr("market.watch.direction_sell"),
      color::status::ONLINE,
    ),
  ]);

  container(row)
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn direction_segment<'a>(
  direction: WatchDirection,
  active_direction: WatchDirection,
  label: &str,
  accent: iced::Color,
) -> Element<'a, Message> {
  let active = direction == active_direction;
  let fill = if active { accent } else { color::text::secondary() };

  button(
    container(
      text(label.to_owned())
        .font(typography::body::MEDIUM)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(fill)),
    )
    .center_x(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: DIRECTION_PAD_Y,
    right: 0.0,
    bottom: DIRECTION_PAD_Y,
    left: 0.0,
  })
  .on_press(Message::WatchDirectionSelected(direction))
  .style(move |_, _| direction_style(active, accent))
  .into()
}

fn target_field<'a>(form: &'a WatchForm) -> Element<'a, Message> {
  TextInput::new(
    tr("market.watch.target_placeholder"),
    &form.target,
    Message::WatchTargetChanged,
  )
  .height(FIELD_HEIGHT)
  .font_size(typography::size::LG)
  .on_submit(Message::WatchSubmitted)
  .width(Length::Fill)
  .render()
}

fn readout<'a>(form: &'a WatchForm, state: &State) -> Element<'a, Message> {
  let Some(_) = form.item.as_ref() else {
    return Space::new().into();
  };

  let (label_key, price) = match (form.direction, state.book.as_ref()) {
    (WatchDirection::Buy, book) => ("market.watch.current_sell", book.and_then(|book| book.best_sell)),
    (WatchDirection::Sell, book) => ("market.watch.current_buy", book.and_then(|book| book.best_buy)),
  };

  Row::with_children(vec![
    eyebrow_text(&t!(label_key), Some(color::text::tertiary())).into(),
    text(fmt_isk_opt(price))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn footer<'a>(form: &'a WatchForm) -> Element<'a, Message> {
  let save_label = if form.editing.is_some() {
    tr("market.watch.save")
  } else {
    tr("market.watch.add")
  };

  let cancel: Element<'a, Message> = Button::secondary(tr("market.watch.cancel"))
    .on_press(Message::WatchModalClosed)
    .into();
  let save: Element<'a, Message> = Button::primary(save_label)
    .on_press_maybe(form.is_valid().then_some(Message::WatchSubmitted))
    .into();

  let row = Row::with_children(vec![Space::new().width(Length::Fill).into(), cancel, save])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 20.0,
      bottom: 16.0,
      left: 20.0,
    })
    .style(border_top)
    .into()
}

// ── Styles ────────────────────────────────────────────────────

fn well_style(open: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let border_color = if open || hovered {
    color::accent()
  } else {
    color::rule_strong()
  };
  button::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::CARD.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn row_style(selected: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let background = if selected {
    Some(color::with_alpha(color::accent(), 0.12))
  } else if hovered {
    Some(color::with_alpha(color::text::PRIMARY, 0.04))
  } else {
    None
  };
  button::Style {
    background: background.map(Background::Color),
    text_color: color::text::PRIMARY,
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn direction_style(active: bool, accent: iced::Color) -> button::Style {
  button::Style {
    background: active.then(|| Background::Color(color::with_alpha(accent, 0.14))),
    text_color: if active { accent } else { color::text::PRIMARY },
    ..button::Style::default()
  }
}

fn border_bottom(_: &iced::Theme) -> container::Style {
  container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.08),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  }
}

fn border_top(theme: &iced::Theme) -> container::Style {
  border_bottom(theme)
}

fn tr(key: &str) -> &'static str {
  super::i18n::tr_static(key)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn tree() -> MarketTree {
    use crate::store::model::{ItemType, MarketGroup};

    let groups = vec![MarketGroup {
      description: String::new(),
      has_types: false,
      icon_id: None,
      id: 1,
      name: "Frigates".to_owned(),
      parent_id: None,
    }];
    let items = vec![ItemType {
      capacity: None,
      description: None,
      dogma_attributes: "[]".to_owned(),
      group_id: 0,
      icon_id: None,
      id: 587,
      market_group_id: Some(1),
      name: "Rifter".to_owned(),
      packaged_volume: None,
      portion_size: None,
      published: true,
      radius: None,
      volume: None,
    }];
    super::super::tree::build_market_tree(&groups, &items)
  }

  fn watch() -> MarketWatch {
    MarketWatch {
      created_at: "2026-07-13T00:00:00Z".to_owned(),
      direction: "sell".to_owned(),
      id: 42,
      location_id: None,
      location_tier: None,
      region_id: Some(10_000_002),
      target_price: Some(6_500_000.0),
      type_id: 587,
      updated_at: "2026-07-13T00:00:00Z".to_owned(),
    }
  }

  mod form {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_a_blank_new_form() {
      let form = WatchForm::new(None);

      assert!(form.editing.is_none());
      assert!(form.item.is_none());
      assert_eq!(form.direction, WatchDirection::Buy);
      assert!(!form.is_valid());
    }

    #[test]
    fn it_hydrates_an_edit_form_from_a_watch() {
      let form = WatchForm::editing(&watch(), &tree(), "The Forge".to_owned());

      assert_eq!(form.editing, Some(42));
      assert_eq!(form.item.as_ref().map(|item| item.type_id), Some(587));
      assert_eq!(
        form.item.as_ref().map(|item| item.name.clone()),
        Some("Rifter".to_owned())
      );
      assert_eq!(form.direction, WatchDirection::Sell);
      assert_eq!(form.region.as_ref().map(|region| region.id), Some(10_000_002));
      assert_eq!(
        form.region.as_ref().map(|region| region.name.clone()),
        Some("The Forge".to_owned())
      );
      assert_eq!(form.target, "6500000".to_owned());
    }

    #[test]
    fn it_gates_validity_on_an_item_and_a_positive_target() {
      let mut form = WatchForm::new(None);
      form.set_target("120".to_owned());
      assert!(!form.is_valid(), "no item yet");

      form.pick_item(34, "Tritanium".to_owned());
      assert!(form.is_valid());

      form.set_target("0".to_owned());
      assert!(!form.is_valid(), "target must be positive");
    }

    #[test]
    fn it_strips_non_numeric_target_input() {
      let mut form = WatchForm::new(None);
      form.set_target("1,250.50 ISK".to_owned());

      assert_eq!(form.target_value(), Some(1250.50));
    }

    #[test]
    fn it_closes_the_item_picker_after_a_pick() {
      let mut form = WatchForm::new(None);
      form.toggle_item_picker();
      assert!(form.item_picker_open);

      form.pick_item(587, "Rifter".to_owned());

      assert!(!form.item_picker_open);
      assert_eq!(form.item.map(|item| item.type_id), Some(587));
    }
  }

  mod submit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_a_submit_from_a_valid_form() {
      let mut form = WatchForm::new(Some(crate::features::market::region_location(
        10_000_002,
        "The Forge".to_owned(),
      )));
      form.pick_item(587, "Rifter".to_owned());
      form.set_direction(WatchDirection::Sell);
      form.set_target("6500000".to_owned());

      let submit = to_submit(&form).expect("valid form yields a submit");

      assert_eq!(submit.type_id, 587);
      assert_eq!(submit.direction, WatchDirection::Sell);
      assert_eq!(submit.location.as_ref().map(|location| location.id), Some(10_000_002));
      assert_eq!(
        submit.location.as_ref().and_then(|location| location.tier),
        Some(LocationTier::Region)
      );
      assert_eq!(submit.target_price, Some(6_500_000.0));
      assert_eq!(submit.editing, None);
    }

    #[test]
    fn it_refuses_a_submit_without_an_item() {
      let mut form = WatchForm::new(None);
      form.set_target("100".to_owned());

      assert!(to_submit(&form).is_none());
    }
  }

  mod browse {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::market::book::OrderBook;

    fn market(id: i64, tier: LocationTier) -> LocationRef {
      scope_location(id, "Market".to_owned(), tier)
    }

    fn browse_state(place: LocationRef, best_sell: Option<f64>) -> State {
      let mut state = State::new();
      state.selected = Some(587);
      state.active_region = Some(crate::features::market::region_location(
        10_000_002,
        "The Forge".to_owned(),
      ));
      state.active_place = Some(place);
      let mut book = OrderBook::default();
      book.best_sell = best_sell;
      state.book = Some(book);
      state
    }

    fn card(watch: MarketWatch) -> WatchCard {
      WatchCard {
        direction: WatchDirection::Sell,
        location_label: String::new(),
        region_id: watch.region_id,
        region_label: String::new(),
        system_label: String::new(),
        target: watch.target_price,
        type_id: watch.type_id,
        watch,
      }
    }

    #[test]
    fn it_builds_a_sell_watch_at_the_current_market_from_the_best_sell() {
      let state = browse_state(market(10_000_002, LocationTier::Region), Some(6_500_000.0));

      let submit = browse_submit(&state).expect("a selected item at a market yields a submit");

      assert_eq!(submit.type_id, 587);
      assert_eq!(submit.direction, WatchDirection::Sell);
      assert_eq!(submit.target_price, Some(6_500_000.0));
      assert_eq!(submit.location.as_ref().map(|place| place.id), Some(10_000_002));
      assert_eq!(submit.editing, None);
    }

    #[test]
    fn it_omits_the_target_when_the_book_has_no_sell_orders() {
      let state = browse_state(market(10_000_002, LocationTier::Region), None);

      let submit = browse_submit(&state).expect("an empty book still yields a submit");

      assert_eq!(submit.target_price, None);
    }

    #[test]
    fn it_refuses_a_submit_without_a_selection_or_market() {
      let mut state = browse_state(market(10_000_002, LocationTier::Region), Some(1.0));
      state.selected = None;
      assert!(browse_submit(&state).is_none());

      let mut state = browse_state(market(10_000_002, LocationTier::Region), Some(1.0));
      state.active_place = None;
      state.active_region = None;
      assert!(browse_submit(&state).is_none());
    }

    #[test]
    fn it_marks_the_type_watched_immediately_after_the_click() {
      let mut state = browse_state(market(10_000_002, LocationTier::Region), Some(5.5));
      assert!(!is_watched(&state, 587));
      assert!(matches!(
        plan(&state, &Message::BrowseWatchSubmitted),
        Follow::Persist(_)
      ));

      reduce(&mut state, Message::BrowseWatchSubmitted);

      assert!(is_watched(&state, 587));
      assert!(matches!(plan(&state, &Message::BrowseWatchSubmitted), Follow::None));
    }

    #[test]
    fn it_keys_the_watched_state_on_the_picked_market_tier() {
      let structure_id = 1_035_466_617_800;
      let mut state = browse_state(market(structure_id, LocationTier::Structure), Some(5.5));
      reduce(&mut state, Message::BrowseWatchSubmitted);
      assert!(is_watched(&state, 587));

      state.active_place = Some(market(10_000_002, LocationTier::Region));
      assert!(!is_watched(&state, 587));

      state.active_place = Some(market(structure_id, LocationTier::Structure));
      assert!(is_watched(&state, 587));
    }

    #[test]
    fn it_matches_a_region_scoped_watch_without_a_location_id() {
      let cards = vec![card(watch())];

      assert!(watched_at(&cards, 587, 10_000_002));
      assert!(!watched_at(&cards, 587, 10_000_043));
      assert!(!watched_at(&cards, 34, 10_000_002));
    }
  }

  mod compare_watch {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::market::{
      BookAccess,
      book::OrderBook,
      compare::{BlockId, CompareBlock, CompareColumn},
    };

    fn column(place_id: i64, best_sell: Option<f64>) -> CompareColumn {
      CompareColumn {
        access: BookAccess::Ok,
        book: Some(OrderBook {
          best_sell,
          ..OrderBook::default()
        }),
        place: scope_location(place_id, "Market".to_owned(), LocationTier::Station),
        row: None,
      }
    }

    fn pinned_state(columns: Vec<CompareColumn>) -> State {
      let mut state = State::new();
      state.compare_pins = vec![CompareBlock {
        columns,
        id: BlockId::Pin(7),
        type_id: 587,
      }];
      state
    }

    fn block(state: &State) -> &CompareBlock {
      &state.compare_pins[0]
    }

    #[test]
    fn it_builds_a_sell_watch_at_the_blocks_first_market() {
      let state = pinned_state(vec![
        column(60_003_760, Some(6_500_000.0)),
        column(60_008_494, Some(5_000_000.0)),
      ]);

      let submit = compare_submit(&state, BlockId::Pin(7)).expect("a pinned block yields a submit");

      assert_eq!(submit.type_id, 587);
      assert_eq!(submit.direction, WatchDirection::Sell);
      assert_eq!(submit.target_price, Some(6_500_000.0));
      assert_eq!(submit.location.as_ref().map(|place| place.id), Some(60_003_760));
      assert_eq!(submit.editing, None);
    }

    #[test]
    fn it_keys_the_watched_state_on_the_first_column_only() {
      let mut state = pinned_state(vec![
        column(60_003_760, Some(6_500_000.0)),
        column(60_008_494, Some(5_000_000.0)),
      ]);
      let mut source = watch();
      source.location_id = Some(60_008_494);
      state.watches = vec![WatchCard {
        direction: WatchDirection::Sell,
        location_label: String::new(),
        region_id: None,
        region_label: String::new(),
        system_label: String::new(),
        target: source.target_price,
        type_id: source.type_id,
        watch: source,
      }];

      assert!(!is_block_watched(&state, block(&state)));
      assert!(compare_submit(&state, BlockId::Pin(7)).is_some());
    }

    #[test]
    fn it_marks_the_block_watched_immediately_after_the_click() {
      let mut state = pinned_state(vec![column(60_003_760, Some(6_500_000.0))]);
      assert!(!is_block_watched(&state, block(&state)));
      assert!(matches!(
        plan(&state, &Message::CompareWatchSubmitted(BlockId::Pin(7))),
        Follow::Persist(_)
      ));

      reduce(&mut state, Message::CompareWatchSubmitted(BlockId::Pin(7)));

      assert!(is_block_watched(&state, block(&state)));
      assert!(matches!(
        plan(&state, &Message::CompareWatchSubmitted(BlockId::Pin(7))),
        Follow::None
      ));
    }

    #[test]
    fn it_refuses_a_submit_for_an_unknown_block() {
      let state = pinned_state(vec![column(60_003_760, Some(6_500_000.0))]);

      assert!(compare_submit(&state, BlockId::Pin(99)).is_none());
    }
  }

  mod catalog {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flattens_the_catalog_with_group_names() {
      let flat = flat_items(&tree());

      assert_eq!(flat.len(), 1);
      assert_eq!(flat[0].name, "Rifter");
      assert_eq!(flat[0].group, "Frigates");
    }

    #[test]
    fn it_matches_items_by_name_and_group_case_insensitively() {
      let flat = flat_items(&tree());

      assert!(matches_query(&flat[0], "rift"));
      assert!(matches_query(&flat[0], "frigate"));
      assert!(matches_query(&flat[0], ""));
      assert!(!matches_query(&flat[0], "cruiser"));
    }

    #[test]
    fn it_finds_an_item_by_type_id() {
      assert_eq!(find_item(&tree(), 587).map(|item| item.name), Some("Rifter".to_owned()));
      assert!(find_item(&tree(), 999).is_none());
    }
  }

  mod reduce {
    use super::*;

    fn open_state() -> State {
      let mut state = State::new();
      reduce(&mut state, Message::WatchNew);
      state
    }

    #[test]
    fn it_opens_and_closes_the_modal() {
      let mut state = open_state();
      assert!(state.watch_modal.is_some());

      reduce(&mut state, Message::WatchModalClosed);
      assert!(state.watch_modal.is_none());
    }

    #[test]
    fn it_records_an_item_pick_into_the_open_form() {
      let mut state = open_state();

      reduce(&mut state, Message::WatchItemPicked(587, "Rifter".to_owned()));

      let form = state.watch_modal.as_ref().unwrap();
      assert_eq!(form.item.as_ref().map(|item| item.type_id), Some(587));
    }

    #[test]
    fn it_flips_the_direction() {
      let mut state = open_state();

      reduce(&mut state, Message::WatchDirectionSelected(WatchDirection::Sell));

      assert_eq!(state.watch_modal.as_ref().unwrap().direction, WatchDirection::Sell);
    }

    #[test]
    fn it_clears_the_modal_on_submit() {
      let mut state = open_state();

      reduce(&mut state, Message::WatchSubmitted);

      assert!(state.watch_modal.is_none());
    }

    #[test]
    fn it_opens_an_edit_form_from_a_watch_message() {
      let mut state = State::new();
      state.tree = tree();

      reduce(&mut state, Message::WatchEdit(Box::new(watch())));

      let form = state.watch_modal.as_ref().unwrap();
      assert_eq!(form.editing, Some(42));
      assert_eq!(form.item.as_ref().map(|item| item.type_id), Some(587));
    }

    #[test]
    fn it_drives_the_region_picker_through_the_form() {
      let mut state = open_state();

      reduce(&mut state, Message::WatchRegionPickerToggled);
      assert!(state.watch_modal.as_ref().unwrap().region_picker_open);

      reduce(&mut state, Message::WatchRegionSearchChanged("forge".to_owned()));

      let region = LocationRef {
        context: None,
        id: 10_000_002,
        name: "The Forge".to_owned(),
        security_status: None,
        tier: Some(crate::services::location_search::LocationTier::Region),
      };
      reduce(&mut state, Message::WatchRegionResultsLoaded(0, vec![region.clone()]));
      reduce(&mut state, Message::WatchRegionPicked(region));

      let form = state.watch_modal.as_ref().unwrap();
      assert_eq!(form.region.as_ref().map(|location| location.id), Some(10_000_002));
      assert!(!form.region_picker_open);
    }
  }

  mod menu {
    use pretty_assertions::assert_eq;

    use super::*;

    fn seeded() -> State {
      let mut state = State::new();
      state.tree = tree();
      state.watches = vec![WatchCard {
        direction: WatchDirection::Sell,
        location_label: String::new(),
        region_id: Some(10_000_002),
        region_label: "The Forge".to_owned(),
        system_label: String::new(),
        target: Some(6_500_000.0),
        type_id: 587,
        watch: watch(),
      }];
      state
    }

    #[test]
    fn it_opens_a_menu_anchored_at_the_tracked_cursor() {
      let mut state = seeded();
      reduce(&mut state, Message::WatchCursorMoved(Point::new(120.0, 80.0)));

      reduce(&mut state, Message::WatchMenuOpened(42));

      let menu = state.watch_menu().expect("the menu opens for a known watch");
      assert_eq!(menu.watch.id, 42);
      assert_eq!(menu.anchor, Point::new(120.0, 80.0));
    }

    #[test]
    fn it_ignores_a_menu_open_for_an_unknown_watch() {
      let mut state = seeded();

      reduce(&mut state, Message::WatchMenuOpened(999));

      assert!(state.watch_menu().is_none());
    }

    #[test]
    fn it_dismisses_the_menu() {
      let mut state = seeded();
      reduce(&mut state, Message::WatchMenuOpened(42));
      assert!(state.watch_menu().is_some());

      reduce(&mut state, Message::WatchMenuDismissed);

      assert!(state.watch_menu().is_none());
    }

    #[test]
    fn it_clears_the_menu_when_a_watch_is_removed() {
      let mut state = seeded();
      reduce(&mut state, Message::WatchMenuOpened(42));

      reduce(&mut state, Message::WatchRemoved(42));

      assert!(state.watch_menu().is_none());
    }

    #[test]
    fn it_opens_a_prefilled_edit_form_and_closes_the_menu() {
      let mut state = seeded();
      reduce(&mut state, Message::WatchMenuOpened(42));

      reduce(&mut state, Message::WatchEdit(Box::new(watch())));

      assert!(state.watch_menu().is_none(), "opening the editor dismisses the menu");
      let form = state.watch_modal.as_ref().expect("the edit form opens");
      assert_eq!(form.editing, Some(42));
      assert_eq!(form.item.as_ref().map(|item| item.type_id), Some(587));
    }

    #[test]
    fn it_mounts_the_menu_overlay() {
      let mut state = seeded();
      reduce(&mut state, Message::WatchMenuOpened(42));

      let _el: Element<'_, Message> = mount(Space::new().into(), &state);
    }
  }

  mod drag {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card_with_id(id: i64) -> WatchCard {
      let mut source = watch();
      source.id = id;
      WatchCard {
        direction: WatchDirection::Sell,
        location_label: String::new(),
        region_id: Some(10_000_002),
        region_label: "The Forge".to_owned(),
        system_label: String::new(),
        target: Some(6_500_000.0),
        type_id: 587,
        watch: source,
      }
    }

    fn seeded(ids: &[i64]) -> State {
      let mut state = State::new();
      state.watches = ids.iter().copied().map(card_with_id).collect();
      state
    }

    fn ids(state: &State) -> Vec<i64> {
      state.watches.iter().map(|card| card.watch.id).collect()
    }

    #[test]
    fn it_arms_the_drag_from_a_grip_press() {
      let mut state = seeded(&[1, 2, 3]);

      reduce(&mut state, Message::WatchDragStarted(2));

      assert_eq!(state.dragging_watch, Some(2));
      assert_eq!(state.watch_drop_target, None);
    }

    #[test]
    fn it_tracks_a_drop_target_only_while_dragging() {
      let mut state = seeded(&[1, 2]);

      reduce(&mut state, Message::WatchDropEntered(2));
      assert_eq!(state.watch_drop_target, None);

      reduce(&mut state, Message::WatchDragStarted(1));
      reduce(&mut state, Message::WatchDropEntered(2));
      assert_eq!(state.watch_drop_target, Some(2));
    }

    #[test]
    fn it_never_targets_the_dragged_card_itself() {
      let mut state = seeded(&[1, 2]);
      reduce(&mut state, Message::WatchDragStarted(1));

      reduce(&mut state, Message::WatchDropEntered(1));

      assert_eq!(state.watch_drop_target, None);
    }

    #[test]
    fn it_clears_only_a_matching_target_on_exit() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::WatchDragStarted(1));
      reduce(&mut state, Message::WatchDropEntered(2));

      reduce(&mut state, Message::WatchDropExited(3));
      assert_eq!(state.watch_drop_target, Some(2));

      reduce(&mut state, Message::WatchDropExited(2));
      assert_eq!(state.watch_drop_target, None);
    }

    #[test]
    fn it_splices_the_dragged_card_to_the_target_index_on_release() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::WatchDragStarted(1));
      reduce(&mut state, Message::WatchDropEntered(3));

      reduce(&mut state, Message::WatchDropReleased);

      assert_eq!(ids(&state), vec![2, 3, 1]);
      assert_eq!(state.dragging_watch, None);
      assert_eq!(state.watch_drop_target, None);
    }

    #[test]
    fn it_splices_a_later_card_before_an_earlier_target() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::WatchDragStarted(3));
      reduce(&mut state, Message::WatchDropEntered(1));

      reduce(&mut state, Message::WatchDropReleased);

      assert_eq!(ids(&state), vec![3, 1, 2]);
    }

    #[test]
    fn it_keeps_the_order_when_released_without_a_target() {
      let mut state = seeded(&[1, 2, 3]);
      reduce(&mut state, Message::WatchDragStarted(2));

      reduce(&mut state, Message::WatchDropReleased);

      assert_eq!(ids(&state), vec![1, 2, 3]);
      assert_eq!(state.dragging_watch, None);
    }

    #[test]
    fn it_keeps_the_order_when_spliced_onto_itself() {
      let mut cards: Vec<WatchCard> = [1, 2, 3].iter().copied().map(card_with_id).collect();

      splice_watches(&mut cards, 2, 2);

      let order: Vec<i64> = cards.iter().map(|card| card.watch.id).collect();
      assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn it_plans_a_persist_only_for_a_real_drop() {
      let mut state = seeded(&[1, 2]);
      assert!(matches!(plan(&state, &Message::WatchDropReleased), Follow::None));

      reduce(&mut state, Message::WatchDragStarted(1));
      reduce(&mut state, Message::WatchDropEntered(2));

      assert!(matches!(
        plan(&state, &Message::WatchDropReleased),
        Follow::PersistOrder
      ));
    }

    #[test]
    fn it_tracks_grip_hover_per_card() {
      let mut state = seeded(&[1, 2]);

      reduce(&mut state, Message::WatchGripEntered(1));
      assert_eq!(state.watch_grip_hover, Some(1));

      reduce(&mut state, Message::WatchGripExited(2));
      assert_eq!(state.watch_grip_hover, Some(1));

      reduce(&mut state, Message::WatchGripExited(1));
      assert_eq!(state.watch_grip_hover, None);
    }
  }

  mod dispatch {
    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_hands_back_a_non_watch_message() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let outcome = try_dispatch(
        &mut state,
        Message::TabSelected(crate::features::market::Tab::Browse),
        &db,
      );

      assert!(
        outcome.is_err(),
        "browse messages are handed back to the market reducer"
      );
    }

    #[tokio::test]
    async fn it_handles_a_watch_message() {
      let db = store::open_test().await.unwrap();
      let mut state = State::new();

      let outcome = try_dispatch(&mut state, Message::WatchNew, &db);

      assert!(outcome.is_ok());
      assert!(state.watch_modal.is_some());
    }

    #[tokio::test]
    async fn it_persists_a_new_watch() {
      let db = store::open_test().await.unwrap();
      let submit = WatchSubmit {
        direction: WatchDirection::Buy,
        editing: None,
        location: Some(scope_location(10_000_002, "The Forge".to_owned(), LocationTier::Region)),
        target_price: Some(5.0),
        type_id: 34,
      };

      persist(db.clone(), submit).await;

      let rows = market_watchlist::list(&db).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].type_id, 34);
      assert_eq!(rows[0].location_id, Some(10_000_002));
      assert_eq!(rows[0].location_tier, Some("region".to_owned()));
      assert_eq!(rows[0].region_id, Some(10_000_002));
    }

    #[tokio::test]
    async fn it_reloads_the_grid_after_persisting_a_watch() {
      let db = store::open_test().await.unwrap();
      let submit = WatchSubmit {
        direction: WatchDirection::Buy,
        editing: None,
        location: Some(scope_location(10_000_002, "The Forge".to_owned(), LocationTier::Region)),
        target_price: Some(5.0),
        type_id: 34,
      };

      let cards = persist_and_fetch(db.clone(), submit).await;

      assert_eq!(cards.len(), 1);
      assert_eq!(cards[0].watch.type_id, 34);

      let edit = WatchSubmit {
        direction: WatchDirection::Sell,
        editing: Some(cards[0].watch.id),
        location: Some(scope_location(10_000_002, "The Forge".to_owned(), LocationTier::Region)),
        target_price: Some(9.5),
        type_id: 34,
      };

      let cards = persist_and_fetch(db.clone(), edit).await;

      assert_eq!(cards.len(), 1);
      assert_eq!(cards[0].watch.direction, "sell");
      assert_eq!(cards[0].watch.target_price, Some(9.5));
    }

    #[tokio::test]
    async fn it_removes_a_watch_so_the_reloaded_grid_drops_it() {
      let db = store::open_test().await.unwrap();
      let new = NewWatch {
        direction: WatchDirection::Sell,
        location_id: None,
        location_tier: None,
        region_id: Some(10_000_002),
        target_price: Some(100.0),
        type_id: 34,
      };
      let created = market_watchlist::create(&db, &new).await.unwrap();
      assert_eq!(crate::features::market::fetch_watches(db.clone()).await.len(), 1);

      market_watchlist::delete(&db, created.id).await.unwrap();

      assert!(crate::features::market::fetch_watches(db.clone()).await.is_empty());
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_empty_surface() {
      let state = State::new();
      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_mounts_without_a_modal() {
      let state = State::new();
      let _el: Element<'_, Message> = mount(Space::new().into(), &state);
    }

    #[test]
    fn it_renders_the_new_modal_card() {
      let mut state = State::new();
      reduce(&mut state, Message::WatchNew);
      reduce(&mut state, Message::WatchItemPickerToggled);

      let _el: Element<'_, Message> = mount(Space::new().into(), &state);
    }

    #[test]
    fn it_renders_the_readout_once_an_item_is_picked() {
      let mut state = State::new();
      reduce(&mut state, Message::WatchNew);
      reduce(&mut state, Message::WatchItemPicked(587, "Rifter".to_owned()));

      let _el: Element<'_, Message> = mount(Space::new().into(), &state);
    }
  }

  mod grid {
    use pretty_assertions::assert_eq;

    use super::*;

    fn card(direction: WatchDirection, region_id: Option<i64>, target: Option<f64>) -> WatchCard {
      WatchCard {
        direction,
        location_label: String::new(),
        region_id,
        region_label: "The Forge".to_owned(),
        system_label: "Jita".to_owned(),
        target,
        type_id: 34,
        watch: MarketWatch {
          direction: direction.as_str().to_owned(),
          id: 7,
          region_id,
          target_price: target,
          type_id: 34,
          ..MarketWatch::default()
        },
      }
    }

    fn prices() -> watch_eval::PriceMap {
      let mut map = watch_eval::PriceMap::new();
      map.insert(
        (34, 10_000_002),
        watch_eval::BestPrices::available(Some(9.0), Some(8.0)),
      );
      map
    }

    #[test]
    fn it_counts_only_the_met_watches() {
      let cards = vec![
        card(WatchDirection::Buy, Some(10_000_002), Some(10.0)),
        card(WatchDirection::Buy, Some(10_000_002), Some(5.0)),
        card(WatchDirection::Buy, None, Some(10.0)),
      ];

      assert_eq!(count_met(&cards, &prices()), 1);
    }

    #[test]
    fn it_joins_region_and_system_into_a_subtitle() {
      assert_eq!(
        subtitle(&card(WatchDirection::Buy, None, None)),
        "The Forge \u{b7} Jita"
      );

      let mut region_only = card(WatchDirection::Buy, None, None);
      region_only.system_label = String::new();
      assert_eq!(subtitle(&region_only), "The Forge");

      let mut bare = card(WatchDirection::Buy, None, None);
      bare.region_label = String::new();
      bare.system_label = String::new();
      assert_eq!(subtitle(&bare), "");
    }

    #[test]
    fn it_labels_the_distance_above_below_and_awaiting() {
      assert_eq!(
        distance_label(Some(110.0), Some(100.0)),
        t!("market.watchlist_above_target", pct => "10.0".to_owned()).into_owned()
      );
      assert_eq!(
        distance_label(Some(90.0), Some(100.0)),
        t!("market.watchlist_below_target", pct => "10.0".to_owned()).into_owned()
      );
      assert_eq!(
        distance_label(None, Some(100.0)),
        t!("market.watchlist_awaiting_data").into_owned()
      );
    }

    #[test]
    fn it_resolves_an_item_name_from_the_tree() {
      assert_eq!(item_name(&tree(), 587), "Rifter");
      assert_eq!(
        item_name(&tree(), 999),
        t!("market.book_item_fallback", id => 999).into_owned()
      );
    }

    #[test]
    fn it_renders_a_populated_surface_grid() {
      let mut state = State::new();
      state.tree = tree();
      crate::features::market::update(
        &mut state,
        Message::WatchesLoaded(vec![card(WatchDirection::Sell, Some(10_000_002), Some(100.0))]),
      );
      crate::features::market::update(&mut state, Message::WatchPricesLoaded(prices()));

      let _el: Element<'_, Message> = surface(&state);
    }
  }
}
