//! Characters window view — shows a Characters/Corporations tab panel.

pub mod characters_tab;
pub mod confirm_dialog;
pub mod corporation_confirm_dialog;
pub mod corporations_tab;
pub mod empty_state;
pub mod header;
pub mod search_filter;
pub mod tag_modal;

pub use characters_tab::Component as CharacterPane;
pub use confirm_dialog::Component as ConfirmDialog;
pub use corporation_confirm_dialog::Component as CorporationConfirmDialog;
pub use corporations_tab::Component as CorporationPane;
pub use empty_state::Component as EmptyState;
pub use header::Component as Header;
use iced::{
  Background, Element, Length, Padding,
  alignment::Horizontal,
  widget::{column, container, stack},
};
use pod_model::{Character, Corporation};
pub use search_filter::Component as SearchFilter;
pub use tag_modal::Component as TagModal;

use crate::{
  components,
  style::{color, spacing},
};

/// Active tab in the Characters window.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  /// The character grid tab (default).
  #[default]
  Characters,
  /// The corporation grid tab.
  Corporations,
}

impl Tab {
  /// Returns the string id used by the tab strip component.
  pub fn as_str(&self) -> &'static str {
    match self {
      Tab::Characters => "characters",
      Tab::Corporations => "corporations",
    }
  }
}

/// Full view state for the Characters window.
pub struct State {
  /// Currently active tab.
  pub active_tab: Tab,
  /// Add-character status message (e.g., an error string).
  pub add_status: Option<String>,
  /// All loaded characters (unfiltered).
  pub all_characters: Vec<Character>,
  /// All loaded corporations (unfiltered).
  pub all_corporations: Vec<Corporation>,
  /// All known tags.
  pub all_tags: Vec<(i32, String)>,
  /// Characters tab state.
  pub character_pane: characters_tab::State,
  /// Filtered character list (may equal all_characters).
  pub characters: Vec<Character>,
  /// Character id pending removal confirmation, if any.
  pub confirm_remove: Option<i64>,
  /// Corporation id pending removal confirmation, if any.
  pub confirm_remove_corporation: Option<i64>,
  /// Corporations tab state.
  pub corporation_pane: corporations_tab::State,
  /// Filtered corporation list (may equal all_corporations).
  pub corporations: Vec<Corporation>,
  /// Feature: location_tracking enabled.
  pub feat_location_tracking: bool,
  /// Feature: skill_monitoring enabled.
  pub feat_skill_monitoring: bool,
  /// Feature: wallet enabled.
  pub feat_wallet: bool,
  /// Header state.
  pub header: header::State,
  /// Search filter state.
  pub search_filter: search_filter::State,
  /// Pre-computed tag corpus for the open tag modal (empty when modal is closed).
  pub tag_corpus: Vec<(String, usize)>,
  /// Tag modal state, if open.
  pub tag_modal: Option<tag_modal::State>,
}

impl State {
  /// Creates a new State with the given initial characters.
  pub fn new(characters: Vec<Character>) -> Self {
    Self {
      active_tab: Tab::default(),
      add_status: None,
      all_characters: characters.clone(),
      all_corporations: Vec::new(),
      all_tags: Vec::new(),
      character_pane: characters_tab::State::new(),
      characters,
      confirm_remove: None,
      confirm_remove_corporation: None,
      corporation_pane: corporations_tab::State::default(),
      feat_location_tracking: true,
      feat_skill_monitoring: true,
      feat_wallet: true,
      corporations: Vec::new(),
      header: header::State,
      search_filter: search_filter::State::new(),
      tag_corpus: Vec::new(),
      tag_modal: None,
    }
  }
}

/// Messages that can be sent to the Characters window.
#[derive(Clone, Debug)]
pub enum Message {
  /// An error occurred while adding a character.
  AddCharacterError(String),
  /// An error occurred while adding a corporation.
  AddCorporationError(String),
  /// All tags were loaded.
  AllTagsLoaded(Vec<(i32, String)>),
  /// Message from the character grid tab.
  CharactersTab(characters_tab::Message),
  /// Confirm removal of the pending character.
  ConfirmRemove,
  /// Confirm removal of the pending corporation.
  ConfirmRemoveCorporation,
  /// Message from the corporation grid tab.
  CorporationsTab(corporations_tab::Message),
  /// Dismiss the character-remove confirmation dialog.
  DismissConfirmRemove,
  /// Dismiss the corporation-remove confirmation dialog.
  DismissConfirmRemoveCorporation,
  /// Message from the header (tab selection, add buttons).
  Header(header::Message),
  /// Message from the search filter bar.
  SearchFilter(search_filter::Message),
  /// Message from the tag modal overlay.
  TagModal(tag_modal::Message),
  /// Tags were applied successfully.
  TagsApplied,
}

/// Builder for the Characters window root element.
pub struct Component<'a> {
  state: &'a State,
  window_height: f32,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new component for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_height: spacing::layout::WINDOW_DEFAULT_HEIGHT,
      window_width: spacing::layout::WINDOW_DEFAULT_WIDTH,
    }
  }

  /// Sets the window dimensions used for layout calculations.
  pub fn window_size(mut self, width: f32, height: f32) -> Self {
    self.window_height = height;
    self.window_width = width;
    self
  }

  /// Renders the full Characters window into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let window_width = self.window_width;
    let window_height = self.window_height;

    let query = &state.search_filter.query;
    let is_filtered = !query.trim().is_empty();

    let base = render_base(state, query, is_filtered, window_width, window_height);

    let needs_overlay = state.search_filter.help_pop_over.visible
      || state.confirm_remove.is_some()
      || state.confirm_remove_corporation.is_some()
      || state.tag_modal.is_some();

    if !needs_overlay {
      return base;
    }

    let mut layers: Vec<Element<'_, Message>> = vec![base];
    push_help_overlay(state, &mut layers);
    push_confirm_remove_overlay(state, window_width, window_height, &mut layers);
    push_confirm_remove_corporation_overlay(state, window_width, window_height, &mut layers);
    push_tag_modal_overlay(state, window_width, window_height, &mut layers);

    stack(layers).into()
  }
}

fn render_base<'a>(
  state: &'a State,
  query: &'a str,
  is_filtered: bool,
  window_width: f32,
  window_height: f32,
) -> Element<'a, Message> {
  let header_el = Header::new(
    state.active_tab.as_str(),
    state.characters.len(),
    state.all_characters.len(),
    state.corporations.len(),
    state.all_corporations.len(),
    is_filtered,
  )
  .render()
  .map(Message::Header);

  let filter_el = SearchFilter::new(&state.search_filter)
    .render()
    .map(Message::SearchFilter);

  let grid_el = render_tab_content(state, is_filtered, query, window_width, window_height);

  container(column([header_el, filter_el, grid_el]).width(Length::Fill))
    .height(Length::Fill)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn push_help_overlay<'a>(state: &'a State, layers: &mut Vec<Element<'a, Message>>) {
  if !state.search_filter.help_pop_over.visible {
    return;
  }

  let help_el = search_filter::HelpPopOver::new(&state.search_filter.help_pop_over, &state.all_tags)
    .render()
    .map(search_filter::Message::HelpPopOver)
    .map(Message::SearchFilter);

  let positioned = container(help_el)
    .height(Length::Fill)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::layout::HEADER_HEIGHT + 64.0,
      right: spacing::SPACE_8,
      ..Padding::ZERO
    })
    .align_x(Horizontal::Right);

  layers.push(positioned.into());
}

fn push_confirm_remove_overlay<'a>(
  state: &'a State,
  window_width: f32,
  window_height: f32,
  layers: &mut Vec<Element<'a, Message>>,
) {
  let Some(character_id) = state.confirm_remove else {
    return;
  };

  let character_name = state
    .all_characters
    .iter()
    .find(|c| *c.id() == character_id)
    .map(|c| c.name().clone())
    .unwrap_or_default();

  let backdrop = components::Backdrop::new(Message::DismissConfirmRemove).render();
  let dialog = ConfirmDialog::new(character_name)
    .window_size(window_width, window_height)
    .render()
    .map(|msg| match msg {
      confirm_dialog::Message::Confirmed => Message::ConfirmRemove,
      confirm_dialog::Message::Dismissed => Message::DismissConfirmRemove,
    });

  layers.push(backdrop);
  layers.push(dialog);
}

fn push_confirm_remove_corporation_overlay<'a>(
  state: &'a State,
  window_width: f32,
  window_height: f32,
  layers: &mut Vec<Element<'a, Message>>,
) {
  let Some(corporation_id) = state.confirm_remove_corporation else {
    return;
  };

  let corporation_name = state
    .all_corporations
    .iter()
    .find(|c| *c.id() == corporation_id)
    .map(|c| c.name().clone())
    .unwrap_or_default();

  let backdrop = components::Backdrop::new(Message::DismissConfirmRemoveCorporation).render();
  let dialog = CorporationConfirmDialog::new(corporation_name)
    .window_size(window_width, window_height)
    .render()
    .map(|msg| match msg {
      corporation_confirm_dialog::Message::Confirmed => Message::ConfirmRemoveCorporation,
      corporation_confirm_dialog::Message::Dismissed => Message::DismissConfirmRemoveCorporation,
    });

  layers.push(backdrop);
  layers.push(dialog);
}

fn push_tag_modal_overlay<'a>(
  state: &'a State,
  window_width: f32,
  window_height: f32,
  layers: &mut Vec<Element<'a, Message>>,
) {
  let Some(modal_state) = &state.tag_modal else {
    return;
  };

  let corpus = state.tag_corpus.clone();
  let backdrop = components::Backdrop::new(Message::TagModal(tag_modal::Message::Close)).render();
  let modal = TagModal::new(modal_state, corpus)
    .window_size(window_width, window_height)
    .render()
    .map(Message::TagModal);

  layers.push(backdrop);
  layers.push(modal);
}

fn render_tab_content<'a>(
  state: &'a State,
  is_filtered: bool,
  query: &'a str,
  window_width: f32,
  window_height: f32,
) -> Element<'a, Message> {
  match state.active_tab {
    Tab::Characters => {
      let visible: Vec<&Character> = state.characters.iter().collect();
      if visible.is_empty() && !is_filtered {
        EmptyState::new()
          .add_status(state.add_status.as_deref())
          .render::<Message>()
      } else if visible.is_empty() {
        EmptyState::new().filtered(query).render::<Message>()
      } else {
        let pane_h = (window_height - spacing::layout::HEADER_HEIGHT - spacing::SPACE_8 * 2.0).max(0.0);
        CharacterPane::new(visible, &state.character_pane)
          .feat_skill_monitoring(state.feat_skill_monitoring)
          .feat_wallet(state.feat_wallet)
          .window_width(window_width)
          .pane_height(pane_h)
          .render()
          .map(Message::CharactersTab)
      }
    }
    Tab::Corporations => {
      let visible: Vec<&Corporation> = state.corporations.iter().collect();
      if visible.is_empty() && !is_filtered {
        corporation_empty_state()
      } else if visible.is_empty() {
        corporation_filtered_empty_state(query)
      } else {
        CorporationPane::new(visible, &state.corporation_pane)
          .characters(&state.all_characters)
          .window_width(window_width)
          .render()
          .map(Message::CorporationsTab)
      }
    }
  }
}

fn corporation_empty_state<'a>() -> Element<'a, Message> {
  use iced::widget::{container, text};

  use crate::style::{color, typography};

  container(
    text("Add your first corporation to get started")
      .font(typography::body::REGULAR)
      .size(15.0)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .center_x(Length::Fill)
  .center_y(Length::Fill)
  .into()
}

fn corporation_filtered_empty_state<'a>(query: &str) -> Element<'a, Message> {
  use iced::{
    alignment::Horizontal,
    widget::{column, container, text},
  };

  use crate::style::{color, spacing, typography};

  let q = query.to_owned();

  container(
    column([
      text("No results")
        .font(typography::body::MEDIUM)
        .size(15.0)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(format!("No corporations match \"{q}\""))
        .font(typography::body::REGULAR)
        .size(13.0)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_1)
    .align_x(Horizontal::Center),
  )
  .height(Length::Fill)
  .width(Length::Fill)
  .center_x(Length::Fill)
  .center_y(Length::Fill)
  .into()
}
