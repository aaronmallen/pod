use std::collections::BTreeSet;

use iced::{
  Background, Border, Color, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, text, text_input},
};

use super::{
  budget::{self, Category},
  budget_view::{
    character_name, color_dot, count_pill, editor_input_style, eyebrow_label, mono_caption, rule_delete_button,
    rule_display_name, switch,
  },
  rule_pack,
};
use crate::{
  features::wallet::budget_engine as engine,
  store::{
    Database,
    model::{MatchMode, Rule, RuleField, RuleOp},
  },
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown, backdrop, button::Button, icon::Icon, modal_overlay::stable_overlay,
    },
    style::{color, spacing, typography},
  },
};

pub const BUDGET_RULES_WINDOW_HEIGHT: f32 = 680.0;
pub const BUDGET_RULES_WINDOW_WIDTH: f32 = 760.0;

const EDITOR_PANEL_WIDTH: f32 = 860.0;
const PREVIEW_WIDTH: f32 = 332.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorSeed {
  Existing(i64),
  New(i64),
}

#[derive(Debug, Default)]
struct ExportDraft {
  name: String,
  note: String,
  selected: BTreeSet<i64>,
}

#[derive(Debug)]
struct ImportDraft {
  pack: rule_pack::PackEnvelope,
  plan: rule_pack::ImportPlan,
  skipped: BTreeSet<usize>,
}

#[derive(Clone, Debug)]
pub enum Message {
  Closed,
  ConditionAdded,
  ConditionFieldChanged(usize, RuleField),
  ConditionOpChanged(usize, RuleOp),
  ConditionRemoved(usize),
  ConditionValue2Changed(usize, String),
  ConditionValueChanged(usize, String),
  DragStarted(i64),
  DropReleased,
  DropTargetEntered(i64),
  DropTargetLeft,
  EditorAdvancedToggled,
  EditorClosed,
  EditorCommitted,
  EditorMatchModeSelected(MatchMode),
  EditorNameChanged(String),
  EditorSearchChanged(String),
  EditorSelectToggled(Option<budget::RuleSelectKey>),
  EscapePressed,
  ExportAllSelected,
  ExportClosed,
  ExportConfirmed,
  ExportFinished,
  ExportNameChanged(String),
  ExportNoneSelected,
  ExportNoteChanged(String),
  ExportOpened(Option<i64>),
  ExportRuleToggled(i64),
  ImportClosed,
  ImportConfirmed,
  ImportErrorDismissed,
  ImportFileLoaded(Option<String>),
  ImportOpened,
  ImportSkipToggled(usize),
  RuleDeleted(i64),
  RuleEditOpened(i64),
  RuleToggled(i64, bool),
}

#[derive(Debug, Default)]
pub struct State {
  dragging: Option<i64>,
  drop_target: Option<i64>,
  editor: Option<budget::RuleDraft>,
  export: Option<ExportDraft>,
  import: Option<ImportDraft>,
  import_error: Option<rule_pack::ParseError>,
}

impl State {
  pub fn clear_editor_for_rule(&mut self, rule_id: i64) {
    if self.editor.as_ref().is_some_and(|draft| draft.rule_id == Some(rule_id)) {
      self.editor = None;
    }
  }

  pub fn open_editor(&mut self, wallet: &super::State, seed: EditorSeed) {
    self.editor = match seed {
      EditorSeed::Existing(rule_id) => wallet
        .budget_rules()
        .iter()
        .find(|rule| rule.id() == rule_id)
        .map(budget::RuleDraft::from_rule),
      EditorSeed::New(category_id) => Some(budget::RuleDraft::new(category_id)),
    };
  }
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      super::is_left_released(&event).then_some(Message::DropReleased)
    }));
  }
  if state.editor.is_some() || state.export.is_some() || state.import.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      super::is_escape_pressed(&event).then_some(Message::EscapePressed)
    }));
  }
  iced::Subscription::batch(subs)
}

pub fn update(state: &mut State, wallet: &mut super::State, db: &Database, message: Message) -> Task<super::Message> {
  match message {
    Message::Closed => Task::none(),
    Message::DragStarted(rule_id) => {
      state.dragging = Some(rule_id);
      state.drop_target = None;
      Task::none()
    }
    Message::DropReleased => drop_released(state, wallet, db),
    Message::DropTargetEntered(rule_id) => {
      if state.dragging.is_some() {
        state.drop_target = Some(rule_id);
      }
      Task::none()
    }
    Message::DropTargetLeft => {
      // While a drag is active, leaving a row does not clear the highlight: it stays on the last-entered
      // row until DropTargetEntered overwrites it or the drag ends, avoiding a flicker to "no target" as
      // the pointer crosses row boundaries.
      if state.dragging.is_none() {
        state.drop_target = None;
      }
      Task::none()
    }
    Message::RuleDeleted(rule_id) => {
      state.clear_editor_for_rule(rule_id);
      super::budget_delete_rule(wallet, db, rule_id)
    }
    Message::RuleEditOpened(rule_id) => {
      state.open_editor(wallet, EditorSeed::Existing(rule_id));
      Task::none()
    }
    Message::RuleToggled(rule_id, enabled) => super::budget_toggle_rule(wallet, db, rule_id, enabled),
    other => update_pack(state, wallet, db, other),
  }
}

pub fn view<'a>(wallet: &'a super::State, state: &'a State) -> Element<'a, Message> {
  let base = manager_body(wallet, state);
  let mut layers: Vec<Element<'a, Message>> = Vec::new();
  if let Some(draft) = state.editor.as_ref() {
    layers.push(backdrop::backdrop(Message::EditorClosed));
    layers.push(editor_modal(wallet, draft));
  }
  if let Some(draft) = state.export.as_ref() {
    layers.push(backdrop::backdrop(Message::ExportClosed));
    layers.push(export_modal(wallet, draft));
  }
  if let Some(draft) = state.import.as_ref() {
    layers.push(backdrop::backdrop(Message::ImportClosed));
    layers.push(import_modal(wallet, draft));
  }
  stable_overlay(base, layers)
}

fn update_pack(state: &mut State, wallet: &mut super::State, db: &Database, message: Message) -> Task<super::Message> {
  match message {
    Message::EscapePressed => {
      // Overlays can be stacked (editor -> export -> import); close topmost-first, mirroring the push
      // order in `view()`, so Escape peels one layer at a time instead of dropping straight to the base.
      if state.import.is_some() {
        state.import = None;
      } else if state.export.is_some() {
        state.export = None;
      } else {
        state.editor = None;
      }
      Task::none()
    }
    Message::ExportAllSelected => {
      if let Some(draft) = state.export.as_mut() {
        draft.selected = wallet.budget_rules().iter().map(Rule::id).collect();
      }
      Task::none()
    }
    Message::ExportClosed => {
      state.export = None;
      Task::none()
    }
    Message::ExportConfirmed => export_confirmed(state, wallet),
    Message::ExportFinished => Task::none(),
    Message::ExportNameChanged(name) => {
      if let Some(draft) = state.export.as_mut() {
        draft.name = name;
      }
      Task::none()
    }
    Message::ExportNoneSelected => {
      if let Some(draft) = state.export.as_mut() {
        draft.selected.clear();
      }
      Task::none()
    }
    Message::ExportNoteChanged(note) => {
      if let Some(draft) = state.export.as_mut() {
        draft.note = note;
      }
      Task::none()
    }
    Message::ExportOpened(rule_id) => {
      state.export = Some(ExportDraft {
        name: String::new(),
        note: String::new(),
        selected: match rule_id {
          Some(id) => BTreeSet::from([id]),
          None => wallet.budget_rules().iter().map(Rule::id).collect(),
        },
      });
      Task::none()
    }
    Message::ExportRuleToggled(rule_id) => {
      if let Some(draft) = state.export.as_mut()
        && !draft.selected.remove(&rule_id)
      {
        draft.selected.insert(rule_id);
      }
      Task::none()
    }
    other => update_import(state, wallet, db, other),
  }
}

fn update_import(
  state: &mut State,
  wallet: &mut super::State,
  db: &Database,
  message: Message,
) -> Task<super::Message> {
  match message {
    Message::ImportClosed => {
      state.import = None;
      Task::none()
    }
    Message::ImportConfirmed => import_confirmed(state, wallet, db),
    Message::ImportErrorDismissed => {
      state.import_error = None;
      Task::none()
    }
    Message::ImportFileLoaded(content) => {
      import_file_loaded(state, wallet, content);
      Task::none()
    }
    Message::ImportOpened => {
      state.import_error = None;
      Task::perform(pick_pack_file(), |content| {
        super::Message::BudgetRulesWindow(Message::ImportFileLoaded(content))
      })
    }
    Message::ImportSkipToggled(index) => {
      if let Some(draft) = state.import.as_mut()
        && !draft.skipped.remove(&index)
      {
        draft.skipped.insert(index);
      }
      Task::none()
    }
    other => update_editor(state, wallet, db, other),
  }
}

fn budget_groups(wallet: &super::State) -> &[budget::Group] {
  wallet.budget().map_or(&[], |view| view.groups.as_slice())
}

fn export_confirmed(state: &mut State, wallet: &super::State) -> Task<super::Message> {
  let Some(draft) = state.export.take() else {
    return Task::none();
  };
  let groups = budget_groups(wallet);
  let portables: Vec<rule_pack::PortableRule> = wallet
    .budget_rules()
    .iter()
    .filter(|rule| draft.selected.contains(&rule.id()))
    .map(|rule| rule_pack::portable_rule(rule, rule_display_name(wallet, rule), groups))
    .collect();
  if portables.is_empty() {
    return Task::none();
  }
  let pack = rule_pack::build_pack(portables, &draft.name, &draft.note);
  let Ok(contents) = rule_pack::encode_pack(&pack) else {
    return Task::none();
  };
  let file_name = rule_pack::pack_file_name(&pack.name);
  Task::perform(save_pack_file(file_name, contents), |_| {
    super::Message::BudgetRulesWindow(Message::ExportFinished)
  })
}

fn import_file_loaded(state: &mut State, wallet: &super::State, content: Option<String>) {
  let Some(content) = content else {
    return;
  };
  match rule_pack::parse_pack(&content) {
    Ok(pack) => {
      let plan = rule_pack::plan_import(&pack, wallet.budget_rules(), budget_groups(wallet));
      // Rules that already exist are pre-skipped; the user must explicitly toggle one back on to
      // re-import a duplicate.
      let skipped = plan
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.is_duplicate)
        .map(|(index, _)| index)
        .collect();
      state.import = Some(ImportDraft {
        pack,
        plan,
        skipped,
      });
      state.import_error = None;
    }
    Err(error) => {
      state.import = None;
      state.import_error = Some(error);
    }
  }
}

fn import_confirmed(state: &mut State, wallet: &super::State, db: &Database) -> Task<super::Message> {
  let Some(draft) = state.import.take() else {
    return Task::none();
  };
  if draft.skipped.len() >= draft.plan.items.len() {
    return Task::none();
  }
  let group_position_base = budget_groups(wallet).len() as i64;
  let rule_position_base = wallet.budget_rules().len() as i64;
  let plan = draft.plan;
  let skipped = draft.skipped;
  super::budget_persist_then_reload(wallet, db, move |db, _month| {
    Box::pin(async move {
      let _ = rule_pack::commit_import(&db, &plan, &skipped, group_position_base, rule_position_base).await;
    })
  })
}

async fn pick_pack_file() -> Option<String> {
  #[cfg(not(test))]
  {
    let filter = t!("wallet.budget.pack_file_filter");
    let handle = rfd::AsyncFileDialog::new()
      .set_title(t!("wallet.budget.pack_import_dialog_title").into_owned())
      .add_filter(&*filter, &[rule_pack::PACK_EXTENSION])
      .pick_file()
      .await?;
    Some(String::from_utf8_lossy(&handle.read().await).into_owned())
  }
  #[cfg(test)]
  {
    None
  }
}

async fn save_pack_file(default_name: String, contents: String) -> Option<std::path::PathBuf> {
  #[cfg(not(test))]
  {
    let filter = t!("wallet.budget.pack_file_filter");
    let handle = rfd::AsyncFileDialog::new()
      .set_title(t!("wallet.budget.pack_export_dialog_title").into_owned())
      .set_file_name(default_name)
      .add_filter(&*filter, &[rule_pack::PACK_EXTENSION])
      .save_file()
      .await?;
    let path = handle.path().to_path_buf();
    std::fs::write(&path, contents).ok()?;
    Some(path)
  }
  #[cfg(test)]
  {
    let _ = (default_name, contents);
    None
  }
}

fn update_editor(
  state: &mut State,
  wallet: &mut super::State,
  db: &Database,
  message: Message,
) -> Task<super::Message> {
  match message {
    Message::EditorAdvancedToggled => {
      if let Some(draft) = state.editor.as_mut() {
        draft.show_advanced = !draft.show_advanced;
      }
      Task::none()
    }
    Message::EditorClosed => {
      state.editor = None;
      Task::none()
    }
    Message::EditorCommitted => commit_editor(state, wallet, db),
    Message::EditorMatchModeSelected(mode) => {
      if let Some(draft) = state.editor.as_mut() {
        draft.match_mode = mode;
      }
      Task::none()
    }
    Message::EditorNameChanged(name) => {
      if let Some(draft) = state.editor.as_mut() {
        draft.name = name;
        draft.name_edited = true;
      }
      Task::none()
    }
    Message::EditorSearchChanged(value) => {
      set_search(state, value);
      Task::none()
    }
    Message::EditorSelectToggled(key) => {
      if let Some(draft) = state.editor.as_mut() {
        draft.open_select = if draft.open_select == key { None } else { key };
      }
      Task::none()
    }
    other => update_condition(state, other),
  }
}

fn update_condition(state: &mut State, message: Message) -> Task<super::Message> {
  match message {
    Message::ConditionAdded => {
      if let Some(draft) = state.editor.as_mut() {
        draft.conditions.push(engine::new_condition(RuleField::Party));
      }
    }
    Message::ConditionFieldChanged(index, field) => {
      mutate_condition(state, index, |condition| *condition = engine::new_condition(field));
      close_select(state);
    }
    Message::ConditionOpChanged(index, op) => {
      mutate_condition(state, index, |condition| {
        condition.op = op;
        if op == RuleOp::Between && condition.value2.is_none() {
          condition.value2 = Some(String::new());
        }
      });
      close_select(state);
    }
    Message::ConditionRemoved(index) => {
      if let Some(draft) = state.editor.as_mut()
        && draft.conditions.len() > 1
        && index < draft.conditions.len()
      {
        draft.conditions.remove(index);
      }
    }
    Message::ConditionValue2Changed(index, value) => {
      mutate_condition(state, index, |condition| condition.value2 = Some(value));
    }
    Message::ConditionValueChanged(index, value) => {
      mutate_condition(state, index, |condition| condition.value = value);
      close_select(state);
    }
    _ => {}
  }
  Task::none()
}

fn mutate_condition(state: &mut State, index: usize, mutate: impl FnOnce(&mut crate::store::model::RuleCondition)) {
  if let Some(condition) = state.editor.as_mut().and_then(|draft| draft.conditions.get_mut(index)) {
    mutate(condition);
  }
}

fn close_select(state: &mut State) {
  if let Some(draft) = state.editor.as_mut() {
    draft.open_select = None;
  }
}

fn set_search(state: &mut State, value: String) {
  let Some(draft) = state.editor.as_mut() else {
    return;
  };
  match draft.search_index() {
    Some(index) => draft.conditions[index].value = value,
    None => draft.conditions.insert(
      0,
      crate::store::model::RuleCondition {
        field: RuleField::Text,
        op: RuleOp::Contains,
        value,
        value2: None,
      },
    ),
  }
}

fn commit_editor(state: &mut State, wallet: &super::State, db: &Database) -> Task<super::Message> {
  let Some(draft) = state.editor.take() else {
    return Task::none();
  };
  let active: Vec<crate::store::model::RuleCondition> = draft
    .conditions
    .iter()
    .filter(|c| engine::is_active_condition(c))
    .cloned()
    .collect();
  if active.is_empty() {
    return Task::none();
  }
  let name = super::budget_effective_rule_name(wallet, &draft);
  let position = i64::try_from(wallet.budget_rules().len()).unwrap_or(i64::MAX);
  let category_id = draft.category_id;
  let enabled = draft.enabled;
  let match_mode = draft.match_mode;
  let rule_id = draft.rule_id;

  super::budget_persist_then_reload(wallet, db, move |db, _month| {
    Box::pin(async move {
      super::persist_rule_draft(&db, rule_id, category_id, enabled, match_mode, name, position, active).await;
    })
  })
}

fn drop_released(state: &mut State, wallet: &super::State, db: &Database) -> Task<super::Message> {
  let drop = state.dragging.take().zip(state.drop_target.take());
  let Some((dragged, before)) = drop else {
    return Task::none();
  };
  if dragged == before {
    return Task::none();
  }
  let mut ordered: Vec<i64> = wallet
    .budget_rules()
    .iter()
    .map(Rule::id)
    .filter(|id| *id != dragged)
    .collect();
  let position = ordered.iter().position(|id| *id == before).unwrap_or(ordered.len());
  ordered.insert(position, dragged);
  super::budget_persist_then_reload(wallet, db, move |db, _month| {
    Box::pin(async move {
      let _ = crate::store::repo::budget::reorder_rules(&db, &ordered).await;
    })
  })
}

fn manager_body<'a>(wallet: &'a super::State, state: &'a State) -> Element<'a, Message> {
  let rules = wallet.budget_rules();
  let outflows = wallet.budget_match_targets();
  let enabled_count = rules.iter().filter(|rule| rule.enabled()).count();

  let mut sections: Vec<Element<'a, Message>> = vec![manager_header(rules.len(), enabled_count), priority_note()];

  if let Some(error) = state.import_error.as_ref() {
    sections.push(import_error_banner(error));
  }

  if rules.is_empty() {
    sections.push(empty_state());
    sections.push(Space::new().height(Length::Fill).into());
  } else {
    let rows = rules
      .iter()
      .enumerate()
      .map(|(index, rule)| rule_row(wallet, state, rule, index, engine::match_count(rule, &outflows)))
      .collect::<Vec<Element<'a, Message>>>();
    sections.push(
      container(
        scrollable(Column::with_children(rows).width(Length::Fill))
          .style(crate::ui::style::control::scrollbar)
          .width(Length::Fill)
          .height(Length::Fill),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
    );
  }

  container(Column::with_children(sections).width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn manager_header<'a>(rule_count: usize, enabled_count: usize) -> Element<'a, Message> {
  let rules_phrase = if rule_count == 1 {
    t!("wallet.budget.global_rules_singular", count => rule_count).into_owned()
  } else {
    t!("wallet.budget.global_rules_plural", count => rule_count).into_owned()
  };
  let summary = t!(
    "wallet.budget.global_header_summary",
    rules => rules_phrase,
    active => enabled_count
  )
  .into_owned();

  let left = Column::with_children(vec![
    text(t!("wallet.budget.automation_rules"))
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    mono_caption(summary, color::text::secondary(), typography::size::XS_PLUS),
  ])
  .spacing(4.0)
  .width(Length::Fill);

  let header = Row::with_children(vec![
    left.into(),
    header_action_button(
      t!("wallet.budget.pack_import").into_owned(),
      Icon::upload(),
      Some(Message::ImportOpened),
    ),
    header_action_button(
      t!("wallet.budget.pack_export").into_owned(),
      Icon::arrow_out(),
      (rule_count > 0).then_some(Message::ExportOpened(None)),
    ),
    close_button(),
  ])
  .spacing(14.0)
  .align_y(Vertical::Center);

  container(header)
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 18.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn close_button<'a>() -> Element<'a, Message> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(7.0)
  .on_press(Message::Closed)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: 8.0.into(),
      },
      text_color: if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn priority_note<'a>() -> Element<'a, Message> {
  let line = Row::with_children(vec![
    text(t!("wallet.budget.priority_note_prefix"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text(t!("wallet.budget.priority_note_emphasis"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("wallet.budget.priority_note_suffix"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(4.0)
  .align_y(Vertical::Center)
  .wrap();

  container(line)
    .width(Length::Fill)
    .padding(Padding {
      top: 11.0,
      right: 20.0,
      bottom: 11.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      text(t!("wallet.budget.global_rules_empty"))
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .align_x(Horizontal::Center)
        .style(typography::colored(color::text::secondary()))
        .into(),
      header_action_button(
        t!("wallet.budget.pack_import_link").into_owned(),
        Icon::upload(),
        Some(Message::ImportOpened),
      ),
    ])
    .spacing(14.0)
    .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Center)
  .padding(Padding {
    top: 48.0,
    right: 24.0,
    bottom: 48.0,
    left: 24.0,
  })
  .into()
}

fn category_label<'a>(wallet: &'a super::State, rule: &Rule) -> (Option<&'a str>, String) {
  match wallet.budget().and_then(|view| view.category(rule.category_id())) {
    Some(category) => (category.tone.as_deref(), category.name.clone()),
    None => (None, String::new()),
  }
}

fn rule_row<'a>(
  wallet: &'a super::State,
  state: &'a State,
  rule: &'a Rule,
  index: usize,
  count: usize,
) -> Element<'a, Message> {
  let rule_id = rule.id();
  let (tone, category_name) = category_label(wallet, rule);
  let dragging = state.dragging == Some(rule_id);
  let is_drop_target = state.drop_target == Some(rule_id);

  let title = Row::with_children(vec![
    text(rule_display_name(wallet, rule))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    mono_caption(
      t!("wallet.budget.files_into", category => category_name).into_owned(),
      color::text::tertiary(),
      typography::size::XS,
    ),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let summary = engine::summarize_rule(
    rule,
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(wallet, key),
  );

  let detail = Column::with_children(vec![
    title.into(),
    mono_caption(summary, color::text::secondary(), typography::size::XS_PLUS),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    drag_grip(rule_id),
    container(mono_caption(
      format!("{}", index + 1),
      color::text::secondary(),
      typography::size::SM,
    ))
    .width(Length::Fixed(18.0))
    .align_x(Horizontal::Right)
    .into(),
    color_dot(tone, 9.0),
    detail.into(),
    count_pill(count, rule.enabled()),
    switch(rule.enabled(), Message::RuleToggled(rule_id, !rule.enabled())),
    share_button(rule_id),
    edit_button(rule_id),
    rule_delete_button(Message::RuleDeleted(rule_id)),
  ])
  .spacing(12.0)
  .align_y(Vertical::Center);

  let body = container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 13.0,
      right: 20.0,
      bottom: 13.0,
      left: 20.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(if dragging {
        color::with_alpha(color::accent::PLASMA, 0.06)
      } else {
        Color::TRANSPARENT
      })),
      border: Border {
        color: if is_drop_target {
          color::accent::PLASMA
        } else {
          color::rule()
        },
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  mouse_area(body)
    .on_enter(Message::DropTargetEntered(rule_id))
    .on_exit(Message::DropTargetLeft)
    .into()
}

fn drag_grip<'a>(rule_id: i64) -> Element<'a, Message> {
  let handle = text("\u{283f}")
    .font(typography::mono::REGULAR)
    .size(15.0)
    .style(typography::colored(color::text::tertiary()));

  mouse_area(container(handle).align_x(Horizontal::Center))
    .on_press(Message::DragStarted(rule_id))
    .interaction(iced::mouse::Interaction::Grab)
    .into()
}

fn edit_button<'a>(rule_id: i64) -> Element<'a, Message> {
  row_icon_button(Icon::pencil(), Message::RuleEditOpened(rule_id))
}

fn share_button<'a>(rule_id: i64) -> Element<'a, Message> {
  row_icon_button(Icon::arrow_out(), Message::ExportOpened(Some(rule_id)))
}

fn row_icon_button<'a>(icon: Icon, message: Message) -> Element<'a, Message> {
  button(icon.size(13.0).color(color::text::secondary()).render())
    .width(Length::Fixed(28.0))
    .height(Length::Fixed(28.0))
    .padding(Padding::ZERO)
    .on_press(message)
    .style(|_, status| {
      let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
          color: if active { color::accent::PLASMA } else { color::rule() },
          width: 1.0,
          radius: 6.0.into(),
        },
        text_color: if active {
          color::accent::PLASMA
        } else {
          color::text::secondary()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn editor_modal<'a>(wallet: &'a super::State, draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let category = wallet.budget().and_then(|view| view.category(draft.category_id));

  let panel = container(
    Column::with_children(vec![
      editor_header(draft, category),
      editor_body(wallet, draft, category),
      editor_footer(draft),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .max_width(EDITOR_PANEL_WIDTH)
  .max_height(720.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: 14.0.into(),
    },
    ..container::Style::default()
  });

  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(28.0)
    .into()
}

fn editor_header<'a>(draft: &'a budget::RuleDraft, category: Option<&'a Category>) -> Element<'a, Message> {
  let eyebrow = if draft.rule_id.is_some() {
    super::i18n::tr_static("wallet.budget.edit_rule")
  } else {
    super::i18n::tr_static("wallet.budget.new_rule")
  };

  let mut title: Vec<Element<'a, Message>> = vec![
    text(t!("wallet.budget.file_matches_into"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if let Some(category) = category {
    title.push(color_dot(category.tone.as_deref(), 9.0));
    title.push(
      text(category.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    );
  }

  let left = Column::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text(eyebrow, None).into(),
    Row::with_children(title)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
  ])
  .spacing(5.0)
  .width(Length::Fill);

  let header = Row::with_children(vec![left.into(), editor_close_button()])
    .spacing(14.0)
    .align_y(Vertical::Center);

  container(header)
    .width(Length::Fill)
    .padding(Padding {
      top: 16.0,
      right: 18.0,
      bottom: 16.0,
      left: 18.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn editor_close_button<'a>() -> Element<'a, Message> {
  modal_close_button(Message::EditorClosed)
}

fn modal_close_button<'a>(message: Message) -> Element<'a, Message> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(7.0)
  .on_press(message)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: 8.0.into(),
      },
      text_color: if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn editor_body<'a>(
  wallet: &'a super::State,
  draft: &'a budget::RuleDraft,
  category: Option<&'a Category>,
) -> Element<'a, Message> {
  let builder = container(
    scrollable(rule_builder(wallet, draft))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  });

  let preview = container(rule_preview(wallet, draft, category))
    .width(Length::Fixed(PREVIEW_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  container(
    Row::with_children(vec![builder.into(), preview.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn rule_builder<'a>(wallet: &'a super::State, draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![rule_search_box(draft)];
  if draft.show_advanced {
    children.push(rule_advanced_block(wallet, draft));
  }
  children.push(rule_name_block(wallet, draft));

  Column::with_children(children)
    .spacing(16.0)
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 18.0,
      left: 20.0,
    })
    .into()
}

fn rule_search_box<'a>(draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let input = text_input(
    super::i18n::tr_static("wallet.budget.search_placeholder"),
    draft.search_value(),
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .padding(Padding {
    top: 8.0,
    right: 11.0,
    bottom: 8.0,
    left: 11.0,
  })
  .width(Length::Fill)
  .on_input(Message::EditorSearchChanged)
  .style(editor_input_style);

  let search_row = Row::with_children(vec![
    Icon::search().size(14.0).color(color::text::secondary()).render(),
    input.into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let caption = Row::with_children(vec![
    text(t!("wallet.budget.search_caption"))
      .font(typography::body::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(Length::Fill)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    advanced_toggle(draft.show_advanced),
  ])
  .align_y(Vertical::Center);

  Column::with_children(vec![
    eyebrow_label(super::i18n::tr_static("wallet.budget.match_containing")),
    search_row.into(),
    caption.into(),
  ])
  .spacing(9.0)
  .width(Length::Fill)
  .into()
}

fn advanced_toggle<'a>(advanced: bool) -> Element<'a, Message> {
  let tint = if advanced {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let label = if advanced {
    t!("wallet.budget.hide_advanced")
  } else {
    t!("wallet.budget.add_conditions")
  };
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(tint)),
  )
  .padding(0)
  .on_press(Message::EditorAdvancedToggled)
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    ..button::Style::default()
  })
  .into()
}

fn rule_advanced_block<'a>(wallet: &'a super::State, draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let mode_row = Row::with_children(vec![
    text(t!("wallet.budget.match_label"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
    match_mode_segment(draft.match_mode),
    text(t!("wallet.budget.of_these_conditions"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(10.0)
  .align_y(Vertical::Center);

  let removable = draft.conditions.len() > 1;
  let rows = draft
    .conditions
    .iter()
    .enumerate()
    .map(|(index, condition)| condition_row(wallet, draft, index, condition, removable))
    .collect::<Vec<Element<'a, Message>>>();

  let add = button(
    Row::with_children(vec![
      text("+")
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
      text(t!("wallet.budget.add_condition"))
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(7.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 7.0,
    right: 12.0,
    bottom: 7.0,
    left: 12.0,
  })
  .on_press(Message::ConditionAdded)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active {
          color::accent::PLASMA
        } else {
          color::rule_strong()
        },
        width: 1.0,
        radius: 7.0.into(),
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    }
  });

  let mut children: Vec<Element<'a, Message>> = vec![mode_row.into()];
  children.extend(rows);
  children.push(add.into());

  container(Column::with_children(children).spacing(8.0).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 0.0,
      bottom: 0.0,
      left: 0.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn condition_row<'a>(
  wallet: &'a super::State,
  draft: &'a budget::RuleDraft,
  index: usize,
  condition: &'a crate::store::model::RuleCondition,
  removable: bool,
) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    field_select(draft, index, condition.field()),
    op_select(draft, index, condition.field(), condition.op()),
    condition_value_editor(wallet, draft, index, condition),
    remove_condition_button(index, removable),
  ])
  .spacing(7.0)
  .align_y(Vertical::Center);

  row.into()
}

fn field_select<'a>(draft: &'a budget::RuleDraft, index: usize, active: RuleField) -> Element<'a, Message> {
  let key = budget::RuleSelectKey::Field(index);
  let options = engine::rule_fields()
    .into_iter()
    .map(|field| {
      anchored_option(
        engine::field_label(field),
        field == active,
        Message::ConditionFieldChanged(index, field),
      )
    })
    .collect::<Vec<Element<'a, Message>>>();

  select_dropdown(
    engine::field_label(active),
    options,
    144.0,
    key,
    draft.open_select == Some(key),
  )
}

fn op_select<'a>(draft: &'a budget::RuleDraft, index: usize, field: RuleField, active: RuleOp) -> Element<'a, Message> {
  let key = budget::RuleSelectKey::Op(index);
  let options = engine::ops_for_field(field)
    .iter()
    .map(|op| {
      anchored_option(
        engine::op_label(*op),
        *op == active,
        Message::ConditionOpChanged(index, *op),
      )
    })
    .collect::<Vec<Element<'a, Message>>>();

  select_dropdown(
    engine::op_label(active),
    options,
    150.0,
    key,
    draft.open_select == Some(key),
  )
}

fn condition_value_editor<'a>(
  wallet: &'a super::State,
  draft: &'a budget::RuleDraft,
  index: usize,
  condition: &'a crate::store::model::RuleCondition,
) -> Element<'a, Message> {
  let open = draft.open_select == Some(budget::RuleSelectKey::Value(index));
  match engine::field_kind(condition.field()) {
    engine::FieldKind::Type => value_select(
      index,
      condition.value(),
      super::i18n::tr_static("wallet.budget.select_type"),
      rule_type_options(wallet),
      open,
    ),
    engine::FieldKind::Character => value_select(
      index,
      condition.value(),
      super::i18n::tr_static("wallet.budget.select_character"),
      rule_character_options(wallet),
      open,
    ),
    engine::FieldKind::Direction => value_select(
      index,
      condition.value(),
      "",
      engine::direction_options()
        .into_iter()
        .map(|(id, label)| (id.to_owned(), label.to_owned()))
        .collect(),
      open,
    ),
    engine::FieldKind::Amount if condition.op() == RuleOp::Between => amount_between_editor(index, condition),
    engine::FieldKind::Amount => amount_value_input(index, condition.value()),
    engine::FieldKind::Text => condition_text_input(index, condition.value()),
  }
}

fn condition_text_input<'a>(index: usize, value: &'a str) -> Element<'a, Message> {
  text_input(
    super::i18n::tr_static("wallet.budget.condition_text_placeholder"),
    value,
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .padding(Padding {
    top: 7.0,
    right: 10.0,
    bottom: 7.0,
    left: 10.0,
  })
  .width(Length::Fill)
  .on_input(move |value| Message::ConditionValueChanged(index, value))
  .style(editor_input_style)
  .into()
}

fn amount_value_input<'a>(index: usize, value: &'a str) -> Element<'a, Message> {
  text_input(super::i18n::tr_static("wallet.budget.amount_example_lower"), value)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 7.0,
      right: 10.0,
      bottom: 7.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .on_input(move |value| Message::ConditionValueChanged(index, value))
    .style(editor_input_style)
    .into()
}

fn amount_between_editor<'a>(index: usize, condition: &'a crate::store::model::RuleCondition) -> Element<'a, Message> {
  let lower = text_input(
    super::i18n::tr_static("wallet.budget.amount_example_lower"),
    condition.value(),
  )
  .font(typography::mono::REGULAR)
  .size(typography::size::MD)
  .padding(Padding {
    top: 7.0,
    right: 10.0,
    bottom: 7.0,
    left: 10.0,
  })
  .width(Length::Fill)
  .align_x(Horizontal::Right)
  .on_input(move |value| Message::ConditionValueChanged(index, value))
  .style(editor_input_style);

  let upper = text_input(
    super::i18n::tr_static("wallet.budget.amount_example_upper"),
    condition.value2().as_deref().unwrap_or(""),
  )
  .font(typography::mono::REGULAR)
  .size(typography::size::MD)
  .padding(Padding {
    top: 7.0,
    right: 10.0,
    bottom: 7.0,
    left: 10.0,
  })
  .width(Length::Fill)
  .align_x(Horizontal::Right)
  .on_input(move |value| Message::ConditionValue2Changed(index, value))
  .style(editor_input_style);

  Row::with_children(vec![
    lower.into(),
    text("\u{2013}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    upper.into(),
  ])
  .spacing(6.0)
  .align_y(Vertical::Center)
  .width(Length::Fill)
  .into()
}

fn value_select<'a>(
  index: usize,
  active: &'a str,
  placeholder: &'a str,
  options: Vec<(String, String)>,
  open: bool,
) -> Element<'a, Message> {
  let label = options
    .iter()
    .find(|(id, _)| id == active)
    .map(|(_, label)| label.clone())
    .unwrap_or_else(|| placeholder.to_owned());

  let rows = options
    .into_iter()
    .map(|(id, label)| {
      let selected = id == active;
      anchored_option(&label, selected, Message::ConditionValueChanged(index, id))
    })
    .collect::<Vec<Element<'a, Message>>>();

  select_dropdown(&label, rows, 0.0, budget::RuleSelectKey::Value(index), open)
}

fn remove_condition_button<'a>(index: usize, removable: bool) -> Element<'a, Message> {
  let mut remove = button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(if removable {
        color::text::secondary()
      } else {
        color::text::tertiary()
      })),
  )
  .padding(6.0)
  .style(move |_, status| {
    let active = removable && matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::status::DANGER } else { color::rule() },
        width: 1.0,
        radius: 6.0.into(),
      },
      text_color: if active {
        color::status::DANGER
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  });
  if removable {
    remove = remove.on_press(Message::ConditionRemoved(index));
  }
  remove.into()
}

fn rule_name_block<'a>(wallet: &'a super::State, draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let suggestion = rule_draft_suggestion(wallet, draft);
  let value = if draft.name_edited {
    draft.name.clone()
  } else if draft.name.is_empty() {
    suggestion.clone()
  } else {
    draft.name.clone()
  };
  let placeholder = if suggestion.is_empty() {
    t!("wallet.budget.name_this_rule").into_owned()
  } else {
    suggestion
  };

  let input = text_input(&placeholder, &value)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 7.0,
      right: 10.0,
      bottom: 7.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .on_input(Message::EditorNameChanged)
    .style(editor_input_style);

  container(
    Column::with_children(vec![
      eyebrow_label(super::i18n::tr_static("wallet.budget.rule_name")),
      input.into(),
    ])
    .spacing(8.0)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 16.0,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn rule_preview<'a>(
  wallet: &'a super::State,
  draft: &'a budget::RuleDraft,
  category: Option<&'a Category>,
) -> Element<'a, Message> {
  let outflows = wallet.budget_match_targets();
  let rule = draft_to_rule(draft);
  let live_rules = live_rules(wallet);
  let manual = wallet.budget_manual_index();
  let rows = engine::preview_entries(&rule, &live_rules, &manual, draft.category_id, &outflows);
  let active_conditions = draft.conditions.iter().any(engine::is_active_condition);
  let will_assign = rows
    .iter()
    .filter(|(_, status)| *status == engine::PreviewStatus::Assign)
    .count();

  let count_color = if active_conditions {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };
  let count_row = Row::with_children(vec![
    text(rows.len().to_string())
      .font(typography::body::MEDIUM)
      .size(26.0)
      .style(typography::colored(count_color))
      .into(),
    text(if rows.len() == 1 {
      t!("wallet.budget.preview_match_singular")
    } else {
      t!("wallet.budget.preview_match_plural")
    })
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()))
    .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom);

  let mut head: Vec<Element<'a, Message>> = vec![
    eyebrow_label(super::i18n::tr_static("wallet.budget.live_preview")),
    count_row.into(),
  ];
  if active_conditions {
    let name = category.map(|category| category.name.clone()).unwrap_or_default();
    head.push(mono_caption(
      t!("wallet.budget.will_file_into", count => will_assign, name => name).into_owned(),
      if will_assign > 0 {
        color::status::ONLINE
      } else {
        color::text::tertiary()
      },
      10.0,
    ));
  }

  let header = container(Column::with_children(head).spacing(8.0).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 16.0,
      bottom: 14.0,
      left: 16.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  let list: Element<'a, Message> = if !active_conditions {
    preview_empty(super::i18n::tr_static("wallet.budget.preview_empty_no_conditions"))
  } else if rows.is_empty() {
    preview_empty(super::i18n::tr_static("wallet.budget.preview_empty_no_matches"))
  } else {
    let cards = rows
      .iter()
      .map(|(index, status)| preview_row(&outflows[*index], *status))
      .collect::<Vec<Element<'a, Message>>>();
    scrollable(Column::with_children(cards).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  };

  Column::with_children(vec![
    header.into(),
    container(list).width(Length::Fill).height(Length::Fill).into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn preview_empty<'a>(message: &'a str) -> Element<'a, Message> {
  container(
    text(message.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .align_x(Horizontal::Center)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 40.0,
    right: 22.0,
    bottom: 40.0,
    left: 22.0,
  })
  .into()
}

fn preview_row<'a>(target: &engine::MatchTarget, status: engine::PreviewStatus) -> Element<'a, Message> {
  let (label, tint) = preview_status_chip(status);
  let primary = if target.item.is_empty() {
    target.reference.clone()
  } else {
    target.item.clone()
  };

  let info = Column::with_children(vec![
    text(primary)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(target.type_token.clone())
      .font(typography::mono::REGULAR)
      .size(9.5)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    info.into(),
    text(format!("-{}", crate::ui::format::fmt_isk(target.amount)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::status::DANGER))
      .into(),
    status_chip(label, tint),
  ])
  .spacing(10.0)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 9.0,
      right: 14.0,
      bottom: 9.0,
      left: 14.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn preview_status_chip(status: engine::PreviewStatus) -> (&'static str, Color) {
  match status {
    engine::PreviewStatus::Already => (
      super::i18n::tr_static("wallet.budget.preview_already"),
      color::text::secondary(),
    ),
    engine::PreviewStatus::Assign => (
      super::i18n::tr_static("wallet.budget.preview_will_file"),
      color::status::ONLINE,
    ),
    engine::PreviewStatus::Manual => (
      super::i18n::tr_static("wallet.budget.preview_manual"),
      color::status::WARNING,
    ),
    engine::PreviewStatus::Preempted => (
      super::i18n::tr_static("wallet.budget.preview_preempted"),
      color::accent::PLASMA,
    ),
  }
}

fn status_chip<'a>(label: &'a str, tint: Color) -> Element<'a, Message> {
  container(
    text(label.to_owned())
      .font(typography::mono::REGULAR)
      .size(8.5)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 3.0,
    right: 7.0,
    bottom: 3.0,
    left: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.11))),
    border: Border {
      color: color::with_alpha(tint, 0.32),
      width: 1.0,
      radius: 4.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn editor_footer<'a>(draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let can_save = draft.conditions.iter().any(engine::is_active_condition);
  let save_label = if draft.rule_id.is_some() {
    super::i18n::tr_static("wallet.budget.save_rule")
  } else {
    super::i18n::tr_static("wallet.budget.create_rule")
  };

  let footer = Row::with_children(vec![
    text(t!("wallet.budget.rule_footer"))
      .font(typography::body::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(Length::Fill)
      .style(typography::colored(color::text::secondary()))
      .into(),
    cancel_button(),
    save_button(save_label, can_save),
  ])
  .spacing(12.0)
  .align_y(Vertical::Center);

  container(footer)
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 18.0,
      bottom: 14.0,
      left: 18.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn cancel_button<'a>() -> Element<'a, Message> {
  Button::ghost(t!("wallet.budget.cancel"))
    .on_press(Message::EditorClosed)
    .into()
}

fn save_button<'a>(label: &'a str, enabled: bool) -> Element<'a, Message> {
  Button::primary(label)
    .on_press_maybe(enabled.then_some(Message::EditorCommitted))
    .into()
}

fn select_dropdown<'a>(
  label: &str,
  options: Vec<Element<'a, Message>>,
  fixed_width: f32,
  key: budget::RuleSelectKey,
  open: bool,
) -> Element<'a, Message> {
  let toggle = if open {
    Message::EditorSelectToggled(None)
  } else {
    Message::EditorSelectToggled(Some(key))
  };
  let trigger = select_trigger(label, toggle);

  let popover = open.then(|| {
    container(
      scrollable(Column::with_children(options).spacing(2.0))
        .style(crate::ui::style::control::scrollbar)
        .height(Length::Shrink),
    )
    .padding(spacing::SPACE_2)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: crate::ui::style::radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
  });

  let dropdown = AnchoredDropdown::new(trigger, popover).on_dismiss(Message::EditorSelectToggled(None));

  let element: Element<'a, Message> = dropdown.into();
  if fixed_width > 0.0 {
    container(element).width(Length::Fixed(fixed_width)).into()
  } else {
    container(element).width(Length::Fill).into()
  }
}

fn select_trigger<'a>(label: &str, toggle: Message) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      text(label.to_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .width(Length::Fill)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      Icon::chevron_down()
        .size(typography::size::XS)
        .color(color::text::tertiary())
        .render(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    right: 10.0,
    bottom: 7.0,
    left: 10.0,
  })
  .on_press(toggle)
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 6.0.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn anchored_option<'a>(label: &str, selected: bool, on_press: Message) -> Element<'a, Message> {
  button(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(if selected {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      })),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
  })
  .on_press(on_press)
  .style(move |_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(if selected || active {
        color::with_alpha(color::accent::PLASMA, 0.08)
      } else {
        Color::TRANSPARENT
      })),
      border: Border {
        radius: crate::ui::style::radius::SUBTLE.into(),
        ..Border::default()
      },
      text_color: if selected {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn match_mode_segment<'a>(active: MatchMode) -> Element<'a, Message> {
  let segments = [
    (MatchMode::All, super::i18n::tr_static("wallet.budget.match_mode_all")),
    (MatchMode::Any, super::i18n::tr_static("wallet.budget.match_mode_any")),
  ];
  let buttons = segments
    .into_iter()
    .map(|(mode, label)| {
      let is_active = mode == active;
      button(
        text(label)
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(typography::colored(if is_active {
            color::accent::PLASMA
          } else {
            color::text::secondary()
          })),
      )
      .padding(Padding {
        top: 6.0,
        right: 13.0,
        bottom: 6.0,
        left: 13.0,
      })
      .on_press(Message::EditorMatchModeSelected(mode))
      .style(move |_, _| button::Style {
        background: Some(Background::Color(if is_active {
          color::with_alpha(color::accent::PLASMA, 0.14)
        } else {
          Color::TRANSPARENT
        })),
        text_color: if is_active {
          color::accent::PLASMA
        } else {
          color::text::secondary()
        },
        ..button::Style::default()
      })
      .into()
    })
    .collect::<Vec<Element<'a, Message>>>();

  container(Row::with_children(buttons))
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 7.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn rule_type_options(wallet: &super::State) -> Vec<(String, String)> {
  let mut seen = std::collections::BTreeMap::new();
  for target in wallet.budget_match_targets() {
    seen
      .entry(target.type_token.clone())
      .or_insert_with(|| engine::humanize_ref_type(&target.type_token));
  }
  let mut options: Vec<(String, String)> = seen.into_iter().collect();
  options.sort_by(|a, b| a.1.cmp(&b.1));
  options
}

fn rule_character_options(wallet: &super::State) -> Vec<(String, String)> {
  wallet
    .roster()
    .iter()
    .map(|pilot| (pilot.id.to_string(), pilot.name.clone()))
    .collect()
}

fn rule_draft_suggestion(wallet: &super::State, draft: &budget::RuleDraft) -> String {
  engine::suggest_name(
    &draft_to_rule(draft),
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(wallet, key),
  )
}

fn draft_to_rule(draft: &budget::RuleDraft) -> Rule {
  Rule {
    category_id: draft.category_id,
    conditions: draft.conditions.clone(),
    enabled: draft.enabled,
    id: draft.rule_id.unwrap_or(0),
    match_mode: draft.match_mode,
    name: draft.name.clone(),
  }
}

fn live_rules(wallet: &super::State) -> Vec<Rule> {
  wallet.budget_rules().to_vec()
}

fn header_action_button<'a>(label: String, icon: Icon, message: Option<Message>) -> Element<'a, Message> {
  let enabled = message.is_some();
  let tint = if enabled {
    color::text::secondary()
  } else {
    color::text::tertiary()
  };
  let content = Row::with_children(vec![
    icon.size(13.0).color(tint).render(),
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(tint))
      .into(),
  ])
  .spacing(6.0)
  .align_y(Vertical::Center);

  let mut control = button(content)
    .padding(Padding {
      top: 6.0,
      right: 11.0,
      bottom: 6.0,
      left: 11.0,
    })
    .style(move |_, status| {
      let active = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
          color: if active { color::rule_strong() } else { color::rule() },
          width: 1.0,
          radius: 7.0.into(),
        },
        text_color: if active { color::text::PRIMARY } else { tint },
        ..button::Style::default()
      }
    });
  if let Some(message) = message {
    control = control.on_press(message);
  }
  control.into()
}

fn import_error_banner<'a>(error: &rule_pack::ParseError) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    text(import_error_text(error))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .width(Length::Fill)
      .style(typography::colored(color::status::DANGER))
      .into(),
    button(
      text("\u{2715}")
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::status::DANGER)),
    )
    .padding(2.0)
    .on_press(Message::ImportErrorDismissed)
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into(),
  ])
  .spacing(9.0)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 10.0,
      right: 20.0,
      bottom: 10.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::DANGER, 0.09))),
      border: Border {
        color: color::with_alpha(color::status::DANGER, 0.32),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn import_error_text(error: &rule_pack::ParseError) -> String {
  match error {
    rule_pack::ParseError::Empty => t!("wallet.budget.pack_import_error_empty"),
    rule_pack::ParseError::NoUsableConditions => t!("wallet.budget.pack_import_error_no_conditions"),
    rule_pack::ParseError::NotAPack => t!("wallet.budget.pack_import_error_not_a_pack"),
    rule_pack::ParseError::UnsupportedVersion => t!("wallet.budget.pack_import_error_version"),
    rule_pack::ParseError::WrongFormat => t!("wallet.budget.pack_import_error_wrong_format"),
  }
  .into_owned()
}

fn pack_panel<'a>(content: Element<'a, Message>, max_width: f32) -> Element<'a, Message> {
  let panel = container(content)
    .width(Length::Fill)
    .max_width(max_width)
    .max_height(680.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: 14.0.into(),
      },
      ..container::Style::default()
    });

  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(28.0)
    .into()
}

fn pack_modal_header<'a>(
  icon: Icon,
  title: String,
  subtitle: Element<'a, Message>,
  close: Message,
) -> Element<'a, Message> {
  let badge = container(icon.size(17.0).color(color::accent::PLASMA).render())
    .width(Length::Fixed(34.0))
    .height(Length::Fixed(34.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, 0.3),
        width: 1.0,
        radius: 8.0.into(),
      },
      ..container::Style::default()
    });

  let detail = Column::with_children(vec![
    text(title)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    subtitle,
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let header = Row::with_children(vec![badge.into(), detail.into(), modal_close_button(close)])
    .spacing(13.0)
    .align_y(Vertical::Center);

  container(header)
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 18.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn pack_modal_footer<'a>(
  note: String,
  cancel: Message,
  confirm_label: String,
  confirm: Option<Message>,
) -> Element<'a, Message> {
  let footer = Row::with_children(vec![
    text(note)
      .font(typography::body::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(Length::Fill)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    Button::secondary(t!("wallet.budget.cancel")).on_press(cancel).into(),
    Button::primary(confirm_label).on_press_maybe(confirm).into(),
  ])
  .spacing(12.0)
  .align_y(Vertical::Center);

  container(footer)
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 20.0,
      bottom: 14.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn checkbox_visual<'a>(on: bool) -> Element<'a, Message> {
  let inner: Element<'a, Message> = if on {
    Icon::check().size(12.0).color(color::surface::BASE).render()
  } else {
    Space::new().into()
  };
  container(inner)
    .width(Length::Fixed(18.0))
    .height(Length::Fixed(18.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(if on {
        color::accent::PLASMA
      } else {
        Color::TRANSPARENT
      })),
      border: Border {
        color: if on {
          color::accent::PLASMA
        } else {
          color::rule_strong()
        },
        width: 1.0,
        radius: 5.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn mini_link<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::accent::PLASMA)),
  )
  .padding(Padding {
    top: 2.0,
    right: 4.0,
    bottom: 2.0,
    left: 4.0,
  })
  .on_press(message)
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    ..button::Style::default()
  })
  .into()
}

fn rule_summary(wallet: &super::State, rule: &Rule) -> String {
  engine::summarize_rule(
    rule,
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(wallet, key),
  )
}

fn export_modal<'a>(wallet: &'a super::State, draft: &'a ExportDraft) -> Element<'a, Message> {
  let chosen = wallet
    .budget_rules()
    .iter()
    .filter(|rule| draft.selected.contains(&rule.id()))
    .count();
  let subtitle = text(t!("wallet.budget.pack_export_blurb").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::secondary()))
    .into();

  let content = Column::with_children(vec![
    pack_modal_header(
      Icon::arrow_out(),
      t!("wallet.budget.pack_export_title").into_owned(),
      subtitle,
      Message::ExportClosed,
    ),
    export_body(wallet, draft, chosen),
    pack_modal_footer(
      t!("wallet.budget.pack_export_footnote").into_owned(),
      Message::ExportClosed,
      t!("wallet.budget.pack_export_confirm", count => chosen).into_owned(),
      (chosen > 0).then_some(Message::ExportConfirmed),
    ),
  ])
  .width(Length::Fill);

  pack_panel(content.into(), 560.0)
}

fn export_body<'a>(wallet: &'a super::State, draft: &'a ExportDraft, chosen: usize) -> Element<'a, Message> {
  let rules = wallet.budget_rules();
  let name_field = Column::with_children(vec![
    eyebrow_label(super::i18n::tr_static("wallet.budget.pack_name_label")),
    text_input(
      super::i18n::tr_static("wallet.budget.pack_name_placeholder"),
      &draft.name,
    )
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 9.0,
      right: 11.0,
      bottom: 9.0,
      left: 11.0,
    })
    .width(Length::Fill)
    .on_input(Message::ExportNameChanged)
    .style(editor_input_style)
    .into(),
  ])
  .spacing(6.0)
  .width(Length::Fill);

  let note_field = Column::with_children(vec![
    eyebrow_label(super::i18n::tr_static("wallet.budget.pack_note_label")),
    text_input(
      super::i18n::tr_static("wallet.budget.pack_note_placeholder"),
      &draft.note,
    )
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 9.0,
      right: 11.0,
      bottom: 9.0,
      left: 11.0,
    })
    .width(Length::Fill)
    .on_input(Message::ExportNoteChanged)
    .style(editor_input_style)
    .into(),
  ])
  .spacing(6.0)
  .width(Length::Fill);

  let count_row = Row::with_children(vec![
    container(mono_caption(
      t!("wallet.budget.pack_rules_count", chosen => chosen, total => rules.len()).into_owned(),
      color::text::secondary(),
      typography::size::XS,
    ))
    .width(Length::Fill)
    .into(),
    mini_link(
      super::i18n::tr_static("wallet.budget.pack_select_all"),
      Message::ExportAllSelected,
    ),
    mini_link(
      super::i18n::tr_static("wallet.budget.pack_select_none"),
      Message::ExportNoneSelected,
    ),
  ])
  .spacing(6.0)
  .align_y(Vertical::Center);

  let rows = rules
    .iter()
    .map(|rule| export_rule_row(wallet, rule, draft.selected.contains(&rule.id())))
    .collect::<Vec<Element<'a, Message>>>();
  let list = container(Column::with_children(rows).width(Length::Fill)).style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 9.0.into(),
    },
    ..container::Style::default()
  });

  let body = Column::with_children(vec![
    name_field.into(),
    note_field.into(),
    count_row.into(),
    list.into(),
  ])
  .spacing(14.0)
  .width(Length::Fill)
  .padding(Padding {
    top: 16.0,
    right: 20.0,
    bottom: 16.0,
    left: 20.0,
  });

  scrollable(body)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn export_rule_row<'a>(wallet: &'a super::State, rule: &'a Rule, selected: bool) -> Element<'a, Message> {
  let (tone, category_name) = category_label(wallet, rule);
  let title = Row::with_children(vec![
    text(rule_display_name(wallet, rule))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    color_dot(tone, 8.0),
    mono_caption(category_name, color::text::tertiary(), typography::size::XS),
  ])
  .spacing(8.0)
  .align_y(Vertical::Center);

  let detail = Column::with_children(vec![
    title.into(),
    mono_caption(
      rule_summary(wallet, rule),
      color::text::secondary(),
      typography::size::XS,
    ),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![checkbox_visual(selected), detail.into()])
    .spacing(11.0)
    .align_y(Vertical::Center);

  button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 10.0,
      right: 13.0,
      bottom: 10.0,
      left: 13.0,
    })
    .on_press(Message::ExportRuleToggled(rule.id()))
    .style(move |_, _| button::Style {
      background: Some(Background::Color(if selected {
        color::with_alpha(color::accent::PLASMA, 0.05)
      } else {
        Color::TRANSPARENT
      })),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn import_modal<'a>(wallet: &'a super::State, draft: &'a ImportDraft) -> Element<'a, Message> {
  let kept = draft.plan.items.len().saturating_sub(draft.skipped.len());
  let title = if draft.pack.name.trim().is_empty() {
    t!("wallet.budget.pack_import_fallback_title").into_owned()
  } else {
    draft.pack.name.clone()
  };

  let mut children: Vec<Element<'a, Message>> = vec![pack_modal_header(
    Icon::upload(),
    title,
    mono_caption(import_meta(draft), color::text::secondary(), typography::size::XS_PLUS),
    Message::ImportClosed,
  )];
  if !draft.pack.note.trim().is_empty() {
    children.push(import_note(&draft.pack.note));
  }

  let rows = draft
    .plan
    .items
    .iter()
    .enumerate()
    .map(|(index, item)| import_item_row(wallet, draft, index, item))
    .collect::<Vec<Element<'a, Message>>>();
  children.push(
    scrollable(Column::with_children(rows).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
  );

  children.push(pack_modal_footer(
    import_footer_note(draft),
    Message::ImportClosed,
    t!("wallet.budget.pack_import_confirm", count => kept).into_owned(),
    (kept > 0).then_some(Message::ImportConfirmed),
  ));

  pack_panel(Column::with_children(children).width(Length::Fill).into(), 600.0)
}

fn import_meta(draft: &ImportDraft) -> String {
  let count = draft.pack.rules.len();
  let rules_phrase = if count == 1 {
    t!("wallet.budget.global_rules_singular", count => count).into_owned()
  } else {
    t!("wallet.budget.global_rules_plural", count => count).into_owned()
  };
  if draft.pack.author.trim().is_empty() {
    rules_phrase
  } else {
    format!(
      "{rules_phrase} {}",
      t!("wallet.budget.pack_from_author", author => draft.pack.author)
    )
  }
}

fn import_note<'a>(note: &str) -> Element<'a, Message> {
  container(
    text(format!("\u{201c}{note}\u{201d}"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 11.0,
    right: 20.0,
    bottom: 11.0,
    left: 20.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.05))),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn import_item_row<'a>(
  wallet: &'a super::State,
  draft: &'a ImportDraft,
  index: usize,
  item: &'a rule_pack::PlanItem,
) -> Element<'a, Message> {
  let kept = !draft.skipped.contains(&index);
  let (tone, category_name) = plan_target_label(wallet, &draft.plan, item.target);

  let mut title_children: Vec<Element<'a, Message>> = vec![
    text(item.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    mono_caption("\u{2192}", color::text::tertiary(), typography::size::XS),
    color_dot(tone.as_deref(), 8.0),
    mono_caption(category_name, color::text::secondary(), typography::size::XS),
  ];
  if matches!(item.target, rule_pack::CategoryTarget::Create(_)) {
    title_children.push(status_chip(
      super::i18n::tr_static("wallet.budget.pack_new_envelope_badge"),
      color::status::ONLINE,
    ));
  }
  if item.is_duplicate {
    title_children.push(status_chip(
      super::i18n::tr_static("wallet.budget.pack_duplicate_badge"),
      color::status::WARNING,
    ));
  }

  let mut content: Vec<Element<'a, Message>> = vec![
    Row::with_children(title_children)
      .spacing(8.0)
      .align_y(Vertical::Center)
      .wrap()
      .into(),
    mono_caption(
      plan_item_summary(wallet, item),
      color::text::secondary(),
      typography::size::XS,
    ),
  ];
  if item.is_duplicate && kept {
    content.push(
      text(t!("wallet.budget.pack_duplicate_warning"))
        .font(typography::body::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::status::WARNING))
        .into(),
    );
  }

  let detail = Column::with_children(content).spacing(4.0).width(Length::Fill);
  let row = Row::with_children(vec![checkbox_visual(kept), detail.into()])
    .spacing(11.0)
    .align_y(Vertical::Center);

  button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 12.0,
      right: 20.0,
      bottom: 12.0,
      left: 20.0,
    })
    .on_press(Message::ImportSkipToggled(index))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn plan_target_label(
  wallet: &super::State,
  plan: &rule_pack::ImportPlan,
  target: rule_pack::CategoryTarget,
) -> (Option<String>, String) {
  match target {
    rule_pack::CategoryTarget::Create(index) => {
      let planned = &plan.created_categories[index];
      (Some(planned.tone.clone()), planned.name.clone())
    }
    rule_pack::CategoryTarget::Existing(id) => match wallet.budget().and_then(|view| view.category(id)) {
      Some(category) => (category.tone.clone(), category.name.clone()),
      None => (None, String::new()),
    },
  }
}

fn plan_item_summary(wallet: &super::State, item: &rule_pack::PlanItem) -> String {
  let rule = Rule {
    category_id: 0,
    conditions: item.conditions.clone(),
    enabled: true,
    id: 0,
    match_mode: item.match_mode,
    name: item.name.clone(),
  };
  rule_summary(wallet, &rule)
}

fn import_footer_note(draft: &ImportDraft) -> String {
  let duplicates = draft.plan.items.iter().filter(|item| item.is_duplicate).count();
  let created = draft
    .plan
    .items
    .iter()
    .enumerate()
    .filter(|(index, item)| {
      !draft.skipped.contains(index) && matches!(item.target, rule_pack::CategoryTarget::Create(_))
    })
    .count();
  let mut parts: Vec<String> = Vec::new();
  if duplicates == 1 {
    parts.push(t!("wallet.budget.pack_dups_skipped_singular", count => duplicates).into_owned());
  } else if duplicates > 1 {
    parts.push(t!("wallet.budget.pack_dups_skipped_plural", count => duplicates).into_owned());
  }
  if created == 1 {
    parts.push(t!("wallet.budget.pack_new_envelopes_singular", count => created).into_owned());
  } else if created > 1 {
    parts.push(t!("wallet.budget.pack_new_envelopes_plural", count => created).into_owned());
  }
  parts.join(" ")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn category(id: i64) -> budget::Category {
    budget::Category {
      activity: -50.0,
      assigned: 400.0,
      avg_assigned: 100.0,
      carry: 200.0,
      id,
      last_assigned: 120.0,
      name: format!("Category {id}"),
      note: Some("note".to_owned()),
      spent_last: 80.0,
      target: budget::Target {
        amount: 1_000.0,
        by_date: None,
        kind: budget::TargetKind::Monthly,
      },
      tone: Some("plasma".to_owned()),
    }
  }

  fn sample_rule(id: i64, category_id: i64) -> Rule {
    Rule {
      category_id,
      conditions: vec![crate::store::model::RuleCondition {
        field: RuleField::Text,
        op: RuleOp::Contains,
        value: "Cerberus".to_owned(),
        value2: None,
      }],
      enabled: true,
      id,
      match_mode: MatchMode::All,
      name: "Doctrine hulls".to_owned(),
    }
  }

  fn wallet_with_rules(rules: Vec<Rule>) -> super::super::State {
    let mut wallet = super::super::State::new(crate::config::FeatureFlags::default());
    wallet.budget = Some(budget::BudgetView {
      groups: vec![budget::Group {
        categories: vec![category(1), category(2)],
        id: 10,
        name: "Bills".to_owned(),
      }],
      month: budget::current_month(),
      overspent: 0.0,
      pool: 5_000.0,
      ready_to_assign: 1_500.0,
    });
    wallet.budget_chips.resolution.rules = rules;
    wallet
  }

  mod open_editor {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_the_editor_from_an_existing_rule() {
      let wallet = wallet_with_rules(vec![sample_rule(5, 1)]);
      let mut state = State::default();

      state.open_editor(&wallet, EditorSeed::Existing(5));

      let draft = state.editor.as_ref().unwrap();
      assert_eq!(draft.rule_id, Some(5));
      assert_eq!(draft.category_id, 1);
      assert_eq!(draft.name, "Doctrine hulls");
    }

    #[test]
    fn it_clears_the_editor_for_an_unknown_rule() {
      let wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      state.open_editor(&wallet, EditorSeed::Existing(99));

      assert!(state.editor.is_none());
    }

    #[test]
    fn it_opens_a_blank_draft_for_a_new_rule() {
      let wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();

      state.open_editor(&wallet, EditorSeed::New(2));

      let draft = state.editor.as_ref().unwrap();
      assert_eq!(draft.category_id, 2);
      assert!(draft.rule_id.is_none());
    }
  }

  mod clear_editor_for_rule {
    use super::*;

    #[test]
    fn it_clears_only_a_draft_editing_the_deleted_rule() {
      let wallet = wallet_with_rules(vec![sample_rule(5, 1)]);
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::Existing(5));

      state.clear_editor_for_rule(4);
      assert!(state.editor.is_some());

      state.clear_editor_for_rule(5);
      assert!(state.editor.is_none());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_opens_and_closes_the_editor() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(5, 1)]);
      let mut state = State::default();

      let _ = update(&mut state, &mut wallet, &db, Message::RuleEditOpened(5));
      assert_eq!(state.editor.as_ref().map(|draft| draft.rule_id), Some(Some(5)));

      let _ = update(&mut state, &mut wallet, &db, Message::EditorClosed);
      assert!(state.editor.is_none());
    }

    #[tokio::test]
    async fn it_threads_the_search_box_into_the_first_text_condition() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::EditorSearchChanged("Cerberus".to_owned()),
      );

      assert_eq!(state.editor.as_ref().unwrap().search_value(), "Cerberus");
    }

    #[tokio::test]
    async fn it_adds_and_removes_advanced_conditions() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let _ = update(&mut state, &mut wallet, &db, Message::ConditionAdded);
      assert_eq!(state.editor.as_ref().unwrap().conditions.len(), 2);

      let _ = update(&mut state, &mut wallet, &db, Message::ConditionRemoved(1));
      assert_eq!(state.editor.as_ref().unwrap().conditions.len(), 1);
    }

    #[tokio::test]
    async fn it_keeps_at_least_one_condition_when_removing() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let _ = update(&mut state, &mut wallet, &db, Message::ConditionRemoved(0));

      assert_eq!(state.editor.as_ref().unwrap().conditions.len(), 1);
    }

    #[tokio::test]
    async fn it_changes_a_condition_field_and_seeds_its_default_op() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ConditionFieldChanged(0, RuleField::Amount),
      );

      let condition = &state.editor.as_ref().unwrap().conditions[0];
      assert_eq!(condition.field(), RuleField::Amount);
      assert_eq!(condition.op(), RuleOp::GreaterThan);
    }

    #[tokio::test]
    async fn it_tracks_the_open_select_dropdown() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let key = budget::RuleSelectKey::Field(0);
      let _ = update(&mut state, &mut wallet, &db, Message::EditorSelectToggled(Some(key)));
      assert_eq!(state.editor.as_ref().unwrap().open_select, Some(key));

      let _ = update(&mut state, &mut wallet, &db, Message::EditorSelectToggled(Some(key)));
      assert!(state.editor.as_ref().unwrap().open_select.is_none());
    }

    #[tokio::test]
    async fn it_closes_the_editor_on_commit() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let _ = update(&mut state, &mut wallet, &db, Message::EditorCommitted);

      assert!(state.editor.is_none());
    }

    #[tokio::test]
    async fn it_edits_the_open_drafts_metadata() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::EditorNameChanged("My rule".to_owned()),
      );
      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::EditorMatchModeSelected(MatchMode::Any),
      );
      let _ = update(&mut state, &mut wallet, &db, Message::EditorAdvancedToggled);

      let draft = state.editor.as_ref().unwrap();
      assert_eq!(draft.name, "My rule");
      assert!(draft.name_edited);
      assert_eq!(draft.match_mode, MatchMode::Any);
      assert!(draft.show_advanced);
    }

    #[tokio::test]
    async fn it_edits_a_conditions_values() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));
      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ConditionFieldChanged(0, RuleField::Amount),
      );
      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ConditionOpChanged(0, RuleOp::Between),
      );

      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ConditionValueChanged(0, "100m".to_owned()),
      );
      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ConditionValue2Changed(0, "1b".to_owned()),
      );

      let condition = &state.editor.as_ref().unwrap().conditions[0];
      assert_eq!(condition.op(), RuleOp::Between);
      assert_eq!(condition.value(), "100m");
      assert_eq!(condition.value2.as_deref(), Some("1b"));
    }

    #[tokio::test]
    async fn it_tracks_a_rule_drag_and_drop_target() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();

      let _ = update(&mut state, &mut wallet, &db, Message::DragStarted(1));
      assert_eq!(state.dragging, Some(1));

      let _ = update(&mut state, &mut wallet, &db, Message::DropTargetEntered(2));
      assert_eq!(state.drop_target, Some(2));

      let _ = update(&mut state, &mut wallet, &db, Message::DropTargetLeft);
      assert_eq!(state.drop_target, Some(2));
    }

    #[tokio::test]
    async fn it_ignores_drop_target_changes_without_an_active_drag() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();

      let _ = update(&mut state, &mut wallet, &db, Message::DropTargetEntered(2));
      assert!(state.drop_target.is_none());

      let _ = update(&mut state, &mut wallet, &db, Message::DropTargetLeft);
      assert!(state.drop_target.is_none());
    }

    #[tokio::test]
    async fn it_clears_the_drag_state_when_dropping_a_rule_on_itself() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 2)]);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::DragStarted(1));
      let _ = update(&mut state, &mut wallet, &db, Message::DropTargetEntered(1));

      let _ = update(&mut state, &mut wallet, &db, Message::DropReleased);

      assert!(state.dragging.is_none());
      assert!(state.drop_target.is_none());
    }

    #[tokio::test]
    async fn it_reorders_the_rules_on_a_drop() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 2), sample_rule(3, 1)]);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::DragStarted(3));
      let _ = update(&mut state, &mut wallet, &db, Message::DropTargetEntered(1));

      let _ = update(&mut state, &mut wallet, &db, Message::DropReleased);

      assert!(state.dragging.is_none());
      assert!(state.drop_target.is_none());
    }

    #[tokio::test]
    async fn it_clears_a_matching_editor_when_its_rule_is_deleted() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(5, 1)]);
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::Existing(5));

      let _ = update(&mut state, &mut wallet, &db, Message::RuleDeleted(5));

      assert!(state.editor.is_none());
    }
  }

  mod pack {
    use pretty_assertions::assert_eq;

    use super::*;

    fn encoded_pack(wallet: &crate::features::wallet::State) -> String {
      let groups = budget_groups(wallet);
      let portables = wallet
        .budget_rules()
        .iter()
        .map(|rule| rule_pack::portable_rule(rule, rule.name().clone(), groups))
        .collect();
      rule_pack::encode_pack(&rule_pack::build_pack(portables, "Test pack", "note")).unwrap()
    }

    #[tokio::test]
    async fn it_opens_the_export_modal_preselecting_all_rules() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 2)]);
      let mut state = State::default();

      let _ = update(&mut state, &mut wallet, &db, Message::ExportOpened(None));

      let draft = state.export.as_ref().unwrap();
      assert_eq!(draft.selected, BTreeSet::from([1, 2]));
    }

    #[tokio::test]
    async fn it_opens_the_export_modal_for_a_single_shared_rule() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 2)]);
      let mut state = State::default();

      let _ = update(&mut state, &mut wallet, &db, Message::ExportOpened(Some(2)));

      let draft = state.export.as_ref().unwrap();
      assert_eq!(draft.selected, BTreeSet::from([2]));
    }

    #[tokio::test]
    async fn it_toggles_and_bulk_selects_export_rules() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 2)]);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::ExportOpened(None));

      let _ = update(&mut state, &mut wallet, &db, Message::ExportRuleToggled(1));
      assert_eq!(state.export.as_ref().unwrap().selected, BTreeSet::from([2]));

      let _ = update(&mut state, &mut wallet, &db, Message::ExportNoneSelected);
      assert!(state.export.as_ref().unwrap().selected.is_empty());

      let _ = update(&mut state, &mut wallet, &db, Message::ExportAllSelected);
      assert_eq!(state.export.as_ref().unwrap().selected, BTreeSet::from([1, 2]));
    }

    #[tokio::test]
    async fn it_edits_the_pack_name_and_note() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1)]);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::ExportOpened(None));

      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ExportNameChanged("Doctrine".to_owned()),
      );
      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ExportNoteChanged("For line members".to_owned()),
      );

      let draft = state.export.as_ref().unwrap();
      assert_eq!(draft.name, "Doctrine");
      assert_eq!(draft.note, "For line members");
    }

    #[tokio::test]
    async fn it_closes_the_export_modal_on_confirm() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1)]);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::ExportOpened(None));

      let _ = update(&mut state, &mut wallet, &db, Message::ExportConfirmed);

      assert!(state.export.is_none());
    }

    #[tokio::test]
    async fn it_stages_an_import_preview_and_preskips_duplicates() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 2)]);
      let encoded = encoded_pack(&wallet);
      let mut state = State::default();

      let _ = update(&mut state, &mut wallet, &db, Message::ImportFileLoaded(Some(encoded)));

      let draft = state.import.as_ref().unwrap();
      assert_eq!(draft.plan.items.len(), 2);
      assert!(draft.plan.items.iter().all(|item| item.is_duplicate));
      assert_eq!(draft.skipped, BTreeSet::from([0, 1]));
      assert!(state.import_error.is_none());
    }

    #[tokio::test]
    async fn it_surfaces_an_error_for_a_corrupt_file_and_dismisses_it() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();

      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ImportFileLoaded(Some("not a pack".to_owned())),
      );
      assert!(state.import.is_none());
      assert_eq!(state.import_error, Some(rule_pack::ParseError::NotAPack));

      let _ = update(&mut state, &mut wallet, &db, Message::ImportErrorDismissed);
      assert!(state.import_error.is_none());
    }

    #[tokio::test]
    async fn it_ignores_a_cancelled_file_dialog() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();

      let _ = update(&mut state, &mut wallet, &db, Message::ImportFileLoaded(None));

      assert!(state.import.is_none());
      assert!(state.import_error.is_none());
    }

    #[tokio::test]
    async fn it_toggles_the_skip_state_of_an_import_item() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1)]);
      let encoded = encoded_pack(&wallet);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::ImportFileLoaded(Some(encoded)));

      let _ = update(&mut state, &mut wallet, &db, Message::ImportSkipToggled(0));
      assert!(state.import.as_ref().unwrap().skipped.is_empty());

      let _ = update(&mut state, &mut wallet, &db, Message::ImportSkipToggled(0));
      assert_eq!(state.import.as_ref().unwrap().skipped, BTreeSet::from([0]));
    }

    #[tokio::test]
    async fn it_closes_the_import_modal_on_confirm() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1)]);
      let encoded = encoded_pack(&wallet);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::ImportFileLoaded(Some(encoded)));
      let _ = update(&mut state, &mut wallet, &db, Message::ImportSkipToggled(0));

      let _ = update(&mut state, &mut wallet, &db, Message::ImportConfirmed);

      assert!(state.import.is_none());
    }

    #[tokio::test]
    async fn it_closes_the_topmost_layer_on_escape() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1)]);
      let encoded = encoded_pack(&wallet);
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::Existing(1));
      let _ = update(&mut state, &mut wallet, &db, Message::ExportOpened(None));
      let _ = update(&mut state, &mut wallet, &db, Message::ImportFileLoaded(Some(encoded)));

      let _ = update(&mut state, &mut wallet, &db, Message::EscapePressed);
      assert!(state.import.is_none());
      assert!(state.export.is_some());

      let _ = update(&mut state, &mut wallet, &db, Message::EscapePressed);
      assert!(state.export.is_none());
      assert!(state.editor.is_some());

      let _ = update(&mut state, &mut wallet, &db, Message::EscapePressed);
      assert!(state.editor.is_none());
    }

    #[tokio::test]
    async fn it_renders_the_export_and_import_modals() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 999)]);
      let encoded = encoded_pack(&wallet);
      let mut state = State::default();
      let _ = update(&mut state, &mut wallet, &db, Message::ExportOpened(None));
      let _ = update(&mut state, &mut wallet, &db, Message::ImportFileLoaded(Some(encoded)));

      let _el: Element<'_, Message> = view(&wallet, &state);
    }

    #[tokio::test]
    async fn it_renders_the_import_error_banner() {
      let db = crate::store::open_test().await.unwrap();
      let mut wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      let _ = update(
        &mut state,
        &mut wallet,
        &db,
        Message::ImportFileLoaded(Some("garbage".to_owned())),
      );

      let _el: Element<'_, Message> = view(&wallet, &state);
    }

    #[test]
    fn it_maps_every_parse_error_to_a_message() {
      for error in [
        rule_pack::ParseError::Empty,
        rule_pack::ParseError::NoUsableConditions,
        rule_pack::ParseError::NotAPack,
        rule_pack::ParseError::UnsupportedVersion,
        rule_pack::ParseError::WrongFormat,
      ] {
        assert!(!import_error_text(&error).is_empty());
      }
    }
  }

  mod subscription {
    use super::*;

    #[test]
    fn it_listens_only_while_dragging_or_editing() {
      let wallet = wallet_with_rules(vec![sample_rule(5, 1)]);
      let mut state = State::default();
      let _ = subscription(&state);

      state.dragging = Some(5);
      let _ = subscription(&state);

      state.open_editor(&wallet, EditorSeed::Existing(5));
      let _ = subscription(&state);
    }
  }

  mod view {
    use pretty_assertions::assert_eq;

    use super::*;

    fn condition(field: RuleField, op: RuleOp) -> crate::store::model::RuleCondition {
      crate::store::model::RuleCondition {
        field,
        op,
        value: "100m".to_owned(),
        value2: Some("1b".to_owned()),
      }
    }

    #[test]
    fn it_renders_the_empty_state() {
      let wallet = wallet_with_rules(Vec::new());
      let state = State::default();

      let _el: Element<'_, Message> = view(&wallet, &state);
    }

    #[test]
    fn it_renders_a_rule_row_while_dragging() {
      let wallet = wallet_with_rules(vec![sample_rule(1, 1), sample_rule(2, 999)]);
      let state = State {
        dragging: Some(1),
        drop_target: Some(2),
        ..State::default()
      };

      let _el: Element<'_, Message> = view(&wallet, &state);
    }

    #[test]
    fn it_renders_the_editor_overlay_for_a_new_rule() {
      let wallet = wallet_with_rules(Vec::new());
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::New(1));

      let _el: Element<'_, Message> = view(&wallet, &state);
    }

    #[test]
    fn it_renders_the_editor_in_advanced_mode() {
      let wallet = wallet_with_rules(vec![sample_rule(5, 1)]);
      let mut state = State::default();
      state.open_editor(&wallet, EditorSeed::Existing(5));
      if let Some(draft) = state.editor.as_mut() {
        draft.show_advanced = true;
        draft.conditions.push(condition(RuleField::Amount, RuleOp::Between));
      }

      let _el: Element<'_, Message> = view(&wallet, &state);
    }

    #[test]
    fn it_renders_an_editor_for_every_field_kind() {
      let wallet = wallet_with_rules(Vec::new());
      let draft = budget::RuleDraft::new(1);
      let conditions = [
        condition(RuleField::Type, RuleOp::Is),
        condition(RuleField::Character, RuleOp::Is),
        condition(RuleField::Direction, RuleOp::Is),
        condition(RuleField::Amount, RuleOp::Between),
        condition(RuleField::Amount, RuleOp::GreaterThan),
        condition(RuleField::Text, RuleOp::Contains),
      ];

      for (index, condition) in conditions.iter().enumerate() {
        let _el: Element<'_, Message> = condition_value_editor(&wallet, &draft, index, condition);
      }
    }

    #[test]
    fn it_labels_the_target_category_with_its_tone() {
      let wallet = wallet_with_rules(Vec::new());

      let (tone, name) = category_label(&wallet, &sample_rule(1, 1));

      assert_eq!(tone, Some("plasma"));
      assert_eq!(name, "Category 1");
    }

    #[test]
    fn it_falls_back_to_an_empty_label_for_an_unknown_category() {
      let wallet = wallet_with_rules(Vec::new());

      let (tone, name) = category_label(&wallet, &sample_rule(1, 999));

      assert_eq!(tone, None);
      assert_eq!(name, String::new());
    }
  }
}
