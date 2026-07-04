use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use crate::{
  features::skills::{
    attributes,
    optimizer::{Attribute, Attributes},
    plan_math::{self, MilestoneAnchor, PlanEntry, PlanStep, RemapPoint},
    plan_summary::fmt_time_short,
  },
  store::{
    Database, images,
    model::SkillPlanEntry,
    repo::{character, org, skills},
  },
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      avatar::avatar,
      button::{Button, Size},
      chip::chip,
      clip::clip_layer,
      eyebrow::eyebrow_text,
      header,
      icon::Icon,
      rule,
    },
    format::fmt_sp,
    style::{color, radius, spacing, typography},
  },
};

pub const MANAGE_PLANS_WINDOW_HEIGHT: f32 = 620.0;
pub const MANAGE_PLANS_WINDOW_WIDTH: f32 = 940.0;

const RAIL_WIDTH: f32 = 256.0;
const RAIL_PORTRAIT: f32 = 32.0;
const DETAIL_PORTRAIT: f32 = 36.0;
const COPY_MENU_WIDTH: f32 = 280.0;
const MILESTONE_BAR_HEIGHT: f32 = 4.0;
const MILESTONE_BAR_WIDTH: f32 = 200.0;

#[derive(Clone, Debug)]
pub enum Message {
  CancelDelete,
  CharacterSelected(i64),
  CloseCopyMenu,
  ConfirmDelete(i64),
  CopyPlan { plan_id: i64, target_character_id: i64 },
  Loaded(Box<Roster>),
  NewPlan(i64),
  NewTemplate,
  OpenPlan { character_id: i64, plan_id: i64 },
  OpenTemplate(i64),
  RequestDelete(i64),
  TabSelected(Tab),
  ToggleCopyMenu(i64),
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MilestoneProgress {
  pub done: usize,
  pub steps_done: usize,
  pub steps_total: usize,
  pub total: usize,
}

impl MilestoneProgress {
  pub fn complete(&self) -> bool {
    self.done == self.total
  }

  pub fn fill_ratio(&self) -> f32 {
    if self.steps_total > 0 {
      self.steps_done as f32 / self.steps_total as f32
    } else {
      0.0
    }
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlanRow {
  pub edited: String,
  pub entry_count: usize,
  pub id: i64,
  pub milestones: MilestoneProgress,
  pub name: String,
  pub remaining_steps: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Roster {
  pub entries: Vec<RosterEntry>,
  pub templates: Vec<TemplateRow>,
}

impl Roster {
  pub fn plan_total(&self) -> usize {
    self.entries.iter().map(|entry| entry.plans.len()).sum()
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .entries
      .iter()
      .filter_map(|entry| entry.portrait.stale_key())
      .collect()
  }
}

#[derive(Clone, Debug)]
pub struct RosterEntry {
  pub character_id: i64,
  pub corp: String,
  pub name: String,
  pub plans: Vec<PlanRow>,
  pub portrait: images::ImageState,
}

#[derive(Debug)]
pub struct State {
  confirm_delete: Option<i64>,
  copy_menu: Option<i64>,
  roster: Roster,
  selected: Option<i64>,
  tab: Tab,
}

impl State {
  pub fn new() -> Self {
    State {
      confirm_delete: None,
      copy_menu: None,
      roster: Roster::default(),
      selected: None,
      tab: Tab::default(),
    }
  }

  pub fn arm_delete(&mut self, plan_id: i64) {
    self.confirm_delete = Some(plan_id);
    self.copy_menu = None;
  }

  pub fn clear_delete(&mut self) {
    self.confirm_delete = None;
  }

  pub fn close_copy_menu(&mut self) {
    self.copy_menu = None;
  }

  pub fn confirm_delete(&self) -> Option<i64> {
    self.confirm_delete
  }

  pub fn copy_menu(&self) -> Option<i64> {
    self.copy_menu
  }

  pub fn copy_targets(&self, source_character_id: i64) -> Vec<&RosterEntry> {
    self
      .roster
      .entries
      .iter()
      .filter(|entry| entry.character_id != source_character_id)
      .collect()
  }

  pub fn entries(&self) -> &[RosterEntry] {
    &self.roster.entries
  }

  pub fn select(&mut self, character_id: i64) {
    if self
      .roster
      .entries
      .iter()
      .any(|entry| entry.character_id == character_id)
    {
      self.selected = Some(character_id);
      self.confirm_delete = None;
      self.copy_menu = None;
    }
  }

  #[cfg(test)]
  pub fn selected(&self) -> Option<i64> {
    self.selected
  }

  pub fn set_roster(&mut self, roster: Roster) {
    self.roster = roster;
    let selected_still_present = self
      .selected
      .is_some_and(|id| self.roster.entries.iter().any(|entry| entry.character_id == id));
    if !selected_still_present {
      self.selected = self.default_selection();
    }
    let plan_ids: Vec<i64> = self
      .roster
      .entries
      .iter()
      .flat_map(|entry| entry.plans.iter().map(|plan| plan.id))
      .chain(self.roster.templates.iter().map(|template| template.id))
      .collect();
    if self.confirm_delete.is_some_and(|id| !plan_ids.contains(&id)) {
      self.confirm_delete = None;
    }
    if self.copy_menu.is_some_and(|id| !plan_ids.contains(&id)) {
      self.copy_menu = None;
    }
  }

  pub fn set_tab(&mut self, tab: Tab) {
    self.tab = tab;
    self.confirm_delete = None;
    self.copy_menu = None;
  }

  #[cfg(test)]
  pub fn tab(&self) -> Tab {
    self.tab
  }

  pub fn toggle_copy_menu(&mut self, plan_id: i64) {
    self.copy_menu = if self.copy_menu == Some(plan_id) {
      None
    } else {
      self.confirm_delete = None;
      Some(plan_id)
    };
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self.roster.stale_images()
  }

  fn default_selection(&self) -> Option<i64> {
    self
      .roster
      .entries
      .iter()
      .find(|entry| !entry.plans.is_empty())
      .or_else(|| self.roster.entries.first())
      .map(|entry| entry.character_id)
  }

  fn selected_entry(&self) -> Option<&RosterEntry> {
    let id = self.selected?;
    self.roster.entries.iter().find(|entry| entry.character_id == id)
  }
}

impl Default for State {
  fn default() -> Self {
    State::new()
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  Characters,
  #[default]
  Templates,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplateRow {
  pub edited: String,
  pub id: i64,
  pub name: String,
  pub step_count: usize,
  pub total_sec: f64,
  pub total_sp: i64,
}

pub fn load(db: &Database) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { Box::new(load_roster(&db).await) }, Message::Loaded)
}

pub async fn load_roster(db: &Database) -> Roster {
  let owned = character::all_owned(db).await.unwrap_or_default();

  let mut entries = Vec::with_capacity(owned.len());
  for character in owned {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|corp| corp.ticker().to_owned())
      .unwrap_or_default();
    let portrait = images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      character.id(),
    );
    let plans = load_plan_rows(db, character.id()).await;

    entries.push(RosterEntry {
      character_id: character.id(),
      corp,
      name: character.name().to_owned(),
      plans,
      portrait,
    });
  }

  entries.sort_by_key(|entry| entry.name.to_lowercase());
  let templates = load_template_rows(db).await;
  Roster {
    entries,
    templates,
  }
}

async fn load_template_rows(db: &Database) -> Vec<TemplateRow> {
  let templates = skills::templates(db).await.unwrap_or_default();

  let mut rows = Vec::with_capacity(templates.len());
  for template in templates {
    let entries = skills::entries(db, template.id()).await.unwrap_or_default();
    // A skill can appear as more than one step (e.g. a level bump added later); only the
    // highest requested level is costed, so `total_sp` isn't a per-step sum.
    let mut levels: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();
    for entry in &entries {
      let level = entry.to_level().clamp(0, 5) as u8;
      levels
        .entry(entry.skill_id())
        .and_modify(|current| *current = (*current).max(level))
        .or_insert(level);
    }

    rows.push(TemplateRow {
      edited: relative_time(template.updated_at()),
      id: template.id(),
      name: template.name().to_owned(),
      step_count: entries.len(),
      total_sec: template_training_secs(db, template.id(), &entries).await,
      total_sp: zero_based_sp(db, &levels).await,
    });
  }
  rows
}

/// Costs every skill from level 0 with no synced progress, since a template has no owning
/// character to read a trained level or partial SP from.
async fn template_training_secs(db: &Database, template_id: i64, entries: &[SkillPlanEntry]) -> f64 {
  let mut plan_entries = Vec::with_capacity(entries.len());
  for entry in entries {
    let metadata = skills::get_skill_metadata(db, entry.skill_id()).await.ok().flatten();
    let rank = metadata
      .map(|meta| meta.rank())
      .unwrap_or(1)
      .clamp(1, i64::from(u8::MAX));
    let primary = metadata
      .map(|meta| attributes::attribute_from_neural_id(meta.primary_attribute()))
      .unwrap_or(Attribute::Perception);
    let secondary = metadata
      .map(|meta| attributes::attribute_from_neural_id(meta.secondary_attribute()))
      .unwrap_or(Attribute::Perception);

    plan_entries.push(PlanEntry {
      partial_sp_at_from: 0,
      primary,
      rank: rank as f64,
      secondary,
      skill_id: entry.skill_id(),
      synced_trained_level: 0,
      to_level: entry.to_level().clamp(0, 5) as u8,
    });
  }

  let entry_ids: Vec<i64> = entries.iter().map(|entry| entry.id()).collect();
  let remap_points = skills::milestones(db, template_id)
    .await
    .unwrap_or_default()
    .iter()
    .filter_map(|point| {
      let after_index = match point.after_entry_id() {
        // -1 is the "applies before the first entry" sentinel plan_math matches on.
        None => -1,
        Some(id) => entry_ids.iter().position(|&candidate| candidate == id)? as i64,
      };
      Some(RemapPoint {
        after_index,
        base: Attributes {
          charisma: point.base_charisma()?.max(0) as u32,
          intelligence: point.base_intelligence()?.max(0) as u32,
          memory: point.base_memory()?.max(0) as u32,
          perception: point.base_perception()?.max(0) as u32,
          willpower: point.base_willpower()?.max(0) as u32,
        },
      })
    })
    .collect();

  plan_math::template_plan(&plan_entries, remap_points).total_sec
}

/// Costs every skill from level 0, since a template has no owning character to read a trained
/// level from (and thus no ETA to net out), unlike `load_plan_rows`'s remaining-steps math.
async fn zero_based_sp(db: &Database, levels: &std::collections::HashMap<i64, u8>) -> i64 {
  let mut total = 0;
  for (&skill_id, &level) in levels {
    let rank = skills::get_skill_metadata(db, skill_id)
      .await
      .ok()
      .flatten()
      .map(|metadata| metadata.rank())
      .unwrap_or(1)
      .clamp(1, i64::from(u8::MAX));
    total += plan_math::step_sp(rank as f64, 0, level, 0);
  }
  total
}

async fn load_plan_rows(db: &Database, character_id: i64) -> Vec<PlanRow> {
  let plans = skills::for_character(db, character_id).await.unwrap_or_default();

  let trained: std::collections::HashMap<i64, u8> = character::skills(db, character_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|skill| (skill.skill_id(), skill.trained_skill_level().clamp(0, 5) as u8))
    .collect();

  let mut rows = Vec::with_capacity(plans.len());
  for plan in plans {
    let entries = skills::entries(db, plan.id()).await.unwrap_or_default();
    let entry_ids: Vec<i64> = entries.iter().map(|entry| entry.id()).collect();
    let steps: Vec<PlanStep> = entries
      .iter()
      .map(|entry| PlanStep {
        skill_id: entry.skill_id(),
        to_level: entry.to_level().clamp(0, 5) as u8,
      })
      .collect();

    let anchors: Vec<MilestoneAnchor> = skills::milestones(db, plan.id())
      .await
      .unwrap_or_default()
      .iter()
      .map(|milestone| MilestoneAnchor {
        after_entry_id: milestone.after_entry_id(),
        order: milestone.position(),
      })
      .collect();

    rows.push(PlanRow {
      edited: relative_time(plan.updated_at()),
      entry_count: steps.len(),
      id: plan.id(),
      milestones: milestone_progress(&entry_ids, &steps, &anchors, &trained),
      name: plan.name().to_owned(),
      remaining_steps: plan_math::remaining_steps(&steps, &trained),
    });
  }
  rows
}

/// A milestone is done when every step in its segment is already met by the trained level.
///
/// A step counts as met if its target level is at or below the trained level *or* an earlier step
/// already scheduled that skill to at least that level, so redundant/lowered steps don't
/// understate completion. A milestone anchored past the last entry still adds to `total` but owns
/// an empty segment, so it can never add to `done`.
fn milestone_progress(
  entry_ids: &[i64],
  steps: &[PlanStep],
  milestones: &[MilestoneAnchor],
  trained: &std::collections::HashMap<i64, u8>,
) -> MilestoneProgress {
  let segments = plan_math::plan_segments(entry_ids, milestones);

  let mut scheduled: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();
  let mut skipped = Vec::with_capacity(steps.len());
  for step in steps {
    let trained_level = trained.get(&step.skill_id).copied().unwrap_or(0);
    let prior = scheduled.get(&step.skill_id).copied().unwrap_or(0);
    let starting = trained_level.max(prior);
    skipped.push(step.to_level <= starting);
    scheduled.insert(step.skill_id, starting.max(step.to_level));
  }

  let mut progress = MilestoneProgress::default();
  for segment in segments {
    if segment.milestone.is_none() {
      continue;
    }
    progress.total += 1;

    let mut total = 0;
    let mut done = 0;
    for &is_skipped in skipped.iter().take(segment.end).skip(segment.start) {
      total += 1;
      if is_skipped {
        done += 1;
      }
    }

    progress.steps_total += total;
    progress.steps_done += done;
    if total > 0 && done == total {
      progress.done += 1;
    }
  }

  progress
}

pub fn view(state: &State) -> Element<'_, Message> {
  window_body(state)
}

fn window_body(state: &State) -> Element<'_, Message> {
  let body: Element<'_, Message> = match state.tab {
    Tab::Characters => Row::with_children(vec![rail(state), detail(state)])
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
    Tab::Templates => templates_tab(state),
  };

  container(
    Column::with_children(vec![header(state), tab_strip(state.tab), body])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  })
  .into()
}

fn header(state: &State) -> Element<'_, Message> {
  let info = Column::with_children(vec![
    text(t!("skills.manager.title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(header_summary(state))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT);

  header::header(vec![info.into()], Vec::new())
}

fn header_summary(state: &State) -> String {
  match state.tab {
    Tab::Characters => {
      let total = state.roster.plan_total();
      let characters = state.roster.entries.len();
      let plan_word = if total == 1 {
        t!("skills.manager.plan_singular")
      } else {
        t!("skills.manager.plan_plural")
      };
      let char_word = if characters == 1 {
        t!("skills.manager.character_singular")
      } else {
        t!("skills.manager.character_plural")
      };
      t!(
        "skills.manager.header_summary",
        plan_count => total,
        plan_word => plan_word,
        char_count => characters,
        char_word => char_word
      )
      .into_owned()
    }
    Tab::Templates => {
      let count = state.roster.templates.len();
      let template_word = if count == 1 {
        t!("skills.manager.template_singular")
      } else {
        t!("skills.manager.template_plural")
      };
      t!(
        "skills.manager.header_summary_templates",
        count => count,
        template_word => template_word
      )
      .into_owned()
    }
  }
}

fn tab_strip<'a>(active: Tab) -> Element<'a, Message> {
  let tabs = Row::with_children(vec![
    tab_button(
      t!("skills.manager.templates").into_owned(),
      active == Tab::Templates,
      Message::TabSelected(Tab::Templates),
    ),
    tab_button(
      t!("skills.manager.characters").into_owned(),
      active == Tab::Characters,
      Message::TabSelected(Tab::Characters),
    ),
  ])
  .spacing(spacing::UNIT);

  Column::with_children(vec![
    container(tabs)
      .width(Length::Fill)
      .padding(Padding {
        top: 0.0,
        right: spacing::SPACE_3_5 + spacing::SPACE_2,
        bottom: 0.0,
        left: spacing::SPACE_3_5 + spacing::SPACE_2,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        ..container::Style::default()
      })
      .into(),
    rule::horizontal(),
  ])
  .width(Length::Fill)
  .into()
}

fn tab_button<'a>(label: String, active: bool, message: Message) -> Element<'a, Message> {
  let content = Column::with_children(vec![
    container(text(label).font(typography::body::MEDIUM).size(typography::size::SM))
      .padding(Padding {
        top: spacing::SPACE_2,
        right: spacing::SPACE_3,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3,
      })
      .into(),
    tab_underline(active),
  ]);

  button(content)
    .padding(0.0)
    .on_press(message)
    .style(move |_, status| tab_button_style(active, status))
    .into()
}

fn tab_underline<'a, M: 'a>(active: bool) -> Element<'a, M> {
  container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
    .width(Length::Fill)
    .height(Length::Fixed(2.0))
    .style(move |_| container::Style {
      background: active.then_some(Background::Color(color::accent())),
      ..container::Style::default()
    })
    .into()
}

fn tab_button_style(active: bool, status: button::Status) -> button::Style {
  let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    text_color: if active || hover {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    },
    ..button::Style::default()
  }
}

fn templates_tab(state: &State) -> Element<'_, Message> {
  let body = container(
    scrollable(template_list(state))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill);

  Column::with_children(vec![templates_header(), body.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn templates_header<'a>() -> Element<'a, Message> {
  let info = Column::with_children(vec![
    text(t!("skills.manager.templates_title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(t!("skills.manager.templates_hint").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![info.into(), new_template_button()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5 + spacing::SPACE_2,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5 + spacing::SPACE_2,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn template_list(state: &State) -> Element<'_, Message> {
  if state.roster.templates.is_empty() {
    return templates_empty();
  }

  let targets: Vec<&RosterEntry> = state.roster.entries.iter().collect();
  let cards: Vec<Element<'_, Message>> = state
    .roster
    .templates
    .iter()
    .map(|template| {
      template_card(
        template,
        state.confirm_delete() == Some(template.id),
        state.copy_menu() == Some(template.id),
        &targets,
      )
    })
    .collect();

  Column::with_children(cards)
    .spacing(spacing::SPACE_2_5)
    .padding(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn templates_empty<'a>() -> Element<'a, Message> {
  container(
    Column::with_children(vec![
      text(t!("skills.manager.no_templates").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
      text(t!("skills.manager.no_templates_hint").into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_3_5)
  .into()
}

fn template_card<'a>(
  template: &TemplateRow,
  confirming_delete: bool,
  copy_menu_open: bool,
  targets: &[&RosterEntry],
) -> Element<'a, Message> {
  let step_word = if template.step_count == 1 {
    t!("skills.manager.step_singular")
  } else {
    t!("skills.manager.step_plural")
  };
  let meta = t!(
    "skills.manager.template_meta",
    step_count => template.step_count,
    step_word => step_word,
    sp => fmt_sp(template.total_sp),
    time => fmt_time_short(template.total_sec),
    edited => template.edited
  )
  .into_owned();

  let actions: Element<'a, Message> = if confirming_delete {
    delete_confirm_actions(template.id)
  } else {
    Row::with_children(vec![
      ghost_button(
        t!("skills.manager.open").into_owned(),
        Message::OpenTemplate(template.id),
      ),
      // Importing a template reuses `Message::CopyPlan`: a template is just an ownerless skill
      // plan, so the same materialize-onto-character path copies it onto the chosen character.
      copy_to_button(
        template.id,
        targets,
        copy_menu_open,
        t!("skills.manager.import_to").into_owned(),
        &t!("skills.manager.import_onto_character"),
      ),
      delete_button(template.id),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
  };

  card_row(
    chip(template.step_count.to_string(), Some(color::accent())),
    card_info(template.name.clone(), meta, None),
    actions,
  )
}

fn rail(state: &State) -> Element<'_, Message> {
  let mut items: Vec<Element<'_, Message>> = vec![
    container(eyebrow_text(
      t!("skills.manager.characters").as_ref(),
      Some(color::text::tertiary()),
    ))
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
    })
    .into(),
  ];
  for entry in &state.roster.entries {
    items.push(rail_item(entry, state.selected == Some(entry.character_id)));
  }

  container(
    scrollable(Column::with_children(items).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fill),
  )
  .width(Length::Fixed(RAIL_WIDTH))
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.06),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn rail_item(entry: &RosterEntry, active: bool) -> Element<'_, Message> {
  let mut lines: Vec<Element<'_, Message>> = vec![
    text(entry.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if !entry.corp.is_empty() {
    lines.push(
      text(entry.corp.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let row = Row::with_children(vec![
    portrait_tile(&entry.portrait, &entry.name, RAIL_PORTRAIT),
    Column::with_children(lines).spacing(2.0).width(Length::Fill).into(),
    chip(entry.plans.len().to_string(), count_tint(entry.plans.len(), active)),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  button(container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5 - 2.0,
  }))
  .padding(0.0)
  .on_press(Message::CharacterSelected(entry.character_id))
  .style(move |_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = if active {
      Some(Background::Color(color::with_alpha(color::accent(), 0.10)))
    } else if hover {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
    } else {
      None
    };
    button::Style {
      background,
      border: Border {
        color: if active {
          color::accent()
        } else {
          iced::Color::TRANSPARENT
        },
        width: 0.0,
        radius: 0.0.into(),
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn detail(state: &State) -> Element<'_, Message> {
  let Some(entry) = state.selected_entry() else {
    return container(
      text(t!("skills.manager.no_characters").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into();
  };

  let body = container(
    scrollable(detail_plans(state, entry))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill);

  container(
    Column::with_children(vec![detail_header(entry), body.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn detail_header(entry: &RosterEntry) -> Element<'_, Message> {
  let mut lines: Vec<Element<'_, Message>> = vec![
    text(entry.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if !entry.corp.is_empty() {
    lines.push(
      text(entry.corp.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    );
  }

  let identity = Row::with_children(vec![
    portrait_tile(&entry.portrait, &entry.name, DETAIL_PORTRAIT),
    Column::with_children(lines).spacing(2.0).width(Length::Fill).into(),
    new_plan_button(entry.character_id),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(identity)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5 + spacing::SPACE_2,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5 + spacing::SPACE_2,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn detail_plans<'a>(state: &'a State, entry: &'a RosterEntry) -> Element<'a, Message> {
  if entry.plans.is_empty() {
    return container(
      Column::with_children(vec![
        text(t!("skills.manager.no_plans_for", name => first_name(&entry.name)).into_owned())
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          })
          .into(),
        text(t!("skills.manager.no_plans_hint").into_owned())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          })
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(spacing::SPACE_3_5)
    .into();
  }

  let targets = state.copy_targets(entry.character_id);
  let cards: Vec<Element<'a, Message>> = entry
    .plans
    .iter()
    .map(|plan| {
      plan_card(
        plan,
        entry.character_id,
        state.confirm_delete() == Some(plan.id),
        state.copy_menu() == Some(plan.id),
        &targets,
      )
    })
    .collect();

  Column::with_children(cards)
    .spacing(spacing::SPACE_2_5)
    .padding(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn plan_card<'a>(
  plan: &PlanRow,
  character_id: i64,
  confirming_delete: bool,
  copy_menu_open: bool,
  targets: &[&RosterEntry],
) -> Element<'a, Message> {
  let count = plan.entry_count;
  let skill_word = if count == 1 {
    t!("skills.manager.skill_singular")
  } else {
    t!("skills.manager.skill_plural")
  };
  let meta = t!(
    "skills.manager.plan_meta",
    skill_count => count,
    skill_word => skill_word,
    edited => plan.edited
  )
  .into_owned();

  let actions: Element<'a, Message> = if confirming_delete {
    delete_confirm_actions(plan.id)
  } else {
    Row::with_children(vec![
      ghost_button(
        t!("skills.manager.open").into_owned(),
        Message::OpenPlan {
          character_id,
          plan_id: plan.id,
        },
      ),
      copy_to_button(
        plan.id,
        targets,
        copy_menu_open,
        t!("skills.manager.copy_to").into_owned(),
        &t!("skills.manager.copy_to_character"),
      ),
      delete_button(plan.id),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
  };

  let progress = (plan.milestones.total > 0).then(|| milestone_progress_row(plan.milestones));

  card_row(
    chip(plan.remaining_steps.to_string(), Some(color::accent())),
    card_info(plan.name.clone(), meta, progress),
    actions,
  )
}

fn card_info<'a>(name: String, meta: String, extra: Option<Element<'a, Message>>) -> Element<'a, Message> {
  let mut lines: Vec<Element<'a, Message>> = vec![
    text(name)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(meta)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ];
  if let Some(extra) = extra {
    lines.push(extra);
  }

  Column::with_children(lines).spacing(2.0).width(Length::Fill).into()
}

fn milestone_progress_row<'a>(progress: MilestoneProgress) -> Element<'a, Message> {
  let complete = progress.complete();
  let fill_color = if complete {
    color::status::ONLINE
  } else {
    color::accent()
  };
  let label_color = if complete {
    color::status::ONLINE
  } else {
    color::text::secondary()
  };
  let fill_width = (progress.fill_ratio() * MILESTONE_BAR_WIDTH).round();

  let fill = container(
    Space::new()
      .width(Length::Fixed(fill_width))
      .height(Length::Fixed(MILESTONE_BAR_HEIGHT)),
  )
  .height(Length::Fixed(MILESTONE_BAR_HEIGHT))
  .style(move |_| container::Style {
    background: Some(Background::Color(fill_color)),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let track = container(fill)
    .width(Length::Fixed(MILESTONE_BAR_WIDTH))
    .height(Length::Fixed(MILESTONE_BAR_HEIGHT))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.10))),
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let label = text(milestone_label(progress))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(label_color),
    });

  container(
    Row::with_children(vec![track.into(), label.into()])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  })
  .into()
}

fn milestone_label(progress: MilestoneProgress) -> String {
  let word = if progress.total == 1 {
    t!("skills.manager.milestone_singular")
  } else {
    t!("skills.manager.milestone_plural")
  };
  t!(
    "skills.manager.milestone_progress",
    done => progress.done,
    total => progress.total,
    milestone_word => word
  )
  .into_owned()
}

fn card_row<'a>(
  badge: Element<'a, Message>,
  info: Element<'a, Message>,
  actions: Element<'a, Message>,
) -> Element<'a, Message> {
  container(
    Row::with_children(vec![badge, info, actions])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.08),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn delete_confirm_actions<'a>(plan_id: i64) -> Element<'a, Message> {
  Row::with_children(vec![
    text(t!("skills.manager.delete_confirm").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      })
      .into(),
    ghost_button(t!("skills.manager.cancel").into_owned(), Message::CancelDelete),
    danger_button(
      t!("skills.manager.delete").into_owned(),
      Message::ConfirmDelete(plan_id),
    ),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn copy_menu<'a>(plan_id: i64, targets: &[&RosterEntry], heading: &str) -> Element<'a, Message> {
  let mut items: Vec<Element<'a, Message>> = vec![
    container(eyebrow_text(heading, Some(color::text::tertiary())))
      .padding(Padding {
        top: spacing::SPACE_2,
        right: spacing::SPACE_3,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3,
      })
      .into(),
  ];
  for target in targets {
    items.push(copy_menu_item(plan_id, target));
  }

  container(Column::with_children(items).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn copy_menu_item<'a>(plan_id: i64, target: &RosterEntry) -> Element<'a, Message> {
  let mut lines: Vec<Element<'a, Message>> = vec![
    text(target.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if !target.corp.is_empty() {
    lines.push(
      text(target.corp.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let row = Row::with_children(vec![
    portrait_tile(&target.portrait, &target.name, RAIL_PORTRAIT),
    Column::with_children(lines).spacing(2.0).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let target_character_id = target.character_id;
  button(container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  }))
  .padding(0.0)
  .on_press(Message::CopyPlan {
    plan_id,
    target_character_id,
  })
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.04))),
      ..button::Style::default()
    }
  })
  .into()
}

fn new_plan_button<'a>(character_id: i64) -> Element<'a, Message> {
  Button::primary(t!("skills.manager.new_plan").into_owned())
    .icon(Icon::plus())
    .size(Size::Sm)
    .on_press(Message::NewPlan(character_id))
    .into()
}

fn new_template_button<'a>() -> Element<'a, Message> {
  Button::primary(t!("skills.manager.new_template").into_owned())
    .icon(Icon::plus())
    .size(Size::Sm)
    .on_press(Message::NewTemplate)
    .into()
}

fn ghost_button<'a>(label: String, message: Message) -> Element<'a, Message> {
  Button::secondary(label).size(Size::Sm).on_press(message).into()
}

fn copy_to_button<'a>(
  plan_id: i64,
  targets: &[&RosterEntry],
  menu_open: bool,
  label: String,
  heading: &str,
) -> Element<'a, Message> {
  let enabled = !targets.is_empty();
  let trigger = copy_to_trigger(plan_id, enabled, menu_open, label);
  let popover = (menu_open && enabled).then(|| copy_menu(plan_id, targets, heading));

  AnchoredDropdown::new(trigger, popover)
    .on_dismiss(Message::CloseCopyMenu)
    .popover_width(COPY_MENU_WIDTH)
    .into()
}

fn copy_to_trigger<'a>(plan_id: i64, enabled: bool, menu_open: bool, label: String) -> Element<'a, Message> {
  let label_color = if enabled {
    color::accent()
  } else {
    color::text::tertiary()
  };
  let label = button(
    Row::with_children(vec![
      text(label)
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(move |_| copy_button_label_style(enabled))
        .into(),
      Icon::chevron_down().size(13.0).color(label_color).render(),
    ])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 6.0,
    right: spacing::SPACE_2_5,
    bottom: 6.0,
    left: spacing::SPACE_2_5,
  })
  .style(move |_, status| copy_button_style(enabled, menu_open, status));

  if enabled {
    label.on_press(Message::ToggleCopyMenu(plan_id)).into()
  } else {
    label.into()
  }
}

fn copy_button_label_style(enabled: bool) -> text::Style {
  text::Style {
    color: Some(if enabled {
      color::accent()
    } else {
      color::text::tertiary()
    }),
  }
}

fn copy_button_style(enabled: bool, menu_open: bool, status: button::Status) -> button::Style {
  let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let border_color = if menu_open || (enabled && hover) {
    color::accent()
  } else if enabled {
    color::accent_muted()
  } else {
    color::with_alpha(color::text::PRIMARY, 0.1)
  };
  button::Style {
    background: (enabled && hover).then(|| Background::Color(color::with_alpha(color::accent(), 0.10))),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    text_color: if enabled {
      color::accent()
    } else {
      color::text::tertiary()
    },
    ..button::Style::default()
  }
}

fn danger_button<'a>(label: String, message: Message) -> Element<'a, Message> {
  Button::danger(label).size(Size::Sm).on_press(message).into()
}

fn delete_button<'a>(plan_id: i64) -> Element<'a, Message> {
  Button::danger_icon(Icon::close())
    .size(Size::Sm)
    .on_press(Message::RequestDelete(plan_id))
    .into()
}

fn portrait_tile<'a, M: 'a>(portrait: &images::ImageState, name: &str, box_size: f32) -> Element<'a, M> {
  match portrait.path() {
    Some(path) => container(clip_layer(
      image(image::Handle::from_path(path))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
      Length::Fill,
      Length::Fill,
    ))
    .width(Length::Fixed(box_size))
    .height(Length::Fixed(box_size))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into(),
    None => avatar(0, name, Length::Fixed(box_size), box_size, None),
  }
}

fn count_tint(count: usize, active: bool) -> Option<iced::Color> {
  if count == 0 {
    Some(color::text::tertiary())
  } else if active {
    Some(color::accent())
  } else {
    Some(color::text::secondary())
  }
}

fn first_name(name: &str) -> String {
  name.split_whitespace().next().unwrap_or(name).to_owned()
}

pub fn relative_time(iso: &str) -> String {
  let Some(ts) = parse_iso8601(iso) else {
    return iso.to_owned();
  };
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let diff = now - ts;
  if diff < 60 {
    t!("skills.manager.time_just_now").into_owned()
  } else if diff < 3600 {
    t!("skills.manager.time_minutes_ago", count => diff / 60).into_owned()
  } else if diff < 86_400 {
    t!("skills.manager.time_hours_ago", count => diff / 3600).into_owned()
  } else {
    t!("skills.manager.time_days_ago", count => diff / 86_400).into_owned()
  }
}

fn days_since_epoch(y: i64, m: i64, d: i64) -> i64 {
  let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let doy = (153 * m + 2) / 5 + d - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146_097 + doe - 719_468
}

fn parse_iso8601(s: &str) -> Option<i64> {
  let s = s.trim().trim_end_matches('Z');
  let (date, time) = s.split_once('T')?;
  let date_parts: Vec<i64> = date.split('-').filter_map(|p| p.parse().ok()).collect();
  let time_parts: Vec<i64> = time
    .split('+')
    .next()
    .unwrap_or("")
    .split(':')
    .filter_map(|p| p.parse::<f64>().ok().map(|v| v as i64))
    .collect();
  if date_parts.len() < 3 || time_parts.len() < 3 {
    return None;
  }
  let days = days_since_epoch(date_parts[0], date_parts[1], date_parts[2]);
  Some(days * 86_400 + time_parts[0] * 3600 + time_parts[1] * 60 + time_parts[2])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn plan(id: i64, name: &str, entry_count: usize, remaining_steps: usize) -> PlanRow {
    PlanRow {
      edited: "2d ago".to_owned(),
      entry_count,
      id,
      milestones: MilestoneProgress::default(),
      name: name.to_owned(),
      remaining_steps,
    }
  }

  fn entry(character_id: i64, name: &str, plans: Vec<PlanRow>) -> RosterEntry {
    RosterEntry {
      character_id,
      corp: "TST".to_owned(),
      name: name.to_owned(),
      plans,
      portrait: images::ImageState::Fresh("/tmp/p.jpg".into()),
    }
  }

  fn roster() -> Roster {
    Roster {
      entries: vec![
        entry(1, "Aria", vec![plan(10, "Combat", 5, 2), plan(11, "Industry", 0, 0)]),
        entry(2, "Borin", Vec::new()),
        entry(3, "Cassi", vec![plan(12, "Logi", 3, 3)]),
      ],
      templates: vec![template(50, "Doctrine", 4, 1_200_000)],
    }
  }

  fn template(id: i64, name: &str, step_count: usize, total_sp: i64) -> TemplateRow {
    TemplateRow {
      edited: "2d ago".to_owned(),
      id,
      name: name.to_owned(),
      step_count,
      total_sec: 0.0,
      total_sp,
    }
  }

  mod roster {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_sums_plans_across_every_character() {
      assert_eq!(super::roster().plan_total(), 3);
    }
  }

  mod set_roster {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_the_selection_to_the_first_character_with_plans() {
      let mut state = State::new();

      state.set_roster(super::roster());

      assert_eq!(state.selected(), Some(1));
    }

    #[test]
    fn it_falls_back_to_the_first_character_when_none_have_plans() {
      let mut state = State::new();
      let empty = Roster {
        entries: vec![entry(7, "Solo", Vec::new())],
        templates: Vec::new(),
      };

      state.set_roster(empty);

      assert_eq!(state.selected(), Some(7));
    }

    #[test]
    fn it_keeps_a_still_present_selection_across_reloads() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.select(3);

      state.set_roster(super::roster());

      assert_eq!(state.selected(), Some(3));
    }

    #[test]
    fn it_reselects_when_the_previous_character_is_gone() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.select(3);

      state.set_roster(Roster {
        entries: vec![entry(9, "New", vec![plan(20, "Fresh", 2, 1)])],
        templates: Vec::new(),
      });

      assert_eq!(state.selected(), Some(9));
    }

    #[test]
    fn it_clears_the_selection_for_an_empty_roster() {
      let mut state = State::new();
      state.set_roster(super::roster());

      state.set_roster(Roster::default());

      assert_eq!(state.selected(), None);
    }

    #[test]
    fn it_keeps_an_armed_affordance_on_a_still_present_template() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(50);

      state.set_roster(super::roster());

      assert_eq!(state.confirm_delete(), Some(50));
    }

    #[test]
    fn it_drops_a_stale_menu_when_the_template_is_gone() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.toggle_copy_menu(50);

      state.set_roster(Roster {
        entries: vec![entry(1, "Aria", vec![plan(10, "Combat", 5, 2)])],
        templates: Vec::new(),
      });

      assert_eq!(state.copy_menu(), None);
    }
  }

  mod set_tab {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_to_the_templates_tab() {
      assert_eq!(State::new().tab(), Tab::Templates);
    }

    #[test]
    fn it_clears_armed_affordances_when_switching_tabs() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(10);
      state.toggle_copy_menu(11);

      state.set_tab(Tab::Characters);

      assert_eq!(state.tab(), Tab::Characters);
      assert_eq!(state.confirm_delete(), None);
      assert_eq!(state.copy_menu(), None);
    }
  }

  mod select {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ignores_an_unknown_character() {
      let mut state = State::new();
      state.set_roster(super::roster());

      state.select(999);

      assert_eq!(state.selected(), Some(1));
    }

    #[test]
    fn it_clears_armed_affordances_on_select() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(10);
      state.toggle_copy_menu(11);

      state.select(3);

      assert_eq!(state.confirm_delete(), None);
      assert_eq!(state.copy_menu(), None);
    }
  }

  mod copy_targets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_excludes_the_source_character() {
      let mut state = State::new();
      state.set_roster(super::roster());

      let target_ids: Vec<i64> = state.copy_targets(1).iter().map(|e| e.character_id).collect();

      assert_eq!(target_ids, vec![2, 3]);
    }
  }

  mod toggle_copy_menu {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_then_closes_the_same_plan_and_disarms_delete() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(10);

      state.toggle_copy_menu(10);
      assert_eq!(state.copy_menu(), Some(10));
      assert_eq!(
        state.confirm_delete(),
        None,
        "opening the menu disarms a pending delete"
      );

      state.toggle_copy_menu(10);
      assert_eq!(state.copy_menu(), None);
    }
  }

  mod arm_delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_arms_the_confirm_and_closes_an_open_menu() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.toggle_copy_menu(11);

      state.arm_delete(10);

      assert_eq!(state.confirm_delete(), Some(10));
      assert_eq!(state.copy_menu(), None);
    }

    #[test]
    fn it_drops_a_stale_confirm_when_the_plan_is_gone() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(10);

      state.set_roster(Roster {
        entries: vec![entry(3, "Cassi", vec![plan(12, "Logi", 3, 3)])],
        templates: Vec::new(),
      });

      assert_eq!(state.confirm_delete(), None);
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_one_key_per_stale_portrait() {
      let roster = Roster {
        entries: vec![
          RosterEntry {
            character_id: 1,
            corp: "TST".to_owned(),
            name: "Aria".to_owned(),
            plans: Vec::new(),
            portrait: images::ImageState::Stale {
              id: 1,
              kind: images::ImageKind::CharacterPortrait,
            },
          },
          entry(2, "Borin", Vec::new()),
        ],
        templates: Vec::new(),
      };

      let keys = roster.stale_images();

      assert_eq!(keys, vec![(images::ImageKind::CharacterPortrait, 1)]);
    }
  }

  mod relative_time {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_buckets_a_parseable_timestamp_into_a_relative_label() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);
      let label = relative_time("2000-01-01T00:00:00Z");

      assert!(label.ends_with("d ago"), "expected a days-ago bucket, got {label}");
    }

    #[test]
    fn it_falls_back_to_the_raw_string_for_an_unparseable_value() {
      assert_eq!(relative_time("not-a-date"), "not-a-date");
    }
  }

  mod milestone_progress {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn step(skill_id: i64, to_level: u8) -> PlanStep {
      PlanStep {
        skill_id,
        to_level,
      }
    }

    fn anchor(after_entry_id: Option<i64>, order: i64) -> MilestoneAnchor {
      MilestoneAnchor {
        after_entry_id,
        order,
      }
    }

    #[test]
    fn it_reports_no_milestones_for_a_plan_without_any() {
      let ids = [10, 11, 12];
      let steps = [step(3300, 1), step(3300, 2), step(3300, 3)];

      let progress = super::super::milestone_progress(&ids, &steps, &[], &HashMap::new());

      assert_eq!(progress, MilestoneProgress::default());
    }

    #[test]
    fn it_marks_a_milestone_done_when_every_segment_skill_is_trained() {
      let ids = [10, 11, 12];
      let steps = [step(3300, 1), step(3300, 2), step(3300, 3)];
      let milestones = [anchor(None, 0)];
      let trained = HashMap::from([(3300, 3)]);

      let progress = super::super::milestone_progress(&ids, &steps, &milestones, &trained);

      assert_eq!(
        progress,
        MilestoneProgress {
          done: 1,
          steps_done: 3,
          steps_total: 3,
          total: 1,
        }
      );
    }

    #[test]
    fn it_leaves_a_partially_trained_milestone_undone() {
      let ids = [10, 11, 12];
      let steps = [step(3300, 1), step(3300, 2), step(3300, 3)];
      let milestones = [anchor(None, 0)];
      let trained = HashMap::from([(3300, 1)]);

      let progress = super::super::milestone_progress(&ids, &steps, &milestones, &trained);

      assert_eq!(
        progress,
        MilestoneProgress {
          done: 0,
          steps_done: 1,
          steps_total: 3,
          total: 1,
        }
      );
    }

    #[test]
    fn it_counts_one_done_milestone_across_two_segments() {
      let ids = [10, 11, 12, 13];
      let steps = [step(3300, 1), step(3300, 2), step(3301, 1), step(3301, 2)];
      let milestones = [anchor(None, 0), anchor(Some(11), 0)];
      let trained = HashMap::from([(3300, 2)]);

      let progress = super::super::milestone_progress(&ids, &steps, &milestones, &trained);

      assert_eq!(
        progress,
        MilestoneProgress {
          done: 1,
          steps_done: 2,
          steps_total: 4,
          total: 2,
        },
        "the first segment is fully trained; the second is untrained"
      );
    }

    #[test]
    fn it_counts_an_empty_segment_milestone_toward_total_but_never_done() {
      let ids = [10, 11];
      let steps = [step(3300, 1), step(3300, 2)];
      let milestones = [anchor(Some(11), 0)];
      let trained = HashMap::from([(3300, 5)]);

      let progress = super::super::milestone_progress(&ids, &steps, &milestones, &trained);

      assert_eq!(
        progress,
        MilestoneProgress {
          done: 0,
          steps_done: 0,
          steps_total: 0,
          total: 1,
        },
        "a milestone anchored past the last entry owns an empty segment"
      );
    }
  }

  mod load_roster {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self, Database,
      model::{Alliance, Bloodline, Character, CharacterSkill, Corporation, Gender, OwnerType, Race},
      repo::{character, infra, skills},
    };

    async fn seed_owned(db: &Database, id: i64, name: &str) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_builds_a_per_character_plan_count_and_header_totals() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, 42, "Aria").await;
      seed_owned(&db, 7, "Borin").await;
      let plan = skills::create(&db, 42, "Combat").await.unwrap();
      skills::insert_entry(&db, plan.id(), 3300, 5).await.unwrap();
      skills::create(&db, 42, "Industry").await.unwrap();
      character::replace_skills(
        &db,
        42,
        &[CharacterSkill {
          active_skill_level: 5,
          character_id: 42,
          skill_id: 3300,
          skillpoints_in_skill: 256_000,
          trained_skill_level: 5,
        }],
      )
      .await
      .unwrap();

      let roster = load_roster(&db).await;

      assert_eq!(roster.entries.len(), 2);
      assert_eq!(roster.plan_total(), 2);
      let aria = roster.entries.iter().find(|entry| entry.character_id == 42).unwrap();
      assert_eq!(aria.corp, "TSC");
      assert_eq!(aria.plans.len(), 2);
      let combat = aria.plans.iter().find(|plan| plan.name == "Combat").unwrap();
      assert_eq!(combat.entry_count, 1);
      assert_eq!(
        combat.remaining_steps, 0,
        "a plan whose only skill is fully trained reports no remaining steps"
      );
      let borin = roster.entries.iter().find(|entry| entry.character_id == 7).unwrap();
      assert!(borin.plans.is_empty());
    }

    async fn seed_skill_type(db: &Database, skill_id: i64, name: &str, rank: i64) {
      use crate::store::{
        model::{ItemCategory, ItemGroup, ItemType, SkillMetadata},
        repo::sde,
      };

      sde::upsert_item_category(
        db,
        &ItemCategory {
          id: 16,
          icon_id: None,
          name: "Skill".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sde::upsert_item_group(
        db,
        &ItemGroup {
          category_id: 16,
          icon_id: None,
          id: 255,
          name: "Gunnery".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      sde::upsert_item_type(
        db,
        &ItemType {
          capacity: None,
          description: Some("A skill.".to_owned()),
          dogma_attributes: "[]".to_owned(),
          group_id: 255,
          icon_id: None,
          id: skill_id,
          market_group_id: None,
          name: name.to_owned(),
          packaged_volume: None,
          portion_size: None,
          published: true,
          radius: None,
          volume: None,
        },
      )
      .await
      .unwrap();
      skills::upsert_skill_metadata(
        db,
        &SkillMetadata {
          primary_attribute: 165,
          rank,
          secondary_attribute: 166,
          skill_id,
        },
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_lists_templates_with_zero_based_sp_and_step_counts() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, 42, "Aria").await;
      seed_skill_type(&db, 3300, "Gunnery", 1).await;
      seed_skill_type(&db, 3301, "Sharpshooter", 2).await;
      let template = skills::create_template(&db, "Doctrine").await.unwrap();
      skills::insert_entry(&db, template.id(), 3300, 3).await.unwrap();
      skills::insert_entry(&db, template.id(), 3300, 5).await.unwrap();
      skills::insert_entry(&db, template.id(), 3301, 1).await.unwrap();

      let roster = load_roster(&db).await;

      assert_eq!(roster.templates.len(), 1);
      let row = &roster.templates[0];
      assert_eq!(row.name, "Doctrine");
      assert_eq!(row.step_count, 3);
      assert_eq!(
        row.total_sp, 256_500,
        "a repeated skill costs its highest level once, zero-based"
      );
      let aria = roster.entries.iter().find(|entry| entry.character_id == 42).unwrap();
      assert!(aria.plans.is_empty(), "templates never appear in a character's plans");
    }

    #[tokio::test]
    async fn it_costs_template_training_time_against_the_unmapped_baseline() {
      let db = store::open_test().await.unwrap();
      seed_skill_type(&db, 3300, "Gunnery", 1).await;
      seed_skill_type(&db, 3301, "Sharpshooter", 3).await;
      let template = skills::create_template(&db, "Doctrine").await.unwrap();
      skills::insert_entry(&db, template.id(), 3300, 5).await.unwrap();
      skills::insert_entry(&db, template.id(), 3301, 4).await.unwrap();

      let roster = load_roster(&db).await;

      let row = &roster.templates[0];
      let expected = plan_math::template_plan(
        &[
          PlanEntry {
            partial_sp_at_from: 0,
            primary: Attribute::Intelligence,
            rank: 1.0,
            secondary: Attribute::Memory,
            skill_id: 3300,
            synced_trained_level: 0,
            to_level: 5,
          },
          PlanEntry {
            partial_sp_at_from: 0,
            primary: Attribute::Intelligence,
            rank: 3.0,
            secondary: Attribute::Memory,
            skill_id: 3301,
            synced_trained_level: 0,
            to_level: 4,
          },
        ],
        Vec::new(),
      )
      .total_sec;

      assert!(row.total_sec > 0.0, "an unmapped baseline yields real training time");
      assert!((row.total_sec - expected).abs() < 1e-6);
    }

    #[tokio::test]
    async fn it_honors_a_manual_remap_divider_in_the_list_time() {
      let db = store::open_test().await.unwrap();
      seed_skill_type(&db, 3300, "Gunnery", 1).await;
      let template = skills::create_template(&db, "Doctrine").await.unwrap();
      skills::insert_entry(&db, template.id(), 3300, 5).await.unwrap();
      skills::upsert_milestone(
        &db,
        template.id(),
        None,
        "Milestone",
        false,
        0,
        Some((27, 17, 17, 21, 17)),
      )
      .await
      .unwrap();

      let roster = load_roster(&db).await;

      let baseline = plan_math::template_plan(
        &[PlanEntry {
          partial_sp_at_from: 0,
          primary: Attribute::Intelligence,
          rank: 1.0,
          secondary: Attribute::Memory,
          skill_id: 3300,
          synced_trained_level: 0,
          to_level: 5,
        }],
        Vec::new(),
      )
      .total_sec;
      let remapped = plan_math::template_plan(
        &[PlanEntry {
          partial_sp_at_from: 0,
          primary: Attribute::Intelligence,
          rank: 1.0,
          secondary: Attribute::Memory,
          skill_id: 3300,
          synced_trained_level: 0,
          to_level: 5,
        }],
        vec![RemapPoint {
          after_index: -1,
          base: Attributes {
            charisma: 17,
            intelligence: 21,
            memory: 17,
            perception: 27,
            willpower: 17,
          },
        }],
      )
      .total_sec;

      let row = &roster.templates[0];
      assert!((row.total_sec - remapped).abs() < 1e-6);
      assert!(
        row.total_sec < baseline,
        "a favorable remap divider shortens the listed time"
      );
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_master_detail_window() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.set_tab(Tab::Characters);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_empty_detail_for_a_character_without_plans() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.set_tab(Tab::Characters);
      state.select(2);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_an_empty_roster() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_templates_tab_with_cards() {
      let mut state = State::new();
      state.set_roster(super::roster());

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_plan_card_with_milestone_progress() {
      let mut plan = super::plan(30, "Milestoned", 4, 2);
      plan.milestones = MilestoneProgress {
        done: 1,
        steps_done: 2,
        steps_total: 4,
        total: 2,
      };
      let mut state = State::new();
      state.set_roster(Roster {
        entries: vec![super::entry(1, "Aria", vec![plan])],
        templates: Vec::new(),
      });
      state.set_tab(Tab::Characters);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_template_card_with_the_import_menu_open() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.toggle_copy_menu(50);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_template_delete_confirm() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(50);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_templates_empty_state() {
      let mut state = State::new();
      state.set_roster(Roster {
        entries: vec![super::entry(2, "Borin", Vec::new())],
        templates: Vec::new(),
      });

      let _el: Element<'_, Message> = view(&state);
    }
  }

  mod copy_to_button {
    use super::*;

    #[test]
    fn it_builds_an_enabled_button_with_an_open_menu() {
      let entry = super::entry(2, "Borin", Vec::new());
      let targets = [&entry];
      let _el: Element<'_, Message> =
        super::super::copy_to_button(10, &targets, true, "Copy to".to_owned(), "Copy to character");
    }

    #[test]
    fn it_builds_an_enabled_button_with_a_closed_menu() {
      let entry = super::entry(2, "Borin", Vec::new());
      let targets = [&entry];
      let _el: Element<'_, Message> =
        super::super::copy_to_button(10, &targets, false, "Copy to".to_owned(), "Copy to character");
    }

    #[test]
    fn it_builds_a_disabled_button_when_there_are_no_targets() {
      let _el: Element<'_, Message> =
        super::super::copy_to_button(10, &[], false, "Copy to".to_owned(), "Copy to character");
    }
  }

  mod copy_button_label_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_accent_when_enabled() {
      assert_eq!(super::super::copy_button_label_style(true).color, Some(color::accent()));
    }

    #[test]
    fn it_dims_the_label_when_disabled() {
      assert_eq!(
        super::super::copy_button_label_style(false).color,
        Some(color::text::tertiary())
      );
    }
  }

  mod copy_button_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accents_the_border_for_an_open_menu_even_without_hover() {
      let style = super::super::copy_button_style(true, true, button::Status::Active);

      assert_eq!(style.border.color, color::accent());
      assert_eq!(style.background, None);
    }

    #[test]
    fn it_accents_and_fills_an_enabled_button_on_hover() {
      let style = super::super::copy_button_style(true, false, button::Status::Hovered);

      assert_eq!(style.border.color, color::accent());
      assert!(style.background.is_some());
      assert_eq!(style.text_color, color::accent());
    }

    #[test]
    fn it_uses_the_muted_border_for_an_enabled_resting_button() {
      let style = super::super::copy_button_style(true, false, button::Status::Active);

      assert_eq!(style.border.color, color::accent_muted());
      assert_eq!(style.background, None);
    }

    #[test]
    fn it_dims_the_border_and_text_for_a_disabled_button() {
      let style = super::super::copy_button_style(false, false, button::Status::Hovered);

      assert_eq!(style.border.color, color::with_alpha(color::text::PRIMARY, 0.1));
      assert_eq!(style.text_color, color::text::tertiary());
      assert_eq!(style.background, None, "a disabled button never fills on hover");
    }
  }
}
