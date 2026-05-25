//! Skill group accordion component.

pub mod group_header;
pub mod search_bar;
pub mod skill_row;

pub use group_header::Component as GroupHeader;
use iced::{
  Element, Length,
  widget::{Space, column},
};
pub use search_bar::SearchBar;
pub use skill_row::Component as SkillRow;

use super::super::{State, queue_levels};

/// Messages produced by the browser tab.
#[derive(Clone, Debug)]
pub enum Message {
  GroupToggle(String),
  SearchChanged(String),
}

/// Browser tab body component for the skills panel.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new [`Component`] bound to the given view [`State`].
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the browser tab into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let q_levels = queue_levels(&self.state.queue);
    let lc = self.state.search_query.trim().to_lowercase();
    let mut group_els: Vec<Element<'_, Message>> = vec![SearchBar::new(&self.state.search_query).render()];
    for group in &self.state.skill_groups {
      self.append_group_els(&mut group_els, group, &lc, &q_levels);
    }
    group_els.push(Space::new().height(12.0).into());
    column(group_els).width(Length::Fill).into()
  }

  fn append_group_els<'g>(
    &self,
    group_els: &mut Vec<Element<'_, Message>>,
    group: &'g pod_model::SkillGroupDef,
    lc: &str,
    q_levels: &std::collections::HashMap<String, u8>,
  ) where
    'a: 'g,
  {
    let filtered = filter_group_skills(group, lc);
    if filtered.is_empty() {
      return;
    }
    let total_sp = group_total_sp(group, &self.state.char_skill_map);
    let trained_count = group_trained_count(group, &self.state.char_skill_map);
    let is_expanded = self.state.expanded_groups.contains(&group.id);
    group_els.push(
      GroupHeader::new(
        &group.name,
        is_expanded,
        trained_count,
        group.skills.len(),
        total_sp,
        group.id.clone(),
      )
      .render(),
    );
    if is_expanded {
      self.push_expanded_skill_rows(group_els, &filtered, q_levels);
    }
  }

  fn push_expanded_skill_rows<'g>(
    &self,
    group_els: &mut Vec<Element<'_, Message>>,
    filtered: &[&'g pod_model::SkillDef],
    q_levels: &std::collections::HashMap<String, u8>,
  ) where
    'a: 'g,
  {
    for skill in filtered {
      let char_level = self
        .state
        .char_skill_map
        .get(skill.name.as_str())
        .map(|(l, _)| *l)
        .unwrap_or(0);
      let queue_level = q_levels.get(skill.name.as_str()).copied().unwrap_or(char_level);
      group_els.push(SkillRow::new(skill, char_level, queue_level).render());
    }
  }
}

fn filter_group_skills<'g>(group: &'g pod_model::SkillGroupDef, lc: &str) -> Vec<&'g pod_model::SkillDef> {
  if lc.is_empty() {
    group.skills.iter().collect()
  } else {
    group
      .skills
      .iter()
      .filter(|s| s.name.to_lowercase().contains(lc))
      .collect()
  }
}

fn group_total_sp(
  group: &pod_model::SkillGroupDef,
  char_skill_map: &std::collections::HashMap<String, (u8, i64)>,
) -> u64 {
  group
    .skills
    .iter()
    .map(|s| {
      char_skill_map
        .get(s.name.as_str())
        .map(|(_, sp)| *sp as u64)
        .unwrap_or(0)
    })
    .sum()
}

fn group_trained_count(
  group: &pod_model::SkillGroupDef,
  char_skill_map: &std::collections::HashMap<String, (u8, i64)>,
) -> usize {
  group
    .skills
    .iter()
    .filter(|s| {
      char_skill_map
        .get(s.name.as_str())
        .map(|(l, _)| *l >= 5)
        .unwrap_or(false)
    })
    .count()
}
