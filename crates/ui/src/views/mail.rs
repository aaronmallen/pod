//! Mail controller and view: three-pane EVE mail client.

pub mod compose_overlay;
pub mod folder_pane;
pub mod message_list_pane;
pub mod reading_pane;
pub mod snooze_picker;

use std::collections::HashMap;

pub use compose_panel::ComposeRecipient;
use iced::{
  Background, Border, Element, Event, Length, Padding, Subscription, Theme, keyboard, mouse,
  widget::{Space, button, column, container, image, mouse_area, row, stack, text},
};
use pod_model::missing_scopes;

use crate::{
  components::{CharacterPicker, ComposePanel, Icon, ScopeMissing, character_picker, compose_panel, scope_missing},
  style::{
    color, spacing,
    typography::{body, mono},
  },
};

/// A single mailbox account.
#[derive(Clone, Debug)]
pub struct MailAccount {
  pub id: i64,
  pub name: String,
  pub corp: String,
  pub tone: u16,
  pub unread: u32,
}

/// A single mail message (header only — body fetched on demand).
#[derive(Clone, Debug)]
pub struct MailMessage {
  pub character_id: i64,
  pub mail_id: i64,
  pub from_id: Option<i64>,
  pub id: String,
  pub folder: String,
  pub from_name: String,
  pub from_tone: u16,
  pub from_corp: bool,
  pub from_system: bool,
  pub subject: String,
  pub preview: String,
  pub body: Vec<String>,
  pub body_loaded: bool,
  pub time: String,
  pub date_label: String,
  pub unread: bool,
  pub starred: bool,
  pub pinned: bool,
  pub has_attachment: bool,
  pub labels: Vec<String>,
  pub important: bool,
  pub snoozed: Option<String>,
  pub recipients_display: String,
}

/// Selectable folder or virtual view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Folder {
  All,
  Inbox,
  Starred,
  Snoozed,
  Sent,
  Drafts,
  Archive,
  Trash,
  Label(String),
}

/// Which drag handle is currently being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraggingPane {
  FolderList,
  MessageReader,
}

/// Messages produced by the mail controller.
#[derive(Clone, Debug)]
pub enum Message {
  AccountPicker(character_picker::Message),
  Compose(compose_panel::Message),
  ComposePressed,
  FolderPane(folder_pane::Message),
  MailBodyLoaded(String, Vec<String>),
  MailDeleted,
  MailHeadersLoaded(Result<Vec<MailMessage>, String>),
  MessageList(message_list_pane::Message),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart(DraggingPane),
  ReadingPane(reading_pane::Message),
  ReauthorizeCharacter(i64),
}

/// Runtime state for the mail controller.
pub struct State {
  pub accounts: Vec<MailAccount>,
  pub characters: Vec<pod_model::Character>,
  pub portrait_handles: HashMap<i64, image::Handle>,
  pub account_picker: CharacterPicker,
  pub compose: ComposePanel,
  pub compose_open: bool,
  pub context_menu: Option<(String, f32, f32)>,
  pub cursor_pos: (f32, f32),
  pub dragging_pane: Option<DraggingPane>,
  pub folder_pane_width: f32,
  pub last_drag_x: f32,
  pub message_list_width: f32,
  pub messages: Vec<MailMessage>,
  pub search_query: String,
  pub selected_folder: Folder,
  pub selected_message_id: Option<String>,
  /// When `Some`, the calendar date/time picker is open.
  pub snooze_calendar: Option<snooze_picker::CalendarState>,
  /// Whether the snooze preset dropdown is open.
  pub snooze_popover_open: bool,
}

impl State {
  pub fn current_account_id(&self) -> i64 {
    self.account_picker.selected_character_id().unwrap_or(0)
  }
}

/// View title for the mail section.
pub fn title() -> &'static str {
  "Mail"
}

/// Returns a subscription that handles pane dragging, context menu, suggestion keyboard nav, and snooze timer.
pub fn subscription(state: &State) -> Subscription<Message> {
  if state.dragging_pane.is_some() {
    return pane_drag_subscription();
  }

  let mut subs: Vec<Subscription<Message>> = Vec::new();

  if state.context_menu.is_some() {
    subs.push(context_menu_dismiss_subscription());
  }
  if let Some(s) = compose_keyboard_subscription(state) {
    subs.push(s);
  }
  if state.messages.iter().any(|m| m.snoozed.is_some()) {
    subs.push(snooze_timer_subscription());
  }

  if subs.is_empty() {
    Subscription::none()
  } else {
    Subscription::batch(subs)
  }
}

fn pane_drag_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, _status, _id| match event {
    Event::Mouse(mouse::Event::CursorMoved {
      position,
    }) => Some(Message::PaneDrag(position.x)),
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::PaneDragEnd),
    _ => None,
  })
}

fn context_menu_dismiss_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, _status, _id| match event {
    Event::Mouse(mouse::Event::ButtonPressed(_)) => {
      Some(Message::MessageList(message_list_pane::Message::ContextMenuClose))
    }
    _ => None,
  })
}

fn compose_suggestion_key_event(event: Event) -> Option<Message> {
  let Event::Keyboard(keyboard::Event::KeyPressed {
    key, ..
  }) = event
  else {
    return None;
  };
  match key {
    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
      Some(Message::Compose(compose_panel::Message::SuggestionCursorMove(-1)))
    }
    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
      Some(Message::Compose(compose_panel::Message::SuggestionCursorMove(1)))
    }
    keyboard::Key::Named(keyboard::key::Named::Enter) => {
      Some(Message::Compose(compose_panel::Message::SuggestionCursorConfirm))
    }
    _ => None,
  }
}

fn compose_keyboard_subscription(state: &State) -> Option<Subscription<Message>> {
  if !state.compose_open {
    return None;
  }
  let to_active = !state.compose.to_suggestions.is_empty() && !state.compose.to_search.is_empty();
  let cc_active =
    state.compose.cc_visible && !state.compose.cc_suggestions.is_empty() && !state.compose.cc_search.is_empty();
  if !to_active && !cc_active {
    return None;
  }
  Some(iced::event::listen_with(|event, _status, _id| {
    compose_suggestion_key_event(event)
  }))
}

fn snooze_timer_subscription() -> Subscription<Message> {
  iced::time::every(std::time::Duration::from_secs(60))
    .map(|_| Message::ReadingPane(reading_pane::Message::CheckSnoozed))
}

/// Builder for the mail view.
pub struct Component<'a> {
  state: &'a State,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Create a new view builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_width: 1200.0,
    }
  }

  /// Set the window width so pane widths can be clamped correctly.
  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;

    if let Some(el) = mail_scope_gate(state) {
      return el;
    }

    let base = mail_base(state, self.window_width);
    let mut layers: Vec<Element<'_, Message>> = vec![base];

    for overlay in mail_overlays(state) {
      layers.push(overlay);
    }
    if state.dragging_pane.is_some() {
      layers.push(mail_drag_capture_layer());
    }

    if layers.len() == 1 {
      layers.remove(0)
    } else {
      stack(layers).into()
    }
  }
}

fn mail_scope_gate(state: &State) -> Option<Element<'_, Message>> {
  let account_id = state.current_account_id();
  if account_id == 0 {
    return None;
  }
  let character = state.characters.iter().find(|c| *c.id() == account_id)?;
  let granted = character.granted_scopes_list();
  if missing_scopes(&granted, &["esi-mail.read_mail.v1"]).is_empty() {
    return None;
  }
  Some(ScopeMissing::new(account_id, "mail").render().map(|m| match m {
    scope_missing::Message::ReauthorizePressed(id) => Message::ReauthorizeCharacter(id),
  }))
}

fn mail_base<'a>(state: &'a State, window_width: f32) -> Element<'a, Message> {
  let folder_w = effective_folder_width(state, window_width);
  let msg_w = effective_message_list_width(state, window_width, folder_w);
  let header = mail_header(state);
  let panes = row([
    folder_pane::Component::new(state)
      .width(folder_w)
      .render()
      .map(Message::FolderPane),
    folder_list_drag_handle(),
    message_list_pane::Component::new(state)
      .width(msg_w)
      .render()
      .map(Message::MessageList),
    message_reader_drag_handle(),
    reading_pane::Component::new(state).render().map(Message::ReadingPane),
  ])
  .width(Length::Fill)
  .height(Length::Fill);
  column([header, panes.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn mail_overlays(state: &State) -> Vec<Element<'_, Message>> {
  let mut layers: Vec<Element<'_, Message>> = Vec::new();
  if let Some(o) = account_picker_overlay(state) {
    layers.push(o);
  }
  if let Some(o) = compose_panel_overlay(state) {
    layers.push(o);
  }
  if let Some(o) = context_menu_overlay(state) {
    layers.push(o);
  }
  layers
}

fn mail_drag_capture_layer() -> Element<'static, Message> {
  mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
    .on_move(|pt| Message::PaneDrag(pt.x))
    .on_release(Message::PaneDragEnd)
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn effective_folder_width(state: &State, window_width: f32) -> f32 {
  let min_msg = 200.0;
  let min_read = 200.0;
  let handle = 4.0;
  let content = window_width - (spacing::layout::RAIL_WIDTH + 1.0);
  let max = (content - handle - min_msg - handle - min_read).max(80.0);
  state.folder_pane_width.clamp(80.0, max)
}

fn effective_message_list_width(state: &State, window_width: f32, folder_w: f32) -> f32 {
  let min_read = 200.0;
  let handle = 4.0;
  let content = window_width - (spacing::layout::RAIL_WIDTH + 1.0);
  let max = (content - folder_w - handle - handle - min_read).max(100.0);
  state.message_list_width.clamp(100.0, max)
}

fn drag_handle_inner() -> Element<'static, Message> {
  row([
    Space::new().width(1.5).height(Length::Fill).into(),
    container(Space::new().width(1.0).height(Length::Fill))
      .width(1.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into(),
    Space::new().width(1.5).height(Length::Fill).into(),
  ])
  .width(4.0)
  .height(Length::Fill)
  .into()
}

fn folder_list_drag_handle() -> Element<'static, Message> {
  mouse_area(drag_handle_inner())
    .on_press(Message::PaneDragStart(DraggingPane::FolderList))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn message_reader_drag_handle() -> Element<'static, Message> {
  mouse_area(drag_handle_inner())
    .on_press(Message::PaneDragStart(DraggingPane::MessageReader))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn account_picker_overlay(state: &State) -> Option<Element<'_, Message>> {
  if !state.account_picker.is_open {
    return None;
  }
  let dropdown = state.account_picker.dropdown().map(Message::AccountPicker);
  Some(
    container(dropdown)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(iced::alignment::Horizontal::Left)
      .padding(Padding {
        top: spacing::layout::HEADER_HEIGHT + 8.0,
        left: spacing::SPACE_8,
        ..Padding::ZERO
      })
      .into(),
  )
}

fn compose_panel_overlay(state: &State) -> Option<Element<'_, Message>> {
  if !state.compose_open {
    return None;
  }
  Some(compose_overlay::Component::new(&state.compose, state.compose.expanded).render())
}

fn context_menu_overlay(state: &State) -> Option<Element<'_, Message>> {
  let (msg_id, x, y) = state.context_menu.as_ref()?;
  let msg = state.messages.iter().find(|m| &m.id == msg_id)?;
  let starred = msg.starred;
  let snoozed = msg.snoozed.is_some();
  let star_label = if starred { "Unstar" } else { "Star" };
  let snooze_label = if snoozed { "Unsnooze" } else { "Snooze" };

  let items: Vec<Element<'_, Message>> = vec![
    context_menu_btn("Reply", Message::ReadingPane(reading_pane::Message::ReplyPressed)),
    context_menu_btn(
      "Reply All",
      Message::ReadingPane(reading_pane::Message::ReplyAllPressed),
    ),
    context_menu_btn("Forward", Message::ReadingPane(reading_pane::Message::ForwardPressed)),
    crate::components::Separator::horizontal().render(),
    context_menu_btn(star_label, Message::ReadingPane(reading_pane::Message::StarToggle)),
    context_menu_btn(snooze_label, Message::ReadingPane(reading_pane::Message::SnoozeToggle)),
    context_menu_btn("Archive", Message::ReadingPane(reading_pane::Message::ArchivePressed)),
    crate::components::Separator::horizontal().render(),
    context_menu_danger_btn("Delete", Message::ReadingPane(reading_pane::Message::DeletePressed)),
  ];

  Some(crate::components::ContextMenu::new(items).position(*x, *y).render())
}

fn context_menu_btn(label: &str, msg: Message) -> Element<'_, Message> {
  button(
    text(label.to_string())
      .size(13.0)
      .font(crate::style::typography::body::REGULAR)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: iced::Border {
      radius: 5.0.into(),
      ..iced::Border::default()
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn context_menu_danger_btn(label: &str, msg: Message) -> Element<'_, Message> {
  button(
    text(label.to_string())
      .size(13.0)
      .font(crate::style::typography::body::REGULAR)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      })
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::status::DANGER_FAINT)),
      _ => None,
    },
    border: iced::Border {
      radius: 5.0.into(),
      ..iced::Border::default()
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
  .into()
}

const FOLDER_LABELS: &[(u8, &str)] = &[
  (0, "All Inboxes"),
  (1, "Inbox"),
  (2, "Starred"),
  (3, "Snoozed"),
  (4, "Sent"),
  (5, "Drafts"),
  (6, "Archive"),
  (7, "Trash"),
];

fn folder_discriminant_low(folder: &Folder) -> Option<u8> {
  match folder {
    Folder::All => Some(0),
    Folder::Inbox => Some(1),
    Folder::Starred => Some(2),
    Folder::Snoozed => Some(3),
    Folder::Sent => Some(4),
    _ => None,
  }
}

fn folder_discriminant_high(folder: &Folder) -> u8 {
  match folder {
    Folder::Drafts => 5,
    Folder::Archive => 6,
    Folder::Trash => 7,
    _ => 255,
  }
}

fn folder_discriminant(folder: &Folder) -> u8 {
  folder_discriminant_low(folder).unwrap_or_else(|| folder_discriminant_high(folder))
}

fn folder_display_label(folder: &Folder) -> &'static str {
  let key = folder_discriminant(folder);
  FOLDER_LABELS
    .iter()
    .find(|(k, _)| *k == key)
    .map(|(_, v)| *v)
    .unwrap_or("Label")
}

fn message_matches_folder_kind_ext(m: &MailMessage, folder: &Folder) -> bool {
  match folder {
    Folder::Sent => m.folder == "sent",
    Folder::Snoozed => m.snoozed.is_some(),
    Folder::Starred => m.starred,
    Folder::Trash => m.folder == "trash",
    _ => false,
  }
}

fn message_matches_folder_kind(m: &MailMessage, folder: &Folder) -> bool {
  match folder {
    Folder::All | Folder::Inbox => m.folder == "inbox" && m.snoozed.is_none(),
    Folder::Archive => m.folder == "archive",
    Folder::Drafts => m.folder == "drafts",
    Folder::Label(l) => m.labels.contains(l),
    _ => message_matches_folder_kind_ext(m, folder),
  }
}

fn message_in_folder(m: &MailMessage, folder: &Folder, account_id: i64) -> bool {
  if matches!(folder, Folder::All) {
    return m.folder == "inbox" && m.snoozed.is_none();
  }
  m.character_id == account_id && message_matches_folder_kind(m, folder)
}

fn folder_visible_stats(state: &State) -> (usize, u32) {
  let account_id = state.current_account_id();
  state
    .messages
    .iter()
    .filter(|m| message_in_folder(m, &state.selected_folder, account_id))
    .fold((0usize, 0u32), |(t, u), m| (t + 1, u + u32::from(m.unread)))
}

fn mail_header(state: &State) -> Element<'_, Message> {
  let (total_messages, total_unread) = folder_visible_stats(state);
  let switcher = state.account_picker.render().map(Message::AccountPicker);
  let divider = container(Space::new().width(1.0).height(44.0)).style(|_| container::Style {
    background: Some(Background::Color(color::border::SUBTLE)),
    ..container::Style::default()
  });
  let content = container(
    row([
      switcher,
      divider.into(),
      mail_header_count_col(&state.selected_folder, total_messages, total_unread),
      Space::new().width(Length::Fill).into(),
      mail_header_compose_btn(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .spacing(16.0)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::SPACE_8,
      right: spacing::SPACE_8,
    }),
  )
  .width(Length::Fill)
  .center_y(spacing::layout::HEADER_HEIGHT)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  });
  let border_line = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  column([content.into(), border_line.into()]).width(Length::Fill).into()
}

fn mail_header_count_col(folder: &Folder, total_messages: usize, total_unread: u32) -> Element<'_, Message> {
  let folder_label = folder_display_label(folder);
  let count_row = row([
    text(format!("{total_messages} messages · "))
      .font(mono::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("{total_unread} unread"))
      .font(mono::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
  ]);
  column([
    text(folder_label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    count_row.into(),
  ])
  .spacing(4.0)
  .into()
}

fn compose_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
  let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: active.then(|| Background::Color(color::state::HOVER_OVERLAY)),
    border: Border {
      color: if active {
        color::border::DEFAULT
      } else {
        color::border::SUBTLE
      },
      radius: 8.0.into(),
      width: 1.0,
    },
    text_color: if active {
      color::text::PRIMARY
    } else {
      color::text::SECONDARY
    },
    ..button::Style::default()
  }
}

fn mail_header_compose_btn() -> Element<'static, Message> {
  button(
    row([
      Icon::pencil()
        .size(14.0)
        .color(color::text::SECONDARY)
        .render::<Message>(),
      text("Compose")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(8.0)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
  })
  .on_press(Message::ComposePressed)
  .style(compose_btn_style)
  .into()
}
