use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use super::{
  Message, State,
  tree::{MarketNode, MarketTree},
};
use crate::{
  features::assets::LocationRef,
  store::{
    Database,
    model::{Character, MarketWatch, NewWatch, WatchDirection},
    repo::{character, market_watchlist},
  },
  ui::{
    components::{
      button::Button,
      eyebrow::eyebrow_text,
      icon::Icon,
      location_combobox::{LocationCombobox, LocationSearch},
      modal_overlay,
      text_input::TextInput,
    },
    format::fmt_isk_opt,
    style::{color, radius, shadow, spacing, typography},
  },
};

const CARD_WIDTH: f32 = 460.0;
const BODY_MAX_HEIGHT: f32 = 460.0;
const ITEM_LIST_HEIGHT: f32 = 260.0;
const DIRECTION_PAD_Y: f32 = 9.0;
const FIELD_HEIGHT: f32 = 42.0;
const MAX_ITEM_RESULTS: usize = 50;

// ── Item identity ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub(super) struct WatchItem {
  pub name: String,
  pub type_id: i64,
}

struct FlatItem {
  group: String,
  name: String,
  type_id: i64,
}

// ── Modal form ────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub(super) struct WatchForm {
  character_id: i64,
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
      character_id: 0,
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

  fn editing(watch: &MarketWatch, tree: &MarketTree) -> Self {
    let item = find_item(tree, watch.type_id);
    let region = watch.region_id.map(|id| super::region_location(id, String::new()));
    Self {
      character_id: watch.character_id,
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
  character_id: i64,
  direction: WatchDirection,
  editing: Option<i64>,
  region_id: Option<i64>,
  target_price: f64,
  type_id: i64,
}

fn to_submit(form: &WatchForm) -> Option<WatchSubmit> {
  let item = form.item.as_ref()?;
  let target_price = form.target_value()?;
  Some(WatchSubmit {
    character_id: form.character_id,
    direction: form.direction,
    editing: form.editing,
    region_id: form.region.as_ref().map(|region| region.id),
    target_price,
    type_id: item.type_id,
  })
}

// ── Reducer + follow-ups ──────────────────────────────────────

enum Follow {
  Book,
  None,
  Persist(WatchSubmit),
  Search(String),
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
    | Message::WatchSaved => {}
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
    Message::WatchRegionSearchChanged(query) => Follow::Search(query.clone()),
    Message::WatchSubmitted => state
      .watch_modal
      .as_ref()
      .and_then(to_submit)
      .map_or(Follow::None, Follow::Persist),
    _ => Follow::None,
  }
}

pub(super) fn reduce(state: &mut State, message: Message) {
  match message {
    Message::WatchNew => state.watch_modal = Some(WatchForm::new(state.active_region.clone())),
    Message::WatchEdit(watch) => state.watch_modal = Some(WatchForm::editing(&watch, &state.tree)),
    Message::WatchModalClosed | Message::WatchSubmitted => state.watch_modal = None,
    Message::WatchItemPickerToggled => with_form(state, WatchForm::toggle_item_picker),
    Message::WatchItemSearchChanged(query) => with_form(state, |form| form.set_item_query(query)),
    Message::WatchItemPicked(type_id, name) => with_form(state, |form| form.pick_item(type_id, name)),
    Message::WatchDirectionSelected(direction) => with_form(state, |form| form.set_direction(direction)),
    Message::WatchTargetChanged(target) => with_form(state, |form| form.set_target(target)),
    _ => reduce_region(state, message),
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

fn with_form(state: &mut State, apply: impl FnOnce(&mut WatchForm)) {
  if let Some(form) = state.watch_modal.as_mut() {
    apply(form);
  }
}

fn execute(state: &State, db: &Database, follow: Follow) -> Task<Message> {
  match follow {
    Follow::None => Task::none(),
    Follow::Book => fetch_book_task(state, db),
    Follow::Persist(submit) => Task::perform(persist(db.clone(), submit), |()| Message::WatchSaved),
    Follow::Search(query) => search_task(state, db, query),
  }
}

fn fetch_book_task(state: &State, db: &Database) -> Task<Message> {
  let Some(form) = state.watch_modal.as_ref() else {
    return Task::none();
  };
  match (
    form.region.as_ref().map(|region| region.id),
    form.item.as_ref().map(|item| item.type_id),
  ) {
    (Some(region_id), Some(type_id)) => super::load_book(db, region_id, type_id),
    _ => Task::none(),
  }
}

fn search_task(state: &State, db: &Database, query: String) -> Task<Message> {
  let Some(form) = state.watch_modal.as_ref() else {
    return Task::none();
  };
  if !form.region_search.searchable() {
    return Task::none();
  }
  let generation = form.region_search.generation();
  Task::perform(
    super::search_regions(db.clone(), query, generation),
    |(generation, results)| Message::WatchRegionResultsLoaded(generation, results),
  )
}

async fn persist(db: Database, submit: WatchSubmit) {
  let character_id = if submit.character_id != 0 {
    submit.character_id
  } else if let Some(id) = default_owner(&db).await {
    id
  } else {
    return;
  };

  let new = NewWatch {
    character_id,
    direction: submit.direction,
    location_id: None,
    region_id: submit.region_id,
    target_price: Some(submit.target_price),
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

async fn default_owner(db: &Database) -> Option<i64> {
  character::all_owned(db)
    .await
    .ok()
    .and_then(|characters| characters.first().map(Character::id))
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

// ── Surface + overlay mount ───────────────────────────────────

pub(super) fn surface<'a>() -> Element<'a, Message> {
  let icon = container(
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
  });

  let new_button: Element<'a, Message> = Button::primary(tr("market.watch.new_button"))
    .icon(Icon::plus())
    .on_press(Message::WatchNew)
    .into();

  let stack = Column::with_children(vec![
    icon.into(),
    text(t!("market.watchlist_empty_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("market.watchlist_empty_body").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::Word)
      .style(typography::colored(color::text::secondary()))
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
  .spacing(spacing::SPACE_2)
  .align_x(Horizontal::Center);

  container(container(stack).max_width(360.0))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(spacing::SPACE_6)
    .into()
}

pub(super) fn mount<'a>(base: Element<'a, Message>, state: &'a State) -> Element<'a, Message> {
  let layers = match state.watch_modal.as_ref() {
    Some(form) => modal_overlay::modal_layers(Message::WatchModalClosed, card(form, state)),
    None => Vec::new(),
  };
  modal_overlay::stable_overlay(base, layers)
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

  button(row)
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
      character_id: 90_000_001,
      created_at: "2026-07-13T00:00:00Z".to_owned(),
      direction: "sell".to_owned(),
      id: 42,
      location_id: None,
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
      let form = WatchForm::editing(&watch(), &tree());

      assert_eq!(form.editing, Some(42));
      assert_eq!(form.item.as_ref().map(|item| item.type_id), Some(587));
      assert_eq!(
        form.item.as_ref().map(|item| item.name.clone()),
        Some("Rifter".to_owned())
      );
      assert_eq!(form.direction, WatchDirection::Sell);
      assert_eq!(form.region.as_ref().map(|region| region.id), Some(10_000_002));
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
      assert_eq!(submit.region_id, Some(10_000_002));
      assert_eq!(submit.target_price, 6_500_000.0);
      assert_eq!(submit.editing, None);
    }

    #[test]
    fn it_refuses_a_submit_without_an_item() {
      let mut form = WatchForm::new(None);
      form.set_target("100".to_owned());

      assert!(to_submit(&form).is_none());
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
        tier: Some(crate::features::assets::LocationTier::Region),
      };
      reduce(&mut state, Message::WatchRegionResultsLoaded(0, vec![region.clone()]));
      reduce(&mut state, Message::WatchRegionPicked(region));

      let form = state.watch_modal.as_ref().unwrap();
      assert_eq!(form.region.as_ref().map(|location| location.id), Some(10_000_002));
      assert!(!form.region_picker_open);
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
    async fn it_persists_a_new_watch_to_the_owner_character() {
      let db = store::open_test().await.unwrap();
      seed_owner(&db).await;
      let submit = WatchSubmit {
        character_id: 0,
        direction: WatchDirection::Buy,
        editing: None,
        region_id: Some(10_000_002),
        target_price: 5.0,
        type_id: 34,
      };

      persist(db.clone(), submit).await;

      let rows = market_watchlist::list(&db).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].type_id, 34);
      assert_eq!(rows[0].character_id, 90_000_001);
    }

    async fn seed_owner(db: &Database) {
      use crate::store::model::{Alliance, Bloodline, Character as Char, Corporation, Gender, Race};

      let id = 90_000_001;
      let corp_id = 98_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let mut character = Char::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character.set_security_status(0.0);
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO credentials \
          (owner_id, owner_type, access_token, refresh_token, expires_at, authorized_by, scopes, created_at, updated_at) \
        VALUES (?, 'character', 'a', 'r', 0, NULL, NULL, 0, 0)",
      )
      .bind(id)
      .execute(db.writer())
      .await
      .unwrap();
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_empty_surface() {
      let _el: Element<'_, Message> = surface();
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
}
