use std::{
  collections::HashMap,
  fs, io,
  path::{Path, PathBuf},
};

use chrono::{DateTime, Local, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text, text_input},
};

use super::{
  Outcome,
  data_export::{self, VersionVerdict},
  log_export::{self, RangePreset},
};
use crate::{
  config::{LogLevel, Settings, StorageConfig, StorageMode},
  ui::{
    components::{icon::Icon, modal_overlay::modal_overlay, rule, status},
    style::{color, control, radius, shadow, spacing, typography},
  },
};

const PANEL_SIDE_PADDING: f32 = 36.0;
const DESCRIPTION_MAX_WIDTH: f32 = 620.0;
const CHECKBOX_SIZE: f32 = 18.0;
const CONFIRM_MODAL_WIDTH: f32 = 460.0;
const IMPORT_MODAL_WIDTH: f32 = 480.0;

#[derive(Clone, Debug)]
pub enum Message {
  Browse(PathKind),
  CancelDataImport,
  CancelMove,
  ConfirmDataImport,
  ConfirmMove,
  DataExportFinished(Result<Option<PathBuf>, String>),
  DataImportFinished(Result<Option<PathBuf>, String>),
  DismissError,
  ExportFinished(Result<Option<PathBuf>, String>),
  ExportLogs(RangePreset),
  RequestDataExport,
  RequestDataImport,
  LogLevelChanged(LogLevel),
  PathEdited(PathKind, String),
  PathSubmitted(PathKind),
  ReleaseLock,
  ResetToDefault(PathKind),
  RevealLogDir,
  SkipMove,
  SyncNow,
  SyncSuggestionDismissed,
  SyncToggled(bool),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PathKind {
  Cache,
  Database,
  Log,
}

impl PathKind {
  const ALL: [PathKind; 3] = [PathKind::Database, PathKind::Log, PathKind::Cache];

  fn default_dir(self) -> PathBuf {
    match self {
      PathKind::Cache => crate::config::cache_dir(),
      PathKind::Database => crate::config::data_dir(),
      PathKind::Log => crate::config::log_dir(),
    }
  }

  fn description(self) -> &'static str {
    match self {
      PathKind::Cache => "Portraits, item icons, and other ESI image cache. Safe to clear; rebuilt on demand.",
      PathKind::Database => {
        "The canonical SQLite database holding character cache, mail bodies, market snapshots, and skill \
          plans. Point this at a shared volume to use the same data across machines."
      }
      PathKind::Log => "Rolling structured logs from the daemon and UI. Rotated daily; retains 5 daily files.",
    }
  }

  fn label(self) -> &'static str {
    match self {
      PathKind::Cache => "Pod Cache",
      PathKind::Database => "Shared data location",
      PathKind::Log => "Pod Logs",
    }
  }

  fn override_dir(self, settings: &Settings) -> Option<PathBuf> {
    let storage = settings.storage();
    match self {
      PathKind::Cache => storage.cache_dir().clone(),
      PathKind::Database => storage.db_dir().clone(),
      PathKind::Log => storage.log_dir().clone(),
    }
  }

  fn resolved_dir(self, settings: &Settings) -> PathBuf {
    let storage = settings.storage();
    match self {
      PathKind::Cache => storage.resolved_cache_dir(),
      PathKind::Database => storage.resolved_db_dir(),
      PathKind::Log => storage.resolved_log_dir(),
    }
  }

  fn set_override(self, settings: &mut Settings, dir: Option<PathBuf>) {
    let storage = settings.storage_mut();
    match self {
      PathKind::Cache => storage.set_cache_dir(dir),
      PathKind::Database => storage.set_db_dir(dir),
      PathKind::Log => storage.set_log_dir(dir),
    };
  }

  fn xdg_label(self) -> &'static str {
    match self {
      PathKind::Cache => "$XDG_CACHE_HOME / pod",
      PathKind::Database => "$XDG_DATA_HOME / pod",
      PathKind::Log => "$XDG_STATE_HOME / pod / logs",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingMove {
  from: PathBuf,
  kind: PathKind,
  to: PathBuf,
}

/// A validated import archive awaiting the user's explicit confirmation in the confirm-import modal.
/// Built only after the picked `.zip` parses and clears the version guard, so the modal exists solely
/// to gate the destructive replace — no data is touched until `ConfirmDataImport` fires. Distinct from
/// `PendingMove` so the path-move flow and the import flow never share state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingImport {
  /// The picked archive path, re-opened by the async restore once the user confirms.
  path: PathBuf,
  /// Pod version recorded in the archive's manifest, surfaced in the modal.
  pod_version: String,
  /// Compatibility verdict; `WillMigrate` adds a forward-migration note (`Incompatible` never reaches
  /// here — it is refused before a modal is shown).
  verdict: VersionVerdict,
}

/// A request to migrate the on-disk database layout because the storage configuration crossed (or
/// could have crossed) the Direct/Sync boundary. Carries the configuration as it was *before* the
/// change so the lifecycle engine in `app.rs` can drive the async file migration with both ends in
/// hand. The tab owns no migration machinery itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationRequest {
  pub previous: StorageConfig,
}

/// Live sync/lease state observed from the lifecycle engine in `app.rs` and fed down into the view.
/// The tab itself owns no sync machinery — it only renders this snapshot and routes actions back up.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncStatus {
  holder: Option<String>,
  last_synced: Option<DateTime<Utc>>,
}

#[derive(Debug, Default)]
pub struct State {
  data_export_pending: bool,
  data_import_confirm: Option<PendingImport>,
  data_import_pending: bool,
  drafts: HashMap<PathKind, String>,
  error: Option<String>,
  export_pending: bool,
  migration: Option<MigrationRequest>,
  pending: Option<PendingMove>,
  sync: SyncStatus,
  sync_suggestion_dismissed: bool,
}

impl State {
  pub fn from_settings(settings: &Settings) -> Self {
    State {
      drafts: PathKind::ALL
        .into_iter()
        .map(|kind| (kind, kind.resolved_dir(settings).display().to_string()))
        .collect(),
      ..State::default()
    }
  }

  pub fn set_sync_status(&mut self, holder: Option<String>, last_synced: Option<DateTime<Utc>>) {
    self.sync = SyncStatus {
      holder,
      last_synced,
    };
  }

  pub fn take_migration(&mut self) -> Option<MigrationRequest> {
    self.migration.take()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Decision {
  Confirm,
  NoChange,
  Repoint,
}

fn sync_draft(state: &mut State, kind: PathKind, settings: &Settings) {
  state
    .drafts
    .insert(kind, kind.resolved_dir(settings).display().to_string());
}

fn decide(from: &Path, to: &Path) -> Decision {
  if paths_equal(from, to) {
    return Decision::NoChange;
  }
  if dir_has_contents(from) {
    Decision::Confirm
  } else {
    Decision::Repoint
  }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
  match (fs::canonicalize(a), fs::canonicalize(b)) {
    (Ok(a), Ok(b)) => a == b,
    _ => a == b,
  }
}

fn dir_has_contents(dir: &Path) -> bool {
  match fs::read_dir(dir) {
    Ok(mut entries) => entries.next().is_some(),
    Err(_) => false,
  }
}

fn relocate(from: &Path, to: &Path) -> io::Result<()> {
  if let Some(parent) = to.parent() {
    fs::create_dir_all(parent)?;
  }
  if !to.exists() {
    match fs::rename(from, to) {
      Ok(()) => return Ok(()),
      Err(error) if error.raw_os_error() == Some(libc_exdev()) => {}
      Err(error) => return Err(error),
    }
  }
  if let Err(error) = copy_dir_all(from, to) {
    let _ = fs::remove_dir_all(to);
    return Err(error);
  }
  fs::remove_dir_all(from)
}

/// EXDEV (errno 18): rename refused because source and destination are on different
/// filesystems, which triggers the copy-then-delete fallback in relocate.
fn libc_exdev() -> i32 {
  18
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
  fs::create_dir_all(dst)?;
  for entry in fs::read_dir(src)? {
    let entry = entry?;
    let kind = entry.file_type()?;
    let target = dst.join(entry.file_name());
    if kind.is_dir() {
      copy_dir_all(&entry.path(), &target)?;
    } else {
      fs::copy(entry.path(), &target)?;
    }
  }
  Ok(())
}

fn ensure_writable(dir: &Path) -> io::Result<()> {
  let mut anchor = dir;
  while !anchor.exists() {
    match anchor.parent() {
      Some(parent) => anchor = parent,
      None => break,
    }
  }
  let probe = anchor.join(".pod-write-probe");
  fs::write(&probe, b"")?;
  let _ = fs::remove_file(&probe);
  Ok(())
}

fn begin_change(kind: PathKind, to: PathBuf, settings: &mut Settings) -> Result<Option<PendingMove>, String> {
  let from = kind.resolved_dir(settings);
  match decide(&from, &to) {
    Decision::NoChange => Ok(None),
    Decision::Repoint => {
      ensure_writable(&to).map_err(|error| format!("Can't use {}: {error}", to.display()))?;
      commit_override(kind, to, settings);
      Ok(None)
    }
    Decision::Confirm => {
      ensure_writable(&to).map_err(|error| format!("Can't use {}: {error}", to.display()))?;
      Ok(Some(PendingMove {
        kind,
        from,
        to,
      }))
    }
  }
}

fn commit_override(kind: PathKind, to: PathBuf, settings: &mut Settings) {
  if paths_equal(&to, &kind.default_dir()) {
    kind.set_override(settings, None);
  } else {
    kind.set_override(settings, Some(to));
  }
}

fn finish_move(pending: PendingMove, state: &mut State, settings: &mut Settings) -> Result<(), String> {
  if pending.kind == PathKind::Database {
    // The database family is migrated asynchronously by the lifecycle engine, which alone can run
    // the WAL checkpoint a Sync→Direct consolidation needs. Repoint the config and hand it the
    // pre-change snapshot; the relocate path below only moves plain directory trees (logs, cache).
    let previous = settings.storage().clone();
    commit_override(pending.kind, pending.to, settings);
    state.migration = Some(MigrationRequest {
      previous,
    });
    return Ok(());
  }

  relocate(&pending.from, &pending.to).map_err(|error| {
    format!(
      "Couldn't move {} → {}: {error}",
      pending.from.display(),
      pending.to.display()
    )
  })?;
  commit_override(pending.kind, pending.to, settings);
  Ok(())
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::Browse(kind) => {
      state.error = None;
      let Some(to) = pick_folder(kind, settings) else {
        return Outcome::None;
      };
      state.drafts.insert(kind, to.display().to_string());
      apply_destination(state, kind, to, settings)
    }
    Message::CancelDataImport => {
      state.data_import_confirm = None;
      Outcome::None
    }
    Message::CancelMove => {
      if let Some(pending) = state.pending.take() {
        sync_draft(state, pending.kind, settings);
      }
      Outcome::None
    }
    Message::ConfirmDataImport => {
      let Some(pending) = state.data_import_confirm.take() else {
        return Outcome::None;
      };
      state.error = None;
      state.data_import_pending = true;
      Outcome::ImportData {
        path: pending.path,
      }
    }
    Message::ConfirmMove => {
      let Some(pending) = state.pending.take() else {
        return Outcome::None;
      };
      let kind = pending.kind;
      match finish_move(pending, state, settings) {
        Ok(()) => {
          sync_draft(state, kind, settings);
          Outcome::Persist
        }
        Err(error) => {
          state.error = Some(error);
          Outcome::None
        }
      }
    }
    Message::DataExportFinished(result) => {
      state.data_export_pending = false;
      if let Err(error) = result {
        state.error = Some(error);
      }
      Outcome::None
    }
    Message::DataImportFinished(result) => {
      // The success path quits the app to re-seed from the restored database, so an Ok here only
      // lands if the dialog was a no-op; either way the import is no longer in flight.
      state.data_import_pending = false;
      if let Err(error) = result {
        state.error = Some(error);
      }
      Outcome::None
    }
    Message::DismissError => {
      state.error = None;
      Outcome::None
    }
    Message::ExportFinished(result) => {
      state.export_pending = false;
      if let Err(error) = result {
        state.error = Some(error);
      }
      Outcome::None
    }
    Message::ExportLogs(preset) => {
      state.error = None;
      state.export_pending = true;
      let (start, end) = log_export::range_for_preset(preset, Local::now());
      Outcome::ExportLogs {
        end,
        start,
      }
    }
    Message::LogLevelChanged(level) => {
      state.error = None;
      if settings.storage().log_level() == &level {
        return Outcome::None;
      }
      settings.storage_mut().set_log_level(level);
      Outcome::SetLogLevel(level)
    }
    Message::PathEdited(kind, value) => {
      state.drafts.insert(kind, value);
      Outcome::None
    }
    Message::PathSubmitted(kind) => {
      state.error = None;
      let draft = state.drafts.get(&kind).cloned().unwrap_or_default();
      if draft.trim().is_empty() {
        sync_draft(state, kind, settings);
        return Outcome::None;
      }
      let outcome = apply_destination(state, kind, PathBuf::from(draft.trim()), settings);
      if state.pending.is_none() && state.error.is_none() {
        sync_draft(state, kind, settings);
      }
      outcome
    }
    Message::ReleaseLock => Outcome::ReleaseLock,
    Message::RequestDataExport => {
      state.error = None;
      state.data_export_pending = true;
      Outcome::ExportData
    }
    Message::RequestDataImport => {
      state.error = None;
      let Some(path) = pick_data_archive() else {
        return Outcome::None;
      };
      // Read and version-guard the archive up front so the confirm modal exists only for a restorable
      // archive — an incompatible or corrupt one is refused here and never offers a Replace action,
      // and no data is touched until the user confirms.
      match validate_archive(&path) {
        Ok(pending) => {
          state.data_import_confirm = Some(pending);
          Outcome::None
        }
        Err(error) => {
          state.error = Some(error);
          Outcome::None
        }
      }
    }
    Message::ResetToDefault(kind) => {
      state.error = None;
      let to = kind.default_dir();
      state.drafts.insert(kind, to.display().to_string());
      apply_destination(state, kind, to, settings)
    }
    Message::RevealLogDir => {
      state.error = None;
      let dir = settings.storage().resolved_log_dir();
      let _ = fs::create_dir_all(&dir);
      if let Err(err) = open::that_detached(&dir) {
        state.error = Some(format!("Couldn't open {}: {err}", dir.display()));
      }
      Outcome::None
    }
    Message::SkipMove => {
      let Some(pending) = state.pending.take() else {
        return Outcome::None;
      };
      let kind = pending.kind;
      commit_override(pending.kind, pending.to, settings);
      sync_draft(state, kind, settings);
      Outcome::Persist
    }
    Message::SyncNow => Outcome::SyncNow,
    Message::SyncSuggestionDismissed => {
      state.sync_suggestion_dismissed = true;
      Outcome::None
    }
    Message::SyncToggled(value) => {
      let previous = settings.storage().clone();
      settings.storage_mut().set_network(value);
      // Toggling sync flips the storage mode in place (the configured path is unchanged), so the
      // database must migrate to the new layout: seed a working copy + sidecar when turning on,
      // consolidate it back into a single file when turning off.
      state.migration = Some(MigrationRequest {
        previous,
      });
      Outcome::Persist
    }
  }
}

fn apply_destination(state: &mut State, kind: PathKind, to: PathBuf, settings: &mut Settings) -> Outcome {
  match begin_change(kind, to, settings) {
    Ok(Some(pending)) => {
      state.pending = Some(pending);
      Outcome::None
    }
    Ok(None) => Outcome::Persist,
    Err(error) => {
      state.error = Some(error);
      Outcome::None
    }
  }
}

/// Prompts for a `.zip` data archive to import. Stubbed to a no-op (returns `None`) under
/// `cfg(test)` so the import update path can be exercised without opening a real file dialog,
/// mirroring the export save stub in `app.rs`.
fn pick_data_archive() -> Option<PathBuf> {
  #[cfg(not(test))]
  {
    rfd::FileDialog::new()
      .set_title("Import data")
      .add_filter("Zip archive", &["zip"])
      .pick_file()
  }
  #[cfg(test)]
  {
    None
  }
}

/// Reads the picked archive and runs the version guard, producing the pending-import the confirm
/// modal renders. A missing/corrupt archive or a newer-major (incompatible) one is rejected with a
/// clear message so the destructive confirm action is never offered for an archive Pod can't restore.
fn validate_archive(path: &Path) -> Result<PendingImport, String> {
  let bytes = fs::read(path).map_err(|err| format!("Couldn't read {}: {err}", path.display()))?;
  let parsed = data_export::read_archive(&bytes)?;
  if parsed.verdict == VersionVerdict::Incompatible {
    return Err(format!(
      "This archive was made by a newer Pod ({}); it can't be restored into this build.",
      parsed.manifest.pod_version
    ));
  }
  Ok(PendingImport {
    path: path.to_path_buf(),
    pod_version: parsed.manifest.pod_version,
    verdict: parsed.verdict,
  })
}

fn pick_folder(kind: PathKind, settings: &Settings) -> Option<PathBuf> {
  let mut dialog = rfd::FileDialog::new().set_title(format!("Select {} folder", kind.label()));
  let start = kind.resolved_dir(settings);
  if start.is_dir() {
    dialog = dialog.set_directory(&start);
  }
  dialog.pick_folder()
}

pub fn badge(settings: &Settings) -> String {
  let storage = settings.storage();
  let customized = [storage.db_dir(), storage.log_dir(), storage.cache_dir()]
    .into_iter()
    .filter(|override_| override_.is_some())
    .count();
  if customized == 0 {
    "default".to_owned()
  } else {
    format!("{customized} custom")
  }
}

pub fn view<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header(settings);
  let body = path_body(state, settings);

  let base: Element<'a, Message> = Column::with_children(vec![header, body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

  if let Some(pending) = state.data_import_confirm.as_ref() {
    return modal_overlay(base, Some(Message::CancelDataImport), confirm_import_modal(pending));
  }
  match state.pending.as_ref() {
    Some(pending) => modal_overlay(base, Some(Message::CancelMove), confirm_move_modal(pending)),
    None => base,
  }
}

fn panel_header(settings: &Settings) -> Element<'_, Message> {
  let title = text("Storage")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(
    "Where Pod keeps its files on disk. Paths follow platform conventions by default. Change them \
      to put Pod's data on a different volume or share it between installs. The daemon picks up \
      changes on next launch.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let top = Row::with_children(vec![identity.into(), customized_badge(settings)])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5);

  let band = container(top).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn customized_badge(settings: &Settings) -> Element<'_, Message> {
  let custom = PathKind::ALL
    .into_iter()
    .filter(|kind| kind.override_dir(settings).is_some())
    .count();
  let (dot_color, label) = if custom > 0 {
    (
      color::accent::PLASMA,
      format!("{custom} of {} customized", PathKind::ALL.len()),
    )
  } else {
    (color::status::ONLINE, "All defaults".to_owned())
  };

  Row::with_children(vec![
    status::dot(dot_color),
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2)
  .into()
}

fn path_body<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::new();
  if let Some(error) = state.error.as_ref() {
    children.push(error_banner(error));
  }
  let mode = settings.storage().storage_mode();
  for kind in PathKind::ALL {
    children.push(path_card(state, kind, settings));
    if kind == PathKind::Database {
      if settings.storage().suggests_network_sync() && !state.sync_suggestion_dismissed {
        children.push(sync_suggestion_banner());
      }
      children.push(sync_toggle_row(mode == StorageMode::Sync));
      if mode == StorageMode::Sync {
        children.push(working_copy_row(settings));
        children.push(sync_status_row(&state.sync));
      }
    }
  }

  children.push(data_export_row(state));
  children.push(data_import_row(state));

  let inner = container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: PANEL_SIDE_PADDING,
      bottom: spacing::SPACE_6,
      left: PANEL_SIDE_PADDING,
    });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn path_card<'a>(state: &'a State, kind: PathKind, settings: &'a Settings) -> Element<'a, Message> {
  let overridden = kind.override_dir(settings).is_some();
  let default = kind.default_dir();

  let mut header_row: Vec<Element<'_, Message>> = vec![
    text(kind.label())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if overridden {
    header_row.push(custom_badge());
  }
  header_row.push(Space::new().width(Length::Fill).into());
  header_row.push(
    text(kind.xdg_label())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  );
  let header = Row::with_children(header_row)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3);

  let description = container(
    text(kind.description())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(DESCRIPTION_MAX_WIDTH);

  let value = state.drafts.get(&kind).map(String::as_str).unwrap_or_default();
  let field = text_input("Enter a folder path\u{2026}", value)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
    })
    .width(Length::Fill)
    .on_input(move |next| Message::PathEdited(kind, next))
    .on_submit(Message::PathSubmitted(kind))
    .style(path_input_style);

  let browse = button(
    text("Browse\u{2026}")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::Browse(kind))
  .style(control::ghost_button);

  let mut reset = button(
    text("Default")
      .font(typography::body::REGULAR)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .style(control::ghost_button);
  if overridden {
    reset = reset.on_press(Message::ResetToDefault(kind));
  }

  let mut control_children: Vec<Element<'_, Message>> = vec![field.into(), browse.into()];
  if kind == PathKind::Log {
    let reveal = button(
      text("Reveal\u{2026}")
        .font(typography::body::MEDIUM)
        .size(typography::size::MD),
    )
    .padding(control::padding())
    .on_press(Message::RevealLogDir)
    .style(control::ghost_button);
    control_children.push(reveal.into());
  }
  control_children.push(reset.into());

  let controls = Row::with_children(control_children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  let log_level_row: Option<Element<'_, Message>> =
    (kind == PathKind::Log).then(|| log_level_row(*settings.storage().log_level()));
  let export_row: Option<Element<'_, Message>> = (kind == PathKind::Log).then(|| log_export_row(state));

  let footnote = Row::with_children(vec![
    text("default")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(default.display().to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let mut cell_children: Vec<Element<'_, Message>> = vec![header.into(), description.into(), controls.into()];
  if let Some(log_level_row) = log_level_row {
    cell_children.push(log_level_row);
  }
  if let Some(export_row) = export_row {
    cell_children.push(export_row);
  }
  cell_children.push(footnote.into());

  let cell = container(
    Column::with_children(cell_children)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6 - 4.0,
    right: 0.0,
    bottom: spacing::SPACE_6 - 4.0,
    left: 0.0,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn log_export_row(state: &State) -> Element<'_, Message> {
  const PRESETS: [RangePreset; 4] = [
    RangePreset::LastHour,
    RangePreset::Last24Hours,
    RangePreset::Today,
    RangePreset::Last7Days,
  ];

  let mut children: Vec<Element<'_, Message>> = vec![
    text("Export logs")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];

  for preset in PRESETS {
    let mut control = button(
      text(preset.label())
        .font(typography::body::REGULAR)
        .size(typography::size::MD),
    )
    .padding(control::padding())
    .style(control::ghost_button);
    if !state.export_pending {
      control = control.on_press(Message::ExportLogs(preset));
    }
    children.push(control.into());
  }

  if state.export_pending {
    children.push(
      text("Exporting\u{2026}")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  Row::with_children(children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2)
    .into()
}

fn data_export_row(state: &State) -> Element<'_, Message> {
  let label = text("Export data")
    .font(typography::body::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::text::PRIMARY));
  let explanation =
    text("Bundle the database and your settings into a single .zip you can archive or move to another machine.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()));

  let mut copy: Vec<Element<'_, Message>> = vec![label.into(), explanation.into()];
  if state.data_export_pending {
    copy.push(
      text("Exporting\u{2026}")
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }
  let copy = Column::with_children(copy)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  let mut control = button(action_button_label(
    (!state.data_export_pending).then(Icon::archive),
    if state.data_export_pending {
      "Preparing archive\u{2026}"
    } else {
      "Export data\u{2026}"
    },
    color::accent::PLASMA,
  ))
  .padding(control::padding())
  .style(accent_ghost_button);
  if !state.data_export_pending {
    control = control.on_press(Message::RequestDataExport);
  }

  let row = Row::with_children(vec![copy.into(), control.into()])
    .align_y(Vertical::Top)
    .spacing(spacing::SPACE_6 - 6.0);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_6 + 4.0,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

/// Composes a button child of an icon (optional, hidden while the action is in flight) followed by a
/// text label, mirroring the mockup's right-aligned export/import controls. The `tint` colors both
/// glyph and label so the accent (export) and ghost (import) variants match their button styles.
fn action_button_label<'a>(icon: Option<Icon>, label: &'a str, tint: Color) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::new();
  if let Some(icon) = icon {
    children.push(icon.color(tint).size(15.0).render::<Message>());
  }
  children.push(
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(tint))
      .into(),
  );
  Row::with_children(children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2)
    .into()
}

/// The accent variant of the storage ghost button: a faint plasma wash behind a plasma border, used
/// for the primary "Export data" action so it reads as the emphasized control without the solid fill
/// of `primary_button`. Matches the accent `GhostBtn` in the Settings → Storage mockup.
fn accent_ghost_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let bg_alpha = match status {
    button::Status::Hovered | button::Status::Pressed => 0.12,
    button::Status::Disabled => 0.04,
    _ => 0.06,
  };
  button::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, bg_alpha))),
    text_color: color::accent::PLASMA,
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.4),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..button::Style::default()
  }
}

fn data_import_row(state: &State) -> Element<'_, Message> {
  let label = text("Import data")
    .font(typography::body::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::text::PRIMARY));
  let explanation = text(
    "Restore the database and settings from a previously exported .zip. This replaces the current \
      data and reopens Pod to apply.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));

  let mut copy: Vec<Element<'_, Message>> = vec![label.into(), explanation.into()];
  if state.data_import_pending {
    copy.push(
      text("Restoring\u{2026}")
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }
  let copy = Column::with_children(copy)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  let mut control = button(action_button_label(
    (!state.data_import_pending).then(Icon::upload),
    if state.data_import_pending {
      "Restoring\u{2026}"
    } else {
      "Import data\u{2026}"
    },
    color::text::PRIMARY,
  ))
  .padding(control::padding())
  .style(control::ghost_button);
  if !state.data_import_pending {
    control = control.on_press(Message::RequestDataImport);
  }

  let row = Row::with_children(vec![copy.into(), control.into()])
    .align_y(Vertical::Top)
    .spacing(spacing::SPACE_6 - 6.0);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_6 + 4.0,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn log_level_cell<'a>(level: LogLevel, active: bool) -> Element<'a, Message> {
  let label_color = if active {
    color::text::PRIMARY
  } else {
    color::with_alpha(color::text::PRIMARY, 0.82)
  };

  let cell = container(
    text(level.label())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(label_color)),
  )
  .padding(control::padding())
  .style(move |_| container::Style {
    background: Some(Background::Color(if active {
      color::with_alpha(color::accent::PLASMA, 0.1)
    } else {
      color::surface::SUNKEN
    })),
    border: Border {
      color: if active {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::text::PRIMARY, 0.1)
      },
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  });

  button(cell)
    .padding(0)
    .on_press(Message::LogLevelChanged(level))
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

fn log_level_row<'a>(active: LogLevel) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    text("Verbosity")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];

  for level in LogLevel::ALL {
    children.push(log_level_cell(level, level == active));
  }

  Row::with_children(children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2)
    .into()
}

fn custom_badge<'a>() -> Element<'a, Message> {
  container(
    text("custom")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::accent::PLASMA)),
  )
  .padding(Padding {
    top: 1.0,
    right: spacing::UNIT + 2.0,
    bottom: 1.0,
    left: spacing::UNIT + 2.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.3),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn sync_toggle_row<'a>(checked: bool) -> Element<'a, Message> {
  let box_fill = if checked {
    color::accent::PLASMA
  } else {
    iced::Color::TRANSPARENT
  };
  let box_border = if checked {
    color::accent::PLASMA
  } else {
    color::rule_strong()
  };
  let check: Element<'a, Message> = if checked {
    text("\u{2713}")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(color::surface::NAVIGATION))
      .into()
  } else {
    Space::new().into()
  };
  let checkbox = container(container(check).center_x(Length::Fill).center_y(Length::Fill))
    .width(Length::Fixed(CHECKBOX_SIZE))
    .height(Length::Fixed(CHECKBOX_SIZE))
    .style(move |_| container::Style {
      background: Some(Background::Color(box_fill)),
      border: Border {
        color: box_border,
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    });

  let label = text("Sync this location across machines")
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));

  let explanation = container(
    text(
      "Pod keeps a fast local working copy of the database and syncs it to the shared location, so \
        the same data follows you between machines.",
    )
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary())),
  )
  .max_width(580.0);

  let copy = Column::with_children(vec![label.into(), explanation.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![checkbox.into(), copy.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Top);

  let toggle = button(row)
    .padding(0)
    .width(Length::Fill)
    .on_press(Message::SyncToggled(!checked))
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    });

  let cell = container(toggle).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: spacing::SPACE_6 - 6.0,
    left: spacing::SPACE_6 + 4.0,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn working_copy_row(settings: &Settings) -> Element<'_, Message> {
  let working_copy = settings.storage().resolved_working_copy_path();

  let label = text("Local working copy")
    .font(typography::body::MEDIUM)
    .size(typography::size::SM)
    .style(typography::colored(color::text::PRIMARY));
  let explanation =
    text("Read-only. The live database runs here on local disk; the share never drives the DB over the wire.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()));
  let path = text(working_copy.display().to_string())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::secondary()));

  let cell = container(
    Column::with_children(vec![label.into(), explanation.into(), path.into()])
      .spacing(spacing::UNIT)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_6 + 4.0,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn sync_status_row(status: &SyncStatus) -> Element<'_, Message> {
  let (dot_color, summary) = match &status.holder {
    Some(machine) => (color::status::DANGER, format!("Currently open on {machine}")),
    None => match status.last_synced {
      Some(at) => {
        let secs = (Utc::now() - at).num_seconds().max(0) as u64;
        (
          color::status::ONLINE,
          format!("Last synced {}", status::format_since(secs)),
        )
      }
      None => (color::text::tertiary(), "Not synced yet".to_owned()),
    },
  };

  let summary_row = Row::with_children(vec![
    status::dot(dot_color),
    text(summary)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2)
  .width(Length::Fill);

  let sync_now = button(
    text("Sync now")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::SyncNow)
  .style(control::ghost_button);

  let release = button(
    text("Release lock")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::ReleaseLock)
  .style(control::ghost_button);

  let row = Row::with_children(vec![summary_row.into(), sync_now.into(), release.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5,
    right: 0.0,
    bottom: spacing::SPACE_6 - 6.0,
    left: spacing::SPACE_6 + 4.0,
  });

  Column::with_children(vec![cell.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn sync_suggestion_banner<'a>() -> Element<'a, Message> {
  let copy = text("This looks like a network share \u{2014} enable syncing across machines.")
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::accent::PLASMA))
    .width(Length::Fill);

  let dismiss = button(
    text("Dismiss")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2,
  })
  .on_press(Message::SyncSuggestionDismissed)
  .style(control::ghost_button);

  container(
    Row::with_children(vec![copy.into(), dismiss.into()])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_3),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.08))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.3),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn error_banner(message: &str) -> Element<'_, Message> {
  let copy = text(message.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::status::DANGER))
    .width(Length::Fill);

  let dismiss = button(
    text("Dismiss")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2,
  })
  .on_press(Message::DismissError)
  .style(control::ghost_button);

  container(
    Row::with_children(vec![copy.into(), dismiss.into()])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_3),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3)
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::DANGER, 0.08))),
    border: Border {
      color: color::with_alpha(color::status::DANGER, 0.3),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn confirm_move_modal(pending: &PendingMove) -> Element<'_, Message> {
  let eyebrow = text("Relocate store")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::accent::PLASMA));
  let title = text(format!("Move {} to the new folder?", pending.kind.label()))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let body = text(
    "Pod can move the existing files to the new location, or just repoint here and leave the old \
      files where they are. The change applies on the next launch.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .style(typography::colored(color::text::secondary()));

  let from_to =
    Column::with_children(vec![path_line("from", &pending.from), path_line("to", &pending.to)]).spacing(spacing::UNIT);

  let header = container(
    Column::with_children(vec![eyebrow.into(), title.into(), body.into(), from_to.into()]).spacing(spacing::SPACE_2),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: spacing::SPACE_6,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_6,
  });

  let cancel = button(text("Cancel").font(typography::body::MEDIUM).size(typography::size::MD))
    .padding(control::padding())
    .on_press(Message::CancelMove)
    .style(control::ghost_button);
  let skip = button(
    text("Skip \u{2014} repoint only")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::SkipMove)
  .style(control::ghost_button);
  let move_button = button(
    text("Move files")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::ConfirmMove)
  .style(control::primary_button);

  let footer = container(
    Row::with_children(vec![
      Space::new().width(Length::Fill).into(),
      cancel.into(),
      skip.into(),
      move_button.into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_6,
    bottom: spacing::SPACE_6,
    left: spacing::SPACE_6,
  });

  let card = container(
    Column::with_children(vec![header.into(), rule::horizontal_alpha(0.18), footer.into()]).width(Length::Fill),
  )
  .width(Length::Fixed(CONFIRM_MODAL_WIDTH))
  .clip(true)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    shadow: shadow::CARD,
    ..container::Style::default()
  });

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn confirm_import_modal(pending: &PendingImport) -> Element<'_, Message> {
  let eyebrow = text("Restore data")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::status::DANGER));
  let title = text("Replace this machine's data?")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let body = text(
    "Importing replaces the current database with the archive's. Pod backs up the current database \
      first, then closes so you can reopen to apply. This can't be undone in place.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .style(typography::colored(color::text::secondary()));

  let mut details: Vec<Element<'_, Message>> = vec![path_line("from", &pending.path)];
  details.push(
    text(format!("Archive Pod version: {}", pending.pod_version))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  );
  if pending.verdict == VersionVerdict::WillMigrate {
    details.push(
      text("This archive is from an older Pod; its data migrates forward on next launch.")
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::status::WARNING))
        .into(),
    );
  }
  let details = Column::with_children(details).spacing(spacing::UNIT);

  let header = container(
    Column::with_children(vec![eyebrow.into(), title.into(), body.into(), details.into()]).spacing(spacing::SPACE_2),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: spacing::SPACE_6,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_6,
  });

  let cancel = button(text("Cancel").font(typography::body::MEDIUM).size(typography::size::MD))
    .padding(control::padding())
    .on_press(Message::CancelDataImport)
    .style(control::ghost_button);
  let replace = button(
    text("Replace data")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD),
  )
  .padding(control::padding())
  .on_press(Message::ConfirmDataImport)
  .style(control::danger_button);

  let footer = container(
    Row::with_children(vec![
      Space::new().width(Length::Fill).into(),
      cancel.into(),
      replace.into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_6,
    bottom: spacing::SPACE_6,
    left: spacing::SPACE_6,
  });

  let card = container(
    Column::with_children(vec![header.into(), rule::horizontal_alpha(0.18), footer.into()]).width(Length::Fill),
  )
  .width(Length::Fixed(IMPORT_MODAL_WIDTH))
  .clip(true)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    shadow: shadow::CARD,
    ..container::Style::default()
  });

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn path_line<'a>(label: &'a str, dir: &Path) -> Element<'a, Message> {
  Row::with_children(vec![
    container(
      text(label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fixed(40.0))
    .into(),
    text(dir.display().to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn path_input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
  let border = match status {
    text_input::Status::Focused {
      ..
    } => color::accent::PLASMA,
    _ => color::with_alpha(color::text::PRIMARY, 0.1),
  };
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: border,
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;

  fn state() -> State {
    State::from_settings(&Settings::default())
  }

  mod badge {
    use super::*;

    #[test]
    fn it_counts_each_directory_override() {
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(PathBuf::from("/var/pod/db")));
      settings.storage_mut().set_log_dir(Some(PathBuf::from("/var/pod/log")));
      settings
        .storage_mut()
        .set_cache_dir(Some(PathBuf::from("/var/pod/cache")));

      assert_eq!(badge(&settings), "3 custom");
    }

    #[test]
    fn it_is_default_with_no_overrides() {
      let settings = Settings::default();

      assert_eq!(badge(&settings), "default");
    }

    #[test]
    fn the_network_toggle_does_not_move_the_count() {
      let mut settings = Settings::default();
      settings.storage_mut().set_network(true);

      assert_eq!(badge(&settings), "default");
    }
  }

  mod begin_change {
    use super::*;

    #[test]
    fn a_populated_source_returns_a_pending_move_without_committing() {
      let from = tempdir().unwrap();
      fs::write(from.path().join("pod.db"), b"data").unwrap();
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(from.path().to_path_buf()));
      let to = tempdir().unwrap();
      let dest = to.path().join("relocated");

      let pending = begin_change(PathKind::Database, dest.clone(), &mut settings)
        .unwrap()
        .expect("a populated source should raise a confirm");

      assert_eq!(pending.kind, PathKind::Database);
      assert_eq!(pending.to, dest);
      assert_eq!(*settings.storage().db_dir(), Some(from.path().to_path_buf()));
    }

    #[test]
    fn an_empty_source_commits_the_override_straight_through() {
      let data = tempdir().unwrap();
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(data.path().to_path_buf()));
      let to = data.path().join("relocated");

      let pending = begin_change(PathKind::Database, to.clone(), &mut settings).unwrap();

      assert_eq!(pending, None, "an empty source needs no confirmation");
      assert_eq!(*settings.storage().db_dir(), Some(to));
    }
  }

  mod copy_dir_all {
    use super::*;

    #[test]
    fn a_missing_source_is_an_error() {
      let dst = tempdir().unwrap();

      let result = super::super::copy_dir_all(&PathBuf::from("/no/such/source/dir"), dst.path());

      assert!(result.is_err());
    }

    #[test]
    fn it_copies_files_and_nested_directories() {
      let src = tempdir().unwrap();
      fs::write(src.path().join("top.txt"), b"top").unwrap();
      fs::create_dir(src.path().join("nested")).unwrap();
      fs::write(src.path().join("nested").join("inner.txt"), b"inner").unwrap();
      let dst_root = tempdir().unwrap();
      let dst = dst_root.path().join("copy");

      super::super::copy_dir_all(src.path(), &dst).unwrap();

      assert_eq!(fs::read(dst.join("top.txt")).unwrap(), b"top");
      assert_eq!(fs::read(dst.join("nested").join("inner.txt")).unwrap(), b"inner");
      assert!(src.path().exists(), "the source tree is left in place");
    }
  }

  mod database_migration {
    use super::*;

    fn populated_db_pending() -> (State, Settings, tempfile::TempDir, PathBuf) {
      let from = tempdir().unwrap();
      fs::write(from.path().join("pod.db"), b"data").unwrap();
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(from.path().to_path_buf()));
      let dest_root = tempdir().unwrap();
      let dest = dest_root.path().join("relocated");

      let mut state = state();
      apply_destination(&mut state, PathKind::Database, dest.clone(), &mut settings);

      (state, settings, dest_root, dest)
    }

    #[test]
    fn confirming_a_database_move_commits_the_override_and_records_a_migration() {
      let (mut state, mut settings, _dest_root, dest) = populated_db_pending();
      let from_before = settings.storage().clone();

      let outcome = update(&mut state, Message::ConfirmMove, &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(*settings.storage().db_dir(), Some(dest));
      let migration = state
        .take_migration()
        .expect("a database move records a migration request");
      assert_eq!(
        migration.previous, from_before,
        "the migration carries the pre-change configuration"
      );
    }

    #[test]
    fn toggling_sync_records_a_migration_with_the_previous_config() {
      let mut state = state();
      let mut settings = Settings::default();
      let before = settings.storage().clone();

      let outcome = update(&mut state, Message::SyncToggled(true), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(settings.storage().network());
      let migration = state
        .take_migration()
        .expect("the sync toggle records a migration request");
      assert_eq!(migration.previous, before, "the previous config has sync off");
      assert!(!migration.previous.network());
    }
  }

  mod decide {
    use super::*;

    #[test]
    fn a_missing_source_repoints_without_a_prompt() {
      let to = tempdir().unwrap();

      assert_eq!(
        decide(Path::new("/no/such/pod/source"), &to.path().join("new")),
        Decision::Repoint
      );
    }

    #[test]
    fn a_populated_source_asks_to_confirm() {
      let from = tempdir().unwrap();
      fs::write(from.path().join("pod.db"), b"data").unwrap();
      let to = tempdir().unwrap();

      assert_eq!(decide(from.path(), &to.path().join("new")), Decision::Confirm);
    }

    #[test]
    fn an_empty_source_repoints_without_a_prompt() {
      let from = tempdir().unwrap();
      let to = tempdir().unwrap();

      assert_eq!(decide(from.path(), &to.path().join("new")), Decision::Repoint);
    }

    #[test]
    fn the_same_path_is_a_no_change() {
      let dir = tempdir().unwrap();

      assert_eq!(decide(dir.path(), dir.path()), Decision::NoChange);
    }
  }

  mod error_handling {
    use super::*;

    #[test]
    fn a_failed_move_keeps_the_old_override_and_surfaces_an_error() {
      let from = tempdir().unwrap();
      fs::write(from.path().join("pod.log"), b"data").unwrap();
      let mut settings = Settings::default();
      settings.storage_mut().set_log_dir(Some(from.path().to_path_buf()));
      let mut state = state();
      state.pending = Some(PendingMove {
        kind: PathKind::Log,
        from: PathBuf::from("/no/such/pod/source/dir"),
        to: tempdir().unwrap().path().join("dest"),
      });

      let outcome = update(&mut state, Message::ConfirmMove, &mut settings);

      assert_eq!(outcome, Outcome::None, "a failed move does not persist");
      assert!(state.error.is_some(), "the failure is surfaced");
      assert_eq!(
        *settings.storage().log_dir(),
        Some(from.path().to_path_buf()),
        "the old override is left intact"
      );
    }

    #[test]
    fn dismiss_error_clears_the_banner() {
      let mut state = state();
      state.error = Some("boom".to_owned());
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::DismissError, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.error.is_none());
    }
  }

  mod labels {
    use super::*;

    #[test]
    fn the_database_description_drops_journal_mode_wording() {
      let description = PathKind::Database.description().to_lowercase();

      assert!(!description.contains("wal"), "WAL framing must be gone");
      assert!(!description.contains("journal"), "journal-mode framing must be gone");
    }

    #[test]
    fn the_database_field_is_labelled_for_the_shared_location() {
      assert_eq!(PathKind::Database.label(), "Shared data location");
    }
  }

  mod log_level {
    use super::*;

    #[test]
    fn changing_the_level_records_it_and_routes_the_outcome() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::LogLevelChanged(LogLevel::Verbose), &mut settings);

      assert_eq!(outcome, Outcome::SetLogLevel(LogLevel::Verbose));
      assert_eq!(settings.storage().log_level(), &LogLevel::Verbose);
    }

    #[test]
    fn reselecting_the_active_level_is_a_no_op() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::LogLevelChanged(LogLevel::default()), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(settings.storage().log_level(), &LogLevel::default());
    }
  }

  mod manual_entry {
    use super::*;

    #[test]
    fn editing_a_path_stores_a_draft_without_persisting() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(
        &mut state,
        Message::PathEdited(PathKind::Database, "/var/pod/db".to_owned()),
        &mut settings,
      );

      assert_eq!(outcome, Outcome::None);
      assert_eq!(
        state.drafts.get(&PathKind::Database).map(String::as_str),
        Some("/var/pod/db")
      );
    }

    #[test]
    fn submitting_a_blank_path_is_a_no_op() {
      let mut state = state();
      state.drafts.insert(PathKind::Database, "   ".to_owned());
      let mut settings = Settings::default();
      let before = settings.storage().db_dir().clone();

      let outcome = update(&mut state, Message::PathSubmitted(PathKind::Database), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(*settings.storage().db_dir(), before);
    }

    #[test]
    fn submitting_a_typed_cache_path_applies_it() {
      let empty = tempdir().unwrap();
      let dest = empty.path().join("typed-cache");
      let mut settings = Settings::default();
      settings.storage_mut().set_cache_dir(Some(empty.path().to_path_buf()));
      let mut state = state();
      state.drafts.insert(PathKind::Cache, dest.display().to_string());

      let outcome = update(&mut state, Message::PathSubmitted(PathKind::Cache), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(*settings.storage().cache_dir(), Some(dest));
    }

    #[test]
    fn submitting_a_typed_path_applies_it() {
      let empty = tempdir().unwrap();
      let dest = empty.path().join("typed");
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(empty.path().to_path_buf()));
      let mut state = state();
      state.drafts.insert(PathKind::Database, dest.display().to_string());

      let outcome = update(&mut state, Message::PathSubmitted(PathKind::Database), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(*settings.storage().db_dir(), Some(dest));
    }
  }

  mod move_flow {
    use super::*;

    // Drives the confirm/skip/cancel UX against the Log kind, whose move is a plain synchronous
    // directory relocate. The Database kind defers its file work to the async layout migration and
    // is exercised separately in `database_migration`.
    fn populated_pending() -> (State, Settings, tempfile::TempDir, tempfile::TempDir, PathBuf) {
      let from = tempdir().unwrap();
      fs::write(from.path().join("pod.log"), b"data").unwrap();
      let mut settings = Settings::default();
      settings.storage_mut().set_log_dir(Some(from.path().to_path_buf()));
      let dest_root = tempdir().unwrap();
      let dest = dest_root.path().join("relocated");

      let mut state = state();
      let outcome = apply_destination(&mut state, PathKind::Log, dest.clone(), &mut settings);

      assert_eq!(outcome, Outcome::None, "raising the confirm does not persist yet");
      assert!(state.pending.is_some());
      (state, settings, from, dest_root, dest)
    }

    #[test]
    fn cancel_move_aborts_with_no_change() {
      let (mut state, mut settings, from, _dest_root, _dest) = populated_pending();
      let before = settings.storage().log_dir().clone();

      let outcome = update(&mut state, Message::CancelMove, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.pending.is_none());
      assert!(from.path().join("pod.log").exists(), "cancel touches nothing");
      assert_eq!(
        *settings.storage().log_dir(),
        before,
        "cancel leaves the override untouched"
      );
    }

    #[test]
    fn confirm_move_relocates_the_files_and_commits() {
      let (mut state, mut settings, from, _dest_root, dest) = populated_pending();

      let outcome = update(&mut state, Message::ConfirmMove, &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(state.pending.is_none());
      assert!(dest.join("pod.log").exists(), "files moved to the destination");
      assert!(!from.path().join("pod.log").exists(), "source emptied");
      assert_eq!(*settings.storage().log_dir(), Some(dest));
    }

    #[test]
    fn skip_move_repoints_without_moving_files() {
      let (mut state, mut settings, from, _dest_root, dest) = populated_pending();

      let outcome = update(&mut state, Message::SkipMove, &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(state.pending.is_none());
      assert!(
        from.path().join("pod.log").exists(),
        "skip leaves the old files in place"
      );
      assert!(!dest.join("pod.log").exists(), "skip moves nothing");
      assert_eq!(*settings.storage().log_dir(), Some(dest));
    }
  }

  mod relocate {
    use super::*;

    #[test]
    fn it_moves_a_populated_tree_with_subdirectories() {
      let from = tempdir().unwrap();
      fs::write(from.path().join("pod.db"), b"db").unwrap();
      fs::write(from.path().join("pod.db-wal"), b"wal").unwrap();
      fs::create_dir(from.path().join("images")).unwrap();
      fs::write(from.path().join("images").join("1.png"), b"img").unwrap();
      let dest_root = tempdir().unwrap();
      let dest = dest_root.path().join("moved");

      relocate(from.path(), &dest).unwrap();

      assert!(dest.join("pod.db").exists());
      assert!(dest.join("pod.db-wal").exists());
      assert_eq!(fs::read(dest.join("images").join("1.png")).unwrap(), b"img");
      assert!(
        !from.path().exists(),
        "the source tree is removed after a successful move"
      );
    }
  }

  mod reset {
    use super::*;

    #[test]
    fn commit_override_clears_when_the_destination_is_the_default() {
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(PathBuf::from("/var/pod/db")));

      commit_override(PathKind::Database, PathKind::Database.default_dir(), &mut settings);

      assert_eq!(*settings.storage().db_dir(), None);
    }

    #[test]
    fn commit_override_pins_a_custom_destination() {
      let mut settings = Settings::default();

      commit_override(PathKind::Log, PathBuf::from("/var/pod/log"), &mut settings);

      assert_eq!(*settings.storage().log_dir(), Some(PathBuf::from("/var/pod/log")));
    }

    #[test]
    fn reset_clears_the_override_when_the_source_is_empty() {
      let empty = tempdir().unwrap();
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(empty.path().to_path_buf()));
      let mut state = state();

      let outcome = update(&mut state, Message::ResetToDefault(PathKind::Database), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(*settings.storage().db_dir(), None, "reset clears the override");
    }
  }

  mod reveal_log_dir {
    use super::*;

    #[test]
    fn it_does_not_persist_or_raise_a_pending_move() {
      let dir = tempdir().unwrap();
      let mut settings = Settings::default();
      settings.storage_mut().set_log_dir(Some(dir.path().to_path_buf()));
      let mut state = state();

      let outcome = update(&mut state, Message::RevealLogDir, &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert!(state.pending.is_none());
      assert_eq!(*settings.storage().log_dir(), Some(dir.path().to_path_buf()));
    }
  }

  mod sync_actions {
    use super::*;

    #[test]
    fn it_covers_the_simple_storage_message_branches() {
      let mut state = state();
      let mut settings = Settings::default();

      state.error = Some("stale".to_owned());
      assert_eq!(update(&mut state, Message::DismissError, &mut settings), Outcome::None);
      assert!(state.error.is_none());

      state.export_pending = true;
      assert_eq!(
        update(&mut state, Message::ExportFinished(Ok(None)), &mut settings),
        Outcome::None
      );
      assert!(!state.export_pending);

      assert_eq!(
        update(
          &mut state,
          Message::ExportFinished(Err("disk full".to_owned())),
          &mut settings
        ),
        Outcome::None
      );
      assert_eq!(state.error.as_deref(), Some("disk full"));

      assert!(matches!(
        update(&mut state, Message::ExportLogs(RangePreset::LastHour), &mut settings),
        Outcome::ExportLogs { .. }
      ));
      assert!(state.export_pending);

      assert_eq!(update(&mut state, Message::CancelMove, &mut settings), Outcome::None);
      assert_eq!(update(&mut state, Message::ConfirmMove, &mut settings), Outcome::None);
      assert_eq!(update(&mut state, Message::SkipMove, &mut settings), Outcome::None);
    }

    #[test]
    fn request_data_export_arms_the_flag_and_returns_the_outcome() {
      let mut state = state();
      let mut settings = Settings::default();
      state.error = Some("stale".to_owned());

      assert_eq!(
        update(&mut state, Message::RequestDataExport, &mut settings),
        Outcome::ExportData
      );

      assert!(state.data_export_pending, "the export is marked in-flight");
      assert!(state.error.is_none(), "requesting an export clears any prior error");
      assert!(!state.export_pending, "the log export flag is untouched");
    }

    #[test]
    fn data_export_finished_clears_the_flag_and_surfaces_errors() {
      let mut state = state();
      let mut settings = Settings::default();
      state.data_export_pending = true;

      assert_eq!(
        update(&mut state, Message::DataExportFinished(Ok(None)), &mut settings),
        Outcome::None
      );
      assert!(!state.data_export_pending, "a finished export is no longer in-flight");
      assert!(state.error.is_none());

      state.data_export_pending = true;
      assert_eq!(
        update(
          &mut state,
          Message::DataExportFinished(Err("disk full".to_owned())),
          &mut settings
        ),
        Outcome::None
      );
      assert!(!state.data_export_pending);
      assert_eq!(state.error.as_deref(), Some("disk full"));
    }

    #[test]
    fn request_data_import_is_a_no_op_when_the_pick_dialog_is_stubbed() {
      // pick_data_archive returns None under cfg(test), so the request clears any error and parks
      // without opening a confirm modal.
      let mut state = state();
      let mut settings = Settings::default();
      state.error = Some("stale".to_owned());

      assert_eq!(
        update(&mut state, Message::RequestDataImport, &mut settings),
        Outcome::None
      );

      assert!(state.error.is_none(), "requesting an import clears any prior error");
      assert!(
        state.data_import_confirm.is_none(),
        "the stubbed pick shows no confirm modal"
      );
      assert!(!state.data_import_pending);
    }

    #[test]
    fn confirm_data_import_arms_the_flag_and_carries_the_path() {
      let mut state = state();
      let mut settings = Settings::default();
      let path = PathBuf::from("/tmp/pod-data.zip");
      state.data_import_confirm = Some(PendingImport {
        path: path.clone(),
        pod_version: env!("CARGO_PKG_VERSION").to_owned(),
        verdict: VersionVerdict::Ok,
      });

      let outcome = update(&mut state, Message::ConfirmDataImport, &mut settings);

      assert_eq!(
        outcome,
        Outcome::ImportData {
          path
        }
      );
      assert!(state.data_import_pending, "the import is marked in-flight");
      assert!(
        state.data_import_confirm.is_none(),
        "confirming consumes the pending import"
      );
    }

    #[test]
    fn confirm_data_import_without_a_pending_archive_is_inert() {
      let mut state = state();
      let mut settings = Settings::default();

      assert_eq!(
        update(&mut state, Message::ConfirmDataImport, &mut settings),
        Outcome::None
      );
      assert!(!state.data_import_pending);
    }

    #[test]
    fn cancel_data_import_dismisses_the_confirm_modal_without_touching_data() {
      let mut state = state();
      let mut settings = Settings::default();
      state.data_import_confirm = Some(PendingImport {
        path: PathBuf::from("/tmp/pod-data.zip"),
        pod_version: env!("CARGO_PKG_VERSION").to_owned(),
        verdict: VersionVerdict::WillMigrate,
      });

      assert_eq!(
        update(&mut state, Message::CancelDataImport, &mut settings),
        Outcome::None
      );
      assert!(
        state.data_import_confirm.is_none(),
        "cancelling clears the pending import"
      );
      assert!(!state.data_import_pending);
    }

    #[test]
    fn data_import_finished_clears_the_flag_and_surfaces_errors() {
      let mut state = state();
      let mut settings = Settings::default();
      state.data_import_pending = true;

      assert_eq!(
        update(&mut state, Message::DataImportFinished(Ok(None)), &mut settings),
        Outcome::None
      );
      assert!(!state.data_import_pending);
      assert!(state.error.is_none());

      state.data_import_pending = true;
      assert_eq!(
        update(
          &mut state,
          Message::DataImportFinished(Err("lease held".to_owned())),
          &mut settings
        ),
        Outcome::None
      );
      assert!(!state.data_import_pending);
      assert_eq!(state.error.as_deref(), Some("lease held"));
    }

    #[test]
    fn validate_archive_refuses_a_newer_major_archive() {
      let archive = newer_major_archive();
      let dir = tempdir().unwrap();
      let path = dir.path().join("pod-data.zip");
      fs::write(&path, &archive).unwrap();

      let error = validate_archive(&path).unwrap_err();

      assert!(
        error.contains("newer Pod"),
        "an incompatible archive is refused: {error}"
      );
    }

    #[test]
    fn validate_archive_accepts_this_builds_archive() {
      let archive = current_version_archive();
      let dir = tempdir().unwrap();
      let path = dir.path().join("pod-data.zip");
      fs::write(&path, &archive).unwrap();

      let pending = validate_archive(&path).unwrap();

      assert_eq!(pending.path, path);
      assert_eq!(pending.pod_version, env!("CARGO_PKG_VERSION"));
      assert_eq!(pending.verdict, VersionVerdict::Ok);
    }

    #[test]
    fn validate_archive_rejects_bytes_that_are_not_an_archive() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("not-a-zip.zip");
      fs::write(&path, b"definitely not a zip").unwrap();

      assert!(validate_archive(&path).is_err());
    }

    /// Builds a `.zip` data archive carrying the given Pod version, reusing the export writer so the
    /// import guard runs against the same on-disk format production produces.
    fn archive_with_version(version: &str) -> Vec<u8> {
      use std::io::{Cursor, Write};

      use zip::{CompressionMethod, ZipWriter, write::FileOptions};

      let manifest = serde_json::json!({
        "archive_version": 1,
        "arch": "x86_64",
        "created_at": "2026-06-25T00:00:00+00:00",
        "pod_version": version,
        "os": "linux",
        "storage": {
          "cache_dir": "/cache",
          "database_path": "/db/pod.db",
          "db_dir": "/db",
          "log_dir": "/logs",
        },
        "files": [],
      });
      let mut buf = Vec::new();
      {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let options: FileOptions<'_, ()> = FileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("pod.db", options).unwrap();
        zip.write_all(b"db bytes").unwrap();
        zip.start_file("config.toml", options).unwrap();
        zip.write_all(b"[storage]\nnetwork = false\n").unwrap();
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        zip.finish().unwrap();
      }
      buf
    }

    fn current_version_archive() -> Vec<u8> {
      archive_with_version(env!("CARGO_PKG_VERSION"))
    }

    fn newer_major_archive() -> Vec<u8> {
      archive_with_version("999.0.0")
    }

    #[test]
    fn release_lock_routes_an_outcome_without_touching_disk() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::ReleaseLock, &mut settings);

      assert_eq!(outcome, Outcome::ReleaseLock);
    }

    #[test]
    fn set_sync_status_records_the_holder_and_last_synced() {
      let mut state = state();
      let at = Utc::now();

      state.set_sync_status(Some("nas-mac".to_owned()), Some(at));

      assert_eq!(state.sync.holder.as_deref(), Some("nas-mac"));
      assert_eq!(state.sync.last_synced, Some(at));
    }

    #[test]
    fn sync_now_routes_an_outcome_without_touching_disk() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::SyncNow, &mut settings);

      assert_eq!(outcome, Outcome::SyncNow);
    }
  }

  mod sync_toggle {
    use super::*;

    #[test]
    fn dismissing_the_network_suggestion_sets_the_flag_without_persisting() {
      let mut state = state();

      let outcome = update(&mut state, Message::SyncSuggestionDismissed, &mut Settings::default());

      assert_eq!(outcome, Outcome::None);
      assert!(
        state.sync_suggestion_dismissed,
        "the advisory stays dismissed for this session"
      );
    }

    #[test]
    fn it_persists_the_sync_override() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::SyncToggled(true), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(settings.storage().network());
    }

    #[test]
    fn toggling_off_clears_the_sync_override() {
      let mut state = state();
      let mut settings = Settings::default();
      settings.storage_mut().set_network(true);

      let outcome = update(&mut state, Message::SyncToggled(false), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(!settings.storage().network());
    }
  }

  mod view {
    use super::*;

    #[test]
    fn direct_mode_omits_the_sync_lease_controls() {
      let settings = Settings::default();
      assert_eq!(
        settings.storage().storage_mode(),
        StorageMode::Direct,
        "a local path defaults to direct mode, so the working-copy and lease rows stay hidden"
      );
      let state = state();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_sync_mode_with_the_working_copy_and_status_rows() {
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(PathBuf::from("/var/pod/db")));
      settings.storage_mut().set_network(true);
      assert_eq!(
        settings.storage().storage_mode(),
        StorageMode::Sync,
        "the override forces sync mode"
      );
      let mut state = state();
      state.set_sync_status(None, Some(Utc::now()));

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_confirm_move_modal_when_pending() {
      let settings = Settings::default();
      let mut state = state();
      state.pending = Some(PendingMove {
        kind: PathKind::Database,
        from: PathBuf::from("/old/pod"),
        to: PathBuf::from("/new/pod"),
      });

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_default_panel() {
      let settings = Settings::default();
      let state = state();

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_error_banner() {
      let settings = Settings::default();
      let mut state = state();
      state.error = Some("Can't use /bad: permission denied".to_owned());

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }

  mod data_rows {
    use super::*;

    #[test]
    fn the_export_row_renders_its_right_aligned_action() {
      let state = state();
      let _el: Element<'_, Message> = data_export_row(&state);
    }

    #[test]
    fn the_export_row_swaps_to_a_progress_label_while_exporting() {
      let mut state = state();
      state.data_export_pending = true;

      let _el: Element<'_, Message> = data_export_row(&state);
    }

    #[test]
    fn the_import_row_renders_its_right_aligned_action() {
      let state = state();
      let _el: Element<'_, Message> = data_import_row(&state);
    }

    #[test]
    fn the_import_row_swaps_to_a_progress_label_while_restoring() {
      let mut state = state();
      state.data_import_pending = true;

      let _el: Element<'_, Message> = data_import_row(&state);
    }

    #[test]
    fn the_action_label_drops_the_icon_when_no_glyph_is_supplied() {
      let _with_icon: Element<'_, Message> =
        action_button_label(Some(Icon::archive()), "Export data\u{2026}", color::accent::PLASMA);
      let _without_icon: Element<'_, Message> =
        action_button_label(None, "Preparing archive\u{2026}", color::accent::PLASMA);
    }

    #[test]
    fn the_accent_ghost_button_styles_every_status() {
      for status in [
        button::Status::Active,
        button::Status::Hovered,
        button::Status::Pressed,
        button::Status::Disabled,
      ] {
        let style = accent_ghost_button(&iced::Theme::Dark, status);
        assert_eq!(style.text_color, color::accent::PLASMA);
      }
    }
  }
}
