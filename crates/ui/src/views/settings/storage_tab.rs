//! Storage settings panel: override database, cache, and log paths.

use std::path::PathBuf;

use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text, text_input},
};

use crate::style::{color, spacing, typography};

/// Which storage path is being edited in the UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathId {
  /// Directory that holds `pod.db`.
  CacheDir,
  /// Directory for rolling log files.
  DbDir,
  /// Directory for the ESI HTTP cache.
  LogDir,
}

/// Messages produced by the storage settings panel.
#[derive(Clone, Debug)]
pub enum Message {
  /// User clicked Browse for the given path.
  Browse(PathId),
  /// Commit the pending path change for the given id.
  Commit(PathId),
  /// User confirmed "Move" in the move-or-skip dialog.
  ConfirmMove(PathId),
  /// User confirmed "Skip" in the move-or-skip dialog.
  ConfirmSkip(PathId),
  /// User cancelled the pending path change.
  ConfirmCancel(PathId),
  /// OS picker returned a folder path.
  PathSelected(PathId, PathBuf),
  /// Reset the given path back to its platform default (None).
  ResetPath(PathId),
}

/// State for a single path row in the storage panel.
#[derive(Clone, Debug, Default)]
pub struct PathRowState {
  /// The currently displayed/draft path text.
  pub draft: String,
  /// The previous path (used to restore on Cancel).
  pub previous: String,
  /// Whether a move-or-skip confirmation dialog is showing.
  pub confirm_move: bool,
}

/// Runtime state for the storage settings panel.
#[derive(Clone, Debug, Default)]
pub struct State {
  /// State for the database directory row.
  pub cache_dir: PathRowState,
  /// State for the log directory row.
  pub db_dir: PathRowState,
  /// State for the ESI cache directory row.
  pub log_dir: PathRowState,
}

impl State {
  /// Initialise from optional saved paths (None = platform default).
  pub fn from_paths(db_dir: Option<&PathBuf>, cache_dir: Option<&PathBuf>, log_dir: Option<&PathBuf>) -> Self {
    Self {
      cache_dir: PathRowState {
        draft: cache_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        previous: cache_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        confirm_move: false,
      },
      db_dir: PathRowState {
        draft: db_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        previous: db_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        confirm_move: false,
      },
      log_dir: PathRowState {
        draft: log_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        previous: log_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        confirm_move: false,
      },
    }
  }

  /// Return a mutable reference to the row for `id`.
  pub fn row_mut(&mut self, id: &PathId) -> &mut PathRowState {
    match id {
      PathId::CacheDir => &mut self.cache_dir,
      PathId::DbDir => &mut self.db_dir,
      PathId::LogDir => &mut self.log_dir,
    }
  }

  /// Return an immutable reference to the row for `id`.
  pub fn row(&self, id: &PathId) -> &PathRowState {
    match id {
      PathId::CacheDir => &self.cache_dir,
      PathId::DbDir => &self.db_dir,
      PathId::LogDir => &self.log_dir,
    }
  }
}

/// Builder for the storage settings panel.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Create a new storage panel builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    render_storage_panel(self.state)
  }
}

fn render_storage_panel(state: &State) -> Element<'_, Message> {
  let header = storage_panel_header();
  let inner_header_border = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    });
  let body = storage_panel_body(state);
  column([header, inner_header_border.into(), body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn storage_panel_header() -> Element<'static, Message> {
  let title = text("Storage").size(18.0).color(color::text::PRIMARY);
  let desc = text(
    "Override the default locations for Pod\u{2019}s database, \
    cache, and log files. Changes take effect after restarting Pod.",
  )
  .size(13.0)
  .color(color::text::SECONDARY);
  column([
    row([title.into(), Space::new().width(Length::Fill).into()])
      .align_y(Vertical::Center)
      .into(),
    Space::new().height(4.0).into(),
    desc.into(),
  ])
  .padding(Padding {
    top: 24.0,
    bottom: spacing::SPACE_3_5,
    left: 36.0,
    right: 36.0,
  })
  .into()
}

fn storage_panel_body(state: &State) -> Element<'_, Message> {
  let rows: Vec<Element<'_, Message>> = vec![
    path_row(
      "Database",
      "Directory that holds pod.db (the main data store)",
      PathId::DbDir,
      &state.db_dir,
    ),
    path_row_separator(),
    path_row(
      "Cache",
      "Directory for the ESI HTTP response cache",
      PathId::CacheDir,
      &state.cache_dir,
    ),
    path_row_separator(),
    path_row(
      "Logs",
      "Directory for rolling diagnostic log files",
      PathId::LogDir,
      &state.log_dir,
    ),
  ];

  iced::widget::scrollable(column(rows).width(Length::Fill).padding(Padding {
    top: 0.0,
    bottom: 60.0,
    left: 36.0,
    right: 36.0,
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn path_row_separator() -> Element<'static, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn path_row<'a>(
  label: &'static str,
  description: &'static str,
  id: PathId,
  row_state: &'a PathRowState,
) -> Element<'a, Message> {
  if row_state.confirm_move {
    return path_row_confirm(label, id);
  }

  let id_browse = id.clone();
  let id_reset = id.clone();
  let id_commit = id.clone();

  let label_el = text(label)
    .size(14.0)
    .font(typography::body::MEDIUM)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });
  let desc_el = text(description).size(12.0).style(|_| iced::widget::text::Style {
    color: Some(color::text::SECONDARY),
  });
  let meta_col: Element<'_, Message> = column([label_el.into(), Space::new().height(2.0).into(), desc_el.into()])
    .width(Length::FillPortion(2))
    .into();

  let input = text_input("Platform default", &row_state.draft)
    .size(13.0)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 10.0,
      right: 10.0,
    })
    .on_submit(Message::Commit(id_commit));

  let browse_btn = button(
    text("Browse\u{2026}")
      .size(12.0)
      .font(typography::body::REGULAR)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::Browse(id_browse))
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::surface::RAISED,
      _ => color::surface::SUNKEN,
    })),
    border: iced::Border {
      color: color::border::SUBTLE,
      radius: 5.0.into(),
      width: 1.0,
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  });

  let reset_btn =
    button(
      text("Reset")
        .size(12.0)
        .font(typography::body::REGULAR)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::ResetPath(id_reset))
    .style(|_, status| button::Style {
      background: Some(Background::Color(match status {
        button::Status::Hovered | button::Status::Pressed => color::surface::RAISED,
        _ => iced::Color::TRANSPARENT,
      })),
      border: iced::Border {
        color: color::border::SUBTLE,
        radius: 5.0.into(),
        width: 1.0,
      },
      text_color: color::text::SECONDARY,
      ..button::Style::default()
    });

  let controls: Element<'_, Message> = row([
    input.into(),
    Space::new().width(spacing::SPACE_2).into(),
    browse_btn.into(),
    Space::new().width(spacing::SPACE_2).into(),
    reset_btn.into(),
  ])
  .align_y(Vertical::Center)
  .width(Length::FillPortion(3))
  .into();

  container(
    row([meta_col, Space::new().width(spacing::SPACE_7).into(), controls])
      .align_y(Vertical::Center)
      .padding(Padding {
        top: 16.0,
        bottom: 16.0,
        left: 0.0,
        right: 0.0,
      }),
  )
  .width(Length::Fill)
  .into()
}

fn path_row_confirm(label: &'static str, id: PathId) -> Element<'static, Message> {
  let id_move = id.clone();
  let id_skip = id.clone();
  let id_cancel = id;

  let label_el = text(format!("Move existing {label} files?"))
    .size(14.0)
    .font(typography::body::MEDIUM)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let move_btn = action_confirm_button("Move", Message::ConfirmMove(id_move));
  let skip_btn = action_confirm_button("Skip", Message::ConfirmSkip(id_skip));
  let cancel_btn = ghost_confirm_button("Cancel", Message::ConfirmCancel(id_cancel));

  container(
    row([
      label_el.into(),
      Space::new().width(Length::Fill).into(),
      cancel_btn,
      Space::new().width(spacing::SPACE_2).into(),
      skip_btn,
      Space::new().width(spacing::SPACE_2).into(),
      move_btn,
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 16.0,
      bottom: 16.0,
      left: 0.0,
      right: 0.0,
    }),
  )
  .width(Length::Fill)
  .into()
}

fn action_confirm_button(label: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    text(label)
      .size(12.0)
      .font(typography::body::MEDIUM)
      .style(|_| iced::widget::text::Style {
        color: Some(color::surface::BASE),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_HOVER,
      _ => color::accent::PLASMA,
    })),
    border: iced::Border {
      radius: 5.0.into(),
      ..iced::Border::default()
    },
    text_color: color::surface::BASE,
    ..button::Style::default()
  })
  .into()
}

fn ghost_confirm_button(label: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    text(label)
      .size(12.0)
      .font(typography::body::REGULAR)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::surface::RAISED,
      _ => iced::Color::TRANSPARENT,
    })),
    border: iced::Border {
      color: color::border::SUBTLE,
      radius: 5.0.into(),
      width: 1.0,
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}
