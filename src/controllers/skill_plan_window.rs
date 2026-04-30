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

  match seed {
    PlanSeed::Existing(plan_id) => {
      let state = State {
        window_id,
        character_id,
        plan_id: Some(plan_id.clone()),
        plan_name: "Loading\u{2026}".to_string(),
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
        base_attrs,
        current_effective_attrs,
        clone_data_missing,
        implant_bonus,
        remap_cooldown_days: 0,
        remap_available: true,
        bonus_remaps: 0,
        computed: ComputedPlan::default(),
        eff: eff.clone(),
        implant_savings: Vec::new(),
        planned_levels: HashMap::new(),
        picker_pane_width,
        summary_pane_width,
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
      };
      let plan_task = if let Some(db) = db.clone() {
        iced::Task::perform(
          async move { db.skill_plans().find(&plan_id).await.ok().flatten() },
          Message::PlanLoaded,
        )
      } else {
        iced::Task::perform(async { None }, Message::PlanLoaded)
      };
      let groups_task = if let Some(db) = db {
        iced::Task::perform(
          async move { db.universe().item_types().find_skill_groups().await.unwrap_or_default() },
          Message::SkillGroupsLoaded,
        )
      } else {
        iced::Task::none()
      };
      (state, iced::Task::batch([plan_task, groups_task]))
    }
    PlanSeed::FromQueue(items) => {
      let plan_name = "Plan from queue".to_string();
      let snapshot = plan_snapshot(&plan_name, &[]);
      let mut state = State {
        window_id,
        character_id,
        plan_id: None,
        plan_name,
        entries: Vec::new(),
        picker_open: false,
        picker_search: String::new(),
        picker_expanded_groups: HashSet::new(),
        dirty: false,
        saved_snapshot: snapshot,
        implant_set: ImplantSet::None,
        optimizer_result: None,
        optimizer_running: false,
        show_remap: false,
        show_implant_suggestions: false,
        import_dropdown_open: false,
        export_dropdown_open: false,
        confirm_close: false,
        note_expanded: None,
        base_attrs,
        current_effective_attrs,
        clone_data_missing,
        implant_bonus,
        remap_cooldown_days: 0,
        remap_available: true,
        bonus_remaps: 0,
        computed: ComputedPlan::default(),
        eff,
        implant_savings: Vec::new(),
        planned_levels: HashMap::new(),
        picker_pane_width,
        summary_pane_width,
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
        pending_from_queue: Some(items),
      };
      recompute(&mut state);
      let task = if let Some(db) = db {
        iced::Task::perform(
          async move { db.universe().item_types().find_skill_groups().await.unwrap_or_default() },
          Message::SkillGroupsLoaded,
        )
      } else {
        iced::Task::none()
      };
      (state, task)
    }
    PlanSeed::New => {
      let state = State {
        window_id,
        character_id,
        plan_id: None,
        plan_name: "Untitled plan".to_string(),
        entries: Vec::new(),
        picker_open: true,
        picker_search: String::new(),
        picker_expanded_groups: HashSet::new(),
        dirty: false,
        saved_snapshot: plan_snapshot("Untitled plan", &[]),
        implant_set: ImplantSet::None,
        optimizer_result: None,
        optimizer_running: false,
        show_remap: false,
        show_implant_suggestions: false,
        import_dropdown_open: false,
        export_dropdown_open: false,
        confirm_close: false,
        note_expanded: None,
        base_attrs,
        current_effective_attrs,
        clone_data_missing,
        implant_bonus,
        remap_cooldown_days: 0,
        remap_available: true,
        bonus_remaps: 0,
        computed: ComputedPlan::default(),
        eff,
        implant_savings: Vec::new(),
        planned_levels: HashMap::new(),
        picker_pane_width,
        summary_pane_width,
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
      };
      let task = if let Some(db) = db {
        iced::Task::perform(
          async move { db.universe().item_types().find_skill_groups().await.unwrap_or_default() },
          Message::SkillGroupsLoaded,
        )
      } else {
        iced::Task::none()
      };
      (state, task)
    }
  }
}

/// Processes a skill plan window message and returns a task.
pub fn update(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::SkillPicked(skill_name, level) => {
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

    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      iced::Task::none()
    }

    Message::PickerSearchChanged(q) => {
      match state.picker_tab {
        1 => state.picker_ship_search = q,
        2 => state.picker_module_search = q,
        _ => state.picker_search = q,
      }
      iced::Task::none()
    }

    Message::PickerGroupToggled(name) => {
      if state.picker_expanded_groups.contains(&name) {
        state.picker_expanded_groups.remove(&name);
      } else {
        state.picker_expanded_groups.insert(name);
      }
      iced::Task::none()
    }

    Message::EntryRemoved(id) => {
      state.entries.retain(|e| e.id != id);
      recompute(state);
      update_dirty(state);
      iced::Task::none()
    }

    Message::EntryDragStart(id) => {
      state.dragging_entry_id = Some(id);
      state.drag_hover_entry_id = None;
      iced::Task::none()
    }

    Message::EntryDragHover(id) => {
      if state.dragging_entry_id.is_some() {
        state.drag_hover_entry_id = Some(id);
      }
      iced::Task::none()
    }

    Message::EntryDragEnd => {
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

    Message::PaneDragStart(edge) => {
      state.dragging_pane = Some(edge);
      state.last_drag_x = 0.0;
      iced::Task::none()
    }

    Message::PaneDrag(x) => {
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

    Message::PaneDragEnd => {
      state.dragging_pane = None;
      iced::Task::none()
    }

    Message::EntryPriorityChanged(id, priority) => {
      if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id) {
        entry.priority = priority;
        update_dirty(state);
      }
      iced::Task::none()
    }

    Message::EntryNoteChanged(id, note) => {
      if note.is_empty() {
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
      } else if let Some(entry) = state.entries.iter_mut().find(|e| e.id == id) {
        state.note_expanded = Some(id.clone());
        entry.note = Some(note);
        update_dirty(state);
      }
      iced::Task::none()
    }

    Message::NameChanged(name) => {
      state.plan_name = name;
      update_dirty(state);
      iced::Task::none()
    }

    Message::SaveRequested => {
      let Some(db) = services.db.clone() else {
        return iced::Task::none();
      };
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

    Message::SaveCompleted => {
      state.saved_snapshot = plan_snapshot(&state.plan_name, &state.entries);
      state.dirty = false;
      iced::Task::none()
    }

    Message::OptimizerRequested => {
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

    Message::OptimizerCompleted(result) => {
      state.optimizer_running = false;
      state.optimizer_result = result;
      iced::Task::none()
    }

    Message::ImplantSetChanged(set) => {
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

    Message::ImplantSuggestionsToggled => {
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

    Message::ImportDropdownToggled => {
      state.import_dropdown_open = !state.import_dropdown_open;
      state.export_dropdown_open = false;
      iced::Task::none()
    }

    Message::ExportDropdownToggled => {
      state.export_dropdown_open = !state.export_dropdown_open;
      state.import_dropdown_open = false;
      iced::Task::none()
    }

    Message::ImportFromClipboard => {
      state.import_dropdown_open = false;
      let text = arboard::Clipboard::new()
        .and_then(|mut cb| cb.get_text())
        .unwrap_or_default();
      let parsed = parse_import_text(&text);
      if !parsed.is_empty() {
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
      iced::Task::none()
    }

    Message::ImportFromFile => {
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

    Message::ImportPathChosen(Some(path)) => {
      let text = std::fs::read_to_string(&path).unwrap_or_default();
      let parsed = parse_import_text(&text);
      if !parsed.is_empty() {
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
      iced::Task::none()
    }

    Message::ImportPathChosen(None) => iced::Task::none(),

    Message::ExportToClipboard => {
      state.export_dropdown_open = false;
      let lines: Vec<String> = state
        .entries
        .iter()
        .filter(|e| !e.auto)
        .map(|e| {
          let level_str = match e.to_level {
            1 => "I",
            2 => "II",
            3 => "III",
            4 => "IV",
            5 => "V",
            n => return format!("{} {}", e.skill_name, n),
          };
          format!("{} {}", e.skill_name, level_str)
        })
        .collect();
      let content = lines.join("\n");
      let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(content));
      iced::Task::none()
    }

    Message::ExportToFile => {
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

    Message::ExportPathChosen(Some(path)) => {
      let lines: Vec<String> = state
        .entries
        .iter()
        .filter(|e| !e.auto)
        .map(|e| {
          let level_str = match e.to_level {
            1 => "I",
            2 => "II",
            3 => "III",
            4 => "IV",
            5 => "V",
            n => return format!("{} {}", e.skill_name, n),
          };
          format!("{} {}", e.skill_name, level_str)
        })
        .collect();
      let content = lines.join("\n");
      let _ = std::fs::write(&path, content);
      iced::Task::none()
    }

    Message::ExportPathChosen(None) => iced::Task::none(),

    Message::CloseRequested => {
      if state.dirty {
        state.confirm_close = true;
        iced::Task::none()
      } else {
        iced::window::close(state.window_id)
      }
    }

    Message::ConfirmClose => iced::window::close(state.window_id),

    Message::CancelClose => {
      state.confirm_close = false;
      iced::Task::none()
    }

    Message::PlanLoaded(Some(plan)) => {
      state.plan_name = plan.name.clone();
      state.plan_id = Some(plan.id.clone());
      state.entries = plan_entries_to_plan_entries(&plan);
      state.saved_snapshot = plan_snapshot(&state.plan_name, &state.entries);
      state.dirty = false;
      recompute(state);
      iced::Task::none()
    }

    Message::PlanLoaded(None) => {
      state.plan_name = "Untitled plan".to_string();
      state.entries = Vec::new();
      state.saved_snapshot = plan_snapshot(&state.plan_name, &[]);
      state.dirty = false;
      recompute(state);
      iced::Task::none()
    }

    Message::PickerTabChanged(tab) => {
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

    Message::ShipsLoaded(ships) => {
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

    Message::ModulesLoaded(modules) => {
      state.picker_modules = modules;
      state.modules_loaded = true;
      iced::Task::none()
    }

    Message::CertificatesLoaded(certs) => {
      state.certificates = certs.into_iter().map(|c| (c.id, c)).collect();
      iced::Task::none()
    }

    Message::SkillGroupsLoaded(groups) => {
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

    Message::ShipMasteryChanged(type_id, level) => {
      state.ship_mastery_selection.insert(type_id, level);
      iced::Task::none()
    }

    Message::ShipSelected(type_id, _ship_name, mastery) => {
      let type_id_to_name: HashMap<i32, String> = state
        .skill_groups
        .iter()
        .flat_map(|g| g.skills.iter())
        .map(|s| (s.type_id, s.name.clone()))
        .collect();
      let lookup = |tid: i32| type_id_to_name.get(&tid).cloned();
      let Some(ship) = state.picker_ships.iter().find(|s| s.id == type_id) else {
        return iced::Task::none();
      };
      let has_cert_data = ship.mastery_cert_ids.iter().any(|v| !v.is_empty()) && !state.certificates.is_empty();
      let mut skill_wishes = if has_cert_data {
        skills_for_mastery(&ship.mastery_cert_ids, mastery, &state.certificates, &lookup)
      } else {
        vec![]
      };
      if skill_wishes.is_empty() {
        skill_wishes = skills_for_module(&ship.skill_requirements);
      }
      merge_wishes_into_plan(state, skill_wishes);
      iced::Task::none()
    }

    Message::ModuleSelected(type_id, _module_name) => {
      let Some(module) = state.picker_modules.iter().find(|m| m.id == type_id) else {
        return iced::Task::none();
      };
      merge_wishes_into_plan(state, skills_for_module(&module.skill_requirements));
      iced::Task::none()
    }

    Message::AllCertsLoaded(certs) => {
      state.picker_certs = certs;
      state.certs_loaded = true;
      iced::Task::none()
    }

    Message::CertProficiencyChanged(cert_id, prof) => {
      state.cert_proficiency_selection.insert(cert_id, prof);
      iced::Task::none()
    }

    Message::CertSelected(cert_id, _name, prof) => {
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

    Message::AttrsLoaded {
      base_attrs,
      current_effective_attrs,
      clone_data_missing,
    } => {
      state.base_attrs = base_attrs;
      state.current_effective_attrs = current_effective_attrs;
      state.clone_data_missing = clone_data_missing;
      state.implant_bonus = ImplantBonus::default();
      state.implant_set = ImplantSet::None;
      recompute(state);
      iced::Task::none()
    }
  }
}

/// Returns background subscriptions for the skill plan window.
pub fn subscription(state: &State) -> iced::Subscription<Message> {
  use iced::{
    event::{self, Event},
    mouse,
  };

  if state.dragging_pane.is_some() {
    return event::listen_with(|event, _status, _id| match event {
      Event::Mouse(mouse::Event::CursorMoved {
        position,
      }) => Some(Message::PaneDrag(position.x)),
      Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::PaneDragEnd),
      _ => None,
    });
  }

  if state.dragging_entry_id.is_some() {
    return event::listen_with(|event, _status, _id| match event {
      Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::EntryDragEnd),
      _ => None,
    });
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

  let picker_col: Option<iced::Element<'_, Message>> = if state.picker_open {
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
  } else {
    None
  };

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

  let base_view: Element<'_, Message> = container(main_row)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into();

  let base_view = if state.dragging_pane.is_some() {
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
  } else {
    base_view
  };

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

  let discard_btn = button(
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
  });

  let cancel_btn = button(
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
  });

  let btn_row = row([
    Space::new().width(Length::Fill).into(),
    cancel_btn.into(),
    Space::new().width(spacing::SPACE_2).into(),
    discard_btn.into(),
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
