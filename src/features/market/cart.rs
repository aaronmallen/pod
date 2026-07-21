use std::sync::Arc;

use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Radians, Rectangle, Size as IcedSize, Task,
  advanced::{
    Layout, Widget,
    layout::{self, Limits, Node},
    mouse, renderer,
    widget::Tree,
  },
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, mouse_area, opaque, scrollable, svg, text, text_input},
};

use super::{
  Message, State,
  tree::{MarketNode, MarketTree},
};
use crate::{
  clients::{esi, eve_image::Size as ImageSize, eve_sso},
  features::assets::stockpile_multibuy,
  services::market_prices::{self, BestSellPrices, MarketScope},
  store::{
    Database,
    images::{self, IconResolution},
    model::{MarketCart, MarketCartLine},
    repo::market_cart,
  },
  ui::{
    components::{
      backdrop,
      button::{Button, Size},
      clip::clip_layer,
      icon::Icon,
      icon_tile::icon_tile,
      modal_overlay,
    },
    format::{fmt_count, fmt_isk_full, fmt_isk_opt},
    style::{color, control, spacing, typography},
  },
};

const BADGE_HEIGHT: f32 = 18.0;
const BADGE_RADIUS: f32 = 9.0;
const CART_BUTTON_HEIGHT: f32 = 34.0;
const CART_BUTTON_RADIUS: f32 = 8.0;
const COPIED_FLASH_MS: u64 = 1800;
const DRAWER_WIDTH: f32 = 520.0;
const EMPTY_COPY_WIDTH: f32 = 320.0;
const EMPTY_GLYPH_SIZE: f32 = 44.0;
const LINE_ICON_IMAGE: ImageSize = ImageSize::S64;
const LINE_ICON_TILE: f32 = 30.0;
const LINE_TOTAL_WIDTH: f32 = 92.0;
const REMOVE_BUTTON_SIZE: f32 = 26.0;
const SAVED_ACTION_SIZE: f32 = 28.0;
const SAVED_ACTION_RADIUS: f32 = 6.0;
const STEPPER_HEIGHT: f32 = 26.0;
const STEPPER_INPUT_WIDTH: f32 = 44.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cart {
  copied: bool,
  copied_generation: u64,
  lines: Vec<MarketCartLine>,
  open: bool,
  price_scope: Option<i64>,
  prices: BestSellPrices,
  rename: Option<Rename>,
  save_name: Option<String>,
  saved: Vec<SavedCart>,
  view: View,
}

impl Cart {
  pub fn is_open(&self) -> bool {
    self.open
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedCart {
  pub cart: MarketCart,
  pub lines: Vec<MarketCartLine>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
  pub lines: Vec<MarketCartLine>,
  pub saved: Vec<SavedCart>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum View {
  #[default]
  Current,
  Saved,
}

#[derive(Clone, Debug, PartialEq)]
struct Rename {
  cart_id: i64,
  name: String,
}

#[derive(Debug, PartialEq)]
enum Follow {
  Clear,
  Delete(i64),
  Export { generation: u64, text: String },
  LoadSaved(i64),
  MergeSaved(i64),
  None,
  Refresh,
  RemoveLine(i64),
  Rename { cart_id: i64, name: String },
  Save(String),
  SetQuantity { quantity: i64, type_id: i64 },
}

pub(super) fn try_dispatch(state: &mut State, message: Message, db: &Database) -> Result<Task<Message>, Message> {
  if !is_cart_message(&message) {
    return Err(message);
  }
  let follow = plan(state, &message);
  reduce(state, message);
  Ok(execute(db, follow))
}

fn is_cart_message(message: &Message) -> bool {
  matches!(
    message,
    Message::CartCleared
      | Message::CartClosed
      | Message::CartEscapePressed
      | Message::CartExportFlashEnded(_)
      | Message::CartExported
      | Message::CartLineRemoved(_)
      | Message::CartLoaded(_)
      | Message::CartOpened
      | Message::CartPricesLoaded(..)
      | Message::CartQtyChanged(..)
      | Message::CartSaveCancelled
      | Message::CartSaveCommitted
      | Message::CartSaveNameChanged(_)
      | Message::CartSaveStarted
      | Message::CartSavedCartLoaded(_)
      | Message::CartSavedCartMerged(_)
      | Message::CartSavedDeleted(_)
      | Message::CartSavedRenameChanged(_)
      | Message::CartSavedRenameCommitted
      | Message::CartSavedRenameStarted(_)
      | Message::CartTabSelected(_)
  )
}

fn plan(state: &State, message: &Message) -> Follow {
  match message {
    Message::CartCleared => Follow::Clear,
    Message::CartExported => Follow::Export {
      generation: state.cart.copied_generation + 1,
      text: multibuy_text(state),
    },
    Message::CartLineRemoved(type_id) => Follow::RemoveLine(*type_id),
    Message::CartOpened => Follow::Refresh,
    Message::CartQtyChanged(type_id, quantity) => Follow::SetQuantity {
      quantity: (*quantity).max(1),
      type_id: *type_id,
    },
    Message::CartSaveCommitted => Follow::Save(state.cart.save_name.clone().unwrap_or_default()),
    Message::CartSavedCartLoaded(cart_id) => Follow::LoadSaved(*cart_id),
    Message::CartSavedCartMerged(cart_id) => Follow::MergeSaved(*cart_id),
    Message::CartSavedDeleted(cart_id) => Follow::Delete(*cart_id),
    Message::CartSavedRenameCommitted => rename_follow(state),
    _ => Follow::None,
  }
}

fn rename_follow(state: &State) -> Follow {
  match &state.cart.rename {
    Some(rename) if !rename.name.trim().is_empty() => Follow::Rename {
      cart_id: rename.cart_id,
      name: rename.name.trim().to_owned(),
    },
    _ => Follow::None,
  }
}

pub(super) fn reduce(state: &mut State, message: Message) {
  match message {
    Message::CartCleared => state.cart.lines.clear(),
    Message::CartClosed => close(&mut state.cart),
    Message::CartEscapePressed => escape(&mut state.cart),
    Message::CartExportFlashEnded(generation) if state.cart.copied_generation == generation => {
      state.cart.copied = false;
    }
    Message::CartExported => {
      state.cart.copied = true;
      state.cart.copied_generation += 1;
      state.cart.lines.clear();
    }
    Message::CartLineRemoved(type_id) => state.cart.lines.retain(|line| line.type_id != type_id),
    Message::CartLoaded(snapshot) => {
      state.cart.lines = snapshot.lines;
      state.cart.saved = snapshot.saved;
    }
    Message::CartOpened => state.cart.open = true,
    Message::CartPricesLoaded(scope_id, prices) => apply_prices(state, scope_id, prices),
    Message::CartQtyChanged(type_id, quantity) => set_line_quantity(&mut state.cart, type_id, quantity.max(1)),
    Message::CartSaveCancelled => state.cart.save_name = None,
    Message::CartSaveCommitted => {
      state.cart.save_name = None;
      state.cart.view = View::Saved;
    }
    Message::CartSaveNameChanged(name) => state.cart.save_name = Some(name),
    Message::CartSaveStarted => state.cart.save_name = Some(String::new()),
    Message::CartSavedCartLoaded(_) | Message::CartSavedCartMerged(_) => state.cart.view = View::Current,
    Message::CartSavedDeleted(cart_id) => delete_saved(&mut state.cart, cart_id),
    Message::CartSavedRenameChanged(name) => {
      if let Some(rename) = &mut state.cart.rename {
        rename.name = name;
      }
    }
    Message::CartSavedRenameCommitted => commit_rename(&mut state.cart),
    Message::CartSavedRenameStarted(cart_id) => start_rename(&mut state.cart, cart_id),
    Message::CartTabSelected(view) => {
      state.cart.view = view;
      state.cart.rename = None;
    }
    _ => {}
  }
}

fn apply_prices(state: &mut State, scope_id: i64, prices: BestSellPrices) {
  if state.active_location().map(|location| location.id) != Some(scope_id) {
    return;
  }
  if state.cart.price_scope == Some(scope_id) {
    state.cart.prices.extend(prices);
  } else {
    state.cart.prices = prices;
  }
  state.cart.price_scope = Some(scope_id);
}

fn close(cart: &mut Cart) {
  cart.copied = false;
  cart.open = false;
  cart.rename = None;
  cart.save_name = None;
}

fn commit_rename(cart: &mut Cart) {
  let Some(rename) = cart.rename.take() else {
    return;
  };
  let trimmed = rename.name.trim();
  if trimmed.is_empty() {
    return;
  }
  if let Some(entry) = cart.saved.iter_mut().find(|entry| entry.cart.id == rename.cart_id) {
    entry.cart.name = Some(trimmed.to_owned());
  }
}

fn delete_saved(cart: &mut Cart, cart_id: i64) {
  cart.saved.retain(|entry| entry.cart.id != cart_id);
  if cart.rename.as_ref().is_some_and(|rename| rename.cart_id == cart_id) {
    cart.rename = None;
  }
}

fn escape(cart: &mut Cart) {
  if cart.save_name.is_some() {
    cart.save_name = None;
  } else if cart.rename.is_some() {
    cart.rename = None;
  } else {
    close(cart);
  }
}

fn set_line_quantity(cart: &mut Cart, type_id: i64, quantity: i64) {
  if let Some(line) = cart.lines.iter_mut().find(|line| line.type_id == type_id) {
    line.quantity = quantity;
  }
}

fn start_rename(cart: &mut Cart, cart_id: i64) {
  let Some(entry) = cart.saved.iter().find(|entry| entry.cart.id == cart_id) else {
    return;
  };
  cart.rename = Some(Rename {
    cart_id,
    name: entry.cart.name.clone().unwrap_or_default(),
  });
}

fn execute(db: &Database, follow: Follow) -> Task<Message> {
  match follow {
    Follow::Export {
      generation,
      text,
    } => export_task(db, generation, text),
    Follow::None => Task::none(),
    other => Task::perform(run_follow(db.clone(), other), |snapshot| {
      Message::CartLoaded(Box::new(snapshot))
    }),
  }
}

fn export_task(db: &Database, generation: u64, text: String) -> Task<Message> {
  Task::batch([
    iced::clipboard::write(text),
    Task::perform(run_follow(db.clone(), Follow::Clear), |snapshot| {
      Message::CartLoaded(Box::new(snapshot))
    }),
    Task::perform(flash_delay(), move |()| Message::CartExportFlashEnded(generation)),
  ])
}

async fn flash_delay() {
  tokio::time::sleep(std::time::Duration::from_millis(COPIED_FLASH_MS)).await;
}

async fn run_follow(db: Database, follow: Follow) -> Snapshot {
  apply_follow(&db, follow).await;
  fetch_snapshot(db).await
}

async fn apply_follow(db: &Database, follow: Follow) {
  let _ = match follow {
    Follow::Clear => market_cart::clear_live(db).await.map(|_| 0),
    Follow::Delete(cart_id) => market_cart::delete(db, cart_id).await.map(|_| 0),
    Follow::LoadSaved(cart_id) => market_cart::load_into_live(db, cart_id).await.map(|_| 0),
    Follow::MergeSaved(cart_id) => market_cart::merge_into_live(db, cart_id).await.map(|_| 0),
    Follow::RemoveLine(type_id) => market_cart::remove_line(db, type_id).await.map(|_| 0),
    Follow::Rename {
      cart_id,
      name,
    } => market_cart::rename(db, cart_id, &name).await.map(|_| 0),
    Follow::Save(name) => market_cart::save_from_live(db, Some(&name)).await.map(|_| 0),
    Follow::SetQuantity {
      quantity,
      type_id,
    } => market_cart::set_quantity(db, type_id, quantity).await.map(|_| 0),
    Follow::Export {
      ..
    }
    | Follow::None
    | Follow::Refresh => Ok(0),
  };
}

pub(super) fn load_snapshot_task(db: &Database) -> Task<Message> {
  Task::perform(fetch_snapshot(db.clone()), |snapshot| {
    Message::CartLoaded(Box::new(snapshot))
  })
}

async fn fetch_snapshot(db: Database) -> Snapshot {
  let lines = market_cart::live_lines(&db).await.unwrap_or_default();
  let mut saved = Vec::new();
  for cart in market_cart::list_saved(&db).await.unwrap_or_default() {
    let lines = market_cart::lines(&db, cart.id).await.unwrap_or_default();
    saved.push(SavedCart {
      cart,
      lines,
    });
  }
  Snapshot {
    lines,
    saved,
  }
}

pub(super) fn wants_prices(message: &Message) -> bool {
  matches!(
    message,
    Message::CartLoaded(_)
      | Message::CartOpened
      | Message::DefaultMarketResolved(_)
      | Message::RegionPicked(_)
      | Message::RegionResolved(_)
  )
}

pub(super) fn prices_task(
  state: &State,
  db: &Database,
  esi: Arc<esi::Client>,
  sso: Arc<eve_sso::Client>,
) -> Task<Message> {
  if !state.cart.open {
    return Task::none();
  }
  let Some(location) = state.active_location() else {
    return Task::none();
  };
  let scope_id = location.id;
  let type_ids = unresolved_type_ids(&state.cart, scope_id);
  if type_ids.is_empty() {
    return Task::none();
  }
  let scope = MarketScope::new(scope_id, state.active_region_id());
  Task::perform(
    market_prices::resolve_best_sell(db.clone(), esi, sso, scope, type_ids),
    move |prices| Message::CartPricesLoaded(scope_id, prices),
  )
}

fn all_type_ids(cart: &Cart) -> Vec<i64> {
  let mut ids: Vec<i64> = cart
    .lines
    .iter()
    .map(|line| line.type_id)
    .chain(
      cart
        .saved
        .iter()
        .flat_map(|entry| entry.lines.iter().map(|line| line.type_id)),
    )
    .collect();
  ids.sort_unstable();
  ids.dedup();
  ids
}

fn unresolved_type_ids(cart: &Cart, scope_id: i64) -> Vec<i64> {
  let stale = cart.price_scope != Some(scope_id);
  all_type_ids(cart)
    .into_iter()
    .filter(|type_id| stale || !cart.prices.contains_key(type_id))
    .collect()
}

fn resolved_price(state: &State, type_id: i64) -> Option<f64> {
  let scope_id = state.active_location()?.id;
  if state.cart.price_scope != Some(scope_id) {
    return None;
  }
  state.cart.prices.get(&type_id).copied().flatten()
}

fn priced_sum(state: &State, lines: &[MarketCartLine]) -> Option<f64> {
  let scope_id = state.active_location()?.id;
  if state.cart.price_scope != Some(scope_id) {
    return None;
  }
  let mut total = 0.0;
  for line in lines {
    let price = state.cart.prices.get(&line.type_id)?;
    total += price.unwrap_or(0.0) * line.quantity as f64;
  }
  Some(total)
}

fn units(lines: &[MarketCartLine]) -> i64 {
  lines.iter().map(|line| line.quantity).sum()
}

fn line_identity(tree: &MarketTree, type_id: i64) -> (String, String) {
  tree
    .roots
    .iter()
    .find_map(|root| find_identity(root, type_id))
    .unwrap_or_else(|| (format!("#{type_id}"), String::new()))
}

fn find_identity(node: &MarketNode, type_id: i64) -> Option<(String, String)> {
  if let Some(leaf) = node.items.iter().find(|leaf| leaf.type_id == type_id) {
    return Some((leaf.name.clone(), node.name.clone()));
  }
  node.children.iter().find_map(|child| find_identity(child, type_id))
}

pub(super) fn multibuy_lines(tree: &MarketTree, lines: &[MarketCartLine]) -> Vec<(String, u64)> {
  lines
    .iter()
    .map(|line| (line_identity(tree, line.type_id).0, line.quantity.max(0) as u64))
    .collect()
}

fn multibuy_text(state: &State) -> String {
  stockpile_multibuy::serialize(&multibuy_lines(&state.tree, &state.cart.lines))
}

fn summary_label(lines: usize, units: i64) -> String {
  if lines == 1 {
    t!("market.cart_summary_one", units => fmt_count(units)).into_owned()
  } else {
    t!("market.cart_summary_many", lines => lines, units => fmt_count(units)).into_owned()
  }
}

fn saved_summary_label(lines: usize, units: i64, value: Option<f64>) -> String {
  let value = fmt_isk_opt(value);
  if lines == 1 {
    t!("market.cart_saved_summary_one", units => fmt_count(units), value => value).into_owned()
  } else {
    t!("market.cart_saved_summary_many", lines => lines, units => fmt_count(units), value => value).into_owned()
  }
}

pub(super) fn mount<'a>(base: Element<'a, Message>, state: &'a State) -> Element<'a, Message> {
  let layers = if state.cart.open {
    vec![backdrop::backdrop(Message::CartClosed), drawer_layer(state)]
  } else {
    Vec::new()
  };
  modal_overlay::stable_overlay(base, layers)
}

fn drawer_layer(state: &State) -> Element<'_, Message> {
  container(opaque(drawer(state)))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .into()
}

fn drawer(state: &State) -> Element<'_, Message> {
  let mut children = vec![header(state), view_tabs(state)];
  match state.cart.view {
    View::Current if state.cart.lines.is_empty() => children.push(current_empty(state)),
    View::Current => {
      children.push(line_list(state));
      children.push(footer(state));
    }
    View::Saved if state.cart.saved.is_empty() => children.push(saved_empty()),
    View::Saved => children.push(saved_list(state)),
  }

  let panel = container(Column::with_children(children).width(Length::Fill).height(Length::Fill))
    .width(Length::Fixed(DRAWER_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  Row::with_children(vec![vertical_rule(color::rule_strong()), panel.into()])
    .height(Length::Fill)
    .into()
}

fn header(state: &State) -> Element<'_, Message> {
  let cart = &state.cart;
  let titles = Column::with_children(vec![
    text(t!("market.cart_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(16.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    mono_caps(
      summary_label(cart.lines.len(), units(&cart.lines)),
      10.0,
      color::text::secondary(),
    ),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let mut children: Vec<Element<'_, Message>> =
    vec![Icon::cart().size(20.0).color(color::accent()).render(), titles.into()];
  if cart.view == View::Current && !cart.lines.is_empty() {
    children.push(clear_button());
  }
  children.push(
    Button::secondary_icon(Icon::close())
      .size(Size::Sm)
      .on_press(Message::CartClosed)
      .into(),
  );

  container(Row::with_children(children).spacing(13.0).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 14.0,
      left: 20.0,
    })
    .style(sunken_band)
    .into()
}

fn clear_button<'a>() -> Element<'a, Message> {
  button(
    text(t!("market.cart_clear").to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::UNIT + 2.0,
    bottom: spacing::UNIT,
    left: spacing::UNIT + 2.0,
  })
  .on_press(Message::CartCleared)
  .style(|_, status| button::Style {
    background: None,
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::status::DANGER,
      _ => color::text::tertiary(),
    },
    ..button::Style::default()
  })
  .into()
}

fn view_tabs(state: &State) -> Element<'_, Message> {
  let tabs = Row::with_children(vec![
    drawer_tab(
      t!("market.cart_tab_current").into_owned(),
      state.cart.lines.len(),
      state.cart.view == View::Current,
      Message::CartTabSelected(View::Current),
    ),
    drawer_tab(
      t!("market.cart_tab_saved").into_owned(),
      state.cart.saved.len(),
      state.cart.view == View::Saved,
      Message::CartTabSelected(View::Saved),
    ),
  ])
  .spacing(spacing::UNIT);

  let band = container(tabs).width(Length::Fill).padding(Padding {
    top: 0.0,
    right: 16.0,
    bottom: 0.0,
    left: 16.0,
  });

  Column::with_children(vec![
    container(band).style(sunken_band).into(),
    horizontal_rule(color::rule()),
  ])
  .width(Length::Fill)
  .into()
}

fn drawer_tab<'a>(label: String, count: usize, selected: bool, on_press: Message) -> Element<'a, Message> {
  let count_color = if selected {
    color::accent()
  } else {
    color::text::tertiary()
  };
  let mut labels: Vec<Element<'a, Message>> = vec![text(label).font(typography::body::MEDIUM).size(12.5).into()];
  if count > 0 {
    labels.push(
      text(count.to_string())
        .font(typography::mono::REGULAR)
        .size(9.5)
        .style(typography::colored(count_color))
        .into(),
    );
  }

  let content = Column::with_children(vec![
    container(Row::with_children(labels).spacing(7.0).align_y(Vertical::Center))
      .padding(Padding {
        top: 8.0,
        right: 14.0,
        bottom: 8.0,
        left: 14.0,
      })
      .into(),
    container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
      .width(Length::Fill)
      .style(move |_| container::Style {
        background: selected.then(|| Background::Color(color::accent())),
        ..container::Style::default()
      })
      .into(),
  ]);

  button(content)
    .padding(Padding::ZERO)
    .on_press(on_press)
    .style(move |_, status| button::Style {
      background: None,
      text_color: match status {
        _ if selected => color::text::PRIMARY,
        button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
        _ => color::text::secondary(),
      },
      ..button::Style::default()
    })
    .into()
}

fn line_list(state: &State) -> Element<'_, Message> {
  let store = images::default_store();
  let count = state.cart.lines.len();
  let mut rows: Vec<Element<'_, Message>> = Vec::new();
  for (index, line) in state.cart.lines.iter().enumerate() {
    rows.push(line_row(state, &store, line));
    if index + 1 < count {
      rows.push(horizontal_rule(color::rule()));
    }
  }
  scroll_body(rows)
}

fn line_row<'a>(state: &'a State, store: &images::Store, line: &MarketCartLine) -> Element<'a, Message> {
  let (name, group) = line_identity(&state.tree, line.type_id);
  let unit_price = resolved_price(state, line.type_id);
  let line_total = unit_price.map(|price| price * line.quantity as f64);
  let sub = t!("market.cart_line_sub", group => group, price => fmt_isk_opt(unit_price)).into_owned();

  let identity = Column::with_children(vec![
    text(name)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(sub)
      .font(typography::mono::REGULAR)
      .size(9.5)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    line_tile(store, line.type_id),
    clip_layer(identity, Length::Fill, Length::Shrink),
    stepper(line.type_id, line.quantity),
    container(
      text(fmt_isk_opt(line_total))
        .font(typography::mono::REGULAR)
        .size(12.5)
        .style(typography::colored(color::text::PRIMARY)),
    )
    .width(Length::Fixed(LINE_TOTAL_WIDTH))
    .align_x(Horizontal::Right)
    .into(),
    glyph_frame_button(
      Icon::close(),
      13.0,
      REMOVE_BUTTON_SIZE,
      FrameHover::Danger,
      false,
      Message::CartLineRemoved(line.type_id),
    ),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 10.0,
      right: 16.0,
      bottom: 10.0,
      left: 16.0,
    })
    .into()
}

fn line_tile<'a>(store: &images::Store, type_id: i64) -> Element<'a, Message> {
  let content: Element<'a, Message> = match store.resolve_type_icon(type_id, None, LINE_ICON_IMAGE) {
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
  icon_tile(content, LINE_ICON_TILE)
}

fn stepper<'a>(type_id: i64, quantity: i64) -> Element<'a, Message> {
  let value = quantity.to_string();
  let input = text_input("", &value)
    .on_input(move |raw| Message::CartQtyChanged(type_id, parse_quantity(&raw)))
    .font(typography::mono::REGULAR)
    .size(12.0)
    .align_x(Horizontal::Center)
    .padding(Padding::ZERO)
    .width(Length::Fixed(STEPPER_INPUT_WIDTH))
    .style(|_, _| text_input::Style {
      background: Background::Color(iced::Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::secondary(),
      placeholder: color::text::tertiary(),
      selection: color::accent_muted(),
      value: color::text::PRIMARY,
    });

  let row = Row::with_children(vec![
    stepper_button("\u{2212}", Message::CartQtyChanged(type_id, (quantity - 1).max(1))),
    vertical_rule(color::rule()),
    container(input).height(Length::Fill).align_y(Vertical::Center).into(),
    vertical_rule(color::rule()),
    stepper_button("+", Message::CartQtyChanged(type_id, quantity + 1)),
  ])
  .align_y(Vertical::Center)
  .height(Length::Fixed(STEPPER_HEIGHT));

  container(row)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 7.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn parse_quantity(raw: &str) -> i64 {
  let digits: String = raw.chars().filter(char::is_ascii_digit).take(9).collect();
  digits.parse::<i64>().map_or(1, |quantity| quantity.max(1))
}

fn stepper_button<'a>(label: &'a str, on_press: Message) -> Element<'a, Message> {
  button(
    container(text(label).font(typography::mono::REGULAR).size(14.0))
      .center_x(Length::Fill)
      .center_y(Length::Fill),
  )
  .width(Length::Fixed(STEPPER_HEIGHT))
  .height(Length::Fill)
  .padding(Padding::ZERO)
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: None,
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::secondary(),
    },
    ..button::Style::default()
  })
  .into()
}

fn current_empty(state: &State) -> Element<'_, Message> {
  let mut children = empty_stack("market.cart_empty_title", "market.cart_empty_body");
  if !state.cart.saved.is_empty() {
    children.push(
      Button::secondary(t!("market.cart_empty_open_saved").into_owned())
        .size(Size::Sm)
        .on_press(Message::CartTabSelected(View::Saved))
        .into(),
    );
  }
  empty_body(children)
}

fn saved_empty<'a>() -> Element<'a, Message> {
  empty_body(empty_stack(
    "market.cart_saved_empty_title",
    "market.cart_saved_empty_body",
  ))
}

fn empty_stack<'a>(title_key: &str, body_key: &str) -> Vec<Element<'a, Message>> {
  vec![
    Icon::cart()
      .size(EMPTY_GLYPH_SIZE)
      .color(color::with_alpha(color::text::PRIMARY, 0.16))
      .render(),
    text(t!(title_key).into_owned())
      .font(typography::body::REGULAR)
      .size(15.0)
      .style(typography::colored(color::with_alpha(color::text::PRIMARY, 0.6)))
      .into(),
    container(
      text(t!(body_key).into_owned())
        .font(typography::mono::REGULAR)
        .size(10.5)
        .wrapping(text::Wrapping::Word)
        .align_x(Horizontal::Center)
        .style(typography::colored(color::text::tertiary())),
    )
    .max_width(EMPTY_COPY_WIDTH)
    .into(),
  ]
}

fn empty_body<'a>(children: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  container(
    Column::with_children(children)
      .spacing(spacing::SPACE_3)
      .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(40.0)
  .into()
}

fn footer(state: &State) -> Element<'_, Message> {
  let mut children = vec![horizontal_rule(color::rule())];
  if let Some(name) = &state.cart.save_name {
    children.push(save_row(name));
    children.push(horizontal_rule(color::rule()));
  }
  children.push(total_row(state));
  children.push(action_row(state));

  container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(sunken_band)
    .into()
}

fn save_row<'a>(name: &'a str) -> Element<'a, Message> {
  let placeholder = super::i18n::tr_static("market.cart_save_placeholder");
  let input = text_input(placeholder, name)
    .on_input(Message::CartSaveNameChanged)
    .on_submit(Message::CartSaveCommitted)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 8.0,
      right: 10.0,
      bottom: 8.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .style(|_, _| text_input::Style {
      background: Background::Color(color::surface::BASE),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: 7.0.into(),
      },
      icon: color::text::secondary(),
      placeholder: color::text::tertiary(),
      selection: color::accent_muted(),
      value: color::text::PRIMARY,
    });

  container(
    Row::with_children(vec![
      input.into(),
      Button::primary(t!("market.cart_save_commit").into_owned())
        .size(Size::Sm)
        .icon(Icon::check())
        .on_press(Message::CartSaveCommitted)
        .into(),
      Button::secondary(t!("market.cart_save_cancel").into_owned())
        .size(Size::Sm)
        .on_press(Message::CartSaveCancelled)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 12.0,
    right: 20.0,
    bottom: 12.0,
    left: 20.0,
  })
  .into()
}

fn total_row(state: &State) -> Element<'_, Message> {
  let market = state
    .active_location()
    .map(|location| location.name.clone())
    .unwrap_or_else(|| t!("market.region_fallback_name").into_owned());
  let total = priced_sum(state, &state.cart.lines)
    .map(fmt_isk_full)
    .unwrap_or_else(|| fmt_isk_opt(None));

  let amount = Row::with_children(vec![
    text(total)
      .font(typography::mono::REGULAR)
      .size(17.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("market.cart_isk").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Bottom);

  container(
    Row::with_children(vec![
      mono_caps(
        t!("market.cart_total_label", market => market).into_owned(),
        9.5,
        color::text::tertiary(),
      ),
      Space::new().width(Length::Fill).into(),
      amount.into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 14.0,
    right: 20.0,
    bottom: 10.0,
    left: 20.0,
  })
  .into()
}

fn action_row(state: &State) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = Vec::new();
  if state.cart.save_name.is_none() {
    children.push(
      Button::secondary(t!("market.cart_save").into_owned())
        .on_press(Message::CartSaveStarted)
        .into(),
    );
  }
  children.push(Space::new().width(Length::Fill).into());
  children.push(export_button(state));

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::UNIT,
    right: 20.0,
    bottom: 18.0,
    left: 20.0,
  })
  .into()
}

fn export_button(state: &State) -> Element<'_, Message> {
  if !state.cart.copied {
    return Button::primary(t!("market.cart_export").into_owned())
      .icon(Icon::copy())
      .on_press(Message::CartExported)
      .into();
  }
  button(
    Row::with_children(vec![
      TintedGlyph::new(Icon::check().handle(), 15.0).into(),
      text(t!("market.cart_export_copied").into_owned())
        .font(typography::body::MEDIUM)
        .size(13.5)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .height(Length::Fixed(38.0))
  .padding(Padding {
    top: 0.0,
    right: 18.0,
    bottom: 0.0,
    left: 18.0,
  })
  .on_press(Message::CartExported)
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::with_alpha(color::status::ONLINE, 0.14))),
    border: Border {
      color: color::with_alpha(color::status::ONLINE, 0.5),
      width: 1.0,
      radius: 10.0.into(),
    },
    text_color: color::status::ONLINE,
    ..button::Style::default()
  })
  .into()
}

fn saved_list(state: &State) -> Element<'_, Message> {
  let count = state.cart.saved.len();
  let mut rows: Vec<Element<'_, Message>> = Vec::new();
  for (index, entry) in state.cart.saved.iter().enumerate() {
    rows.push(saved_row(state, entry));
    if index + 1 < count {
      rows.push(horizontal_rule(color::rule()));
    }
  }
  scroll_body(rows)
}

fn saved_row<'a>(state: &'a State, entry: &'a SavedCart) -> Element<'a, Message> {
  let cart_id = entry.cart.id;
  let summary = saved_summary_label(entry.lines.len(), units(&entry.lines), priced_sum(state, &entry.lines));

  let identity = Column::with_children(vec![
    saved_name(state, entry),
    text(summary)
      .font(typography::mono::REGULAR)
      .size(9.5)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let title_row = Row::with_children(vec![
    identity.into(),
    glyph_frame_button(
      Icon::pencil(),
      13.0,
      SAVED_ACTION_SIZE,
      FrameHover::Neutral,
      true,
      Message::CartSavedRenameStarted(cart_id),
    ),
    glyph_frame_button(
      Icon::close(),
      13.0,
      SAVED_ACTION_SIZE,
      FrameHover::Danger,
      true,
      Message::CartSavedDeleted(cart_id),
    ),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let actions = Row::with_children(vec![
    Button::primary(t!("market.cart_load").into_owned())
      .size(Size::Sm)
      .block()
      .on_press(Message::CartSavedCartLoaded(cart_id))
      .into(),
    Button::secondary(t!("market.cart_merge").into_owned())
      .size(Size::Sm)
      .icon(Icon::plus())
      .on_press(Message::CartSavedCartMerged(cart_id))
      .into(),
  ])
  .spacing(spacing::SPACE_2);

  container(Column::with_children(vec![title_row.into(), actions.into()]).spacing(spacing::SPACE_2_5))
    .width(Length::Fill)
    .padding(Padding {
      top: 12.0,
      right: 16.0,
      bottom: 12.0,
      left: 16.0,
    })
    .into()
}

fn saved_name<'a>(state: &'a State, entry: &'a SavedCart) -> Element<'a, Message> {
  let cart_id = entry.cart.id;
  let renaming = state.cart.rename.as_ref().filter(|rename| rename.cart_id == cart_id);
  if let Some(rename) = renaming {
    return text_input("", &rename.name)
      .on_input(Message::CartSavedRenameChanged)
      .on_submit(Message::CartSavedRenameCommitted)
      .font(typography::body::REGULAR)
      .size(13.5)
      .padding(Padding {
        top: 4.0,
        right: 8.0,
        bottom: 4.0,
        left: 8.0,
      })
      .width(Length::Fill)
      .style(|_, _| text_input::Style {
        background: Background::Color(color::surface::BASE),
        border: Border {
          color: color::accent(),
          width: 1.0,
          radius: SAVED_ACTION_RADIUS.into(),
        },
        icon: color::text::secondary(),
        placeholder: color::text::tertiary(),
        selection: color::accent_muted(),
        value: color::text::PRIMARY,
      })
      .into();
  }

  let label = entry.cart.name.clone().unwrap_or_default();
  mouse_area(
    text(label)
      .font(typography::body::REGULAR)
      .size(13.5)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .on_double_click(Message::CartSavedRenameStarted(cart_id))
  .into()
}

pub(super) fn tab_button(state: &State) -> Element<'_, Message> {
  let count = state.cart.lines.len();
  let mut children: Vec<Element<'_, Message>> = vec![
    TintedGlyph::new(Icon::cart().handle(), 16.0).into(),
    text(t!("market.cart_button").into_owned())
      .font(typography::body::MEDIUM)
      .size(12.5)
      .into(),
  ];
  if count > 0 {
    children.push(badge(count));
  }

  button(
    Row::with_children(children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .height(Length::Fixed(CART_BUTTON_HEIGHT))
  .padding(Padding {
    top: 0.0,
    right: 13.0,
    bottom: 0.0,
    left: 13.0,
  })
  .on_press(Message::CartOpened)
  .style(move |_, status| cart_button_style(count > 0, status))
  .into()
}

fn cart_button_style(filled: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  if filled {
    return button::Style {
      background: Some(Background::Color(color::with_alpha(color::accent(), 0.12))),
      border: Border {
        color: color::with_alpha(color::accent(), 0.4),
        width: 1.0,
        radius: CART_BUTTON_RADIUS.into(),
      },
      text_color: color::accent(),
      ..button::Style::default()
    };
  }
  button::Style {
    background: None,
    border: Border {
      color: if hovered { color::rule_strong() } else { color::rule() },
      width: 1.0,
      radius: CART_BUTTON_RADIUS.into(),
    },
    text_color: if hovered {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    },
    ..button::Style::default()
  }
}

fn badge<'a>(count: usize) -> Element<'a, Message> {
  container(
    text(count.to_string())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::accent_ink())),
  )
  .height(Length::Fixed(BADGE_HEIGHT))
  .padding(Padding {
    top: 0.0,
    right: 5.0,
    bottom: 0.0,
    left: 5.0,
  })
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::accent())),
    border: Border {
      radius: BADGE_RADIUS.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

enum FrameHover {
  Danger,
  Neutral,
}

fn glyph_frame_button<'a>(
  icon: Icon,
  glyph_size: f32,
  frame_size: f32,
  hover: FrameHover,
  bordered: bool,
  on_press: Message,
) -> Element<'a, Message> {
  button(
    container(TintedGlyph::new(icon.handle(), glyph_size))
      .center_x(Length::Fill)
      .center_y(Length::Fill),
  )
  .width(Length::Fixed(frame_size))
  .height(Length::Fixed(frame_size))
  .padding(Padding::ZERO)
  .on_press(on_press)
  .style(move |_, status| frame_style(&hover, bordered, status))
  .into()
}

fn frame_style(hover: &FrameHover, bordered: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let (background, border_color, text_color) = match (hover, hovered) {
    (FrameHover::Danger, true) => (
      Some(color::with_alpha(color::status::DANGER, 0.12)),
      color::with_alpha(color::status::DANGER, 0.4),
      color::status::DANGER,
    ),
    (FrameHover::Danger, false) => (None, color::rule(), color::text::tertiary()),
    (FrameHover::Neutral, true) => (None, color::rule_strong(), color::text::PRIMARY),
    (FrameHover::Neutral, false) => (None, color::rule(), color::text::secondary()),
  };
  button::Style {
    background: background.map(Background::Color),
    border: Border {
      color: if bordered {
        border_color
      } else {
        iced::Color::TRANSPARENT
      },
      width: 1.0,
      radius: SAVED_ACTION_RADIUS.into(),
    },
    text_color,
    ..button::Style::default()
  }
}

fn scroll_body<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  container(
    scrollable(Column::with_children(rows).width(Length::Fill))
      .style(control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn mono_caps<'a>(label: String, size: f32, fill: iced::Color) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(size)
    .style(typography::colored(fill))
    .into()
}

fn sunken_band(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  }
}

fn horizontal_rule<'a>(fill: iced::Color) -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    })
    .into()
}

fn vertical_rule<'a>(fill: iced::Color) -> Element<'a, Message> {
  container(Space::new().width(Length::Fixed(1.0)).height(Length::Fill))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    })
    .into()
}

struct TintedGlyph {
  handle: svg::Handle,
  size: f32,
}

impl TintedGlyph {
  fn new(handle: svg::Handle, size: f32) -> Self {
    Self {
      handle,
      size,
    }
  }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TintedGlyph
where
  Renderer: iced::advanced::svg::Renderer,
{
  fn size(&self) -> IcedSize<Length> {
    IcedSize::new(Length::Fixed(self.size), Length::Fixed(self.size))
  }

  fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
    layout::atomic(limits, Length::Fixed(self.size), Length::Fixed(self.size))
  }

  fn draw(
    &self,
    _tree: &Tree,
    renderer: &mut Renderer,
    _theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    _cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    renderer.draw_svg(
      iced::advanced::svg::Svg {
        color: Some(style.text_color),
        handle: self.handle.clone(),
        opacity: 1.0,
        rotation: Radians(0.0),
      },
      layout.bounds(),
      *viewport,
    );
  }
}

impl<'a, Message, Theme, Renderer> From<TintedGlyph> for Element<'a, Message, Theme, Renderer>
where
  Message: 'a,
  Theme: 'a,
  Renderer: iced::advanced::svg::Renderer + 'a,
{
  fn from(glyph: TintedGlyph) -> Self {
    Element::new(glyph)
  }
}

#[cfg(test)]
mod tests {
  use super::{super::tree::MarketLeaf, *};
  use crate::services::location_search::{LocationRef, LocationTier};

  fn leaf(type_id: i64, name: &str) -> MarketLeaf {
    MarketLeaf {
      best_sell: None,
      name: name.to_owned(),
      type_id,
    }
  }

  fn tree() -> MarketTree {
    MarketTree {
      roots: vec![MarketNode {
        children: vec![MarketNode {
          children: vec![],
          id: 11,
          item_count: 2,
          items: vec![leaf(34, "Tritanium"), leaf(35, "Pyerite")],
          name: "Minerals".to_owned(),
        }],
        id: 10,
        item_count: 2,
        items: vec![],
        name: "Materials".to_owned(),
      }],
    }
  }

  fn line(type_id: i64, quantity: i64) -> MarketCartLine {
    MarketCartLine {
      cart_id: 1,
      id: type_id,
      position: type_id,
      quantity,
      type_id,
    }
  }

  fn saved_entry(cart_id: i64, name: &str, lines: Vec<MarketCartLine>) -> SavedCart {
    SavedCart {
      cart: MarketCart {
        created_at: String::new(),
        id: cart_id,
        is_live: false,
        name: Some(name.to_owned()),
        updated_at: String::new(),
      },
      lines,
    }
  }

  fn region(id: i64) -> LocationRef {
    LocationRef {
      context: None,
      id,
      name: "The Forge".to_owned(),
      security_status: None,
      tier: Some(LocationTier::Region),
    }
  }

  fn state_with_lines(lines: Vec<MarketCartLine>) -> State {
    let mut state = State::new();
    state.tree = tree();
    state.cart.lines = lines;
    state
  }

  mod multibuy {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_serializes_cart_lines_as_name_tab_qty() {
      let lines = vec![line(34, 1000), line(35, 3)];

      let text = stockpile_multibuy::serialize(&multibuy_lines(&tree(), &lines));

      assert_eq!(text, "Tritanium\t1000\nPyerite\t3");
    }

    #[test]
    fn it_writes_quantities_without_thousands_separators() {
      let lines = vec![line(34, 1_234_567)];

      let text = stockpile_multibuy::serialize(&multibuy_lines(&tree(), &lines));

      assert_eq!(text, "Tritanium\t1234567");
    }

    #[test]
    fn it_falls_back_to_a_hash_id_for_an_unknown_type() {
      let lines = vec![line(99, 2)];

      assert_eq!(multibuy_lines(&tree(), &lines), vec![("#99".to_owned(), 2)]);
    }
  }

  mod plan {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_a_quantity_change_to_a_floor_of_one() {
      let state = state_with_lines(vec![line(34, 5)]);

      assert_eq!(
        plan(&state, &Message::CartQtyChanged(34, 0)),
        Follow::SetQuantity {
          quantity: 1,
          type_id: 34,
        }
      );
    }

    #[test]
    fn it_saves_with_a_blank_name_when_no_input_was_typed() {
      let state = state_with_lines(vec![line(34, 5)]);

      assert_eq!(plan(&state, &Message::CartSaveCommitted), Follow::Save(String::new()));
    }

    #[test]
    fn it_skips_a_blank_rename_commit() {
      let mut state = State::new();
      state.cart.saved = vec![saved_entry(7, "Restock", vec![line(34, 5)])];
      state.cart.rename = Some(Rename {
        cart_id: 7,
        name: "   ".to_owned(),
      });

      assert_eq!(plan(&state, &Message::CartSavedRenameCommitted), Follow::None);
    }

    #[test]
    fn it_builds_the_export_text_and_bumps_the_flash_generation() {
      let state = state_with_lines(vec![line(34, 5)]);

      assert_eq!(
        plan(&state, &Message::CartExported),
        Follow::Export {
          generation: 1,
          text: "Tritanium\t5".to_owned(),
        }
      );
    }
  }

  mod reduce {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_tracks_the_distinct_line_count_for_the_badge() {
      let mut state = State::new();

      reduce(
        &mut state,
        Message::CartLoaded(Box::new(Snapshot {
          lines: vec![line(34, 500), line(35, 3)],
          saved: vec![],
        })),
      );

      assert_eq!(state.cart.lines.len(), 2);
    }

    #[test]
    fn it_opens_and_closes_the_drawer() {
      let mut state = State::new();

      reduce(&mut state, Message::CartOpened);
      assert!(state.cart.is_open());

      reduce(&mut state, Message::CartClosed);
      assert!(!state.cart.is_open());
    }

    #[test]
    fn it_cancels_the_save_input_before_closing_on_escape() {
      let mut state = state_with_lines(vec![line(34, 5)]);
      reduce(&mut state, Message::CartOpened);
      reduce(&mut state, Message::CartSaveStarted);

      reduce(&mut state, Message::CartEscapePressed);

      assert_eq!(state.cart.save_name, None);
      assert!(state.cart.is_open());

      reduce(&mut state, Message::CartEscapePressed);

      assert!(!state.cart.is_open());
    }

    #[test]
    fn it_cancels_a_rename_before_closing_on_escape() {
      let mut state = State::new();
      state.cart.saved = vec![saved_entry(7, "Restock", vec![line(34, 5)])];
      reduce(&mut state, Message::CartOpened);
      reduce(&mut state, Message::CartSavedRenameStarted(7));

      reduce(&mut state, Message::CartEscapePressed);

      assert_eq!(state.cart.rename, None);
      assert!(state.cart.is_open());
    }

    #[test]
    fn it_commits_a_save_by_clearing_the_input_and_switching_to_saved() {
      let mut state = state_with_lines(vec![line(34, 5)]);
      reduce(&mut state, Message::CartSaveStarted);
      reduce(&mut state, Message::CartSaveNameChanged("Doctrine".to_owned()));

      reduce(&mut state, Message::CartSaveCommitted);

      assert_eq!(state.cart.save_name, None);
      assert_eq!(state.cart.view, View::Saved);
    }

    #[test]
    fn it_switches_to_current_on_load_and_merge() {
      let mut state = State::new();
      state.cart.view = View::Saved;

      reduce(&mut state, Message::CartSavedCartLoaded(7));
      assert_eq!(state.cart.view, View::Current);

      state.cart.view = View::Saved;
      reduce(&mut state, Message::CartSavedCartMerged(7));
      assert_eq!(state.cart.view, View::Current);
    }

    #[test]
    fn it_prefills_and_applies_a_rename() {
      let mut state = State::new();
      state.cart.saved = vec![saved_entry(7, "Restock", vec![line(34, 5)])];

      reduce(&mut state, Message::CartSavedRenameStarted(7));
      assert_eq!(
        state.cart.rename,
        Some(Rename {
          cart_id: 7,
          name: "Restock".to_owned(),
        })
      );

      reduce(&mut state, Message::CartSavedRenameChanged("Doctrine".to_owned()));
      reduce(&mut state, Message::CartSavedRenameCommitted);

      assert_eq!(state.cart.rename, None);
      assert_eq!(state.cart.saved[0].cart.name.as_deref(), Some("Doctrine"));
    }

    #[test]
    fn it_keeps_the_old_name_on_a_blank_rename_commit() {
      let mut state = State::new();
      state.cart.saved = vec![saved_entry(7, "Restock", vec![line(34, 5)])];
      reduce(&mut state, Message::CartSavedRenameStarted(7));
      reduce(&mut state, Message::CartSavedRenameChanged("   ".to_owned()));

      reduce(&mut state, Message::CartSavedRenameCommitted);

      assert_eq!(state.cart.saved[0].cart.name.as_deref(), Some("Restock"));
    }

    #[test]
    fn it_clears_the_live_cart_and_flashes_on_export() {
      let mut state = state_with_lines(vec![line(34, 5)]);

      reduce(&mut state, Message::CartExported);

      assert!(state.cart.copied);
      assert!(state.cart.lines.is_empty());
      assert_eq!(state.cart.copied_generation, 1);
    }

    #[test]
    fn it_ends_the_copied_flash_only_for_the_matching_generation() {
      let mut state = state_with_lines(vec![line(34, 5)]);
      reduce(&mut state, Message::CartExported);

      reduce(&mut state, Message::CartExportFlashEnded(0));
      assert!(state.cart.copied);

      reduce(&mut state, Message::CartExportFlashEnded(1));
      assert!(!state.cart.copied);
    }

    #[test]
    fn it_applies_prices_for_the_active_scope() {
      let mut state = state_with_lines(vec![line(34, 5)]);
      state.active_place = Some(region(10_000_002));
      let prices = BestSellPrices::from([(34, Some(6.5))]);

      reduce(&mut state, Message::CartPricesLoaded(10_000_002, prices));

      assert_eq!(state.cart.price_scope, Some(10_000_002));
      assert_eq!(state.cart.prices.get(&34), Some(&Some(6.5)));
    }

    #[test]
    fn it_ignores_prices_for_a_stale_scope() {
      let mut state = state_with_lines(vec![line(34, 5)]);
      state.active_place = Some(region(10_000_002));
      let prices = BestSellPrices::from([(34, Some(6.5))]);

      reduce(&mut state, Message::CartPricesLoaded(10_000_043, prices));

      assert_eq!(state.cart.price_scope, None);
      assert!(state.cart.prices.is_empty());
    }
  }

  mod prices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_requests_every_id_when_the_scope_is_stale() {
      let cart = Cart {
        lines: vec![line(34, 5)],
        price_scope: Some(10_000_043),
        prices: BestSellPrices::from([(34, Some(6.5))]),
        ..Cart::default()
      };

      assert_eq!(unresolved_type_ids(&cart, 10_000_002), vec![34]);
    }

    #[test]
    fn it_requests_only_missing_ids_for_the_current_scope() {
      let cart = Cart {
        lines: vec![line(34, 5), line(35, 2)],
        price_scope: Some(10_000_002),
        prices: BestSellPrices::from([(34, Some(6.5))]),
        ..Cart::default()
      };

      assert_eq!(unresolved_type_ids(&cart, 10_000_002), vec![35]);
    }

    #[test]
    fn it_includes_saved_cart_lines_in_the_batch() {
      let cart = Cart {
        lines: vec![line(34, 5)],
        saved: vec![saved_entry(7, "Restock", vec![line(35, 2)])],
        ..Cart::default()
      };

      assert_eq!(unresolved_type_ids(&cart, 10_000_002), vec![34, 35]);
    }

    #[test]
    fn it_sums_only_fully_priced_lines() {
      let mut state = state_with_lines(vec![line(34, 5), line(35, 2)]);
      state.active_place = Some(region(10_000_002));
      state.cart.price_scope = Some(10_000_002);
      state.cart.prices = BestSellPrices::from([(34, Some(6.5))]);

      assert_eq!(priced_sum(&state, &state.cart.lines.clone()), None);

      state.cart.prices.insert(35, Some(2.0));
      assert_eq!(
        priced_sum(&state, &state.cart.lines.clone()),
        Some(6.5 * 5.0 + 2.0 * 2.0)
      );
    }

    #[test]
    fn it_triggers_on_open_load_and_market_switch() {
      assert!(wants_prices(&Message::CartOpened));
      assert!(wants_prices(&Message::CartLoaded(Box::default())));
      assert!(wants_prices(&Message::RegionPicked(region(10_000_002))));
      assert!(wants_prices(&Message::RegionResolved(region(10_000_002))));
      assert!(wants_prices(&Message::DefaultMarketResolved(region(10_000_002))));
      assert!(!wants_prices(&Message::CartCleared));
    }
  }
}
