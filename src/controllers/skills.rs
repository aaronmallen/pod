//! Skills controller: queue state construction and message dispatch.

use std::collections::{HashMap, HashSet};

use iced::{Subscription, widget::image};
use pod_model::{AttrKey, Character, TrainingQueueEntry};
use pod_ui::{
  components::{
    CharacterPicker,
    character_picker::{self, CharacterEntry, PickerSelection},
  },
  format::{sp_cost, sp_per_sec},
  views::skills::{self, ComputedQueueItem, Message, QueueItem, RightTab, State, right_panel, skill_data::find_skill},
};

use crate::services::Services;

/// Creates the initial skills state from the given characters.
pub fn new(characters: Vec<Character>, left_pane_width: f32, services: &Services) -> (State, iced::Task<Message>) {
  let selected_char_id = characters.first().map(|c| *c.id()).unwrap_or(0);
  let char_skill_map = build_char_skill_map(&characters, selected_char_id);
  let queue = characters
    .iter()
    .find(|c| *c.id() == selected_char_id)
    .map(build_queue_from_character)
    .unwrap_or_default();
  let picker = build_skills_picker(&characters, selected_char_id);
  let mut state = build_initial_skills_state(characters, left_pane_width, picker, queue, char_skill_map);
  recompute_queue(&mut state);
  let task = build_skill_groups_task(services);
  (state, task)
}

/// Dispatches a skills message, rebuilding queue and skill map on character selection.
pub fn update(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  let was_char_switch = is_character_switch(&message);
  if was_char_switch {
    apply_char_switch(state, &message);
  }

  match &message {
    Message::PlansTabOpened => return handle_plans_tab_opened(state, message, services),
    Message::PlanDeleteConfirmed(_) => {
      return handle_plan_delete_confirmed(state, message, services);
    }
    Message::RightPanel(right_panel::Message::PlansTab(_)) => {
      return handle_right_panel_plans_tab(state, message);
    }
    Message::RightPanel(right_panel::Message::TabSelected(RightTab::Plans)) => {
      let _ = skills::update(state, message);
      recompute_queue(state);
      return iced::Task::done(Message::PlansTabOpened);
    }
    _ => {}
  }

  let base_task = skills::update(state, message);
  recompute_queue(state);
  if was_char_switch && state.right_tab == RightTab::Plans {
    iced::Task::batch([base_task, iced::Task::done(Message::PlansTabOpened)])
  } else {
    base_task
  }
}

/// Updates characters in the skills view and rebuilds the queue.
/// Call this when characters are refreshed externally (e.g., after ESI skill queue sync).
pub fn refresh_characters(state: &mut State, characters: Vec<Character>) {
  state.characters = characters;
  let char_id = state.selected_char_id();
  state.picker = build_skills_picker(&state.characters, char_id);
  state.queue = state
    .characters
    .iter()
    .find(|c| *c.id() == char_id)
    .map(build_queue_from_character)
    .unwrap_or_default();
  state.char_skill_map = build_char_skill_map(&state.characters, char_id);
  recompute_queue(state);
}

/// Returns background subscriptions for the skills view.
pub fn subscription(state: &State) -> Subscription<Message> {
  skills::subscription(state)
}

fn is_character_switch(message: &Message) -> bool {
  matches!(
    message,
    Message::Picker(character_picker::Message::Select(PickerSelection::Character(_)))
  )
}

fn apply_char_switch(state: &mut State, message: &Message) {
  let Message::Picker(character_picker::Message::Select(PickerSelection::Character(id))) = message else {
    return;
  };
  let id = *id;
  tracing::info!("skills: character switched — character_id: {id}");
  state.queue = state
    .characters
    .iter()
    .find(|c| *c.id() == id)
    .map(build_queue_from_character)
    .unwrap_or_default();
  state.char_skill_map = build_char_skill_map(&state.characters, id);
  state.plans = Vec::new();
  state.plans_loaded = false;
  state.confirm_delete_plan_id = None;
}

fn handle_plans_tab_opened(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  let char_id = state.selected_char_id();
  tracing::debug!("skills: plans tab opened — character_id: {char_id}");
  if let Some(db) = services.db.clone() {
    let task = iced::Task::perform(
      async move { db.skill_plans().all_for_character(char_id).await.unwrap_or_default() },
      Message::PlansLoaded,
    );
    let _ = skills::update(state, message);
    recompute_queue(state);
    return task;
  }
  iced::Task::none()
}

fn handle_plan_delete_confirmed(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  let Message::PlanDeleteConfirmed(plan_id) = &message else {
    return iced::Task::none();
  };
  let id = plan_id.clone();
  tracing::info!("skills: plan delete confirmed — plan_id: {id}");
  state.confirm_delete_plan_id = None;
  if let Some(db) = services.db.clone() {
    let id_for_task = id.clone();
    return iced::Task::perform(
      async move {
        let _ = db.skill_plans().delete(&id_for_task).await;
        id_for_task
      },
      Message::PlanDeleted,
    );
  }
  state.plans.retain(|p| p.id != id);
  recompute_queue(state);
  iced::Task::none()
}

fn handle_right_panel_plans_tab(state: &mut State, message: Message) -> iced::Task<Message> {
  let translated = if let Message::RightPanel(right_panel::Message::PlansTab(tab_msg)) = &message {
    match tab_msg {
      right_panel::plans_tab::Message::NewPlan => Some(Message::PlanNewRequested),
      right_panel::plans_tab::Message::FromQueue => Some(Message::PlanFromQueueRequested),
      right_panel::plans_tab::Message::OpenPlan(id) => Some(Message::PlanOpenRequested(id.clone())),
      right_panel::plans_tab::Message::DeleteRequested(id) => Some(Message::PlanDeleteRequested(id.clone())),
      right_panel::plans_tab::Message::DeleteConfirmed(id) => Some(Message::PlanDeleteConfirmed(id.clone())),
      right_panel::plans_tab::Message::DeleteCancelled => Some(Message::PlanDeleteCancelled),
    }
  } else {
    None
  };
  let _ = skills::update(state, message);
  recompute_queue(state);
  if let Some(msg) = translated {
    iced::Task::done(msg)
  } else {
    iced::Task::none()
  }
}

fn build_skills_picker(characters: &[Character], selected_char_id: i64) -> CharacterPicker {
  let picker_entries = characters
    .iter()
    .map(|c| CharacterEntry {
      id: Some(*c.id()),
      name: c.name().clone(),
      corp_name: c.corp_name().clone(),
      tone: *c.portrait_tone() as u16,
      portrait_handle: c.portrait_data().as_ref().map(|b| image::Handle::from_bytes(b.clone())),
    })
    .collect();
  CharacterPicker::new()
    .entries(picker_entries)
    .selected(PickerSelection::Character(selected_char_id))
}

fn build_initial_skills_state(
  characters: Vec<Character>,
  left_pane_width: f32,
  picker: CharacterPicker,
  queue: Vec<QueueItem>,
  char_skill_map: HashMap<String, (u8, i64)>,
) -> State {
  let mut expanded_groups = HashSet::new();
  expanded_groups.insert("spaceship".to_string());
  State {
    char_skill_map,
    characters,
    computed_queue: Vec::new(),
    confirm_delete_plan_id: None,
    dragging_pane: false,
    expanded_groups,
    last_drag_x: 0.0,
    left_pane_width,
    picker,
    plans: Vec::new(),
    plans_loaded: false,
    queue,
    queue_id_counter: 100,
    right_tab: RightTab::Browse,
    search_query: String::new(),
    skill_groups: Vec::new(),
    sp_rate: 0.0,
  }
}

fn build_skill_groups_task(services: &Services) -> iced::Task<Message> {
  if let Some(db) = services.db.clone() {
    iced::Task::perform(
      async move { db.universe().item_types().find_skill_groups().await.unwrap_or_default() },
      Message::SkillGroupsLoaded,
    )
  } else {
    iced::Task::none()
  }
}

fn compute_sp_rate(state: &State) -> f32 {
  let attr_pair = state
    .queue
    .first()
    .and_then(|q| find_skill(&q.skill_name, &state.skill_groups))
    .map(|(s, _)| (s.primary, s.secondary))
    .or_else(|| {
      state
        .active_character()
        .and_then(|c| c.active_training())
        .and_then(|t| t.skill_name.as_ref())
        .and_then(|n| find_skill(n, &state.skill_groups))
        .map(|(s, _)| (s.primary, s.secondary))
    })
    .unwrap_or((AttrKey::Perception, AttrKey::Willpower));
  sp_per_sec(state.attr_value(attr_pair.0), state.attr_value(attr_pair.1))
}

fn compute_queue(
  queue: &[QueueItem],
  sp_rate: f32,
  skill_groups: &[pod_model::SkillGroupDef],
) -> Vec<ComputedQueueItem> {
  let mut cursor = 0.0f32;
  let mut result = Vec::new();
  for (i, item) in queue.iter().enumerate() {
    let (rank, primary, secondary, group_name, sp_base) = find_skill(&item.skill_name, skill_groups)
      .map(|(s, g)| (s.rank, s.primary, s.secondary, g.to_owned(), s.sp))
      .unwrap_or((1, AttrKey::Perception, AttrKey::Willpower, String::new(), 0));

    let to_level = item.to_level;
    let from_level = item.from_level;

    let (sp_needed, sp_now, sp_to, progress) = if i == 0 {
      let total_for_range: u64 = ((from_level + 1)..=to_level).map(|l| sp_cost(rank as f64, l)).sum();
      let done = (total_for_range as f32 * item.progress) as u64;
      let needed = total_for_range.saturating_sub(done);
      (needed, sp_base + done, sp_base + total_for_range, item.progress)
    } else {
      let needed: u64 = ((from_level + 1)..=to_level).map(|l| sp_cost(rank as f64, l)).sum();
      (needed, sp_base, sp_base + needed, 0.0)
    };

    let duration_secs = if sp_rate > 0.0 { sp_needed as f32 / sp_rate } else { 0.0 };
    let cum_start = cursor;
    cursor += duration_secs;

    result.push(ComputedQueueItem {
      cum_start_secs: cum_start,
      duration_secs,
      from_level,
      group_name,
      primary,
      progress,
      rank,
      secondary,
      skill_name: item.skill_name.clone(),
      sp_needed,
      sp_now,
      sp_to,
      to_level,
    });
  }
  result
}

fn recompute_queue(state: &mut State) {
  state.sp_rate = compute_sp_rate(state);
  state.computed_queue = compute_queue(&state.queue, state.sp_rate, &state.skill_groups);
}

fn build_char_skill_map(characters: &[Character], char_id: i64) -> HashMap<String, (u8, i64)> {
  characters
    .iter()
    .find(|c| *c.id() == char_id)
    .map(|c| {
      c.skills()
        .iter()
        .filter_map(|s| {
          s.skill_name
            .as_ref()
            .map(|name| (name.clone(), (s.trained_level as u8, s.skillpoints)))
        })
        .collect()
    })
    .unwrap_or_default()
}

fn build_queue_from_character(c: &Character) -> Vec<QueueItem> {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs() as i64;

  let skill_names: HashMap<i32, String> = c
    .skills()
    .iter()
    .filter_map(|s| s.skill_name.as_ref().map(|n| (s.skill_id, n.clone())))
    .collect();

  c.training_queue()
    .iter()
    .filter_map(|entry| {
      let skill_name = entry
        .skill_name
        .clone()
        .or_else(|| skill_names.get(&entry.skill_id).cloned())?;
      let progress = queue_entry_progress(entry, now);
      Some(QueueItem {
        id: format!("real-{}-{}", entry.skill_id, entry.to_level),
        skill_name,
        from_level: entry.from_level as u8,
        to_level: entry.to_level as u8,
        progress,
      })
    })
    .collect()
}

fn queue_entry_progress(entry: &TrainingQueueEntry, now: i64) -> f32 {
  if let (Some(level_start), Some(level_end), Some(run_start_sp), Some(run_start), Some(run_end)) = (
    entry.level_start_sp,
    entry.level_end_sp,
    entry.training_start_sp,
    entry.start_date,
    entry.finish_date,
  ) {
    let level_range = (level_end - level_start) as f64;
    if level_range <= 0.0 {
      return 1.0;
    }
    let run_duration = (run_end - run_start) as f64;
    let sp_rate = if run_duration > 0.0 {
      (level_end - run_start_sp) as f64 / run_duration
    } else {
      return 1.0;
    };
    let current_sp = run_start_sp as f64 + (now - run_start).max(0) as f64 * sp_rate;
    return ((current_sp - level_start as f64) / level_range).clamp(0.0, 1.0) as f32;
  }
  if let (Some(start), Some(end)) = (entry.start_date, entry.finish_date)
    && end > start
  {
    return ((now - start) as f32 / (end - start) as f32).clamp(0.0, 1.0);
  }
  0.0
}
