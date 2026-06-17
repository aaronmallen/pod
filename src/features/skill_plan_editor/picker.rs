use std::collections::{HashMap, HashSet};

use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Column, Space, column, container, scrollable},
};

use super::Message;
use crate::{
  features::skills::browse::SkillCatalog,
  store::model::CertificateSkill,
  ui::style::{color, spacing},
};

pub(super) mod empty_state;
pub(super) mod group_header;
pub(super) mod item_row;
pub(super) mod result_row;
pub(super) mod search_bar;
pub(super) mod tabs;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PickerTab {
  Certs,
  Modules,
  Ships,
  #[default]
  Skills,
}

impl PickerTab {
  pub(super) const ALL: [PickerTab; 4] = [
    PickerTab::Skills,
    PickerTab::Ships,
    PickerTab::Modules,
    PickerTab::Certs,
  ];

  pub(super) fn label(self) -> &'static str {
    match self {
      PickerTab::Certs => "Certs",
      PickerTab::Modules => "Modules",
      PickerTab::Ships => "Ships",
      PickerTab::Skills => "Skills",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickerShip {
  pub group_id: i64,
  pub group_name: String,
  pub id: i64,
  pub name: String,
  pub own_requirements: Vec<(i64, u8)>,
  pub tier_cert_skills: Vec<Vec<CertificateSkill>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickerModule {
  pub group_id: i64,
  pub group_name: String,
  pub id: i64,
  pub name: String,
  pub requirements: Vec<(i64, u8)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickerCert {
  pub grade: i64,
  pub id: i64,
  pub name: String,
  pub skills: Vec<CertificateSkill>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PickerState {
  pub active_tab: PickerTab,
  pub catalog: Option<SkillCatalog>,
  pub cert_proficiency: HashMap<i64, usize>,
  pub certs: Option<Vec<PickerCert>>,
  pub expanded_groups: HashSet<i64>,
  pub modules: Option<Vec<PickerModule>>,
  pub query: String,
  pub ship_mastery: HashMap<i64, u8>,
  pub ships: Option<Vec<PickerShip>>,
  pub trained_levels: HashMap<i64, u8>,
}

pub(super) fn picker<'a>(state: &'a PickerState, planned: &HashMap<i64, u8>) -> Element<'a, Message> {
  let body: Element<'a, Message> = match state.active_tab {
    PickerTab::Skills => match state.catalog.as_ref() {
      None => empty_state::empty_state("Loading skills\u{2026}"),
      Some(catalog) => skills_tab(catalog, state, planned),
    },
    PickerTab::Ships => ships_tab(state),
    PickerTab::Modules => modules_tab(state),
    PickerTab::Certs => certs_tab(state),
  };

  let content = Column::with_children(vec![tabs::tabs(state.active_tab), body])
    .height(Length::Fill)
    .width(Length::Fill);

  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn skills_tab<'a>(
  catalog: &'a SkillCatalog,
  state: &'a PickerState,
  planned: &HashMap<i64, u8>,
) -> Element<'a, Message> {
  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();

  let mut items: Vec<Element<'a, Message>> = vec![
    container(search_bar::search_bar(&state.query, "Search skills\u{2026}"))
      .padding(Padding {
        top: spacing::SPACE_3,
        bottom: spacing::SPACE_3,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .width(Length::Fill)
      .into(),
  ];

  let mut any_visible = false;
  for group in &catalog.groups {
    let visible: Vec<&crate::features::skills::browse::SkillCatalogEntry> = if searching {
      group
        .skills
        .iter()
        .filter(|skill| skill.name.to_lowercase().contains(&query))
        .collect()
    } else {
      group.skills.iter().collect()
    };
    if visible.is_empty() {
      continue;
    }
    any_visible = true;

    let open = searching || state.expanded_groups.contains(&group.id);
    let trained_count = group
      .skills
      .iter()
      .filter(|skill| state.trained_levels.get(&skill.type_id).copied().unwrap_or(0) >= 5)
      .count();

    items.push(group_header::group_header(
      group.id,
      &group.name,
      trained_count,
      group.skills.len(),
      open,
    ));

    if open {
      for skill in visible {
        let trained = state.trained_levels.get(&skill.type_id).copied().unwrap_or(0);
        let planned_level = planned.get(&skill.type_id).copied().unwrap_or(0);
        items.push(result_row::result_row(
          skill.type_id,
          &skill.name,
          skill.rank,
          trained,
          planned_level,
        ));
      }
    }
  }

  if !any_visible {
    return empty_state::empty_state("No skills match your search");
  }

  items.push(Space::new().height(12.0).into());
  scrollable(column(items).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn ships_tab(state: &PickerState) -> Element<'_, Message> {
  let Some(ships) = state.ships.as_ref() else {
    return empty_state::empty_state("Loading ships\u{2026}");
  };

  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();
  let mut items = vec![search_bar_item(&state.query, "Search ships\u{2026}")];

  let groups = group_by_item_group(
    ships,
    |s| (s.group_id, s.group_name.as_str(), s.name.as_str()),
    &query,
    searching,
  );
  if groups.is_empty() {
    return empty_state::empty_state("No ships match your search");
  }

  for (group_id, group_name, members) in groups {
    let open = searching || state.expanded_groups.contains(&group_id);
    items.push(group_header::group_header(group_id, group_name, 0, members.len(), open));
    if open {
      for ship in members {
        let tier = state.ship_mastery.get(&ship.id).copied().unwrap_or(1);
        items.push(item_row::ship_row(ship.id, &ship.name, tier));
      }
    }
  }

  scrollable_list(items)
}

fn modules_tab(state: &PickerState) -> Element<'_, Message> {
  let Some(modules) = state.modules.as_ref() else {
    return empty_state::empty_state("Loading modules\u{2026}");
  };

  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();
  let mut items = vec![search_bar_item(&state.query, "Search modules\u{2026}")];

  let groups = group_by_item_group(
    modules,
    |m| (m.group_id, m.group_name.as_str(), m.name.as_str()),
    &query,
    searching,
  );
  if groups.is_empty() {
    return empty_state::empty_state("No modules match your search");
  }

  for (group_id, group_name, members) in groups {
    let open = searching || state.expanded_groups.contains(&group_id);
    items.push(group_header::group_header(group_id, group_name, 0, members.len(), open));
    if open {
      for module in members {
        items.push(item_row::module_row(module.id, &module.name));
      }
    }
  }

  scrollable_list(items)
}

fn certs_tab(state: &PickerState) -> Element<'_, Message> {
  let Some(certs) = state.certs.as_ref() else {
    return empty_state::empty_state("Loading certificates\u{2026}");
  };

  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();
  let mut items = vec![search_bar_item(&state.query, "Search certificates\u{2026}")];

  let mut any_visible = false;
  for cert in certs {
    if searching && !cert.name.to_lowercase().contains(&query) {
      continue;
    }
    any_visible = true;
    let prof = state.cert_proficiency.get(&cert.id).copied().unwrap_or(0);
    items.push(item_row::cert_row(cert.id, &cert.name, prof));
  }

  if !any_visible {
    return empty_state::empty_state("No certificates match your search");
  }

  scrollable_list(items)
}

fn search_bar_item<'a>(query: &'a str, placeholder: &'a str) -> Element<'a, Message> {
  container(search_bar::search_bar(query, placeholder))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .width(Length::Fill)
    .into()
}

fn scrollable_list(mut items: Vec<Element<'_, Message>>) -> Element<'_, Message> {
  items.push(Space::new().height(12.0).into());
  scrollable(column(items).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn group_by_item_group<'a, T>(
  items: &'a [T],
  key: impl Fn(&'a T) -> (i64, &'a str, &'a str),
  query: &str,
  searching: bool,
) -> Vec<(i64, &'a str, Vec<&'a T>)> {
  let mut groups: Vec<(i64, &'a str, Vec<&'a T>)> = Vec::new();
  for item in items {
    let (group_id, group_name, item_name) = key(item);
    if searching && !item_name.to_lowercase().contains(query) {
      continue;
    }
    match groups.iter_mut().find(|(id, _, _)| *id == group_id) {
      Some((_, _, members)) => members.push(item),
      None => groups.push((group_id, group_name, vec![item])),
    }
  }
  groups
}
