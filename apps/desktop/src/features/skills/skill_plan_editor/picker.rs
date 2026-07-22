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
    let resolved = match self {
      PickerTab::Certs => t!("skills.editor_picker.tab_certs"),
      PickerTab::Modules => t!("skills.editor_picker.tab_modules"),
      PickerTab::Ships => t!("skills.editor_picker.tab_ships"),
      PickerTab::Skills => t!("skills.editor_picker.tab_skills"),
    };
    intern_tab_label(&resolved)
  }
}

/// Interns a resolved tab label as a `'static` string.
///
/// [`Tab::label`](crate::ui::components::tab_select::Tab) borrows for the
/// lifetime of the rendered element, but `t!` yields an owned, locale-dependent
/// value. Each distinct resolved label is interned once so the borrow stays
/// valid for the program lifetime; the pool is bounded by the four tabs times
/// the installed locales.
fn intern_tab_label(value: &str) -> &'static str {
  use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
  };

  static POOL: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
  let pool = POOL.get_or_init(|| Mutex::new(HashSet::new()));
  let mut guard = pool.lock().expect("tab label pool poisoned");
  if let Some(existing) = guard.get(value) {
    return existing;
  }
  let leaked: &'static str = Box::leak(value.to_owned().into_boxed_str());
  guard.insert(leaked);
  leaked
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
      None => empty_state::empty_state(t!("skills.editor_picker.loading_skills").into_owned()),
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
  scrollable_list(skills_tab_items(catalog, state, planned))
}

fn skills_tab_items<'a>(
  catalog: &'a SkillCatalog,
  state: &'a PickerState,
  planned: &HashMap<i64, u8>,
) -> Vec<Element<'a, Message>> {
  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();

  let mut items = vec![search_bar_item(
    &state.query,
    t!("skills.editor_picker.search_skills").into_owned(),
  )];

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
    items.push(empty_state::empty_state(
      t!("skills.editor_picker.no_skills_match").into_owned(),
    ));
  }

  items
}

fn ships_tab(state: &PickerState) -> Element<'_, Message> {
  let Some(ships) = state.ships.as_ref() else {
    return empty_state::empty_state(t!("skills.editor_picker.loading_ships").into_owned());
  };

  scrollable_list(ships_tab_items(ships, state))
}

fn ships_tab_items<'a>(ships: &'a [PickerShip], state: &'a PickerState) -> Vec<Element<'a, Message>> {
  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();
  let mut items = vec![search_bar_item(
    &state.query,
    t!("skills.editor_picker.search_ships").into_owned(),
  )];

  let groups = group_by_item_group(
    ships,
    |s| (s.group_id, s.group_name.as_str(), s.name.as_str()),
    &query,
    searching,
  );
  if groups.is_empty() {
    items.push(empty_state::empty_state(
      t!("skills.editor_picker.no_ships_match").into_owned(),
    ));
    return items;
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

  items
}

fn modules_tab(state: &PickerState) -> Element<'_, Message> {
  let Some(modules) = state.modules.as_ref() else {
    return empty_state::empty_state(t!("skills.editor_picker.loading_modules").into_owned());
  };

  scrollable_list(modules_tab_items(modules, state))
}

fn modules_tab_items<'a>(modules: &'a [PickerModule], state: &'a PickerState) -> Vec<Element<'a, Message>> {
  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();
  let mut items = vec![search_bar_item(
    &state.query,
    t!("skills.editor_picker.search_modules").into_owned(),
  )];

  let groups = group_by_item_group(
    modules,
    |m| (m.group_id, m.group_name.as_str(), m.name.as_str()),
    &query,
    searching,
  );
  if groups.is_empty() {
    items.push(empty_state::empty_state(
      t!("skills.editor_picker.no_modules_match").into_owned(),
    ));
    return items;
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

  items
}

fn certs_tab(state: &PickerState) -> Element<'_, Message> {
  let Some(certs) = state.certs.as_ref() else {
    return empty_state::empty_state(t!("skills.editor_picker.loading_certificates").into_owned());
  };

  scrollable_list(certs_tab_items(certs, state))
}

fn certs_tab_items<'a>(certs: &'a [PickerCert], state: &'a PickerState) -> Vec<Element<'a, Message>> {
  let query = state.query.trim().to_lowercase();
  let searching = !query.is_empty();
  let mut items = vec![search_bar_item(
    &state.query,
    t!("skills.editor_picker.search_certificates").into_owned(),
  )];

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
    items.push(empty_state::empty_state(
      t!("skills.editor_picker.no_certificates_match").into_owned(),
    ));
  }

  items
}

fn search_bar_item<'a>(query: &'a str, placeholder: String) -> Element<'a, Message> {
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::skills::browse::{AttrKey, SkillCatalogEntry, SkillCatalogGroup};

  fn catalog() -> SkillCatalog {
    SkillCatalog {
      groups: vec![SkillCatalogGroup {
        id: 255,
        name: "Gunnery".to_owned(),
        skills: vec![SkillCatalogEntry {
          group_id: 255,
          group_name: "Gunnery".to_owned(),
          name: "Gunnery".to_owned(),
          prereqs: vec![],
          primary_attr: AttrKey::Perception,
          rank: 1,
          secondary_attr: AttrKey::Willpower,
          type_id: 3300,
        }],
      }],
    }
  }

  fn cert() -> PickerCert {
    PickerCert {
      grade: 1,
      id: 1,
      name: "Gunnery Basics".to_owned(),
      skills: vec![],
    }
  }

  fn module() -> PickerModule {
    PickerModule {
      group_id: 55,
      group_name: "Projectile Weapon".to_owned(),
      id: 12_058,
      name: "125mm Gatling AutoCannon".to_owned(),
      requirements: vec![],
    }
  }

  fn ship() -> PickerShip {
    PickerShip {
      group_id: 25,
      group_name: "Frigate".to_owned(),
      id: 587,
      name: "Rifter".to_owned(),
      own_requirements: vec![],
      tier_cert_skills: vec![],
    }
  }

  fn state_with_query(query: &str) -> PickerState {
    PickerState {
      query: query.to_owned(),
      ..PickerState::default()
    }
  }

  mod certs_tab_items {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_the_search_bar_and_appends_the_empty_message_on_zero_results() {
      let certs = vec![cert()];
      let state = state_with_query("nonsense");

      let items = certs_tab_items(&certs, &state);

      assert_eq!(items.len(), 2, "search bar plus the empty message remain");
    }

    #[test]
    fn it_renders_results_without_the_empty_message_when_the_query_matches() {
      let certs = vec![
        cert(),
        PickerCert {
          grade: 1,
          id: 2,
          name: "Gunnery Specialist".to_owned(),
          skills: vec![],
        },
      ];
      let state = state_with_query("gunnery");

      let items = certs_tab_items(&certs, &state);

      assert!(items.len() > 2, "search bar plus matching rows, no empty message");
    }
  }

  mod modules_tab_items {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_the_search_bar_and_appends_the_empty_message_on_zero_results() {
      let modules = vec![module()];
      let state = state_with_query("nonsense");

      let items = modules_tab_items(&modules, &state);

      assert_eq!(items.len(), 2, "search bar plus the empty message remain");
    }

    #[test]
    fn it_renders_results_without_the_empty_message_when_the_query_matches() {
      let modules = vec![module()];
      let state = state_with_query("autocannon");

      let items = modules_tab_items(&modules, &state);

      assert!(items.len() > 2, "search bar plus matching rows, no empty message");
    }
  }

  mod ships_tab_items {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_the_search_bar_and_appends_the_empty_message_on_zero_results() {
      let ships = vec![ship()];
      let state = state_with_query("nonsense");

      let items = ships_tab_items(&ships, &state);

      assert_eq!(items.len(), 2, "search bar plus the empty message remain");
    }

    #[test]
    fn it_renders_results_without_the_empty_message_when_the_query_matches() {
      let ships = vec![ship()];
      let state = state_with_query("rifter");

      let items = ships_tab_items(&ships, &state);

      assert!(items.len() > 2, "search bar plus matching rows, no empty message");
    }
  }

  mod skills_tab_items {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_the_search_bar_and_appends_the_empty_message_on_zero_results() {
      let catalog = catalog();
      let state = state_with_query("nonsense");

      let items = skills_tab_items(&catalog, &state, &HashMap::new());

      assert_eq!(items.len(), 2, "search bar plus the empty message remain");
    }

    #[test]
    fn it_renders_results_without_the_empty_message_when_the_query_matches() {
      let catalog = catalog();
      let state = state_with_query("gunnery");

      let items = skills_tab_items(&catalog, &state, &HashMap::new());

      assert!(items.len() > 2, "search bar plus matching rows, no empty message");
    }
  }
}
