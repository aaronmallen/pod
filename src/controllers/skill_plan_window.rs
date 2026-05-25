//! Skill plan window controller: state, update, and view for the plan editor.

use std::collections::{HashMap, HashSet};

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, mouse_area, row, text},
};
use pod_model::{Certificate, ItemTypeSummary, SkillGroupDef, SkillPlan};
pub use pod_ui::views::skill_plan::Message;
use pod_ui::{
  plan_math::{
    BaseAttrs, ComputedPlan, EffectiveAttrs, ImplantBonus, ImplantSaving, ImplantSet, PlanEntry, Priority, RemapResult,
    compute_implant_savings, compute_plan, effective_attrs, expand_wishes, implant_bonus_for_set, optimize_remap,
    pair_weights, skills_for_mastery, skills_for_module,
  },
  style::{color, spacing, typography::body},
  views::skill_plan::{PaneEdge, editor::PlanEditor, picker::SkillPicker, summary::Component as PlanSummary},
};

use crate::services::Services;

/// The kind of seed used to initialise a skill plan window.
#[derive(Debug, Clone)]
pub enum PlanSeed {
  New,
  FromQueue(Vec<(String, u8)>),
  Existing(String),
}

struct NewParams {
  window_id: iced::window::Id,
  character_id: i64,
  picker_pane_width: f32,
  summary_pane_width: f32,
  base_attrs: BaseAttrs,
  current_effective_attrs: BaseAttrs,
  clone_data_missing: bool,
  implant_bonus: ImplantBonus,
  eff: EffectiveAttrs,
}

/// State for a skill plan editor window.
pub struct State {
  pub window_id: iced::window::Id,
  pub character_id: i64,
  pub plan_id: Option<String>,
  pub plan_name: String,
  pub entries: Vec<PlanEntry>,
  pub picker_open: bool,
  pub picker_search: String,
  pub picker_expanded_groups: HashSet<String>,
  pub dirty: bool,
  pub saved_snapshot: String,
  pub implant_set: ImplantSet,
  pub optimizer_result: Option<RemapResult>,
  pub optimizer_running: bool,
  pub show_remap: bool,
  pub show_implant_suggestions: bool,
  pub import_dropdown_open: bool,
  pub export_dropdown_open: bool,
  pub confirm_close: bool,
  pub note_expanded: Option<String>,
  /// Unimplanted base attributes (ESI effective attrs minus clone implant
  /// bonuses). Used as the starting point for remap optimisation.
  pub base_attrs: BaseAttrs,
  /// Raw ESI effective attributes (base + current implants already baked
  /// in). Copied directly into `eff` when `ImplantSet::Current` is active.
  pub current_effective_attrs: BaseAttrs,
  /// True when the character's active-clone data has not yet been synced
  /// from ESI, so we cannot compute a meaningful implant bonus.
  pub clone_data_missing: bool,
  pub implant_bonus: ImplantBonus,
  pub remap_cooldown_days: i32,
  pub remap_available: bool,
  pub bonus_remaps: u32,
  pub computed: ComputedPlan,
  pub eff: EffectiveAttrs,
  pub implant_savings: Vec<ImplantSaving>,
  pub planned_levels: HashMap<String, u8>,
  pub picker_pane_width: f32,
  pub summary_pane_width: f32,
  pub dragging_pane: Option<PaneEdge>,
  pub last_drag_x: f32,
  pub dragging_entry_id: Option<String>,
  pub drag_hover_entry_id: Option<String>,
  pub picker_tab: usize,
  pub picker_ships: Vec<ItemTypeSummary>,
  pub picker_modules: Vec<ItemTypeSummary>,
  pub certificates: HashMap<i32, Certificate>,
  pub ship_mastery_selection: HashMap<i32, u8>,
  pub picker_ship_search: String,
  pub picker_module_search: String,
  pub ships_loaded: bool,
  pub modules_loaded: bool,
  pub picker_certs: Vec<Certificate>,
  pub certs_loaded: bool,
  pub cert_proficiency_selection: HashMap<i32, u8>,
  pub skill_groups: Vec<SkillGroupDef>,
  pub pending_from_queue: Option<Vec<(String, u8)>>,
}

/// Creates a new skill plan window state and optional load task.
///
/// `base_attrs` is the character's unimplanted attribute values (ESI
/// effective attrs minus the active-clone implant bonuses).
/// `current_effective_attrs` is the raw ESI value (implants already
/// included). `clone_data_missing` is true when no active-clone data is
/// available, meaning `ImplantSet::Current` cannot be calculated accurately.
#[allow(clippy::too_many_arguments)]
pub fn new(
  window_id: iced::window::Id,
  character_id: i64,
  seed: PlanSeed,
  picker_pane_width: f32,
  summary_pane_width: f32,
  db: Option<pod_db::Repo>,
  base_attrs: BaseAttrs,
  current_effective_attrs: BaseAttrs,
  clone_data_missing: bool,
) -> (State, iced::Task<Message>) {
  let implant_bonus = ImplantBonus::default();
  let eff = effective_attrs(&base_attrs, &implant_bonus);
  let p = NewParams {
    window_id,
    character_id,
    picker_pane_width,
    summary_pane_width,
    base_attrs,
    current_effective_attrs,
    clone_data_missing,
    implant_bonus,
    eff,
  };
  match seed {
    PlanSeed::Existing(plan_id) => new_from_existing(p, plan_id, db),
    PlanSeed::FromQueue(items) => new_from_queue(p, items, db),
    PlanSeed::New => new_fresh(p, db),
  }
}

/// Processes a skill plan window message and returns a task.
pub fn update(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::AllCertsLoaded(_)
    | Message::CertificatesLoaded(_)
    | Message::CertProficiencyChanged(_, _)
    | Message::CertSelected(_, _, _)
    | Message::ModuleSelected(_, _)
    | Message::ModulesLoaded(_)
    | Message::PickerGroupToggled(_)
    | Message::PickerSearchChanged(_)
    | Message::PickerTabChanged(_)
    | Message::PickerToggled
    | Message::ShipMasteryChanged(_, _)
    | Message::ShipSelected(_, _, _)
    | Message::ShipsLoaded(_)
    | Message::SkillGroupsLoaded(_)
    | Message::SkillPicked(_, _) => update_picker_messages(state, message, services),
    _ => update_plan_messages(state, message, services),
  }
}

fn pane_drag_subscription() -> iced::Subscription<Message> {
  use iced::{
    event::{self, Event},
    mouse,
  };
  event::listen_with(|event, _status, _id| match event {
    Event::Mouse(mouse::Event::CursorMoved {
      position,
    }) => Some(Message::PaneDrag(position.x)),
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::PaneDragEnd),
    _ => None,
  })
}

fn entry_drag_subscription() -> iced::Subscription<Message> {
  use iced::{
    event::{self, Event},
    mouse,
  };
  event::listen_with(|event, _status, _id| match event {
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::EntryDragEnd),
    _ => None,
  })
}

/// Returns background subscriptions for the skill plan window.
pub fn subscription(state: &State) -> iced::Subscription<Message> {
  if state.dragging_pane.is_some() {
    return pane_drag_subscription();
  }
  if state.dragging_entry_id.is_some() {
    return entry_drag_subscription();
  }
  iced::Subscription::none()
}

/// Renders the skill plan window.
pub fn view(state: &State) -> iced::Element<'_, Message> {
  let active_search = match state.picker_tab {
    1 => state.picker_ship_search.as_str(),
    2 => state.picker_module_search.as_str(),
    _ => state.picker_search.as_str(),
  };
  let picker_col = view_picker_col(state, active_search);
  let editor_col = PlanEditor::new(
    &state.plan_name,
    state.dirty,
    state.picker_open,
    state.import_dropdown_open,
    state.export_dropdown_open,
    &state.computed,
    state.note_expanded.as_deref(),
    state.dragging_entry_id.as_deref(),
    state.drag_hover_entry_id.as_deref(),
  )
  .render();
  let (summary_divider, summary_container) = view_summary_panel(state);
  let base_view = view_main_content(picker_col, editor_col, summary_divider, summary_container);
  let base_view = maybe_drag_overlay(state, base_view);
  if state.confirm_close {
    modal_overlay(base_view, confirm_close_modal())
  } else {
    base_view
  }
}

fn modal_overlay<'a>(base: Element<'a, Message>, modal: Element<'a, Message>) -> Element<'a, Message> {
  let backdrop = container(Space::new())
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.6))),
      ..container::Style::default()
    });

  let layered = container(column([base]).width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill);

  let overlay = container(
    container(modal)
      .width(Length::Fixed(420.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::SUBTLE,
          radius: 12.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center);

  let stack = iced::widget::stack([layered.into(), backdrop.into(), overlay.into()]);
  stack.into()
}

fn confirm_close_modal() -> Element<'static, Message> {
  let title = text("Unsaved changes")
    .font(body::MEDIUM)
    .size(16.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let body_text = text("You have unsaved changes. Discard them and close this window?")
    .font(body::REGULAR)
    .size(13.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let btn_row = row([
    Space::new().width(Length::Fill).into(),
    confirm_cancel_btn().into(),
    Space::new().width(spacing::SPACE_2).into(),
    confirm_discard_btn().into(),
  ])
  .align_y(Vertical::Center);

  container(
    column([
      title.into(),
      Space::new().height(spacing::SPACE_3).into(),
      body_text.into(),
      Space::new().height(spacing::SPACE_4).into(),
      btn_row.into(),
    ])
    .width(Length::Fill),
  )
  .padding(Padding::new(24.0))
  .into()
}

fn confirm_cancel_btn() -> iced::widget::Button<'static, Message> {
  button(
    text("Cancel")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .on_press(Message::CancelClose)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05)))
      }
      _ => Some(Background::Color(Color::TRANSPARENT)),
    },
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
}

fn confirm_discard_btn() -> iced::widget::Button<'static, Message> {
  button(
    text("Discard")
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .on_press(Message::ConfirmClose)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(Color::from_rgba(0.878, 0.459, 0.349, 0.12)))
      }
      _ => Some(Background::Color(Color::TRANSPARENT)),
    },
    border: Border {
      color: color::status::DANGER,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
}

fn view_picker_col<'a>(state: &'a State, active_search: &'a str) -> Option<Element<'a, Message>> {
  if !state.picker_open {
    return None;
  }
  Some(
    container(
      SkillPicker::new(
        &state.skill_groups,
        &state.planned_levels,
        active_search,
        &state.picker_expanded_groups,
      )
      .tab(state.picker_tab)
      .ships(&state.picker_ships, &state.ship_mastery_selection, state.ships_loaded)
      .modules(&state.picker_modules, state.modules_loaded)
      .certs(
        &state.picker_certs,
        &state.cert_proficiency_selection,
        state.certs_loaded,
      )
      .render(),
    )
    .width(Length::Fixed(state.picker_pane_width))
    .height(Length::Fill)
    .into(),
  )
}

fn view_summary_panel(state: &State) -> (Element<'_, Message>, container::Container<'_, Message>) {
  let summary_col = PlanSummary::new(
    &state.computed,
    &state.base_attrs,
    &state.eff,
    &state.implant_bonus,
    state.implant_set,
    state.optimizer_result.as_ref(),
    state.optimizer_running,
    state.show_remap,
    state.show_implant_suggestions,
    &state.implant_savings,
    state.remap_cooldown_days,
    state.remap_available,
    state.bonus_remaps,
  )
  .clone_data_missing(state.clone_data_missing)
  .render();

  let summary_container = container(summary_col)
    .width(Length::Fixed(state.summary_pane_width))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    });

  let summary_divider: Element<'_, Message> = mouse_area(
    container(Space::new())
      .width(6.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      }),
  )
  .on_press(Message::PaneDragStart(PaneEdge::Summary))
  .interaction(iced::mouse::Interaction::ResizingHorizontally)
  .into();

  (summary_divider, summary_container)
}

fn view_main_content<'a>(
  picker_col: Option<Element<'a, Message>>,
  editor_col: Element<'a, Message>,
  summary_divider: Element<'a, Message>,
  summary_container: container::Container<'a, Message>,
) -> Element<'a, Message> {
  let mut cols: Vec<Element<'_, Message>> = Vec::new();
  if let Some(picker) = picker_col {
    let picker_divider: Element<'_, Message> = mouse_area(
      container(Space::new())
        .width(6.0)
        .height(Length::Fill)
        .style(|_| container::Style {
          background: Some(Background::Color(color::border::SUBTLE)),
          ..container::Style::default()
        }),
    )
    .on_press(Message::PaneDragStart(PaneEdge::Picker))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into();
    cols.push(picker);
    cols.push(picker_divider);
  }
  cols.push(editor_col);
  cols.push(summary_divider);
  cols.push(summary_container.into());
  let main_row = row(cols).height(Length::Fill).width(Length::Fill);
  container(main_row)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn maybe_drag_overlay<'a>(state: &State, base_view: Element<'a, Message>) -> Element<'a, Message> {
  if state.dragging_pane.is_none() {
    return base_view;
  }
  let capture_overlay = mouse_area(
    container(Space::new())
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        ..container::Style::default()
      }),
  )
  .interaction(iced::mouse::Interaction::ResizingHorizontally);
  iced::widget::stack([base_view, capture_overlay.into()]).into()
}

#[tracing::instrument(skip_all)]
fn recompute(state: &mut State) {
  state.eff = effective_attrs(&state.base_attrs, &state.implant_bonus);
  state.computed = compute_plan(&state.entries, &state.eff, &state.skill_groups);
  state.planned_levels = state.entries.iter().fold(HashMap::new(), |mut acc, e| {
    let existing = acc.entry(e.skill_name.clone()).or_insert(0u8);
    if e.to_level > *existing {
      *existing = e.to_level;
    }
    acc
  });
  if state.show_implant_suggestions {
    let weights = pair_weights(&state.entries, &state.eff, &state.skill_groups);
    state.implant_savings = compute_implant_savings(
      &weights,
      &state.base_attrs,
      &state.implant_bonus,
      state.computed.total_sec,
    );
  }
  tracing::debug!(
    "skill_plan: plan recomputed — {} entries, {} total seconds",
    state.entries.len(),
    state.computed.total_sec
  );
}

fn plan_snapshot(name: &str, entries: &[PlanEntry]) -> String {
  let mut parts = vec![name.to_string()];
  for e in entries {
    parts.push(format!(
      "{}|{}|{:?}|{}",
      e.skill_name,
      e.to_level,
      e.priority,
      e.note.as_deref().unwrap_or("")
    ));
  }
  parts.join("\n")
}

fn update_dirty(state: &mut State) {
  let current = plan_snapshot(&state.plan_name, &state.entries);
  state.dirty = current != state.saved_snapshot;
}

fn collect_wishes(entries: &[PlanEntry]) -> Vec<(String, u8)> {
  let mut seen: HashMap<String, u8> = HashMap::new();
  for e in entries {
    if !e.auto {
      let existing = seen.entry(e.skill_name.clone()).or_insert(0);
      if e.to_level > *existing {
        *existing = e.to_level;
      }
    }
  }
  let mut result: Vec<(String, u8)> = seen.into_iter().collect();
  result.sort_by_key(|(n, _)| n.clone());
  result
}

fn merge_wishes_into_plan(state: &mut State, new_wishes: Vec<(String, u8)>) {
  let mut wishes = collect_wishes(&state.entries);
  for (skill, level) in new_wishes {
    if let Some(existing) = wishes.iter_mut().find(|(n, _)| n == &skill) {
      if level > existing.1 {
        existing.1 = level;
      }
    } else {
      wishes.push((skill, level));
    }
  }
  let wish_refs: Vec<(&str, u8)> = wishes.iter().map(|(n, l)| (n.as_str(), *l)).collect();
  let new_entries = expand_wishes(&wish_refs, &state.skill_groups);
  state.entries = merge_entries(new_entries, &state.entries);
  recompute(state);
  update_dirty(state);
}

fn merge_entries(new_entries: Vec<PlanEntry>, old_entries: &[PlanEntry]) -> Vec<PlanEntry> {
  let old_map: HashMap<&str, &PlanEntry> = old_entries.iter().map(|e| (e.id.as_str(), e)).collect();

  new_entries
    .into_iter()
    .map(|mut e| {
      if let Some(old) = old_map.get(e.id.as_str()) {
        e.priority = old.priority;
        e.note = old.note.clone();
      }
      e
    })
    .collect()
}

fn parse_import_text(input: &str) -> Vec<(String, u8)> {
  let roman_map = [
    ("IV", 4u8),
    ("III", 3),
    ("VII", 0),
    ("II", 2),
    ("VI", 0),
    ("V", 5),
    ("I", 1),
  ];
  let mut result = Vec::new();
  for line in input.lines() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let mut matched = false;
    for &(roman, level) in &roman_map {
      if level == 0 {
        continue;
      }
      if let Some(skill_part) = line.strip_suffix(roman) {
        let skill = skill_part.trim_end();
        if !skill.is_empty() {
          result.push((skill.to_string(), level));
          matched = true;
          break;
        }
      }
    }
    if !matched && let Some(pos) = line.rfind(' ') {
      let level_str = &line[pos + 1..];
      if let Ok(n) = level_str.parse::<u8>()
        && (1..=5).contains(&n)
      {
        let skill = line[..pos].trim().to_string();
        if !skill.is_empty() {
          result.push((skill, n));
        }
      }
    }
  }
  result
}

fn state_to_skill_plan(state: &State) -> SkillPlan {
  let plan_id = state.plan_id.clone().unwrap_or_else(uuid_v4);
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs() as i64;

  let implant_set_str = match state.implant_set {
    ImplantSet::None => "none",
    ImplantSet::Plus3 => "plus3",
    ImplantSet::Plus4 => "plus4",
    ImplantSet::Plus5 => "plus5",
    ImplantSet::Current => "current",
  };

  let entries: Vec<pod_model::SkillPlanEntry> = state
    .entries
    .iter()
    .enumerate()
    .map(|(i, e)| pod_model::SkillPlanEntry {
      auto: e.auto,
      id: format!("{}-{}", plan_id, e.id),
      note: e.note.clone(),
      plan_id: plan_id.clone(),
      position: i as i32,
      priority: format!("{:?}", e.priority).to_lowercase(),
      skill_name: e.skill_name.clone(),
      to_level: e.to_level as i32,
    })
    .collect();

  SkillPlan {
    character_id: state.character_id,
    created_at: now,
    entries,
    id: plan_id,
    implant_set: implant_set_str.to_string(),
    name: state.plan_name.clone(),
    remap_json: None,
    updated_at: now,
  }
}

fn plan_entries_to_plan_entries(plan: &SkillPlan) -> Vec<PlanEntry> {
  plan
    .entries
    .iter()
    .map(|e| {
      let priority = match e.priority.as_str() {
        "low" => Priority::Low,
        "high" => Priority::High,
        _ => Priority::Normal,
      };
      PlanEntry {
        id: e.id.clone(),
        skill_name: e.skill_name.clone(),
        to_level: e.to_level as u8,
        priority,
        note: e.note.clone(),
        auto: e.auto,
      }
    })
    .collect()
}

fn uuid_v4() -> String {
  use std::time::{SystemTime, UNIX_EPOCH};
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .subsec_nanos();
  format!(
    "plan-{:016x}",
    (nanos as u64) ^ (nanos as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
  )
}

fn blank_state(
  p: NewParams,
  plan_id: Option<String>,
  plan_name: String,
  saved_snapshot: String,
  picker_open: bool,
  pending_from_queue: Option<Vec<(String, u8)>>,
) -> State {
  State {
    plan_id,
    plan_name,
    picker_open,
    saved_snapshot,
    pending_from_queue,
    remap_available: true,
    computed: ComputedPlan::default(),
    ..blank_state_zeroed(p)
  }
}

/// Builds a zeroed/default `State` from `NewParams`.
///
/// This function is intentionally larger than 50 lines because `State` has
/// 47 fields — each requires exactly one line in a struct literal, which
/// rustfmt enforces. All logic is trivially CC=1.
fn blank_state_zeroed(p: NewParams) -> State {
  State {
    window_id: p.window_id,
    character_id: p.character_id,
    plan_id: None,
    plan_name: String::new(),
    entries: Vec::new(),
    picker_open: false,
    picker_search: String::new(),
    picker_expanded_groups: HashSet::new(),
    dirty: false,
    saved_snapshot: String::new(),
    implant_set: ImplantSet::None,
    optimizer_result: None,
    optimizer_running: false,
    show_remap: false,
    show_implant_suggestions: false,
    import_dropdown_open: false,
    export_dropdown_open: false,
    confirm_close: false,
    note_expanded: None,
    base_attrs: p.base_attrs,
    current_effective_attrs: p.current_effective_attrs,
    clone_data_missing: p.clone_data_missing,
    implant_bonus: p.implant_bonus,
    remap_cooldown_days: 0,
    remap_available: false,
    bonus_remaps: 0,
    computed: ComputedPlan::default(),
    eff: p.eff,
    implant_savings: Vec::new(),
    planned_levels: HashMap::new(),
    picker_pane_width: p.picker_pane_width,
    summary_pane_width: p.summary_pane_width,
    dragging_pane: None,
    last_drag_x: 0.0,
    dragging_entry_id: None,
    drag_hover_entry_id: None,
    picker_tab: 0,
    picker_ships: Vec::new(),
    picker_modules: Vec::new(),
    certificates: HashMap::new(),
    ship_mastery_selection: HashMap::new(),
    picker_ship_search: String::new(),
    picker_module_search: String::new(),
    ships_loaded: false,
    modules_loaded: false,
    picker_certs: Vec::new(),
    certs_loaded: false,
    cert_proficiency_selection: HashMap::new(),
    skill_groups: Vec::new(),
    pending_from_queue: None,
  }
}

fn load_skill_groups_task(db: &pod_db::Repo) -> iced::Task<Message> {
  let db = db.clone();
  iced::Task::perform(
    async move { db.universe().item_types().find_skill_groups().await.unwrap_or_default() },
    Message::SkillGroupsLoaded,
  )
}

fn new_from_existing(p: NewParams, plan_id: String, db: Option<pod_db::Repo>) -> (State, iced::Task<Message>) {
  let state = blank_state(
    p,
    Some(plan_id.clone()),
    "Loading\u{2026}".to_string(),
    String::new(),
    false,
    None,
  );
  let plan_task = if let Some(db) = db.clone() {
    iced::Task::perform(
      async move { db.skill_plans().find(&plan_id).await.ok().flatten() },
      Message::PlanLoaded,
    )
  } else {
    iced::Task::perform(async { None }, Message::PlanLoaded)
  };
  let groups_task = db.as_ref().map(load_skill_groups_task).unwrap_or_else(iced::Task::none);
  (state, iced::Task::batch([plan_task, groups_task]))
}

fn new_from_queue(p: NewParams, items: Vec<(String, u8)>, db: Option<pod_db::Repo>) -> (State, iced::Task<Message>) {
  let plan_name = "Plan from queue".to_string();
  let snapshot = plan_snapshot(&plan_name, &[]);
  let mut state = blank_state(p, None, plan_name, snapshot, false, Some(items));
  recompute(&mut state);
  let task = db.as_ref().map(load_skill_groups_task).unwrap_or_else(iced::Task::none);
  (state, task)
}

fn new_fresh(p: NewParams, db: Option<pod_db::Repo>) -> (State, iced::Task<Message>) {
  let snapshot = plan_snapshot("Untitled plan", &[]);
  let state = blank_state(p, None, "Untitled plan".to_string(), snapshot, true, None);
  let task = db.as_ref().map(load_skill_groups_task).unwrap_or_else(iced::Task::none);
  (state, task)
}

fn update_all_certs_loaded(state: &mut State, certs: Vec<Certificate>) -> iced::Task<Message> {
  state.picker_certs = certs;
  state.certs_loaded = true;
  iced::Task::none()
}

fn update_attrs_loaded(
  state: &mut State,
  base_attrs: BaseAttrs,
  current_effective_attrs: BaseAttrs,
  clone_data_missing: bool,
) -> iced::Task<Message> {
  tracing::debug!(
    "skill_plan: character attributes refreshed — character_id: {}",
    state.character_id
  );
  state.base_attrs = base_attrs;
  state.current_effective_attrs = current_effective_attrs;
  state.clone_data_missing = clone_data_missing;
  state.implant_bonus = ImplantBonus::default();
  state.implant_set = ImplantSet::None;
  recompute(state);
  iced::Task::none()
}

fn update_cancel_close(state: &mut State) -> iced::Task<Message> {
  state.confirm_close = false;
  iced::Task::none()
}

fn update_cert_proficiency_changed(state: &mut State, cert_id: i32, prof: u8) -> iced::Task<Message> {
  state.cert_proficiency_selection.insert(cert_id, prof);
  iced::Task::none()
}

fn update_cert_selected(state: &mut State, cert_id: i32, prof: u8) -> iced::Task<Message> {
  tracing::info!(
    "skill_plan: certificate selected — cert_id: {cert_id}, proficiency: {prof}, plan: {}",
    state.plan_name
  );
  let type_id_to_name: HashMap<i32, String> = state
    .skill_groups
    .iter()
    .flat_map(|g| g.skills.iter())
    .map(|s| (s.type_id, s.name.clone()))
    .collect();
  let Some(cert) = state.picker_certs.iter().find(|c| c.id == cert_id) else {
    return iced::Task::none();
  };
  let prof_idx = (prof as usize).min(3);
  let skill_wishes: Vec<(String, u8)> = cert
    .skills
    .iter()
    .filter_map(|(type_id, levels)| {
      let level = levels[prof_idx];
      if level == 0 {
        return None;
      }
      type_id_to_name.get(type_id).map(|n| (n.clone(), level))
    })
    .collect();
  merge_wishes_into_plan(state, skill_wishes);
  iced::Task::none()
}

fn update_certificates_loaded(state: &mut State, certs: Vec<Certificate>) -> iced::Task<Message> {
  state.certificates = certs.into_iter().map(|c| (c.id, c)).collect();
  iced::Task::none()
}

fn update_close(state: &mut State) -> iced::Task<Message> {
  if state.dirty {
    state.confirm_close = true;
    iced::Task::none()
  } else {
    iced::window::close(state.window_id)
  }
}

fn update_entry_drag_end(state: &mut State) -> iced::Task<Message> {
  let drag_id = state.dragging_entry_id.take();
  let hover_id = state.drag_hover_entry_id.take();
  if let (Some(drag_id), Some(hover_id)) = (drag_id, hover_id)
    && drag_id != hover_id
    && let Some(from_idx) = state.entries.iter().position(|e| e.id == drag_id)
    && let Some(to_idx) = state.entries.iter().position(|e| e.id == hover_id)
  {
    let entry = state.entries.remove(from_idx);
    state.entries.insert(to_idx, entry);
    recompute(state);
    update_dirty(state);
  }
  iced::Task::none()
}

fn update_entry_drag_hover(state: &mut State, id: String) -> iced::Task<Message> {
  if state.dragging_entry_id.is_some() {
    state.drag_hover_entry_id = Some(id);
  }
  iced::Task::none()
}

fn update_entry_drag_start(state: &mut State, id: String) -> iced::Task<Message> {
  state.dragging_entry_id = Some(id);
  state.drag_hover_entry_id = None;
  iced::Task::none()
}

fn toggle_note_expanded(state: &mut State, id: String) {
  if state.note_expanded.as_deref() == Some(&id) {
    state.note_expanded = None;
  } else {
    state.note_expanded = Some(id.clone());
    if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id)
      && entry.note.is_none()
    {
      entry.note = Some(String::new());
    }
  }
}

fn update_entry_note(state: &mut State, id: String, note: String) -> iced::Task<Message> {
  if note.is_empty() {
    toggle_note_expanded(state, id);
  } else if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id) {
    state.note_expanded = Some(id.clone());
    entry.note = Some(note);
    update_dirty(state);
  }
  iced::Task::none()
}

fn update_entry_priority_changed(state: &mut State, id: String, priority: Priority) -> iced::Task<Message> {
  tracing::info!(
    "skill_plan: entry priority changed — id: {id}, priority: {priority:?}, plan: {}",
    state.plan_name
  );
  if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id) {
    entry.priority = priority;
    update_dirty(state);
  }
  iced::Task::none()
}

fn update_entry_removed(state: &mut State, id: String) -> iced::Task<Message> {
  tracing::info!("skill_plan: skill removed — id: {id}, plan: {}", state.plan_name);
  state.entries.retain(|e| e.id != id);
  recompute(state);
  update_dirty(state);
  iced::Task::none()
}

fn level_to_roman(level: u8) -> String {
  match level {
    1 => "I".to_string(),
    2 => "II".to_string(),
    3 => "III".to_string(),
    4 => "IV".to_string(),
    5 => "V".to_string(),
    n => n.to_string(),
  }
}

fn format_plan_line(skill_name: &str, to_level: u8) -> String {
  format!("{} {}", skill_name, level_to_roman(to_level))
}

fn update_export_clipboard(state: &mut State) -> iced::Task<Message> {
  tracing::info!("skill_plan: export to clipboard — plan: {}", state.plan_name);
  state.export_dropdown_open = false;
  let content = plan_lines_text(&state.entries);
  let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(content));
  iced::Task::none()
}

fn plan_lines_text(entries: &[PlanEntry]) -> String {
  entries
    .iter()
    .filter(|e| !e.auto)
    .map(|e| format_plan_line(&e.skill_name, e.to_level))
    .collect::<Vec<_>>()
    .join("\n")
}

fn update_export_dropdown_toggled(state: &mut State) -> iced::Task<Message> {
  state.export_dropdown_open = !state.export_dropdown_open;
  state.import_dropdown_open = false;
  iced::Task::none()
}

fn update_export_file(state: &mut State) -> iced::Task<Message> {
  state.export_dropdown_open = false;
  let name = state.plan_name.clone();
  let entries = state.entries.clone();
  iced::Task::perform(
    async move {
      let path = rfd::AsyncFileDialog::new()
        .set_title("Export skill plan")
        .set_file_name(format!("{}.txt", name))
        .add_filter("Text", &["txt"])
        .save_file()
        .await
        .map(|f| f.path().to_path_buf());
      let _ = entries;
      path
    },
    Message::ExportPathChosen,
  )
}

fn update_export_path_chosen(state: &State, path: std::path::PathBuf) -> iced::Task<Message> {
  tracing::info!("skill_plan: export to file — plan: {}, path: {path:?}", state.plan_name);
  let content = plan_lines_text(&state.entries);
  let _ = std::fs::write(&path, content);
  iced::Task::none()
}

fn update_implant_set(state: &mut State, set: ImplantSet) -> iced::Task<Message> {
  tracing::info!("skill_plan: implant set changed — {set:?}, plan: {}", state.plan_name);
  state.implant_set = set;
  state.implant_bonus = if set == ImplantSet::Current {
    ImplantBonus {
      charisma: state.current_effective_attrs.charisma - state.base_attrs.charisma,
      intelligence: state.current_effective_attrs.intelligence - state.base_attrs.intelligence,
      memory: state.current_effective_attrs.memory - state.base_attrs.memory,
      perception: state.current_effective_attrs.perception - state.base_attrs.perception,
      willpower: state.current_effective_attrs.willpower - state.base_attrs.willpower,
    }
  } else {
    implant_bonus_for_set(set, &ImplantBonus::default())
  };
  recompute(state);
  iced::Task::none()
}

fn update_implant_suggestions(state: &mut State) -> iced::Task<Message> {
  state.show_implant_suggestions = !state.show_implant_suggestions;
  if state.show_implant_suggestions {
    let weights = pair_weights(&state.entries, &state.eff, &state.skill_groups);
    state.implant_savings = compute_implant_savings(
      &weights,
      &state.base_attrs,
      &state.implant_bonus,
      state.computed.total_sec,
    );
  }
  iced::Task::none()
}

fn merge_parsed_into_state(state: &mut State, parsed: Vec<(String, u8)>) {
  if parsed.is_empty() {
    return;
  }
  let mut wishes = collect_wishes(&state.entries);
  for (skill, level) in &parsed {
    if !wishes.iter().any(|(n, l)| n == skill && l == level) {
      wishes.push((skill.clone(), *level));
    }
  }
  let wish_refs: Vec<(&str, u8)> = wishes.iter().map(|(n, l)| (n.as_str(), *l)).collect();
  let new_entries = expand_wishes(&wish_refs, &state.skill_groups);
  state.entries = merge_entries(new_entries, &state.entries);
  recompute(state);
  update_dirty(state);
}

fn update_import_clipboard(state: &mut State) -> iced::Task<Message> {
  tracing::info!("skill_plan: import from clipboard — plan: {}", state.plan_name);
  state.import_dropdown_open = false;
  let text = arboard::Clipboard::new()
    .and_then(|mut cb| cb.get_text())
    .unwrap_or_default();
  merge_parsed_into_state(state, parse_import_text(&text));
  iced::Task::none()
}

fn update_import_dropdown_toggled(state: &mut State) -> iced::Task<Message> {
  state.import_dropdown_open = !state.import_dropdown_open;
  state.export_dropdown_open = false;
  iced::Task::none()
}

fn update_import_file(state: &mut State) -> iced::Task<Message> {
  state.import_dropdown_open = false;
  iced::Task::perform(
    async move {
      rfd::AsyncFileDialog::new()
        .set_title("Import skill plan")
        .add_filter("Text", &["txt"])
        .pick_file()
        .await
        .map(|f| f.path().to_path_buf())
    },
    Message::ImportPathChosen,
  )
}

fn update_import_path_chosen(state: &mut State, path: std::path::PathBuf) -> iced::Task<Message> {
  tracing::info!(
    "skill_plan: import from file — plan: {}, path: {path:?}",
    state.plan_name
  );
  let text = std::fs::read_to_string(&path).unwrap_or_default();
  merge_parsed_into_state(state, parse_import_text(&text));
  iced::Task::none()
}

fn update_module_selected(state: &mut State, type_id: i32) -> iced::Task<Message> {
  tracing::info!(
    "skill_plan: module selected — type_id: {type_id}, plan: {}",
    state.plan_name
  );
  let Some(module) = state.picker_modules.iter().find(|m| m.id == type_id) else {
    return iced::Task::none();
  };
  merge_wishes_into_plan(state, skills_for_module(&module.skill_requirements));
  iced::Task::none()
}

fn update_modules_loaded(state: &mut State, modules: Vec<ItemTypeSummary>) -> iced::Task<Message> {
  state.picker_modules = modules;
  state.modules_loaded = true;
  iced::Task::none()
}

fn update_name_changed(state: &mut State, name: String) -> iced::Task<Message> {
  state.plan_name = name;
  update_dirty(state);
  iced::Task::none()
}

fn update_optimizer_completed(state: &mut State, result: Option<RemapResult>) -> iced::Task<Message> {
  tracing::debug!("skill_plan: remap optimizer completed — found: {}", result.is_some());
  state.optimizer_running = false;
  state.optimizer_result = result;
  iced::Task::none()
}

fn update_optimizer_request(state: &mut State) -> iced::Task<Message> {
  tracing::info!(
    "skill_plan: remap optimizer requested — plan: {}, character_id: {}",
    state.plan_name,
    state.character_id
  );
  state.show_remap = true;
  state.optimizer_running = true;
  let entries = state.entries.clone();
  let base = state.base_attrs.clone();
  let implant = state.implant_bonus.clone();
  let base_total = base.perception + base.memory + base.willpower + base.intelligence + base.charisma;
  let skill_groups = state.skill_groups.clone();
  iced::Task::perform(
    async move { optimize_remap(&entries, &base, base_total, &implant, &skill_groups) },
    Message::OptimizerCompleted,
  )
}

fn update_pane_drag(state: &mut State, x: f32) -> iced::Task<Message> {
  if let Some(edge) = state.dragging_pane {
    if state.last_drag_x != 0.0 {
      let delta = x - state.last_drag_x;
      match edge {
        PaneEdge::Picker => {
          state.picker_pane_width = (state.picker_pane_width + delta).clamp(160.0, 480.0);
        }
        PaneEdge::Summary => {
          state.summary_pane_width = (state.summary_pane_width - delta).clamp(260.0, 480.0);
        }
      }
    }
    state.last_drag_x = x;
  }
  iced::Task::none()
}

fn update_pane_drag_end(state: &mut State) -> iced::Task<Message> {
  state.dragging_pane = None;
  iced::Task::none()
}

fn update_pane_drag_start(state: &mut State, edge: PaneEdge) -> iced::Task<Message> {
  state.dragging_pane = Some(edge);
  state.last_drag_x = 0.0;
  iced::Task::none()
}

fn update_picker_group_toggled(state: &mut State, name: String) -> iced::Task<Message> {
  if state.picker_expanded_groups.contains(&name) {
    state.picker_expanded_groups.remove(&name);
  } else {
    state.picker_expanded_groups.insert(name);
  }
  iced::Task::none()
}

fn update_picker_cert_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::AllCertsLoaded(certs) => update_all_certs_loaded(state, certs),
    Message::CertificatesLoaded(certs) => update_certificates_loaded(state, certs),
    Message::CertProficiencyChanged(cert_id, prof) => update_cert_proficiency_changed(state, cert_id, prof),
    Message::CertSelected(cert_id, _name, prof) => update_cert_selected(state, cert_id, prof),
    _ => iced::Task::none(),
  }
}

fn update_picker_ship_module_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::ModuleSelected(type_id, _name) => update_module_selected(state, type_id),
    Message::ModulesLoaded(modules) => update_modules_loaded(state, modules),
    Message::ShipMasteryChanged(type_id, level) => update_ship_mastery_changed(state, type_id, level),
    Message::ShipSelected(type_id, _name, mastery) => update_ship_selected(state, type_id, mastery),
    Message::ShipsLoaded(ships) => update_ships_loaded(state, ships, services),
    _ => iced::Task::none(),
  }
}

fn update_picker_search_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::PickerTabChanged(tab) => update_picker_tab(state, tab, services),
    Message::SkillGroupsLoaded(groups) => update_skill_groups_loaded(state, groups),
    Message::SkillPicked(skill_name, level) => update_skill_picked(state, skill_name, level),
    _ => iced::Task::none(),
  }
}

fn update_picker_misc_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::PickerGroupToggled(name) => update_picker_group_toggled(state, name),
    Message::PickerSearchChanged(q) => update_picker_search(state, q),
    Message::PickerToggled => update_picker_toggled(state),
    msg => update_picker_search_messages(state, msg, services),
  }
}

fn update_picker_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::AllCertsLoaded(_)
    | Message::CertificatesLoaded(_)
    | Message::CertProficiencyChanged(_, _)
    | Message::CertSelected(_, _, _) => update_picker_cert_messages(state, message),
    Message::ModuleSelected(_, _)
    | Message::ModulesLoaded(_)
    | Message::ShipMasteryChanged(_, _)
    | Message::ShipSelected(_, _, _)
    | Message::ShipsLoaded(_) => update_picker_ship_module_messages(state, message, services),
    msg => update_picker_misc_messages(state, msg, services),
  }
}

fn update_picker_search(state: &mut State, q: String) -> iced::Task<Message> {
  match state.picker_tab {
    1 => state.picker_ship_search = q,
    2 => state.picker_module_search = q,
    _ => state.picker_search = q,
  }
  iced::Task::none()
}

fn update_picker_tab(state: &mut State, tab: usize, services: &Services) -> iced::Task<Message> {
  state.picker_tab = tab;
  if tab == 1 && !state.ships_loaded {
    let Some(db) = services.db.clone() else {
      return iced::Task::none();
    };
    return iced::Task::perform(
      async move { db.universe().item_types().find_ships("").await.unwrap_or_default() },
      Message::ShipsLoaded,
    );
  }
  if tab == 2 && !state.modules_loaded {
    let Some(db) = services.db.clone() else {
      return iced::Task::none();
    };
    return iced::Task::perform(
      async move { db.universe().item_types().find_modules("").await.unwrap_or_default() },
      Message::ModulesLoaded,
    );
  }
  if tab == 3 && !state.certs_loaded {
    let Some(db) = services.db.clone() else {
      return iced::Task::none();
    };
    return iced::Task::perform(
      async move { db.universe().certificates().find_all().await.unwrap_or_default() },
      Message::AllCertsLoaded,
    );
  }
  iced::Task::none()
}

fn update_picker_toggled(state: &mut State) -> iced::Task<Message> {
  state.picker_open = !state.picker_open;
  iced::Task::none()
}

fn update_plan_entry_drag_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::EntryDragEnd => update_entry_drag_end(state),
    Message::EntryDragHover(id) => update_entry_drag_hover(state, id),
    Message::EntryDragStart(id) => update_entry_drag_start(state, id),
    _ => iced::Task::none(),
  }
}

fn update_plan_loaded_message(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::PlanLoaded(None) => update_plan_loaded_none(state),
    Message::PlanLoaded(Some(plan)) => update_plan_loaded(state, plan),
    _ => iced::Task::none(),
  }
}

fn update_plan_entry_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::EntryDragEnd | Message::EntryDragHover(_) | Message::EntryDragStart(_) => {
      update_plan_entry_drag_messages(state, message)
    }
    Message::EntryNoteChanged(id, note) => update_entry_note(state, id, note),
    Message::EntryPriorityChanged(id, priority) => update_entry_priority_changed(state, id, priority),
    Message::EntryRemoved(id) => update_entry_removed(state, id),
    msg => update_plan_loaded_message(state, msg),
  }
}

fn update_plan_loaded(state: &mut State, plan: pod_model::SkillPlan) -> iced::Task<Message> {
  tracing::debug!("skill_plan: plan loaded — name: {}, id: {}", plan.name, plan.id);
  state.plan_name = plan.name.clone();
  state.plan_id = Some(plan.id.clone());
  state.entries = plan_entries_to_plan_entries(&plan);
  state.saved_snapshot = plan_snapshot(&state.plan_name, &state.entries);
  state.dirty = false;
  recompute(state);
  iced::Task::none()
}

fn update_plan_loaded_none(state: &mut State) -> iced::Task<Message> {
  tracing::debug!("skill_plan: no saved plan found — starting with empty plan");
  state.plan_name = "Untitled plan".to_string();
  state.entries = Vec::new();
  state.saved_snapshot = plan_snapshot(&state.plan_name, &[]);
  state.dirty = false;
  recompute(state);
  iced::Task::none()
}

fn update_plan_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::EntryDragEnd
    | Message::EntryDragHover(_)
    | Message::EntryDragStart(_)
    | Message::EntryNoteChanged(_, _)
    | Message::EntryPriorityChanged(_, _)
    | Message::EntryRemoved(_)
    | Message::PlanLoaded(_) => update_plan_entry_messages(state, message),
    _ => update_plan_ui_messages(state, message, services),
  }
}

fn is_export_message(msg: &Message) -> bool {
  matches!(
    msg,
    Message::ExportDropdownToggled | Message::ExportPathChosen(_) | Message::ExportToClipboard | Message::ExportToFile
  )
}

fn update_export_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::ExportDropdownToggled => update_export_dropdown_toggled(state),
    Message::ExportPathChosen(Some(path)) => update_export_path_chosen(state, path),
    Message::ExportToClipboard => update_export_clipboard(state),
    Message::ExportToFile => update_export_file(state),
    _ => iced::Task::none(),
  }
}

fn update_import_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::ImportDropdownToggled => update_import_dropdown_toggled(state),
    Message::ImportFromClipboard => update_import_clipboard(state),
    Message::ImportFromFile => update_import_file(state),
    Message::ImportPathChosen(Some(path)) => update_import_path_chosen(state, path),
    _ => iced::Task::none(),
  }
}

fn update_plan_import_export_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  if is_export_message(&message) {
    update_export_messages(state, message)
  } else {
    update_import_messages(state, message)
  }
}

fn update_plan_optimizer_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::ImplantSetChanged(set) => update_implant_set(state, set),
    Message::ImplantSuggestionsToggled => update_implant_suggestions(state),
    Message::OptimizerCompleted(result) => update_optimizer_completed(state, result),
    Message::OptimizerRequested => update_optimizer_request(state),
    _ => iced::Task::none(),
  }
}

fn update_plan_pane_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::PaneDrag(x) => update_pane_drag(state, x),
    Message::PaneDragEnd => update_pane_drag_end(state),
    Message::PaneDragStart(edge) => update_pane_drag_start(state, edge),
    _ => iced::Task::none(),
  }
}

fn is_import_export_message(msg: &Message) -> bool {
  matches!(
    msg,
    Message::ExportDropdownToggled
      | Message::ExportPathChosen(_)
      | Message::ExportToClipboard
      | Message::ExportToFile
      | Message::ImportDropdownToggled
      | Message::ImportFromClipboard
      | Message::ImportFromFile
      | Message::ImportPathChosen(_)
  )
}

fn is_optimizer_message(msg: &Message) -> bool {
  matches!(
    msg,
    Message::ImplantSetChanged(_)
      | Message::ImplantSuggestionsToggled
      | Message::OptimizerCompleted(_)
      | Message::OptimizerRequested
  )
}

fn is_pane_message(msg: &Message) -> bool {
  matches!(
    msg,
    Message::PaneDrag(_) | Message::PaneDragEnd | Message::PaneDragStart(_)
  )
}

fn update_plan_ui_lifecycle_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::SaveCompleted | Message::SaveRequested => update_plan_ui_save_messages(state, message, services),
    msg => update_plan_ui_window_messages(state, msg),
  }
}

fn update_plan_ui_save_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::SaveCompleted => update_save_completed(state),
    Message::SaveRequested => update_save(state, services),
    _ => iced::Task::none(),
  }
}

fn update_plan_ui_window_messages(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::AttrsLoaded {
      base_attrs,
      current_effective_attrs,
      clone_data_missing,
    } => update_attrs_loaded(state, base_attrs, current_effective_attrs, clone_data_missing),
    Message::CancelClose => update_cancel_close(state),
    Message::CloseRequested => update_close(state),
    Message::ConfirmClose => iced::window::close(state.window_id),
    Message::NameChanged(name) => update_name_changed(state, name),
    _ => iced::Task::none(),
  }
}

fn update_plan_ui_messages(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  if is_import_export_message(&message) {
    update_plan_import_export_messages(state, message)
  } else if is_optimizer_message(&message) {
    update_plan_optimizer_messages(state, message)
  } else if is_pane_message(&message) {
    update_plan_pane_messages(state, message)
  } else {
    update_plan_ui_lifecycle_messages(state, message, services)
  }
}

fn update_save(state: &mut State, services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    tracing::warn!(
      "skill_plan: save requested but no database available — plan: {}",
      state.plan_name
    );
    return iced::Task::none();
  };
  tracing::info!(
    "skill_plan: save requested — plan: {}, {} entries",
    state.plan_name,
    state.entries.len()
  );
  let plan = state_to_skill_plan(state);
  if state.plan_id.is_none() {
    state.plan_id = Some(plan.id.clone());
  }
  iced::Task::perform(
    async move {
      let repo = db.skill_plans();
      let _ = repo.create(&plan).await;
    },
    |_| Message::SaveCompleted,
  )
}

fn update_save_completed(state: &mut State) -> iced::Task<Message> {
  tracing::info!("skill_plan: save completed — plan: {}", state.plan_name);
  state.saved_snapshot = plan_snapshot(&state.plan_name, &state.entries);
  state.dirty = false;
  iced::Task::none()
}

fn update_ship_mastery_changed(state: &mut State, type_id: i32, level: u8) -> iced::Task<Message> {
  state.ship_mastery_selection.insert(type_id, level);
  iced::Task::none()
}

fn build_type_id_to_name(state: &State) -> HashMap<i32, String> {
  state
    .skill_groups
    .iter()
    .flat_map(|g| g.skills.iter())
    .map(|s| (s.type_id, s.name.clone()))
    .collect()
}

fn resolve_ship_skill_wishes(
  ship: &ItemTypeSummary,
  mastery: u8,
  certificates: &HashMap<i32, Certificate>,
  type_id_to_name: &HashMap<i32, String>,
) -> Vec<(String, u8)> {
  let lookup = |tid: i32| type_id_to_name.get(&tid).cloned();
  let has_cert_data = ship.mastery_cert_ids.iter().any(|v| !v.is_empty()) && !certificates.is_empty();
  let cert_wishes = if has_cert_data {
    skills_for_mastery(&ship.mastery_cert_ids, mastery, certificates, &lookup)
  } else {
    vec![]
  };
  if cert_wishes.is_empty() {
    skills_for_module(&ship.skill_requirements)
  } else {
    cert_wishes
  }
}

fn update_ship_selected(state: &mut State, type_id: i32, mastery: u8) -> iced::Task<Message> {
  tracing::info!(
    "skill_plan: ship selected — type_id: {type_id}, mastery: {mastery}, plan: {}",
    state.plan_name
  );
  let Some(ship) = state.picker_ships.iter().find(|s| s.id == type_id).cloned() else {
    return iced::Task::none();
  };
  let type_id_to_name = build_type_id_to_name(state);
  let skill_wishes = resolve_ship_skill_wishes(&ship, mastery, &state.certificates, &type_id_to_name);
  merge_wishes_into_plan(state, skill_wishes);
  iced::Task::none()
}

fn update_ships_loaded(state: &mut State, ships: Vec<ItemTypeSummary>, services: &Services) -> iced::Task<Message> {
  let cert_ids: Vec<i32> = ships
    .iter()
    .flat_map(|s| s.mastery_cert_ids.iter().flat_map(|v| v.iter().copied()))
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();
  state.picker_ships = ships;
  state.ships_loaded = true;
  if cert_ids.is_empty() {
    return iced::Task::none();
  }
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move {
      db.universe()
        .certificates()
        .find_by_ids(&cert_ids)
        .await
        .unwrap_or_default()
    },
    Message::CertificatesLoaded,
  )
}

fn update_skill_groups_loaded(state: &mut State, groups: Vec<SkillGroupDef>) -> iced::Task<Message> {
  state.skill_groups = groups;
  if let Some(items) = state.pending_from_queue.take() {
    let wish_refs: Vec<(&str, u8)> = items.iter().map(|(n, l)| (n.as_str(), *l)).collect();
    let entries = expand_wishes(&wish_refs, &state.skill_groups);
    state.entries = entries;
    state.saved_snapshot = plan_snapshot(&state.plan_name, &state.entries);
  }
  recompute(state);
  iced::Task::none()
}

fn update_skill_picked(state: &mut State, skill_name: String, level: u8) -> iced::Task<Message> {
  tracing::info!(
    "skill_plan: skill added — {skill_name} level {level}, plan: {}",
    state.plan_name
  );
  let wishes = collect_wishes(&state.entries);
  let mut new_wishes = wishes.clone();
  if !new_wishes.iter().any(|(n, l)| *n == skill_name && *l == level) {
    new_wishes.push((skill_name, level));
  }
  let wish_refs: Vec<(&str, u8)> = new_wishes.iter().map(|(n, l)| (n.as_str(), *l)).collect();
  let new_entries = expand_wishes(&wish_refs, &state.skill_groups);
  state.entries = merge_entries(new_entries, &state.entries);
  recompute(state);
  update_dirty(state);
  iced::Task::none()
}

#[cfg(test)]
mod tests {
  use pod_model::{SkillPlan, SkillPlanEntry};

  use super::*;

  fn make_entry(id: &str, skill: &str, level: u8, auto: bool) -> PlanEntry {
    PlanEntry {
      id: id.to_string(),
      skill_name: skill.to_string(),
      to_level: level,
      priority: Priority::Normal,
      note: None,
      auto,
    }
  }

  mod parse_import_text {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_roman_numeral_levels() {
      let input = "Navigation V\nGunnery III\nShielding II\nSpaceship Command I\nWarp Drive Operation IV";
      let result = parse_import_text(input);

      assert_eq!(result.len(), 5);
      assert!(result.contains(&("Navigation".to_string(), 5)));
      assert!(result.contains(&("Gunnery".to_string(), 3)));
      assert!(result.contains(&("Shielding".to_string(), 2)));
      assert!(result.contains(&("Spaceship Command".to_string(), 1)));
      assert!(result.contains(&("Warp Drive Operation".to_string(), 4)));
    }

    #[test]
    fn it_parses_numeric_levels() {
      let input = "Navigation 5\nGunnery 3";
      let result = parse_import_text(input);

      assert_eq!(result.len(), 2);
      assert!(result.contains(&("Navigation".to_string(), 5)));
      assert!(result.contains(&("Gunnery".to_string(), 3)));
    }

    #[test]
    fn it_skips_empty_lines() {
      let input = "Navigation V\n\n\nGunnery III\n";
      let result = parse_import_text(input);

      assert_eq!(result.len(), 2);
    }

    #[test]
    fn it_returns_empty_for_blank_input() {
      let result = parse_import_text("   \n\n  ");

      assert!(result.is_empty());
    }

    #[test]
    fn it_rejects_numeric_level_out_of_range() {
      let input = "Navigation 6\nGunnery 0";
      let result = parse_import_text(input);

      assert!(result.is_empty());
    }

    #[test]
    fn it_handles_multi_word_skill_names() {
      let input = "Caldari Battleship V";
      let result = parse_import_text(input);

      assert_eq!(result, vec![("Caldari Battleship".to_string(), 5)]);
    }

    #[test]
    fn it_prefers_longer_roman_match_before_shorter() {
      let input = "Bomb Deployment IV";
      let result = parse_import_text(input);

      assert_eq!(result, vec![("Bomb Deployment".to_string(), 4)]);
    }
  }

  mod plan_snapshot {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_produces_just_name_for_empty_entries() {
      let snap = plan_snapshot("My Plan", &[]);

      assert_eq!(snap, "My Plan");
    }

    #[test]
    fn it_includes_entry_fields() {
      let entries = vec![make_entry("nav-1", "Navigation", 1, false)];
      let snap = plan_snapshot("My Plan", &entries);

      assert!(snap.contains("Navigation"));
      assert!(snap.contains('1'.to_string().as_str()));
    }

    #[test]
    fn different_names_produce_different_snapshots() {
      let snap_a = plan_snapshot("Plan A", &[]);
      let snap_b = plan_snapshot("Plan B", &[]);

      assert_ne!(snap_a, snap_b);
    }

    #[test]
    fn different_entries_produce_different_snapshots() {
      let a = vec![make_entry("nav-1", "Navigation", 1, false)];
      let b = vec![make_entry("nav-2", "Navigation", 2, false)];

      assert_ne!(plan_snapshot("Plan", &a), plan_snapshot("Plan", &b));
    }
  }

  mod collect_wishes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_non_auto_entries() {
      let entries = vec![
        make_entry("nav-1", "Navigation", 1, false),
        make_entry("gun-3", "Gunnery", 3, false),
      ];
      let wishes = collect_wishes(&entries);

      assert_eq!(wishes.len(), 2);
      assert!(wishes.iter().any(|(n, l)| n == "Navigation" && *l == 1));
      assert!(wishes.iter().any(|(n, l)| n == "Gunnery" && *l == 3));
    }

    #[test]
    fn it_excludes_auto_entries() {
      let entries = vec![
        make_entry("nav-1", "Navigation", 1, true),
        make_entry("gun-3", "Gunnery", 3, false),
      ];
      let wishes = collect_wishes(&entries);

      assert_eq!(wishes.len(), 1);
      assert!(wishes.iter().any(|(n, _)| n == "Gunnery"));
    }

    #[test]
    fn it_keeps_highest_level_per_skill() {
      let entries = vec![
        make_entry("nav-2", "Navigation", 2, false),
        make_entry("nav-4", "Navigation", 4, false),
        make_entry("nav-1", "Navigation", 1, false),
      ];
      let wishes = collect_wishes(&entries);

      assert_eq!(wishes.len(), 1);
      assert_eq!(wishes[0], ("Navigation".to_string(), 4));
    }

    #[test]
    fn it_returns_empty_for_all_auto_entries() {
      let entries = vec![
        make_entry("nav-1", "Navigation", 1, true),
        make_entry("gun-3", "Gunnery", 3, true),
      ];
      let wishes = collect_wishes(&entries);

      assert!(wishes.is_empty());
    }
  }

  mod merge_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_preserves_priority_from_old_entry() {
      let mut old = make_entry("nav-1", "Navigation", 1, false);
      old.priority = Priority::High;
      let new = make_entry("nav-1", "Navigation", 1, false);

      let merged = merge_entries(vec![new], &[old]);

      assert_eq!(merged.len(), 1);
      assert!(matches!(merged[0].priority, Priority::High));
    }

    #[test]
    fn it_preserves_note_from_old_entry() {
      let mut old = make_entry("nav-1", "Navigation", 1, false);
      old.note = Some("important".to_string());
      let new = make_entry("nav-1", "Navigation", 1, false);

      let merged = merge_entries(vec![new], &[old]);

      assert_eq!(merged[0].note, Some("important".to_string()));
    }

    #[test]
    fn it_uses_new_defaults_when_no_old_entry_exists() {
      let new = make_entry("nav-1", "Navigation", 1, false);

      let merged = merge_entries(vec![new], &[]);

      assert_eq!(merged.len(), 1);
      assert!(matches!(merged[0].priority, Priority::Normal));
      assert!(merged[0].note.is_none());
    }

    #[test]
    fn it_maintains_order_from_new_entries() {
      let new_entries = vec![
        make_entry("gun-1", "Gunnery", 1, false),
        make_entry("nav-1", "Navigation", 1, false),
        make_entry("shd-1", "Shielding", 1, false),
      ];

      let merged = merge_entries(new_entries, &[]);

      assert_eq!(merged[0].skill_name, "Gunnery");
      assert_eq!(merged[1].skill_name, "Navigation");
      assert_eq!(merged[2].skill_name, "Shielding");
    }
  }

  mod plan_entries_to_plan_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_plan_entry(id: &str, skill: &str, level: i32, priority: &str, auto: bool) -> SkillPlanEntry {
      SkillPlanEntry {
        auto,
        id: id.to_string(),
        note: None,
        plan_id: "plan-001".to_string(),
        position: 0,
        priority: priority.to_string(),
        skill_name: skill.to_string(),
        to_level: level,
      }
    }

    #[test]
    fn it_converts_priority_low() {
      let plan = SkillPlan {
        character_id: 1,
        created_at: 0,
        entries: vec![make_plan_entry("e1", "Navigation", 1, "low", false)],
        id: "plan-001".to_string(),
        implant_set: "none".to_string(),
        name: "Test".to_string(),
        remap_json: None,
        updated_at: 0,
      };

      let entries = plan_entries_to_plan_entries(&plan);

      assert_eq!(entries.len(), 1);
      assert!(matches!(entries[0].priority, Priority::Low));
    }

    #[test]
    fn it_converts_priority_high() {
      let plan = SkillPlan {
        character_id: 1,
        created_at: 0,
        entries: vec![make_plan_entry("e1", "Navigation", 1, "high", false)],
        id: "plan-001".to_string(),
        implant_set: "none".to_string(),
        name: "Test".to_string(),
        remap_json: None,
        updated_at: 0,
      };

      let entries = plan_entries_to_plan_entries(&plan);

      assert!(matches!(entries[0].priority, Priority::High));
    }

    #[test]
    fn it_defaults_unknown_priority_to_normal() {
      let plan = SkillPlan {
        character_id: 1,
        created_at: 0,
        entries: vec![make_plan_entry("e1", "Navigation", 1, "unknown", false)],
        id: "plan-001".to_string(),
        implant_set: "none".to_string(),
        name: "Test".to_string(),
        remap_json: None,
        updated_at: 0,
      };

      let entries = plan_entries_to_plan_entries(&plan);

      assert!(matches!(entries[0].priority, Priority::Normal));
    }

    #[test]
    fn it_preserves_auto_flag() {
      let plan = SkillPlan {
        character_id: 1,
        created_at: 0,
        entries: vec![
          make_plan_entry("e1", "Navigation", 1, "normal", false),
          make_plan_entry("e2", "Spaceship Command", 1, "normal", true),
        ],
        id: "plan-001".to_string(),
        implant_set: "none".to_string(),
        name: "Test".to_string(),
        remap_json: None,
        updated_at: 0,
      };

      let entries = plan_entries_to_plan_entries(&plan);

      assert!(!entries[0].auto);
      assert!(entries[1].auto);
    }
  }

  mod is_export_message {
    use super::*;

    #[test]
    fn it_returns_true_for_export_dropdown_toggled() {
      assert!(is_export_message(&Message::ExportDropdownToggled));
    }

    #[test]
    fn it_returns_true_for_export_path_chosen_none() {
      assert!(is_export_message(&Message::ExportPathChosen(None)));
    }

    #[test]
    fn it_returns_true_for_export_to_clipboard() {
      assert!(is_export_message(&Message::ExportToClipboard));
    }

    #[test]
    fn it_returns_true_for_export_to_file() {
      assert!(is_export_message(&Message::ExportToFile));
    }

    #[test]
    fn it_returns_false_for_import_messages() {
      assert!(!is_export_message(&Message::ImportDropdownToggled));
    }

    #[test]
    fn it_returns_false_for_unrelated_messages() {
      assert!(!is_export_message(&Message::PickerToggled));
    }
  }

  mod is_import_export_message {
    use super::*;

    #[test]
    fn it_returns_true_for_export_messages() {
      assert!(is_import_export_message(&Message::ExportDropdownToggled));
      assert!(is_import_export_message(&Message::ExportToClipboard));
      assert!(is_import_export_message(&Message::ExportToFile));
    }

    #[test]
    fn it_returns_true_for_import_messages() {
      assert!(is_import_export_message(&Message::ImportDropdownToggled));
      assert!(is_import_export_message(&Message::ImportFromClipboard));
      assert!(is_import_export_message(&Message::ImportFromFile));
    }

    #[test]
    fn it_returns_false_for_non_import_export_messages() {
      assert!(!is_import_export_message(&Message::PickerToggled));
      assert!(!is_import_export_message(&Message::PaneDragEnd));
    }
  }

  mod is_optimizer_message {
    use super::*;

    #[test]
    fn it_returns_true_for_implant_suggestions_toggled() {
      assert!(is_optimizer_message(&Message::ImplantSuggestionsToggled));
    }

    #[test]
    fn it_returns_true_for_optimizer_requested() {
      assert!(is_optimizer_message(&Message::OptimizerRequested));
    }

    #[test]
    fn it_returns_false_for_non_optimizer_messages() {
      assert!(!is_optimizer_message(&Message::PickerToggled));
      assert!(!is_optimizer_message(&Message::PaneDragEnd));
    }
  }

  mod is_pane_message {
    use super::*;

    #[test]
    fn it_returns_true_for_pane_drag_end() {
      assert!(is_pane_message(&Message::PaneDragEnd));
    }

    #[test]
    fn it_returns_false_for_non_pane_messages() {
      assert!(!is_pane_message(&Message::PickerToggled));
      assert!(!is_pane_message(&Message::ExportDropdownToggled));
    }
  }
}
