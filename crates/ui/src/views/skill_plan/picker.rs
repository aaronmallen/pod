//! Skill picker: three-tab accordion for adding skills, ships, and modules to a plan.

pub mod empty_state;
pub mod group_header;
pub mod result_row;
pub mod search_bar;
pub mod utils;

use std::collections::{HashMap, HashSet};

pub use empty_state::Component as PickerEmptyState;
pub use group_header::Component as PickerGroupHeader;
use iced::{
  Background, Border, Element, Length,
  widget::{Space, column, container, scrollable},
};
use pod_model::{Certificate, ItemTypeSummary};
pub use search_bar::Component as PickerSearchBar;

use super::Message;

static EMPTY_MASTERY_MAP: std::sync::LazyLock<HashMap<i32, u8>> = std::sync::LazyLock::new(HashMap::new);

fn filter_skills<'a>(
  skills: &'a [crate::views::skills::skill_data::SkillDef],
  lc: &str,
  searching: bool,
) -> Vec<&'a crate::views::skills::skill_data::SkillDef> {
  if searching {
    skills.iter().filter(|s| s.name.to_lowercase().contains(lc)).collect()
  } else {
    skills.iter().collect()
  }
}

fn count_trained(skills: &[crate::views::skills::skill_data::SkillDef]) -> usize {
  skills.iter().filter(|s| s.level >= 5).count()
}

use crate::{
  components::tab_strip::{Component as TabStrip, TabItem},
  style::color,
  views::skills::skill_data::SkillGroupDef,
};

/// Builder for the full skill picker panel with tab navigation.
pub struct SkillPicker<'a> {
  cert_proficiency_selection: &'a HashMap<i32, u8>,
  certs: &'a [Certificate],
  certs_loaded: bool,
  expanded_groups: &'a HashSet<String>,
  groups: &'a [SkillGroupDef],
  modules: &'a [ItemTypeSummary],
  modules_loaded: bool,
  picker_tab: usize,
  planned_levels: &'a HashMap<String, u8>,
  search_query: &'a str,
  ship_mastery_selection: &'a HashMap<i32, u8>,
  ships: &'a [ItemTypeSummary],
  ships_loaded: bool,
}

impl<'a> SkillPicker<'a> {
  /// Creates a new skill picker with the given skill groups and plan state.
  pub fn new(
    groups: &'a [SkillGroupDef],
    planned_levels: &'a HashMap<String, u8>,
    search_query: &'a str,
    expanded_groups: &'a HashSet<String>,
  ) -> Self {
    Self {
      cert_proficiency_selection: &EMPTY_MASTERY_MAP,
      certs: &[],
      certs_loaded: false,
      expanded_groups,
      groups,
      modules: &[],
      modules_loaded: false,
      picker_tab: 0,
      planned_levels,
      search_query,
      ship_mastery_selection: &EMPTY_MASTERY_MAP,
      ships: &[],
      ships_loaded: false,
    }
  }

  /// Sets the certificate list and proficiency selection.
  pub fn certs(mut self, certs: &'a [Certificate], proficiency: &'a HashMap<i32, u8>, loaded: bool) -> Self {
    self.cert_proficiency_selection = proficiency;
    self.certs = certs;
    self.certs_loaded = loaded;
    self
  }

  /// Sets the module list.
  pub fn modules(mut self, modules: &'a [ItemTypeSummary], loaded: bool) -> Self {
    self.modules = modules;
    self.modules_loaded = loaded;
    self
  }

  /// Sets the active tab index.
  pub fn tab(mut self, tab: usize) -> Self {
    self.picker_tab = tab;
    self
  }

  /// Sets the ship list and mastery selection.
  pub fn ships(mut self, ships: &'a [ItemTypeSummary], mastery_selection: &'a HashMap<i32, u8>, loaded: bool) -> Self {
    self.ship_mastery_selection = mastery_selection;
    self.ships = ships;
    self.ships_loaded = loaded;
    self
  }

  /// Renders the skill picker panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let tabs = TabStrip::new(vec![
      TabItem {
        label: "Skills".to_string(),
        count: None,
      },
      TabItem {
        label: "Ships".to_string(),
        count: None,
      },
      TabItem {
        label: "Modules".to_string(),
        count: None,
      },
      TabItem {
        label: "Certs".to_string(),
        count: None,
      },
    ])
    .active(self.picker_tab)
    .render(Message::PickerTabChanged);

    let body: Element<'_, Message> = match self.picker_tab {
      1 => self.ships_tab(),
      2 => self.modules_tab(),
      3 => self.certs_tab(),
      _ => self.skills_tab(),
    };

    let content = column([tabs, body]).height(Length::Fill).width(Length::Fill);

    container(content)
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::SUBTLE,
          radius: 0.0.into(),
          width: 0.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn certs_tab(self) -> Element<'a, Message> {
    if !self.certs_loaded {
      return PickerEmptyState::new("Loading certificates\u{2026}").render();
    }

    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();
    let mut items: Vec<Element<'_, Message>> =
      vec![PickerSearchBar::new(self.search_query, "Search certificates\u{2026}").render()];

    for cert in self.certs {
      if searching && !cert.name.to_lowercase().contains(&lc) {
        continue;
      }
      let prof = self.cert_proficiency_selection.get(&cert.id).copied().unwrap_or(3);
      items.push(result_row::CertRow::new(cert, prof).render());
    }

    items.push(Space::new().height(12.0).into());
    scrollable(column(items).width(Length::Fill))
      .height(Length::Fill)
      .width(Length::Fill)
      .into()
  }

  fn modules_tab(self) -> Element<'a, Message> {
    if !self.modules_loaded {
      return PickerEmptyState::new("Loading modules\u{2026}").render();
    }
    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();
    let mut items: Vec<Element<'_, Message>> =
      vec![PickerSearchBar::new(self.search_query, "Search modules\u{2026}").render()];
    let groups = utils::collect_item_groups(self.modules, &lc, searching);
    for (group_name, mods) in &groups {
      let is_expanded = searching || self.expanded_groups.contains(*group_name);
      items.push(PickerGroupHeader::dynamic(group_name, mods.len(), is_expanded).render());
      if is_expanded {
        for module in mods {
          items.push(result_row::ModuleRow::new(module).render());
        }
      }
    }
    items.push(Space::new().height(12.0).into());
    scrollable(column(items).width(Length::Fill))
      .height(Length::Fill)
      .width(Length::Fill)
      .into()
  }

  fn ships_tab(self) -> Element<'a, Message> {
    if !self.ships_loaded {
      return PickerEmptyState::new("Loading ships\u{2026}").render();
    }
    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();
    let mut items: Vec<Element<'_, Message>> =
      vec![PickerSearchBar::new(self.search_query, "Search ships\u{2026}").render()];
    let groups = utils::collect_item_groups(self.ships, &lc, searching);
    for (group_name, ships) in &groups {
      let is_expanded = searching || self.expanded_groups.contains(*group_name);
      items.push(PickerGroupHeader::dynamic(group_name, ships.len(), is_expanded).render());
      if is_expanded {
        for ship in ships {
          let mastery = self.ship_mastery_selection.get(&ship.id).copied().unwrap_or(1);
          items.push(result_row::ShipRow::new(ship, mastery).render());
        }
      }
    }
    items.push(Space::new().height(12.0).into());
    scrollable(column(items).width(Length::Fill))
      .height(Length::Fill)
      .width(Length::Fill)
      .into()
  }

  fn skills_tab(self) -> Element<'a, Message> {
    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();

    let mut items: Vec<Element<'_, Message>> =
      vec![PickerSearchBar::new(self.search_query, "Search skills\u{2026}").render()];

    for group in self.groups {
      let filtered = filter_skills(&group.skills, &lc, searching);
      if filtered.is_empty() {
        continue;
      }

      let is_expanded = searching || self.expanded_groups.contains(&*group.name);
      let trained_count = count_trained(&group.skills);
      items.push(PickerGroupHeader::new(&group.name, is_expanded, trained_count, group.skills.len()).render());

      if is_expanded {
        self.push_skill_rows(&mut items, &filtered);
      }
    }

    items.push(Space::new().height(12.0).into());
    let content = column(items).width(Length::Fill);
    scrollable(content).height(Length::Fill).width(Length::Fill).into()
  }

  fn push_skill_rows<'b>(
    &self,
    items: &mut Vec<Element<'b, Message>>,
    skills: &[&'b crate::views::skills::skill_data::SkillDef],
  ) where
    'a: 'b,
  {
    for skill in skills {
      let planned = self.planned_levels.get(skill.name.as_str()).copied().unwrap_or(0);
      items.push(result_row::SkillRow::new(skill, planned).render());
    }
  }
}
