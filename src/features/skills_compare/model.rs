use std::collections::HashMap;

use crate::features::skills::browse::SkillCatalog;

#[derive(Clone, Debug, Default)]
pub struct CompareModel {
  pub at_iv_count: usize,
  pub at_v_count: usize,
  pub groups: Vec<GroupModel>,
  pub levels: HashMap<i64, u8>,
  pub total_sp: u64,
  pub trained_count: usize,
}

impl CompareModel {
  pub fn build(catalog: &SkillCatalog, levels: HashMap<i64, u8>, total_sp: u64) -> Self {
    let mut at_iv_count = 0;
    let mut at_v_count = 0;
    let mut trained_count = 0;

    let groups = catalog
      .groups
      .iter()
      .map(|group| {
        let mut at_v = 0;
        let mut trained = 0;
        let mut level_sum = 0u32;

        for skill in &group.skills {
          let level = levels.get(&skill.type_id).copied().unwrap_or(0);
          level_sum += u32::from(level);
          if level >= 5 {
            at_v += 1;
            at_v_count += 1;
          }
          if level >= 4 {
            at_iv_count += 1;
          }
          if level > 0 {
            trained += 1;
            trained_count += 1;
          }
        }

        let total = group.skills.len();
        let cap_avg = if total == 0 {
          0.0
        } else {
          f64::from(level_sum) / total as f64
        };

        GroupModel {
          at_v,
          cap_avg,
          id: group.id,
          total,
          trained,
        }
      })
      .collect();

    CompareModel {
      at_iv_count,
      at_v_count,
      groups,
      levels,
      total_sp,
      trained_count,
    }
  }

  pub fn group(&self, group_id: i64) -> Option<&GroupModel> {
    self.groups.iter().find(|group| group.id == group_id)
  }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GroupModel {
  pub at_v: usize,
  pub cap_avg: f64,
  pub id: i64,
  pub total: usize,
  pub trained: usize,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod compare_model {
    use super::*;
    use crate::features::skills::browse::{AttrKey, SkillCatalogEntry, SkillCatalogGroup};

    fn entry(type_id: i64, group_id: i64, rank: u8) -> SkillCatalogEntry {
      SkillCatalogEntry {
        group_id,
        group_name: "Group".to_owned(),
        name: format!("Skill {type_id}"),
        primary_attr: AttrKey::Intelligence,
        prereqs: Vec::new(),
        rank,
        secondary_attr: AttrKey::Memory,
        type_id,
      }
    }

    fn catalog() -> SkillCatalog {
      SkillCatalog {
        groups: vec![
          SkillCatalogGroup {
            id: 1,
            name: "Gunnery".to_owned(),
            skills: vec![entry(10, 1, 1), entry(11, 1, 2), entry(12, 1, 3)],
          },
          SkillCatalogGroup {
            id: 2,
            name: "Missiles".to_owned(),
            skills: vec![entry(20, 2, 4)],
          },
        ],
      }
    }

    mod build {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_counts_skills_at_v_iv_and_trained() {
        let levels = HashMap::from([(10, 5), (11, 4), (12, 0), (20, 1)]);

        let model = CompareModel::build(&catalog(), levels, 1_000);

        assert_eq!(model.at_v_count, 1);
        assert_eq!(model.at_iv_count, 2);
        assert_eq!(model.trained_count, 3);
      }

      #[test]
      fn it_averages_group_levels_into_cap_avg() {
        let levels = HashMap::from([(10, 3), (11, 0), (12, 3), (20, 5)]);

        let model = CompareModel::build(&catalog(), levels, 0);

        assert_eq!(model.group(1).unwrap().cap_avg, 2.0);
        assert_eq!(model.group(2).unwrap().cap_avg, 5.0);
      }

      #[test]
      fn it_tallies_per_group_at_v_trained_and_total() {
        let levels = HashMap::from([(10, 5), (11, 2), (12, 0), (20, 5)]);

        let model = CompareModel::build(&catalog(), levels, 0);
        let gunnery = model.group(1).unwrap();

        assert_eq!(gunnery.at_v, 1);
        assert_eq!(gunnery.trained, 2);
        assert_eq!(gunnery.total, 3);
      }

      #[test]
      fn it_treats_missing_skills_as_level_zero() {
        let model = CompareModel::build(&catalog(), HashMap::new(), 42);

        assert_eq!(model.at_v_count, 0);
        assert_eq!(model.trained_count, 0);
        assert_eq!(model.total_sp, 42);
        assert_eq!(model.group(1).unwrap().cap_avg, 0.0);
      }
    }
  }
}
