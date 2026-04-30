//! Skill group accordion component.

pub mod group_header;
pub mod skill_row;

pub use group_header::Component as GroupHeader;
use iced::{
  Element, Length, Padding,
  widget::{Space, column, container},
};
pub use skill_row::Component as SkillRow;

use super::super::{State, queue_levels};
use crate::{
  components::SearchBox,
  style::{color, spacing},
};

/// Messages produced by the browser tab.
#[derive(Clone, Debug)]
pub enum Message {
  GroupToggle(String),
  SearchChanged(String),
}

pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let q_levels = queue_levels(&self.state.queue);
    let lc = self.state.search_query.trim().to_lowercase();

    let mut group_els: Vec<Element<'_, Message>> = vec![search_bar(&self.state.search_query)];

    for group in &self.state.skill_groups {
      let filtered: Vec<_> = if lc.is_empty() {
        group.skills.iter().collect()
      } else {
        group
          .skills
          .iter()
          .filter(|s| s.name.to_lowercase().contains(&lc))
          .collect()
      };

      if filtered.is_empty() {
        continue;
      }

      let total_sp: u64 = group
        .skills
        .iter()
        .map(|s| {
          self
            .state
            .char_skill_map
            .get(s.name.as_str())
            .map(|(_, sp)| *sp as u64)
            .unwrap_or(0)
        })
        .sum();
      let trained_count = group
        .skills
        .iter()
        .filter(|s| {
          self
            .state
            .char_skill_map
            .get(s.name.as_str())
            .map(|(l, _)| *l >= 5)
            .unwrap_or(false)
        })
        .count();
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
        for skill in &filtered {
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

    group_els.push(Space::new().height(12.0).into());

    column(group_els).width(Length::Fill).into()
  }
}

fn search_bar<'a>(query: &'a str) -> Element<'a, Message> {
  let search_box = SearchBox::new("Search skills…", query, Message::SearchChanged)
    .height(36.0)
    .icon_size(14.0)
    .icon_spacing(10.0)
    .horizontal_padding(spacing::SPACE_3)
    .background(color::surface::BASE)
    .render();

  container(search_box)
    .padding(Padding {
      top: 14.0,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}
