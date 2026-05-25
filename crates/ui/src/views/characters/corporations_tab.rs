//! Grid pane / tab that displays all tracked corporations as cards.

pub mod context_menu;
pub mod corporation_card;
mod grid_row;

use std::collections::HashMap;

pub use context_menu::Component as ContextMenu;
pub use corporation_card::Component as CorporationCard;
use iced::{
  Element, Length, Padding, Point, Task,
  widget::{column, container, mouse_area, stack},
};
use pod_model::{Character, Corporation};

use crate::style::spacing;

/// State for the corporation pane.
pub struct State {
  /// Open context menu, if any.
  pub context_menu: Option<context_menu::State>,
  /// Cached image handles keyed by corporation id.
  pub icon_handles: HashMap<i64, iced::widget::image::Handle>,
  cursor_position: Point,
}

impl State {
  pub fn new() -> Self {
    Self {
      context_menu: None,
      cursor_position: Point::ORIGIN,
      icon_handles: HashMap::new(),
    }
  }

  pub fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
      Message::Card(_, corporation_card::Message::CardRightPressed(id, name)) => {
        self.context_menu = Some(context_menu::State {
          corporation_id: id,
          corporation_name: name,
          x: self.cursor_position.x,
          y: self.cursor_position.y,
        });
        Task::none()
      }
      Message::ContextMenu(inner) => match inner {
        context_menu::Message::Close => {
          self.context_menu = None;
          Task::none()
        }
        context_menu::Message::RemoveRequested => {
          let id = self.context_menu.as_ref().map(|s| s.corporation_id);
          self.context_menu = None;
          if let Some(id) = id {
            Task::done(Message::RemoveCorporation(id))
          } else {
            Task::none()
          }
        }
      },
      Message::CursorMoved(pt) => {
        self.cursor_position = pt;
        Task::none()
      }
      Message::Card(_, corporation_card::Message::TagsPressed(_)) => Task::none(),
      Message::CorporationAdded(_)
      | Message::CorporationRemoved(_)
      | Message::CorporationsLoaded(_)
      | Message::CorporationTagsLoaded(_, _)
      | Message::CorpPublicRefreshed(_)
      | Message::CorpPublicRefreshTick
      | Message::CorpWalletRefreshTick
      | Message::HqNamesLoaded(_)
      | Message::RemoveCorporation(_) => Task::none(),
    }
  }
}

impl Default for State {
  fn default() -> Self {
    Self::new()
  }
}

/// Messages for the corporations tab.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
  /// Forwarded from a corporation card.
  Card(i64, corporation_card::Message),
  /// Forwarded from the context menu.
  ContextMenu(context_menu::Message),
  /// A corporation was successfully added.
  CorporationAdded(Corporation),
  /// A corporation was removed.
  CorporationRemoved(i64),
  /// Tags were reloaded for a corporation.
  CorporationTagsLoaded(i64, Vec<(i32, String, Option<String>)>),
  /// The corporations list was loaded.
  CorporationsLoaded(Vec<Corporation>),
  /// HQ station names resolved for corporations; carries (corp_id, station_name) pairs.
  HqNamesLoaded(Vec<(i64, String)>),
  /// Public corporation data was refreshed.
  CorpPublicRefreshed(Vec<Corporation>),
  /// Periodic corporation public data refresh tick.
  CorpPublicRefreshTick,
  /// Periodic corporation wallet refresh tick.
  CorpWalletRefreshTick,
  /// Cursor moved; carries the new position.
  CursorMoved(Point),
  /// Request to remove a corporation (from context menu).
  RemoveCorporation(i64),
}

/// Builder for the corporation grid pane.
pub struct Component<'a> {
  characters: &'a [Character],
  corporations: Vec<&'a Corporation>,
  state: &'a State,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new corporation pane for the given list of corporations and pane state.
  pub fn new(corporations: Vec<&'a Corporation>, state: &'a State) -> Self {
    Self {
      characters: &[],
      corporations,
      state,
      window_width: spacing::layout::WINDOW_DEFAULT_WIDTH,
    }
  }

  /// Provides the full character list used to resolve CEO names.
  pub fn characters(mut self, characters: &'a [Character]) -> Self {
    self.characters = characters;
    self
  }

  /// Sets the window width used to compute the grid column count.
  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  /// Renders the corporation grid into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let context_menu = self.state.context_menu.as_ref();
    let grid = self.render_grid();

    match context_menu {
      None => grid,
      Some(ctx) => {
        let menu = ContextMenu::new(ctx).render().map(Message::ContextMenu);
        stack(vec![grid, menu]).into()
      }
    }
  }

  fn render_grid(self) -> Element<'a, Message> {
    let cols = grid_row::corp_grid_cols(self.window_width);
    let icon_handles = &self.state.icon_handles;
    let characters = self.characters;

    let grid_rows = grid_row::build_corp_grid_rows(self.corporations, cols, icon_handles, characters);

    let grid = container(
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

    mouse_area(grid).on_move(Message::CursorMoved).into()
  }
}
