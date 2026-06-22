pub mod compose;
mod draft;
mod folder_pane;
mod labels;
mod loaders;
mod markup;
mod message_list;
mod outbox_indicator;
mod read_state;
mod reading_pane;
mod shell;
mod snooze;
mod switcher;
mod triage;

use std::collections::HashMap;

use iced::{Element, Point, Task, widget::text_editor};

pub use self::loaders::{FolderPaneData, OutboxIndicator, RosterPilot};
use self::{
  labels::LabelDraft,
  loaders::{FolderLabel, MessageLabel},
  message_list::MessageRow,
};
use crate::{
  store::{
    Database, images,
    model::{
      CharacterMail,
      character_mail_view::{MailRender, UnifiedMail},
      mail_overlay_state::MailOverlayState,
    },
    repo::mail,
  },
  ui::{
    components::{
      entity_search::EntityRef,
      resizable_pane::{self, PaneDrag},
    },
    load_epoch::LoadEpoch,
  },
  window_state::{self, UiState},
};

pub const FOLDER_PANE_KEY: &str = "mail.folder";

pub const MESSAGE_LIST_PANE_KEY: &str = "mail.message_list";

const FOLDER_PANE_DEFAULT_WIDTH: f32 = 240.0;

const MESSAGE_LIST_PANE_DEFAULT_WIDTH: f32 = 380.0;

const FOLDER_PANE_MIN_WIDTH: f32 = 80.0;

const MESSAGE_LIST_PANE_MIN_WIDTH: f32 = resizable_pane::MIN_PANE_WIDTH;

pub const EMPTY_MAIL_SELECTION: i64 = 0;

pub const RECIPIENT_SEARCH_MIN_CHARS: usize = 3;

/// Load more once the viewport scrolls within this fraction of the bottom.
const LIST_SCROLL_THRESHOLD: f32 = 0.85;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Folder {
  Label(i64),
  Standard(StandardFolder),
  #[default]
  Unified,
}

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  drafts: Vec<draft::DraftRow>,
  folder: Folder,
  folder_data: FolderPaneData,
  folder_pane_width: f32,
  headers: Vec<CharacterMail>,
  message_list_pane_width: f32,
  messages: Vec<MessageRow>,
  messages_has_more: bool,
  outbox_indicator: OutboxIndicator,
  overlays: HashMap<i64, MailOverlayState>,
  roster: Vec<RosterPilot>,
  scope: Scope,
  unified: Vec<UnifiedMail>,
  unified_unread: i64,
}

#[derive(Clone, Debug)]
pub enum Message {
  Archive(i64),
  ComposeBodyChanged(text_editor::Action),
  ComposeBold,
  ComposeCcCommitted,
  ComposeCcInput(String),
  ComposeCcPicked(EntityRef),
  ComposeCcRemoved(usize),
  ComposeCcSearched {
    generation: u64,
    results: Vec<EntityRef>,
  },
  ComposeCcShown,
  ComposeDiscarded,
  ComposeFromChanged(i64),
  ComposeFromToggled,
  ComposeItalic,
  ComposeLinkInsert,
  ComposeLinkKindSelected(compose::LinkKind),
  ComposeLinkPicked(EntityRef),
  ComposeLinkSearchInput(String),
  ComposeLinkSearched {
    generation: u64,
    results: Vec<EntityRef>,
  },
  ComposeLinkToggled,
  ComposeLinkUrlChanged(String),
  ComposeOpened,
  ComposeSend,
  ComposeSent(Result<(), String>),
  ComposeSubjectChanged(String),
  ComposeToCommitted,
  ComposeToInput(String),
  ComposeToPicked(EntityRef),
  ComposeToRemoved(usize),
  ComposeToSearched {
    generation: u64,
    results: Vec<EntityRef>,
  },
  Delete(i64),
  DraftDeleted(i64),
  DraftLoaded(Box<Option<crate::store::model::MailDraft>>),
  DraftOpened(i64),
  DraftRowsLoaded(Vec<draft::DraftRow>),
  /// An auto-save finished; the row id is threaded back onto the still-open compose so the next
  /// save updates the same row and a send deletes it by id.
  DraftSaved(Option<i64>),
  DropTargetEntered(DropTarget),
  DropTargetLeft(DropTarget),
  FolderPaneDragEnd,
  FolderPaneDragStart,
  FolderPaneDragged(f32),
  FolderSelected(Folder),
  Forward(i64),
  LabelColorPicked(String),
  LabelDeleteCancelled,
  LabelDeleteConfirmed,
  LabelDeleteRequested(i64),
  LabelDragMoved(Point),
  LabelDropReleased,
  LabelModalClosed,
  LabelModalOpened,
  LabelModalSubmitted,
  LabelNameChanged(String),
  LabelPickerClosed,
  LabelPickerOpened(i64),
  LabelRowMenuOpened(i64),
  LabelToggled(i64, i64),
  LabelsWritten,
  ListPaneDragEnd,
  ListPaneDragStart,
  ListPaneDragged(f32),
  /// The message list scrolled. `relative` (0.0–1.0) drives the load-more threshold;
  /// `absolute` is the pixel offset stored to window the virtual list.
  ListScrolled {
    absolute: f32,
    relative: f32,
  },
  Loaded(Box<Loaded>),
  MarkedRead,
  /// One more keyset page of the listing finished loading.
  MessagesPageLoaded {
    epoch: u64,
    rows: Vec<MessageRow>,
  },
  OutboxDismiss(i64),
  OutboxRefreshed(Box<OutboxIndicator>),
  OutboxRetry(i64),
  OverlayWritten,
  PaneSettled(&'static str, f32),
  PickerToggled,
  ReauthRequested(i64),
  RenderLoaded {
    mail_id: i64,
    render: Box<Option<ReadingRender>>,
  },
  Reply(i64),
  ReplyAll(i64),
  ScopeSelected(Scope),
  SearchChanged(String),
  /// One more keyset page of search results finished loading, tagged with the query
  /// it was issued for so a stale page from a superseded query can be dropped.
  SearchPageLoaded {
    query: String,
    rows: Vec<MessageRow>,
  },
  Selected(i64),
  SnoozeCalendarBack,
  // Constructed only by handler-routing tests; the set-time arm is wired but not yet triggered from the UI.
  #[allow(dead_code)]
  SnoozeCalendarChip(u32, u32),
  SnoozeCalendarConfirmed,
  SnoozeCalendarDaySelected(i32, u32, u32),
  SnoozeCalendarHourDown,
  SnoozeCalendarHourUp,
  SnoozeCalendarMinuteDown,
  SnoozeCalendarMinuteUp,
  SnoozeCalendarNextMonth,
  SnoozeCalendarOpened,
  SnoozeCalendarPrevMonth,
  SnoozeMenuToggled,
  SnoozePreset(snooze::Preset),
  ToggleStar(i64),
  Trash(i64),
  Unsnooze(i64),
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows (roster portraits, mail sender portraits),
  /// so the shell should recheck for stale images. Interaction-only messages return `false` to keep the staleness
  /// scan off the per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(
      self,
      Message::Loaded(_)
        | Message::MessagesPageLoaded { .. }
        | Message::RenderLoaded { .. }
        | Message::SearchPageLoaded { .. }
    )
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadingRender {
  is_starred: bool,
  labels: Vec<MessageLabel>,
  mail: MailRender,
  sender_portrait: images::ImageState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
  Character(i64),
}

impl Default for Scope {
  fn default() -> Self {
    Scope::Character(EMPTY_MAIL_SELECTION)
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardFolder {
  Archive,
  Drafts,
  Inbox,
  Sent,
  Snoozed,
  Starred,
  Trash,
}

/// Where a dragged message row can be dropped: onto one of the standard boxes (a pure local move)
/// or onto a custom label (the existing tag behaviour).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropTarget {
  Label(i64),
  StandardFolder(StandardFolder),
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  all_messages: Vec<MessageRow>,
  cursor: Option<Point>,
  drafts: Vec<draft::DraftRow>,
  dragging_mail: Option<i64>,
  drop_target: Option<DropTarget>,
  folder: Folder,
  folder_data: FolderPaneData,
  folder_pane: PaneDrag,
  headers: Vec<CharacterMail>,
  label_modal: Option<LabelDraft>,
  label_picker: Option<LabelPicker>,
  list_scroll_offset: f32,
  message_list_pane: PaneDrag,
  messages: Vec<MessageRow>,
  messages_cursor: Option<mail::MailCursor>,
  messages_has_more: bool,
  messages_loading: bool,
  messages_page_epoch: LoadEpoch,
  outbox_indicator: OutboxIndicator,
  overlays: HashMap<i64, MailOverlayState>,
  pending_label_delete: Option<i64>,
  picker_open: bool,
  render: Option<ReadingRender>,
  roster: Vec<RosterPilot>,
  search: String,
  search_cursor: Option<mail::MailCursor>,
  search_has_more: bool,
  search_loading: bool,
  selected: Option<i64>,
  snooze_calendar: Option<snooze::Calendar>,
  snooze_menu: SnoozeMenu,
  unified: Vec<UnifiedMail>,
  unified_unread: i64,
}

impl State {
  pub fn new(active: i64) -> Self {
    State {
      active: Scope::Character(active),
      all_messages: Vec::new(),
      cursor: None,
      drafts: Vec::new(),
      dragging_mail: None,
      drop_target: None,
      folder: Folder::default(),
      folder_data: FolderPaneData::default(),
      folder_pane: PaneDrag::with_min_width(
        FOLDER_PANE_DEFAULT_WIDTH,
        FOLDER_PANE_MIN_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      ),
      headers: Vec::new(),
      label_modal: None,
      label_picker: None,
      list_scroll_offset: 0.0,
      message_list_pane: PaneDrag::with_min_width(
        MESSAGE_LIST_PANE_DEFAULT_WIDTH,
        MESSAGE_LIST_PANE_MIN_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      ),
      messages: Vec::new(),
      messages_cursor: None,
      messages_has_more: false,
      messages_loading: false,
      messages_page_epoch: LoadEpoch::default(),
      outbox_indicator: OutboxIndicator::default(),
      overlays: HashMap::new(),
      pending_label_delete: None,
      picker_open: false,
      render: None,
      roster: Vec::new(),
      search: String::new(),
      search_cursor: None,
      search_has_more: false,
      search_loading: false,
      selected: None,
      snooze_calendar: None,
      snooze_menu: SnoozeMenu::Closed,
      unified: Vec::new(),
      unified_unread: 0,
    }
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("main", crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.folder_pane = restore_pane(
      ui,
      FOLDER_PANE_KEY,
      FOLDER_PANE_DEFAULT_WIDTH,
      FOLDER_PANE_MIN_WIDTH,
      host_width,
    );
    self.message_list_pane = restore_pane(
      ui,
      MESSAGE_LIST_PANE_KEY,
      MESSAGE_LIST_PANE_DEFAULT_WIDTH,
      MESSAGE_LIST_PANE_MIN_WIDTH,
      host_width,
    );
    self
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.folder_pane.set_host_width(host_width);
    self.message_list_pane.set_host_width(host_width);
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    let roster = self.roster.iter().map(|pilot| &pilot.portrait);
    let messages = self.messages.iter().map(|row| &row.sender_portrait);
    let all_messages = self.all_messages.iter().map(|row| &row.sender_portrait);
    let render = self.render.iter().map(|render| &render.sender_portrait);

    roster
      .chain(messages)
      .chain(all_messages)
      .chain(render)
      .filter_map(images::ImageState::stale_key)
      .filter(|(_, id)| *id > 0)
      .collect()
  }

  pub fn roster(&self) -> &[RosterPilot] {
    &self.roster
  }

  pub(super) fn scope_gate(&self) -> Option<(i64, &str, Vec<&'static str>)> {
    let Scope::Character(id) = self.active;
    let pilot = self.roster.iter().find(|pilot| pilot.id == id)?;
    let required = crate::features::registry::descriptor(crate::config::Feature::Mail).scopes;
    let missing = crate::ui::components::forbidden::missing_scopes(pilot.granted_scopes.as_deref(), required);
    if missing.is_empty() {
      return None;
    }
    Some((id, pilot.name.as_str(), missing))
  }

  pub fn unified_unread(&self) -> i64 {
    self.unified_unread
  }

  pub fn picker_open(&self) -> bool {
    self.picker_open
  }

  pub fn folder(&self) -> Folder {
    self.folder
  }

  pub fn folder_data(&self) -> &FolderPaneData {
    &self.folder_data
  }

  pub fn selected(&self) -> Option<i64> {
    self.selected
  }

  pub fn render(&self) -> Option<&ReadingRender> {
    self.render.as_ref()
  }

  pub fn search(&self) -> &str {
    &self.search
  }

  pub fn messages(&self) -> &[MessageRow] {
    &self.messages
  }

  pub(super) fn drafts(&self) -> &[draft::DraftRow] {
    &self.drafts
  }

  /// The default sender for a new compose window opened from this mail view (the active character).
  pub fn default_from(&self) -> Option<i64> {
    let Scope::Character(id) = self.active;
    Some(id)
  }

  /// Builds the seed for a reply/reply-all/forward compose window from the currently-rendered mail.
  /// `None` when no mail is open or the open render is for a different mail than `mail_id`.
  pub fn reply_seed(&self, mail_id: i64, kind: compose::Kind) -> Option<compose::Seed> {
    let render = self.render.as_ref()?;
    if render.mail.header.mail_id() != mail_id {
      return None;
    }
    Some(compose::Seed::Reply {
      kind,
      render: Box::new(render.mail.clone()),
    })
  }

  pub fn list_scroll_offset(&self) -> f32 {
    self.list_scroll_offset
  }

  pub fn all_messages(&self) -> &[MessageRow] {
    &self.all_messages
  }

  pub fn outbox_indicator(&self) -> &OutboxIndicator {
    &self.outbox_indicator
  }

  pub fn folder_pane_width(&self) -> f32 {
    self.folder_pane.width()
  }

  pub fn message_list_pane_width(&self) -> f32 {
    self.message_list_pane.width()
  }

  pub fn snooze_presets_open(&self) -> bool {
    self.snooze_menu == SnoozeMenu::Presets
  }

  pub fn snooze_calendar(&self) -> Option<&snooze::Calendar> {
    if self.snooze_menu == SnoozeMenu::Calendar {
      self.snooze_calendar.as_ref()
    } else {
      None
    }
  }

  pub fn open_mail_snoozed(&self) -> bool {
    self
      .selected
      .and_then(|mail_id| self.overlay_for(mail_id))
      .map(|o| o.is_snoozed())
      .unwrap_or(false)
  }

  pub(super) fn label_modal(&self) -> Option<&LabelDraft> {
    self.label_modal.as_ref()
  }

  pub(super) fn pending_label_delete(&self) -> Option<&FolderLabel> {
    let label_id = self.pending_label_delete?;
    self.folder_data.labels.iter().find(|label| label.label_id == label_id)
  }

  pub(super) fn dragging_mail(&self) -> Option<i64> {
    self.dragging_mail
  }

  pub(super) fn drop_target(&self) -> Option<DropTarget> {
    self.drop_target
  }

  pub(super) fn label_picker_view(&self) -> Option<(i64, Option<Point>, Vec<i64>)> {
    let picker = self.label_picker?;
    Some((picker.mail_id, picker.anchor, self.applied_label_ids(picker.mail_id)))
  }

  fn applied_label_ids(&self, mail_id: i64) -> Vec<i64> {
    if let Some(render) = self.render.as_ref()
      && render.mail.header.mail_id() == mail_id
    {
      return render.mail.label_ids.clone();
    }
    self
      .messages
      .iter()
      .chain(self.all_messages.iter())
      .find(|row| row.mail_id == mail_id)
      .map(|row| row.label_ids.clone())
      .unwrap_or_default()
  }

  fn character_for(&self, mail_id: i64) -> Option<i64> {
    if let Some(render) = self.render.as_ref()
      && render.mail.header.mail_id() == mail_id
    {
      return Some(render.mail.header.character_id());
    }
    if let Some(row) = self.messages.iter().find(|r| r.mail_id == mail_id) {
      return Some(row.character_id);
    }
    let Scope::Character(id) = self.active;
    Some(id)
  }

  fn overlay_for(&self, mail_id: i64) -> Option<&MailOverlayState> {
    self.overlays.get(&mail_id)
  }
}

impl Default for State {
  fn default() -> Self {
    Self::new(EMPTY_MAIL_SELECTION)
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LabelPicker {
  anchor: Option<Point>,
  mail_id: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SnoozeMenu {
  Calendar,
  #[default]
  Closed,
  Presets,
}

fn restore_pane(ui: &UiState, key: &str, default: f32, min: f32, host_width: f32) -> PaneDrag {
  PaneDrag::from_store_with_min(ui, key, default, min, host_width)
}

/// Persists a compose window's pending save (built from its [`compose::Draft`]) before the app exits,
/// so a draft in flight at quit is present in Drafts on next launch. Awaited by the app's shutdown
/// sequence rather than dispatched as a message, since the UI is tearing down.
pub async fn persist_pending_draft(db: Database, id: Option<i64>, input: mail::DraftInput) {
  let _ = draft::persist(db, id, input).await;
}

/// Deletes a persisted draft row by id, used by the app when a compose window sends successfully.
pub async fn delete_draft(db: Database, id: i64) {
  draft::delete(db, id).await;
}

pub fn load(db: &Database, character: i64) -> Task<Message> {
  Task::perform(
    load_mail(db.clone(), Scope::Character(character), Folder::Unified),
    |loaded| Message::Loaded(Box::new(loaded)),
  )
}

pub fn reload(db: &Database, scope: Scope) -> Task<Message> {
  Task::perform(load_mail(db.clone(), scope, Folder::Unified), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

async fn load_render(db: Database, character_id: i64, mail_id: i64) -> Option<ReadingRender> {
  let mail = mail::mail(&db, character_id, mail_id).await.ok().flatten()?;
  let is_starred = mail::overlay_state(&db, character_id, mail_id)
    .await
    .map(|overlay| overlay.is_starred)
    .unwrap_or(false);
  let sender_portrait = loaders::resolve_sender_portrait(mail.header.from_id());
  let labels = loaders::resolve_message_labels(&db, character_id, &mail.label_ids).await;
  Some(ReadingRender {
    is_starred,
    labels,
    mail,
    sender_portrait,
  })
}

fn reload_for(db: &Database, scope: Scope, folder: Folder) -> Task<Message> {
  Task::perform(load_mail(db.clone(), scope, folder), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

/// Build a keyset cursor at a loaded row's position.
fn cursor_of(row: &MessageRow) -> mail::MailCursor {
  mail::MailCursor::new(row.timestamp.clone(), row.mail_id)
}

/// Kick the first page of a search. Results stream into `all_messages`, the
/// search-results accumulator that [`message_list::pane`] renders while a query is
/// active.
fn start_search(state: &mut State, db: &Database) -> Task<Message> {
  state.search_loading = true;
  let (db, scope, folder, needle) = (db.clone(), state.active, state.folder, state.search.clone());
  Task::perform(
    async move {
      let rows = message_list::load_search_page(&db, scope, folder, &needle, None).await;
      (needle, rows)
    },
    |(query, rows)| Message::SearchPageLoaded {
      query,
      rows,
    },
  )
}

fn update_pagination(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ListScrolled {
      absolute,
      relative,
    } => {
      state.list_scroll_offset = absolute;
      if relative < LIST_SCROLL_THRESHOLD {
        return Task::none();
      }
      if state.search.trim().is_empty() {
        load_more_messages(state, db)
      } else {
        load_more_search(state, db)
      }
    }
    Message::MessagesPageLoaded {
      epoch,
      rows,
    } => {
      // Drop a page captured against a folder/scope the user has since left, so it can't append
      // foreign rows to the current list.
      if !state.messages_page_epoch.matches(epoch) {
        return Task::none();
      }
      state.messages_loading = false;
      state.messages_has_more = rows.len() as i64 == message_list::MESSAGE_PAGE_SIZE;
      if let Some(last) = rows.last() {
        state.messages_cursor = Some(cursor_of(last));
      }
      state.messages.extend(rows);
      Task::none()
    }
    Message::SearchPageLoaded {
      query,
      rows,
    } => {
      // Drop a page whose query the user has since changed; its `all_messages`
      // accumulator was already cleared by the newer `SearchChanged`.
      if query != state.search {
        return Task::none();
      }
      state.search_loading = false;
      state.search_has_more = rows.len() as i64 == message_list::MESSAGE_PAGE_SIZE;
      if let Some(last) = rows.last() {
        state.search_cursor = Some(cursor_of(last));
      }
      state.all_messages.extend(rows);
      Task::none()
    }
    _ => Task::none(),
  }
}

fn load_more_messages(state: &mut State, db: &Database) -> Task<Message> {
  if state.messages_loading || !state.messages_has_more {
    return Task::none();
  }
  let Some(cursor) = state.messages_cursor.clone() else {
    return Task::none();
  };
  state.messages_loading = true;
  let epoch = state.messages_page_epoch.current();
  let (db, scope, folder) = (db.clone(), state.active, state.folder);
  Task::perform(
    async move { message_list::load_messages_page(&db, scope, folder, cursor).await },
    move |rows| Message::MessagesPageLoaded {
      epoch,
      rows,
    },
  )
}

fn load_more_search(state: &mut State, db: &Database) -> Task<Message> {
  if state.search_loading || !state.search_has_more {
    return Task::none();
  }
  let Some(cursor) = state.search_cursor.clone() else {
    return Task::none();
  };
  state.search_loading = true;
  let (db, scope, folder, needle) = (db.clone(), state.active, state.folder, state.search.clone());
  Task::perform(
    async move {
      let rows = message_list::load_search_page(&db, scope, folder, &needle, Some(cursor)).await;
      (needle, rows)
    },
    |(query, rows)| Message::SearchPageLoaded {
      query,
      rows,
    },
  )
}

fn triage_write<Fut>(state: &State, db: &Database, mail_id: i64, op: fn(Database, i64, i64) -> Fut) -> Task<Message>
where
  Fut: std::future::Future<Output = ()> + Send + 'static,
{
  let Some(character_id) = state.character_for(mail_id) else {
    return Task::none();
  };
  Task::perform(op(db.clone(), character_id, mail_id), |()| Message::OverlayWritten)
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Loaded(loaded) => handle_loaded(state, *loaded, db),
    Message::ScopeSelected(_)
    | Message::PickerToggled
    | Message::FolderSelected(_)
    | Message::SearchChanged(_)
    | Message::RenderLoaded {
      ..
    } => update_navigation(state, message, db),
    Message::FolderPaneDragStart
    | Message::FolderPaneDragged(_)
    | Message::FolderPaneDragEnd
    | Message::ListPaneDragStart
    | Message::ListPaneDragged(_)
    | Message::ListPaneDragEnd
    | Message::PaneSettled(..) => update_pane_drag(state, message),
    Message::ListScrolled {
      ..
    }
    | Message::MessagesPageLoaded {
      ..
    }
    | Message::SearchPageLoaded {
      ..
    } => update_pagination(state, message, db),
    Message::Selected(mail_id) => handle_message_selected(state, mail_id, db),
    Message::MarkedRead => reload_for(db, state.active, state.folder),
    Message::ToggleStar(mail_id) => triage_write(state, db, mail_id, triage::toggle_star),
    Message::Archive(mail_id) => triage_write(state, db, mail_id, triage::archive),
    Message::Delete(mail_id) => triage_write(state, db, mail_id, triage::delete),
    Message::Trash(mail_id) => triage_write(state, db, mail_id, triage::trash),
    Message::OverlayWritten => reload_for(db, state.active, state.folder),

    Message::SnoozeMenuToggled
    | Message::SnoozePreset(_)
    | Message::SnoozeCalendarOpened
    | Message::SnoozeCalendarBack
    | Message::SnoozeCalendarConfirmed
    | Message::Unsnooze(_) => update_snooze(state, message, db),
    Message::SnoozeCalendarPrevMonth
    | Message::SnoozeCalendarNextMonth
    | Message::SnoozeCalendarDaySelected(..)
    | Message::SnoozeCalendarHourUp
    | Message::SnoozeCalendarHourDown
    | Message::SnoozeCalendarMinuteUp
    | Message::SnoozeCalendarMinuteDown
    | Message::SnoozeCalendarChip(..) => update_snooze_calendar(state, message),

    // Compose now lives in detached windows: the app layer intercepts every compose/reply/forward
    // and per-window compose message before it reaches here, so these arms are unreachable in practice
    // and kept only to satisfy the match.
    Message::Reply(_)
    | Message::ReplyAll(_)
    | Message::Forward(_)
    | Message::ComposeOpened
    | Message::ComposeToInput(_)
    | Message::ComposeCcInput(_)
    | Message::ComposeToSearched {
      ..
    }
    | Message::ComposeCcSearched {
      ..
    }
    | Message::ComposeToCommitted
    | Message::ComposeCcCommitted
    | Message::ComposeToPicked(_)
    | Message::ComposeCcPicked(_)
    | Message::ComposeToRemoved(_)
    | Message::ComposeCcRemoved(_)
    | Message::ComposeCcShown
    | Message::ComposeSubjectChanged(_)
    | Message::ComposeBodyChanged(_)
    | Message::ComposeBold
    | Message::ComposeItalic
    | Message::ComposeLinkToggled
    | Message::ComposeLinkKindSelected(_)
    | Message::ComposeLinkUrlChanged(_)
    | Message::ComposeLinkSearchInput(_)
    | Message::ComposeLinkSearched {
      ..
    }
    | Message::ComposeLinkPicked(_)
    | Message::ComposeLinkInsert
    | Message::ComposeFromChanged(_)
    | Message::ComposeFromToggled
    | Message::ComposeDiscarded
    | Message::ComposeSend
    | Message::ComposeSent(_)
    | Message::DraftLoaded(_) => Task::none(),
    Message::DraftDeleted(_) | Message::DraftOpened(_) | Message::DraftRowsLoaded(_) | Message::DraftSaved(_) => {
      update_drafts(state, message, db)
    }
    Message::DropTargetEntered(_)
    | Message::DropTargetLeft(_)
    | Message::LabelColorPicked(_)
    | Message::LabelDeleteCancelled
    | Message::LabelDeleteConfirmed
    | Message::LabelDeleteRequested(_)
    | Message::LabelDragMoved(_)
    | Message::LabelDropReleased
    | Message::LabelModalClosed
    | Message::LabelModalOpened
    | Message::LabelModalSubmitted
    | Message::LabelNameChanged(_)
    | Message::LabelPickerClosed
    | Message::LabelPickerOpened(_)
    | Message::LabelRowMenuOpened(_)
    | Message::LabelToggled(..)
    | Message::LabelsWritten => update_labels(state, message, db),
    Message::OutboxRetry(_) | Message::OutboxDismiss(_) | Message::OutboxRefreshed(_) => {
      update_outbox(state, message, db)
    }
    Message::ReauthRequested(_) => Task::none(),
  }
}

fn update_navigation(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ScopeSelected(scope) => {
      // Compose lives in detached windows now, so navigating the main view never disturbs an open
      // compose and no longer auto-saves a draft mid-edit.
      state.messages_page_epoch.next();
      state.active = scope;
      state.folder = Folder::Unified;
      state.selected = None;
      state.render = None;
      state.picker_open = false;
      state.snooze_menu = SnoozeMenu::Closed;
      state.snooze_calendar = None;
      state.label_modal = None;
      state.label_picker = None;
      state.pending_label_delete = None;
      state.dragging_mail = None;
      state.drop_target = None;
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::FolderSelected(folder) => {
      state.messages_page_epoch.next();
      state.folder = folder;
      state.selected = None;
      state.render = None;
      state.snooze_menu = SnoozeMenu::Closed;
      state.snooze_calendar = None;
      state.label_picker = None;
      reload_for(db, state.active, folder)
    }
    Message::SearchChanged(query) => {
      state.search = query;
      state.list_scroll_offset = 0.0;
      state.all_messages.clear();
      state.search_cursor = None;
      state.search_has_more = false;
      state.search_loading = false;
      if state.search.trim().is_empty() {
        Task::none()
      } else {
        start_search(state, db)
      }
    }
    Message::RenderLoaded {
      mail_id,
      render,
    } => {
      // A render that completes after the selection moved on (or was cleared) belongs to a mail we are no longer
      // showing; dropping it stops a stale body from landing under the current selection.
      if state.selected == Some(mail_id) {
        state.render = *render;
      }
      Task::none()
    }
    _ => Task::none(),
  }
}

fn update_outbox(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::OutboxRetry(id) => Task::perform(read_state::retry_outbox(db.clone(), id), |indicator| {
      Message::OutboxRefreshed(Box::new(indicator))
    }),
    Message::OutboxDismiss(id) => Task::perform(read_state::dismiss_outbox(db.clone(), id), |indicator| {
      Message::OutboxRefreshed(Box::new(indicator))
    }),
    Message::OutboxRefreshed(indicator) => {
      state.outbox_indicator = *indicator;
      Task::none()
    }
    _ => Task::none(),
  }
}

fn handle_loaded(state: &mut State, loaded: Loaded, db: &Database) -> Task<Message> {
  let Loaded {
    drafts,
    folder,
    folder_data,
    folder_pane_width,
    headers,
    message_list_pane_width,
    messages,
    messages_has_more,
    outbox_indicator,
    overlays,
    roster,
    scope,
    unified,
    unified_unread,
  } = loaded;
  state.outbox_indicator = outbox_indicator;
  state.roster = roster;
  state.unified = unified;
  state.unified_unread = unified_unread;
  if !state.folder_pane.is_active() {
    state.folder_pane.set_ratio_from_store(folder_pane_width);
  }
  if !state.message_list_pane.is_active() {
    state.message_list_pane.set_ratio_from_store(message_list_pane_width);
  }
  if scope == state.active && folder == state.folder {
    // A fresh folder load supersedes any in-flight scroll page captured against the prior list.
    state.messages_page_epoch.next();
    state.drafts = drafts;
    state.folder_data = folder_data;
    state.headers = headers;
    state.overlays = overlays;
    state.messages = messages;
    state.messages_cursor = state.messages.last().map(cursor_of);
    state.messages_has_more = messages_has_more;
    state.messages_loading = false;
    state.list_scroll_offset = 0.0;
    // Archiving or trashing the open mail drops it from the reloaded folder; clear the reading pane (mirroring
    // FolderSelected) so it stops rendering a mail no longer in the list.
    if let Some(selected) = state.selected
      && !state.messages.iter().any(|row| row.mail_id == selected)
    {
      state.selected = None;
      state.render = None;
    }
    // A fresh folder load supersedes any in-flight search paging.
    state.all_messages.clear();
    state.search_cursor = None;
    state.search_has_more = false;
    state.search_loading = false;
    if state.search.trim().is_empty() {
      Task::none()
    } else {
      start_search(state, db)
    }
  } else {
    reload_for(db, state.active, state.folder)
  }
}

fn update_pane_drag(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::FolderPaneDragStart => {
      state.folder_pane.start();
      Task::none()
    }
    Message::FolderPaneDragged(x) => {
      state.folder_pane.drag_to(x);
      Task::none()
    }
    Message::FolderPaneDragEnd => {
      state.folder_pane.end();
      Task::done(Message::PaneSettled(FOLDER_PANE_KEY, state.folder_pane.ratio()))
    }
    Message::ListPaneDragStart => {
      state.message_list_pane.start();
      Task::none()
    }
    Message::ListPaneDragged(x) => {
      state.message_list_pane.drag_to(x);
      Task::none()
    }
    Message::ListPaneDragEnd => {
      state.message_list_pane.end();
      Task::done(Message::PaneSettled(
        MESSAGE_LIST_PANE_KEY,
        state.message_list_pane.ratio(),
      ))
    }
    _ => Task::none(),
  }
}

fn handle_message_selected(state: &mut State, mail_id: i64, db: &Database) -> Task<Message> {
  state.selected = Some(mail_id);
  state.snooze_menu = SnoozeMenu::Closed;
  state.snooze_calendar = None;
  state.label_picker = None;
  // A row press both selects and arms a potential drag-to-label; the drag is a no-op unless released over a label target.
  state.dragging_mail = Some(mail_id);
  state.drop_target = None;
  let Some(character_id) = state
    .messages
    .iter()
    .find(|r| r.mail_id == mail_id)
    .map(|r| r.character_id)
  else {
    state.render = None;
    return Task::none();
  };
  let render = Task::perform(load_render(db.clone(), character_id, mail_id), move |render| {
    Message::RenderLoaded {
      mail_id,
      render: Box::new(render),
    }
  });
  match read_state::open_target(state, mail_id) {
    Some((character_id, mail_id)) => Task::batch([
      render,
      Task::perform(read_state::mark_read_on_open(db.clone(), character_id, mail_id), |()| {
        Message::MarkedRead
      }),
    ]),
    None => render,
  }
}

fn update_snooze(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::SnoozeMenuToggled => {
      state.snooze_menu = if state.snooze_menu == SnoozeMenu::Closed {
        SnoozeMenu::Presets
      } else {
        SnoozeMenu::Closed
      };
      state.snooze_calendar = None;
      Task::none()
    }
    Message::SnoozePreset(preset) => {
      state.snooze_menu = SnoozeMenu::Closed;
      let Some(mail_id) = state.selected else {
        return Task::none();
      };
      let Some(character_id) = state.character_for(mail_id) else {
        return Task::none();
      };
      let until = preset.resolve(chrono::Utc::now()).to_rfc3339();
      Task::perform(snooze::snooze_until(db.clone(), character_id, mail_id, until), |()| {
        Message::OverlayWritten
      })
    }
    Message::SnoozeCalendarOpened => {
      state.snooze_menu = SnoozeMenu::Calendar;
      state.snooze_calendar = Some(snooze::Calendar::open(chrono::Utc::now()));
      Task::none()
    }
    Message::SnoozeCalendarBack => {
      state.snooze_menu = SnoozeMenu::Presets;
      state.snooze_calendar = None;
      Task::none()
    }
    Message::SnoozeCalendarConfirmed => {
      let until = state.snooze_calendar.and_then(|c| c.resolved()).map(|d| d.to_rfc3339());
      state.snooze_menu = SnoozeMenu::Closed;
      state.snooze_calendar = None;
      let (Some(until), Some(mail_id)) = (until, state.selected) else {
        return Task::none();
      };
      let Some(character_id) = state.character_for(mail_id) else {
        return Task::none();
      };
      Task::perform(snooze::snooze_until(db.clone(), character_id, mail_id, until), |()| {
        Message::OverlayWritten
      })
    }
    Message::Unsnooze(mail_id) => {
      state.snooze_menu = SnoozeMenu::Closed;
      state.snooze_calendar = None;
      let Some(character_id) = state.character_for(mail_id) else {
        return Task::none();
      };
      Task::perform(snooze::unsnooze(db.clone(), character_id, mail_id), |()| {
        Message::OverlayWritten
      })
    }
    _ => Task::none(),
  }
}

fn update_snooze_calendar(state: &mut State, message: Message) -> Task<Message> {
  let Some(cal) = state.snooze_calendar.as_mut() else {
    return Task::none();
  };
  match message {
    Message::SnoozeCalendarPrevMonth => cal.prev_month(),
    Message::SnoozeCalendarNextMonth => cal.next_month(),
    Message::SnoozeCalendarDaySelected(year, month0, day) => cal.select_day(year, month0, day),
    Message::SnoozeCalendarHourUp => cal.hour_up(),
    Message::SnoozeCalendarHourDown => cal.hour_down(),
    Message::SnoozeCalendarMinuteUp => cal.minute_up(),
    Message::SnoozeCalendarMinuteDown => cal.minute_down(),
    Message::SnoozeCalendarChip(hour, minute) => cal.set_time(hour, minute),
    _ => {}
  }
  Task::none()
}

fn reload_drafts(state: &State, db: &Database) -> Task<Message> {
  let Scope::Character(character_id) = state.active;
  Task::perform(draft::load_rows(db.clone(), character_id), Message::DraftRowsLoaded)
}

fn update_drafts(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::DraftDeleted(id) => {
      state.drafts.retain(|row| row.id != id);
      let reload = reload_for(db, state.active, state.folder);
      Task::perform(draft::delete(db.clone(), id), |()| ())
        .discard()
        .chain(reload)
    }
    Message::DraftOpened(id) => {
      let db = db.clone();
      Task::perform(async move { mail::draft(&db, id).await.ok().flatten() }, |row| {
        Message::DraftLoaded(Box::new(row))
      })
    }
    Message::DraftRowsLoaded(rows) => {
      state.drafts = rows;
      Task::none()
    }
    // A compose window's save completed; refresh the main-view Drafts list and folder badge. The
    // persisted row id is threaded back to the originating window by the app, not here.
    Message::DraftSaved(_) => reload_drafts(state, db),
    _ => Task::none(),
  }
}

fn update_labels(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::LabelModalOpened => {
      state.label_modal = Some(LabelDraft::blank());
      state.label_picker = None;
      Task::none()
    }
    Message::LabelModalClosed => {
      state.label_modal = None;
      Task::none()
    }
    Message::LabelNameChanged(value) => {
      if let Some(draft) = state.label_modal.as_mut() {
        draft.name = value.chars().take(labels::NAME_MAX_CHARS).collect();
      }
      Task::none()
    }
    Message::LabelColorPicked(hex) => {
      if let Some(draft) = state.label_modal.as_mut() {
        draft.color = hex;
      }
      Task::none()
    }
    Message::LabelModalSubmitted => {
      let Some(draft) = state.label_modal.as_ref() else {
        return Task::none();
      };
      if !draft.can_create() {
        return Task::none();
      }
      let draft = draft.clone();
      state.label_modal = None;
      let Scope::Character(character_id) = state.active;
      let temp_id = labels::temp_label_id();
      Task::perform(labels::enqueue_create(db.clone(), character_id, temp_id, draft), |()| {
        Message::LabelsWritten
      })
    }
    Message::LabelPickerOpened(mail_id) => {
      state.label_modal = None;
      state.snooze_menu = SnoozeMenu::Closed;
      state.label_picker = Some(LabelPicker {
        anchor: None,
        mail_id,
      });
      Task::none()
    }
    Message::LabelRowMenuOpened(mail_id) => {
      state.selected = Some(mail_id);
      state.label_modal = None;
      state.label_picker = Some(LabelPicker {
        anchor: state.cursor,
        mail_id,
      });
      Task::none()
    }
    Message::LabelPickerClosed => {
      state.label_picker = None;
      Task::none()
    }
    Message::LabelToggled(mail_id, label_id) => {
      let Some(character_id) = state.character_for(mail_id) else {
        return Task::none();
      };
      Task::perform(
        labels::enqueue_toggle(db.clone(), character_id, mail_id, label_id),
        |()| Message::LabelsWritten,
      )
    }
    Message::LabelDeleteRequested(label_id) => {
      state.pending_label_delete = Some(label_id);
      Task::none()
    }
    Message::LabelDeleteCancelled => {
      state.pending_label_delete = None;
      Task::none()
    }
    Message::LabelDeleteConfirmed => {
      let Some(label_id) = state.pending_label_delete.take() else {
        return Task::none();
      };
      let Scope::Character(character_id) = state.active;
      Task::perform(labels::enqueue_delete(db.clone(), character_id, label_id), |()| {
        Message::LabelsWritten
      })
    }
    Message::LabelDragMoved(point) => {
      state.cursor = Some(point);
      Task::none()
    }
    Message::DropTargetEntered(_) | Message::DropTargetLeft(_) | Message::LabelDropReleased => {
      update_label_drag(state, message, db)
    }
    Message::LabelsWritten => reload_after_label_write(state, db),
    _ => Task::none(),
  }
}

fn update_label_drag(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::DropTargetEntered(target) => {
      if state.dragging_mail.is_some() {
        state.drop_target = Some(target);
      }
      Task::none()
    }
    Message::DropTargetLeft(target) => {
      if state.drop_target == Some(target) {
        state.drop_target = None;
      }
      Task::none()
    }
    Message::LabelDropReleased => {
      let drop = state.dragging_mail.zip(state.drop_target);
      state.dragging_mail = None;
      state.drop_target = None;
      let Some((mail_id, target)) = drop else {
        return Task::none();
      };
      let Some(character_id) = state.character_for(mail_id) else {
        return Task::none();
      };
      match target {
        DropTarget::Label(label_id) => Task::perform(
          labels::enqueue_assign(db.clone(), character_id, mail_id, label_id),
          |()| Message::LabelsWritten,
        ),
        DropTarget::StandardFolder(folder) => {
          Task::perform(triage::move_to_box(db.clone(), character_id, mail_id, folder), |()| {
            Message::LabelsWritten
          })
        }
      }
    }
    _ => Task::none(),
  }
}

fn reload_after_label_write(state: &State, db: &Database) -> Task<Message> {
  let folder = reload_for(db, state.active, state.folder);
  let Some(render) = state.render.as_ref() else {
    return folder;
  };
  let mail_id = render.mail.header.mail_id();
  let character_id = render.mail.header.character_id();
  let render = Task::perform(load_render(db.clone(), character_id, mail_id), move |render| {
    Message::RenderLoaded {
      mail_id,
      render: Box::new(render),
    }
  });
  Task::batch([folder, render])
}

pub fn view(state: &State) -> Element<'_, Message> {
  shell::shell(state)
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if state.folder_pane.is_active() {
    return iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(event, Message::FolderPaneDragged, Message::FolderPaneDragEnd)
    });
  }
  if state.message_list_pane.is_active() {
    return iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(event, Message::ListPaneDragged, Message::ListPaneDragEnd)
    });
  }
  if state.dragging_mail.is_some() {
    return iced::event::listen_with(|event, _status, _id| {
      matches!(
        event,
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
      )
      .then_some(Message::LabelDropReleased)
    });
  }
  iced::Subscription::none()
}

async fn load_mail(db: Database, scope: Scope, folder: Folder) -> Loaded {
  let roster = loaders::load_roster(&db).await;
  let unified = loaders::load_unified(&db).await;
  let unified_unread = loaders::load_unified_unread(&db).await;

  let Scope::Character(scope_id) = scope;

  let folder_data = match folder {
    Folder::Unified => loaders::load_folder_pane_unified(&db, &roster).await,
    _ => loaders::load_folder_pane(&db, scope_id).await,
  };

  let first_page = message_list::load_first_page(&db, scope, folder).await;
  let outbox_indicator = loaders::load_outbox_indicator(&db).await;
  let drafts = draft::load_rows(db.clone(), scope_id).await;

  let (headers, overlays) = match folder {
    Folder::Unified => (Vec::new(), HashMap::new()),
    _ => {
      let headers = loaders::load_headers(&db, scope_id).await;
      let overlays = loaders::load_overlays(&db, scope_id).await;
      (headers, overlays)
    }
  };

  let ui = window_state::load();
  let folder_pane_width = ui
    .panes
    .get(FOLDER_PANE_KEY)
    .copied()
    .unwrap_or(FOLDER_PANE_DEFAULT_WIDTH);
  let message_list_pane_width = ui
    .panes
    .get(MESSAGE_LIST_PANE_KEY)
    .copied()
    .unwrap_or(MESSAGE_LIST_PANE_DEFAULT_WIDTH);

  Loaded {
    drafts,
    folder,
    folder_data,
    folder_pane_width,
    headers,
    message_list_pane_width,
    messages: first_page.tail,
    messages_has_more: first_page.has_more,
    outbox_indicator,
    overlays,
    roster,
    scope,
    unified,
    unified_unread,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample_render() -> crate::store::model::character_mail_view::MailRender {
    use crate::store::model::{
      CharacterMail, CharacterMailBody, CharacterMailRecipient, character_mail_view::MailRender,
    };
    MailRender {
      body: CharacterMailBody {
        body: "<p>Form up at Jita.</p>".to_owned(),
        character_id: 42,
        mail_id: 7,
      },
      header: CharacterMail {
        character_id: 42,
        from_id: 95_000_001,
        from_name: "Vex Voronova".to_owned(),
        is_read: true,
        mail_id: 7,
        subject: Some("CTA tonight".to_owned()),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
        ..Default::default()
      },
      label_ids: vec![8],
      recipients: vec![CharacterMailRecipient {
        character_id: 42,
        mail_id: 7,
        recipient_id: 42,
        recipient_name: "Vex Voronova".to_owned(),
        recipient_type: "character".to_owned(),
      }],
      recipients_display: "Vex Voronova".to_owned(),
    }
  }

  mod scope_gate {
    use pretty_assertions::assert_eq;

    use super::*;

    fn pilot(id: i64, granted: Option<&str>) -> RosterPilot {
      RosterPilot {
        corp: "VEX".to_owned(),
        granted_scopes: granted.map(str::to_owned),
        id,
        name: "Vex".to_owned(),
        portrait: images::ImageState::Fresh("/cache/42.jpg".into()),
        unread: 0,
      }
    }

    #[test]
    fn it_does_not_gate_when_the_active_character_has_the_mail_scopes() {
      let granted = crate::features::registry::descriptor(crate::config::Feature::Mail)
        .scopes
        .join(" ");
      let mut state = State::new(42);
      state.roster = vec![pilot(42, Some(&granted))];

      assert!(state.scope_gate().is_none());
    }

    #[test]
    fn it_does_not_gate_when_the_active_character_is_absent_from_the_roster() {
      let state = State::new(42);

      assert!(state.scope_gate().is_none());
    }

    #[test]
    fn it_gates_when_the_active_character_lacks_the_mail_scopes() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, None)];

      let gate = state.scope_gate().expect("missing scope should gate");

      assert_eq!(gate.0, 42);
      assert!(!gate.2.is_empty());
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::mail::message_list::{DayBucket, MessageRow, SenderKind};

    fn message_row(mail_id: i64, sender_id: i64) -> MessageRow {
      MessageRow {
        bucket: DayBucket::Today,
        character_id: 42,
        is_read: true,
        is_starred: false,
        has_attachment: false,
        important: false,
        sender_kind: SenderKind::Character,
        label_ids: Vec::new(),
        labels: Vec::new(),
        mail_id,
        sender: "Vex".to_owned(),
        sender_id,
        sender_portrait: images::ImageState::Stale {
          id: sender_id,
          kind: images::ImageKind::CharacterPortrait,
        },
        snippet: String::new(),
        subject: "S".to_owned(),
        time: "10:00".to_owned(),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
      }
    }

    #[test]
    fn it_collects_stale_keys_from_the_roster_messages_and_open_render() {
      let mut state = State::new(42);
      state.roster = vec![RosterPilot {
        corp: "VEX".to_owned(),
        granted_scopes: None,
        id: 42,
        name: "Vex".to_owned(),
        portrait: images::ImageState::Stale {
          id: 42,
          kind: images::ImageKind::CharacterPortrait,
        },
        unread: 0,
      }];
      state.messages = vec![message_row(7, 95_000_001)];
      state.render = Some(ReadingRender {
        is_starred: false,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_002,
          kind: images::ImageKind::CharacterPortrait,
        },
      });

      let stale = state.stale_images();

      assert!(stale.contains(&(images::ImageKind::CharacterPortrait, 42)));
      assert!(stale.contains(&(images::ImageKind::CharacterPortrait, 95_000_001)));
      assert!(stale.contains(&(images::ImageKind::CharacterPortrait, 95_000_002)));
    }

    #[test]
    fn it_is_empty_for_a_fresh_default_state() {
      let state = State::new(42);

      assert!(state.stale_images().is_empty());
    }

    #[test]
    fn it_omits_a_fresh_portrait_and_a_non_positive_sender_id() {
      let mut state = State::new(42);
      state.roster = vec![RosterPilot {
        corp: "VEX".to_owned(),
        granted_scopes: None,
        id: 42,
        name: "Vex".to_owned(),
        portrait: images::ImageState::Fresh("/cache/42.jpg".into()),
        unread: 0,
      }];
      state.messages = vec![message_row(7, 0)];

      assert_eq!(state.stale_images(), Vec::new());
    }
  }

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_a_restored_folder_width_to_its_80px_minimum() {
      let mut ui = UiState::default();
      ui.panes.insert(FOLDER_PANE_KEY.to_owned(), 40.0);
      ui.panes.insert(MESSAGE_LIST_PANE_KEY.to_owned(), 60.0);

      let state = State::new(42).with_restored_panes(&ui);

      assert_eq!(state.folder_pane_width(), FOLDER_PANE_MIN_WIDTH);
      assert_eq!(state.message_list_pane_width(), MESSAGE_LIST_PANE_MIN_WIDTH);
    }

    #[test]
    fn it_falls_back_to_default_pane_widths_when_unsized() {
      let state = State::new(42).with_restored_panes(&UiState::default());

      assert_eq!(state.folder_pane_width(), FOLDER_PANE_DEFAULT_WIDTH);
      assert_eq!(state.message_list_pane_width(), MESSAGE_LIST_PANE_DEFAULT_WIDTH);
    }

    #[test]
    fn it_opens_scoped_to_the_starting_character() {
      let state = State::new(42);

      assert_eq!(state.active(), Scope::Character(42));
      assert_eq!(state.unified_unread(), 0);
    }

    #[test]
    fn it_restores_pane_widths_from_the_keyed_store() {
      let mut ui = UiState::default();
      ui.panes.insert(FOLDER_PANE_KEY.to_owned(), 300.0);
      ui.panes.insert(MESSAGE_LIST_PANE_KEY.to_owned(), 420.0);

      let state = State::new(42).with_restored_panes(&ui);

      assert_eq!(state.folder_pane_width(), 300.0);
      assert_eq!(state.message_list_pane_width(), 420.0);
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;
    fn list_row(mail_id: i64, character_id: i64, is_read: bool) -> message_list::MessageRow {
      message_list::MessageRow {
        bucket: message_list::DayBucket::Today,
        character_id,
        is_read,
        is_starred: false,
        has_attachment: false,
        important: false,
        sender_kind: message_list::SenderKind::Character,
        label_ids: Vec::new(),
        labels: Vec::new(),
        mail_id,
        sender: "Vex".to_owned(),
        sender_id: 95_000_001,
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
        snippet: String::new(),
        subject: "S".to_owned(),
        time: "10:00".to_owned(),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
      }
    }

    #[tokio::test]
    async fn it_clamps_a_folder_drag_to_the_80px_minimum() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::FolderPaneDragStart, &db);
      let _ = update(&mut state, Message::FolderPaneDragged(500.0), &db);
      let _ = update(&mut state, Message::FolderPaneDragged(0.0), &db);
      let _ = update(&mut state, Message::FolderPaneDragEnd, &db);

      assert!((state.folder_pane_width() - FOLDER_PANE_MIN_WIDTH).abs() < 0.5);
    }

    #[tokio::test]
    async fn it_clears_the_open_mail_when_a_reload_no_longer_lists_it() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.active = Scope::Character(42);
      state.folder = Folder::Standard(StandardFolder::Inbox);
      state.selected = Some(7);
      state.render = Some(ReadingRender {
        is_starred: false,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      });
      let loaded = Loaded {
        folder: Folder::Standard(StandardFolder::Inbox),
        messages: vec![list_row(8, 42, true)],
        scope: Scope::Character(42),
        ..Loaded::default()
      };

      let _ = update(&mut state, Message::Loaded(Box::new(loaded)), &db);

      assert!(
        state.selected().is_none(),
        "archiving the open mail drops it from the reload and clears the selection"
      );
      assert!(
        state.render().is_none(),
        "the reading pane stops rendering the gone mail"
      );
    }

    #[tokio::test]
    async fn it_clears_the_open_render_when_the_folder_changes() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.render = Some(ReadingRender {
        is_starred: false,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      });

      let _ = update(
        &mut state,
        Message::FolderSelected(Folder::Standard(StandardFolder::Starred)),
        &db,
      );

      assert!(state.render().is_none());
    }

    #[tokio::test]
    async fn it_clears_the_render_when_selecting_a_row_no_longer_in_the_list() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.render = Some(ReadingRender {
        is_starred: false,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      });

      let _ = update(&mut state, Message::Selected(999), &db);

      assert_eq!(state.selected(), Some(999));
      assert!(state.render().is_none());
    }

    #[tokio::test]
    async fn it_closes_the_dropdown_and_resets_the_folder_on_a_scope_selection() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.picker_open = true;
      state.folder = Folder::Standard(StandardFolder::Starred);
      state.selected = Some(7);

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(42)), &db);

      assert_eq!(state.active(), Scope::Character(42));
      assert_eq!(state.folder(), Folder::Unified);
      assert!(!state.picker_open());
      assert!(state.selected().is_none());
    }

    #[tokio::test]
    async fn it_confirms_a_calendar_snooze_and_unsnoozes() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.active = Scope::Character(42);
      state.selected = Some(7);

      let _ = update(&mut state, Message::SnoozeCalendarOpened, &db);
      let _ = update(&mut state, Message::SnoozeCalendarConfirmed, &db);
      assert!(state.snooze_calendar().is_none());

      let _ = update(&mut state, Message::Unsnooze(7), &db);
      assert!(!state.snooze_presets_open());
    }

    #[tokio::test]
    async fn it_dispatches_the_remaining_arms_without_panicking() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.active = Scope::Character(42);
      state.messages = vec![list_row(7, 42, true)];
      state.selected = Some(7);
      state.render = Some(ReadingRender {
        is_starred: false,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      });

      for message in [
        Message::PickerToggled,
        Message::MarkedRead,
        Message::OverlayWritten,
        Message::ToggleStar(7),
        Message::Archive(7),
        Message::Trash(7),
        Message::Reply(7),
        Message::ReplyAll(7),
        Message::Forward(7),
        Message::OutboxRetry(1),
        Message::OutboxDismiss(1),
        Message::SearchChanged("cta".to_owned()),
        Message::RenderLoaded {
          mail_id: 7,
          render: Box::new(None),
        },
        Message::PaneSettled(FOLDER_PANE_KEY, 240.0),
      ] {
        let _ = update(&mut state, message, &db);
      }

      assert_eq!(state.search(), "cta");
      assert!(state.render().is_none());
    }

    #[tokio::test]
    async fn it_does_not_adopt_a_stale_scope_loads_scope_specific_picture() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.active = Scope::Character(42);
      state.folder_data = FolderPaneData {
        labels: vec![loaders::FolderLabel {
          color: None,
          label_id: 99,
          name: "Sentinel".to_owned(),
          unread: 7,
        }],
        ..FolderPaneData::default()
      };
      let loaded = Loaded {
        scope: Scope::Character(0),
        folder: Folder::Unified,
        ..Loaded::default()
      };

      let _ = update(&mut state, Message::Loaded(Box::new(loaded)), &db);

      assert_eq!(state.folder_data().labels.len(), 1);
      assert_eq!(state.active(), Scope::Character(42));
    }

    #[tokio::test]
    async fn it_drops_a_render_for_a_mail_that_is_no_longer_selected() {
      let mut state = State::new(42);
      state.selected = Some(9);
      let db = crate::store::open_test().await.unwrap();
      let render = ReadingRender {
        is_starred: false,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      };

      let _ = update(
        &mut state,
        Message::RenderLoaded {
          mail_id: 7,
          render: Box::new(Some(render)),
        },
        &db,
      );

      assert!(
        state.render().is_none(),
        "a render for mail 7 must not land while mail 9 is selected"
      );
    }

    #[tokio::test]
    async fn it_echoes_a_settled_folder_pane_width_for_the_app_to_persist() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::FolderPaneDragStart, &db);
      let _ = update(&mut state, Message::FolderPaneDragged(500.0), &db);
      let _ = update(&mut state, Message::FolderPaneDragged(540.0), &db);
      let _ = update(&mut state, Message::FolderPaneDragEnd, &db);

      assert_eq!(state.folder_pane_width(), FOLDER_PANE_DEFAULT_WIDTH + 40.0);
    }

    #[tokio::test]
    async fn it_folds_a_refreshed_outbox_indicator_into_state() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      let indicator = OutboxIndicator {
        pending: 2,
        failed: Vec::new(),
      };

      let _ = update(&mut state, Message::OutboxRefreshed(Box::new(indicator)), &db);

      assert_eq!(state.outbox_indicator().pending, 2);
    }

    #[tokio::test]
    async fn it_keeps_the_open_mail_when_a_reload_still_lists_it() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.active = Scope::Character(42);
      state.folder = Folder::Standard(StandardFolder::Inbox);
      state.selected = Some(7);
      let loaded = Loaded {
        folder: Folder::Standard(StandardFolder::Inbox),
        messages: vec![list_row(7, 42, true), list_row(8, 42, true)],
        scope: Scope::Character(42),
        ..Loaded::default()
      };

      let _ = update(&mut state, Message::Loaded(Box::new(loaded)), &db);

      assert_eq!(
        state.selected(),
        Some(7),
        "a reload that still lists the open mail keeps the selection"
      );
    }

    #[tokio::test]
    async fn it_opens_and_edits_the_snooze_calendar() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::SnoozeCalendarOpened, &db);
      assert!(state.snooze_calendar().is_some());
      for message in [
        Message::SnoozeCalendarNextMonth,
        Message::SnoozeCalendarPrevMonth,
        Message::SnoozeCalendarHourUp,
        Message::SnoozeCalendarHourDown,
        Message::SnoozeCalendarMinuteUp,
        Message::SnoozeCalendarMinuteDown,
        Message::SnoozeCalendarChip(11, 0),
        Message::SnoozeCalendarDaySelected(2026, 5, 15),
      ] {
        let _ = update(&mut state, message, &db);
      }
      assert!(state.snooze_calendar().is_some());
      let _ = update(&mut state, Message::SnoozeCalendarBack, &db);
      assert!(state.snooze_calendar().is_none());
      assert!(state.snooze_presets_open());
    }

    #[tokio::test]
    async fn it_records_a_scope_selection() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(42)), &db);

      assert_eq!(state.active(), Scope::Character(42));
    }

    #[tokio::test]
    async fn it_resolves_the_owning_character_for_an_action() {
      let mut state = State::new(42);
      state.active = Scope::Character(42);
      state.render = Some(ReadingRender {
        is_starred: false,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      });
      assert_eq!(state.character_for(7), Some(42));
      state.render = None;
      state.messages = vec![list_row(8, 43, true)];
      assert_eq!(state.character_for(8), Some(43));
      assert_eq!(state.character_for(999), Some(42));
    }

    #[tokio::test]
    async fn it_selects_a_row_and_clears_any_open_snooze_menu() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.messages = vec![list_row(7, 42, true)];
      state.snooze_menu = SnoozeMenu::Presets;

      let _ = update(&mut state, Message::Selected(7), &db);

      assert_eq!(state.selected(), Some(7));
      assert!(!state.snooze_presets_open());
    }

    #[tokio::test]
    async fn it_stores_a_landed_reading_pane_render() {
      let mut state = State::new(42);
      state.selected = Some(7);
      let db = crate::store::open_test().await.unwrap();
      let render = ReadingRender {
        is_starred: true,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      };

      let _ = update(
        &mut state,
        Message::RenderLoaded {
          mail_id: 7,
          render: Box::new(Some(render.clone())),
        },
        &db,
      );

      assert_eq!(state.render(), Some(&render));
    }

    #[tokio::test]
    async fn it_toggles_the_account_switcher_dropdown() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      assert!(!state.picker_open());
      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open());
      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open());
    }

    #[tokio::test]
    async fn it_toggles_the_snooze_preset_menu() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::SnoozeMenuToggled, &db);
      assert!(state.snooze_presets_open());
      let _ = update(&mut state, Message::SnoozeMenuToggled, &db);
      assert!(!state.snooze_presets_open());
    }

    #[tokio::test]
    async fn it_writes_a_preset_snooze_for_the_open_mail_and_closes_the_menu() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.active = Scope::Character(42);
      state.selected = Some(7);
      state.snooze_menu = SnoozeMenu::Presets;

      let _ = update(&mut state, Message::SnoozePreset(snooze::Preset::Tomorrow), &db);
      assert!(!state.snooze_presets_open());

      state.selected = None;
      let _ = update(&mut state, Message::SnoozePreset(snooze::Preset::Tomorrow), &db);
    }

    mod labels_dispatch {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_assigns_the_label_when_releasing_onto_a_target() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.messages = vec![list_row(7, 42, true)];
        state.dragging_mail = Some(7);
        state.drop_target = Some(DropTarget::Label(8));

        let _ = update_labels(&mut state, Message::LabelDropReleased, &db);

        assert!(state.dragging_mail.is_none());
        assert!(state.drop_target.is_none());
      }

      #[tokio::test]
      async fn it_clears_drag_state_when_releasing_onto_a_standard_box() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.messages = vec![list_row(7, 42, true)];
        state.dragging_mail = Some(7);
        state.drop_target = Some(DropTarget::StandardFolder(StandardFolder::Archive));

        let _ = update_labels(&mut state, Message::LabelDropReleased, &db);

        assert!(state.dragging_mail.is_none());
        assert!(state.drop_target.is_none());
      }

      #[tokio::test]
      async fn it_cancels_a_pending_label_deletion() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.pending_label_delete = Some(8);

        let _ = update_labels(&mut state, Message::LabelDeleteCancelled, &db);

        assert!(state.pending_label_delete.is_none());
      }

      #[tokio::test]
      async fn it_clears_drag_state_when_releasing_with_no_target() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.dragging_mail = Some(7);
        state.drop_target = None;

        let _ = update_labels(&mut state, Message::LabelDropReleased, &db);

        assert!(state.dragging_mail.is_none());
        assert!(state.drop_target.is_none());
      }

      #[tokio::test]
      async fn it_clears_only_the_matching_drop_target_on_leave() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.drop_target = Some(DropTarget::Label(8));

        let _ = update_labels(&mut state, Message::DropTargetLeft(DropTarget::Label(9)), &db);
        assert_eq!(state.drop_target, Some(DropTarget::Label(8)));

        let _ = update_labels(&mut state, Message::DropTargetLeft(DropTarget::Label(8)), &db);
        assert!(state.drop_target.is_none());
      }

      #[tokio::test]
      async fn it_closes_the_label_modal() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_modal = Some(LabelDraft::blank());

        let _ = update_labels(&mut state, Message::LabelModalClosed, &db);

        assert!(state.label_modal.is_none());
      }

      #[tokio::test]
      async fn it_closes_the_label_picker() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_picker = Some(LabelPicker {
          anchor: None,
          mail_id: 7,
        });

        let _ = update_labels(&mut state, Message::LabelPickerClosed, &db);

        assert!(state.label_picker.is_none());
      }

      #[tokio::test]
      async fn it_closes_the_modal_when_submitting_a_creatable_draft() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_modal = Some(LabelDraft {
          color: "#ff6600".to_owned(),
          name: "Fleet".to_owned(),
        });

        let _ = update_labels(&mut state, Message::LabelModalSubmitted, &db);

        assert!(state.label_modal.is_none());
      }

      #[tokio::test]
      async fn it_consumes_the_pending_label_on_a_delete_confirmation() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.pending_label_delete = Some(8);

        let _ = update_labels(&mut state, Message::LabelDeleteConfirmed, &db);

        assert!(state.pending_label_delete.is_none());
      }

      #[tokio::test]
      async fn it_drops_a_submission_for_an_uncreatable_draft() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_modal = Some(LabelDraft::blank());

        let _ = update_labels(&mut state, Message::LabelModalSubmitted, &db);

        assert!(state.label_modal.is_some());
      }

      #[tokio::test]
      async fn it_drops_a_submission_with_no_open_modal() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();

        let _ = update_labels(&mut state, Message::LabelModalSubmitted, &db);

        assert!(state.label_modal.is_none());
      }

      #[tokio::test]
      async fn it_ignores_a_delete_confirmation_with_nothing_pending() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();

        let _ = update_labels(&mut state, Message::LabelDeleteConfirmed, &db);

        assert!(state.pending_label_delete.is_none());
      }

      #[tokio::test]
      async fn it_ignores_a_name_change_with_no_open_modal() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();

        let _ = update_labels(&mut state, Message::LabelNameChanged("Fleet".to_owned()), &db);

        assert!(state.label_modal.is_none());
      }

      #[tokio::test]
      async fn it_marks_a_drop_target_only_while_dragging() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();

        let _ = update_labels(&mut state, Message::DropTargetEntered(DropTarget::Label(8)), &db);
        assert!(state.drop_target.is_none());

        state.dragging_mail = Some(7);
        let _ = update_labels(&mut state, Message::DropTargetEntered(DropTarget::Label(8)), &db);
        assert_eq!(state.drop_target, Some(DropTarget::Label(8)));
      }

      #[tokio::test]
      async fn it_opens_a_blank_label_modal_and_clears_the_picker() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_picker = Some(LabelPicker {
          anchor: None,
          mail_id: 7,
        });

        let _ = update_labels(&mut state, Message::LabelModalOpened, &db);

        assert_eq!(state.label_modal, Some(LabelDraft::blank()));
        assert!(state.label_picker.is_none());
      }

      #[tokio::test]
      async fn it_opens_a_cursor_anchored_label_picker_from_a_row_menu() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.cursor = Some(Point::new(12.0, 34.0));
        state.label_modal = Some(LabelDraft::blank());

        let _ = update_labels(&mut state, Message::LabelRowMenuOpened(7), &db);

        assert_eq!(state.selected, Some(7));
        assert!(state.label_modal.is_none());
        assert_eq!(
          state.label_picker,
          Some(LabelPicker {
            anchor: Some(Point::new(12.0, 34.0)),
            mail_id: 7,
          })
        );
      }

      #[tokio::test]
      async fn it_opens_an_unanchored_label_picker() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_modal = Some(LabelDraft::blank());
        state.snooze_menu = SnoozeMenu::Presets;

        let _ = update_labels(&mut state, Message::LabelPickerOpened(7), &db);

        assert!(state.label_modal.is_none());
        assert_eq!(state.snooze_menu, SnoozeMenu::Closed);
        assert_eq!(
          state.label_picker,
          Some(LabelPicker {
            anchor: None,
            mail_id: 7,
          })
        );
      }

      #[tokio::test]
      async fn it_records_a_pending_label_deletion() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();

        let _ = update_labels(&mut state, Message::LabelDeleteRequested(8), &db);

        assert_eq!(state.pending_label_delete, Some(8));
      }

      #[tokio::test]
      async fn it_records_a_picked_label_color() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_modal = Some(LabelDraft::blank());

        let _ = update_labels(&mut state, Message::LabelColorPicked("#ff6600".to_owned()), &db);

        assert_eq!(
          state.label_modal.as_ref().map(|d| d.color.clone()),
          Some("#ff6600".to_owned())
        );
      }

      #[tokio::test]
      async fn it_reloads_after_a_label_write() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();

        let _ = update_labels(&mut state, Message::LabelsWritten, &db);
      }

      #[tokio::test]
      async fn it_toggles_a_label_for_a_known_mail() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.messages = vec![list_row(7, 42, true)];

        let _ = update_labels(&mut state, Message::LabelToggled(7, 8), &db);
      }

      #[tokio::test]
      async fn it_tracks_the_cursor_during_a_label_drag() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        let point = Point::new(5.0, 6.0);

        let _ = update_labels(&mut state, Message::LabelDragMoved(point), &db);

        assert_eq!(state.cursor, Some(point));
      }

      #[tokio::test]
      async fn it_truncates_the_label_name_to_the_max_length() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.label_modal = Some(LabelDraft::blank());
        let oversized = "x".repeat(labels::NAME_MAX_CHARS + 10);

        let _ = update_labels(&mut state, Message::LabelNameChanged(oversized), &db);

        let name = state.label_modal.as_ref().map(|d| d.name.clone()).unwrap();
        assert_eq!(name.chars().count(), labels::NAME_MAX_CHARS);
      }
    }

    mod drafts {
      use pretty_assertions::assert_eq;

      use super::*;
      use crate::store::{
        self,
        model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
        repo::{character, mail},
      };

      async fn seed_character(db: &Database, id: i64) {
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
        let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
        character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
          .await
          .unwrap();
      }

      fn typed_compose() -> compose::Draft {
        let mut draft = compose::Draft::blank(42);
        draft.subject = "CTA".to_owned();
        draft.body = text_editor::Content::with_text("Form up.");
        draft.to.push(compose::Recipient::typed("Vex"));
        draft
      }

      #[tokio::test]
      async fn it_persists_one_row_for_a_non_empty_compose_window() {
        let db = store::open_test().await.unwrap();
        seed_character(&db, 42).await;

        let (id, input) = typed_compose()
          .pending_save()
          .expect("a non-empty compose is worth saving");
        draft::persist(db.clone(), id, input).await;

        assert_eq!(mail::count_drafts_for_character(&db, 42).await.unwrap(), 1);
      }

      #[tokio::test]
      async fn it_offers_no_pending_save_for_a_blank_compose() {
        assert!(compose::Draft::blank(42).pending_save().is_none());
      }

      #[tokio::test]
      async fn it_updates_the_same_row_when_re_saving_an_opened_draft() {
        let db = store::open_test().await.unwrap();
        seed_character(&db, 42).await;
        let id = mail::upsert_draft(&db, None, &typed_compose().persist_input())
          .await
          .unwrap();
        let mut compose = typed_compose();
        compose.set_id(Some(id));
        compose.subject = "Edited".to_owned();

        let (saved_id, input) = compose.pending_save().unwrap();
        assert_eq!(saved_id, Some(id), "the persisted id is threaded back into the save");
        draft::persist(db.clone(), saved_id, input).await;

        assert_eq!(mail::count_drafts_for_character(&db, 42).await.unwrap(), 1);
        assert_eq!(mail::draft(&db, id).await.unwrap().unwrap().subject, "Edited");
      }

      #[tokio::test]
      async fn it_deletes_the_row_by_id_on_a_successful_send() {
        let db = store::open_test().await.unwrap();
        seed_character(&db, 42).await;
        let id = mail::upsert_draft(&db, None, &typed_compose().persist_input())
          .await
          .unwrap();

        // A successful send deletes the persisted draft by id; verify the by-id delete path directly.
        delete_draft(db.clone(), id).await;
        assert_eq!(mail::count_drafts_for_character(&db, 42).await.unwrap(), 0);
      }

      #[tokio::test]
      async fn it_fills_a_compose_from_a_loaded_draft_row() {
        let db = store::open_test().await.unwrap();
        seed_character(&db, 42).await;
        let id = mail::upsert_draft(&db, None, &typed_compose().persist_input())
          .await
          .unwrap();
        let row = mail::draft(&db, id).await.unwrap().unwrap();

        let compose = compose::Draft::from_persisted(&row);

        assert_eq!(compose.id, Some(id));
        assert_eq!(compose.subject, "CTA");
        assert_eq!(compose.body.text(), "Form up.");
      }

      #[tokio::test]
      async fn it_drops_a_deleted_draft_from_the_in_memory_list() {
        let db = store::open_test().await.unwrap();
        seed_character(&db, 42).await;
        let id = mail::upsert_draft(&db, None, &typed_compose().persist_input())
          .await
          .unwrap();
        let mut state = State::new(42);
        state.drafts = draft::load_rows(db.clone(), 42).await;

        let _ = update(&mut state, Message::DraftDeleted(id), &db);

        assert!(state.drafts().is_empty());
      }
    }

    mod pagination {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_appends_a_loaded_search_page_into_the_search_accumulator() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.search = "cta".to_owned();
        state.all_messages = vec![list_row(7, 42, true)];
        state.search_loading = true;

        let _ = update(
          &mut state,
          Message::SearchPageLoaded {
            query: "cta".to_owned(),
            rows: vec![list_row(8, 42, true)],
          },
          &db,
        );

        assert_eq!(
          state.all_messages().iter().map(|r| r.mail_id).collect::<Vec<_>>(),
          [7, 8]
        );
        assert!(!state.search_loading);
      }

      #[tokio::test]
      async fn it_appends_a_loaded_tail_page_and_advances_the_cursor() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.messages = vec![list_row(7, 42, true)];
        state.messages_loading = true;

        let epoch = state.messages_page_epoch.current();
        let _ = update(
          &mut state,
          Message::MessagesPageLoaded {
            epoch,
            rows: vec![list_row(8, 42, true)],
          },
          &db,
        );

        assert_eq!(
          state.messages().iter().map(|r| r.mail_id).collect::<Vec<_>>(),
          [7, 8],
          "the new page is appended to the tail"
        );
        assert!(!state.messages_loading);
        assert!(
          !state.messages_has_more,
          "a short page (under the page size) ends pagination"
        );
        assert!(
          state.messages_cursor.is_some(),
          "the cursor advances to the last loaded row"
        );
      }

      #[tokio::test]
      async fn it_clears_search_paging_when_the_query_is_emptied() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.search = "cta".to_owned();
        state.all_messages = vec![list_row(7, 42, true)];
        state.search_loading = true;

        let _ = update(&mut state, Message::SearchChanged(String::new()), &db);

        assert_eq!(state.search(), "");
        assert!(state.all_messages().is_empty());
        assert!(!state.search_loading, "no search runs for an empty query");
      }

      #[tokio::test]
      async fn it_does_not_load_more_when_the_tail_is_exhausted() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.folder = Folder::Standard(StandardFolder::Inbox);
        state.messages = vec![list_row(7, 42, true)];
        state.messages_cursor = Some(cursor_of(&state.messages[0]));
        state.messages_has_more = false;

        let _ = update(
          &mut state,
          Message::ListScrolled {
            absolute: 9_000.0,
            relative: 0.99,
          },
          &db,
        );

        assert!(!state.messages_loading, "no page is requested past the last page");
      }

      #[tokio::test]
      async fn it_drops_a_messages_page_captured_before_a_folder_switch() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.messages = vec![list_row(7, 42, true)];
        state.messages_loading = true;

        // The user scrolled (capturing the epoch) and then switched folders, which bumps the epoch.
        let stale_epoch = state.messages_page_epoch.current();
        let _ = update(
          &mut state,
          Message::FolderSelected(Folder::Standard(StandardFolder::Sent)),
          &db,
        );

        let _ = update(
          &mut state,
          Message::MessagesPageLoaded {
            epoch: stale_epoch,
            rows: vec![list_row(8, 42, true)],
          },
          &db,
        );

        assert_eq!(
          state.messages().iter().map(|r| r.mail_id).collect::<Vec<_>>(),
          [7],
          "a page from the previous folder must not append foreign rows"
        );
      }

      #[tokio::test]
      async fn it_drops_a_search_page_from_a_superseded_query() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.search = "wormhole".to_owned();
        state.search_loading = true;

        let _ = update(
          &mut state,
          Message::SearchPageLoaded {
            query: "cta".to_owned(),
            rows: vec![list_row(8, 42, true)],
          },
          &db,
        );

        assert!(
          state.all_messages().is_empty(),
          "a page tagged with a stale query is discarded"
        );
        assert!(
          state.search_loading,
          "the in-flight load for the current query is left untouched"
        );
      }

      #[tokio::test]
      async fn it_resets_search_paging_and_kicks_a_query_on_search_change() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.all_messages = vec![list_row(7, 42, true)];
        state.search_cursor = Some(cursor_of(&state.all_messages[0]));

        let _ = update(&mut state, Message::SearchChanged("cta".to_owned()), &db);

        assert_eq!(state.search(), "cta");
        assert!(state.all_messages().is_empty(), "the prior search results are cleared");
        assert!(
          state.search_cursor.is_none(),
          "the search cursor resets for the new query"
        );
        assert!(state.search_loading, "the first search page is requested");
      }

      #[tokio::test]
      async fn it_starts_a_tail_load_when_scrolling_past_the_threshold_with_more_pages() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();
        state.folder = Folder::Standard(StandardFolder::Inbox);
        state.messages = vec![list_row(7, 42, true)];
        state.messages_cursor = Some(cursor_of(&state.messages[0]));
        state.messages_has_more = true;

        let _ = update(
          &mut state,
          Message::ListScrolled {
            absolute: 9_000.0,
            relative: 0.99,
          },
          &db,
        );

        assert!(
          state.messages_loading,
          "a deep scroll with more pages requests the next"
        );
      }

      #[tokio::test]
      async fn it_stores_the_absolute_scroll_offset_for_windowing() {
        let mut state = State::new(42);
        let db = crate::store::open_test().await.unwrap();

        let _ = update(
          &mut state,
          Message::ListScrolled {
            absolute: 1_234.0,
            relative: 0.2,
          },
          &db,
        );

        assert_eq!(
          state.list_scroll_offset(),
          1_234.0,
          "the pixel offset is stored so the virtual list can window the body"
        );
        assert!(!state.messages_loading, "a shallow scroll loads no further page");
      }
    }
  }

  mod view {
    use super::*;
    use crate::features::mail::message_list::{DayBucket, MessageRow, SenderKind};

    fn populated_state() -> State {
      let mut state = State::new(42);
      state.active = Scope::Character(42);
      state.roster = vec![
        RosterPilot {
          corp: "VEX".to_owned(),
          granted_scopes: None,
          id: 42,
          name: "Vex Voronova".to_owned(),
          portrait: images::ImageState::Stale {
            id: 42,
            kind: images::ImageKind::CharacterPortrait,
          },
          unread: 3,
        },
        RosterPilot {
          corp: "ALT".to_owned(),
          granted_scopes: None,
          id: 43,
          name: "Alt Pilot".to_owned(),
          portrait: images::ImageState::Stale {
            id: 43,
            kind: images::ImageKind::CharacterPortrait,
          },
          unread: 0,
        },
      ];
      state.folder_data = FolderPaneData {
        labels: vec![loaders::FolderLabel {
          color: Some("#ff6600".to_owned()),
          label_id: 99,
          name: "Fleet".to_owned(),
          unread: 2,
        }],
        standard_counts: loaders::StandardFolderCounts {
          inbox: 3,
          starred: 1,
          ..loaders::StandardFolderCounts::default()
        },
      };
      state.messages = vec![
        row(1, DayBucket::Today, true, false, false, &[]),
        row(2, DayBucket::Today, false, false, false, &["Fleet"]),
        row(3, DayBucket::Yesterday, false, true, false, &[]),
        row(4, DayBucket::Earlier, false, false, true, &["Ops", "Fleet"]),
      ];
      state.unified_unread = 3;
      state.outbox_indicator = OutboxIndicator {
        pending: 1,
        failed: vec![loaders::FailedMutation {
          id: 7,
          kind: "mail.send".to_owned(),
          last_error: "ESI 520".to_owned(),
        }],
      };
      state.render = Some(ReadingRender {
        is_starred: true,
        labels: Vec::new(),
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      });
      state.selected = Some(7);
      state
    }

    fn row(
      mail_id: i64,
      bucket: DayBucket,
      is_read: bool,
      is_starred: bool,
      _unused: bool,
      labels: &[&str],
    ) -> MessageRow {
      MessageRow {
        bucket,
        character_id: 42,
        is_read,
        is_starred,
        has_attachment: false,
        important: false,
        sender_kind: SenderKind::Character,
        label_ids: Vec::new(),
        labels: labels
          .iter()
          .map(|name| MessageLabel {
            color: None,
            name: (*name).to_owned(),
          })
          .collect(),
        mail_id,
        sender: "Vex Voronova".to_owned(),
        sender_id: 95_000_001,
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
        snippet: "Form up at Jita.".to_owned(),
        subject: "CTA tonight".to_owned(),
        time: "10:00".to_owned(),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
      }
    }

    #[test]
    fn it_filters_the_message_list_live_on_search() {
      let mut state = populated_state();
      state.search = "nothing matches".to_owned();
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_every_folder_caption() {
      let mut state = populated_state();
      for folder in [
        Folder::Unified,
        Folder::Standard(StandardFolder::Archive),
        Folder::Standard(StandardFolder::Drafts),
        Folder::Standard(StandardFolder::Inbox),
        Folder::Standard(StandardFolder::Sent),
        Folder::Standard(StandardFolder::Snoozed),
        Folder::Standard(StandardFolder::Starred),
        Folder::Standard(StandardFolder::Trash),
        Folder::Label(99),
        Folder::Label(123),
      ] {
        state.folder = folder;
        let _el: Element<'_, Message> = view(&state);
      }
    }

    #[test]
    fn it_renders_the_account_switcher_dropdown_overlay() {
      let mut state = populated_state();
      state.picker_open = true;
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_detached_compose_window_body() {
      let mut draft = compose::Draft::blank(42);
      draft.to.push(compose::Recipient::typed("Vex Voronova"));
      draft.show_cc = true;
      draft.cc.push(compose::Recipient::typed("Alt Pilot"));
      draft.subject = "CTA".to_owned();
      draft.body = text_editor::Content::with_text("Form up.");
      draft.from_picker_open = true;
      draft.error = Some("enqueue failed".to_owned());

      let _el: Element<'_, Message> = compose::view(&draft, &[]);
    }

    #[test]
    fn it_renders_the_populated_shell_with_an_open_mail() {
      let state = populated_state();
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_snooze_calendar_overlay() {
      let mut state = populated_state();
      state.snooze_menu = SnoozeMenu::Calendar;
      state.snooze_calendar = Some(snooze::Calendar::open(chrono::Utc::now()));
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_snooze_presets_overlay() {
      let mut state = populated_state();
      state.snooze_menu = SnoozeMenu::Presets;
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_three_pane_shell() {
      let state = State::new(42);
      let _el: Element<'_, Message> = view(&state);
    }
  }
}
