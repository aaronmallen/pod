//! Storage settings panel: override database, cache, and log paths.

use std::path::PathBuf;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, row, text, text_input},
};

use crate::style::{color, spacing, typography};

/// Which storage path is being edited in the UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathId {
  /// Directory for the ESI HTTP cache (`XDG_CACHE_HOME`).
  CacheDir,
  /// Directory that holds `pod.db` (`XDG_DATA_HOME`).
  DbDir,
  /// Directory for rolling log files (`XDG_STATE_HOME`).
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
  /// User typed in the path text input.
  Edited(PathId, String),
  /// OS picker returned a folder path.
  PathSelected(PathId, PathBuf),
  /// Reset the given path back to its platform default (None).
  ResetPath(PathId),
  /// User toggled the network database checkbox.
  ToggleNetworkDb,
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
  /// State for the ESI cache directory row.
  pub cache_dir: PathRowState,
  /// Platform-default ESI cache directory (shown as hint).
  pub cache_default: String,
  /// State for the database directory row.
  pub db_dir: PathRowState,
  /// Platform-default database directory (shown as hint).
  pub db_default: String,
  /// State for the log directory row.
  pub log_dir: PathRowState,
  /// Platform-default log directory (shown as hint).
  pub log_default: String,
  /// Whether the database is on a network drive (disables WAL mode).
  pub network_db: bool,
}

impl State {
  /// Initialise from optional saved paths, platform defaults, and network flag.
  ///
  /// `*_default` strings should be the OS-resolved default for each
  /// path (i.e. what would be used when the override is `None`).
  pub fn from_paths(
    db_dir: Option<&PathBuf>,
    cache_dir: Option<&PathBuf>,
    log_dir: Option<&PathBuf>,
    db_default: String,
    cache_default: String,
    log_default: String,
    network_db: bool,
  ) -> Self {
    Self {
      cache_dir: PathRowState {
        draft: cache_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        previous: cache_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        confirm_move: false,
      },
      cache_default,
      db_dir: PathRowState {
        draft: db_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        previous: db_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        confirm_move: false,
      },
      db_default,
      log_dir: PathRowState {
        draft: log_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        previous: log_dir.map(|p| p.display().to_string()).unwrap_or_default(),
        confirm_move: false,
      },
      log_default,
      network_db,
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

  /// Return the platform-default string for `id`.
  pub fn default_for(&self, id: &PathId) -> &str {
    match id {
      PathId::CacheDir => &self.cache_default,
      PathId::DbDir => &self.db_default,
      PathId::LogDir => &self.log_default,
    }
  }

  /// Count how many paths are currently overriding the platform default.
  pub fn customized_count(&self) -> usize {
    [&self.cache_dir, &self.db_dir, &self.log_dir]
      .iter()
      .filter(|r| !r.draft.is_empty())
      .count()
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
  let header = storage_panel_header(state);
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

fn storage_panel_header(state: &State) -> Element<'_, Message> {
  let count = state.customized_count();
  let count_label = if count == 0 {
    "All defaults".to_string()
  } else {
    format!("{count} of 3 customized")
  };

  let title_row: Element<'_, Message> = row([
    text("Storage").size(18.0).color(color::text::PRIMARY).into(),
    Space::new().width(Length::Fill).into(),
    text(count_label)
      .size(11.0)
      .font(typography::mono::REGULAR)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::MUTED),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into();

  let desc = text(
    "Override the XDG / platform default locations for Pod\u{2019}s database, \
    cache, and log files. Changes take effect after restarting Pod.",
  )
  .size(13.0)
  .color(color::text::SECONDARY);

  column([title_row, Space::new().height(4.0).into(), desc.into()])
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
      "XDG_DATA_HOME",
      PathId::DbDir,
      &state.db_dir,
      &state.db_default,
    ),
    network_drive_row(state.network_db),
    path_row_separator(),
    path_row(
      "Cache",
      "Directory for the ESI HTTP response cache",
      "XDG_CACHE_HOME",
      PathId::CacheDir,
      &state.cache_dir,
      &state.cache_default,
    ),
    path_row_separator(),
    path_row(
      "Logs",
      "Directory for rolling diagnostic log files",
      "XDG_STATE_HOME",
      PathId::LogDir,
      &state.log_dir,
      &state.log_default,
    ),
    path_row_separator(),
    storage_info_note(),
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

fn network_drive_row(checked: bool) -> Element<'static, Message> {
  let checkbox = network_drive_checkbox(checked);

  let label_el = text("Database is on a network drive")
    .size(13.0)
    .font(typography::body::MEDIUM)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let label_row: Element<'_, Message> = if checked {
    let wal_badge: Element<'_, Message> = container(text("WAL OFF").size(10.0).font(typography::mono::REGULAR).style(
      |_| iced::widget::text::Style {
        color: Some(color::accent::GOLD),
      },
    ))
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::GOLD_SUBTLE)),
      border: iced::Border {
        color: color::accent::GOLD_MUTED,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into();
    row([label_el.into(), Space::new().width(spacing::SPACE_2).into(), wal_badge])
      .align_y(Vertical::Center)
      .into()
  } else {
    label_el.into()
  };

  let desc_el = text(
    "SQLite WAL mode is incompatible with NFS and some SMB mounts. \
    When enabled, Pod uses journal_mode=DELETE instead, which is \
    safe on network drives but slightly slower.",
  )
  .size(12.0)
  .style(|_| iced::widget::text::Style {
    color: Some(color::text::SECONDARY),
  });

  let text_col: Element<'_, Message> = column([label_row, Space::new().height(2.0).into(), desc_el.into()]).into();

  let row_content: Element<'_, Message> = row([checkbox, Space::new().width(spacing::SPACE_3).into(), text_col])
    .align_y(Vertical::Center)
    .into();

  container(row_content)
    .width(Length::Fill)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: spacing::SPACE_7,
      right: 0.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn network_drive_checkbox(checked: bool) -> Element<'static, Message> {
  let check_mark: Element<'_, Message> = if checked {
    text("\u{2713}")
      .size(11.0)
      .font(typography::body::MEDIUM)
      .style(|_| iced::widget::text::Style {
        color: Some(color::surface::BASE),
      })
      .into()
  } else {
    Space::new().width(14.0).height(14.0).into()
  };

  let bg_color = if checked {
    color::accent::PLASMA
  } else {
    iced::Color::TRANSPARENT
  };
  let border_color = if checked {
    color::accent::PLASMA
  } else {
    color::border::DEFAULT
  };

  button(
    container(check_mark)
      .width(14.0)
      .height(14.0)
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .width(18.0)
  .height(18.0)
  .padding(Padding::ZERO)
  .on_press(Message::ToggleNetworkDb)
  .style(move |_, _| button::Style {
    background: Some(Background::Color(bg_color)),
    border: iced::Border {
      color: border_color,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..button::Style::default()
  })
  .into()
}

fn storage_info_note() -> Element<'static, Message> {
  let note = text(
    "Tilde (~) is not expanded \u{2014} use absolute paths. \
    Missing directories are created automatically. \
    Changing a path does not move existing data; use Browse to trigger migration.",
  )
  .size(11.0)
  .style(|_| iced::widget::text::Style {
    color: Some(color::text::MUTED),
  });

  container(note)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_4,
      bottom: 0.0,
      left: 0.0,
      right: 0.0,
    })
    .into()
}

fn path_row<'a>(
  label: &'static str,
  description: &'static str,
  xdg_var: &'static str,
  id: PathId,
  row_state: &'a PathRowState,
  default_path: &'a str,
) -> Element<'a, Message> {
  if row_state.confirm_move {
    return path_row_confirm(label, id);
  }

  let is_custom = !row_state.draft.is_empty();

  let id_browse = id.clone();
  let id_reset = id.clone();
  let id_commit = id.clone();
  let id_edit = id.clone();

  let label_el = text(label)
    .size(14.0)
    .font(typography::body::MEDIUM)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let xdg_el = text(xdg_var)
    .size(10.0)
    .font(typography::mono::REGULAR)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::MUTED),
    });

  let desc_el = text(description).size(12.0).style(|_| iced::widget::text::Style {
    color: Some(color::text::SECONDARY),
  });

  let label_row: Element<'_, Message> = row([
    label_el.into(),
    Space::new().width(spacing::SPACE_2).into(),
    xdg_el.into(),
  ])
  .align_y(Vertical::Center)
  .into();

  let mut meta_items: Vec<Element<'_, Message>> = vec![label_row, Space::new().height(2.0).into(), desc_el.into()];

  if is_custom {
    let chip: Element<'_, Message> =
      container(
        text("custom")
          .size(10.0)
          .font(typography::mono::REGULAR)
          .style(|_| iced::widget::text::Style {
            color: Some(color::accent::GOLD),
          }),
      )
      .padding(Padding {
        top: 2.0,
        bottom: 2.0,
        left: 6.0,
        right: 6.0,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::GOLD_SUBTLE)),
        border: iced::Border {
          color: color::accent::GOLD_MUTED,
          radius: 4.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into();

    meta_items.push(Space::new().height(4.0).into());
    meta_items.push(chip);
  }

  let meta_col: Element<'_, Message> = column(meta_items).width(Length::FillPortion(2)).into();

  let input = text_input("Platform default", &row_state.draft)
    .size(13.0)
    .font(typography::body::REGULAR)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 10.0,
      right: 10.0,
    })
    .on_input(move |s| Message::Edited(id_edit.clone(), s))
    .on_submit(Message::Commit(id_commit))
    .style(|_, _| text_input::Style {
      background: Background::Color(iced::Color::TRANSPARENT),
      border: Border {
        color: color::border::SUBTLE,
        radius: 5.0.into(),
        width: 1.0,
      },
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::state::SELECTION,
    });

  let default_hint = text(format!("Default: {default_path}"))
    .size(10.0)
    .font(typography::mono::REGULAR)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });

  let input_col: Element<'_, Message> =
    column([input.into(), Space::new().height(3.0).into(), default_hint.into()]).into();

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

  let reset_btn = if is_custom {
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
    })
  } else {
    button(
      text("Reset")
        .size(12.0)
        .font(typography::body::REGULAR)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        }),
    )
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      border: iced::Border {
        color: color::border::SUBTLE,
        radius: 5.0.into(),
        width: 1.0,
      },
      text_color: color::text::TERTIARY,
      ..button::Style::default()
    })
  };

  let controls: Element<'_, Message> = row([
    input_col,
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
