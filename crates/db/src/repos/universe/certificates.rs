//! Repository for EVE certificate and ship mastery persistence.

use pod_model::Certificate;
use sea_orm::{
  ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder, sea_query::OnConflict,
};

use crate::{
  Error,
  entities::{
    certificate::{ActiveModel as CertActive, Column as CertColumn, Entity as CertEntity},
    ship_mastery_cert::{ActiveModel as MasteryActive, Column as MasteryColumn, Entity as MasteryEntity},
  },
};

pub struct Repo<'a> {
  db: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  pub fn new(db: &'a DatabaseConnection) -> Self {
    Self {
      db,
    }
  }

  pub async fn find_all(&self) -> Result<Vec<Certificate>, Error> {
    let rows = CertEntity::find()
      .order_by(CertColumn::Name, Order::Asc)
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(cert_from_row).collect())
  }

  pub async fn find_by_ids(&self, ids: &[i32]) -> Result<Vec<Certificate>, Error> {
    if ids.is_empty() {
      return Ok(vec![]);
    }
    let rows = CertEntity::find()
      .filter(CertColumn::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(cert_from_row).collect())
  }

  pub async fn upsert_many(&self, certs: &[Certificate]) -> Result<(), Error> {
    for cert in certs {
      let skills_json = serde_json::to_string(
        &cert
          .skills
          .iter()
          .map(|(t, lvls)| {
            serde_json::json!({"type_id": t, "basic": lvls[0], "improved": lvls[1], "advanced": lvls[2], "elite": lvls[3]})
          })
          .collect::<Vec<_>>(),
      )
      .unwrap_or_else(|_| "[]".to_string());

      let active = CertActive {
        id: ActiveValue::Set(cert.id),
        name: ActiveValue::Set(cert.name.clone()),
        description: ActiveValue::Set(cert.description.clone()),
        grade: ActiveValue::Set(cert.grade as i32),
        skills_json: ActiveValue::Set(skills_json),
      };

      CertEntity::insert(active)
        .on_conflict(
          OnConflict::column(CertColumn::Id)
            .update_columns([
              CertColumn::Name,
              CertColumn::Description,
              CertColumn::Grade,
              CertColumn::SkillsJson,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }

  pub async fn upsert_ship_masteries(&self, entries: &[(i32, i32, Vec<i32>)]) -> Result<(), Error> {
    for (ship_id, mastery_level, cert_ids) in entries {
      let cert_ids_json = serde_json::to_string(cert_ids).unwrap_or_else(|_| "[]".to_string());

      let active = MasteryActive {
        ship_id: ActiveValue::Set(*ship_id),
        mastery_level: ActiveValue::Set(*mastery_level),
        cert_ids_json: ActiveValue::Set(cert_ids_json),
      };

      MasteryEntity::insert(active)
        .on_conflict(
          OnConflict::columns([MasteryColumn::ShipId, MasteryColumn::MasteryLevel])
            .update_column(MasteryColumn::CertIdsJson)
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}

/// Extracts and clamps a skill level from a raw integer.
fn clamp_skill_level(v: i32) -> u8 {
  v.clamp(0, 5) as u8
}

fn cert_from_row(row: crate::entities::certificate::Model) -> Certificate {
  #[derive(serde::Deserialize)]
  struct SkillEntry {
    type_id: i32,
    basic: i32,
    improved: i32,
    advanced: i32,
    elite: i32,
  }
  let skills: Vec<SkillEntry> = serde_json::from_str(&row.skills_json).unwrap_or_default();
  Certificate {
    id: row.id,
    name: row.name,
    description: row.description,
    grade: row.grade.max(0) as u8,
    skills: skills
      .into_iter()
      .map(|s| {
        (
          s.type_id,
          [
            clamp_skill_level(s.basic),
            clamp_skill_level(s.improved),
            clamp_skill_level(s.advanced),
            clamp_skill_level(s.elite),
          ],
        )
      })
      .collect(),
  }
}

#[cfg(test)]
mod tests {
  use pod_model::Certificate;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_cert(id: i32, name: &str) -> Certificate {
    Certificate {
      id,
      name: name.to_string(),
      description: None,
      grade: 1,
      skills: vec![],
    }
  }

  mod clamp_skill_level {
    use super::*;

    #[test]
    fn clamps_negative_to_zero() {
      assert_eq!(clamp_skill_level(-1), 0);
    }

    #[test]
    fn clamps_above_five_to_five() {
      assert_eq!(clamp_skill_level(10), 5);
    }

    #[test]
    fn passes_through_values_in_range() {
      for v in 0..=5 {
        assert_eq!(clamp_skill_level(v), v as u8);
      }
    }
  }

  mod find_all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_certs() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.find_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_certs_ordered_by_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo
        .upsert_many(&[make_cert(2, "Zebra"), make_cert(1, "Alpha")])
        .await
        .unwrap();
      let result = repo.find_all().await.unwrap();
      assert_eq!(result.len(), 2);
      assert_eq!(result[0].name, "Alpha");
      assert_eq!(result[1].name, "Zebra");
    }
  }

  mod find_by_ids {
    use super::*;

    #[tokio::test]
    async fn returns_empty_for_empty_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.find_by_ids(&[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_matching_certs() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo
        .upsert_many(&[make_cert(1, "Alpha"), make_cert(2, "Beta")])
        .await
        .unwrap();
      let result = repo.find_by_ids(&[1]).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].id, 1);
    }
  }

  mod upsert_many {
    use super::*;

    #[tokio::test]
    async fn inserts_certs_with_skills() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let cert = Certificate {
        id: 1,
        name: "Engineering".to_string(),
        description: Some("Desc".to_string()),
        grade: 2,
        skills: vec![(3300, [1, 2, 3, 4])],
      };
      repo.upsert_many(&[cert]).await.unwrap();
      let result = repo.find_all().await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skills.len(), 1);
      assert_eq!(result[0].skills[0].0, 3300);
      assert_eq!(result[0].skills[0].1, [1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn updates_existing_cert_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_many(&[make_cert(1, "Old Name")]).await.unwrap();

      let updated = Certificate {
        id: 1,
        name: "New Name".to_string(),
        description: None,
        grade: 3,
        skills: vec![],
      };
      repo.upsert_many(&[updated]).await.unwrap();
      let result = repo.find_all().await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].name, "New Name");
    }
  }

  mod upsert_ship_masteries {
    use super::*;

    #[tokio::test]
    async fn inserts_mastery_entries() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_ship_masteries(&[(587, 1, vec![1, 2, 3])]).await.unwrap();
    }

    #[tokio::test]
    async fn updates_existing_mastery_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_ship_masteries(&[(587, 1, vec![1, 2])]).await.unwrap();
      repo.upsert_ship_masteries(&[(587, 1, vec![5, 6, 7])]).await.unwrap();
    }
  }
}
