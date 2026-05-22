//! Grid pane / tab that displays all tracked characters as cards.

pub mod character_card;
pub mod context_menu;

use std::collections::HashMap;

pub use character_card::Component as CharacterCard;
pub use context_menu::Component as ContextMenu;
use iced::{
  Background, Border, Element, Length, Padding, Point, Task,
  widget::{Id, column, container, mouse_area, row, scrollable, stack},
};
use pod_model::{Character, CharacterSkill, TrainingQueueEntry};

use crate::style::{color, radius, shadow, spacing};

/// Scroll ID for the character grid; exposed so controllers can programmatically restore scroll position.
pub static GRID_SCROLL_ID: std::sync::LazyLock<Id> = std::sync::LazyLock::new(|| Id::new("character-grid"));

pub struct State {
  pub portrait_handles: HashMap<i64, iced::widget::image::Handle>,
  pub context_menu: Option<context_menu::State>,
  cursor_position: Point,
  pub dragging_id: Option<i64>,
  pub drag_hover: Option<i64>,
  scroll_offset: scrollable::AbsoluteOffset,
}

impl State {
  pub fn new() -> Self {
    Self {
      portrait_handles: HashMap::new(),
      context_menu: None,
      cursor_position: Point::ORIGIN,
      dragging_id: None,
      drag_hover: None,
      scroll_offset: scrollable::AbsoluteOffset::default(),
    }
  }

  /// Returns the current scroll position of the character grid.
  pub fn scroll_offset(&self) -> scrollable::AbsoluteOffset {
    self.scroll_offset
  }

  pub fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
      Message::CursorMoved(pt) => {
        self.cursor_position = pt;
        Task::none()
      }
      Message::Card(_, card_msg) => self.update_card(card_msg),
      Message::ContextMenu(inner) => self.update_context_menu(inner),
      Message::DragEnd => {
        self.dragging_id = None;
        self.drag_hover = None;
        Task::none()
      }
      Message::DragMoved(pt, pane_height) => self.update_drag_moved(pt, pane_height),
      Message::ScrollOffsetChanged(viewport) => {
        self.scroll_offset = viewport.absolute_offset();
        Task::none()
      }
      Message::CharacterAdded(_)
      | Message::CharacterPublicRefreshed(_)
      | Message::CharacterPublicRefreshTick
      | Message::CharacterTagsLoaded(_, _)
      | Message::LocationRefreshTick
      | Message::LocationsRefreshed(_)
      | Message::NavigateToDetail(_)
      | Message::NavigateToSkills(_)
      | Message::NavigateToWallet(_)
      | Message::RemoveCharacter(_)
      | Message::SkillQueueRefreshTick
      | Message::SkillQueuesRefreshed(_)
      | Message::WalletRefreshTick
      | Message::WalletsRefreshed(_) => Task::none(),
    }
  }

  fn update_card(&mut self, card_msg: character_card::Message) -> Task<Message> {
    match card_msg {
      character_card::Message::CardRightPressed(id, name) => {
        self.context_menu = Some(context_menu::State {
          character_id: id,
          character_name: name,
          x: self.cursor_position.x,
          y: self.cursor_position.y,
        });
        Task::none()
      }
      character_card::Message::CardPressed(id) => {
        self.context_menu = None;
        self.dragging_id = Some(id);
        Task::none()
      }
      character_card::Message::CardEntered(id) => {
        if self.dragging_id.is_some() {
          self.drag_hover = Some(id);
        }
        Task::none()
      }
      character_card::Message::NamePressed(_)
      | character_card::Message::SkillTrainingPressed(_)
      | character_card::Message::TagsPressed(_)
      | character_card::Message::WalletPressed(_) => {
        self.context_menu = None;
        Task::none()
      }
    }
  }

  fn update_context_menu(&mut self, inner: context_menu::Message) -> Task<Message> {
    match inner {
      context_menu::Message::Close | context_menu::Message::CopyName => {
        self.context_menu = None;
        Task::none()
      }
      context_menu::Message::EditTags => {
        let id = self.context_menu.as_ref().map(|s| s.character_id);
        self.context_menu = None;
        if let Some(id) = id {
          Task::done(Message::Card(id, character_card::Message::TagsPressed(id)))
        } else {
          Task::none()
        }
      }
      context_menu::Message::RemoveRequested => {
        let id = self.context_menu.as_ref().map(|s| s.character_id);
        self.context_menu = None;
        if let Some(id) = id {
          Task::done(Message::RemoveCharacter(id))
        } else {
          Task::none()
        }
      }
    }
  }

  fn update_drag_moved(&mut self, pt: Point, pane_height: f32) -> Task<Message> {
    self.cursor_position = pt;
    if pt.y < spacing::SCROLL_EDGE_THRESHOLD {
      let new_y = (self.scroll_offset.y - spacing::SCROLL_NUDGE_PX).max(0.0);
      self.scroll_offset.y = new_y;
      return iced::widget::operation::scroll_to(
        GRID_SCROLL_ID.clone(),
        scrollable::AbsoluteOffset {
          x: 0.0,
          y: new_y,
        },
      );
    }
    if pt.y > pane_height - spacing::SCROLL_EDGE_THRESHOLD {
      let new_y = self.scroll_offset.y + spacing::SCROLL_NUDGE_PX;
      self.scroll_offset.y = new_y;
      return iced::widget::operation::scroll_to(
        GRID_SCROLL_ID.clone(),
        scrollable::AbsoluteOffset {
          x: 0.0,
          y: new_y,
        },
      );
    }
    Task::none()
  }
}

impl Default for State {
  fn default() -> Self {
    Self::new()
  }
}

/// Messages for the characters tab.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Message {
  /// Forwarded from a character card.
  Card(i64, character_card::Message),
  /// A character was successfully added.
  CharacterAdded(Character),
  /// Character public data (corp) was refreshed.
  CharacterPublicRefreshed(Vec<(i64, i64, String)>),
  /// Periodic character public data refresh tick.
  CharacterPublicRefreshTick,
  /// Tags were loaded for a specific character.
  CharacterTagsLoaded(i64, Vec<(i32, String)>),
  /// Forwarded from the context menu.
  ContextMenu(context_menu::Message),
  /// Cursor moved; carries the new position.
  CursorMoved(Point),
  /// Drag interaction ended.
  DragEnd,
  /// Cursor moved during an active drag; carries cursor position and pane height.
  DragMoved(Point, f32),
  /// Scroll viewport changed; keeps scroll_offset in sync.
  ScrollOffsetChanged(scrollable::Viewport),
  /// Periodic location refresh tick.
  LocationRefreshTick,
  /// Locations were refreshed with new data.
  LocationsRefreshed(Vec<(i64, Option<String>, Option<bool>)>),
  /// Navigate to the character detail view for a specific character.
  NavigateToDetail(i64),
  /// Navigate to the skills view for a specific character.
  NavigateToSkills(i64),
  /// Navigate to the wallet view for a specific character.
  NavigateToWallet(i64),
  /// Request to remove a character (from context menu).
  RemoveCharacter(i64),
  /// Periodic skill queue refresh tick.
  SkillQueueRefreshTick,
  /// Skill queues were refreshed with new data.
  SkillQueuesRefreshed(Vec<(i64, Vec<CharacterSkill>, Vec<TrainingQueueEntry>)>),
  /// Periodic wallet refresh tick.
  WalletRefreshTick,
  /// Wallets were refreshed with new data.
  WalletsRefreshed(Vec<(i64, Option<f64>)>),
}

pub struct Component<'a> {
  characters: Vec<&'a Character>,
  feat_skill_monitoring: bool,
  feat_wallet: bool,
  state: &'a State,
  window_width: f32,
  pane_height: f32,
}

impl<'a> Component<'a> {
  pub fn new(characters: Vec<&'a Character>, state: &'a State) -> Self {
    Self {
      characters,
      feat_skill_monitoring: true,
      feat_wallet: true,
      state,
      window_width: spacing::layout::WINDOW_DEFAULT_WIDTH,
      pane_height: spacing::layout::WINDOW_DEFAULT_HEIGHT,
    }
  }

  pub fn feat_skill_monitoring(mut self, v: bool) -> Self {
    self.feat_skill_monitoring = v;
    self
  }

  pub fn feat_wallet(mut self, v: bool) -> Self {
    self.feat_wallet = v;
    self
  }

  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  pub fn pane_height(mut self, height: f32) -> Self {
    self.pane_height = height;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let context_menu = self.state.context_menu.as_ref();
    let dragging_id = self.state.dragging_id;
    let cursor = self.state.cursor_position;
    let window_width = self.window_width;
    let feat_skill_monitoring = self.feat_skill_monitoring;
    let feat_wallet = self.feat_wallet;

    // Extract ghost character data before consuming self in render_grid().
    // Both references carry lifetime 'a so they outlive the move.
    let ghost_char: Option<&'a Character> =
      dragging_id.and_then(|id| self.characters.iter().find(|c| *c.id() == id).copied());
    let ghost_handle: Option<&'a iced::widget::image::Handle> =
      dragging_id.and_then(|id| self.state.portrait_handles.get(&id));

    let grid = self.render_grid(feat_skill_monitoring, feat_wallet);
    let mut layers: Vec<Element<'a, Message>> = vec![grid];

    if let Some(ctx) = context_menu {
      layers.push(ContextMenu::new(ctx).render().map(Message::ContextMenu));
    }

    if let Some(ghost_c) = ghost_char {
      layers.push(ghost_overlay(ghost_c, ghost_handle, cursor, window_width));
    }

    if layers.len() == 1 {
      layers.remove(0)
    } else {
      stack(layers).into()
    }
  }

  fn render_grid(self, feat_skill_monitoring: bool, feat_wallet: bool) -> Element<'a, Message> {
    let cols = grid_cols(self.window_width);
    let portrait_handles = &self.state.portrait_handles;
    let dragging_id = self.state.dragging_id;
    let drag_hover = self.state.drag_hover;
    let pane_height = self.pane_height;

    let grid_rows = build_grid_rows(
      self.characters,
      cols,
      portrait_handles,
      dragging_id,
      drag_hover,
      feat_skill_monitoring,
      feat_wallet,
    );

    let grid_column = container(
      column(grid_rows)
        .spacing(spacing::SPACE_4)
        .width(Length::Fill)
        .max_width(spacing::layout::GRID_MAX_WIDTH)
        .padding(Padding {
          top: spacing::SPACE_4,
          bottom: spacing::SPACE_4,
          left: spacing::SPACE_8,
          right: spacing::SPACE_8,
        }),
    )
    .center_x(Length::Fill);

    let scroll = scrollable(grid_column)
      .id(GRID_SCROLL_ID.clone())
      .height(Length::Fill)
      .on_scroll(Message::ScrollOffsetChanged);

    mouse_area(scroll)
      .on_move(move |pt| {
        if dragging_id.is_some() {
          Message::DragMoved(pt, pane_height)
        } else {
          Message::CursorMoved(pt)
        }
      })
      .into()
  }
}

fn build_grid_rows<'a>(
  characters: Vec<&'a Character>,
  cols: usize,
  portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  dragging_id: Option<i64>,
  drag_hover: Option<i64>,
  feat_skill_monitoring: bool,
  feat_wallet: bool,
) -> Vec<Element<'a, Message>> {
  let is_dragging = dragging_id.is_some();

  if cols < 3 || characters.is_empty() {
    return build_grid_rows_responsive(
      characters,
      cols,
      portrait_handles,
      dragging_id,
      drag_hover,
      feat_skill_monitoring,
      feat_wallet,
    );
  }

  let max_slot = characters.iter().map(|c| *c.sort_order()).max().unwrap_or(0);
  let row_count = (max_slot / 3 + 2) as usize;

  let slot_map: HashMap<i32, &Character> = characters.iter().map(|c| (*c.sort_order(), *c)).collect();

  let mut grid_rows: Vec<Element<'a, Message>> = Vec::with_capacity(row_count);
  for row_idx in 0..row_count {
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(3);
    for col_idx in 0i32..3 {
      let slot = row_idx as i32 * 3 + col_idx;
      if let Some(c) = slot_map.get(&slot) {
        let id = *c.id();
        cells.push(
          CharacterCard::new(c)
            .portrait_handle(portrait_handles.get(&id))
            .feat_skill_monitoring(feat_skill_monitoring)
            .feat_wallet(feat_wallet)
            .is_dragging(dragging_id == Some(id))
            .is_hover_target(is_dragging && drag_hover == Some(id) && dragging_id != Some(id))
            .render()
            .map(move |msg| Message::Card(id, msg)),
        );
      } else {
        cells.push(empty_slot_placeholder(is_dragging));
      }
    }
    grid_rows.push(row(cells).spacing(spacing::SPACE_4).into());
  }

  grid_rows
}

fn build_grid_rows_responsive<'a>(
  characters: Vec<&'a Character>,
  cols: usize,
  portrait_handles: &'a HashMap<i64, iced::widget::image::Handle>,
  dragging_id: Option<i64>,
  drag_hover: Option<i64>,
  feat_skill_monitoring: bool,
  feat_wallet: bool,
) -> Vec<Element<'a, Message>> {
  let mut grid_rows: Vec<Element<'a, Message>> = Vec::new();
  for chunk in characters.chunks(cols) {
    let mut cells: Vec<Element<'a, Message>> = chunk
      .iter()
      .map(|c| {
        let id = *c.id();
        CharacterCard::new(c)
          .portrait_handle(portrait_handles.get(&id))
          .feat_skill_monitoring(feat_skill_monitoring)
          .feat_wallet(feat_wallet)
          .is_dragging(dragging_id == Some(id))
          .is_hover_target(dragging_id.is_some() && drag_hover == Some(id) && dragging_id != Some(id))
          .render()
          .map(move |msg| Message::Card(id, msg))
      })
      .collect();
    while cells.len() < cols {
      cells.push(iced::widget::Space::new().width(Length::Fill).into());
    }
    grid_rows.push(row(cells).spacing(spacing::SPACE_4).into());
  }
  grid_rows
}

fn empty_slot_placeholder<'a>(is_dragging: bool) -> Element<'a, Message> {
  container(iced::widget::Space::new().width(Length::Fill))
    .width(Length::Fill)
    .height(spacing::layout::CHARACTER_CARD_HEIGHT)
    .style(move |_| {
      if is_dragging {
        container::Style {
          border: Border {
            color: color::border::SUBTLE,
            radius: radius::PANEL.into(),
            width: 1.0,
          },
          ..container::Style::default()
        }
      } else {
        container::Style::default()
      }
    })
    .into()
}

fn ghost_card_container<'a>(content: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
  let mut bg = color::surface::RAISED;
  bg.a = 0.96;

  container(content)
    .width(Length::Fill)
    .height(spacing::layout::CHARACTER_CARD_HEIGHT)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: color::border::DEFAULT,
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      shadow: shadow::POPOVER,
      ..container::Style::default()
    })
    .into()
}

// Non-interactive ghost that follows the cursor during drag.
// Mirrors the full card layout but has zero mouse_area elements, so all
// events fall through to the grid cards beneath (preserving CardEntered).
fn ghost_element<'a>(
  character: &'a Character,
  portrait_handle: Option<&'a iced::widget::image::Handle>,
) -> Element<'a, Message> {
  use crate::components;

  let portrait = character_card::CharacterPortrait::new(character)
    .portrait_handle(portrait_handle)
    .render::<Message>();

  let identity = container(character_card::CharacterDetail::new(character).render::<Message>())
    .padding(Padding {
      right: spacing::SPACE_4,
      ..Padding::ZERO
    })
    .into();

  let tags = ghost_tags(character);
  let training = character_card::CharacterSkillTraining::new(character).render::<Message>();
  let stats = ghost_stats(character);

  let card_content = column([
    portrait,
    identity,
    tags,
    components::Separator::horizontal().render(),
    training,
    components::Separator::horizontal().render(),
    stats,
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  ghost_card_container(card_content)
}

fn ghost_overlay<'a>(
  ghost_c: &'a Character,
  ghost_handle: Option<&'a iced::widget::image::Handle>,
  cursor: Point,
  window_width: f32,
) -> Element<'a, Message> {
  let cols = grid_cols(window_width);
  let effective_width = window_width.min(spacing::layout::GRID_MAX_WIDTH);
  let card_width = (effective_width - spacing::SPACE_8 * 2.0 - spacing::SPACE_4 * (cols - 1) as f32) / cols as f32;

  let ghost_left = (cursor.x - card_width / 2.0).max(0.0);
  let ghost_top = (cursor.y - spacing::layout::CHARACTER_CARD_HEIGHT * 0.3).max(0.0);

  container(container(ghost_element(ghost_c, ghost_handle)).width(card_width))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: ghost_top,
      left: ghost_left,
      ..Padding::ZERO
    })
    .into()
}

fn ghost_stats<'a>(character: &'a Character) -> Element<'a, Message> {
  let location = character.location_name().as_deref();
  let divider: Element<'a, Message> = container(iced::widget::Space::new().height(Length::Fill))
    .width(1.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into();

  row([
    character_card::CharacterLocation::new(location).render::<Message>(),
    divider,
    character_card::CharacterWallet::new(character).render::<Message>(),
  ])
  .into()
}

fn ghost_tags<'a>(character: &'a Character) -> Element<'a, Message> {
  use crate::components;

  let tag_children: Vec<Element<'a, Message>> = character
    .tags()
    .iter()
    .map(|(_, name)| components::Badge::tag(name).render::<Message>())
    .collect();

  if tag_children.is_empty() {
    iced::widget::Space::new().width(Length::Shrink).height(0).into()
  } else {
    container(row(tag_children).spacing(spacing::SPACE_1).wrap())
      .padding(Padding {
        top: 0.0,
        bottom: 10.0,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}

fn grid_cols(window_width: f32) -> usize {
  if window_width >= 1000.0 {
    3
  } else if window_width >= 700.0 {
    2
  } else {
    1
  }
}
