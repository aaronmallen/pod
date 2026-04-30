//! Repository for EVE certificate and ship mastery persistence.

use pod_model::Certificate;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, FromQueryResult, Statement, Value};

use crate::Error;

#[derive(Debug, FromQueryResult)]
struct CertRow {
  id: i32,
  name: String,
  grade: i32,
  skills_json: String,
}

#[derive(serde::Deserialize)]
struct SkillEntry {
  type_id: i32,
  basic: i32,
  improved: i32,
  advanced: i32,
  elite: i32,
}

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
    let rows = CertRow::find_by_statement(Statement::from_sql_and_values(
      DbBackend::Sqlite,
      "SELECT id, name, grade, skills_json FROM certificates ORDER BY name",
      [],
    ))
    .all(self.db)
    .await?;
    Ok(rows.into_iter().map(cert_from_row).collect())
  }

  pub async fn find_by_ids(&self, ids: &[i32]) -> Result<Vec<Certificate>, Error> {
    if ids.is_empty() {
      return Ok(vec![]);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!("SELECT id, name, grade, skills_json FROM certificates WHERE id IN ({placeholders})");
    let values: Vec<Value> = ids.iter().map(|&id| id.into()).collect();
    let rows = CertRow::find_by_statement(Statement::from_sql_and_values(DbBackend::Sqlite, &sql, values))
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
          .map(|(t, lvls)| serde_json::json!({"type_id": t, "basic": lvls[0], "improved": lvls[1], "advanced": lvls[2], "elite": lvls[3]}))
          .collect::<Vec<_>>(),
      )
      .unwrap_or_else(|_| "[]".to_string());

      let desc = cert
        .description
        .as_deref()
        .map(|d| format!("'{}'", d.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".to_string());

      let sql = format!(
        "INSERT OR REPLACE INTO certificates (id, name, description, grade, skills_json) VALUES ({}, '{}', {}, {}, '{}')",
        cert.id,
        cert.name.replace('\'', "''"),
        desc,
        cert.grade as i32,
        skills_json.replace('\'', "''"),
      );
      self.db.execute_unprepared(&sql).await?;
    }
    Ok(())
  }

  pub async fn upsert_ship_masteries(&self, entries: &[(i32, i32, Vec<i32>)]) -> Result<(), Error> {
    for (ship_id, mastery_level, cert_ids) in entries {
      let cert_ids_json = serde_json::to_string(cert_ids).unwrap_or_else(|_| "[]".to_string());
      let sql = format!(
        "INSERT OR REPLACE INTO ship_mastery_certs (ship_id, mastery_level, cert_ids_json) VALUES ({ship_id}, {mastery_level}, '{cert_ids_json}')"
      );
      self.db.execute_unprepared(&sql).await?;
    }
    Ok(())
  }
}

fn cert_from_row(row: CertRow) -> Certificate {
  let skills: Vec<SkillEntry> = serde_json::from_str(&row.skills_json).unwrap_or_default();
  Certificate {
    id: row.id,
    name: row.name,
    description: None,
    grade: row.grade.max(0) as u8,
    skills: skills
      .into_iter()
      .map(|s| {
        let clamp = |v: i32| v.clamp(0, 5) as u8;
        (
          s.type_id,
          [clamp(s.basic), clamp(s.improved), clamp(s.advanced), clamp(s.elite)],
        )
      })
      .collect(),
  }
}
