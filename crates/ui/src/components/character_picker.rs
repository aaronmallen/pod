pub mod entry;
pub mod header;
pub mod portrait;

use entry::{AllPickerEntry, CharacterPickerEntry, CorporationPickerEntry};
use header::{CorpSectionHeader, DropdownHeader};
use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{button, column, container, row, scrollable, text},
};
use portrait::{portrait_swatch, selected_display, trigger_label_col};

use crate::style::{color, spacing};

/// A single selectable character (or "All" sentinel).
#[derive(Clone, Debug)]
pub struct CharacterEntry {
  /// `None` means the "All Wallets / All Characters" sentinel.
  pub id: Option<i64>,
  pub name: String,
  pub corp_name: String,
  /// Portrait hue 0–360, used for the gradient background.
  pub tone: u16,
  /// Pre-loaded portrait image. When `Some`, renders the actual
  /// portrait instead of initials.
  pub portrait_handle: Option<iced::widget::image::Handle>,
}

/// A single selectable corporation.
#[derive(Clone, Debug)]
pub struct CorporationEntry {
  /// Corporation icon PNG pre-loaded as an image handle.
  pub icon_handle: Option<iced::widget::image::Handle>,
  /// EVE corporation ID.
  pub id: i64,
  pub name: String,
  pub ticker: String,
}

/// The current selection state of the picker.
#[derive(Clone, Debug, Default)]
pub enum PickerSelection {
  /// No specific entity selected — show aggregate data.
  #[default]
  All,
  /// A specific character is selected.
  Character(i64),
  /// A specific corporation is selected.
  Corporation(i64),
}

/// Messages emitted by the character picker.
#[derive(Clone, Debug)]
pub enum Message {
  CloseRequested,
  Select(PickerSelection),
  ToggleOpen,
}

/// Stateful character picker organism.
///
/// Holds open/closed state, the selected ID, and the entry list so
/// callers do not need to duplicate that bookkeeping.
#[derive(Debug)]
pub struct Component {
  pub all_label: String,
  pub corp_entries: Vec<CorporationEntry>,
  pub entries: Vec<CharacterEntry>,
  pub is_open: bool,
  pub selected: PickerSelection,
  pub show_all: bool,
}

impl Component {
  /// Create a new picker with default (closed, nothing selected) state.
  pub fn new() -> Self {
    Self {
      all_label: "All Wallets".to_string(),
      corp_entries: Vec::new(),
      entries: Vec::new(),
      is_open: false,
      selected: PickerSelection::All,
      show_all: false,
    }
  }

  /// Builder: set the label for the "show all" sentinel row.
  pub fn all_label(mut self, label: impl Into<String>) -> Self {
    self.all_label = label.into();
    self
  }

  /// Builder: replace the corporation entry list.
  pub fn corp_entries(mut self, v: Vec<CorporationEntry>) -> Self {
    self.corp_entries = v;
    self
  }

  /// Builder: replace the character entry list.
  pub fn entries(mut self, v: Vec<CharacterEntry>) -> Self {
    self.entries = v;
    self
  }

  /// Returns the selected character ID, or `None` when a corporation
  /// or "All" is selected.
  pub fn selected_character_id(&self) -> Option<i64> {
    match self.selected {
      PickerSelection::Character(id) => Some(id),
      _ => None,
    }
  }

  /// Returns the selected corporation ID, or `None` when a character
  /// or "All" is selected.
  pub fn selected_corporation_id(&self) -> Option<i64> {
    match self.selected {
      PickerSelection::Corporation(id) => Some(id),
      _ => None,
    }
  }

  /// Builder: set the initial selection.
  pub fn selected(mut self, sel: PickerSelection) -> Self {
    self.selected = sel;
    self
  }

  /// Builder: enable the "All" sentinel row.
  pub fn show_all(mut self, v: bool) -> Self {
    self.show_all = v;
    self
  }

  /// Process a picker message, mutating internal state.
  pub fn update(&mut self, msg: Message) {
    match msg {
      Message::CloseRequested => self.is_open = false,
      Message::Select(sel) => {
        self.selected = sel;
        self.is_open = false;
      }
      Message::ToggleOpen => self.is_open = !self.is_open,
    }
  }

  /// Render the trigger button (always shown).
  pub fn render(&self) -> Element<'_, Message> {
    let (name, subtitle, tone, portrait_handle) = selected_display(&self.entries, &self.corp_entries, &self.selected);
    let swatch = portrait_swatch(&name, tone, 38.0, 8.0, portrait_handle);
    let label_col = trigger_label_col(name, subtitle);

    let caret: Element<'_, Message> = text("⌄")
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into();

    let inner: Element<'_, Message> = row([swatch, label_col, caret])
      .spacing(spacing::SPACE_3)
      .align_y(iced::alignment::Vertical::Center)
      .into();

    let is_open = self.is_open;
    button(inner)
      .padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 6.0,
        right: spacing::SPACE_2_5,
      })
      .style(move |_, status| trigger_btn_style(is_open, status))
      .on_press(Message::ToggleOpen)
      .into()
  }

  /// Render the dropdown popover panel.
  pub fn dropdown(&self) -> Element<'_, Message> {
    let mut rows: Vec<Element<'_, Message>> = vec![DropdownHeader::new().render()];

    if self.show_all {
      rows.push(AllPickerEntry::new(&self.all_label, matches!(self.selected, PickerSelection::All)).render());
    }
    rows.extend(character_rows(&self.entries, &self.selected));
    rows.extend(corporation_rows(&self.corp_entries, &self.selected));

    let list = scrollable(column(rows).width(Length::Fill)).height(Length::Shrink);

    container(list)
      .width(Length::Fixed(360.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::DEFAULT,
          radius: 10.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn trigger_btn_style(is_open: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let bg = if is_open {
    Some(color::state::SUBTLE_FILL)
  } else if hovered {
    Some(color::state::HOVER_OVERLAY)
  } else {
    None
  };
  button::Style {
    background: bg.map(Background::Color),
    border: Border {
      color: if is_open {
        color::border::DEFAULT
      } else {
        Color::TRANSPARENT
      },
      radius: 10.0.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn character_rows<'a>(entries: &'a [CharacterEntry], selected: &PickerSelection) -> Vec<Element<'a, Message>> {
  entries
    .iter()
    .filter(|e| e.id.is_some())
    .map(|entry| {
      let is_selected = matches!(selected, PickerSelection::Character(id) if entry.id == Some(*id));
      CharacterPickerEntry::new(entry, is_selected).render()
    })
    .collect()
}

fn corporation_rows<'a>(entries: &'a [CorporationEntry], selected: &PickerSelection) -> Vec<Element<'a, Message>> {
  if entries.is_empty() {
    return Vec::new();
  }
  let mut rows: Vec<Element<'a, Message>> = vec![CorpSectionHeader::new().render()];
  for entry in entries {
    let is_selected = matches!(selected, PickerSelection::Corporation(id) if *id == entry.id);
    rows.push(CorporationPickerEntry::new(entry, is_selected).render());
  }
  rows
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}
