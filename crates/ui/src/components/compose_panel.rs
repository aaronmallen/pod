use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, stack, text, text_editor, text_input},
};

use crate::{
  components::{
    Card, PanelHeader,
    character_picker::{self, CharacterEntry, Component as CharacterPicker, PickerSelection},
  },
  style::{color, typography as font},
};

/// A resolved or pending compose recipient.
#[derive(Clone, Debug)]
pub struct ComposeRecipient {
  pub name: String,
  /// Known character/corp/alliance ID; `None` means resolve by name at send time.
  pub id: Option<i64>,
}

/// Messages produced by the compose panel.
#[derive(Clone, Debug)]
pub enum Message {
  Close,
  Expand,
  ToSearchChanged(String),
  ToSearchSelect(i64, String),
  ToSearchResults(Vec<(i64, String)>),
  ToAdd,
  ToRemove(usize),
  CcToggle,
  CcSearchChanged(String),
  CcSearchSelect(i64, String),
  CcSearchResults(Vec<(i64, String)>),
  CcAdd,
  CcRemove(usize),
  SubjectChanged(String),
  BodyAction(text_editor::Action),
  FromPicker(character_picker::Message),
  SuggestionCursorMove(i32),
  SuggestionCursorConfirm,
  SendPressed,
  Sent(Result<i64, String>),
}

/// Stateful compose panel organism.
pub struct Component {
  pub to: Vec<ComposeRecipient>,
  pub to_search: String,
  pub to_suggestions: Vec<(i64, String)>,
  pub to_suggestion_cursor: Option<usize>,
  pub cc: Vec<ComposeRecipient>,
  pub cc_search: String,
  pub cc_suggestions: Vec<(i64, String)>,
  pub cc_suggestion_cursor: Option<usize>,
  pub cc_visible: bool,
  pub subject: String,
  pub body: text_editor::Content,
  pub expanded: bool,
  pub sending: bool,
  pub error: Option<String>,
  pub from_picker: CharacterPicker,
}

impl Component {
  /// Create a new compose panel with default (empty, collapsed) state.
  pub fn new() -> Self {
    Self {
      to: Vec::new(),
      to_search: String::new(),
      to_suggestions: Vec::new(),
      to_suggestion_cursor: None,
      cc: Vec::new(),
      cc_search: String::new(),
      cc_suggestions: Vec::new(),
      cc_suggestion_cursor: None,
      cc_visible: false,
      subject: String::new(),
      body: text_editor::Content::new(),
      expanded: false,
      sending: false,
      error: None,
      from_picker: CharacterPicker::new(),
    }
  }

  /// Builder: set the from-picker entries.
  pub fn from_entries(mut self, entries: Vec<CharacterEntry>) -> Self {
    self.from_picker = self.from_picker.entries(entries);
    self
  }

  /// Builder: set the currently selected from-character ID.
  pub fn from_selected(mut self, id: Option<i64>) -> Self {
    let sel = id.map(PickerSelection::Character).unwrap_or(PickerSelection::All);
    self.from_picker = self.from_picker.selected(sel);
    self
  }

  /// Returns the currently selected from-character ID (from the picker).
  pub fn from_id(&self) -> Option<i64> {
    self.from_picker.selected_character_id()
  }

  /// Reset compose state for a fresh new message.
  pub fn reset(&mut self) {
    self.to.clear();
    self.to_search.clear();
    self.to_suggestions.clear();
    self.to_suggestion_cursor = None;
    self.cc.clear();
    self.cc_search.clear();
    self.cc_suggestions.clear();
    self.cc_suggestion_cursor = None;
    self.cc_visible = false;
    self.subject.clear();
    self.body = text_editor::Content::new();
    self.expanded = false;
    self.sending = false;
    self.error = None;
    self.from_picker.is_open = false;
  }

  /// Process a panel message, mutating internal state.
  pub fn update(&mut self, msg: Message) {
    match msg {
      Message::Close => {
        self.sending = false;
        self.error = None;
        self.from_picker.is_open = false;
        self.to_suggestions.clear();
        self.to_suggestion_cursor = None;
        self.cc_suggestions.clear();
        self.cc_suggestion_cursor = None;
      }
      Message::Expand => {
        self.expanded = !self.expanded;
      }
      Message::ToSearchChanged(val) => {
        self.from_picker.is_open = false;
        if val.is_empty() {
          self.to_suggestions.clear();
          self.to_suggestion_cursor = None;
        }
        self.to_search = val;
      }
      Message::ToSearchResults(results) => {
        self.to_suggestion_cursor = None;
        self.to_suggestions = results;
      }
      Message::ToSearchSelect(id, name) => {
        self.to.push(ComposeRecipient {
          name,
          id: Some(id),
        });
        self.to_search.clear();
        self.to_suggestions.clear();
        self.to_suggestion_cursor = None;
      }
      Message::ToAdd => {
        let name = self.to_search.trim().to_string();
        if !name.is_empty() {
          self.to.push(ComposeRecipient {
            name,
            id: None,
          });
          self.to_search.clear();
          self.to_suggestions.clear();
          self.to_suggestion_cursor = None;
        }
      }
      Message::ToRemove(idx) => {
        if idx < self.to.len() {
          self.to.remove(idx);
        }
      }
      Message::CcToggle => {
        self.cc_visible = !self.cc_visible;
      }
      Message::CcSearchChanged(val) => {
        if val.is_empty() {
          self.cc_suggestions.clear();
          self.cc_suggestion_cursor = None;
        }
        self.cc_search = val;
      }
      Message::CcSearchResults(results) => {
        self.cc_suggestion_cursor = None;
        self.cc_suggestions = results;
      }
      Message::CcSearchSelect(id, name) => {
        self.cc.push(ComposeRecipient {
          name,
          id: Some(id),
        });
        self.cc_search.clear();
        self.cc_suggestions.clear();
        self.cc_suggestion_cursor = None;
      }
      Message::CcAdd => {
        let name = self.cc_search.trim().to_string();
        if !name.is_empty() {
          self.cc.push(ComposeRecipient {
            name,
            id: None,
          });
          self.cc_search.clear();
          self.cc_suggestions.clear();
          self.cc_suggestion_cursor = None;
        }
      }
      Message::CcRemove(idx) => {
        if idx < self.cc.len() {
          self.cc.remove(idx);
        }
      }
      Message::SubjectChanged(val) => {
        self.subject = val;
      }
      Message::BodyAction(action) => {
        self.body.perform(action);
      }
      Message::FromPicker(msg) => {
        self.from_picker.update(msg);
      }
      Message::SuggestionCursorMove(delta) => {
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
      Message::SuggestionCursorConfirm => {
        let to_active = !self.to_suggestions.is_empty() && !self.to_search.is_empty();
        let cc_active = self.cc_visible && !self.cc_suggestions.is_empty() && !self.cc_search.is_empty();
        if to_active {
          if let Some(i) = self.to_suggestion_cursor
            && let Some((id, name)) = self.to_suggestions.get(i).cloned()
          {
            self.to.push(ComposeRecipient {
              name,
              id: Some(id),
            });
            self.to_search.clear();
            self.to_suggestions.clear();
            self.to_suggestion_cursor = None;
          }
        } else if cc_active {
          if let Some(i) = self.cc_suggestion_cursor
            && let Some((id, name)) = self.cc_suggestions.get(i).cloned()
          {
            self.cc.push(ComposeRecipient {
              name,
              id: Some(id),
            });
            self.cc_search.clear();
            self.cc_suggestions.clear();
            self.cc_suggestion_cursor = None;
          }
        }
      }
      Message::SendPressed => {
        self.sending = true;
        self.error = None;
      }
      Message::Sent(Ok(_)) => {
        self.sending = false;
        self.reset();
      }
      Message::Sent(Err(e)) => {
        self.sending = false;
        self.error = Some(e);
      }
    }
  }

  /// Render the compose panel at the appropriate size.
  pub fn render(&self) -> Element<'_, Message> {
    let (panel_width, panel_height): (f32, f32) = if self.expanded { (820.0, 640.0) } else { (540.0, 480.0) };

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
    panel_rows.push(send_footer(self));

    let base = column(panel_rows).width(Length::Fill).height(Length::Fill);

    let inner: Element<'_, Message> = stack([
      base.into(),
      to_suggestions_overlay(self),
      cc_suggestions_overlay(self),
      from_picker_overlay(self),
    ])
    .into();

    Card::new(inner)
      .width(Length::Fixed(panel_width))
      .height(Length::Fixed(panel_height))
      .render()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

fn panel_header(expand_sym: &'static str) -> Element<'static, Message> {
  let close_btn = icon_btn("–", Message::Close);
  let expand_btn = icon_btn(expand_sym, Message::Expand);
  let dismiss_btn = icon_btn("✕", Message::Close);

  PanelHeader::new("NEW MESSAGE")
    .action(close_btn)
    .action(expand_btn)
    .action(dismiss_btn)
    .render()
}

fn to_field(panel: &Component) -> Element<'_, Message> {
  let to_chips: Vec<Element<'_, Message>> = panel
    .to
    .iter()
    .enumerate()
    .map(|(i, r)| recipient_chip(r.name.as_str(), Message::ToRemove(i)))
    .collect();

  let to_input = text_input(
    if panel.to.is_empty() { "Add recipient…" } else { "" },
    &panel.to_search,
  )
  .on_input(Message::ToSearchChanged)
  .on_submit(Message::ToAdd)
  .size(13.0)
  .font(font::body::REGULAR)
  .style(|_, _| text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::SECONDARY,
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
  });

  let mut row_children: Vec<Element<'_, Message>> = to_chips;
  row_children.push(to_input.into());
  if !panel.cc_visible {
    row_children.push(cc_toggle_btn());
  }

  compose_field_row(
    "To",
    row(row_children)
      .spacing(6.0)
      .align_y(iced::alignment::Vertical::Center)
      .into(),
  )
}

fn cc_toggle_btn() -> Element<'static, Message> {
  button(
    text("Cc")
      .font(font::body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding::from([0.0, 0.0]))
  .on_press(Message::CcToggle)
  .style(|_, status| button::Style {
    background: None,
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}

fn cc_field(panel: &Component) -> Element<'_, Message> {
  let cc_chips: Vec<Element<'_, Message>> = panel
    .cc
    .iter()
    .enumerate()
    .map(|(i, r)| recipient_chip(r.name.as_str(), Message::CcRemove(i)))
    .collect();

  let cc_input = text_input(
    if panel.cc.is_empty() { "Add Cc recipient…" } else { "" },
    &panel.cc_search,
  )
  .on_input(Message::CcSearchChanged)
  .on_submit(Message::CcAdd)
  .size(13.0)
  .font(font::body::REGULAR)
  .style(|_, _| text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::SECONDARY,
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
  });

  let mut row_children: Vec<Element<'_, Message>> = cc_chips;
  row_children.push(cc_input.into());

  compose_field_row(
    "Cc",
    row(row_children)
      .spacing(6.0)
      .align_y(iced::alignment::Vertical::Center)
      .into(),
  )
}

fn subject_field(subject: &str) -> Element<'_, Message> {
  let input = text_input("—", subject)
    .on_input(Message::SubjectChanged)
    .size(15.0)
    .font(font::body::MEDIUM)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
    });
  compose_field_row("Subject", input.into())
}

fn body_area(body: &text_editor::Content) -> Element<'_, Message> {
  container(
    text_editor(body)
      .on_action(Message::BodyAction)
      .height(Length::Fill)
      .size(14.0)
      .font(font::body::REGULAR)
      .padding(Padding::ZERO)
      .style(|_, _| text_editor::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: color::text::TERTIARY,
        value: Color::from_rgba(0.957, 0.949, 0.925, 0.92),
        selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .padding(Padding {
    top: 16.0,
    bottom: 16.0,
    left: 16.0,
    right: 16.0,
  })
  .into()
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

fn send_footer(panel: &Component) -> Element<'_, Message> {
  let can_send = !panel.to.is_empty() && !panel.subject.trim().is_empty();
  let from_trigger = panel.from_picker.render().map(Message::FromPicker);

  container(
    row([
      from_trigger,
      Space::new().width(Length::Fill).into(),
      send_button(can_send, panel.sending),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .center_y(52.0)
  .width(Length::Fill)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: 12.0,
    right: 12.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn send_button(can_send: bool, sending: bool) -> Element<'static, Message> {
  let label = if sending { "Sending…" } else { "Send" };
  let btn = button(
    text(label)
      .font(font::body::MEDIUM)
      .size(13.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(if can_send && !sending {
          color::surface::BASE
        } else {
          Color::from_rgba(0.957, 0.949, 0.925, 0.40)
        }),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 16.0,
    right: 16.0,
  })
  .style(move |_, status| button::Style {
    background: Some(Background::Color(if can_send && !sending {
      match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba(0.247, 0.722, 0.859, 0.85),
        _ => color::accent::PLASMA,
      }
    } else {
      Color::from_rgba(0.247, 0.722, 0.859, 0.25)
    })),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::surface::BASE,
    ..button::Style::default()
  });
  if can_send && !sending {
    btn.on_press(Message::SendPressed).into()
  } else {
    btn.into()
  }
}

fn suggestions_box<'a>(
  suggestions: &'a [(i64, String)],
  cursor: Option<usize>,
  make_msg: impl Fn(i64, String) -> Message + 'a,
) -> Element<'a, Message> {
  let rows: Vec<Element<'_, Message>> = suggestions
    .iter()
    .enumerate()
    .map(|(idx, (id, name))| {
      let id = *id;
      let msg_name = name.clone();
      let selected = cursor == Some(idx);
      button(
        text(name.as_str())
          .font(font::body::MEDIUM)
          .size(13.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .width(Length::Fill),
      )
      .width(Length::Fill)
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 12.0,
        right: 12.0,
      })
      .on_press(make_msg(id, msg_name))
      .style(move |_, status| button::Style {
        background: if selected {
          Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.15)))
        } else {
          match status {
            button::Status::Hovered | button::Status::Pressed => {
              Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.06)))
            }
            _ => None,
          }
        },
        border: Border::default(),
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      })
      .into()
    })
    .collect();
  container(column(rows).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 8.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn to_suggestions_overlay(panel: &Component) -> Element<'_, Message> {
  if panel.to_suggestions.is_empty() || panel.to_search.is_empty() {
    return Space::new().into();
  }
  let suggestions = suggestions_box(&panel.to_suggestions, panel.to_suggestion_cursor, |id, name| {
    Message::ToSearchSelect(id, name)
  });
  container(suggestions)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Left)
    .align_y(iced::alignment::Vertical::Top)
    .padding(Padding {
      top: 82.0,
      left: 16.0,
      right: 16.0,
      bottom: 0.0,
    })
    .into()
}

fn cc_suggestions_overlay(panel: &Component) -> Element<'_, Message> {
  if !panel.cc_visible || panel.cc_suggestions.is_empty() || panel.cc_search.is_empty() {
    return Space::new().into();
  }
  let suggestions = suggestions_box(&panel.cc_suggestions, panel.cc_suggestion_cursor, |id, name| {
    Message::CcSearchSelect(id, name)
  });
  container(suggestions)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Left)
    .align_y(iced::alignment::Vertical::Top)
    .padding(Padding {
      top: 123.0,
      left: 16.0,
      right: 16.0,
      bottom: 0.0,
    })
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

fn compose_field_row<'a>(label: &'static str, content: Element<'a, Message>) -> Element<'a, Message> {
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

fn recipient_chip(name: &str, remove_msg: Message) -> Element<'_, Message> {
  container(
    row([
      text(name)
        .font(font::body::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      recipient_remove_btn(remove_msg),
    ])
    .spacing(4.0)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 6.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.06))),
    border: Border {
      color: color::border::SUBTLE,
      radius: 999.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn recipient_remove_btn(remove_msg: Message) -> Element<'static, Message> {
  button(
    text("✕")
      .font(font::mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding::from([0.0, 0.0]))
  .on_press(remove_msg)
  .style(|_, status| button::Style {
    background: None,
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}

fn icon_btn(label: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    container(
      text(label)
        .font(font::mono::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .center_x(16.0)
    .center_y(24.0),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05)))
      }
      _ => None,
    },
    border: Border {
      radius: 5.0.into(),
      ..Border::default()
    },
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}
