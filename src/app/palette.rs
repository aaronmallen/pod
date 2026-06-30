use super::*;

#[derive(Clone, Debug)]
pub(super) enum PaletteMessage {
  Activate(usize),
  ActivateSelected,
  Close,
  MoveDown,
  MoveUp,
  Open,
  QueryChanged(String),
  Select(usize),
}

pub(super) fn palette_overlay(
  state: &command_palette::State,
  entries: Vec<command_palette::Entry>,
) -> Element<'_, Message> {
  command_palette::view(
    state,
    entries,
    |query| Message::Palette(PaletteMessage::QueryChanged(query)),
    |index| Message::Palette(PaletteMessage::Select(index)),
    |index| Message::Palette(PaletteMessage::Activate(index)),
    Message::Palette(PaletteMessage::Close),
  )
}

pub(super) fn palette_key_subscription(app: &App) -> Subscription<Message> {
  // `iced::event::listen_with` only accepts a non-capturing `fn`, so the open/focus context is
  // threaded by picking one of three fixed mappers rather than by capturing into a closure.
  if app.palette.is_some() {
    iced::event::listen_with(map_palette_open)
  } else if app.keyboard_focus.is_text_input_focused() {
    iced::event::listen_with(map_palette_closed_focused)
  } else {
    iced::event::listen_with(map_palette_closed_unfocused)
  }
}

pub(super) fn palette_message(key: shortcuts::PaletteKey) -> Message {
  Message::Palette(match key {
    shortcuts::PaletteKey::Activate => PaletteMessage::ActivateSelected,
    shortcuts::PaletteKey::Close => PaletteMessage::Close,
    shortcuts::PaletteKey::MoveDown => PaletteMessage::MoveDown,
    shortcuts::PaletteKey::MoveUp => PaletteMessage::MoveUp,
    shortcuts::PaletteKey::Open => PaletteMessage::Open,
  })
}

pub(super) fn map_palette_open(event: iced::Event, _status: iced::event::Status, _id: window::Id) -> Option<Message> {
  shortcuts::PaletteKey::for_event(&event, true, false).map(palette_message)
}

pub(super) fn map_palette_closed_focused(
  event: iced::Event,
  _status: iced::event::Status,
  _id: window::Id,
) -> Option<Message> {
  shortcuts::PaletteKey::for_event(&event, false, true).map(palette_message)
}

pub(super) fn map_palette_closed_unfocused(
  event: iced::Event,
  _status: iced::event::Status,
  _id: window::Id,
) -> Option<Message> {
  shortcuts::PaletteKey::for_event(&event, false, false).map(palette_message)
}

pub(super) fn handle_palette(app: &mut App, message: PaletteMessage) -> Task<Message> {
  match message {
    PaletteMessage::Activate(index) => palette_activate(app, index),
    PaletteMessage::ActivateSelected => {
      let index = app.palette.as_ref().map(|state| state.selected).unwrap_or(0);
      palette_activate(app, index)
    }
    PaletteMessage::Close => {
      app.palette = None;
      Task::none()
    }
    PaletteMessage::MoveDown => {
      let count = palette_entries(app).len();
      if let Some(state) = app.palette.as_mut() {
        let max = count.saturating_sub(1);
        state.selected = (state.selected + 1).min(max);
      }
      Task::none()
    }
    PaletteMessage::MoveUp => {
      if let Some(state) = app.palette.as_mut() {
        state.selected = state.selected.saturating_sub(1);
      }
      Task::none()
    }
    PaletteMessage::Open => palette_open(app),
    PaletteMessage::QueryChanged(query) => {
      if let Some(state) = app.palette.as_mut() {
        state.query = query;
        state.selected = 0;
      }
      Task::none()
    }
    PaletteMessage::Select(index) => {
      if let Some(state) = app.palette.as_mut() {
        state.selected = index;
      }
      Task::none()
    }
  }
}

pub(super) fn palette_open(app: &mut App) -> Task<Message> {
  app.palette = Some(command_palette::State::default());
  app.keyboard_focus.set_focused(None);
  iced::widget::operation::focus(command_palette::input_id())
}

pub(super) fn palette_activate(app: &mut App, index: usize) -> Task<Message> {
  let entries = palette_entries(app);
  let Some(entry) = entries.get(index) else {
    return Task::none();
  };
  let action = entry.action.clone();
  palette_activate_action(app, action)
}

pub(super) fn palette_activate_action(app: &mut App, action: PaletteAction) -> Task<Message> {
  app.palette = None;
  match action {
    PaletteAction::Command(command) => palette_command(app, command),
    PaletteAction::Detail(PaletteEntity {
      id,
      kind,
      ..
    }) => match kind {
      PaletteEntityKind::Character => navigate_to_character_detail(app, id),
      PaletteEntityKind::Corporation => navigate_to_corporation_detail(app, id),
    },
    PaletteAction::NavTo(section, sub) => handle_nav_to(app, section.destination, sub),
  }
}

pub(super) fn palette_command(app: &mut App, command: PaletteCommand) -> Task<Message> {
  match command {
    PaletteCommand::AddCharacter => update(app, Message::Auth(auth::Message::Start(feature_flags(app)))),
    PaletteCommand::ComposeMail => match palette_compose_from(app) {
      Some(from_character_id) => open_compose_window(
        app,
        mail::compose::Seed::Blank {
          from_character_id,
        },
      ),
      None => Task::none(),
    },
    PaletteCommand::CreateStockpile => open_stockpile_editor_window(app, assets::EditorSeed::Blank),
    PaletteCommand::ManageSkillPlans => open_manage_plans_window(app),
    PaletteCommand::OpenSettings => handle_nav(app, rail::Destination::Settings),
    PaletteCommand::SyncNow => sync_now(app),
    PaletteCommand::ToggleHighContrast => toggle_high_contrast(app),
  }
}

pub(super) fn palette_compose_from(app: &App) -> Option<i64> {
  if let Some(from) = app.mail.as_ref().and_then(mail::State::default_from) {
    return Some(from);
  }
  let roster = app.roster.as_ref().map(roster::owned_roster).unwrap_or_default();
  resolve_mail_target(&roster, app.selected_character)
}

pub(super) fn toggle_high_contrast(app: &mut App) -> Task<Message> {
  let enabled = !app.accessibility.high_contrast();
  app.accessibility.set_high_contrast(enabled);
  color::set_high_contrast(enabled);
  if let Some(runtime) = app.runtime.as_mut() {
    runtime.settings.accessibility_mut().set_high_contrast(enabled);
    config::save(&runtime.settings);
  }
  if let (Some(runtime), Some(_)) = (app.runtime.as_ref(), app.settings.as_ref()) {
    app.settings = Some(settings::State::new(runtime.settings.clone(), runtime.db.clone()));
  }
  refresh_all_windows(app)
}

pub(super) fn palette_entries(app: &App) -> Vec<command_palette::Entry> {
  let query = app.palette.as_ref().map(|state| state.query.as_str()).unwrap_or("");
  command_palette::build_entries(
    &enabled_features(app),
    &palette_characters(app),
    &palette_corporations(app),
    query,
  )
}

pub(super) fn palette_characters(app: &App) -> Vec<(i64, String)> {
  app
    .roster
    .as_ref()
    .map(roster::owned_roster)
    .unwrap_or_default()
    .into_iter()
    .map(|pilot| (pilot.id, pilot.name))
    .collect()
}

pub(super) fn palette_corporations(app: &App) -> Vec<(i64, String)> {
  app.roster.as_ref().map(roster::owned_corporations).unwrap_or_default()
}
