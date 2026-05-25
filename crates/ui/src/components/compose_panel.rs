//! Compose panel organism for drafting and sending EVE mail.

pub mod body_area;
pub mod cc_field;
pub mod footer;
pub mod header;
pub mod subject_field;
pub mod suggestions;
pub mod to_field;

use body_area::body_area;
use cc_field::cc_field;
use footer::send_footer_inner;
use header::panel_header;
use iced::{
  Background, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, stack, text, text_editor},
};
use subject_field::subject_field;
use suggestions::Suggestions;
use to_field::to_field;

use crate::{
  components::{
    Card,
    character_picker::{self, CharacterEntry, Component as CharacterPicker, PickerSelection},
  },
  style::{color, component, typography as font},
};

/// A resolved or pending compose recipient.
#[derive(Clone, Debug)]
pub struct ComposeRecipient {
  pub id: Option<i64>,
  pub name: String,
}

/// Messages produced by the compose panel.
#[derive(Clone, Debug)]
pub enum Message {
  BodyAction(text_editor::Action),
  CcAdd,
  CcRemove(usize),
  CcSearchChanged(String),
  CcSearchResults(Vec<(i64, String)>),
  CcSearchSelect(i64, String),
  CcToggle,
  Close,
  Expand,
  FromPicker(character_picker::Message),
  SendPressed,
  Sent(Result<i64, String>),
  SubjectChanged(String),
  SuggestionCursorConfirm,
  SuggestionCursorMove(i32),
  ToAdd,
  ToRemove(usize),
  ToSearchChanged(String),
  ToSearchResults(Vec<(i64, String)>),
  ToSearchSelect(i64, String),
}

/// Stateful compose panel organism.
pub struct Component {
  pub body: text_editor::Content,
  pub cc: Vec<ComposeRecipient>,
  pub cc_search: String,
  pub cc_suggestion_cursor: Option<usize>,
  pub cc_suggestions: Vec<(i64, String)>,
  pub cc_visible: bool,
  pub error: Option<String>,
  pub expanded: bool,
  pub from_picker: CharacterPicker,
  pub sending: bool,
  pub subject: String,
  pub to: Vec<ComposeRecipient>,
  pub to_search: String,
  pub to_suggestion_cursor: Option<usize>,
  pub to_suggestions: Vec<(i64, String)>,
}

impl Component {
  /// Create a new compose panel with default (empty, collapsed) state.
  pub fn new() -> Self {
    Self {
      body: text_editor::Content::new(),
      cc: Vec::new(),
      cc_search: String::new(),
      cc_suggestion_cursor: None,
      cc_suggestions: Vec::new(),
      cc_visible: false,
      error: None,
      expanded: false,
      from_picker: CharacterPicker::new(),
      sending: false,
      subject: String::new(),
      to: Vec::new(),
      to_search: String::new(),
      to_suggestion_cursor: None,
      to_suggestions: Vec::new(),
    }
  }

  /// Builder: set the from-picker entries.
  pub fn from_entries(mut self, entries: Vec<CharacterEntry>) -> Self {
    self.from_picker = self.from_picker.entries(entries);
    self
  }

  /// Returns the currently selected from-character ID (from the picker).
  pub fn from_id(&self) -> Option<i64> {
    self.from_picker.selected_character_id()
  }

  /// Builder: set the currently selected from-character ID.
  pub fn from_selected(mut self, id: Option<i64>) -> Self {
    let sel = id.map(PickerSelection::Character).unwrap_or(PickerSelection::All);
    self.from_picker = self.from_picker.selected(sel);
    self
  }

  /// Render the compose panel at the appropriate size.
  pub fn render(&self) -> Element<'_, Message> {
    let (panel_width, panel_height): (f32, f32) = if self.expanded {
      (
        component::compose_panel::EXPANDED_WIDTH,
        component::compose_panel::EXPANDED_HEIGHT,
      )
    } else {
      (
        component::compose_panel::COLLAPSED_WIDTH,
        component::compose_panel::COLLAPSED_HEIGHT,
      )
    };

    let expand_sym: &'static str = if self.expanded { "⤡" } else { "⤢" };

    let header = panel_header(expand_sym);

    let mut panel_rows: Vec<Element<'_, Message>> = vec![header, to_field(self)];
    if self.cc_visible {
      panel_rows.push(cc_field(self));
    }
    panel_rows.push(subject_field(&self.subject));
    panel_rows.push(body_area(&self.body));
    if let Some(err) = &self.error {
      panel_rows.push(error_row(err.as_str()));
    }
    let from_trigger = self.from_picker.render().map(Message::FromPicker);
    let can_send = !self.to.is_empty() && !self.subject.trim().is_empty();
    panel_rows.push(send_footer_inner(can_send, self.sending, from_trigger));

    let base = column(panel_rows).width(Length::Fill).height(Length::Fill);

    let to_overlay = Suggestions::new(&self.to_suggestions, self.to_suggestion_cursor, |id, name| {
      Message::ToSearchSelect(id, name)
    })
    .top_padding(82.0)
    .visible(!self.to_suggestions.is_empty() && !self.to_search.is_empty())
    .render();

    let cc_overlay = Suggestions::new(&self.cc_suggestions, self.cc_suggestion_cursor, |id, name| {
      Message::CcSearchSelect(id, name)
    })
    .top_padding(123.0)
    .visible(self.cc_visible && !self.cc_suggestions.is_empty() && !self.cc_search.is_empty())
    .render();

    let from_picker_el = from_picker_overlay(self);

    let inner: Element<'_, Message> = stack([base.into(), to_overlay, cc_overlay, from_picker_el]).into();

    Card::new(inner)
      .width(Length::Fixed(panel_width))
      .height(Length::Fixed(panel_height))
      .render()
  }

  /// Reset compose state for a fresh new message.
  pub fn reset(&mut self) {
    self.body = text_editor::Content::new();
    self.cc.clear();
    self.cc_search.clear();
    self.cc_suggestion_cursor = None;
    self.cc_suggestions.clear();
    self.cc_visible = false;
    self.error = None;
    self.expanded = false;
    self.from_picker.is_open = false;
    self.sending = false;
    self.subject.clear();
    self.to.clear();
    self.to_search.clear();
    self.to_suggestion_cursor = None;
    self.to_suggestions.clear();
  }

  /// Process a panel message, mutating internal state.
  pub fn update(&mut self, msg: Message) {
    match msg {
      Message::BodyAction(action) => self.body.perform(action),
      Message::Close => self.apply_close(),
      Message::Expand => self.expanded = !self.expanded,
      Message::FromPicker(msg) => self.from_picker.update(msg),
      Message::SubjectChanged(val) => self.subject = val,
      Message::SuggestionCursorConfirm => self.apply_cursor_confirm(),
      Message::SuggestionCursorMove(delta) => self.apply_cursor_move(delta),
      msg => self.apply_field_message(msg),
    }
  }

  fn apply_cc_add(&mut self) {
    let name = self.cc_search.trim().to_string();
    if !name.is_empty() {
      self.cc.push(ComposeRecipient {
        id: None,
        name,
      });
      self.cc_search.clear();
      self.cc_suggestion_cursor = None;
      self.cc_suggestions.clear();
    }
  }

  fn apply_cc_or_send_message(&mut self, msg: Message) {
    match msg {
      Message::CcAdd => self.apply_cc_add(),
      Message::CcRemove(idx) if idx < self.cc.len() => {
        self.cc.remove(idx);
      }
      Message::CcSearchChanged(val) => self.apply_cc_search_changed(val),
      Message::CcSearchResults(results) => self.apply_cc_search_results(results),
      Message::CcSearchSelect(id, name) => self.apply_cc_select(id, name),
      Message::CcToggle => self.cc_visible = !self.cc_visible,
      msg => self.apply_send_message(msg),
    }
  }

  fn apply_cc_search_changed(&mut self, val: String) {
    if val.is_empty() {
      self.cc_suggestion_cursor = None;
      self.cc_suggestions.clear();
    }
    self.cc_search = val;
  }

  fn apply_cc_search_results(&mut self, results: Vec<(i64, String)>) {
    self.cc_suggestion_cursor = None;
    self.cc_suggestions = results;
  }

  fn apply_cc_select(&mut self, id: i64, name: String) {
    self.cc.push(ComposeRecipient {
      id: Some(id),
      name,
    });
    self.cc_search.clear();
    self.cc_suggestion_cursor = None;
    self.cc_suggestions.clear();
  }

  fn apply_close(&mut self) {
    self.cc_suggestion_cursor = None;
    self.cc_suggestions.clear();
    self.error = None;
    self.from_picker.is_open = false;
    self.sending = false;
    self.to_suggestion_cursor = None;
    self.to_suggestions.clear();
  }

  fn apply_cursor_confirm(&mut self) {
    let to_active = !self.to_suggestions.is_empty() && !self.to_search.is_empty();
    let cc_active = self.cc_visible && !self.cc_suggestions.is_empty() && !self.cc_search.is_empty();
    if to_active {
      self.confirm_to_suggestion();
    } else if cc_active {
      self.confirm_cc_suggestion();
    }
  }

  fn apply_cursor_move(&mut self, delta: i32) {
    let to_active = !self.to_suggestions.is_empty() && !self.to_search.is_empty();
    let cc_active = self.cc_visible && !self.cc_suggestions.is_empty() && !self.cc_search.is_empty();
    if to_active {
      let len = self.to_suggestions.len();
      let cur = self.to_suggestion_cursor.unwrap_or(if delta > 0 { len - 1 } else { 0 });
      self.to_suggestion_cursor = Some((cur as i32 + delta).rem_euclid(len as i32) as usize);
    } else if cc_active {
      let len = self.cc_suggestions.len();
      let cur = self.cc_suggestion_cursor.unwrap_or(if delta > 0 { len - 1 } else { 0 });
      self.cc_suggestion_cursor = Some((cur as i32 + delta).rem_euclid(len as i32) as usize);
    }
  }

  fn apply_field_message(&mut self, msg: Message) {
    match msg {
      Message::ToAdd => self.apply_to_add(),
      Message::ToRemove(idx) if idx < self.to.len() => {
        self.to.remove(idx);
      }
      Message::ToSearchChanged(val) => self.apply_to_search_changed(val),
      Message::ToSearchResults(results) => {
        self.to_suggestion_cursor = None;
        self.to_suggestions = results;
      }
      Message::ToSearchSelect(id, name) => self.apply_to_select(id, name),
      msg => self.apply_cc_or_send_message(msg),
    }
  }

  fn apply_send_message(&mut self, msg: Message) {
    match msg {
      Message::SendPressed => {
        self.error = None;
        self.sending = true;
      }
      Message::Sent(Ok(_)) => {
        self.sending = false;
        self.reset();
      }
      Message::Sent(Err(e)) => {
        self.error = Some(e);
        self.sending = false;
      }
      _ => {}
    }
  }

  fn apply_to_add(&mut self) {
    let name = self.to_search.trim().to_string();
    if !name.is_empty() {
      self.to.push(ComposeRecipient {
        id: None,
        name,
      });
      self.to_search.clear();
      self.to_suggestion_cursor = None;
      self.to_suggestions.clear();
    }
  }

  fn apply_to_search_changed(&mut self, val: String) {
    self.from_picker.is_open = false;
    if val.is_empty() {
      self.to_suggestion_cursor = None;
      self.to_suggestions.clear();
    }
    self.to_search = val;
  }

  fn apply_to_select(&mut self, id: i64, name: String) {
    self.to.push(ComposeRecipient {
      id: Some(id),
      name,
    });
    self.to_search.clear();
    self.to_suggestion_cursor = None;
    self.to_suggestions.clear();
  }

  fn confirm_cc_suggestion(&mut self) {
    if let Some(i) = self.cc_suggestion_cursor
      && let Some((id, name)) = self.cc_suggestions.get(i).cloned()
    {
      self.cc.push(ComposeRecipient {
        id: Some(id),
        name,
      });
      self.cc_search.clear();
      self.cc_suggestion_cursor = None;
      self.cc_suggestions.clear();
    }
  }

  fn confirm_to_suggestion(&mut self) {
    if let Some(i) = self.to_suggestion_cursor
      && let Some((id, name)) = self.to_suggestions.get(i).cloned()
    {
      self.to.push(ComposeRecipient {
        id: Some(id),
        name,
      });
      self.to_search.clear();
      self.to_suggestion_cursor = None;
      self.to_suggestions.clear();
    }
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

fn error_row(err: &str) -> Element<'_, Message> {
  container(
    text(err)
      .font(font::body::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::DANGER),
      }),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill)
  .into()
}

fn from_picker_overlay(panel: &Component) -> Element<'_, Message> {
  if !panel.from_picker.is_open {
    return Space::new().into();
  }
  let dropdown = panel.from_picker.dropdown().map(Message::FromPicker);
  container(dropdown)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Left)
    .align_y(iced::alignment::Vertical::Bottom)
    .padding(Padding {
      top: 0.0,
      left: 12.0,
      right: 12.0,
      bottom: 52.0,
    })
    .into()
}

pub(super) fn compose_field_row<'a>(label: &'static str, content: Element<'a, Message>) -> Element<'a, Message> {
  column([
    container(
      row([
        container(
          text(label.to_uppercase())
            .font(font::mono::REGULAR)
            .size(9.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::SECONDARY),
            }),
        )
        .width(56.0)
        .into(),
        container(content).width(Length::Fill).into(),
      ])
      .spacing(14.0)
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .into(),
    container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into(),
  ])
  .into()
}
