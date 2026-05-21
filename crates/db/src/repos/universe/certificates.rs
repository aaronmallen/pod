//! Repository for EVE certificate and ship mastery persistence.

use pod_model::Certificate;
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Order, sea_query::OnConflict};

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
            .update_columns([CertColumn::Name, CertColumn::Description, CertColumn::Grade, CertColumn::SkillsJson])
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
        let clamp = |v: i32| v.clamp(0, 5) as u8;
        (s.type_id, [clamp(s.basic), clamp(s.improved), clamp(s.advanced), clamp(s.elite)])
      })
      .collect(),
  }
}
