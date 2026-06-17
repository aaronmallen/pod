use std::collections::HashMap;

pub mod group_header;
pub mod search_bar;
pub mod skill_row;

use chrono::{DateTime, Utc};
use iced::{
  Element, Length, Padding,
  widget::{Column, Space, scrollable},
};

use super::super::browse::{AttrKey, GroupRow, SkillLeaf, build_browser_tree};
use crate::{
  store::{
    Database,
    repo::{character, skills},
  },
  ui::style::spacing,
};

#[derive(Clone, Debug)]
pub enum Message {
  GroupToggled(i64),
  Loaded(Vec<GroupRow>),
  SearchChanged(String),
  SkillSelected(i64),
}

#[derive(Debug, Default)]
pub struct State {
  collapsed: HashMap<i64, bool>,
  groups: Vec<GroupRow>,
  query: String,
}

impl State {
  pub fn new() -> Self {
    State::default()
  }
}

pub fn load(db: &Database, character_id: i64, now: DateTime<Utc>) -> iced::Task<Message> {
  iced::Task::perform(load_tree(db.clone(), character_id, now), Message::Loaded)
}

pub fn update(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::GroupToggled(group_id) => {
      let entry = state.collapsed.entry(group_id).or_insert(false);
      *entry = !*entry;
      iced::Task::none()
    }
    Message::Loaded(groups) => {
      state.groups = groups;
      state.collapsed = state
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id, index != 0))
        .collect();
      iced::Task::none()
    }
    Message::SearchChanged(query) => {
      state.query = query;
      iced::Task::none()
    }
    Message::SkillSelected(skill_id) => {
      tracing::info!(skill_id, "skill browser leaf selected (no-op until skill plans land)");
      iced::Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  let query = state.query.trim().to_lowercase();

  let mut groups: Vec<Element<'_, Message>> = Vec::with_capacity(state.groups.len() + 1);
  for group in &state.groups {
    if let Some(rendered) = group_row(group, state, &query) {
      groups.push(rendered);
    }
  }
  groups.push(Space::new().height(Length::Fixed(spacing::SPACE_3)).into());

  let body = scrollable(
    Column::with_children(groups)
      .padding(Padding {
        right: spacing::SPACE_2,
        ..Padding::ZERO
      })
      .width(Length::Fill),
  )
  .style(crate::ui::style::control::scrollbar)
  .width(Length::Fill)
  .height(Length::Fill);

  Column::with_children(vec![search_bar::search_box(&state.query), body.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

async fn effective_attrs(db: &Database, character_id: i64) -> [u32; 5] {
  let mut values = [0u32; 5];

  if let Ok(Some(row)) = character::attributes(db, character_id).await {
    values[AttrKey::Perception as usize] = row.perception().max(0) as u32;
    values[AttrKey::Willpower as usize] = row.willpower().max(0) as u32;
    values[AttrKey::Intelligence as usize] = row.intelligence().max(0) as u32;
    values[AttrKey::Memory as usize] = row.memory().max(0) as u32;
    values[AttrKey::Charisma as usize] = row.charisma().max(0) as u32;
  }

  let implants = character::implants(db, character_id).await.unwrap_or_default();
  for implant in implants {
    let bonus = implant.bonus().max(0) as u32;
    let key = AttrKey::from_eve_id(implant.attribute_id().clamp(0, i64::from(u8::MAX)) as u8);
    values[key as usize] += bonus;
  }

  values
}

fn group_row<'a>(group: &'a GroupRow, state: &State, query: &str) -> Option<Element<'a, Message>> {
  let visible: Vec<&SkillLeaf> = if query.is_empty() {
    group.leaves.iter().collect()
  } else {
    group
      .leaves
      .iter()
      .filter(|leaf| leaf.name.to_lowercase().contains(query))
      .collect()
  };

  if visible.is_empty() {
    return None;
  }

  let open = if query.is_empty() {
    !state.collapsed.get(&group.id).copied().unwrap_or(true)
  } else {
    true
  };

  let mut children: Vec<Element<'a, Message>> = vec![group_header::group_header(group, open)];
  if open {
    for leaf in visible {
      children.push(skill_row::skill_row(leaf));
    }
  }

  Some(Column::with_children(children).width(Length::Fill).into())
}

async fn load_tree(db: Database, character_id: i64, now: DateTime<Utc>) -> Vec<GroupRow> {
  let catalog = skills::skill_catalog(&db)
    .await
    .unwrap_or_else(|_| super::super::browse::SkillCatalog {
      groups: Vec::new(),
    });
  let skills = character::skills(&db, character_id).await.unwrap_or_default();
  let queue =
    super::super::queue_timing::active_queue(character::skillqueue(&db, character_id).await.unwrap_or_default(), now);
  let effective_attrs = effective_attrs(&db, character_id).await;

  build_browser_tree(&catalog, &skills, &queue, effective_attrs, now)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{ItemCategory, ItemGroup, ItemType},
    repo::sde,
  };

  const SKILL_CATEGORY_ID: i64 = 16;

  async fn seed_skill(db: &Database, group_id: i64, group_name: &str, skill_id: i64, name: &str) {
    sde::upsert_item_category(
      db,
      &ItemCategory {
        id: SKILL_CATEGORY_ID,
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
        category_id: SKILL_CATEGORY_ID,
        icon_id: None,
        id: group_id,
        name: group_name.to_owned(),
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
        group_id,
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
  }

  fn now() -> DateTime<Utc> {
    use chrono::TimeZone as _;
    Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap()
  }

  mod load_tree {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_loads_a_character_with_no_skills_at_level_zero_without_panicking() {
      let db = store::open_test().await.unwrap();
      seed_skill(&db, 255, "Gunnery", 3300, "Gunnery").await;
      seed_skill(&db, 255, "Gunnery", 3301, "Small Hybrid Turret").await;

      let tree = load_tree(db, 42, now()).await;

      assert_eq!(tree.len(), 1, "one group");
      assert_eq!(tree[0].leaves.len(), 2);
      assert!(
        tree[0].leaves.iter().all(|leaf| leaf.level == 0),
        "every skill is level 0 with no character_skills rows"
      );
      assert_eq!(tree[0].trained_count, 0);
      assert_eq!(tree[0].total_sp, 0);
      assert!(
        tree[0].leaves.iter().all(|leaf| leaf.next_eta == "\u{2014}"),
        "no SP rate → every ETA is the em dash, not a panic or inf"
      );
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    fn loaded_state() -> State {
      let mut state = State::new();
      let groups = vec![
        GroupRow {
          id: 1,
          leaves: Vec::new(),
          name: "Alpha".to_owned(),
          total_skills: 0,
          total_sp: 0,
          trained_count: 0,
        },
        GroupRow {
          id: 2,
          leaves: Vec::new(),
          name: "Beta".to_owned(),
          total_skills: 0,
          total_sp: 0,
          trained_count: 0,
        },
      ];
      let _ = update(&mut state, Message::Loaded(groups));
      state
    }

    #[test]
    fn it_opens_the_first_group_and_collapses_the_rest_on_load() {
      let state = loaded_state();

      assert_eq!(state.collapsed.get(&1), Some(&false), "first group open");
      assert_eq!(state.collapsed.get(&2), Some(&true), "rest collapsed");
    }

    #[test]
    fn it_records_the_search_query() {
      let mut state = loaded_state();

      let _ = update(&mut state, Message::SearchChanged("gun".to_owned()));

      assert_eq!(state.query, "gun");
    }

    #[test]
    fn it_toggles_a_group_open_and_closed() {
      let mut state = loaded_state();

      let _ = update(&mut state, Message::GroupToggled(2));
      assert_eq!(state.collapsed.get(&2), Some(&false), "collapsed → open");

      let _ = update(&mut state, Message::GroupToggled(2));
      assert_eq!(state.collapsed.get(&2), Some(&true), "open → collapsed");
    }

    #[test]
    fn skill_selected_is_a_no_op_that_leaves_state_untouched() {
      let mut state = loaded_state();
      let before: Vec<(i64, bool)> = state.collapsed.iter().map(|(k, v)| (*k, *v)).collect();

      let _ = update(&mut state, Message::SkillSelected(3300));

      let after: Vec<(i64, bool)> = state.collapsed.iter().map(|(k, v)| (*k, *v)).collect();
      assert_eq!(state.query, "", "the seam touches no state");
      assert_eq!(before.len(), after.len());
    }
  }

  mod view {
    use super::*;

    fn leaf(skill_id: i64, name: &str, level: u8, prereqs: Vec<(String, u8)>, queue_delta: u8) -> SkillLeaf {
      SkillLeaf {
        level,
        name: name.to_owned(),
        next_eta: "\u{2014}".to_owned(),
        prereqs,
        queue_delta,
        rank: 1,
        skill_id,
      }
    }

    fn group(id: i64, name: &str, leaves: Vec<SkillLeaf>) -> GroupRow {
      let total_skills = leaves.len();
      let trained_count = leaves.iter().filter(|l| l.level >= 5).count();
      GroupRow {
        id,
        leaves,
        name: name.to_owned(),
        total_skills,
        total_sp: 0,
        trained_count,
      }
    }

    fn populated() -> State {
      let mut state = State::new();
      let groups = vec![
        group(
          1,
          "Gunnery",
          vec![
            leaf(3300, "Gunnery", 5, Vec::new(), 0),
            leaf(3301, "Small Hybrid Turret", 0, vec![("Gunnery".to_owned(), 1)], 2),
          ],
        ),
        group(2, "Drones", vec![leaf(3436, "Drones", 3, Vec::new(), 0)]),
      ];
      let _ = update(&mut state, Message::Loaded(groups));
      state
    }

    #[test]
    fn it_renders_the_default_loaded_state() {
      let state = populated();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_empty_default_state() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_with_an_active_search_query() {
      let mut state = populated();
      let _ = update(&mut state, Message::SearchChanged("drone".to_owned()));

      let _el: Element<'_, Message> = view(&state);
    }
  }
}
