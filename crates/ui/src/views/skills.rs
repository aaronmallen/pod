//! Skills view: queue display, character selection, and skill browser.

pub mod header;
pub mod queue_section;
pub mod right_panel;
pub mod skill_data;
pub mod training_hero;
pub mod warning_strip;

use std::collections::{HashMap, HashSet};

pub use header::Component as Header;
use iced::{
  Background, Element, Event, Length, Padding, Subscription, mouse,
  widget::{Space, column, container, mouse_area, row, scrollable},
};
use pod_model::{AttrKey, Character, SkillGroupDef, SkillPlan, missing_scopes};
pub use right_panel::Component as RightPanel;
pub use warning_strip::Component as WarningStrip;

use crate::{
  components::{CharacterPicker, ScopeMissing, character_picker, scope_missing},
  style::spacing,
};

/// One entry in the training queue.
#[derive(Clone, Debug)]
pub struct QueueItem {
  pub id: String,
  pub skill_name: String,
  pub from_level: u8,
  pub to_level: u8,
  pub progress: f32,
}

/// Which tab is active in the right panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightTab {
  Browse,
  Attrs,
  Plans,
}

/// Runtime state for the skills controller.
pub struct State {
  pub char_skill_map: HashMap<String, (u8, i64)>,
  pub characters: Vec<Character>,
  pub computed_queue: Vec<ComputedQueueItem>,
  pub confirm_delete_plan_id: Option<String>,
  pub dragging_pane: bool,
  pub expanded_groups: HashSet<String>,
  pub last_drag_x: f32,
  pub left_pane_width: f32,
  pub picker: CharacterPicker,
  pub plans: Vec<SkillPlan>,
  pub plans_loaded: bool,
  pub queue: Vec<QueueItem>,
  pub queue_id_counter: u64,
  pub right_tab: RightTab,
  pub search_query: String,
  pub skill_groups: Vec<SkillGroupDef>,
  pub sp_rate: f32,
}

impl State {
  pub fn selected_char_id(&self) -> i64 {
    self.picker.selected_character_id().unwrap_or(0)
  }

  pub fn active_character(&self) -> Option<&Character> {
    self.characters.iter().find(|c| *c.id() == self.selected_char_id())
  }

  /// Returns the effective value for the given neural attribute key.
  pub fn attr_value(&self, key: AttrKey) -> u32 {
    if let Some(attrs) = self.active_character().and_then(|c| c.attributes().as_ref()) {
      return match key {
        AttrKey::Perception => attrs.perception as u32,
        AttrKey::Willpower => attrs.willpower as u32,
        AttrKey::Intelligence => attrs.intelligence as u32,
        AttrKey::Memory => attrs.memory as u32,
        AttrKey::Charisma => attrs.charisma as u32,
      };
    }
    key.value()
  }
}

/// Messages produced by the skills view.
#[derive(Clone, Debug)]
pub enum Message {
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  Picker(character_picker::Message),
  ReauthorizeCharacter(i64),
  RightPanel(right_panel::Message),
  PlansTabOpened,
  PlansLoaded(Vec<SkillPlan>),
  PlanOpenRequested(String),
  PlanNewRequested,
  PlanFromQueueRequested,
  PlanDeleteRequested(String),
  PlanDeleteConfirmed(String),
  PlanDeleteCancelled,
  PlanDeleted(String),
  SkillGroupsLoaded(Vec<SkillGroupDef>),
}

/// Returns a subscription that tracks cursor movement during pane drag.
pub fn subscription(state: &State) -> Subscription<Message> {
  if !state.dragging_pane {
    return Subscription::none();
  }
  iced::event::listen_with(|event, _status, _id| match event {
    Event::Mouse(mouse::Event::CursorMoved {
      position,
    }) => Some(Message::PaneDrag(position.x)),
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::PaneDragEnd),
    _ => None,
  })
}

/// Processes a skills message and returns a task.
pub fn update(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::PaneDrag(x) => update_pane_drag(state, x),
    Message::PaneDragEnd => {
      state.dragging_pane = false;
      state.last_drag_x = 0.0;
    }
    Message::PaneDragStart => {
      state.dragging_pane = true;
      state.last_drag_x = 0.0;
    }
    Message::Picker(msg) => {
      state.picker.update(msg);
    }
    Message::RightPanel(msg) => update_right_panel(state, msg),
    Message::PlansLoaded(plans) => {
      state.plans = plans;
      state.plans_loaded = true;
    }
    Message::PlanDeleteRequested(id) => {
      state.confirm_delete_plan_id = Some(id);
    }
    Message::PlanDeleteCancelled => {
      state.confirm_delete_plan_id = None;
    }
    Message::PlanDeleted(id) => {
      state.plans.retain(|p| p.id != id);
      state.confirm_delete_plan_id = None;
    }
    Message::SkillGroupsLoaded(groups) => {
      state.skill_groups = groups;
    }
    Message::PlansTabOpened
    | Message::PlanOpenRequested(_)
    | Message::PlanNewRequested
    | Message::PlanFromQueueRequested
    | Message::PlanDeleteConfirmed(_)
    | Message::ReauthorizeCharacter(_) => {}
  }
  iced::Task::none()
}

fn update_pane_drag(state: &mut State, x: f32) {
  if state.last_drag_x > 0.0 {
    let delta = x - state.last_drag_x;
    state.left_pane_width = (state.left_pane_width + delta).max(100.0);
  }
  state.last_drag_x = x;
}

fn update_right_panel(state: &mut State, msg: right_panel::Message) {
  match msg {
    right_panel::Message::AttributesTab(_) => {}
    right_panel::Message::BrowserTab(tab_msg) => match tab_msg {
      right_panel::browser_tab::Message::GroupToggle(id) => update_group_toggle(state, id),
      right_panel::browser_tab::Message::SearchChanged(q) => update_search_changed(state, q),
    },
    right_panel::Message::PlansTab(tab_msg) => match tab_msg {
      right_panel::plans_tab::Message::NewPlan => {}
      right_panel::plans_tab::Message::FromQueue => {}
      right_panel::plans_tab::Message::OpenPlan(_) => {}
      right_panel::plans_tab::Message::DeleteRequested(id) => {
        state.confirm_delete_plan_id = Some(id);
      }
      right_panel::plans_tab::Message::DeleteConfirmed(_) => {}
      right_panel::plans_tab::Message::DeleteCancelled => {
        state.confirm_delete_plan_id = None;
      }
    },
    right_panel::Message::TabSelected(tab) => {
      state.right_tab = tab;
    }
  }
}

/// Computed data for one queue entry.
#[derive(Clone)]
pub struct ComputedQueueItem {
  pub cum_start_secs: f32,
  pub duration_secs: f32,
  pub from_level: u8,
  pub group_name: String,
  pub primary: AttrKey,
  pub progress: f32,
  pub rank: u8,
  pub secondary: AttrKey,
  pub skill_name: String,
  pub sp_needed: u64,
  pub sp_now: u64,
  pub sp_to: u64,
  pub to_level: u8,
}

/// Build the queue_levels map: skill_name → highest queued level.
pub fn queue_levels(queue: &[QueueItem]) -> HashMap<String, u8> {
  let mut m = HashMap::new();
  for item in queue {
    let entry = m.entry(item.skill_name.clone()).or_insert(0u8);
    if item.to_level > *entry {
      *entry = item.to_level;
    }
  }
  m
}

/// Format SP as compact string: "47.32M", "5.10K", "256"
pub fn fmt_sp(sp: u64) -> String {
  if sp >= 1_000_000 {
    format!("{:.2}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}K", sp as f64 / 1_000.0)
  } else {
    sp.to_string()
  }
}

/// Builder for the skills view.
pub struct Component<'a> {
  state: &'a State,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Create a new view builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_width: 1200.0,
    }
  }

  /// Set the window width so the left pane can be capped correctly.
  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    if let Some(char_id) = self.state.selected_char_id().checked_sub(0).filter(|&id| id != 0)
      && let Some(character) = self.state.characters.iter().find(|c| *c.id() == char_id)
    {
      let granted = character.granted_scopes_list();
      if !missing_scopes(
        &granted,
        &["esi-skills.read_skills.v1", "esi-skills.read_skillqueue.v1"],
      )
      .is_empty()
      {
        return ScopeMissing::new(char_id, "skill monitoring")
          .render()
          .map(|m| match m {
            scope_missing::Message::ReauthorizePressed(id) => Message::ReauthorizeCharacter(id),
          });
      }
    }
    view(self.state, self.window_width)
  }
}

fn view(state: &State, window_width: f32) -> Element<'_, Message> {
  let computed_items = &state.computed_queue;
  let sp_rate = state.sp_rate;
  let queue_len = state.queue.len();
  let total_secs = computed_items.iter().map(|c| c.duration_secs).sum::<f32>() as u64;
  let low_queue = total_secs > 0 && total_secs < 86400;

  let hdr = Header::new(state, total_secs, queue_len, low_queue).render();
  let warn = WarningStrip::new(low_queue).render();
  let left = left_col(state, computed_items, sp_rate, window_width);
  let right = RightPanel::new(state).render().map(Message::RightPanel);
  let body = row([left, pane_drag_handle(), right]).height(Length::Fill);

  let mut col = vec![hdr];
  if let Some(w) = warn {
    col.push(w);
  }
  col.push(body.into());

  let base: Element<'_, Message> = container(column(col).width(Length::Fill).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(crate::style::color::surface::BASE)),
      ..container::Style::default()
    })
    .into();

  apply_overlays(state, base)
}

fn drag_capture_overlay() -> Element<'static, Message> {
  mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
    .on_move(|pt| Message::PaneDrag(pt.x))
    .on_release(Message::PaneDragEnd)
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn picker_dropdown_overlay(state: &State) -> Element<'_, Message> {
  let dropdown = state.picker.dropdown().map(Message::Picker);
  container(dropdown)
    .padding(Padding {
      top: 98.0,
      left: 28.0,
      right: 0.0,
      bottom: 0.0,
    })
    .into()
}

fn apply_overlays<'a>(state: &'a State, base: Element<'a, Message>) -> Element<'a, Message> {
  match (state.dragging_pane, state.picker.is_open) {
    (false, false) => base,
    (false, true) => iced::widget::stack![base, picker_dropdown_overlay(state)].into(),
    (true, false) => iced::widget::stack![base, drag_capture_overlay()].into(),
    (true, true) => iced::widget::stack![base, drag_capture_overlay(), picker_dropdown_overlay(state)].into(),
  }
}

fn left_col<'a, 'b>(
  state: &'a State,
  computed_items: &'b [ComputedQueueItem],
  sp_rate: f32,
  window_width: f32,
) -> Element<'a, Message>
where
  'a: 'b,
{
  let min_right = 280.0;
  let handle = 4.0;
  let content_width = window_width - (spacing::layout::RAIL_WIDTH + 1.0);
  let max_left = (content_width - handle - min_right).max(100.0);
  let pane_width = state.left_pane_width.min(max_left);
  scrollable(
    column([
      training_hero::Component::new(state, computed_items, sp_rate).render(),
      queue_section::Component::new(state, computed_items).render(),
    ])
    .padding(Padding {
      top: 0.0,
      bottom: spacing::SPACE_4,
      left: 0.0,
      right: 0.0,
    })
    .width(Length::Fill),
  )
  .height(Length::Fill)
  .width(Length::Fixed(pane_width))
  .into()
}

fn pane_drag_handle() -> Element<'static, Message> {
  mouse_area(
    container(Space::new().width(4.0).height(Length::Fill))
      .width(4.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(crate::style::color::border::SUBTLE)),
        ..container::Style::default()
      }),
  )
  .on_press(Message::PaneDragStart)
  .interaction(iced::mouse::Interaction::ResizingHorizontally)
  .into()
}

fn update_search_changed(state: &mut State, q: String) {
  state.search_query = q;
  if !state.search_query.is_empty() {
    let ids: Vec<String> = state.skill_groups.iter().map(|g| g.id.clone()).collect();
    for id in ids {
      state.expanded_groups.insert(id);
    }
  }
}

fn update_group_toggle(state: &mut State, id: String) {
  if state.expanded_groups.contains(&id) {
    state.expanded_groups.remove(&id);
  } else {
    state.expanded_groups.insert(id);
  }
}
