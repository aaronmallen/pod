mod compose;
mod folder_pane;
mod loaders;
mod message_list;
mod outbox_indicator;
mod read_state;
mod reading_pane;
mod shell;
mod snooze;
mod switcher;
mod triage;

use std::collections::HashMap;

use iced::{Element, Task, widget::text_editor};

pub use self::loaders::{FolderPaneData, OutboxIndicator, RosterPilot, search_recipients};
use self::message_list::MessageRow;
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
  ui::components::resizable_pane::{self, PaneDrag},
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
  Character(i64),
}

impl Default for Scope {
  fn default() -> Self {
    Scope::Character(EMPTY_MAIL_SELECTION)
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Folder {
  Label(i64),
  Standard(StandardFolder),
  #[default]
  Unified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardFolder {
  Archive,
  Drafts,
  Inbox,
  Sent,
  Snoozed,
  Starred,
  Trash,
}

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  all_messages: Vec<MessageRow>,
  folder: Folder,
  folder_data: FolderPaneData,
  folder_pane_width: f32,
  headers: Vec<CharacterMail>,
  message_list_pane_width: f32,
  messages: Vec<MessageRow>,
  outbox_indicator: OutboxIndicator,
  overlays: HashMap<i64, MailOverlayState>,
  roster: Vec<RosterPilot>,
  scope: Scope,
  unified: Vec<UnifiedMail>,
  unified_unread: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingRender {
  is_starred: bool,
  mail: MailRender,
  sender_portrait: images::ImageState,
}

#[derive(Clone, Debug)]
pub enum Message {
  Archive(i64),
  ComposeBodyChanged(text_editor::Action),
  ComposeCcCommitted,
  ComposeCcInput(String),
  ComposeCcPicked(i64, String),
  ComposeCcRemoved(usize),
  ComposeCcSearched(Vec<(i64, String)>),
  ComposeCcShown,
  ComposeClosed,
  ComposeExpandToggled,
  ComposeFromChanged(i64),
  ComposeFromToggled,
  ComposeMinimizeToggled,
  ComposeOpened,
  ComposeSend,
  ComposeSent(Result<(), String>),
  ComposeSubjectChanged(String),
  ComposeToCommitted,
  ComposeToInput(String),
  ComposeToPicked(i64, String),
  ComposeToRemoved(usize),
  ComposeToSearched(Vec<(i64, String)>),
  FolderPaneDragEnd,
  FolderPaneDragStart,
  FolderPaneDragged(f32),
  FolderSelected(Folder),
  Forward(i64),
  ListPaneDragEnd,
  ListPaneDragStart,
  ListPaneDragged(f32),
  Loaded(Box<Loaded>),
  MarkedRead,
  OutboxDismiss(i64),
  OutboxRefreshed(Box<OutboxIndicator>),
  OutboxRetry(i64),
  OverlayWritten,
  PaneSettled(&'static str, f32),
  PickerToggled,
  RenderLoaded(Box<Option<ReadingRender>>),
  Reply(i64),
  ReplyAll(i64),
  ScopeSelected(Scope),
  SearchChanged(String),
  Selected(i64),
  SnoozeCalendarBack,
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
  TogglePin(i64),
  ToggleStar(i64),
  Trash(i64),
  Unsnooze(i64),
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  all_messages: Vec<MessageRow>,
  compose: Option<compose::Draft>,
  folder: Folder,
  folder_data: FolderPaneData,
  folder_pane: PaneDrag,
  headers: Vec<CharacterMail>,
  message_list_pane: PaneDrag,
  messages: Vec<MessageRow>,
  outbox_indicator: OutboxIndicator,
  overlays: HashMap<i64, MailOverlayState>,
  picker_open: bool,
  render: Option<ReadingRender>,
  roster: Vec<RosterPilot>,
  search: String,
  selected: Option<i64>,
  snooze_calendar: Option<snooze::Calendar>,
  snooze_menu: SnoozeMenu,
  unified: Vec<UnifiedMail>,
  unified_unread: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SnoozeMenu {
  Calendar,
  #[default]
  Closed,
  Presets,
}

impl State {
  pub fn new(active: i64) -> Self {
    State {
      active: Scope::Character(active),
      all_messages: Vec::new(),
      compose: None,
      folder: Folder::default(),
      folder_data: FolderPaneData::default(),
      folder_pane: PaneDrag::with_min_width(
        FOLDER_PANE_DEFAULT_WIDTH,
        FOLDER_PANE_MIN_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      ),
      headers: Vec::new(),
      message_list_pane: PaneDrag::with_min_width(
        MESSAGE_LIST_PANE_DEFAULT_WIDTH,
        MESSAGE_LIST_PANE_MIN_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      ),
      messages: Vec::new(),
      outbox_indicator: OutboxIndicator::default(),
      overlays: HashMap::new(),
      picker_open: false,
      render: None,
      roster: Vec::new(),
      search: String::new(),
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

  pub fn compose_from_character(&self) -> Option<i64> {
    self.compose.as_ref().map(|draft| draft.from_character_id)
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

  pub fn compose(&self) -> Option<&compose::Draft> {
    self.compose.as_ref()
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

  fn default_from(&self) -> Option<i64> {
    let Scope::Character(id) = self.active;
    Some(id)
  }
}

impl Default for State {
  fn default() -> Self {
    Self::new(EMPTY_MAIL_SELECTION)
  }
}

fn restore_pane(ui: &UiState, key: &str, default: f32, min: f32, host_width: f32) -> PaneDrag {
  PaneDrag::from_store_with_min(ui, key, default, min, host_width)
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
  Some(ReadingRender {
    is_starred,
    mail,
    sender_portrait,
  })
}

fn reload_for(db: &Database, scope: Scope, folder: Folder) -> Task<Message> {
  Task::perform(load_mail(db.clone(), scope, folder), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
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

fn open_reply(state: &mut State, mail_id: i64, kind: compose::Kind) -> Task<Message> {
  let Some(render) = state.render.as_ref() else {
    return Task::none();
  };
  if render.mail.header.mail_id() != mail_id {
    return Task::none();
  }
  state.snooze_menu = SnoozeMenu::Closed;
  state.compose = Some(compose::Draft::from_mail(kind, &render.mail));
  Task::none()
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::Loaded(loaded) => handle_loaded(state, *loaded, db),
    Message::ScopeSelected(scope) => {
      state.active = scope;
      state.folder = Folder::Unified;
      state.selected = None;
      state.render = None;
      state.picker_open = false;
      state.snooze_menu = SnoozeMenu::Closed;
      state.snooze_calendar = None;
      Task::none()
    }
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::FolderPaneDragStart
    | Message::FolderPaneDragged(_)
    | Message::FolderPaneDragEnd
    | Message::ListPaneDragStart
    | Message::ListPaneDragged(_)
    | Message::ListPaneDragEnd
    | Message::PaneSettled(..) => update_pane_drag(state, message),
    Message::FolderSelected(folder) => {
      state.folder = folder;
      state.selected = None;
      state.render = None;
      state.snooze_menu = SnoozeMenu::Closed;
      state.snooze_calendar = None;
      reload_for(db, state.active, folder)
    }
    Message::SearchChanged(query) => {
      state.search = query;
      Task::none()
    }
    Message::Selected(mail_id) => handle_message_selected(state, mail_id, db),
    Message::RenderLoaded(render) => {
      state.render = *render;
      Task::none()
    }
    Message::MarkedRead => reload_for(db, state.active, state.folder),
    Message::ToggleStar(mail_id) => triage_write(state, db, mail_id, triage::toggle_star),
    Message::TogglePin(mail_id) => triage_write(state, db, mail_id, triage::toggle_pin),
    Message::Archive(mail_id) => triage_write(state, db, mail_id, triage::archive),
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

    Message::Reply(mail_id) => open_reply(state, mail_id, compose::Kind::Reply),
    Message::ReplyAll(mail_id) => open_reply(state, mail_id, compose::Kind::ReplyAll),
    Message::Forward(mail_id) => open_reply(state, mail_id, compose::Kind::Forward),
    Message::ComposeToInput(_)
    | Message::ComposeCcInput(_)
    | Message::ComposeToSearched(_)
    | Message::ComposeCcSearched(_)
    | Message::ComposeToCommitted
    | Message::ComposeCcCommitted
    | Message::ComposeToPicked(..)
    | Message::ComposeCcPicked(..)
    | Message::ComposeToRemoved(_)
    | Message::ComposeCcRemoved(_)
    | Message::ComposeCcShown
    | Message::ComposeSubjectChanged(_)
    | Message::ComposeBodyChanged(_)
    | Message::ComposeFromChanged(_) => update_compose_fields(state, message),
    Message::ComposeOpened
    | Message::ComposeFromToggled
    | Message::ComposeExpandToggled
    | Message::ComposeMinimizeToggled
    | Message::ComposeClosed
    | Message::ComposeSend
    | Message::ComposeSent(_) => update_compose(state, message, db),
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
  }
}

fn handle_loaded(state: &mut State, loaded: Loaded, db: &Database) -> Task<Message> {
  let Loaded {
    all_messages,
    folder,
    folder_data,
    folder_pane_width,
    headers,
    message_list_pane_width,
    messages,
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
    state.all_messages = all_messages;
    state.folder_data = folder_data;
    state.headers = headers;
    state.overlays = overlays;
    state.messages = messages;
    Task::none()
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
  let Some(character_id) = state
    .messages
    .iter()
    .find(|r| r.mail_id == mail_id)
    .map(|r| r.character_id)
  else {
    state.render = None;
    return Task::none();
  };
  let render = Task::perform(load_render(db.clone(), character_id, mail_id), |render| {
    Message::RenderLoaded(Box::new(render))
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

fn update_compose(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::ComposeOpened => {
      if let Some(from) = state.default_from() {
        state.compose = Some(compose::Draft::blank(from));
      }
      Task::none()
    }
    Message::ComposeFromToggled => {
      if let Some(draft) = state.compose.as_mut() {
        draft.from_picker_open = !draft.from_picker_open;
      }
      Task::none()
    }
    Message::ComposeExpandToggled => {
      if let Some(draft) = state.compose.as_mut() {
        draft.expanded = !draft.expanded;
        draft.minimized = false;
      }
      Task::none()
    }
    Message::ComposeMinimizeToggled => {
      if let Some(draft) = state.compose.as_mut() {
        draft.minimized = !draft.minimized;
      }
      Task::none()
    }
    Message::ComposeClosed => {
      state.compose = None;
      Task::none()
    }
    Message::ComposeSend => {
      let Some(draft) = state.compose.as_ref() else {
        return Task::none();
      };
      if !draft.can_send() {
        return Task::none();
      }
      let draft = draft.clone();
      Task::perform(compose::enqueue_send(db.clone(), draft), Message::ComposeSent)
    }
    Message::ComposeSent(result) => match result {
      Ok(()) => {
        state.compose = None;
        reload_for(db, state.active, state.folder)
      }
      Err(message) => {
        if let Some(draft) = state.compose.as_mut() {
          draft.error = Some(message);
        }
        Task::none()
      }
    },
    _ => Task::none(),
  }
}

fn update_compose_fields(state: &mut State, message: Message) -> Task<Message> {
  let Some(draft) = state.compose.as_mut() else {
    return Task::none();
  };
  match message {
    Message::ComposeToInput(value) => {
      if value.trim().chars().count() < RECIPIENT_SEARCH_MIN_CHARS {
        draft.to_suggestions.clear();
        draft.to_searching = false;
      } else {
        draft.to_searching = true;
      }
      draft.to_input = value;
    }
    Message::ComposeCcInput(value) => {
      if value.trim().chars().count() < RECIPIENT_SEARCH_MIN_CHARS {
        draft.cc_suggestions.clear();
        draft.cc_searching = false;
      } else {
        draft.cc_searching = true;
      }
      draft.cc_input = value;
    }
    Message::ComposeToSearched(results) => {
      draft.to_suggestions = results;
      draft.to_searching = false;
    }
    Message::ComposeCcSearched(results) => {
      draft.cc_suggestions = results;
      draft.cc_searching = false;
    }
    Message::ComposeToCommitted => {
      let name = draft.to_input.trim().to_owned();
      if !name.is_empty() {
        draft.to.push(compose::Recipient::typed(name));
        draft.to_input.clear();
        draft.to_suggestions.clear();
        draft.to_searching = false;
      }
    }
    Message::ComposeCcCommitted => {
      let name = draft.cc_input.trim().to_owned();
      if !name.is_empty() {
        draft.cc.push(compose::Recipient::typed(name));
        draft.cc_input.clear();
        draft.cc_suggestions.clear();
        draft.cc_searching = false;
      }
    }
    Message::ComposeToPicked(id, name) => {
      draft.to.push(compose::Recipient::character(name, id));
      draft.to_input.clear();
      draft.to_suggestions.clear();
      draft.to_searching = false;
    }
    Message::ComposeCcPicked(id, name) => {
      draft.cc.push(compose::Recipient::character(name, id));
      draft.cc_input.clear();
      draft.cc_suggestions.clear();
      draft.cc_searching = false;
    }
    Message::ComposeToRemoved(index) if index < draft.to.len() => {
      draft.to.remove(index);
    }
    Message::ComposeCcRemoved(index) if index < draft.cc.len() => {
      draft.cc.remove(index);
    }
    Message::ComposeCcShown => draft.show_cc = true,
    Message::ComposeSubjectChanged(value) => draft.subject = value,
    Message::ComposeBodyChanged(action) => draft.body.perform(action),
    Message::ComposeFromChanged(character_id) => {
      draft.from_character_id = character_id;
      draft.from_picker_open = false;
    }
    _ => {}
  }
  Task::none()
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

  let messages = message_list::load_messages(&db, scope, folder).await;
  let all_messages = message_list::load_all_messages(&db, scope, folder).await;
  let outbox_indicator = loaders::load_outbox_indicator(&db).await;

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
    all_messages,
    folder,
    folder_data,
    folder_pane_width,
    headers,
    message_list_pane_width,
    messages,
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

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[test]
    fn it_falls_back_to_default_pane_widths_when_unsized() {
      let state = State::new(42).with_restored_panes(&UiState::default());

      assert_eq!(state.folder_pane_width(), FOLDER_PANE_DEFAULT_WIDTH);
      assert_eq!(state.message_list_pane_width(), MESSAGE_LIST_PANE_DEFAULT_WIDTH);
    }

    #[test]
    fn it_clamps_a_restored_folder_width_to_its_80px_minimum() {
      let mut ui = UiState::default();
      ui.panes.insert(FOLDER_PANE_KEY.to_owned(), 40.0);
      ui.panes.insert(MESSAGE_LIST_PANE_KEY.to_owned(), 60.0);

      let state = State::new(42).with_restored_panes(&ui);

      assert_eq!(state.folder_pane_width(), FOLDER_PANE_MIN_WIDTH);
      assert_eq!(state.message_list_pane_width(), MESSAGE_LIST_PANE_MIN_WIDTH);
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
        is_pinned: false,
        is_read: true,
        is_starred: false,
        has_attachment: false,
        important: false,
        sender_kind: SenderKind::Character,
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
      }
    }

    #[test]
    fn it_is_empty_for_a_fresh_default_state() {
      let state = State::new(42);

      assert!(state.stale_images().is_empty());
    }

    #[test]
    fn it_collects_stale_keys_from_the_roster_messages_and_open_render() {
      let mut state = State::new(42);
      state.roster = vec![RosterPilot {
        corp: "VEX".to_owned(),
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
    fn it_omits_a_fresh_portrait_and_a_non_positive_sender_id() {
      let mut state = State::new(42);
      state.roster = vec![RosterPilot {
        corp: "VEX".to_owned(),
        id: 42,
        name: "Vex".to_owned(),
        portrait: images::ImageState::Fresh("/cache/42.jpg".into()),
        unread: 0,
      }];
      state.messages = vec![message_row(7, 0)];

      assert_eq!(state.stale_images(), Vec::new());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_records_a_scope_selection() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(42)), &db);

      assert_eq!(state.active(), Scope::Character(42));
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
    async fn it_stores_a_landed_reading_pane_render() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      let render = ReadingRender {
        is_starred: true,
        mail: sample_render(),
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      };

      let _ = update(&mut state, Message::RenderLoaded(Box::new(Some(render.clone()))), &db);

      assert_eq!(state.render(), Some(&render));
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
    async fn it_does_not_adopt_a_stale_scope_loads_scope_specific_picture() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.active = Scope::Character(42);
      state.folder_data = FolderPaneData {
        labels: vec![loaders::FolderLabel {
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
    async fn it_clears_the_open_render_when_the_folder_changes() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.render = Some(ReadingRender {
        is_starred: false,
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

    fn list_row(mail_id: i64, character_id: i64, is_read: bool) -> message_list::MessageRow {
      message_list::MessageRow {
        bucket: message_list::DayBucket::Today,
        character_id,
        is_pinned: false,
        is_read,
        is_starred: false,
        has_attachment: false,
        important: false,
        sender_kind: message_list::SenderKind::Character,
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
      }
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
    async fn it_clears_the_render_when_selecting_a_row_no_longer_in_the_list() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.render = Some(ReadingRender {
        is_starred: false,
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
    async fn it_resolves_the_owning_character_for_an_action() {
      let mut state = State::new(42);
      state.active = Scope::Character(42);
      state.render = Some(ReadingRender {
        is_starred: false,
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
    async fn it_toggles_the_snooze_preset_menu() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::SnoozeMenuToggled, &db);
      assert!(state.snooze_presets_open());
      let _ = update(&mut state, Message::SnoozeMenuToggled, &db);
      assert!(!state.snooze_presets_open());
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
    async fn it_opens_and_edits_a_compose_draft() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.roster = vec![RosterPilot {
        corp: "VEX".to_owned(),
        id: 42,
        name: "Vex".to_owned(),
        portrait: images::ImageState::Stale {
          id: 42,
          kind: images::ImageKind::CharacterPortrait,
        },
        unread: 0,
      }];

      let _ = update(&mut state, Message::ComposeOpened, &db);
      assert!(state.compose().is_some());

      let _ = update(&mut state, Message::ComposeToInput("Vex Voronova".to_owned()), &db);
      let _ = update(
        &mut state,
        Message::ComposeToPicked(95_000_001, "Vex Voronova".to_owned()),
        &db,
      );
      assert_eq!(state.compose().unwrap().to[0].id, Some(95_000_001));
      assert!(state.compose().unwrap().to_input.is_empty());
      let _ = update(&mut state, Message::ComposeToRemoved(0), &db);
      let _ = update(&mut state, Message::ComposeToInput("Vex Voronova".to_owned()), &db);
      let _ = update(&mut state, Message::ComposeToCommitted, &db);
      let _ = update(&mut state, Message::ComposeCcShown, &db);
      let _ = update(&mut state, Message::ComposeCcPicked(95_000_009, "Alt".to_owned()), &db);
      assert_eq!(state.compose().unwrap().cc[0].id, Some(95_000_009));
      let _ = update(&mut state, Message::ComposeCcRemoved(0), &db);
      let _ = update(&mut state, Message::ComposeCcInput("Alt".to_owned()), &db);
      let _ = update(&mut state, Message::ComposeCcCommitted, &db);
      let _ = update(&mut state, Message::ComposeSubjectChanged("CTA".to_owned()), &db);
      let _ = update(
        &mut state,
        Message::ComposeBodyChanged(text_editor::Action::Edit(text_editor::Edit::Paste(
          std::sync::Arc::new("Form up.".to_owned()),
        ))),
        &db,
      );
      let _ = update(&mut state, Message::ComposeFromChanged(42), &db);
      let draft = state.compose().unwrap();
      assert_eq!(draft.to.len(), 1);
      assert_eq!(draft.cc.len(), 1);
      assert_eq!(draft.subject, "CTA");
      assert_eq!(draft.body.text(), "Form up.");

      let _ = update(&mut state, Message::ComposeToRemoved(0), &db);
      assert!(state.compose().unwrap().to.is_empty());

      let _ = update(&mut state, Message::ComposeExpandToggled, &db);
      assert!(state.compose().unwrap().expanded);
      let _ = update(&mut state, Message::ComposeMinimizeToggled, &db);
      let _ = update(&mut state, Message::ComposeFromToggled, &db);

      let _ = update(&mut state, Message::ComposeClosed, &db);
      assert!(state.compose().is_none());
    }

    #[tokio::test]
    async fn it_keeps_a_blocked_send_open_with_an_unsendable_draft() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.compose = Some(compose::Draft::blank(42));

      let _ = update(&mut state, Message::ComposeSend, &db);

      assert!(state.compose().is_some());
    }

    #[tokio::test]
    async fn it_surfaces_a_failed_send_inline_and_closes_on_success() {
      let mut state = State::new(42);
      let db = crate::store::open_test().await.unwrap();
      state.compose = Some(compose::Draft::blank(42));

      let _ = update(&mut state, Message::ComposeSent(Err("boom".to_owned())), &db);
      assert_eq!(state.compose().unwrap().error.as_deref(), Some("boom"));

      let _ = update(&mut state, Message::ComposeSent(Ok(())), &db);
      assert!(state.compose().is_none());
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
        Message::TogglePin(7),
        Message::Archive(7),
        Message::Trash(7),
        Message::Reply(7),
        Message::ReplyAll(7),
        Message::Forward(7),
        Message::OutboxRetry(1),
        Message::OutboxDismiss(1),
        Message::SearchChanged("cta".to_owned()),
        Message::RenderLoaded(Box::new(None)),
        Message::PaneSettled(FOLDER_PANE_KEY, 240.0),
      ] {
        let _ = update(&mut state, message, &db);
      }

      assert_eq!(state.search(), "cta");
      assert!(state.render().is_none());
    }
  }

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

  mod view {
    use super::*;
    use crate::features::mail::message_list::{DayBucket, MessageRow, SenderKind};

    fn populated_state() -> State {
      let mut state = State::new(42);
      state.active = Scope::Character(42);
      state.roster = vec![
        RosterPilot {
          corp: "VEX".to_owned(),
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
        row(1, DayBucket::Today, true, false, false, false, &[]),
        row(2, DayBucket::Today, false, true, false, false, &["Fleet"]),
        row(3, DayBucket::Yesterday, false, false, true, false, &[]),
        row(4, DayBucket::Earlier, false, false, false, true, &["Ops", "Fleet"]),
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
      is_pinned: bool,
      is_starred: bool,
      _unused: bool,
      labels: &[&str],
    ) -> MessageRow {
      MessageRow {
        bucket,
        character_id: 42,
        is_pinned,
        is_read,
        is_starred,
        has_attachment: false,
        important: false,
        sender_kind: SenderKind::Character,
        labels: labels.iter().map(|l| (*l).to_owned()).collect(),
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
      }
    }

    #[test]
    fn it_renders_the_three_pane_shell() {
      let state = State::new(42);
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_populated_shell_with_an_open_mail() {
      let state = populated_state();
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_account_switcher_dropdown_overlay() {
      let mut state = populated_state();
      state.picker_open = true;
      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_snooze_presets_overlay() {
      let mut state = populated_state();
      state.snooze_menu = SnoozeMenu::Presets;
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
    fn it_renders_the_compose_panel_overlay() {
      let mut state = populated_state();
      let mut draft = compose::Draft::blank(42);
      draft.to.push(compose::Recipient::typed("Vex Voronova"));
      draft.show_cc = true;
      draft.cc.push(compose::Recipient::typed("Alt Pilot"));
      draft.subject = "CTA".to_owned();
      draft.body = text_editor::Content::with_text("Form up.");
      draft.from_picker_open = true;
      draft.error = Some("enqueue failed".to_owned());
      state.compose = Some(draft);
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
    fn it_filters_the_message_list_live_on_search() {
      let mut state = populated_state();
      state.search = "nothing matches".to_owned();
      let _el: Element<'_, Message> = view(&state);
    }
  }
}
