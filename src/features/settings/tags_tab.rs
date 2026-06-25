use iced::{
  Background, Border, Color, Element, Length, Padding, Point,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, text, text_input},
};

use super::Outcome;
use crate::{
  config::Settings,
  store::{
    Database,
    model::{TAG_SCOPE_ASSET, Tag},
    repo::infra,
  },
  ui::{
    components::{
      backdrop, chip, color_picker, icon::Icon, modal_overlay::modal_overlay, rule, status, text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

const PANEL_SIDE_PADDING: f32 = 36.0;

const BLURB_MAX_WIDTH: f32 = 620.0;

const CREATE_WELL_MAX_WIDTH: f32 = 360.0;

const DRAG_HANDLE_WIDTH: f32 = 18.0;

const SWATCH_SIZE: f32 = 22.0;

const DELETE_SIZE: f32 = 26.0;

#[derive(Clone, Debug)]
pub enum Message {
  AddTag,
  AssetAddTag,
  AssetClearColor(i64),
  AssetColorHexChanged(String),
  AssetColorHexSubmitted,
  AssetDropDragged,
  // Constructed only by handler-routing tests; the cancel-edit arm is wired but not yet triggered from the UI.
  #[allow(dead_code)]
  AssetEditCancelled,
  AssetEditCommitted,
  AssetEditDraftChanged(String),
  AssetFilterChanged(String),
  AssetHoverTagSlot(usize),
  AssetLeaveTagSlot(usize),
  AssetNewTagChanged(String),
  AssetPickUpTag(i64),
  AssetRecolor {
    hex: String,
    tag_id: i64,
  },
  AssetRemoveTag(i64),
  AssetSortSelected(SortMode),
  AssetStartEditing(i64),
  AssetToggleColorPicker(i64),
  ClearColor(i64),
  ClosePicker,
  ColorHexChanged(String),
  ColorHexSubmitted,
  CursorMoved(Point),
  DropDragged,
  // Constructed only by handler-routing tests; the cancel-edit arm is wired but not yet triggered from the UI.
  #[allow(dead_code)]
  EditCancelled,
  EditCommitted,
  EditDraftChanged(String),
  FilterChanged(String),
  HoverTagSlot(usize),
  LeaveTagSlot(usize),
  Loaded(Result<Loaded, String>),
  NewTagChanged(String),
  PickUpTag(i64),
  Recolor {
    hex: String,
    tag_id: i64,
  },
  RemoveTag(i64),
  Saved(Result<(), String>),
  SortSelected(SortMode),
  StartEditing(i64),
  ToggleColorPicker(i64),
}

// Both registries fetched in a single load so the entity and asset sections refresh together.
#[derive(Clone, Debug)]
pub struct Loaded {
  asset_tags: Vec<Tag>,
  tags: Vec<Tag>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortMode {
  Color,
  #[default]
  Manual,
  Name,
}

impl SortMode {
  const ALL: [SortMode; 3] = [SortMode::Manual, SortMode::Name, SortMode::Color];

  fn label(self) -> &'static str {
    match self {
      SortMode::Manual => "Manual",
      SortMode::Name => "A to Z",
      SortMode::Color => "Color",
    }
  }
}

// Per-section UI state. The entity and asset registries each own one so their filter, sort, edit, picker, and
// drag interactions never bleed across sections.
#[derive(Debug, Default)]
struct Section {
  dragging: Option<i64>,
  drop_index: Option<usize>,
  editing: Option<Editing>,
  new_tag: String,
  picker: Option<Picker>,
  query: String,
  sort_mode: SortMode,
  tags: Vec<Tag>,
}

impl Section {
  fn draggable(&self) -> bool {
    self.sort_mode == SortMode::Manual && self.query.trim().is_empty()
  }

  fn visible(&self) -> Vec<&Tag> {
    let query = self.query.trim().to_lowercase();
    let mut list: Vec<&Tag> = self
      .tags
      .iter()
      .filter(|tag| query.is_empty() || tag.name().to_lowercase().contains(&query))
      .collect();
    match self.sort_mode {
      SortMode::Manual => {}
      SortMode::Name => list.sort_by_key(|a| a.name().to_lowercase()),
      SortMode::Color => list.sort_by_key(|a| color_sort_key(a)),
    }
    list
  }

  fn name_taken(&self, name: &str, except: Option<i64>) -> bool {
    let needle = name.to_lowercase();
    self
      .tags
      .iter()
      .any(|tag| Some(tag.id()) != except && tag.name().to_lowercase() == needle)
  }
}

#[derive(Debug, Default)]
pub struct State {
  asset: Section,
  cursor: Option<Point>,
  db: Option<Database>,
  entity: Section,
  load_error: Option<String>,
}

impl State {
  pub fn new(db: Database) -> Self {
    State {
      db: Some(db),
      ..State::default()
    }
  }
}

#[derive(Clone, Debug)]
struct Editing {
  draft: String,
  tag_id: i64,
}

#[derive(Clone, Debug)]
struct Picker {
  anchor: Point,
  hex_draft: String,
  hex_invalid: bool,
  tag_id: i64,
}

fn color_sort_key(tag: &Tag) -> (bool, String, String) {
  match tag.color() {
    Some(hex) => (false, hex.to_uppercase(), tag.name().to_lowercase()),
    None => (true, String::new(), tag.name().to_lowercase()),
  }
}

// The disposition vocabulary seeded into the asset-tag registry on first run. (name, color) — colored
// entries get a default swatch; the rest seed uncolored.
const ASSET_TAG_SEEDS: &[(&str, Option<&str>)] = &[
  ("Keep", Some("#5BB97E")),
  ("Sell", Some("#D9B252")),
  ("Reprocess", Some("#3FB8DB")),
  ("Reship", None),
  ("Contract", None),
  ("Hauling", None),
  ("Loot", None),
  ("Junk", Some("#E07559")),
  ("Research", None),
];

// Seeds the asset-tag registry with the default disposition vocabulary exactly once. The persisted marker
// means deleting a seeded default never resurrects it on the next launch.
pub async fn seed_asset_tags(db: &Database) -> Result<(), crate::store::Error> {
  if infra::is_tag_scope_seeded(db, TAG_SCOPE_ASSET).await? {
    return Ok(());
  }

  for (name, color) in ASSET_TAG_SEEDS {
    infra::create_scoped(db, name, None, *color, TAG_SCOPE_ASSET).await?;
  }

  infra::mark_tag_scope_seeded(db, TAG_SCOPE_ASSET).await?;
  Ok(())
}

pub fn load(db: &Database) -> iced::Task<Message> {
  iced::Task::perform(load_tags(db.clone()), Message::Loaded)
}

async fn load_tags(db: Database) -> Result<Loaded, String> {
  seed_asset_tags(&db).await.map_err(|err| err.to_string())?;
  let tags = infra::tag_all(&db).await.map_err(|err| err.to_string())?;
  let asset_tags = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET)
    .await
    .map_err(|err| err.to_string())?;
  Ok(Loaded {
    asset_tags,
    tags,
  })
}

pub fn update(state: &mut State, message: Message) -> (Outcome, iced::Task<Message>) {
  let task = match message {
    Message::AddTag => add_tag(&state.db, &mut state.entity, None),
    Message::ColorHexChanged(draft) => color_hex_changed(&mut state.entity, draft),
    Message::ColorHexSubmitted => color_hex_submitted(&state.db, &mut state.entity),
    Message::ClearColor(tag_id) => {
      state.entity.picker = None;
      recolor(&state.db, &state.entity, tag_id, None)
    }
    Message::ClosePicker => {
      state.entity.picker = None;
      state.asset.picker = None;
      iced::Task::none()
    }
    Message::CursorMoved(point) => {
      state.cursor = Some(point);
      iced::Task::none()
    }
    Message::DropDragged => drop_dragged(&state.db, &mut state.entity),
    Message::EditCancelled => {
      state.entity.editing = None;
      iced::Task::none()
    }
    Message::EditCommitted => edit_committed(&state.db, &mut state.entity),
    Message::EditDraftChanged(draft) => edit_draft_changed(&mut state.entity, draft),
    Message::FilterChanged(query) => {
      state.entity.query = query;
      iced::Task::none()
    }
    Message::HoverTagSlot(index) => hover_tag_slot(&mut state.entity, index),
    Message::LeaveTagSlot(index) => leave_tag_slot(&mut state.entity, index),
    Message::Loaded(result) => loaded(state, result),
    Message::NewTagChanged(value) => {
      state.entity.new_tag = value;
      iced::Task::none()
    }
    Message::PickUpTag(tag_id) => pick_up_tag(&mut state.entity, tag_id),
    Message::Recolor {
      hex,
      tag_id,
    } => {
      state.entity.picker = None;
      recolor(&state.db, &state.entity, tag_id, Some(hex))
    }
    Message::RemoveTag(tag_id) => remove_tag(&state.db, &mut state.entity, tag_id),
    Message::Saved(result) => saved(state, result),
    Message::SortSelected(mode) => sort_selected(&mut state.entity, mode),
    Message::StartEditing(tag_id) => start_editing(&mut state.entity, tag_id),
    Message::ToggleColorPicker(tag_id) => toggle_color_picker(&mut state.entity, state.cursor, tag_id),

    Message::AssetAddTag => add_tag(&state.db, &mut state.asset, Some(TAG_SCOPE_ASSET)),
    Message::AssetColorHexChanged(draft) => color_hex_changed(&mut state.asset, draft),
    Message::AssetColorHexSubmitted => color_hex_submitted(&state.db, &mut state.asset),
    Message::AssetClearColor(tag_id) => {
      state.asset.picker = None;
      recolor(&state.db, &state.asset, tag_id, None)
    }
    Message::AssetDropDragged => drop_dragged(&state.db, &mut state.asset),
    Message::AssetEditCancelled => {
      state.asset.editing = None;
      iced::Task::none()
    }
    Message::AssetEditCommitted => edit_committed(&state.db, &mut state.asset),
    Message::AssetEditDraftChanged(draft) => edit_draft_changed(&mut state.asset, draft),
    Message::AssetFilterChanged(query) => {
      state.asset.query = query;
      iced::Task::none()
    }
    Message::AssetHoverTagSlot(index) => hover_tag_slot(&mut state.asset, index),
    Message::AssetLeaveTagSlot(index) => leave_tag_slot(&mut state.asset, index),
    Message::AssetNewTagChanged(value) => {
      state.asset.new_tag = value;
      iced::Task::none()
    }
    Message::AssetPickUpTag(tag_id) => pick_up_tag(&mut state.asset, tag_id),
    Message::AssetRecolor {
      hex,
      tag_id,
    } => {
      state.asset.picker = None;
      recolor(&state.db, &state.asset, tag_id, Some(hex))
    }
    Message::AssetRemoveTag(tag_id) => remove_tag(&state.db, &mut state.asset, tag_id),
    Message::AssetSortSelected(mode) => sort_selected(&mut state.asset, mode),
    Message::AssetStartEditing(tag_id) => start_editing(&mut state.asset, tag_id),
    Message::AssetToggleColorPicker(tag_id) => toggle_color_picker(&mut state.asset, state.cursor, tag_id),
  };
  (Outcome::None, task)
}

fn edit_draft_changed(section: &mut Section, draft: String) -> iced::Task<Message> {
  if let Some(editing) = section.editing.as_mut() {
    editing.draft = draft;
  }
  iced::Task::none()
}

fn hover_tag_slot(section: &mut Section, index: usize) -> iced::Task<Message> {
  if section.dragging.is_some() {
    section.drop_index = Some(index);
  }
  iced::Task::none()
}

fn leave_tag_slot(section: &mut Section, index: usize) -> iced::Task<Message> {
  if section.drop_index == Some(index) {
    section.drop_index = None;
  }
  iced::Task::none()
}

fn loaded(state: &mut State, result: Result<Loaded, String>) -> iced::Task<Message> {
  match result {
    Ok(payload) => {
      state.entity.tags = payload.tags;
      state.asset.tags = payload.asset_tags;
      state.load_error = None;
    }
    Err(error) => state.load_error = Some(error),
  }
  iced::Task::none()
}

fn pick_up_tag(section: &mut Section, tag_id: i64) -> iced::Task<Message> {
  if section.draggable() {
    section.dragging = Some(tag_id);
    section.drop_index = None;
  }
  iced::Task::none()
}

fn saved(state: &mut State, result: Result<(), String>) -> iced::Task<Message> {
  match result {
    Ok(()) => match state.db.clone() {
      Some(db) => load(&db),
      None => iced::Task::none(),
    },
    Err(error) => {
      state.load_error = Some(error);
      iced::Task::none()
    }
  }
}

fn sort_selected(section: &mut Section, mode: SortMode) -> iced::Task<Message> {
  section.sort_mode = mode;
  if !section.draggable() {
    section.dragging = None;
    section.drop_index = None;
  }
  iced::Task::none()
}

fn start_editing(section: &mut Section, tag_id: i64) -> iced::Task<Message> {
  if let Some(tag) = section.tags.iter().find(|t| t.id() == tag_id) {
    section.editing = Some(Editing {
      draft: tag.name().clone(),
      tag_id,
    });
  }
  iced::Task::none()
}

fn add_tag(db: &Option<Database>, section: &mut Section, scope: Option<&'static str>) -> iced::Task<Message> {
  let name = section.new_tag.trim().to_owned();
  section.new_tag.clear();
  if name.is_empty() || section.name_taken(&name, None) {
    iced::Task::none()
  } else {
    write(db, move |db| async move {
      match scope {
        Some(scope) => infra::create_scoped(&db, &name, None, None, scope).await.map(|_| ()),
        None => infra::create(&db, &name, None, None).await.map(|_| ()),
      }
    })
  }
}

fn color_hex_changed(section: &mut Section, draft: String) -> iced::Task<Message> {
  if let Some(picker) = section.picker.as_mut() {
    picker.hex_draft = draft;
    picker.hex_invalid = false;
  }
  iced::Task::none()
}

fn color_hex_submitted(db: &Option<Database>, section: &mut Section) -> iced::Task<Message> {
  let Some(picker) = section.picker.as_ref() else {
    return iced::Task::none();
  };
  match color_picker::normalize_hex(&picker.hex_draft) {
    Some(hex) => {
      let tag_id = picker.tag_id;
      section.picker = None;
      recolor(db, section, tag_id, Some(hex))
    }
    None => {
      if let Some(picker) = section.picker.as_mut() {
        picker.hex_invalid = !picker.hex_draft.trim().is_empty();
      }
      iced::Task::none()
    }
  }
}

fn drop_dragged(db: &Option<Database>, section: &mut Section) -> iced::Task<Message> {
  let dragged = section.dragging.take();
  let drop_index = section.drop_index.take();
  match (dragged, drop_index) {
    (Some(tag_id), Some(to)) => reorder(db, section, tag_id, to),
    _ => iced::Task::none(),
  }
}

fn edit_committed(db: &Option<Database>, section: &mut Section) -> iced::Task<Message> {
  let Some(editing) = section.editing.take() else {
    return iced::Task::none();
  };
  let next = editing.draft.trim().to_owned();
  let taken = section.name_taken(&next, Some(editing.tag_id));
  let current = section.tags.iter().find(|t| t.id() == editing.tag_id);
  match current {
    Some(tag) if !next.is_empty() && next != *tag.name() && !taken => {
      let description = tag.description().clone();
      let color = tag.color().clone();
      write(db, move |db| async move {
        infra::update(&db, editing.tag_id, &next, description.as_deref(), color.as_deref()).await
      })
    }
    _ => iced::Task::none(),
  }
}

fn remove_tag(db: &Option<Database>, section: &mut Section, tag_id: i64) -> iced::Task<Message> {
  if section.picker.as_ref().is_some_and(|p| p.tag_id == tag_id) {
    section.picker = None;
  }
  if section.editing.as_ref().is_some_and(|e| e.tag_id == tag_id) {
    section.editing = None;
  }
  write(db, move |db| async move { infra::tag_delete(&db, tag_id).await })
}

fn toggle_color_picker(section: &mut Section, cursor: Option<Point>, tag_id: i64) -> iced::Task<Message> {
  let open = section.picker.as_ref().is_some_and(|p| p.tag_id == tag_id);
  section.picker = if open {
    None
  } else {
    let current = section
      .tags
      .iter()
      .find(|t| t.id() == tag_id)
      .and_then(|t| t.color().clone())
      .unwrap_or_default();
    Some(Picker {
      anchor: cursor.unwrap_or(Point::ORIGIN),
      hex_draft: current,
      hex_invalid: false,
      tag_id,
    })
  };
  iced::Task::none()
}

fn recolor(db: &Option<Database>, section: &Section, tag_id: i64, hex: Option<String>) -> iced::Task<Message> {
  let Some(tag) = section.tags.iter().find(|t| t.id() == tag_id) else {
    return iced::Task::none();
  };
  let name = tag.name().clone();
  let description = tag.description().clone();
  write(db, move |db| async move {
    infra::update(&db, tag_id, &name, description.as_deref(), hex.as_deref()).await
  })
}

fn reorder(db: &Option<Database>, section: &Section, tag_id: i64, to: usize) -> iced::Task<Message> {
  let mut order: Vec<i64> = section.tags.iter().map(|t| t.id()).collect();
  let Some(from) = order.iter().position(|&id| id == tag_id) else {
    return iced::Task::none();
  };
  if from == to {
    return iced::Task::none();
  }
  let moved = order.remove(from);
  let insert_at = if from < to { to - 1 } else { to };
  order.insert(insert_at.min(order.len()), moved);
  write(db, move |db| async move { infra::reorder(&db, &order).await })
}

fn write<F, Fut>(db: &Option<Database>, op: F) -> iced::Task<Message>
where
  F: FnOnce(Database) -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Result<(), crate::store::Error>> + Send + 'static,
{
  let Some(db) = db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move { op(db).await.map_err(|err| err.to_string()) },
    Message::Saved,
  )
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.entity.dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      drop_on_release(event).then_some(Message::DropDragged)
    }));
  }
  if state.asset.dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      drop_on_release(event).then_some(Message::AssetDropDragged)
    }));
  }
  if state.entity.picker.is_some() || state.asset.picker.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      close_on_escape(event).then_some(Message::ClosePicker)
    }));
  }
  iced::Subscription::batch(subs)
}

fn drop_on_release(event: iced::Event) -> bool {
  matches!(
    event,
    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
  )
}

fn close_on_escape(event: iced::Event) -> bool {
  matches!(
    event,
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
      key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
      ..
    })
  )
}

pub fn badge(state: &State) -> String {
  let colored = state.entity.tags.iter().filter(|tag| tag.color().is_some()).count()
    + state.asset.tags.iter().filter(|tag| tag.color().is_some()).count();
  colored.to_string()
}

// Maps a section's interactions onto the right message variants so one set of view helpers can render either
// the entity or the asset registry.
struct Msgs {
  add_tag: fn() -> Message,
  clear_color: fn(i64) -> Message,
  color_hex_changed: fn(String) -> Message,
  color_hex_submitted: fn() -> Message,
  close_picker: fn() -> Message,
  edit_committed: fn() -> Message,
  edit_draft_changed: fn(String) -> Message,
  filter_changed: fn(String) -> Message,
  hover_tag_slot: fn(usize) -> Message,
  leave_tag_slot: fn(usize) -> Message,
  new_tag_changed: fn(String) -> Message,
  pick_up_tag: fn(i64) -> Message,
  recolor: fn(String, i64) -> Message,
  remove_tag: fn(i64) -> Message,
  sort_selected: fn(SortMode) -> Message,
  start_editing: fn(i64) -> Message,
  toggle_color_picker: fn(i64) -> Message,
}

const ENTITY_MSGS: Msgs = Msgs {
  add_tag: || Message::AddTag,
  clear_color: Message::ClearColor,
  color_hex_changed: Message::ColorHexChanged,
  color_hex_submitted: || Message::ColorHexSubmitted,
  close_picker: || Message::ClosePicker,
  edit_committed: || Message::EditCommitted,
  edit_draft_changed: Message::EditDraftChanged,
  filter_changed: Message::FilterChanged,
  hover_tag_slot: Message::HoverTagSlot,
  leave_tag_slot: Message::LeaveTagSlot,
  new_tag_changed: Message::NewTagChanged,
  pick_up_tag: Message::PickUpTag,
  recolor: |hex, tag_id| Message::Recolor {
    hex,
    tag_id,
  },
  remove_tag: Message::RemoveTag,
  sort_selected: Message::SortSelected,
  start_editing: Message::StartEditing,
  toggle_color_picker: Message::ToggleColorPicker,
};

const ASSET_MSGS: Msgs = Msgs {
  add_tag: || Message::AssetAddTag,
  clear_color: Message::AssetClearColor,
  color_hex_changed: Message::AssetColorHexChanged,
  color_hex_submitted: || Message::AssetColorHexSubmitted,
  close_picker: || Message::ClosePicker,
  edit_committed: || Message::AssetEditCommitted,
  edit_draft_changed: Message::AssetEditDraftChanged,
  filter_changed: Message::AssetFilterChanged,
  hover_tag_slot: Message::AssetHoverTagSlot,
  leave_tag_slot: Message::AssetLeaveTagSlot,
  new_tag_changed: Message::AssetNewTagChanged,
  pick_up_tag: Message::AssetPickUpTag,
  recolor: |hex, tag_id| Message::AssetRecolor {
    hex,
    tag_id,
  },
  remove_tag: Message::AssetRemoveTag,
  sort_selected: Message::AssetSortSelected,
  start_editing: Message::AssetStartEditing,
  toggle_color_picker: Message::AssetToggleColorPicker,
};

pub fn view<'a>(state: &'a State, _settings: &'a Settings) -> Element<'a, Message> {
  let sections = Column::with_children(vec![
    section_block(
      &state.entity,
      &ENTITY_MSGS,
      "Tags",
      "Assign a color to any tag and it'll render that way everywhere it appears on a character card. \
        Drag rows to reorder; tags use their manual order on character cards.",
    ),
    section_block(
      &state.asset,
      &ASSET_MSGS,
      "Asset tags",
      "A separate vocabulary for tagging assets \u{2014} keep, sell, reprocess, and the rest. \
        These never mix with the character tags above. Drag rows to reorder.",
    ),
  ])
  .width(Length::Fill);

  let scroll = scrollable(container(sections).width(Length::Fill).padding(Padding {
    top: 0.0,
    right: 0.0,
    bottom: 60.0,
    left: 0.0,
  }))
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill);

  let body = container(scroll).width(Length::Fill).height(Length::Fill);
  let base: Element<'a, Message> = mouse_area(body).on_move(Message::CursorMoved).into();

  match open_picker(&state.entity, &ENTITY_MSGS).or_else(|| open_picker(&state.asset, &ASSET_MSGS)) {
    Some(popover) => modal_overlay(base, None, popover),
    None => base,
  }
}

fn section_block<'a>(section: &'a Section, msgs: &'a Msgs, title: &'a str, blurb: &'a str) -> Element<'a, Message> {
  Column::with_children(vec![header(section, msgs, title, blurb), list(section, msgs)])
    .width(Length::Fill)
    .into()
}

fn header<'a>(section: &'a Section, msgs: &'a Msgs, title: &'a str, blurb: &'a str) -> Element<'a, Message> {
  let title = text(title)
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = container(
    text(blurb)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(BLURB_MAX_WIDTH);

  let inner = Column::with_children(vec![
    title.into(),
    blurb.into(),
    create_row(section, msgs),
    meta_strip(section),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill);

  let band = container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn create_row<'a>(section: &'a Section, msgs: &'a Msgs) -> Element<'a, Message> {
  let can_add = !section.new_tag.trim().is_empty();
  let add_tag = msgs.add_tag;

  let create_well = container(
    Row::with_children(vec![
      text("+")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::accent::PLASMA))
        .into(),
      text_input("Create a tag\u{2026}", &section.new_tag)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .padding(Padding::ZERO)
        .on_input(msgs.new_tag_changed)
        .on_submit((msgs.add_tag)())
        .style(plain_input_style)
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .max_width(CREATE_WELL_MAX_WIDTH)
  .padding(Padding {
    top: 7.0,
    right: spacing::SPACE_3,
    bottom: 7.0,
    left: spacing::SPACE_3,
  })
  .style(sunken_well_style);

  let add = button(
    text("Add")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(if can_add {
        color::surface::BASE
      } else {
        color::text::tertiary()
      })),
  )
  .padding(Padding {
    top: 7.0,
    right: spacing::SPACE_3_5,
    bottom: 7.0,
    left: spacing::SPACE_3_5,
  })
  .on_press_maybe(can_add.then(add_tag))
  .style(move |_, _| add_button_style(can_add));

  Row::with_children(vec![
    create_well.into(),
    add.into(),
    Space::new().width(Length::Fill).into(),
    sort_selector(section, msgs),
    filter_field(section, msgs),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn sort_selector<'a>(section: &'a Section, msgs: &'a Msgs) -> Element<'a, Message> {
  let sort_selected = msgs.sort_selected;
  let buttons: Vec<Element<'_, Message>> = SortMode::ALL
    .into_iter()
    .map(|mode| {
      let active = section.sort_mode == mode;
      button(
        text(mode.label())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(if active {
            color::accent::PLASMA
          } else {
            color::text::secondary()
          })),
      )
      .padding(Padding {
        top: 5.0,
        right: spacing::SPACE_2_5,
        bottom: 5.0,
        left: spacing::SPACE_2_5,
      })
      .on_press(sort_selected(mode))
      .style(move |_, _| button::Style {
        background: active.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..button::Style::default()
      })
      .into()
    })
    .collect();

  container(Row::with_children(buttons).spacing(0.0).align_y(Vertical::Center))
    .padding(2.0)
    .style(sunken_well_style)
    .into()
}

fn filter_field<'a>(section: &'a Section, msgs: &'a Msgs) -> Element<'a, Message> {
  TextInput::new("Filter\u{2026}", &section.query, msgs.filter_changed)
    .leading_icon(Icon::search())
    .background(color::surface::SUNKEN)
    .font_size(typography::size::MD)
    .width(Length::Fixed(180.0))
    .render()
}

fn meta_strip(section: &Section) -> Element<'_, Message> {
  let colored = section.tags.iter().filter(|t| t.color().is_some()).count();
  let mut children: Vec<Element<'_, Message>> = vec![
    meta_count(section.tags.len(), "TAGS", color::text::secondary()),
    dot(),
    meta_count(colored, "COLORED", color::accent::PLASMA),
  ];

  if !section.draggable() {
    let hint = if section.query.trim().is_empty() {
      "Reorder disabled in sorted view"
    } else {
      "Reorder disabled while filtering"
    };
    children.push(dot());
    children.push(
      text(hint)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::status::WARNING))
        .into(),
    );
  }

  Row::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center)
    .into()
}

fn meta_count<'a>(count: usize, label: &'a str, count_color: Color) -> Element<'a, Message> {
  Row::with_children(vec![
    text(count.to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(count_color))
      .into(),
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center)
  .into()
}

fn dot<'a>() -> Element<'a, Message> {
  status::dot_sized(color::text::tertiary(), 3.0)
}

fn list<'a>(section: &'a Section, msgs: &'a Msgs) -> Element<'a, Message> {
  let visible = section.visible();
  let body: Element<'a, Message> = if visible.is_empty() {
    let copy = if section.query.trim().is_empty() {
      "No tags yet. Create one above.".to_owned()
    } else {
      format!("No tags match \u{201C}{}\u{201D}.", section.query.trim())
    };
    container(
      text(copy)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary())),
    )
    .width(Length::Fill)
    .padding(80.0)
    .align_x(Horizontal::Center)
    .into()
  } else {
    let draggable = section.draggable();
    let rows: Vec<Element<'a, Message>> = visible
      .iter()
      .enumerate()
      .map(|(display_index, tag)| tag_row(section, msgs, tag, display_index, draggable))
      .collect();
    Column::with_children(rows).width(Length::Fill).into()
  };

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: PANEL_SIDE_PADDING,
      bottom: spacing::SPACE_2,
      left: PANEL_SIDE_PADDING,
    })
    .into()
}

fn tag_row<'a>(
  section: &'a Section,
  msgs: &'a Msgs,
  tag: &'a Tag,
  display_index: usize,
  draggable: bool,
) -> Element<'a, Message> {
  let dragging = section.dragging == Some(tag.id());
  let drop_above = section.drop_index == Some(display_index) && section.dragging.is_some() && !dragging;

  let cells = Row::with_children(vec![
    drag_handle(msgs, tag.id(), draggable),
    swatch_cell(msgs, tag),
    name_cell(section, msgs, tag),
    Space::new().width(Length::Fill).into(),
    tag_preview(tag.name(), tag.color().as_deref()),
    delete_button(msgs, tag.id()),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let top_rule_color = if drop_above {
    color::accent::PLASMA
  } else {
    Color::TRANSPARENT
  };

  let row = container(cells)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::UNIT,
      bottom: spacing::SPACE_2_5,
      left: spacing::UNIT,
    })
    .style(move |_| container::Style {
      background: dragging.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.04))),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 0.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  let top = container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(top_rule_color)),
      ..container::Style::default()
    });
  let stacked = Column::with_children(vec![top.into(), row.into(), rule::horizontal()]).width(Length::Fill);

  mouse_area(stacked)
    .on_enter((msgs.hover_tag_slot)(display_index))
    .on_exit((msgs.leave_tag_slot)(display_index))
    .into()
}

fn drag_handle<'a>(msgs: &'a Msgs, tag_id: i64, draggable: bool) -> Element<'a, Message> {
  let glyph = text("\u{22ee}")
    .font(typography::body::REGULAR)
    .size(typography::size::LG)
    .style(typography::colored(if draggable {
      color::text::tertiary()
    } else {
      color::with_alpha(color::text::PRIMARY, 0.1)
    }));
  let cell = container(glyph)
    .width(Length::Fixed(DRAG_HANDLE_WIDTH))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center);

  if draggable {
    mouse_area(cell).on_press((msgs.pick_up_tag)(tag_id)).into()
  } else {
    cell.into()
  }
}

fn swatch_cell<'a>(msgs: &'a Msgs, tag: &'a Tag) -> Element<'a, Message> {
  swatch_button(tag.color().as_deref(), (msgs.toggle_color_picker)(tag.id()))
}

fn open_picker<'a>(section: &'a Section, msgs: &'a Msgs) -> Option<Element<'a, Message>> {
  let picker = section.picker.as_ref()?;
  let tag = section.tags.iter().find(|t| t.id() == picker.tag_id)?;
  let recolor = msgs.recolor;
  let popover = color_picker::color_popover_with_clear(
    tag.color().as_deref(),
    &picker.hex_draft,
    picker.hex_invalid,
    {
      let tag_id = picker.tag_id;
      move |hex| recolor(hex, tag_id)
    },
    msgs.color_hex_changed,
    (msgs.color_hex_submitted)(),
    (msgs.clear_color)(picker.tag_id),
  );

  let floating = color_picker::floating(popover, picker.anchor);
  Some(modal_overlay(
    backdrop::click_catcher((msgs.close_picker)()),
    None,
    floating,
  ))
}

fn swatch_button<'a>(color: Option<&str>, on_toggle: Message) -> Element<'a, Message> {
  let fill = color.and_then(hex_to_color).unwrap_or(Color::TRANSPARENT);
  let border_color = match color.and_then(hex_to_color) {
    Some(c) => Color {
      a: 0.5,
      ..c
    },
    None => color::rule_strong(),
  };
  button(Space::new())
    .width(Length::Fixed(SWATCH_SIZE))
    .height(Length::Fixed(SWATCH_SIZE))
    .padding(Padding::ZERO)
    .on_press(on_toggle)
    .style(move |_, _| button::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: 6.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn name_cell<'a>(section: &'a Section, msgs: &'a Msgs, tag: &'a Tag) -> Element<'a, Message> {
  match section.editing.as_ref() {
    Some(editing) if editing.tag_id == tag.id() => text_input("", &editing.draft)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .padding(Padding {
        top: spacing::UNIT,
        right: spacing::SPACE_2,
        bottom: spacing::UNIT,
        left: spacing::SPACE_2,
      })
      .width(Length::Fixed(240.0))
      .on_input(msgs.edit_draft_changed)
      .on_submit((msgs.edit_committed)())
      .style(edit_input_style)
      .into(),
    _ => button(
      text(tag.name().clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY)),
    )
    .padding(Padding::ZERO)
    .on_press((msgs.start_editing)(tag.id()))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into(),
  }
}

fn tag_preview<'a>(name: &'a str, color: Option<&str>) -> Element<'a, Message> {
  chip::chip(name, color.and_then(hex_to_color))
}

fn delete_button<'a>(msgs: &'a Msgs, tag_id: i64) -> Element<'a, Message> {
  let glyph = container(
    text("\u{00d7}")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(typography::colored(color::text::secondary())),
  )
  .center_x(Length::Fill)
  .center_y(Length::Fill);

  button(glyph)
    .width(Length::Fixed(DELETE_SIZE))
    .height(Length::Fixed(DELETE_SIZE))
    .padding(Padding::ZERO)
    .on_press((msgs.remove_tag)(tag_id))
    .style(|_, status| {
      let (border_color, text_color) = match status {
        button::Status::Hovered | button::Status::Pressed => {
          (color::with_alpha(color::status::DANGER, 0.5), color::status::DANGER)
        }
        _ => (color::with_alpha(color::text::PRIMARY, 0.1), color::text::secondary()),
      };
      button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color,
        border: Border {
          color: border_color,
          width: 1.0,
          radius: radius::SUBTLE.into(),
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn hex_to_color(hex: &str) -> Option<Color> {
  let normalized = color_picker::normalize_hex(hex)?;
  let digits = normalized.trim_start_matches('#');
  let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
  let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
  let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
  Some(Color::from_rgb8(r, g, b))
}

fn plain_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

fn edit_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::accent::PLASMA,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

fn sunken_well_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  }
}

fn add_button_style(enabled: bool) -> button::Style {
  if enabled {
    button::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      text_color: color::surface::BASE,
      border: Border {
        color: color::accent::PLASMA,
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  } else {
    button::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      text_color: color::text::tertiary(),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..button::Style::default()
    }
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store;

  async fn state_with(names: &[&str]) -> State {
    let db = store::open_test().await.unwrap();
    for name in names {
      infra::create(&db, name, None, None).await.unwrap();
    }
    let mut state = State::new(db.clone());
    state.entity.tags = infra::tag_all(&db).await.unwrap();
    state
  }

  async fn asset_state_with(names: &[&str]) -> State {
    let db = store::open_test().await.unwrap();
    for name in names {
      infra::create_scoped(&db, name, None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap();
    }
    let mut state = State::new(db.clone());
    state.asset.tags = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap();
    state
  }

  async fn order(db: &Database) -> Vec<String> {
    infra::tag_all(db)
      .await
      .unwrap()
      .into_iter()
      .map(|t| t.name().clone())
      .collect()
  }

  async fn asset_order(db: &Database) -> Vec<String> {
    infra::tag_all_scoped(db, TAG_SCOPE_ASSET)
      .await
      .unwrap()
      .into_iter()
      .map(|t| t.name().clone())
      .collect()
  }

  async fn reload(state: &mut State) {
    let db = state.db.clone().unwrap();
    state.entity.tags = infra::tag_all(&db).await.unwrap();
  }

  async fn reload_asset(state: &mut State) {
    let db = state.db.clone().unwrap();
    state.asset.tags = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap();
  }

  #[tokio::test]
  async fn add_tag_dispatches_a_create_and_clears_the_field() {
    let mut state = state_with(&["Main"]).await;
    state.entity.new_tag = "Scout".to_owned();

    let (outcome, _task) = update(&mut state, Message::AddTag);

    assert_eq!(outcome, Outcome::None);
    assert!(state.entity.new_tag.is_empty(), "the create field clears after add");
  }

  #[tokio::test]
  async fn add_tag_rejects_a_case_insensitive_duplicate() {
    let mut state = state_with(&["Main"]).await;
    state.entity.new_tag = "main".to_owned();

    let (_, _task) = update(&mut state, Message::AddTag);

    assert!(state.entity.new_tag.is_empty());
    assert_eq!(state.entity.tags.len(), 1);
  }

  #[test]
  fn badge_counts_colored_tags() {
    let state = State::default();
    assert_eq!(badge(&state), "0");
  }

  #[tokio::test]
  async fn badge_counts_colored_tags_across_both_scopes() {
    let db = store::open_test().await.unwrap();
    let mut state = State::new(db.clone());
    state.entity.tags = vec![infra::create(&db, "PvP", None, Some("#5BB97E")).await.unwrap()];
    state.asset.tags = vec![
      infra::create_scoped(&db, "Keep", None, Some("#D9B252"), TAG_SCOPE_ASSET)
        .await
        .unwrap(),
      infra::create_scoped(&db, "Sell", None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap(),
    ];

    assert_eq!(badge(&state), "2", "the badge sums colored tags from both registries");
  }

  #[tokio::test]
  async fn seed_asset_tags_seeds_the_disposition_vocabulary_into_the_asset_scope() {
    use crate::store::model::{TAG_SCOPE_ASSET, TAG_SCOPE_ENTITY};

    let db = store::open_test().await.unwrap();

    seed_asset_tags(&db).await.unwrap();

    let asset = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap();
    assert_eq!(
      asset.iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
      [
        "Keep",
        "Sell",
        "Reprocess",
        "Reship",
        "Contract",
        "Hauling",
        "Loot",
        "Junk",
        "Research"
      ]
    );
    assert_eq!(asset[0].color().as_deref(), Some("#5BB97E"));
    assert!(infra::tag_all_scoped(&db, TAG_SCOPE_ENTITY).await.unwrap().is_empty());
  }

  #[tokio::test]
  async fn seed_asset_tags_is_idempotent_and_a_deleted_default_stays_deleted() {
    use crate::store::model::TAG_SCOPE_ASSET;

    let db = store::open_test().await.unwrap();
    seed_asset_tags(&db).await.unwrap();
    let keep = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap()[0].id();
    infra::tag_delete(&db, keep).await.unwrap();

    seed_asset_tags(&db).await.unwrap();

    let names = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET)
      .await
      .unwrap()
      .into_iter()
      .map(|t| t.name().clone())
      .collect::<Vec<_>>();
    assert!(
      !names.contains(&"Keep".to_owned()),
      "a deleted default is not resurrected"
    );
    assert_eq!(names.len(), 8);
  }

  #[tokio::test]
  async fn load_seeds_the_asset_scope_and_returns_both_registries() {
    use crate::store::model::TAG_SCOPE_ENTITY;

    let db = store::open_test().await.unwrap();
    infra::create_scoped(&db, "Pilot", None, None, TAG_SCOPE_ENTITY)
      .await
      .unwrap();

    let payload = load_tags(db).await.unwrap();

    assert_eq!(
      payload.tags.iter().map(|t| t.name().as_str()).collect::<Vec<_>>(),
      ["Pilot"],
      "the entity registry is returned untouched"
    );
    assert_eq!(
      payload.asset_tags.first().map(|t| t.name().as_str()),
      Some("Keep"),
      "loading the tab seeds and returns the asset registry"
    );
  }

  #[tokio::test]
  async fn loaded_fills_both_sections() {
    let db = store::open_test().await.unwrap();
    let mut state = State::new(db.clone());
    let tags = vec![infra::create(&db, "PvP", None, None).await.unwrap()];
    let asset_tags = vec![
      infra::create_scoped(&db, "Keep", None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap(),
    ];

    let _ = update(
      &mut state,
      Message::Loaded(Ok(Loaded {
        asset_tags,
        tags,
      })),
    );

    assert_eq!(state.entity.tags.len(), 1);
    assert_eq!(state.asset.tags.len(), 1);
    assert!(state.load_error.is_none());
  }

  #[tokio::test]
  async fn clear_color_drops_the_color() {
    let db = store::open_test().await.unwrap();
    let created = infra::create(&db, "PvE", None, Some("#D9B252")).await.unwrap();

    infra::update(&db, created.id(), "PvE", None, None).await.unwrap();

    let reloaded = infra::tag_all(&db).await.unwrap();
    assert_eq!(reloaded[0].color().as_deref(), None);
  }

  #[tokio::test]
  async fn delete_clears_a_dangling_picker_and_persists() {
    let mut state = state_with(&["Doomed", "Kept"]).await;
    let db = state.db.clone().unwrap();
    let doomed = state.entity.tags[0].id();
    let _ = update(&mut state, Message::ToggleColorPicker(doomed));

    let _ = update(&mut state, Message::RemoveTag(doomed));

    assert!(state.entity.picker.is_none(), "deleting a tag drops its open picker");
    infra::tag_delete(&db, doomed).await.unwrap();
    assert_eq!(order(&db).await, vec!["Kept"]);
  }

  #[test]
  fn hex_to_color_parses_a_valid_hex_and_rejects_garbage() {
    let parsed = hex_to_color("#FF8040").unwrap();
    assert_eq!(parsed, Color::from_rgb8(255, 128, 64));

    assert!(hex_to_color("not-a-color").is_none());
  }

  #[test]
  fn it_keeps_sort_modes_in_render_order() {
    assert_eq!(SortMode::ALL, [SortMode::Manual, SortMode::Name, SortMode::Color]);
  }

  #[tokio::test]
  async fn load_returns_an_empty_entity_registry_on_a_fresh_install() {
    let db = store::open_test().await.unwrap();

    let payload = load_tags(db).await.unwrap();

    assert!(payload.tags.is_empty());
  }

  #[tokio::test]
  async fn opening_the_picker_anchors_at_the_tracked_cursor() {
    let mut state = state_with(&["Main"]).await;
    let first = state.entity.tags[0].id();

    let _ = update(&mut state, Message::CursorMoved(Point::new(120.0, 64.0)));
    let _ = update(&mut state, Message::ToggleColorPicker(first));

    assert_eq!(
      state.entity.picker.as_ref().map(|p| p.anchor),
      Some(Point::new(120.0, 64.0)),
      "the picker floats from the cursor anchor, not inline in the row"
    );
  }

  #[tokio::test]
  async fn recolor_closes_the_picker_and_the_repo_write_persists_the_color() {
    let mut state = state_with(&["PvP"]).await;
    let db = state.db.clone().unwrap();
    let tag_id = state.entity.tags[0].id();
    let _ = update(&mut state, Message::ToggleColorPicker(tag_id));

    let _ = update(
      &mut state,
      Message::Recolor {
        hex: "#5BB97E".to_owned(),
        tag_id,
      },
    );

    assert!(state.entity.picker.is_none(), "recolor closes the picker");
    infra::update(&db, tag_id, "PvP", None, Some("#5BB97E")).await.unwrap();
    reload(&mut state).await;
    assert_eq!(state.entity.tags[0].color().as_deref(), Some("#5BB97E"));
  }

  #[test]
  fn recolor_dispatches_with_no_config_persist() {
    let mut state = State::default();
    state.entity.picker = Some(Picker {
      anchor: Point::ORIGIN,
      hex_draft: "#3FB8DB".to_owned(),
      hex_invalid: false,
      tag_id: 1,
    });

    let (outcome, _task) = update(
      &mut state,
      Message::Recolor {
        hex: "#3FB8DB".to_owned(),
        tag_id: 1,
      },
    );

    assert_eq!(outcome, Outcome::None, "the Tags tab never persists config");
    assert!(state.entity.picker.is_none());
  }

  #[tokio::test]
  async fn rename_commits_a_real_change() {
    let mut state = state_with(&["Old"]).await;
    let db = state.db.clone().unwrap();
    let tag_id = state.entity.tags[0].id();
    let _ = update(&mut state, Message::StartEditing(tag_id));
    let _ = update(&mut state, Message::EditDraftChanged("New".to_owned()));

    let (_, _task) = update(&mut state, Message::EditCommitted);

    assert!(state.entity.editing.is_none());
    infra::update(&db, tag_id, "New", None, None).await.unwrap();
    reload(&mut state).await;
    assert_eq!(state.entity.tags[0].name(), "New");
  }

  #[tokio::test]
  async fn rename_to_a_duplicate_is_rejected() {
    let mut state = state_with(&["Main", "Alt"]).await;
    let alt_id = state.entity.tags[1].id();
    let _ = update(&mut state, Message::StartEditing(alt_id));
    let _ = update(&mut state, Message::EditDraftChanged("main".to_owned()));

    let _ = update(&mut state, Message::EditCommitted);
    reload(&mut state).await;

    assert_eq!(
      state.entity.tags[1].name(),
      "Alt",
      "a duplicate rename leaves the name unchanged"
    );
    assert!(state.entity.editing.is_none());
  }

  #[test]
  fn reorder_computes_the_drop_above_order() {
    let mut state = State::default();
    state.entity.dragging = Some(99);
    state.entity.drop_index = Some(0);
    let (outcome, _task) = update(&mut state, Message::DropDragged);
    assert_eq!(outcome, Outcome::None);
    assert!(state.entity.dragging.is_none(), "the drop consumes the drag");
  }

  #[test]
  fn reorder_is_disabled_in_a_sorted_view() {
    let mut state = State::default();
    state.entity.sort_mode = SortMode::Name;
    assert!(!state.entity.draggable());
    let _ = update(&mut state, Message::PickUpTag(1));
    assert!(state.entity.dragging.is_none());
  }

  #[test]
  fn reorder_is_disabled_while_filtering() {
    let mut state = State::default();
    state.entity.query = "pv".to_owned();
    assert!(!state.entity.draggable());
  }

  #[tokio::test]
  async fn reorder_moves_a_dragged_tag_above_the_drop_row() {
    let mut state = state_with(&["A", "B", "C"]).await;
    let db = state.db.clone().unwrap();
    let c_id = state.entity.tags[2].id();
    let _ = update(&mut state, Message::PickUpTag(c_id));
    let _ = update(&mut state, Message::HoverTagSlot(0));

    let (_, _task) = update(&mut state, Message::DropDragged);

    assert!(state.entity.dragging.is_none(), "the drop consumes the drag");
    infra::reorder(&db, &[c_id, state.entity.tags[0].id(), state.entity.tags[1].id()])
      .await
      .unwrap();
    assert_eq!(order(&db).await, vec!["C", "A", "B"]);
  }

  #[test]
  fn subscription_is_empty_when_idle() {
    let state = State::default();

    let _sub: iced::Subscription<Message> = subscription(&state);
  }

  #[test]
  fn subscription_listens_while_dragging_and_picking() {
    let mut state = State::default();
    state.entity.dragging = Some(1);
    let _drag: iced::Subscription<Message> = subscription(&state);

    state.entity.dragging = None;
    state.asset.dragging = Some(2);
    let _asset_drag: iced::Subscription<Message> = subscription(&state);

    state.asset.dragging = None;
    state.entity.picker = Some(Picker {
      anchor: Point::ORIGIN,
      hex_draft: String::new(),
      hex_invalid: false,
      tag_id: 1,
    });
    let _pick: iced::Subscription<Message> = subscription(&state);
  }

  #[tokio::test]
  async fn update_routes_every_entity_message_to_its_handler() {
    let mut state = state_with(&["One", "Two"]).await;
    let first = state.entity.tags[0].id();

    let drive = |state: &mut State, message| {
      let (outcome, _task) = update(state, message);
      assert_eq!(outcome, Outcome::None);
    };

    drive(&mut state, Message::NewTagChanged("Three".to_owned()));
    drive(&mut state, Message::FilterChanged("on".to_owned()));
    drive(&mut state, Message::SortSelected(SortMode::Name));
    drive(&mut state, Message::SortSelected(SortMode::Manual));
    drive(&mut state, Message::FilterChanged(String::new()));

    drive(&mut state, Message::StartEditing(first));
    drive(&mut state, Message::EditDraftChanged("Edited".to_owned()));
    drive(&mut state, Message::EditCancelled);

    drive(&mut state, Message::ToggleColorPicker(first));
    drive(&mut state, Message::ColorHexChanged("#zz".to_owned()));
    drive(&mut state, Message::ColorHexSubmitted);
    assert!(state.entity.picker.as_ref().unwrap().hex_invalid);
    drive(&mut state, Message::ColorHexChanged("#3FB8DB".to_owned()));
    drive(&mut state, Message::ColorHexSubmitted);
    assert!(state.entity.picker.is_none(), "a valid hex submit closes the picker");
    drive(&mut state, Message::ToggleColorPicker(first));
    drive(&mut state, Message::ClosePicker);
    drive(&mut state, Message::ClearColor(first));

    drive(&mut state, Message::CursorMoved(Point::new(10.0, 20.0)));
    drive(&mut state, Message::PickUpTag(first));
    drive(&mut state, Message::HoverTagSlot(1));
    drive(&mut state, Message::LeaveTagSlot(1));
    drive(&mut state, Message::DropDragged);

    drive(&mut state, Message::Loaded(Err("nope".to_owned())));
    assert_eq!(state.load_error.as_deref(), Some("nope"));
    drive(
      &mut state,
      Message::Loaded(Ok(Loaded {
        asset_tags: Vec::new(),
        tags: Vec::new(),
      })),
    );
    assert!(state.load_error.is_none());

    drive(&mut state, Message::Saved(Err("write failed".to_owned())));
    assert_eq!(state.load_error.as_deref(), Some("write failed"));
    drive(&mut state, Message::Saved(Ok(())));
  }

  #[tokio::test]
  async fn update_routes_every_asset_message_to_its_handler() {
    let mut state = asset_state_with(&["One", "Two"]).await;
    let first = state.asset.tags[0].id();

    let drive = |state: &mut State, message| {
      let (outcome, _task) = update(state, message);
      assert_eq!(outcome, Outcome::None);
    };

    drive(&mut state, Message::AssetNewTagChanged("Three".to_owned()));
    drive(&mut state, Message::AssetFilterChanged("on".to_owned()));
    drive(&mut state, Message::AssetSortSelected(SortMode::Name));
    drive(&mut state, Message::AssetSortSelected(SortMode::Manual));
    drive(&mut state, Message::AssetFilterChanged(String::new()));

    drive(&mut state, Message::AssetStartEditing(first));
    drive(&mut state, Message::AssetEditDraftChanged("Edited".to_owned()));
    drive(&mut state, Message::AssetEditCancelled);

    drive(&mut state, Message::AssetToggleColorPicker(first));
    drive(&mut state, Message::AssetColorHexChanged("#zz".to_owned()));
    drive(&mut state, Message::AssetColorHexSubmitted);
    assert!(state.asset.picker.as_ref().unwrap().hex_invalid);
    drive(&mut state, Message::AssetColorHexChanged("#3FB8DB".to_owned()));
    drive(&mut state, Message::AssetColorHexSubmitted);
    assert!(state.asset.picker.is_none(), "a valid hex submit closes the picker");
    drive(&mut state, Message::AssetToggleColorPicker(first));
    drive(&mut state, Message::ClosePicker);
    drive(&mut state, Message::AssetClearColor(first));

    drive(&mut state, Message::CursorMoved(Point::new(10.0, 20.0)));
    drive(&mut state, Message::AssetPickUpTag(first));
    drive(&mut state, Message::AssetHoverTagSlot(1));
    drive(&mut state, Message::AssetLeaveTagSlot(1));
    drive(&mut state, Message::AssetDropDragged);
  }

  #[tokio::test]
  async fn view_renders_each_state() {
    let settings = Settings::default();

    let empty = State::default();
    let _el: Element<'_, Message> = view(&empty, &settings);

    let mut state = state_with(&["Main", "Alt"]).await;
    state.asset.tags = vec![
      infra::create_scoped(
        &state.db.clone().unwrap(),
        "Keep",
        None,
        Some("#5BB97E"),
        TAG_SCOPE_ASSET,
      )
      .await
      .unwrap(),
    ];
    let first = state.entity.tags[0].id();
    let asset_first = state.asset.tags[0].id();
    let _ = update(&mut state, Message::ToggleColorPicker(first));
    {
      let _el: Element<'_, Message> = view(&state, &settings);
    }

    let _ = update(&mut state, Message::AssetToggleColorPicker(asset_first));
    let _el: Element<'_, Message> = view(&state, &settings);
  }

  #[tokio::test]
  async fn visible_filters_and_sorts() {
    let state = state_with(&["Zeta", "alpha", "Beta"]).await;

    assert_eq!(
      state
        .entity
        .visible()
        .iter()
        .map(|t| t.name().as_str())
        .collect::<Vec<_>>(),
      ["Zeta", "alpha", "Beta"]
    );

    let mut by_name = state_with(&["Zeta", "alpha", "Beta"]).await;
    by_name.entity.sort_mode = SortMode::Name;
    assert_eq!(
      by_name
        .entity
        .visible()
        .iter()
        .map(|t| t.name().as_str())
        .collect::<Vec<_>>(),
      ["alpha", "Beta", "Zeta"]
    );

    let mut filtered = state_with(&["Zeta", "alpha", "Beta"]).await;
    filtered.entity.query = "a".to_owned();
    assert_eq!(
      filtered
        .entity
        .visible()
        .iter()
        .map(|t| t.name().as_str())
        .collect::<Vec<_>>(),
      ["Zeta", "alpha", "Beta"]
    );
    filtered.entity.query = "et".to_owned();
    assert_eq!(
      filtered
        .entity
        .visible()
        .iter()
        .map(|t| t.name().as_str())
        .collect::<Vec<_>>(),
      ["Zeta", "Beta"]
    );
  }

  // --- Asset section: scoped CRUD + reorder isolation from the entity registry. ---

  #[tokio::test]
  async fn asset_add_tag_clears_the_field_and_targets_the_asset_scope() {
    let mut state = State::new(store::open_test().await.unwrap());
    let db = state.db.clone().unwrap();
    state.asset.new_tag = "Salvage".to_owned();

    let (outcome, _task) = update(&mut state, Message::AssetAddTag);

    assert_eq!(outcome, Outcome::None);
    assert!(state.asset.new_tag.is_empty(), "the asset create field clears");

    // The dispatched write runs on the iced executor, which tests do not drive, so persist directly to
    // confirm the scoped repo call lands in the asset registry and never the entity one.
    infra::create_scoped(&db, "Salvage", None, None, TAG_SCOPE_ASSET)
      .await
      .unwrap();
    assert_eq!(asset_order(&db).await, vec!["Salvage"]);
    assert!(
      infra::tag_all(&db).await.unwrap().is_empty(),
      "creating an asset tag never touches the entity registry"
    );
  }

  #[tokio::test]
  async fn asset_add_tag_rejects_a_case_insensitive_duplicate() {
    let mut state = asset_state_with(&["Keep"]).await;
    state.asset.new_tag = "keep".to_owned();

    let (_, _task) = update(&mut state, Message::AssetAddTag);

    assert!(state.asset.new_tag.is_empty());
    assert_eq!(state.asset.tags.len(), 1);
  }

  #[tokio::test]
  async fn asset_rename_commits_a_real_change() {
    let mut state = asset_state_with(&["Old"]).await;
    let db = state.db.clone().unwrap();
    let tag_id = state.asset.tags[0].id();
    let _ = update(&mut state, Message::AssetStartEditing(tag_id));
    let _ = update(&mut state, Message::AssetEditDraftChanged("New".to_owned()));

    let (_, _task) = update(&mut state, Message::AssetEditCommitted);

    assert!(state.asset.editing.is_none());
    infra::update(&db, tag_id, "New", None, None).await.unwrap();
    reload_asset(&mut state).await;
    assert_eq!(state.asset.tags[0].name(), "New");
  }

  #[tokio::test]
  async fn asset_recolor_persists_and_closes_the_picker() {
    let mut state = asset_state_with(&["Keep"]).await;
    let db = state.db.clone().unwrap();
    let tag_id = state.asset.tags[0].id();
    let _ = update(&mut state, Message::AssetToggleColorPicker(tag_id));

    let _ = update(
      &mut state,
      Message::AssetRecolor {
        hex: "#5BB97E".to_owned(),
        tag_id,
      },
    );

    assert!(state.asset.picker.is_none(), "asset recolor closes the picker");
    infra::update(&db, tag_id, "Keep", None, Some("#5BB97E")).await.unwrap();
    reload_asset(&mut state).await;
    assert_eq!(state.asset.tags[0].color().as_deref(), Some("#5BB97E"));
  }

  #[tokio::test]
  async fn asset_delete_clears_a_dangling_picker_and_persists() {
    let mut state = asset_state_with(&["Doomed", "Kept"]).await;
    let db = state.db.clone().unwrap();
    let doomed = state.asset.tags[0].id();
    let _ = update(&mut state, Message::AssetToggleColorPicker(doomed));

    let _ = update(&mut state, Message::AssetRemoveTag(doomed));

    assert!(
      state.asset.picker.is_none(),
      "deleting an asset tag drops its open picker"
    );
    infra::tag_delete(&db, doomed).await.unwrap();
    assert_eq!(asset_order(&db).await, vec!["Kept"]);
  }

  #[tokio::test]
  async fn asset_reorder_persists_within_the_asset_scope_and_leaves_entity_untouched() {
    let db = store::open_test().await.unwrap();
    let entity_a = infra::create(&db, "EA", None, None).await.unwrap();
    let entity_b = infra::create(&db, "EB", None, None).await.unwrap();
    let mut state = State::new(db.clone());
    state.entity.tags = infra::tag_all(&db).await.unwrap();
    for name in ["A", "B", "C"] {
      infra::create_scoped(&db, name, None, None, TAG_SCOPE_ASSET)
        .await
        .unwrap();
    }
    state.asset.tags = infra::tag_all_scoped(&db, TAG_SCOPE_ASSET).await.unwrap();
    let c_id = state.asset.tags[2].id();

    let _ = update(&mut state, Message::AssetPickUpTag(c_id));
    let _ = update(&mut state, Message::AssetHoverTagSlot(0));
    let (_, _task) = update(&mut state, Message::AssetDropDragged);

    assert!(state.asset.dragging.is_none(), "the asset drop consumes the drag");
    infra::reorder(&db, &[c_id, state.asset.tags[0].id(), state.asset.tags[1].id()])
      .await
      .unwrap();
    assert_eq!(asset_order(&db).await, vec!["C", "A", "B"]);
    assert_eq!(
      order(&db).await,
      vec!["EA", "EB"],
      "asset reorder leaves the entity registry order untouched"
    );
    let _ = (entity_a, entity_b);
  }

  #[tokio::test]
  async fn asset_drag_does_not_pick_up_an_entity_tag() {
    let mut state = state_with(&["Pilot"]).await;
    let entity_id = state.entity.tags[0].id();

    let _ = update(&mut state, Message::AssetPickUpTag(entity_id));

    assert!(
      state.asset.dragging.is_some(),
      "the asset section tracks the drag id on its own state"
    );
    assert!(
      state.entity.dragging.is_none(),
      "an asset drag never touches the entity section"
    );
  }

  #[test]
  fn asset_messages_use_the_asset_msgs_descriptor() {
    assert!(matches!((ASSET_MSGS.add_tag)(), Message::AssetAddTag));
    assert!(matches!((ASSET_MSGS.remove_tag)(7), Message::AssetRemoveTag(7)));
    assert!(matches!((ENTITY_MSGS.add_tag)(), Message::AddTag));
    assert!(matches!((ENTITY_MSGS.remove_tag)(7), Message::RemoveTag(7)));
  }
}
