#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error, images,
  model::{
    Alliance, Bloodline, Character, CharacterAttributes, CharacterClone, CharacterCloneImplant, CharacterContact,
    CharacterContactLabel, CharacterImplant, CharacterJumpClone, CharacterKillEntry, CharacterNotification,
    CharacterSkill, CharacterSkillqueue, CharacterSquad, CharacterStanding, CharacterState, CharacterTelemetry,
    Corporation, ENTITY_TYPE_CHARACTER, Faction, KillmailAttacker, KillmailItem, OwnerType, Race, Squad,
    character_card::{CardRow, CardRowSql, CardTag, CardTraining, TagRowSql},
    character_clone_view::{ActiveCloneRow, CharacterClones, CloneWithImplants},
    character_contacts_view::CharacterContacts,
  },
  repo::infra::like_pattern,
  search::{FilterToken, ParsedQuery},
};

/// Sentinel squad name reserving the bucket for characters that belong to no user squad.
pub const RESERVED_UNASSIGNED_NAME: &str = "__unassigned__";

const SQLITE_MAX_BIND_PARAMS: usize = 999;

const SEARCH_SELECT: &str = "\
  SELECT \
    oc.id AS character_id, \
    oc.name AS name, \
    corp.ticker AS corp_ticker, \
    oc.corporation_id AS corporation_id, \
    CASE \
      WHEN cstate.online IS NULL THEN NULL \
      ELSE (cstate.station_id IS NOT NULL OR cstate.structure_id IS NOT NULL) \
    END AS docked, \
    COALESCE(st.name, sx.name, ss.name) AS location, \
    csq.position AS position, \
    sq.color AS squad_accent_hex, \
    cstate.total_sp AS total_sp, \
    cstate.wallet_balance AS wallet_balance, \
    head.skill_id AS training_skill_id, \
    it.name AS training_skill_name, \
    head.finished_level AS training_finished_level, \
    head.start_date AS training_start_date, \
    head.finish_date AS training_finish_date \
  FROM owned_characters oc \
  LEFT JOIN character_state cstate ON cstate.character_id = oc.id \
  LEFT JOIN corporations corp ON corp.id = oc.corporation_id \
  LEFT JOIN stations st ON st.id = cstate.station_id \
  LEFT JOIN structures sx ON sx.id = cstate.structure_id \
  LEFT JOIN solar_systems ss ON ss.id = cstate.solar_system_id \
  LEFT JOIN character_squads csq ON csq.character_id = oc.id \
  LEFT JOIN squads sq ON sq.id = csq.squad_id \
  LEFT JOIN character_skillqueue head \
    ON head.character_id = oc.id \
    AND head.queue_position = \
      (SELECT MIN(q.queue_position) FROM character_skillqueue q WHERE q.character_id = oc.id) \
  LEFT JOIN item_types it ON it.id = head.skill_id";

pub async fn all(db: &Database) -> Result<Vec<Character>, Error> {
  let rows = sqlx::query_as::<_, Character>(
    "SELECT alliance_id, birthday, bloodline_id, corporation_id, description, \
    faction_id, gender, id, name, race_id, security_status, title FROM characters",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn all_owned(db: &Database) -> Result<Vec<Character>, Error> {
  let rows = sqlx::query_as::<_, Character>(
    "SELECT alliance_id, birthday, bloodline_id, corporation_id, description, \
    faction_id, gender, id, name, race_id, security_status, title FROM owned_characters",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn get(db: &Database, id: i64) -> Result<Option<Character>, Error> {
  let row = sqlx::query_as::<_, Character>(
    "SELECT alliance_id, birthday, bloodline_id, corporation_id, description, \
    faction_id, gender, id, name, race_id, security_status, title FROM characters \
    WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn search(db: &Database, query: &ParsedQuery, now: &str) -> Result<Vec<CardRow>, Error> {
  let mut builder = QueryBuilder::<Sqlite>::new(SEARCH_SELECT);

  for (index, token) in query.tokens.iter().enumerate() {
    builder.push(if index == 0 { " WHERE " } else { " AND " });
    push_token_predicate(&mut builder, token, now);
  }
  builder.push(" ORDER BY oc.name");

  let sql_rows = builder.build_query_as::<CardRowSql>().fetch_all(&db.0).await?;
  let mut rows: Vec<CardRow> = sql_rows.into_iter().map(into_card_row).collect();
  attach_tags(db, &mut rows).await?;
  Ok(rows)
}

async fn attach_tags(db: &Database, rows: &mut [CardRow]) -> Result<(), Error> {
  if rows.is_empty() {
    return Ok(());
  }

  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT et.entity_id AS character_id, tg.color AS color, tg.id AS id, tg.name AS name \
    FROM entity_tags et \
    JOIN tags tg ON tg.id = et.tag_id \
    WHERE et.entity_type = 'character' AND et.entity_id IN (",
  );
  let mut separated = builder.separated(", ");
  for row in rows.iter() {
    separated.push_bind(row.character_id);
  }
  separated.push_unseparated(") ORDER BY tg.position, tg.id");

  let tag_rows = builder.build_query_as::<TagRowSql>().fetch_all(&db.0).await?;

  let mut by_character: HashMap<i64, Vec<CardTag>> = HashMap::new();
  for tag in tag_rows {
    by_character.entry(tag.character_id).or_default().push(CardTag {
      color_hex: tag.color,
      id: tag.id,
      name: tag.name,
    });
  }
  for row in rows.iter_mut() {
    if let Some(tags) = by_character.remove(&row.character_id) {
      row.tags = tags;
    }
  }
  Ok(())
}

fn into_card_row(sql: CardRowSql) -> CardRow {
  let training = sql.training_skill_id.map(|skill_id| CardTraining {
    finish_date: sql.training_finish_date,
    finished_level: sql.training_finished_level.unwrap_or_default(),
    skill_id,
    skill_name: sql.training_skill_name,
    start_date: sql.training_start_date,
  });

  CardRow {
    character_id: sql.character_id,
    corp_ticker: sql.corp_ticker,
    corporation_id: sql.corporation_id,
    docked: sql.docked,
    location: sql.location,
    name: sql.name,
    position: sql.position,
    squad_accent_hex: sql.squad_accent_hex,
    tags: Vec::new(),
    total_sp: sql.total_sp,
    training,
    wallet_balance: sql.wallet_balance,
  }
}

fn push_free_text_predicate(builder: &mut QueryBuilder<Sqlite>, text: &str) {
  let pattern = like_pattern(text);

  builder.push("(oc.name LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR corp.name LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR corp.ticker LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR COALESCE(st.name, sx.name, ss.name) LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR it.name LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(
    " ESCAPE '\\' OR EXISTS (SELECT 1 FROM entity_tags et \
    JOIN tags tg ON tg.id = et.tag_id \
    WHERE et.entity_type = 'character' AND et.entity_id = oc.id AND tg.name LIKE ",
  );
  builder.push_bind(pattern);
  builder.push(" ESCAPE '\\'))");
}

fn push_key_value_predicate(builder: &mut QueryBuilder<Sqlite>, key: &str, value: &str, now: &str) {
  match key {
    "corp" => {
      let pattern = like_pattern(value);
      builder.push("(corp.name LIKE ");
      builder.push_bind(pattern.clone());
      builder.push(" ESCAPE '\\' OR corp.ticker LIKE ");
      builder.push_bind(pattern);
      builder.push(" ESCAPE '\\')");
    }
    "loc" => {
      builder.push("COALESCE(st.name, sx.name, ss.name) LIKE ");
      builder.push_bind(like_pattern(value));
      builder.push(" ESCAPE '\\'");
    }
    "name" => {
      builder.push("oc.name LIKE ");
      builder.push_bind(like_pattern(value));
      builder.push(" ESCAPE '\\'");
    }
    "status" => match value {
      "docked" => {
        builder
          .push("(cstate.online IS NOT NULL AND (cstate.station_id IS NOT NULL OR cstate.structure_id IS NOT NULL))");
      }
      "in-space" | "space" | "undocked" => {
        builder.push("(cstate.online IS NOT NULL AND cstate.station_id IS NULL AND cstate.structure_id IS NULL)");
      }
      _ => {
        // Unknown status/training/facet value compiles to a predicate that matches no rows.
        builder.push("0 = 1");
      }
    },
    "tag" => {
      builder.push(
        "EXISTS (SELECT 1 FROM entity_tags et JOIN tags tg ON tg.id = et.tag_id \
        WHERE et.entity_type = 'character' AND et.entity_id = oc.id AND tg.name = ",
      );
      builder.push_bind(value.to_string());
      builder.push(" COLLATE NOCASE)");
    }
    "training" => match value {
      "active" | "training" => {
        builder.push("(head.start_date IS NOT NULL AND head.finish_date IS NOT NULL AND head.finish_date > ");
        builder.push_bind(now.to_string());
        builder.push(")");
      }
      "idle" => {
        builder.push(
          "(head.character_id IS NULL OR head.start_date IS NULL OR head.finish_date IS NULL OR head.finish_date <= ",
        );
        builder.push_bind(now.to_string());
        builder.push(")");
      }
      _ => {
        builder.push("0 = 1");
      }
    },
    _ => {
      builder.push("0 = 1");
    }
  }
}

fn push_token_predicate(builder: &mut QueryBuilder<Sqlite>, token: &FilterToken, now: &str) {
  match token {
    FilterToken::FreeText {
      negated,
      text,
    } => {
      if *negated {
        builder.push("NOT ");
      }
      push_free_text_predicate(builder, text);
    }
    FilterToken::KeyValue {
      key,
      negated,
      values,
    } => {
      if *negated {
        builder.push("NOT ");
      }
      builder.push("(");
      for (index, value) in values.iter().enumerate() {
        if index > 0 {
          builder.push(" OR ");
        }
        push_key_value_predicate(builder, key, value, now);
      }
      builder.push(")");
    }
  }
}

pub async fn delete(db: &Database, id: i64) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;
  sqlx::query("DELETE FROM credentials WHERE owner_id = ? AND owner_type = ?")
    .bind(id)
    .bind(OwnerType::Character)
    .execute(&mut *tx)
    .await?;
  sqlx::query("DELETE FROM entity_tags WHERE entity_type = ? AND entity_id = ?")
    .bind(ENTITY_TYPE_CHARACTER)
    .bind(id)
    .execute(&mut *tx)
    .await?;
  sqlx::query("DELETE FROM characters WHERE id = ?")
    .bind(id)
    .execute(&mut *tx)
    .await?;
  tx.commit().await?;
  Ok(())
}

// Every `*_with_org` writer persists a character together with that character's OWN corporation
// (char.corporation_id() == corporation.id()) under `defer_foreign_keys = ON`, so the deferred
// characters.corporation_id FK is satisfied by the corporation row inserted in the same
// transaction. The lone exception is owner-corporation resolution, which persists a reference CEO
// whose corp can differ and ensures that corp's row up front (see sync::structure_resolution). This
// helper names the character/corporation on the surviving 787 so a future gap is localizable from
// the log alone, without raising baseline log volume (it only runs on a failing commit).
async fn commit_with_org_context(
  tx: sqlx::Transaction<'_, Sqlite>,
  char: &Character,
  corporation: &Corporation,
) -> Result<(), Error> {
  match tx.commit().await {
    Ok(()) => Ok(()),
    Err(error) if crate::store::is_foreign_key_constraint(&error) => Err(Error::ForeignKey {
      context: format!(
        "character {} (corporation_id {}) alongside corporation {}",
        char.id(),
        char.corporation_id(),
        corporation.id()
      ),
      source: error,
    }),
    Err(error) => Err(Error::Sqlx(error)),
  }
}

pub async fn insert_with_org(
  db: &Database,
  char: &Character,
  bloodline: &Bloodline,
  race: &Race,
  corporation: &Corporation,
  alliance: Option<&Alliance>,
  faction: Option<&Faction>,
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  if let Some(f) = faction {
    sqlx::query(
      "INSERT OR IGNORE INTO factions \
        (corporation_id, description, id, is_unique, militia_corporation_id, name, \
        size_factor, solar_system_id, station_count, station_system_count) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(f.corporation_id())
    .bind(f.description())
    .bind(f.id())
    .bind(f.is_unique())
    .bind(f.militia_corporation_id())
    .bind(f.name())
    .bind(f.size_factor())
    .bind(f.solar_system_id())
    .bind(f.station_count())
    .bind(f.station_system_count())
    .execute(&mut *tx)
    .await?;
  }

  if let Some(a) = alliance {
    sqlx::query(
      "INSERT OR IGNORE INTO alliances \
        (creator_corporation_id, creator_id, date_founded, executor_corporation_id, \
        faction_id, id, name, ticker) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(a.creator_corporation_id())
    .bind(a.creator_id())
    .bind(a.date_founded())
    .bind(a.executor_corporation_id())
    .bind(a.faction_id())
    .bind(a.id())
    .bind(a.name())
    .bind(a.ticker())
    .execute(&mut *tx)
    .await?;
  }

  sqlx::query("INSERT OR IGNORE INTO races (alliance_id, description, id, name) VALUES (?, ?, ?, ?)")
    .bind(race.alliance_id())
    .bind(race.description())
    .bind(race.id())
    .bind(race.name())
    .execute(&mut *tx)
    .await?;

  let ceo_id = corporation.ceo_id().unwrap_or(char.id());
  let creator_id = corporation.creator_id().unwrap_or(char.id());
  let member_count = corporation.member_count().unwrap_or(0);
  let tax_rate = corporation.tax_rate().unwrap_or(0.0);
  sqlx::query(
    "INSERT OR IGNORE INTO corporations \
      (alliance_id, ceo_id, creator_id, date_founded, description, faction_id, \
      home_station_id, id, member_count, name, shares, tax_rate, ticker, url, war_eligible) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(corporation.alliance_id())
  .bind(ceo_id)
  .bind(creator_id)
  .bind(corporation.creation_date())
  .bind(corporation.description())
  .bind(corporation.faction_id())
  .bind(corporation.home_station_id())
  .bind(corporation.id())
  .bind(member_count)
  .bind(corporation.name())
  .bind(corporation.shares())
  .bind(tax_rate)
  .bind(corporation.ticker())
  .bind(corporation.url())
  .bind(corporation.war_eligible())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT OR IGNORE INTO bloodlines \
      (charisma, corporation_id, description, id, intelligence, memory, name, \
      perception, race_id, ship_type_id, willpower) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(bloodline.charisma())
  .bind(bloodline.corporation_id())
  .bind(bloodline.description())
  .bind(bloodline.id())
  .bind(bloodline.intelligence())
  .bind(bloodline.memory())
  .bind(bloodline.name())
  .bind(bloodline.perception())
  .bind(bloodline.race_id())
  .bind(bloodline.ship_type_id())
  .bind(bloodline.willpower())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT OR IGNORE INTO characters \
      (alliance_id, birthday, bloodline_id, corporation_id, description, \
      faction_id, gender, id, name, race_id, security_status, title) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(char.alliance_id())
  .bind(char.birthday())
  .bind(char.bloodline_id())
  .bind(char.corporation_id())
  .bind(char.description())
  .bind(char.faction_id())
  .bind(char.gender())
  .bind(char.id())
  .bind(char.name())
  .bind(char.race_id())
  .bind(char.security_status())
  .bind(char.title())
  .execute(&mut *tx)
  .await?;

  commit_with_org_context(tx, char, corporation).await
}

pub async fn upsert(db: &Database, char: &Character) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO characters \
      (alliance_id, birthday, bloodline_id, corporation_id, description, \
      faction_id, gender, id, name, race_id, security_status, title) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      alliance_id     = excluded.alliance_id, \
      birthday        = excluded.birthday, \
      corporation_id  = excluded.corporation_id, \
      description     = excluded.description, \
      faction_id      = excluded.faction_id, \
      gender          = excluded.gender, \
      name            = excluded.name, \
      security_status = excluded.security_status, \
      title           = excluded.title",
  )
  .bind(char.alliance_id())
  .bind(char.birthday())
  .bind(char.bloodline_id())
  .bind(char.corporation_id())
  .bind(char.description())
  .bind(char.faction_id())
  .bind(char.gender())
  .bind(char.id())
  .bind(char.name())
  .bind(char.race_id())
  .bind(char.security_status())
  .bind(char.title())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn upsert_with_org(
  db: &Database,
  char: &Character,
  bloodline: &Bloodline,
  race: &Race,
  corporation: &Corporation,
  alliance: Option<&Alliance>,
  faction: Option<&Faction>,
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  if let Some(f) = faction {
    sqlx::query(
      "INSERT OR IGNORE INTO factions \
        (corporation_id, description, id, is_unique, militia_corporation_id, name, \
        size_factor, solar_system_id, station_count, station_system_count) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(f.corporation_id())
    .bind(f.description())
    .bind(f.id())
    .bind(f.is_unique())
    .bind(f.militia_corporation_id())
    .bind(f.name())
    .bind(f.size_factor())
    .bind(f.solar_system_id())
    .bind(f.station_count())
    .bind(f.station_system_count())
    .execute(&mut *tx)
    .await?;
  }

  if let Some(a) = alliance {
    sqlx::query(
      "INSERT INTO alliances \
        (creator_corporation_id, creator_id, date_founded, executor_corporation_id, \
        faction_id, id, name, ticker) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        creator_corporation_id  = excluded.creator_corporation_id, \
        creator_id              = excluded.creator_id, \
        date_founded            = excluded.date_founded, \
        executor_corporation_id = excluded.executor_corporation_id, \
        faction_id              = excluded.faction_id, \
        name                    = excluded.name, \
        ticker                  = excluded.ticker",
    )
    .bind(a.creator_corporation_id())
    .bind(a.creator_id())
    .bind(a.date_founded())
    .bind(a.executor_corporation_id())
    .bind(a.faction_id())
    .bind(a.id())
    .bind(a.name())
    .bind(a.ticker())
    .execute(&mut *tx)
    .await?;
  }

  sqlx::query("INSERT OR IGNORE INTO races (alliance_id, description, id, name) VALUES (?, ?, ?, ?)")
    .bind(race.alliance_id())
    .bind(race.description())
    .bind(race.id())
    .bind(race.name())
    .execute(&mut *tx)
    .await?;

  let ceo_id = corporation.ceo_id().unwrap_or(char.id());
  let creator_id = corporation.creator_id().unwrap_or(char.id());
  let member_count = corporation.member_count().unwrap_or(0);
  let tax_rate = corporation.tax_rate().unwrap_or(0.0);
  sqlx::query(
    "INSERT INTO corporations \
      (alliance_id, ceo_id, creator_id, date_founded, description, faction_id, \
      home_station_id, id, member_count, name, shares, tax_rate, ticker, url, war_eligible) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      alliance_id     = excluded.alliance_id, \
      ceo_id          = excluded.ceo_id, \
      creator_id      = excluded.creator_id, \
      date_founded    = excluded.date_founded, \
      description     = excluded.description, \
      faction_id      = excluded.faction_id, \
      home_station_id = excluded.home_station_id, \
      member_count    = excluded.member_count, \
      name            = excluded.name, \
      shares          = excluded.shares, \
      tax_rate        = excluded.tax_rate, \
      ticker          = excluded.ticker, \
      url             = excluded.url, \
      war_eligible    = excluded.war_eligible",
  )
  .bind(corporation.alliance_id())
  .bind(ceo_id)
  .bind(creator_id)
  .bind(corporation.creation_date())
  .bind(corporation.description())
  .bind(corporation.faction_id())
  .bind(corporation.home_station_id())
  .bind(corporation.id())
  .bind(member_count)
  .bind(corporation.name())
  .bind(corporation.shares())
  .bind(tax_rate)
  .bind(corporation.ticker())
  .bind(corporation.url())
  .bind(corporation.war_eligible())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT OR IGNORE INTO bloodlines \
      (charisma, corporation_id, description, id, intelligence, memory, name, \
      perception, race_id, ship_type_id, willpower) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(bloodline.charisma())
  .bind(bloodline.corporation_id())
  .bind(bloodline.description())
  .bind(bloodline.id())
  .bind(bloodline.intelligence())
  .bind(bloodline.memory())
  .bind(bloodline.name())
  .bind(bloodline.perception())
  .bind(bloodline.race_id())
  .bind(bloodline.ship_type_id())
  .bind(bloodline.willpower())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT INTO characters \
      (alliance_id, birthday, bloodline_id, corporation_id, description, \
      faction_id, gender, id, name, race_id, security_status, title) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      alliance_id     = excluded.alliance_id, \
      birthday        = excluded.birthday, \
      corporation_id  = excluded.corporation_id, \
      description     = excluded.description, \
      faction_id      = excluded.faction_id, \
      gender          = excluded.gender, \
      name            = excluded.name, \
      security_status = excluded.security_status, \
      title           = excluded.title",
  )
  .bind(char.alliance_id())
  .bind(char.birthday())
  .bind(char.bloodline_id())
  .bind(char.corporation_id())
  .bind(char.description())
  .bind(char.faction_id())
  .bind(char.gender())
  .bind(char.id())
  .bind(char.name())
  .bind(char.race_id())
  .bind(char.security_status())
  .bind(char.title())
  .execute(&mut *tx)
  .await?;

  commit_with_org_context(tx, char, corporation).await
}

pub async fn attributes(db: &Database, character_id: i64) -> Result<Option<CharacterAttributes>, Error> {
  let row = sqlx::query_as::<_, CharacterAttributes>(
    "SELECT accrued_remap_cooldown_date, bonus_remaps, character_id, charisma, intelligence, \
    last_remap_date, memory, perception, unallocated_sp, willpower FROM character_attributes \
    WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn upsert_attributes(db: &Database, attributes: &CharacterAttributes) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_attributes \
      (accrued_remap_cooldown_date, bonus_remaps, character_id, charisma, intelligence, \
      last_remap_date, memory, perception, unallocated_sp, willpower) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(character_id) DO UPDATE SET \
      accrued_remap_cooldown_date = excluded.accrued_remap_cooldown_date, \
      bonus_remaps                = excluded.bonus_remaps, \
      charisma                    = excluded.charisma, \
      intelligence                = excluded.intelligence, \
      last_remap_date             = excluded.last_remap_date, \
      memory                      = excluded.memory, \
      perception                  = excluded.perception, \
      unallocated_sp              = excluded.unallocated_sp, \
      willpower                   = excluded.willpower",
  )
  .bind(attributes.accrued_remap_cooldown_date())
  .bind(attributes.bonus_remaps())
  .bind(attributes.character_id())
  .bind(attributes.charisma())
  .bind(attributes.intelligence())
  .bind(attributes.last_remap_date())
  .bind(attributes.memory())
  .bind(attributes.perception())
  .bind(attributes.unallocated_sp())
  .bind(attributes.willpower())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn implants(db: &Database, character_id: i64) -> Result<Vec<CharacterImplant>, Error> {
  let rows = sqlx::query_as::<_, CharacterImplant>(
    "SELECT attribute_id, bonus, character_id FROM character_implants \
    WHERE character_id = ? ORDER BY attribute_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_implants(db: &Database, character_id: i64, implants: &[CharacterImplant]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("DELETE FROM character_implants WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for chunk in implants.chunks(SQLITE_MAX_BIND_PARAMS / 3) {
    let mut builder =
      QueryBuilder::<Sqlite>::new("INSERT INTO character_implants (attribute_id, bonus, character_id) ");
    builder.push_values(chunk, |mut row, implant| {
      row
        .push_bind(implant.attribute_id())
        .push_bind(implant.bonus())
        .push_bind(implant.character_id());
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn skills(db: &Database, character_id: i64) -> Result<Vec<CharacterSkill>, Error> {
  let rows = sqlx::query_as::<_, CharacterSkill>(
    "SELECT active_skill_level, character_id, skill_id, skillpoints_in_skill, \
    trained_skill_level FROM character_skills WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_skills(db: &Database, character_id: i64, skills: &[CharacterSkill]) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("DELETE FROM character_skills WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for chunk in skills.chunks(SQLITE_MAX_BIND_PARAMS / 5) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO character_skills \
        (active_skill_level, character_id, skill_id, skillpoints_in_skill, trained_skill_level) ",
    );
    builder.push_values(chunk, |mut row, skill| {
      row
        .push_bind(skill.active_skill_level())
        .push_bind(skill.character_id())
        .push_bind(skill.skill_id())
        .push_bind(skill.skillpoints_in_skill())
        .push_bind(skill.trained_skill_level());
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn current_skillqueue(
  db: &Database,
  character_id: i64,
  now: DateTime<Utc>,
) -> Result<Option<CharacterSkillqueue>, Error> {
  let now = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
  let entry = sqlx::query_as::<_, CharacterSkillqueue>(
    "SELECT character_id, finish_date, finished_level, level_end_sp, level_start_sp, \
    queue_position, skill_id, start_date, training_start_sp FROM character_skillqueue \
    WHERE character_id = ? \
    ORDER BY CASE WHEN finish_date IS NOT NULL AND finish_date > ? THEN 0 ELSE 1 END, queue_position \
    LIMIT 1",
  )
  .bind(character_id)
  .bind(now)
  .fetch_optional(&db.0)
  .await?;
  Ok(entry)
}

pub async fn skillqueue(db: &Database, character_id: i64) -> Result<Vec<CharacterSkillqueue>, Error> {
  let rows = sqlx::query_as::<_, CharacterSkillqueue>(
    "SELECT character_id, finish_date, finished_level, level_end_sp, level_start_sp, \
    queue_position, skill_id, start_date, training_start_sp FROM character_skillqueue \
    WHERE character_id = ? ORDER BY queue_position",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_skillqueue(
  db: &Database,
  character_id: i64,
  entries: &[CharacterSkillqueue],
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("DELETE FROM character_skillqueue WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for chunk in entries.chunks(SQLITE_MAX_BIND_PARAMS / 9) {
    let mut builder = QueryBuilder::<Sqlite>::new(
      "INSERT INTO character_skillqueue \
        (character_id, finish_date, finished_level, level_end_sp, level_start_sp, \
        queue_position, skill_id, start_date, training_start_sp) ",
    );
    builder.push_values(chunk, |mut row, entry| {
      row
        .push_bind(entry.character_id())
        .push_bind(entry.finish_date())
        .push_bind(entry.finished_level())
        .push_bind(entry.level_end_sp())
        .push_bind(entry.level_start_sp())
        .push_bind(entry.queue_position())
        .push_bind(entry.skill_id())
        .push_bind(entry.start_date())
        .push_bind(entry.training_start_sp());
    });
    builder.build().execute(&mut *tx).await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn state(db: &Database, character_id: i64) -> Result<Option<CharacterState>, Error> {
  let state = sqlx::query_as::<_, CharacterState>("SELECT * FROM character_state WHERE character_id = ?")
    .bind(character_id)
    .fetch_optional(&db.0)
    .await?;
  Ok(state)
}

pub async fn all_states(db: &Database) -> Result<Vec<CharacterState>, Error> {
  let states = sqlx::query_as::<_, CharacterState>("SELECT * FROM character_state ORDER BY character_id")
    .fetch_all(&db.0)
    .await?;
  Ok(states)
}

pub async fn telemetry(db: &Database, character_id: i64) -> Result<Option<CharacterTelemetry>, Error> {
  let row = sqlx::query_as::<_, CharacterTelemetry>(
    "SELECT character_id, online, ship_item_id, ship_name, ship_type_id, solar_system_id, \
    station_id, structure_id, synced_at FROM character_telemetry WHERE character_id = ?",
  )
  .bind(character_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn upsert_telemetry(db: &Database, telemetry: &CharacterTelemetry) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_telemetry \
      (character_id, online, ship_item_id, ship_name, ship_type_id, solar_system_id, \
      station_id, structure_id, synced_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(character_id) DO UPDATE SET \
      online          = excluded.online, \
      ship_item_id    = excluded.ship_item_id, \
      ship_name       = excluded.ship_name, \
      ship_type_id    = excluded.ship_type_id, \
      solar_system_id = excluded.solar_system_id, \
      station_id      = excluded.station_id, \
      structure_id    = excluded.structure_id, \
      synced_at       = excluded.synced_at",
  )
  .bind(telemetry.character_id())
  .bind(telemetry.online())
  .bind(telemetry.ship_item_id())
  .bind(telemetry.ship_name())
  .bind(telemetry.ship_type_id())
  .bind(telemetry.solar_system_id())
  .bind(telemetry.station_id())
  .bind(telemetry.structure_id())
  .bind(telemetry.synced_at())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn replace_clones_for_character(
  db: &Database,
  character_id: i64,
  active: &CharacterClone,
  jump_clones: &[CharacterJumpClone],
  implants: &[CharacterCloneImplant],
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("DELETE FROM character_clone_implants WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;
  sqlx::query("DELETE FROM character_jump_clones WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;
  sqlx::query("DELETE FROM character_clones WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  sqlx::query(
    "INSERT INTO character_clones \
      (character_id, home_location_id, home_location_type, home_location_name, last_clone_jump_date, \
      last_station_change_date) \
    VALUES (?, ?, ?, ?, ?, ?)",
  )
  .bind(active.character_id())
  .bind(active.home_location_id())
  .bind(active.home_location_type())
  .bind(active.home_location_name())
  .bind(active.last_clone_jump_date())
  .bind(active.last_station_change_date())
  .execute(&mut *tx)
  .await?;

  for clone in jump_clones {
    sqlx::query(
      "INSERT INTO character_jump_clones \
        (character_id, jump_clone_id, location_id, location_type, location_name, name) \
      VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(clone.character_id())
    .bind(clone.jump_clone_id())
    .bind(clone.location_id())
    .bind(clone.location_type())
    .bind(clone.location_name())
    .bind(clone.name())
    .execute(&mut *tx)
    .await?;
  }

  for implant in implants {
    sqlx::query(
      "INSERT INTO character_clone_implants (character_id, clone_id, type_id, name, icon) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(implant.character_id())
    .bind(implant.clone_id())
    .bind(implant.type_id())
    .bind(implant.name())
    .bind(implant.icon())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn clones(db: &Database, character_id: i64) -> Result<Option<CharacterClones>, Error> {
  let Some(active) = active_clone(db, character_id).await? else {
    return Ok(None);
  };

  let jump_rows = sqlx::query_as::<_, CharacterJumpClone>(
    "SELECT character_id, jump_clone_id, location_id, location_name, location_type, name \
    FROM character_jump_clones WHERE character_id = ? ORDER BY jump_clone_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  let mut jump_clones = Vec::with_capacity(jump_rows.len());
  for clone in jump_rows {
    let implants = clone_implants(db, character_id, Some(clone.jump_clone_id())).await?;
    jump_clones.push(CloneWithImplants {
      clone,
      implants,
    });
  }

  Ok(Some(CharacterClones {
    active,
    jump_clones,
  }))
}

async fn active_clone(db: &Database, character_id: i64) -> Result<Option<CloneWithImplants<CharacterClone>>, Error> {
  let rows = sqlx::query_as::<_, ActiveCloneRow>(
    "SELECT character_id, home_location_id, home_location_type, home_location_name, last_clone_jump_date, \
    last_station_change_date, implant_type_id, implant_name, implant_icon \
    FROM active_clone_with_implants WHERE character_id = ? ORDER BY implant_type_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  let Some(first) = rows.first() else {
    return Ok(None);
  };

  let clone = CharacterClone {
    character_id: first.character_id,
    home_location_id: first.home_location_id,
    home_location_name: first.home_location_name.clone(),
    home_location_type: first.home_location_type.clone(),
    last_clone_jump_date: first.last_clone_jump_date.clone(),
    last_station_change_date: first.last_station_change_date.clone(),
  };

  let implants = rows
    .iter()
    .filter_map(|row| {
      row.implant_type_id.map(|type_id| CharacterCloneImplant {
        character_id: row.character_id,
        clone_id: None,
        icon: row.implant_icon.clone(),
        name: row.implant_name.clone().unwrap_or_default(),
        type_id,
      })
    })
    .collect();

  Ok(Some(CloneWithImplants {
    clone,
    implants,
  }))
}

async fn clone_implants(
  db: &Database,
  character_id: i64,
  clone_id: Option<i64>,
) -> Result<Vec<CharacterCloneImplant>, Error> {
  let rows = sqlx::query_as::<_, CharacterCloneImplant>(
    "SELECT character_id, clone_id, icon, name, type_id FROM character_clone_implants \
    WHERE character_id = ? AND clone_id IS ? ORDER BY type_id",
  )
  .bind(character_id)
  .bind(clone_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_contacts_for_character(
  db: &Database,
  character_id: i64,
  contacts: &[CharacterContact],
  protected: &HashSet<i64>,
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  // `protected` ids carry an unacknowledged add/edit/remove outbox write; leaving their optimistic local row
  // untouched stops a full-replace sync inside the drain window from clobbering a just-made change.
  let mut delete = QueryBuilder::<Sqlite>::new("DELETE FROM character_contacts WHERE character_id = ");
  delete.push_bind(character_id);
  if !protected.is_empty() {
    delete.push(" AND contact_id NOT IN (");
    let mut separated = delete.separated(", ");
    for id in protected {
      separated.push_bind(*id);
    }
    separated.push_unseparated(")");
  }
  delete.build().execute(&mut *tx).await?;

  for contact in contacts {
    if protected.contains(&contact.contact_id()) {
      continue;
    }
    sqlx::query(
      "INSERT INTO character_contacts \
        (character_id, contact_id, contact_type, standing, is_watched, is_blocked, label_ids, contact_name) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(contact.character_id())
    .bind(contact.contact_id())
    .bind(contact.contact_type())
    .bind(contact.standing())
    .bind(contact.is_watched())
    .bind(contact.is_blocked())
    .bind(contact.label_ids())
    .bind(contact.contact_name())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_contact(db: &Database, contact: &CharacterContact) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_contacts \
      (character_id, contact_id, contact_type, standing, is_watched, is_blocked, label_ids, contact_name) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(character_id, contact_id) DO UPDATE SET \
      contact_type = excluded.contact_type, \
      standing = excluded.standing, \
      is_watched = excluded.is_watched, \
      is_blocked = excluded.is_blocked, \
      label_ids = excluded.label_ids, \
      contact_name = excluded.contact_name",
  )
  .bind(contact.character_id())
  .bind(contact.contact_id())
  .bind(contact.contact_type())
  .bind(contact.standing())
  .bind(contact.is_watched())
  .bind(contact.is_blocked())
  .bind(contact.label_ids())
  .bind(contact.contact_name())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn delete_contact(db: &Database, character_id: i64, contact_id: i64) -> Result<(), Error> {
  sqlx::query("DELETE FROM character_contacts WHERE character_id = ? AND contact_id = ?")
    .bind(character_id)
    .bind(contact_id)
    .execute(&db.0)
    .await?;
  Ok(())
}

pub async fn replace_labels_for_character(
  db: &Database,
  character_id: i64,
  labels: &[CharacterContactLabel],
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("DELETE FROM character_contact_labels WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for label in labels {
    sqlx::query("INSERT INTO character_contact_labels (character_id, label_id, label_name) VALUES (?, ?, ?)")
      .bind(label.character_id())
      .bind(label.label_id())
      .bind(label.label_name())
      .execute(&mut *tx)
      .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn contacts(db: &Database, character_id: i64) -> Result<CharacterContacts, Error> {
  let contacts = sqlx::query_as::<_, CharacterContact>(
    "SELECT character_id, contact_id, contact_name, contact_type, is_blocked, is_watched, label_ids, standing \
    FROM character_contacts WHERE character_id = ? ORDER BY contact_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  let labels = sqlx::query_as::<_, CharacterContactLabel>(
    "SELECT character_id, label_id, label_name FROM character_contact_labels \
    WHERE character_id = ? ORDER BY label_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  Ok(CharacterContacts::resolved(&images::default_store(), contacts, labels))
}

/// The column a contacts page is keyset-ordered by. Mirrors the address-book sort header so the UI can push its
/// active sort into SQL instead of holding the full set in memory and sorting client-side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactSortColumn {
  Name,
  Standing,
  Type,
}

/// Sort direction for a contacts page; pairs with [`ContactSortColumn`] to drive the keyset comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactSortDir {
  Asc,
  Desc,
}

/// The keyset cursor for the next contacts page: the active sort column's value of the last row plus its
/// `contact_id` tiebreaker. `Name`/`Type` carry the text value; `Standing` carries the numeric value.
#[derive(Clone, Debug, PartialEq)]
pub enum ContactCursor {
  Number(f64, i64),
  Text(String, i64),
}

/// The keyset-paginated contact labels: small per-character lookup set, fetched once and shared across pages.
pub async fn contact_labels(db: &Database, character_id: i64) -> Result<Vec<CharacterContactLabel>, Error> {
  let labels = sqlx::query_as::<_, CharacterContactLabel>(
    "SELECT character_id, label_id, label_name FROM character_contact_labels \
    WHERE character_id = ? ORDER BY label_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(labels)
}

/// Fetches one keyset page of contacts ordered by `sort`/`dir`, optionally filtered to a single `contact_type`
/// (the address-book facet), starting after `cursor`. The keyset compares `(sort column, contact_id)` so the page
/// is stable across loads; the caller derives the next cursor from the last returned row.
#[allow(clippy::too_many_arguments)]
pub async fn contacts_page(
  db: &Database,
  character_id: i64,
  contact_type: Option<&str>,
  query: Option<&str>,
  sort: ContactSortColumn,
  dir: ContactSortDir,
  cursor: Option<&ContactCursor>,
  limit: i64,
) -> Result<Vec<CharacterContact>, Error> {
  let column = match sort {
    ContactSortColumn::Name => "contact_name",
    ContactSortColumn::Standing => "standing",
    ContactSortColumn::Type => "contact_type",
  };
  // The keyset comparator points the seek the same way the rows are ordered: `>` advances an ascending page,
  // `<` advances a descending one. `contact_id` breaks ties so equal sort values can't strand or repeat a row.
  let (cmp, order) = match dir {
    ContactSortDir::Asc => (">", "ASC"),
    ContactSortDir::Desc => ("<", "DESC"),
  };

  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT character_id, contact_id, contact_name, contact_type, is_blocked, is_watched, label_ids, standing \
    FROM character_contacts WHERE character_id = ",
  );
  builder.push_bind(character_id);
  if let Some(kind) = contact_type {
    builder.push(" AND contact_type = ");
    builder.push_bind(kind.to_owned());
  }
  if let Some(term) = query {
    let term = term.trim();
    if !term.is_empty() {
      builder.push(" AND contact_name LIKE ");
      builder.push_bind(like_pattern(term));
      builder.push(" ESCAPE '\\'");
    }
  }
  if let Some(cursor) = cursor {
    builder.push(" AND (");
    builder.push(column);
    builder.push(format!(" {cmp} "));
    match cursor {
      ContactCursor::Text(value, id) => {
        builder.push_bind(value.clone());
        builder.push(" OR (");
        builder.push(column);
        builder.push(" = ");
        builder.push_bind(value.clone());
        builder.push(format!(" AND contact_id {cmp} "));
        builder.push_bind(*id);
      }
      ContactCursor::Number(value, id) => {
        builder.push_bind(*value);
        builder.push(" OR (");
        builder.push(column);
        builder.push(" = ");
        builder.push_bind(*value);
        builder.push(format!(" AND contact_id {cmp} "));
        builder.push_bind(*id);
      }
    }
    builder.push("))");
  }
  builder.push(format!(" ORDER BY {column} {order}, contact_id {order} LIMIT "));
  builder.push_bind(limit);

  let rows = builder.build_query_as::<CharacterContact>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn upsert_killmail(db: &Database, killmail: &CharacterKillEntry) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_killmails \
      (character_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, victim_corp_id, \
      victim_alliance_id, victim_damage_taken, system_id, \
      value_isk, value_destroyed_isk, value_source, value_recheck_count, value_final, \
      attacker_count, final_blow, kill_time, synced_at) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (character_id, killmail_id) DO UPDATE SET \
      kill_hash = excluded.kill_hash, is_kill = excluded.is_kill, ship_type_id = excluded.ship_type_id, \
      victim_id = excluded.victim_id, victim_corp_id = excluded.victim_corp_id, \
      victim_alliance_id = excluded.victim_alliance_id, victim_damage_taken = excluded.victim_damage_taken, \
      system_id = excluded.system_id, \
      value_isk = excluded.value_isk, value_destroyed_isk = excluded.value_destroyed_isk, \
      value_source = excluded.value_source, value_recheck_count = excluded.value_recheck_count, \
      value_final = excluded.value_final, attacker_count = excluded.attacker_count, \
      final_blow = excluded.final_blow, kill_time = excluded.kill_time, synced_at = excluded.synced_at",
  )
  .bind(killmail.character_id())
  .bind(killmail.killmail_id())
  .bind(killmail.kill_hash())
  .bind(killmail.is_kill())
  .bind(killmail.ship_type_id())
  .bind(killmail.victim_id())
  .bind(killmail.victim_corp_id())
  .bind(killmail.victim_alliance_id())
  .bind(killmail.victim_damage_taken())
  .bind(killmail.system_id())
  .bind(killmail.value_isk())
  .bind(killmail.value_destroyed_isk())
  .bind(killmail.value_source())
  .bind(killmail.value_recheck_count())
  .bind(killmail.value_final())
  .bind(killmail.attacker_count())
  .bind(killmail.final_blow())
  .bind(killmail.kill_time())
  .bind(killmail.synced_at())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn killmails(db: &Database, character_id: i64) -> Result<Vec<CharacterKillEntry>, Error> {
  let rows = sqlx::query_as::<_, CharacterKillEntry>(
    "SELECT character_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, victim_corp_id, \
      victim_alliance_id, victim_damage_taken, system_id, \
      value_isk, value_destroyed_isk, value_source, value_recheck_count, value_final, \
      attacker_count, final_blow, kill_time, synced_at FROM character_killmails \
    WHERE character_id = ? ORDER BY kill_time DESC, killmail_id DESC",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn killmails_page(
  db: &Database,
  character_id: i64,
  after: Option<(String, i64)>,
  limit: i64,
) -> Result<Vec<CharacterKillEntry>, Error> {
  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT character_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, victim_corp_id, \
      victim_alliance_id, victim_damage_taken, system_id, \
      value_isk, value_destroyed_isk, value_source, value_recheck_count, value_final, \
      attacker_count, final_blow, kill_time, synced_at FROM character_killmails WHERE character_id = ",
  );
  builder.push_bind(character_id);
  if let Some((kill_time, killmail_id)) = after {
    builder.push(" AND (kill_time < ");
    builder.push_bind(kill_time.clone());
    builder.push(" OR (kill_time = ");
    builder.push_bind(kill_time);
    builder.push(" AND killmail_id < ");
    builder.push_bind(killmail_id);
    builder.push("))");
  }
  builder.push(" ORDER BY kill_time DESC, killmail_id DESC LIMIT ");
  builder.push_bind(limit);

  let rows = builder.build_query_as::<CharacterKillEntry>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn killmail_ids(db: &Database, character_id: i64) -> Result<HashSet<i64>, Error> {
  let ids = sqlx::query_scalar::<_, i64>("SELECT killmail_id FROM character_killmails WHERE character_id = ?")
    .bind(character_id)
    .fetch_all(&db.0)
    .await?;
  Ok(ids.into_iter().collect())
}

pub async fn killmail_record_absent_recheck(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
  finalize: bool,
) -> Result<(), Error> {
  sqlx::query(
    "UPDATE character_killmails SET value_recheck_count = value_recheck_count + 1, value_final = ? \
    WHERE character_id = ? AND killmail_id = ?",
  )
  .bind(finalize)
  .bind(character_id)
  .bind(killmail_id)
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn killmail_upgrade_to_zkill(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
  value_isk: f64,
) -> Result<(), Error> {
  sqlx::query(
    "UPDATE character_killmails SET value_isk = ?, value_source = 'zkill', value_final = 1 \
    WHERE character_id = ? AND killmail_id = ?",
  )
  .bind(value_isk)
  .bind(character_id)
  .bind(killmail_id)
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn killmails_needing_recheck(db: &Database) -> Result<Vec<CharacterKillEntry>, Error> {
  let rows = sqlx::query_as::<_, CharacterKillEntry>(
    "SELECT character_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, victim_corp_id, \
      victim_alliance_id, victim_damage_taken, system_id, \
      value_isk, value_destroyed_isk, value_source, value_recheck_count, value_final, \
      attacker_count, final_blow, kill_time, synced_at FROM character_killmails \
    WHERE value_source = 'local' AND value_final = 0 ORDER BY kill_time DESC, killmail_id DESC",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn killmails_needing_detail_backfill_count(db: &Database) -> Result<i64, Error> {
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM character_killmails k \
    WHERE NOT EXISTS (SELECT 1 FROM killmail_attackers a \
        WHERE a.character_id = k.character_id AND a.killmail_id = k.killmail_id) \
      AND NOT EXISTS (SELECT 1 FROM killmail_items i \
        WHERE i.character_id = k.character_id AND i.killmail_id = k.killmail_id)",
  )
  .fetch_one(&db.0)
  .await?;
  Ok(count)
}

/// Rows in either child table count as backfilled, so an item-less structure kill stops being
/// selected once its attacker rows land rather than being re-fetched forever.
pub async fn killmails_needing_detail_backfill(db: &Database, limit: i64) -> Result<Vec<(i64, i64, String)>, Error> {
  let rows = sqlx::query_as::<_, (i64, i64, String)>(
    "SELECT character_id, killmail_id, kill_hash FROM character_killmails k \
    WHERE NOT EXISTS (SELECT 1 FROM killmail_attackers a \
        WHERE a.character_id = k.character_id AND a.killmail_id = k.killmail_id) \
      AND NOT EXISTS (SELECT 1 FROM killmail_items i \
        WHERE i.character_id = k.character_id AND i.killmail_id = k.killmail_id) \
    ORDER BY kill_time DESC, killmail_id DESC LIMIT ?",
  )
  .bind(limit)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn upsert_killmail_detail(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
  attackers: &[KillmailAttacker],
  items: &[KillmailItem],
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("DELETE FROM killmail_attackers WHERE character_id = ? AND killmail_id = ?")
    .bind(character_id)
    .bind(killmail_id)
    .execute(&mut *tx)
    .await?;
  sqlx::query("DELETE FROM killmail_items WHERE character_id = ? AND killmail_id = ?")
    .bind(character_id)
    .bind(killmail_id)
    .execute(&mut *tx)
    .await?;

  for attacker in attackers {
    sqlx::query(
      "INSERT INTO killmail_attackers \
        (character_id, killmail_id, ordinal, attacker_character_id, corporation_id, alliance_id, \
        ship_type_id, damage_done, final_blow) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(attacker.character_id())
    .bind(attacker.killmail_id())
    .bind(attacker.ordinal())
    .bind(attacker.attacker_character_id())
    .bind(attacker.corporation_id())
    .bind(attacker.alliance_id())
    .bind(attacker.ship_type_id())
    .bind(attacker.damage_done())
    .bind(attacker.final_blow())
    .execute(&mut *tx)
    .await?;
  }

  for item in items {
    sqlx::query(
      "INSERT INTO killmail_items \
        (character_id, killmail_id, ordinal, type_id, flag, quantity_destroyed, quantity_dropped, value_isk) \
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(item.character_id())
    .bind(item.killmail_id())
    .bind(item.ordinal())
    .bind(item.type_id())
    .bind(item.flag())
    .bind(item.quantity_destroyed())
    .bind(item.quantity_dropped())
    .bind(item.value_isk())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn killmail_attackers(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
) -> Result<Vec<KillmailAttacker>, Error> {
  let rows = sqlx::query_as::<_, KillmailAttacker>(
    "SELECT alliance_id, attacker_character_id, character_id, corporation_id, damage_done, final_blow, \
      killmail_id, ordinal, ship_type_id FROM killmail_attackers \
    WHERE character_id = ? AND killmail_id = ? ORDER BY ordinal",
  )
  .bind(character_id)
  .bind(killmail_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn killmail_items(db: &Database, character_id: i64, killmail_id: i64) -> Result<Vec<KillmailItem>, Error> {
  let rows = sqlx::query_as::<_, KillmailItem>(
    "SELECT character_id, flag, killmail_id, ordinal, quantity_destroyed, quantity_dropped, type_id, value_isk \
    FROM killmail_items WHERE character_id = ? AND killmail_id = ? ORDER BY ordinal",
  )
  .bind(character_id)
  .bind(killmail_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn upsert_notification(db: &Database, notification: &CharacterNotification) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO character_notifications \
      (character_id, notification_id, notif_type, sender_id, sender_type, timestamp, is_read, text, synced_at) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (character_id, notification_id) DO UPDATE SET \
      notif_type = excluded.notif_type, sender_id = excluded.sender_id, sender_type = excluded.sender_type, \
      timestamp = excluded.timestamp, is_read = excluded.is_read, text = excluded.text, synced_at = excluded.synced_at",
  )
  .bind(notification.character_id())
  .bind(notification.notification_id())
  .bind(notification.notif_type())
  .bind(notification.sender_id())
  .bind(notification.sender_type())
  .bind(notification.timestamp())
  .bind(notification.is_read())
  .bind(notification.text())
  .bind(notification.synced_at())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn mark_read(db: &Database, character_id: i64, notification_id: i64) -> Result<(), Error> {
  sqlx::query("UPDATE character_notifications SET is_read = 1 WHERE character_id = ? AND notification_id = ?")
    .bind(character_id)
    .bind(notification_id)
    .execute(&db.0)
    .await?;
  Ok(())
}

pub async fn notifications(db: &Database, character_id: i64) -> Result<Vec<CharacterNotification>, Error> {
  let rows = sqlx::query_as::<_, CharacterNotification>(
    "SELECT character_id, notification_id, notif_type, sender_id, sender_type, timestamp, is_read, text, synced_at \
    FROM character_notifications WHERE character_id = ? ORDER BY timestamp DESC, notification_id DESC",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn all_squads(db: &Database) -> Result<Vec<Squad>, Error> {
  let rows = sqlx::query_as::<_, Squad>(
    "SELECT color, created_at, description, id, name, position, updated_at FROM squads ORDER BY position",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn all_user_squads(db: &Database) -> Result<Vec<Squad>, Error> {
  let rows = sqlx::query_as::<_, Squad>(
    "SELECT color, created_at, description, id, name, position, updated_at FROM squads \
    WHERE name != ? ORDER BY position",
  )
  .bind(RESERVED_UNASSIGNED_NAME)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn assign(db: &Database, character_id: i64, squad_id: i64, position: i64) -> Result<(), Error> {
  let occupants = sqlx::query_as::<_, (i64, i64)>(
    "SELECT character_id, position FROM character_squads WHERE squad_id = ? AND character_id != ?",
  )
  .bind(squad_id)
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;

  let mut tx = db.0.begin().await?;
  for (id, slot) in cascade_positions(&occupants, character_id, position) {
    sqlx::query(
      "INSERT INTO character_squads (character_id, position, squad_id) VALUES (?, ?, ?) \
      ON CONFLICT(character_id) DO UPDATE SET position = excluded.position, squad_id = excluded.squad_id",
    )
    .bind(id)
    .bind(slot)
    .bind(squad_id)
    .execute(&mut *tx)
    .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn create(db: &Database, name: &str, description: Option<&str>, color: Option<&str>) -> Result<Squad, Error> {
  if name == RESERVED_UNASSIGNED_NAME {
    return Err(Error::ReservedSquad);
  }
  create_raw(db, name, description, color).await
}

pub async fn delete_squad(db: &Database, id: i64) -> Result<(), Error> {
  let rows = sqlx::query("DELETE FROM squads WHERE id = ? AND name != ?")
    .bind(id)
    .bind(RESERVED_UNASSIGNED_NAME)
    .execute(&db.0)
    .await?
    .rows_affected();
  if rows == 0 && is_reserved_id(db, id).await? {
    return Err(Error::ReservedSquad);
  }
  Ok(())
}

pub async fn get_squad(db: &Database, id: i64) -> Result<Option<Squad>, Error> {
  let row = sqlx::query_as::<_, Squad>(
    "SELECT color, created_at, description, id, name, position, updated_at FROM squads WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

pub async fn get_or_create_unassigned(db: &Database) -> Result<Squad, Error> {
  if let Some(squad) = by_name(db, RESERVED_UNASSIGNED_NAME).await? {
    return Ok(squad);
  }
  create_raw(db, RESERVED_UNASSIGNED_NAME, None, None).await
}

pub fn is_unassigned(squad: &Squad) -> bool {
  squad.name() == RESERVED_UNASSIGNED_NAME
}

pub async fn members(db: &Database, squad_id: i64) -> Result<Vec<i64>, Error> {
  let ids =
    sqlx::query_scalar::<_, i64>("SELECT character_id FROM character_squads WHERE squad_id = ? ORDER BY position")
      .bind(squad_id)
      .fetch_all(&db.0)
      .await?;
  Ok(ids)
}

pub async fn memberships(db: &Database) -> Result<Vec<CharacterSquad>, Error> {
  let rows = sqlx::query_as::<_, CharacterSquad>(
    "SELECT character_id, position, squad_id FROM character_squads ORDER BY squad_id, position",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn normalize(db: &Database, squad_id: i64) -> Result<(), Error> {
  let rows = sqlx::query_as::<_, (i64, i64)>(
    "SELECT character_id, position FROM character_squads WHERE squad_id = ? ORDER BY position, character_id",
  )
  .bind(squad_id)
  .fetch_all(&db.0)
  .await?;

  let mut seen = HashSet::new();
  let has_duplicates = rows.iter().any(|(_, position)| !seen.insert(*position));
  if !has_duplicates {
    return Ok(());
  }

  let mut tx = db.0.begin().await?;
  for (index, (character_id, _)) in rows.iter().enumerate() {
    sqlx::query("UPDATE character_squads SET position = ? WHERE character_id = ?")
      .bind(index as i64)
      .bind(character_id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn reorder(db: &Database, ordered_ids: &[i64]) -> Result<(), Error> {
  let now = Utc::now().timestamp();
  let mut tx = db.0.begin().await?;
  for (position, id) in ordered_ids.iter().enumerate() {
    sqlx::query("UPDATE squads SET position = ?, updated_at = ? WHERE id = ? AND name != ?")
      .bind(position as i64)
      .bind(now)
      .bind(id)
      .bind(RESERVED_UNASSIGNED_NAME)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn unassign(db: &Database, character_id: i64) -> Result<(), Error> {
  let unassigned = get_or_create_unassigned(db).await?;
  let position = members(db, unassigned.id()).await?.len() as i64;
  assign(db, character_id, unassigned.id(), position).await
}

pub async fn unassigned_id(db: &Database) -> Result<Option<i64>, Error> {
  Ok(by_name(db, RESERVED_UNASSIGNED_NAME).await?.map(|squad| squad.id()))
}

pub async fn update(
  db: &Database,
  id: i64,
  name: &str,
  description: Option<&str>,
  color: Option<&str>,
) -> Result<(), Error> {
  if name == RESERVED_UNASSIGNED_NAME || is_reserved_id(db, id).await? {
    return Err(Error::ReservedSquad);
  }
  let now = Utc::now().timestamp();
  sqlx::query("UPDATE squads SET color = ?, description = ?, name = ?, updated_at = ? WHERE id = ?")
    .bind(color)
    .bind(description)
    .bind(name)
    .bind(now)
    .bind(id)
    .execute(&db.0)
    .await?;
  Ok(())
}

async fn by_name(db: &Database, name: &str) -> Result<Option<Squad>, Error> {
  let row = sqlx::query_as::<_, Squad>(
    "SELECT color, created_at, description, id, name, position, updated_at FROM squads WHERE name = ?",
  )
  .bind(name)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

fn cascade_positions(occupants: &[(i64, i64)], dragged: i64, target: i64) -> Vec<(i64, i64)> {
  let mut by_position: HashMap<i64, i64> = occupants.iter().map(|&(id, position)| (position, id)).collect();
  let mut current_id = dragged;
  let mut current_position = target;
  while let Some(displaced) = by_position.insert(current_position, current_id) {
    current_id = displaced;
    current_position += 1;
  }
  by_position.into_iter().map(|(position, id)| (id, position)).collect()
}

async fn create_raw(db: &Database, name: &str, description: Option<&str>, color: Option<&str>) -> Result<Squad, Error> {
  let now = Utc::now().timestamp();
  let squad = sqlx::query_as::<_, Squad>(
    "INSERT INTO squads (color, created_at, description, name, position, updated_at) \
    VALUES (?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM squads), ?) \
    RETURNING color, created_at, description, id, name, position, updated_at",
  )
  .bind(color)
  .bind(now)
  .bind(description)
  .bind(name)
  .bind(now)
  .fetch_one(&db.0)
  .await?;
  Ok(squad)
}

async fn is_reserved_id(db: &Database, id: i64) -> Result<bool, Error> {
  Ok(get_squad(db, id).await?.as_ref().is_some_and(is_unassigned))
}

pub async fn replace_standings_for_character(
  db: &Database,
  character_id: i64,
  standings: &[CharacterStanding],
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("DELETE FROM character_standings WHERE character_id = ?")
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

  for standing in standings {
    sqlx::query(
      "INSERT INTO character_standings (character_id, from_id, from_type, standing, from_name) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(standing.character_id())
    .bind(standing.from_id())
    .bind(standing.from_type())
    .bind(standing.standing())
    .bind(standing.from_name())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn standings(db: &Database, character_id: i64) -> Result<Vec<CharacterStanding>, Error> {
  let rows = sqlx::query_as::<_, CharacterStanding>(
    "SELECT character_id, from_id, from_name, from_type, standing FROM character_standings \
    WHERE character_id = ? ORDER BY from_type, from_id",
  )
  .bind(character_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    store,
    store::model::{Alliance, Bloodline, Character, Corporation, Faction, Gender, Race},
  };

  fn make_alliance() -> Alliance {
    Alliance::new(
      99_000_001,
      90_000_001,
      12_345_678,
      "2003-01-01",
      "Test Alliance Please Ignore",
      "TAPI",
    )
  }

  fn make_bloodline() -> Bloodline {
    Bloodline::new(1, 90_000_001, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4)
  }

  fn make_character() -> Character {
    Character::new(
      12_345_678,
      1,
      90_000_001,
      2,
      "2003-05-12",
      Gender::Male,
      "Test Character",
    )
  }

  fn make_corporation() -> Corporation {
    let mut corp = Corporation::new(90_000_001, "Test Corporation", "TSTC");
    corp.set_ceo_id(12_345_678);
    corp.set_creator_id(12_345_678);
    corp.set_member_count(100);
    corp.set_tax_rate(0.1);
    corp
  }

  fn make_faction() -> Faction {
    Faction::new(500_001, "Caldari State", true, 1.0, 100, 50)
  }

  fn make_race() -> Race {
    Race::new(2, 99_000_001, "A race.", "Caldari")
  }

  mod killmails_page {
    use pretty_assertions::assert_eq;

    use super::*;

    fn kill(killmail_id: i64, kill_time: &str) -> CharacterKillEntry {
      CharacterKillEntry {
        attacker_count: 1,
        character_id: 42,
        final_blow: true,
        is_kill: true,
        kill_hash: format!("hash{killmail_id}"),
        kill_time: kill_time.to_owned(),
        killmail_id,
        ship_type_id: 670,
        synced_at: "2024-01-01T00:00:00Z".to_owned(),
        system_id: 30_000_142,
        value_destroyed_isk: 0.0,
        value_final: false,
        value_isk: 0.0,
        value_recheck_count: 0,
        value_source: "local".to_owned(),
        victim_alliance_id: None,
        victim_corp_id: None,
        victim_damage_taken: 0,
        victim_id: None,
      }
    }

    fn ids(rows: &[CharacterKillEntry]) -> Vec<i64> {
      rows.iter().map(|row| row.killmail_id()).collect()
    }

    async fn seed_kills(db: &Database) {
      let character = Character::new(42, 1, 90_000_001, 2, "2003-05-12", Gender::Male, "Test Character");
      insert_with_org(
        db,
        &character,
        &make_bloodline(),
        &make_race(),
        &make_corporation(),
        Some(&make_alliance()),
        None,
      )
      .await
      .unwrap();

      // Two share a timestamp so the killmail_id tiebreaker is exercised.
      upsert_killmail(db, &kill(100, "2024-03-01T00:00:00Z")).await.unwrap();
      upsert_killmail(db, &kill(101, "2024-02-01T00:00:00Z")).await.unwrap();
      upsert_killmail(db, &kill(102, "2024-02-01T00:00:00Z")).await.unwrap();
      upsert_killmail(db, &kill(103, "2024-01-01T00:00:00Z")).await.unwrap();
    }

    #[tokio::test]
    async fn it_orders_by_kill_time_then_killmail_id_descending() {
      let db = store::open_test().await.unwrap();
      seed_kills(&db).await;

      let rows = killmails_page(&db, 42, None, 100).await.unwrap();

      assert_eq!(ids(&rows), vec![100, 102, 101, 103]);
    }

    #[tokio::test]
    async fn it_pages_through_kills_without_overlap_via_the_cursor() {
      let db = store::open_test().await.unwrap();
      seed_kills(&db).await;

      let first = killmails_page(&db, 42, None, 2).await.unwrap();
      assert_eq!(ids(&first), vec![100, 102]);

      let last = first.last().unwrap();
      let cursor = Some((last.kill_time().clone(), last.killmail_id()));
      let second = killmails_page(&db, 42, cursor, 2).await.unwrap();

      assert_eq!(ids(&second), vec![101, 103]);
    }

    #[tokio::test]
    async fn it_returns_an_empty_page_past_the_last_kill() {
      let db = store::open_test().await.unwrap();
      seed_kills(&db).await;

      let beyond = killmails_page(&db, 42, Some(("2023-01-01T00:00:00Z".to_owned(), 0)), 10)
        .await
        .unwrap();

      assert!(beyond.is_empty());
    }
  }

  mod search {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{CharacterSkillqueue, CharacterTelemetry, OwnerType},
      repo::infra,
      search::parse,
    };

    const COBALT_EDGE: i64 = 98_000_001;
    const COBALT_RECRUIT: i64 = 1003;
    const COBALT_SCOUT: i64 = 1001;
    const NOW: &str = "2026-01-01T00:00:00Z";
    const RED_BARON: i64 = 1002;
    const RED_FEDERATION: i64 = 98_000_002;

    fn corp(id: i64, name: &str, ticker: &str) -> Corporation {
      let mut corporation = Corporation::new(id, name, ticker);
      corporation.set_ceo_id(12_345_678);
      corporation.set_creator_id(12_345_678);
      corporation.set_member_count(100);
      corporation.set_tax_rate(0.1);
      corporation
    }

    async fn seed_owned(db: &Database, id: i64, name: &str, corporation: &Corporation) {
      let character = Character::new(id, 1, corporation.id(), 2, "2003-05-12", Gender::Male, name);

      insert_with_org(
        db,
        &character,
        &make_bloodline(),
        &make_race(),
        corporation,
        Some(&make_alliance()),
        None,
      )
      .await
      .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    async fn set_telemetry(db: &Database, character_id: i64, station_id: Option<i64>) {
      let telemetry = CharacterTelemetry {
        character_id,
        online: true,
        ship_item_id: None,
        ship_name: None,
        ship_type_id: None,
        solar_system_id: 30_000_142,
        station_id,
        structure_id: None,
        synced_at: 1_700_000_000,
      };

      upsert_telemetry(db, &telemetry).await.unwrap();
    }

    async fn set_active_training(db: &Database, character_id: i64) {
      let entry = CharacterSkillqueue {
        character_id,
        finish_date: Some("2026-06-01T00:00:00Z".to_string()),
        finished_level: 4,
        level_end_sp: None,
        level_start_sp: None,
        queue_position: 0,
        skill_id: 3300,
        start_date: Some("2025-12-01T00:00:00Z".to_string()),
        training_start_sp: None,
      };

      replace_skillqueue(db, character_id, &[entry]).await.unwrap();
    }

    async fn seed_roster(db: &Database) {
      let cobalt = corp(COBALT_EDGE, "Cobalt Edge", "CBLT");
      let red = corp(RED_FEDERATION, "Red Federation", "REDF");

      seed_owned(db, COBALT_SCOUT, "Cobalt Scout", &cobalt).await;
      seed_owned(db, COBALT_RECRUIT, "Cobalt Recruit", &cobalt).await;
      seed_owned(db, RED_BARON, "Red Baron", &red).await;

      set_telemetry(db, COBALT_SCOUT, Some(60_003_760)).await;
      set_telemetry(db, RED_BARON, None).await;

      set_active_training(db, COBALT_SCOUT).await;

      let pvp = infra::create(db, "PvP", None, Some("#ff0000")).await.unwrap();
      let alt = infra::create(db, "Alt", None, None).await.unwrap();
      infra::assign(db, ENTITY_TYPE_CHARACTER, COBALT_SCOUT, pvp.id())
        .await
        .unwrap();
      infra::assign(db, ENTITY_TYPE_CHARACTER, RED_BARON, alt.id())
        .await
        .unwrap();
    }

    async fn matching(db: &Database, query: &str) -> Vec<i64> {
      search(db, &parse(query), NOW)
        .await
        .unwrap()
        .iter()
        .map(|row| row.character_id)
        .collect()
    }

    #[tokio::test]
    async fn it_matches_corp_name_or_ticker() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "corp:cobalt").await, vec![COBALT_RECRUIT, COBALT_SCOUT]);
      assert_eq!(matching(&db, "corp:redf").await, vec![RED_BARON]);
    }

    #[tokio::test]
    async fn it_matches_tags_with_or_within_the_key() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "tag:pvp").await, vec![COBALT_SCOUT]);
      assert_eq!(matching(&db, "tag:pvp,alt").await, vec![COBALT_SCOUT, RED_BARON]);
    }

    #[tokio::test]
    async fn it_negates_a_tag_filter() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "-tag:alt").await, vec![COBALT_RECRUIT, COBALT_SCOUT]);
    }

    #[tokio::test]
    async fn it_filters_by_docked_status() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "status:docked").await, vec![COBALT_SCOUT]);
      assert_eq!(matching(&db, "status:in-space").await, vec![RED_BARON]);
    }

    #[tokio::test]
    async fn it_filters_by_training_state() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "training:active").await, vec![COBALT_SCOUT]);
      assert_eq!(matching(&db, "training:idle").await, vec![COBALT_RECRUIT, RED_BARON]);
    }

    #[tokio::test]
    async fn it_matches_free_text_across_facets() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "cobalt").await, vec![COBALT_RECRUIT, COBALT_SCOUT]);
      assert_eq!(matching(&db, "pvp").await, vec![COBALT_SCOUT]);
    }

    #[tokio::test]
    async fn it_ands_multiple_tokens() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "corp:cobalt tag:pvp").await, vec![COBALT_SCOUT]);
    }

    #[tokio::test]
    async fn it_returns_the_whole_roster_for_an_empty_query() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      assert_eq!(matching(&db, "").await, vec![COBALT_RECRUIT, COBALT_SCOUT, RED_BARON]);
    }

    #[tokio::test]
    async fn it_carries_the_card_projection_fields() {
      let db = store::open_test().await.unwrap();
      seed_roster(&db).await;

      let rows = search(&db, &parse("tag:pvp"), NOW).await.unwrap();

      assert_eq!(rows.len(), 1);
      let scout = &rows[0];
      assert_eq!(scout.corp_ticker.as_deref(), Some("CBLT"));
      assert_eq!(scout.docked, Some(true));
      assert_eq!(
        scout.training.as_ref().map(|training| training.skill_name.as_deref()),
        Some(None)
      );
      assert_eq!(scout.training.as_ref().map(|training| training.skill_id), Some(3300));
      assert_eq!(scout.training.as_ref().map(|training| training.finished_level), Some(4));

      assert_eq!(scout.tags.len(), 1);
      assert_eq!(scout.tags[0].name, "PvP");
      assert_eq!(scout.tags[0].color_hex.as_deref(), Some("#ff0000"));
    }
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_characters_exist() {
      let db = store::open_test().await.unwrap();

      let result = all(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_returns_all_stored_characters() {
      let db = store::open_test().await.unwrap();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();
      let character = make_character();
      insert_with_org(&db, &character, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();

      let result = all(&db).await.unwrap();

      assert_eq!(result.len(), 1);
    }
  }

  mod all_owned {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{model::OwnerType, repo::infra};

    #[tokio::test]
    async fn it_returns_only_credentialed_characters() {
      let db = store::open_test().await.unwrap();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();

      let owned = make_character();
      insert_with_org(&db, &owned, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(&db, owned.id(), OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();

      let reference = Character::new(
        87_654_321,
        1,
        90_000_001,
        2,
        "2003-05-12",
        Gender::Male,
        "Reference Character",
      );
      insert_with_org(&db, &reference, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();

      let owned_rows = all_owned(&db).await.unwrap();
      let all_rows = all(&db).await.unwrap();

      assert_eq!(all_rows.len(), 2);
      assert_eq!(owned_rows.len(), 1);
      assert_eq!(owned_rows[0].id(), owned.id());
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get(&db, 999).await.unwrap();

      assert_eq!(result, None);
    }

    #[tokio::test]
    async fn it_returns_the_character_for_a_known_id() {
      let db = store::open_test().await.unwrap();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();
      let character = make_character();
      insert_with_org(&db, &character, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();

      let result = get(&db, 12_345_678).await.unwrap();

      assert!(result.is_some());
      let found = result.unwrap();
      assert_eq!(found.id(), 12_345_678);
      assert_eq!(found.name(), "Test Character");
      assert_eq!(found.corporation_id(), 90_000_001);
    }
  }

  mod delete {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::ENTITY_TYPE_CORPORATION,
      repo::{infra, org},
    };

    #[tokio::test]
    async fn it_cleans_up_entity_tags_without_a_foreign_key_cascade() {
      let db = store::open_test().await.unwrap();
      let corporation = make_corporation();
      let character = make_character();
      let id = character.id();
      insert_with_org(
        &db,
        &character,
        &make_bloodline(),
        &make_race(),
        &corporation,
        Some(&make_alliance()),
        None,
      )
      .await
      .unwrap();
      let shared = infra::create(&db, "Shared", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, id, shared.id())
        .await
        .unwrap();
      infra::assign(&db, ENTITY_TYPE_CORPORATION, corporation.id(), shared.id())
        .await
        .unwrap();

      delete(&db, id).await.unwrap();

      assert!(
        infra::members(&db, shared.id(), ENTITY_TYPE_CHARACTER)
          .await
          .unwrap()
          .is_empty()
      );
      assert_eq!(
        infra::members(&db, shared.id(), ENTITY_TYPE_CORPORATION).await.unwrap(),
        vec![corporation.id()]
      );
    }

    #[tokio::test]
    async fn it_removes_the_character_its_credential_and_all_cascaded_data() {
      let db = store::open_test().await.unwrap();
      let corporation = make_corporation();
      let character = make_character();
      let id = character.id();
      insert_with_org(
        &db,
        &character,
        &make_bloodline(),
        &make_race(),
        &corporation,
        Some(&make_alliance()),
        None,
      )
      .await
      .unwrap();
      infra::upsert(&db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
      let squad = create(&db, "Crew", None, None).await.unwrap();
      assign(&db, id, squad.id(), 0).await.unwrap();
      let main = infra::create(&db, "Main", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, id, main.id()).await.unwrap();
      sqlx::query(
        "INSERT INTO character_skills (character_id, skill_id, active_skill_level, \
        skillpoints_in_skill, trained_skill_level) VALUES (?, ?, ?, ?, ?)",
      )
      .bind(id)
      .bind(100_i64)
      .bind(5_i64)
      .bind(1_000_000_i64)
      .bind(5_i64)
      .execute(&db.0)
      .await
      .unwrap();

      delete(&db, id).await.unwrap();

      assert_eq!(get(&db, id).await.unwrap(), None);
      assert_eq!(infra::get(&db, id, OwnerType::Character).await.unwrap(), None);
      assert!(memberships(&db).await.unwrap().is_empty());
      assert!(infra::memberships(&db, ENTITY_TYPE_CHARACTER).await.unwrap().is_empty());
      let skills: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM character_skills WHERE character_id = ?")
        .bind(id)
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(skills, 0);
      assert!(org::get_corporation(&db, corporation.id()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_re_adds_a_character_cleanly_after_a_remove() {
      let db = store::open_test().await.unwrap();
      let corporation = make_corporation();
      let character = make_character();
      let id = character.id();
      insert_with_org(
        &db,
        &character,
        &make_bloodline(),
        &make_race(),
        &corporation,
        Some(&make_alliance()),
        None,
      )
      .await
      .unwrap();
      infra::upsert(&db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();

      delete(&db, id).await.unwrap();
      upsert_with_org(
        &db,
        &character,
        &make_bloodline(),
        &make_race(),
        &corporation,
        Some(&make_alliance()),
        None,
      )
      .await
      .expect("re-adding a character whose corp row survived the remove must not 787");

      assert!(get(&db, id).await.unwrap().is_some());
      assert!(org::get_corporation(&db, corporation.id()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_is_a_noop_for_an_unknown_character() {
      let db = store::open_test().await.unwrap();

      delete(&db, 999).await.unwrap();
    }
  }

  mod insert_with_org {
    use super::*;

    #[tokio::test]
    async fn it_inserts_the_full_org_stack_without_fk_violation() {
      let db = store::open_test().await.unwrap();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();
      let character = make_character();

      insert_with_org(&db, &character, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();

      let result = get(&db, 12_345_678).await.unwrap();
      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_inserts_with_faction_when_character_has_no_personal_alliance() {
      let db = store::open_test().await.unwrap();
      let faction = make_faction();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();
      let character = make_character();

      insert_with_org(
        &db,
        &character,
        &bloodline,
        &race,
        &corporation,
        Some(&alliance),
        Some(&faction),
      )
      .await
      .unwrap();

      let result = get(&db, 12_345_678).await.unwrap();
      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_is_idempotent_on_repeat_calls() {
      let db = store::open_test().await.unwrap();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();
      let character = make_character();

      insert_with_org(&db, &character, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();
      insert_with_org(&db, &character, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();

      let result = all(&db).await.unwrap();
      assert_eq!(result.len(), 1);
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stores_a_new_character() {
      let db = store::open_test().await.unwrap();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();
      let character = make_character();
      insert_with_org(&db, &character, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();

      let result = get(&db, 12_345_678).await.unwrap();

      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_updates_mutable_fields_on_conflict() {
      let db = store::open_test().await.unwrap();
      let alliance = make_alliance();
      let race = make_race();
      let corporation = make_corporation();
      let bloodline = make_bloodline();
      let character = make_character();
      insert_with_org(&db, &character, &bloodline, &race, &corporation, Some(&alliance), None)
        .await
        .unwrap();

      let mut updated = make_character();
      updated.set_description("Updated description");
      updated.set_security_status(0.5);
      updated.set_title("CEO");
      upsert(&db, &updated).await.unwrap();

      let result = get(&db, 12_345_678).await.unwrap().unwrap();
      assert_eq!(result.description().as_deref(), Some("Updated description"));
      assert_eq!(result.security_status(), Some(0.5));
      assert_eq!(result.title().as_deref(), Some("CEO"));
    }
  }

  mod upsert_with_org {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::repo::org;

    #[tokio::test]
    async fn it_inserts_the_full_org_stack_when_absent() {
      let db = store::open_test().await.unwrap();

      upsert_with_org(
        &db,
        &make_character(),
        &make_bloodline(),
        &make_race(),
        &make_corporation(),
        Some(&make_alliance()),
        None,
      )
      .await
      .unwrap();

      assert!(get(&db, 12_345_678).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_refreshes_mutable_character_and_corporation_rows() {
      let db = store::open_test().await.unwrap();
      let bloodline = make_bloodline();
      let race = make_race();
      let alliance = make_alliance();
      insert_with_org(
        &db,
        &make_character(),
        &bloodline,
        &race,
        &make_corporation(),
        Some(&alliance),
        None,
      )
      .await
      .unwrap();

      let mut updated_char = make_character();
      updated_char.set_title("CEO");
      updated_char.set_security_status(0.5);
      let mut updated_corp = make_corporation();
      updated_corp.set_member_count(250);
      upsert_with_org(
        &db,
        &updated_char,
        &bloodline,
        &race,
        &updated_corp,
        Some(&alliance),
        None,
      )
      .await
      .unwrap();

      let char_row = get(&db, 12_345_678).await.unwrap().unwrap();
      let corp_row = org::get_corporation(&db, 90_000_001).await.unwrap().unwrap();
      assert_eq!(char_row.title().as_deref(), Some("CEO"));
      assert_eq!(char_row.security_status(), Some(0.5));
      assert_eq!(corp_row.member_count(), Some(250));
    }

    #[tokio::test]
    async fn it_names_the_entities_when_a_corporation_fk_violates_at_commit() {
      let db = store::open_test().await.unwrap();
      let character = make_character();
      // Persist a DIFFERENT corp than the character's own (90_000_001), leaving the deferred
      // characters.corporation_id FK dangling so the commit 787s — the shape of the NPC-CEO bug.
      let mut other_corp = Corporation::new(90_000_002, "Other Corporation", "OTHR");
      other_corp.set_ceo_id(12_345_678);
      other_corp.set_creator_id(12_345_678);
      other_corp.set_member_count(1);
      other_corp.set_tax_rate(0.0);

      let error = upsert_with_org(
        &db,
        &character,
        &make_bloodline(),
        &make_race(),
        &other_corp,
        None,
        None,
      )
      .await
      .expect_err("a dangling corporation_id FK fails at commit");

      assert!(
        error.is_foreign_key_violation(),
        "the 787 is classified as a foreign-key violation"
      );
      let message = error.to_string();
      assert!(message.contains("12345678"), "names the character: {message}");
      assert!(
        message.contains("90000001"),
        "names the dangling corporation_id: {message}"
      );
      assert!(
        message.contains("90000002"),
        "names the corporation that was inserted: {message}"
      );
    }
  }
}

#[cfg(test)]
mod attributes_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn make_attributes(character_id: i64, unallocated_sp: i64) -> CharacterAttributes {
    CharacterAttributes {
      accrued_remap_cooldown_date: Some("2026-01-01T00:00:00Z".to_owned()),
      bonus_remaps: 2,
      character_id,
      charisma: 19,
      intelligence: 20,
      last_remap_date: Some("2025-06-01T00:00:00Z".to_owned()),
      memory: 21,
      perception: 22,
      unallocated_sp,
      willpower: 23,
    }
  }

  mod attributes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_no_row_exists() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(super::attributes(&db, 42).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_round_trips_the_stored_row_with_unallocated_sp() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert_attributes(&db, &make_attributes(42, 15_000)).await.unwrap();

      let result = super::attributes(&db, 42).await.unwrap().unwrap();

      assert_eq!(result.unallocated_sp(), 15_000);
      assert_eq!(result.charisma(), 19);
      assert_eq!(result.willpower(), 23);
      assert_eq!(result.bonus_remaps(), 2);
      assert_eq!(result.last_remap_date().as_deref(), Some("2025-06-01T00:00:00Z"));
      assert_eq!(
        result.accrued_remap_cooldown_date().as_deref(),
        Some("2026-01-01T00:00:00Z")
      );
    }

    #[tokio::test]
    async fn it_round_trips_null_remap_dates() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut attributes = make_attributes(42, 0);
      attributes.last_remap_date = None;
      attributes.accrued_remap_cooldown_date = None;
      upsert_attributes(&db, &attributes).await.unwrap();

      let result = super::attributes(&db, 42).await.unwrap().unwrap();

      assert_eq!(result.last_remap_date(), &None);
      assert_eq!(result.accrued_remap_cooldown_date(), &None);
    }
  }

  mod upsert_attributes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_the_existing_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_attributes(&db, &make_attributes(42, 100)).await.unwrap();

      let mut updated = make_attributes(42, 500);
      updated.charisma = 27;
      super::upsert_attributes(&db, &updated).await.unwrap();

      let result = attributes(&db, 42).await.unwrap().unwrap();
      assert_eq!(result.unallocated_sp(), 500);
      assert_eq!(result.charisma(), 27);
    }
  }
}

#[cfg(test)]
mod implants_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn full_set(character_id: i64, bonus: i64) -> Vec<CharacterImplant> {
    (164..=168)
      .map(|attribute_id| CharacterImplant {
        attribute_id,
        bonus,
        character_id,
      })
      .collect()
  }

  mod implants {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_stored_rows_in_attribute_order() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_implants(&db, 42, &full_set(42, 4)).await.unwrap();

      let result = super::implants(&db, 42).await.unwrap();

      assert_eq!(
        result.iter().map(|i| i.attribute_id()).collect::<Vec<_>>(),
        [164, 165, 166, 167, 168]
      );
    }
  }

  mod replace_implants {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_the_prior_five_row_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_implants(&db, 42, &full_set(42, 4)).await.unwrap();

      super::replace_implants(
        &db,
        42,
        &[CharacterImplant {
          attribute_id: 166,
          bonus: 5,
          character_id: 42,
        }],
      )
      .await
      .unwrap();

      let result = implants(&db, 42).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].attribute_id(), 166);
      assert_eq!(result[0].bonus(), 5);
    }

    #[tokio::test]
    async fn it_leaves_other_characters_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::replace_implants(&db, 42, &full_set(42, 4)).await.unwrap();
      super::replace_implants(&db, 43, &full_set(43, 3)).await.unwrap();

      super::replace_implants(&db, 42, &[]).await.unwrap();

      assert_eq!(implants(&db, 43).await.unwrap().len(), 5);
    }
  }
}

#[cfg(test)]
mod skills_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn make_skill(character_id: i64, skill_id: i64, sp: i64) -> CharacterSkill {
    CharacterSkill {
      active_skill_level: 4,
      character_id,
      skill_id,
      skillpoints_in_skill: sp,
      trained_skill_level: 5,
    }
  }

  mod skills {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_stored_sheet() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_skills(&db, 42, &[make_skill(42, 3300, 100), make_skill(42, 3301, 200)])
        .await
        .unwrap();

      let result = super::skills(&db, 42).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod replace_skills {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_the_whole_sheet() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_skills(&db, 42, &[make_skill(42, 3300, 100), make_skill(42, 3301, 200)])
        .await
        .unwrap();

      super::replace_skills(&db, 42, &[make_skill(42, 3400, 500)])
        .await
        .unwrap();

      let result = skills(&db, 42).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skill_id(), 3400);
      assert_eq!(result[0].skillpoints_in_skill(), 500);
    }

    #[tokio::test]
    async fn it_leaves_other_characters_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::replace_skills(&db, 42, &[make_skill(42, 3300, 100)])
        .await
        .unwrap();
      super::replace_skills(&db, 43, &[make_skill(43, 3301, 200)])
        .await
        .unwrap();

      super::replace_skills(&db, 42, &[]).await.unwrap();

      assert_eq!(skills(&db, 43).await.unwrap().len(), 1);
    }
  }
}

#[cfg(test)]
mod skillqueue_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn make_entry(character_id: i64, queue_position: i64, skill_id: i64) -> CharacterSkillqueue {
    CharacterSkillqueue {
      character_id,
      finish_date: Some("2026-06-01T00:00:00Z".to_owned()),
      finished_level: 5,
      level_end_sp: Some(256_000),
      level_start_sp: Some(45_255),
      queue_position,
      skill_id,
      start_date: Some("2026-05-01T00:00:00Z".to_owned()),
      training_start_sp: Some(45_255),
    }
  }

  mod current_skillqueue {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_empty_queue() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(super::current_skillqueue(&db, 42, Utc::now()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_returns_the_position_zero_entry() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_skillqueue(&db, 42, &[make_entry(42, 1, 3301), make_entry(42, 0, 3300)])
        .await
        .unwrap();

      let entry = super::current_skillqueue(&db, 42, Utc::now()).await.unwrap().unwrap();

      assert_eq!(entry.queue_position(), 0);
      assert_eq!(entry.skill_id(), 3300);
    }

    #[tokio::test]
    async fn it_skips_a_finished_head_skill_for_the_one_still_training() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let now = Utc::now();
      let mut finished = make_entry(42, 0, 3300);
      finished.start_date = Some(
        (now - chrono::Duration::days(2))
          .format("%Y-%m-%dT%H:%M:%SZ")
          .to_string(),
      );
      finished.finish_date = Some(
        (now - chrono::Duration::days(1))
          .format("%Y-%m-%dT%H:%M:%SZ")
          .to_string(),
      );
      let mut training = make_entry(42, 1, 3301);
      training.start_date = Some(
        (now - chrono::Duration::days(1))
          .format("%Y-%m-%dT%H:%M:%SZ")
          .to_string(),
      );
      training.finish_date = Some(
        (now + chrono::Duration::days(1))
          .format("%Y-%m-%dT%H:%M:%SZ")
          .to_string(),
      );
      replace_skillqueue(&db, 42, &[finished, training]).await.unwrap();

      let entry = super::current_skillqueue(&db, 42, now).await.unwrap().unwrap();

      assert_eq!(entry.queue_position(), 1);
      assert_eq!(entry.skill_id(), 3301);
    }
  }

  mod skillqueue {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_entries_in_queue_position_order() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      replace_skillqueue(
        &db,
        42,
        &[
          make_entry(42, 2, 3302),
          make_entry(42, 0, 3300),
          make_entry(42, 1, 3301),
        ],
      )
      .await
      .unwrap();

      let result = super::skillqueue(&db, 42).await.unwrap();

      assert_eq!(
        result.iter().map(|e| e.skill_id()).collect::<Vec<_>>(),
        [3300, 3301, 3302]
      );
    }
  }

  mod replace_skillqueue {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_overwrites_the_whole_queue() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_skillqueue(&db, 42, &[make_entry(42, 0, 3300), make_entry(42, 1, 3301)])
        .await
        .unwrap();

      super::replace_skillqueue(&db, 42, &[make_entry(42, 0, 3400)])
        .await
        .unwrap();

      let result = skillqueue(&db, 42).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].skill_id(), 3400);
    }

    #[tokio::test]
    async fn it_clears_the_queue_when_given_no_entries() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::replace_skillqueue(&db, 42, &[make_entry(42, 0, 3300)])
        .await
        .unwrap();

      super::replace_skillqueue(&db, 42, &[]).await.unwrap();

      assert_eq!(skillqueue(&db, 42).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn it_leaves_other_characters_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_character(&db, 43).await;
      super::replace_skillqueue(&db, 42, &[make_entry(42, 0, 3300)])
        .await
        .unwrap();
      super::replace_skillqueue(&db, 43, &[make_entry(43, 0, 3301)])
        .await
        .unwrap();

      super::replace_skillqueue(&db, 42, &[]).await.unwrap();

      assert_eq!(skillqueue(&db, 43).await.unwrap().len(), 1);
    }
  }
}

#[cfg(test)]
mod state_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
  };

  async fn insert_character(db: &Database, id: i64, name: &str) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn insert_skill(db: &Database, character_id: i64, skill_id: i64, sp: i64) {
    sqlx::query("INSERT INTO character_skills (character_id, skill_id, active_skill_level, skillpoints_in_skill, trained_skill_level) VALUES (?, ?, ?, ?, ?)")
      .bind(character_id)
      .bind(skill_id)
      .bind(0)
      .bind(sp)
      .bind(0)
      .execute(&db.0)
      .await
      .unwrap();
  }

  async fn insert_journal(db: &Database, id: i64, character_id: i64, amount: Option<f64>, balance: Option<f64>) {
    sqlx::query("INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) VALUES (?, ?, ?, ?, ?, ?, ?)")
      .bind(id)
      .bind(character_id)
      .bind("2026-01-01")
      .bind("Test")
      .bind("test")
      .bind(amount)
      .bind(balance)
      .execute(&db.0)
      .await
      .unwrap();
  }

  async fn insert_telemetry(db: &Database, character_id: i64, online: bool, synced_at: i64) {
    sqlx::query(
      "INSERT INTO character_telemetry (character_id, online, solar_system_id, synced_at) VALUES (?, ?, ?, ?)",
    )
    .bind(character_id)
    .bind(online)
    .bind(30_000_142)
    .bind(synced_at)
    .execute(&db.0)
    .await
    .unwrap();
  }

  mod state {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_absent() {
      let db = store::open_test().await.unwrap();
      let result = super::state(&db, 1).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn it_decodes_telemetry_columns_when_a_snapshot_exists() {
      let db = store::open_test().await.unwrap();
      insert_character(&db, 1, "Pilot").await;
      insert_telemetry(&db, 1, true, 1_700_000_000).await;

      let state = super::state(&db, 1).await.unwrap().unwrap();

      assert_eq!(state.online, Some(true));
      assert_eq!(state.solar_system_id, Some(30_000_142));
      assert_eq!(state.synced_at, Some(1_700_000_000));
    }
  }

  mod all_states {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_rows() {
      let db = store::open_test().await.unwrap();
      let result = super::all_states(&db).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod total_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_null_before_skills_exist() {
      let db = store::open_test().await.unwrap();
      insert_character(&db, 1, "Pilot").await;

      let state = state(&db, 1).await.unwrap().unwrap();

      assert!(state.total_sp.is_none());
    }

    #[tokio::test]
    async fn it_sums_skillpoints_after_skills_synced() {
      let db = store::open_test().await.unwrap();
      insert_character(&db, 1, "Pilot").await;
      insert_skill(&db, 1, 100, 5000).await;
      insert_skill(&db, 1, 101, 250).await;

      let state = state(&db, 1).await.unwrap().unwrap();

      assert_eq!(state.total_sp, Some(5250));
    }
  }

  mod wallet_balance {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_null_before_any_journal_row() {
      let db = store::open_test().await.unwrap();
      insert_character(&db, 1, "Pilot").await;

      let state = state(&db, 1).await.unwrap().unwrap();

      assert!(state.wallet_balance.is_none());
    }

    #[tokio::test]
    async fn it_equals_latest_entry_running_balance() {
      let db = store::open_test().await.unwrap();
      insert_character(&db, 1, "Pilot").await;
      insert_journal(&db, 1, 1, Some(100.0), Some(100.0)).await;
      insert_journal(&db, 2, 1, Some(50.0), Some(150.0)).await;

      let state = state(&db, 1).await.unwrap().unwrap();

      assert_eq!(state.wallet_balance, Some(150.0));
    }

    #[tokio::test]
    async fn it_carries_forward_over_trailing_null_balances() {
      let db = store::open_test().await.unwrap();
      insert_character(&db, 1, "Pilot").await;
      insert_journal(&db, 1, 1, Some(100.0), Some(1000.0)).await;
      insert_journal(&db, 2, 1, Some(250.0), None).await;
      insert_journal(&db, 3, 1, Some(-50.0), None).await;

      let state = state(&db, 1).await.unwrap().unwrap();

      assert_eq!(state.wallet_balance, Some(1200.0));
    }
  }
}

#[cfg(test)]
mod telemetry_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn make_telemetry(character_id: i64) -> CharacterTelemetry {
    CharacterTelemetry {
      character_id,
      online: true,
      ship_item_id: Some(1_000_000_016_991),
      ship_name: Some("My Rifter".to_owned()),
      ship_type_id: Some(587),
      solar_system_id: 30_000_142,
      station_id: Some(60_003_760),
      structure_id: None,
      synced_at: 1_700_000_000,
    }
  }

  mod telemetry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_character() {
      let db = store::open_test().await.unwrap();

      let result = super::telemetry(&db, 999).await.unwrap();

      assert_eq!(result, None);
    }
  }

  mod upsert_telemetry {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_stores_a_new_snapshot() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_telemetry(&db, &make_telemetry(42)).await.unwrap();

      let result = telemetry(&db, 42).await.unwrap().unwrap();
      assert_eq!(result.solar_system_id(), 30_000_142);
      assert_eq!(result.ship_name().as_deref(), Some("My Rifter"));
      assert!(result.online());
    }

    #[tokio::test]
    async fn it_replaces_the_snapshot_on_conflict() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      super::upsert_telemetry(&db, &make_telemetry(42)).await.unwrap();

      let mut updated = make_telemetry(42);
      updated.online = false;
      updated.solar_system_id = 30_002_187;
      updated.station_id = None;
      updated.structure_id = Some(1_021_000_000_000);
      updated.ship_name = None;
      updated.synced_at = 1_700_000_999;
      super::upsert_telemetry(&db, &updated).await.unwrap();

      let result = telemetry(&db, 42).await.unwrap().unwrap();
      assert!(!result.online());
      assert_eq!(result.solar_system_id(), 30_002_187);
      assert_eq!(result.station_id(), None);
      assert_eq!(result.structure_id(), Some(1_021_000_000_000));
      assert_eq!(result.ship_name().as_deref(), None);
      assert_eq!(result.synced_at(), 1_700_000_999);
    }
  }
}

#[cfg(test)]
mod clone_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_active_clone(db: &Database, character_id: i64) {
    sqlx::query(
      "INSERT INTO character_clones \
        (character_id, home_location_id, home_location_type, home_location_name, last_clone_jump_date, \
        last_station_change_date) \
      VALUES (?, ?, 'station', 'Jita IV - Moon 4', '2026-01-01T00:00:00Z', '2026-02-01T00:00:00Z')",
    )
    .bind(character_id)
    .bind(60_003_760_i64)
    .execute(&db.0)
    .await
    .unwrap();
  }

  async fn seed_jump_clone(db: &Database, character_id: i64, jump_clone_id: i64, location_id: i64) {
    sqlx::query(
      "INSERT INTO character_jump_clones (character_id, jump_clone_id, location_id, location_type, location_name, name) \
      VALUES (?, ?, ?, 'station', 'Amarr VIII', 'Battle Clone')",
    )
    .bind(character_id)
    .bind(jump_clone_id)
    .bind(location_id)
    .execute(&db.0)
    .await
    .unwrap();
  }

  async fn seed_implant(db: &Database, character_id: i64, clone_id: Option<i64>, type_id: i64, name: &str) {
    sqlx::query(
      "INSERT INTO character_clone_implants (character_id, clone_id, type_id, name, icon) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(character_id)
    .bind(clone_id)
    .bind(type_id)
    .bind(name)
    .bind(format!("type_{type_id}_64.png"))
    .execute(&db.0)
    .await
    .unwrap();
  }

  mod clones {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_no_active_clone_exists() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(super::clones(&db, 42).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_loads_the_active_clone_with_its_implants() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_active_clone(&db, 42).await;
      seed_implant(&db, 42, None, 22_119, "Ocular Filter").await;
      seed_implant(&db, 42, None, 10_212, "Memory Augmentation").await;

      let result = super::clones(&db, 42).await.unwrap().unwrap();

      assert_eq!(result.active.clone.home_location_id(), 60_003_760);
      assert_eq!(
        result.active.clone.home_location_name().as_deref(),
        Some("Jita IV - Moon 4")
      );
      assert_eq!(
        result.active.implants.iter().map(|i| i.type_id()).collect::<Vec<_>>(),
        [10_212, 22_119]
      );
      assert_eq!(result.active.implants[0].name(), "Memory Augmentation");
      assert_eq!(result.active.implants[0].icon().as_deref(), Some("type_10212_64.png"));
      assert!(result.jump_clones.is_empty());
    }

    #[tokio::test]
    async fn it_loads_jump_clones_each_with_their_own_implants() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_active_clone(&db, 42).await;
      seed_jump_clone(&db, 42, 1001, 60_008_494).await;
      seed_jump_clone(&db, 42, 1002, 60_011_866).await;
      seed_implant(&db, 42, Some(1001), 33_516, "Slave Alpha").await;
      seed_implant(&db, 42, Some(1001), 33_517, "Slave Beta").await;

      let result = super::clones(&db, 42).await.unwrap().unwrap();

      assert_eq!(
        result
          .jump_clones
          .iter()
          .map(|c| c.clone.jump_clone_id())
          .collect::<Vec<_>>(),
        [1001, 1002]
      );
      assert_eq!(
        result.jump_clones[0]
          .implants
          .iter()
          .map(|i| i.type_id())
          .collect::<Vec<_>>(),
        [33_516, 33_517]
      );
      assert_eq!(result.jump_clones[0].implants[0].clone_id(), Some(1001));
      assert!(result.jump_clones[1].implants.is_empty());
    }

    #[tokio::test]
    async fn it_returns_an_active_clone_with_no_implants() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_active_clone(&db, 42).await;

      let result = super::clones(&db, 42).await.unwrap().unwrap();

      assert_eq!(result.active.clone.character_id(), 42);
      assert!(result.active.implants.is_empty());
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{CharacterClone, CharacterCloneImplant, CharacterJumpClone};

    fn active(character_id: i64) -> CharacterClone {
      CharacterClone {
        character_id,
        home_location_id: 60_003_760,
        home_location_name: Some("Jita IV - Moon 4".to_owned()),
        home_location_type: "station".to_owned(),
        last_clone_jump_date: Some("2026-01-01T00:00:00Z".to_owned()),
        last_station_change_date: Some("2026-02-01T00:00:00Z".to_owned()),
      }
    }

    fn jump_clone(character_id: i64, jump_clone_id: i64) -> CharacterJumpClone {
      CharacterJumpClone {
        character_id,
        jump_clone_id,
        location_id: 60_008_494,
        location_name: Some("Amarr VIII".to_owned()),
        location_type: "station".to_owned(),
        name: Some("Battle Clone".to_owned()),
      }
    }

    fn implant(character_id: i64, clone_id: Option<i64>, type_id: i64) -> CharacterCloneImplant {
      CharacterCloneImplant {
        character_id,
        clone_id,
        icon: Some(format!("/data/types/{type_id}/icon_64.png")),
        name: format!("Implant {type_id}"),
        type_id,
      }
    }

    #[tokio::test]
    async fn it_replaces_the_whole_clone_picture_atomically() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_active_clone(&db, 42).await;
      seed_jump_clone(&db, 42, 9999, 60_011_866).await;
      seed_implant(&db, 42, None, 1, "Stale").await;

      super::replace_clones_for_character(
        &db,
        42,
        &active(42),
        &[jump_clone(42, 1001), jump_clone(42, 1002)],
        &[implant(42, None, 22_119), implant(42, Some(1001), 33_516)],
      )
      .await
      .unwrap();

      let result = super::clones(&db, 42).await.unwrap().unwrap();
      assert_eq!(
        result.active.clone.home_location_name().as_deref(),
        Some("Jita IV - Moon 4")
      );
      assert_eq!(
        result.active.implants.iter().map(|i| i.type_id()).collect::<Vec<_>>(),
        [22_119]
      );
      assert_eq!(
        result.active.implants[0].icon().as_deref(),
        Some("/data/types/22119/icon_64.png")
      );
      assert_eq!(
        result
          .jump_clones
          .iter()
          .map(|c| c.clone.jump_clone_id())
          .collect::<Vec<_>>(),
        [1001, 1002]
      );
      assert!(result.jump_clones.iter().all(|c| c.clone.jump_clone_id() != 9999));
      assert_eq!(
        result.jump_clones[0]
          .implants
          .iter()
          .map(|i| i.type_id())
          .collect::<Vec<_>>(),
        [33_516]
      );
    }
  }
}

#[cfg(test)]
mod contact_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  #[allow(clippy::too_many_arguments)]
  async fn seed_contact(
    db: &Database,
    character_id: i64,
    contact_id: i64,
    contact_type: &str,
    standing: f64,
    is_watched: bool,
    label_ids: &str,
    name: &str,
  ) {
    sqlx::query(
      "INSERT INTO character_contacts \
        (character_id, contact_id, contact_type, standing, is_watched, is_blocked, label_ids, contact_name) \
      VALUES (?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(character_id)
    .bind(contact_id)
    .bind(contact_type)
    .bind(standing)
    .bind(is_watched)
    .bind(label_ids)
    .bind(name)
    .execute(&db.0)
    .await
    .unwrap();
  }

  async fn seed_label(db: &Database, character_id: i64, label_id: i64, label_name: &str) {
    sqlx::query("INSERT INTO character_contact_labels (character_id, label_id, label_name) VALUES (?, ?, ?)")
      .bind(character_id)
      .bind(label_id)
      .bind(label_name)
      .execute(&db.0)
      .await
      .unwrap();
  }

  mod contacts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_collections_when_nothing_synced() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let result = super::contacts(&db, 42).await.unwrap();

      assert!(result.contacts.is_empty());
      assert!(result.labels.is_empty());
    }

    #[tokio::test]
    async fn it_returns_contacts_and_labels_with_resolved_names() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_label(&db, 42, 1, "Friendlies").await;
      seed_label(&db, 42, 2, "Watchlist").await;
      seed_contact(&db, 42, 95_001, "character", 5.0, true, "[1,2]", "Trusted Pilot").await;
      seed_contact(&db, 42, 98_001, "corporation", -10.0, false, "[]", "Hostile Corp").await;

      let result = super::contacts(&db, 42).await.unwrap();

      assert_eq!(
        result.contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>(),
        [95_001, 98_001]
      );
      let trusted = &result.contacts[0];
      assert_eq!(trusted.contact_name(), "Trusted Pilot");
      assert!(trusted.is_watched());
      assert_eq!(trusted.label_ids(), "[1,2]");
      assert_eq!(
        result
          .labels
          .iter()
          .map(|l| l.label_name().as_str())
          .collect::<Vec<_>>(),
        ["Friendlies", "Watchlist"]
      );
    }
  }

  mod contacts_page {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::repo::character::{ContactCursor, ContactSortColumn, ContactSortDir};

    async fn seed_three(db: &Database) {
      seed_character(db, 42).await;
      seed_contact(db, 42, 95_001, "character", 5.0, true, "[]", "Bravo Pilot").await;
      seed_contact(db, 42, 95_002, "character", -3.0, false, "[]", "Alpha Pilot").await;
      seed_contact(db, 42, 98_001, "corporation", 1.0, false, "[]", "Charlie Corp").await;
    }

    fn names(rows: &[CharacterContact]) -> Vec<&str> {
      rows.iter().map(|c| c.contact_name().as_str()).collect()
    }

    #[tokio::test]
    async fn it_orders_a_name_ascending_page() {
      let db = store::open_test().await.unwrap();
      seed_three(&db).await;

      let rows = super::contacts_page(
        &db,
        42,
        None,
        None,
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        None,
        10,
      )
      .await
      .unwrap();

      assert_eq!(names(&rows), ["Alpha Pilot", "Bravo Pilot", "Charlie Corp"]);
    }

    #[tokio::test]
    async fn it_orders_a_standing_descending_page() {
      let db = store::open_test().await.unwrap();
      seed_three(&db).await;

      let rows = super::contacts_page(
        &db,
        42,
        None,
        None,
        ContactSortColumn::Standing,
        ContactSortDir::Desc,
        None,
        10,
      )
      .await
      .unwrap();

      assert_eq!(names(&rows), ["Bravo Pilot", "Charlie Corp", "Alpha Pilot"]);
    }

    #[tokio::test]
    async fn it_filters_by_a_name_query() {
      let db = store::open_test().await.unwrap();
      seed_three(&db).await;

      let rows = super::contacts_page(
        &db,
        42,
        None,
        Some("pilot"),
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        None,
        10,
      )
      .await
      .unwrap();

      assert_eq!(names(&rows), ["Alpha Pilot", "Bravo Pilot"]);
    }

    #[tokio::test]
    async fn it_filters_by_contact_type() {
      let db = store::open_test().await.unwrap();
      seed_three(&db).await;

      let rows = super::contacts_page(
        &db,
        42,
        Some("corporation"),
        None,
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        None,
        10,
      )
      .await
      .unwrap();

      assert_eq!(names(&rows), ["Charlie Corp"]);
    }

    #[tokio::test]
    async fn it_ignores_a_blank_name_query() {
      let db = store::open_test().await.unwrap();
      seed_three(&db).await;

      let rows = super::contacts_page(
        &db,
        42,
        None,
        Some("   "),
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        None,
        10,
      )
      .await
      .unwrap();

      assert_eq!(names(&rows), ["Alpha Pilot", "Bravo Pilot", "Charlie Corp"]);
    }

    #[tokio::test]
    async fn it_walks_a_text_keyset_cursor_without_repeating_or_skipping() {
      let db = store::open_test().await.unwrap();
      seed_three(&db).await;

      let first = super::contacts_page(
        &db,
        42,
        None,
        None,
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        None,
        2,
      )
      .await
      .unwrap();
      assert_eq!(names(&first), ["Alpha Pilot", "Bravo Pilot"]);

      let last = first.last().unwrap();
      let cursor = ContactCursor::Text(last.contact_name().clone(), last.contact_id());
      let second = super::contacts_page(
        &db,
        42,
        None,
        None,
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        Some(&cursor),
        2,
      )
      .await
      .unwrap();

      assert_eq!(names(&second), ["Charlie Corp"]);
    }

    #[tokio::test]
    async fn it_walks_a_numeric_keyset_cursor() {
      let db = store::open_test().await.unwrap();
      seed_three(&db).await;

      let first = super::contacts_page(
        &db,
        42,
        None,
        None,
        ContactSortColumn::Standing,
        ContactSortDir::Desc,
        None,
        1,
      )
      .await
      .unwrap();
      assert_eq!(names(&first), ["Bravo Pilot"]);

      let last = first.last().unwrap();
      let cursor = ContactCursor::Number(last.standing(), last.contact_id());
      let second = super::contacts_page(
        &db,
        42,
        None,
        None,
        ContactSortColumn::Standing,
        ContactSortDir::Desc,
        Some(&cursor),
        10,
      )
      .await
      .unwrap();

      assert_eq!(names(&second), ["Charlie Corp", "Alpha Pilot"]);
    }
  }

  mod contact_labels {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_the_labels_for_the_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_label(&db, 42, 2, "Watchlist").await;
      seed_label(&db, 42, 1, "Friendlies").await;

      let labels = super::super::contact_labels(&db, 42).await.unwrap();

      assert_eq!(
        labels.iter().map(|l| l.label_name().as_str()).collect::<Vec<_>>(),
        ["Friendlies", "Watchlist"]
      );
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::{CharacterContact, CharacterContactLabel};

    fn contact(
      character_id: i64,
      contact_id: i64,
      contact_type: &str,
      name: &str,
      label_ids: &str,
    ) -> CharacterContact {
      CharacterContact {
        character_id,
        contact_id,
        contact_name: name.to_owned(),
        contact_type: contact_type.to_owned(),
        is_blocked: false,
        is_watched: true,
        label_ids: label_ids.to_owned(),
        standing: 5.0,
      }
    }

    #[tokio::test]
    async fn it_replaces_contacts_atomically() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contact(&db, 42, 91_000, "character", 1.0, false, "[]", "Stale Pilot").await;

      super::replace_contacts_for_character(
        &db,
        42,
        &[
          contact(42, 95_001, "character", "Trusted Pilot", "[1,2]"),
          contact(42, 98_001, "corporation", "Allied Corp", "[]"),
        ],
        &HashSet::new(),
      )
      .await
      .unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      assert_eq!(
        result.contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>(),
        [95_001, 98_001]
      );
      assert_eq!(result.contacts[0].contact_name(), "Trusted Pilot");
      assert_eq!(result.contacts[0].label_ids(), "[1,2]");
    }

    #[tokio::test]
    async fn it_preserves_a_protected_contact_across_a_replace() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contact(&db, 42, 95_010, "character", 9.0, true, "[]", "Just Added").await;

      let protected = HashSet::from([95_010]);
      super::replace_contacts_for_character(
        &db,
        42,
        &[contact(42, 98_001, "corporation", "Server Corp", "[]")],
        &protected,
      )
      .await
      .unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      let ids = result.contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>();
      assert!(
        ids.contains(&95_010),
        "the protected optimistic contact survives the full replace"
      );
      assert!(ids.contains(&98_001), "non-protected server rows are still inserted");

      let preserved = result.contacts.iter().find(|c| c.contact_id() == 95_010).unwrap();
      assert_eq!(
        preserved.contact_name(),
        "Just Added",
        "the optimistic row is left untouched, not overwritten by server data"
      );
    }

    #[tokio::test]
    async fn it_does_not_resurrect_a_protected_contact_absent_from_the_server_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      let protected = HashSet::from([95_020]);
      super::replace_contacts_for_character(
        &db,
        42,
        &[contact(42, 95_020, "character", "Stale Server Name", "[]")],
        &protected,
      )
      .await
      .unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      assert!(
        result.contacts.is_empty(),
        "a contact with a pending remove (locally deleted) is not reinserted from the server set"
      );
    }

    #[tokio::test]
    async fn it_replaces_labels_atomically() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_label(&db, 42, 9, "Stale").await;

      super::replace_labels_for_character(
        &db,
        42,
        &[
          CharacterContactLabel {
            character_id: 42,
            label_id: 1,
            label_name: "Friendlies".to_owned(),
          },
          CharacterContactLabel {
            character_id: 42,
            label_id: 2,
            label_name: "Watchlist".to_owned(),
          },
        ],
      )
      .await
      .unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      assert_eq!(result.labels.iter().map(|l| l.label_id()).collect::<Vec<_>>(), [1, 2]);
      assert!(result.labels.iter().all(|l| l.label_id() != 9));
    }

    #[tokio::test]
    async fn it_clears_existing_rows_when_given_empty_sets() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contact(&db, 42, 91_000, "character", 1.0, false, "[]", "Pilot").await;
      seed_label(&db, 42, 1, "Friendlies").await;

      super::replace_contacts_for_character(&db, 42, &[], &HashSet::new())
        .await
        .unwrap();
      super::replace_labels_for_character(&db, 42, &[]).await.unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      assert!(result.contacts.is_empty());
      assert!(result.labels.is_empty());
    }
  }

  mod single_row {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::CharacterContact;

    fn contact(
      character_id: i64,
      contact_id: i64,
      contact_type: &str,
      name: &str,
      standing: f64,
      is_watched: bool,
      label_ids: &str,
    ) -> CharacterContact {
      CharacterContact {
        character_id,
        contact_id,
        contact_name: name.to_owned(),
        contact_type: contact_type.to_owned(),
        is_blocked: false,
        is_watched,
        label_ids: label_ids.to_owned(),
        standing,
      }
    }

    #[tokio::test]
    async fn it_inserts_a_new_contact() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      super::upsert_contact(&db, &contact(42, 95_001, "character", "New Pilot", 5.0, true, "[1]"))
        .await
        .unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      assert_eq!(
        result.contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>(),
        [95_001]
      );
      assert_eq!(result.contacts[0].contact_name(), "New Pilot");
      assert_eq!(result.contacts[0].standing(), 5.0);
      assert!(result.contacts[0].is_watched());
      assert_eq!(result.contacts[0].label_ids(), "[1]");
    }

    #[tokio::test]
    async fn it_updates_an_existing_contact_in_place() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contact(&db, 42, 95_001, "character", 5.0, true, "[1]", "Old Name").await;

      super::upsert_contact(
        &db,
        &contact(42, 95_001, "character", "New Name", -10.0, false, "[2,3]"),
      )
      .await
      .unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      assert_eq!(result.contacts.len(), 1);
      assert_eq!(result.contacts[0].contact_name(), "New Name");
      assert_eq!(result.contacts[0].standing(), -10.0);
      assert!(!result.contacts[0].is_watched());
      assert_eq!(result.contacts[0].label_ids(), "[2,3]");
    }

    #[tokio::test]
    async fn it_deletes_only_the_targeted_contact() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_contact(&db, 42, 95_001, "character", 5.0, true, "[]", "Keep Pilot").await;
      seed_contact(&db, 42, 98_001, "corporation", 1.0, false, "[]", "Drop Corp").await;

      super::delete_contact(&db, 42, 98_001).await.unwrap();

      let result = super::contacts(&db, 42).await.unwrap();
      assert_eq!(
        result.contacts.iter().map(|c| c.contact_id()).collect::<Vec<_>>(),
        [95_001]
      );
    }
  }
}

#[cfg(test)]
mod killmail_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn entry(character_id: i64, killmail_id: i64, value_isk: f64) -> CharacterKillEntry {
    CharacterKillEntry {
      attacker_count: 3,
      character_id,
      final_blow: true,
      is_kill: true,
      kill_hash: "abc123".to_owned(),
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id,
      ship_type_id: 587,
      synced_at: "2024-01-02T00:00:00Z".to_owned(),
      system_id: 30_000_142,
      value_destroyed_isk: 0.0,
      value_final: false,
      value_isk,
      value_recheck_count: 0,
      value_source: "zkill".to_owned(),
      victim_alliance_id: Some(4004),
      victim_corp_id: Some(3003),
      victim_damage_taken: 0,
      victim_id: Some(2002),
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_a_new_killmail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      upsert_killmail(&db, &entry(42, 100, 1234.5)).await.unwrap();

      let rows = killmails(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].killmail_id(), 100);
      assert_eq!(rows[0].value_isk(), 1234.5);
    }

    #[tokio::test]
    async fn it_is_idempotent_on_the_composite_key() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      upsert_killmail(&db, &entry(42, 100, 1234.5)).await.unwrap();
      upsert_killmail(&db, &entry(42, 100, 9999.0)).await.unwrap();

      let rows = killmails(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].value_isk(), 9999.0);
    }

    #[tokio::test]
    async fn it_round_trips_the_value_provenance_fields() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut kill = entry(42, 100, 1234.5);
      kill.value_destroyed_isk = 1000.0;
      kill.value_final = true;
      kill.value_recheck_count = 3;
      kill.value_source = "local".to_owned();
      upsert_killmail(&db, &kill).await.unwrap();

      let rows = killmails(&db, 42).await.unwrap();
      assert_eq!(rows[0].value_destroyed_isk(), 1000.0);
      assert_eq!(rows[0].value_final(), true);
      assert_eq!(rows[0].value_recheck_count(), 3);
      assert_eq!(rows[0].value_source(), "local");
    }
  }

  mod killmails {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_killmails_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut older = entry(42, 100, 1.0);
      older.kill_time = "2024-01-01T00:00:00Z".to_owned();
      let mut newer = entry(42, 200, 2.0);
      newer.kill_time = "2024-03-01T00:00:00Z".to_owned();
      upsert_killmail(&db, &older).await.unwrap();
      upsert_killmail(&db, &newer).await.unwrap();

      let rows = killmails(&db, 42).await.unwrap();

      assert_eq!(rows.iter().map(|k| k.killmail_id()).collect::<Vec<_>>(), [200, 100]);
    }
  }

  mod killmails_needing_recheck {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_non_final_local_killmails() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut on_zkill = entry(42, 100, 1.0);
      on_zkill.value_source = "zkill".to_owned();
      let mut pending = entry(42, 200, 2.0);
      pending.value_source = "local".to_owned();
      pending.value_final = false;
      let mut finalized = entry(42, 300, 3.0);
      finalized.value_source = "local".to_owned();
      finalized.value_final = true;
      upsert_killmail(&db, &on_zkill).await.unwrap();
      upsert_killmail(&db, &pending).await.unwrap();
      upsert_killmail(&db, &finalized).await.unwrap();

      let rows = killmails_needing_recheck(&db).await.unwrap();

      assert_eq!(rows.iter().map(|k| k.killmail_id()).collect::<Vec<_>>(), [200]);
    }
  }

  mod detail {
    use pretty_assertions::assert_eq;

    use super::*;

    fn attacker(character_id: i64, killmail_id: i64, ordinal: i64) -> KillmailAttacker {
      KillmailAttacker {
        alliance_id: Some(99_000_001),
        attacker_character_id: Some(5005),
        character_id,
        corporation_id: Some(6006),
        damage_done: 1234,
        final_blow: ordinal == 0,
        killmail_id,
        ordinal,
        ship_type_id: Some(670),
      }
    }

    fn item(character_id: i64, killmail_id: i64, ordinal: i64) -> KillmailItem {
      KillmailItem {
        character_id,
        flag: 27,
        killmail_id,
        ordinal,
        quantity_destroyed: 1,
        quantity_dropped: 0,
        type_id: 2185,
        value_isk: 4242.5,
      }
    }

    #[tokio::test]
    async fn it_round_trips_attackers_items_and_victim_columns() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut kill = entry(42, 100, 1234.5);
      kill.victim_alliance_id = Some(7007);
      kill.victim_damage_taken = 56_789;
      upsert_killmail(&db, &kill).await.unwrap();
      let attackers = vec![attacker(42, 100, 0), attacker(42, 100, 1)];
      let items = vec![item(42, 100, 0), item(42, 100, 1)];

      upsert_killmail_detail(&db, 42, 100, &attackers, &items).await.unwrap();

      let read_kill = killmails(&db, 42).await.unwrap();
      assert_eq!(read_kill[0].victim_alliance_id(), Some(7007));
      assert_eq!(read_kill[0].victim_damage_taken(), 56_789);
      assert_eq!(killmail_attackers(&db, 42, 100).await.unwrap(), attackers);
      assert_eq!(killmail_items(&db, 42, 100).await.unwrap(), items);
    }

    #[tokio::test]
    async fn it_replaces_existing_detail_on_rewrite() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert_killmail(&db, &entry(42, 100, 1234.5)).await.unwrap();
      upsert_killmail_detail(&db, 42, 100, &[attacker(42, 100, 0)], &[item(42, 100, 0)])
        .await
        .unwrap();

      upsert_killmail_detail(&db, 42, 100, &[attacker(42, 100, 0), attacker(42, 100, 1)], &[])
        .await
        .unwrap();

      assert_eq!(killmail_attackers(&db, 42, 100).await.unwrap().len(), 2);
      assert_eq!(killmail_items(&db, 42, 100).await.unwrap(), Vec::<KillmailItem>::new());
    }

    #[tokio::test]
    async fn it_cascades_both_child_tables_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      upsert_killmail(&db, &entry(42, 100, 1234.5)).await.unwrap();
      upsert_killmail_detail(&db, 42, 100, &[attacker(42, 100, 0)], &[item(42, 100, 0)])
        .await
        .unwrap();

      character::delete(&db, 42).await.unwrap();

      assert_eq!(
        killmail_attackers(&db, 42, 100).await.unwrap(),
        Vec::<KillmailAttacker>::new()
      );
      assert_eq!(killmail_items(&db, 42, 100).await.unwrap(), Vec::<KillmailItem>::new());
    }
  }
}

#[cfg(test)]
mod notification_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn notification(character_id: i64, notification_id: i64, is_read: bool) -> CharacterNotification {
    CharacterNotification {
      character_id,
      is_read,
      notif_type: "KillReportFinalBlow".to_owned(),
      notification_id,
      sender_id: Some(1001),
      sender_type: Some("character".to_owned()),
      synced_at: "2024-01-02T00:00:00Z".to_owned(),
      text: Some("body".to_owned()),
      timestamp: "2024-01-01T00:00:00Z".to_owned(),
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_a_new_notification() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      upsert_notification(&db, &notification(42, 7, false)).await.unwrap();

      let rows = notifications(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].notification_id(), 7);
      assert!(!rows[0].is_read());
    }

    #[tokio::test]
    async fn it_is_idempotent_on_the_composite_key() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      upsert_notification(&db, &notification(42, 7, false)).await.unwrap();
      upsert_notification(&db, &notification(42, 7, true)).await.unwrap();

      let rows = notifications(&db, 42).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert!(rows[0].is_read());
    }
  }

  mod notifications {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_notifications_newest_first() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let mut older = notification(42, 1, false);
      older.timestamp = "2024-01-01T00:00:00Z".to_owned();
      let mut newer = notification(42, 2, false);
      newer.timestamp = "2024-03-01T00:00:00Z".to_owned();
      upsert_notification(&db, &older).await.unwrap();
      upsert_notification(&db, &newer).await.unwrap();

      let rows = notifications(&db, 42).await.unwrap();

      assert_eq!(rows.iter().map(|n| n.notification_id()).collect::<Vec<_>>(), [2, 1]);
    }
  }
}

#[cfg(test)]
mod squad_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_squads_ordered_by_position() {
      let db = store::open_test().await.unwrap();
      create(&db, "First", None, None).await.unwrap();
      create(&db, "Second", None, None).await.unwrap();

      let squads = all_squads(&db).await.unwrap();

      assert_eq!(squads.iter().map(|s| s.name()).collect::<Vec<_>>(), ["First", "Second"]);
    }
  }

  mod assign {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn positions(db: &Database, squad_id: i64) -> Vec<(i64, i64)> {
      let mut rows: Vec<(i64, i64)> = memberships(db)
        .await
        .unwrap()
        .into_iter()
        .filter(|m| m.squad_id() == squad_id)
        .map(|m| (m.character_id(), m.position()))
        .collect();
      rows.sort();
      rows
    }

    #[tokio::test]
    async fn it_cascades_occupants_so_no_two_members_share_a_position() {
      let db = store::open_test().await.unwrap();
      for id in [1, 2, 3] {
        seed_character(&db, id).await;
      }
      let squad = create(&db, "Crew", None, None).await.unwrap();
      assign(&db, 1, squad.id(), 0).await.unwrap();
      assign(&db, 2, squad.id(), 1).await.unwrap();
      assign(&db, 3, squad.id(), 2).await.unwrap();

      assign(&db, 3, squad.id(), 0).await.unwrap();

      assert_eq!(positions(&db, squad.id()).await, vec![(1, 1), (2, 2), (3, 0)]);
    }

    #[tokio::test]
    async fn it_keeps_characters_past_a_gap_in_place_on_a_cross_squad_drop() {
      let db = store::open_test().await.unwrap();
      for id in [1, 2, 3, 4, 5] {
        seed_character(&db, id).await;
      }
      let crew = create(&db, "Crew", None, None).await.unwrap();
      let other = create(&db, "Other", None, None).await.unwrap();
      assign(&db, 1, crew.id(), 0).await.unwrap();
      assign(&db, 2, crew.id(), 1).await.unwrap();
      assign(&db, 3, crew.id(), 2).await.unwrap();
      assign(&db, 4, crew.id(), 5).await.unwrap();
      assign(&db, 5, other.id(), 0).await.unwrap();

      assign(&db, 5, crew.id(), 0).await.unwrap();

      assert_eq!(
        positions(&db, crew.id()).await,
        vec![(1, 1), (2, 2), (3, 3), (4, 5), (5, 0)]
      );
      assert_eq!(members(&db, other.id()).await.unwrap(), Vec::<i64>::new());
    }

    #[tokio::test]
    async fn it_moves_a_character_out_of_its_previous_squad() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let from = create(&db, "From", None, None).await.unwrap();
      let to = create(&db, "To", None, None).await.unwrap();
      assign(&db, 12_345_678, from.id(), 0).await.unwrap();

      assign(&db, 12_345_678, to.id(), 0).await.unwrap();

      assert_eq!(members(&db, from.id()).await.unwrap(), Vec::<i64>::new());
      assert_eq!(members(&db, to.id()).await.unwrap(), vec![12_345_678]);
      assert_eq!(memberships(&db).await.unwrap().len(), 1);
    }
  }

  mod cascade_positions {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn by_character(result: Vec<(i64, i64)>) -> HashMap<i64, i64> {
      result.into_iter().collect()
    }

    #[test]
    fn it_bumps_a_single_occupant_off_the_target() {
      let occupants = [(2, 1)];

      let result = by_character(cascade_positions(&occupants, 1, 1));

      assert_eq!(result[&1], 1);
      assert_eq!(result[&2], 2);
    }

    #[test]
    fn it_cascades_a_chain_of_occupied_slots() {
      let occupants = [(1, 0), (2, 1), (3, 2)];

      let result = by_character(cascade_positions(&occupants, 4, 1));

      assert_eq!(result[&4], 1);
      assert_eq!(result[&2], 2);
      assert_eq!(result[&3], 3);
      assert_eq!(result[&1], 0);
    }

    #[test]
    fn it_handles_a_single_character_with_no_occupants() {
      let result = by_character(cascade_positions(&[], 1, 5));

      assert_eq!(result[&1], 5);
    }

    #[test]
    fn it_is_a_no_op_when_dropped_on_its_own_now_empty_slot() {
      let occupants = [(2, 1)];

      let result = by_character(cascade_positions(&occupants, 1, 0));

      assert_eq!(result[&1], 0);
      assert_eq!(result[&2], 1);
    }

    #[test]
    fn it_places_the_dragged_character_at_an_empty_target() {
      let occupants = [(2, 3)];

      let result = by_character(cascade_positions(&occupants, 1, 1));

      assert_eq!(result[&1], 1);
      assert_eq!(result[&2], 3);
    }

    #[test]
    fn it_stops_the_cascade_at_the_first_gap() {
      let occupants = [(1, 0), (2, 1), (3, 2), (9, 7)];

      let result = by_character(cascade_positions(&occupants, 8, 0));

      assert_eq!(result[&8], 0);
      assert_eq!(result[&1], 1);
      assert_eq!(result[&2], 2);
      assert_eq!(result[&3], 3);
      assert_eq!(result[&9], 7);
    }
  }

  mod create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_with_increasing_positions() {
      let db = store::open_test().await.unwrap();

      let first = create(&db, "First", Some("the first"), Some("#3FB8DB")).await.unwrap();
      let second = create(&db, "Second", None, None).await.unwrap();

      assert_eq!(first.position(), 0);
      assert_eq!(second.position(), 1);
      assert_eq!(first.description().as_deref(), Some("the first"));
      assert_eq!(first.color().as_deref(), Some("#3FB8DB"));
    }
  }

  mod delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cascades_membership() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let squad = create(&db, "Doomed", None, None).await.unwrap();
      assign(&db, 12_345_678, squad.id(), 0).await.unwrap();

      delete_squad(&db, squad.id()).await.unwrap();

      assert!(get_squad(&db, squad.id()).await.unwrap().is_none());
      assert_eq!(memberships(&db).await.unwrap().len(), 0);
    }
  }

  mod members {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_lists_members_in_position_order() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 100).await;
      seed_character(&db, 200).await;
      let squad = create(&db, "Crew", None, None).await.unwrap();
      assign(&db, 200, squad.id(), 1).await.unwrap();
      assign(&db, 100, squad.id(), 0).await.unwrap();

      assert_eq!(members(&db, squad.id()).await.unwrap(), vec![100, 200]);
    }
  }

  mod normalize {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn insert_membership(db: &Database, character_id: i64, squad_id: i64, position: i64) {
      sqlx::query("INSERT INTO character_squads (character_id, position, squad_id) VALUES (?, ?, ?)")
        .bind(character_id)
        .bind(position)
        .bind(squad_id)
        .execute(&db.0)
        .await
        .unwrap();
    }

    async fn positions(db: &Database, squad_id: i64) -> Vec<(i64, i64)> {
      let mut rows: Vec<(i64, i64)> = memberships(db)
        .await
        .unwrap()
        .into_iter()
        .filter(|m| m.squad_id() == squad_id)
        .map(|m| (m.character_id(), m.position()))
        .collect();
      rows.sort();
      rows
    }

    #[tokio::test]
    async fn it_leaves_a_bucket_with_gaps_untouched() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 1).await;
      seed_character(&db, 2).await;
      let squad = create(&db, "Crew", None, None).await.unwrap();
      insert_membership(&db, 1, squad.id(), 0).await;
      insert_membership(&db, 2, squad.id(), 5).await;

      normalize(&db, squad.id()).await.unwrap();

      assert_eq!(positions(&db, squad.id()).await, vec![(1, 0), (2, 5)]);
    }

    #[tokio::test]
    async fn it_renumbers_a_bucket_that_has_duplicate_positions() {
      let db = store::open_test().await.unwrap();
      for id in [1, 2, 3] {
        seed_character(&db, id).await;
      }
      let squad = create(&db, "Crew", None, None).await.unwrap();
      insert_membership(&db, 1, squad.id(), 0).await;
      insert_membership(&db, 2, squad.id(), 0).await;
      insert_membership(&db, 3, squad.id(), 1).await;

      normalize(&db, squad.id()).await.unwrap();

      assert_eq!(positions(&db, squad.id()).await, vec![(1, 0), (2, 1), (3, 2)]);
    }
  }

  mod reorder {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_rewrites_positions_to_the_given_order() {
      let db = store::open_test().await.unwrap();
      let a = create(&db, "A", None, None).await.unwrap();
      let b = create(&db, "B", None, None).await.unwrap();
      let c = create(&db, "C", None, None).await.unwrap();

      reorder(&db, &[c.id(), a.id(), b.id()]).await.unwrap();

      assert_eq!(
        all_squads(&db)
          .await
          .unwrap()
          .iter()
          .map(|s| s.name())
          .collect::<Vec<_>>(),
        ["C", "A", "B"]
      );
    }
  }

  mod reserved {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn get_or_create_unassigned_is_idempotent() {
      let db = store::open_test().await.unwrap();

      let first = get_or_create_unassigned(&db).await.unwrap();
      let second = get_or_create_unassigned(&db).await.unwrap();

      assert_eq!(first.id(), second.id());
      assert!(is_unassigned(&first));
    }

    #[tokio::test]
    async fn all_user_squads_excludes_the_reserved_bucket() {
      let db = store::open_test().await.unwrap();
      create(&db, "Crew", None, None).await.unwrap();
      get_or_create_unassigned(&db).await.unwrap();

      let user = all_user_squads(&db).await.unwrap();

      assert_eq!(user.iter().map(|s| s.name()).collect::<Vec<_>>(), ["Crew"]);
      assert!(all_squads(&db).await.unwrap().iter().any(is_unassigned));
    }

    #[tokio::test]
    async fn create_rejects_the_reserved_name() {
      let db = store::open_test().await.unwrap();

      let result = create(&db, RESERVED_UNASSIGNED_NAME, None, None).await;

      assert!(matches!(result, Err(Error::ReservedSquad)));
    }

    #[tokio::test]
    async fn delete_and_update_refuse_the_reserved_bucket() {
      let db = store::open_test().await.unwrap();
      let reserved = get_or_create_unassigned(&db).await.unwrap();

      assert!(matches!(
        delete_squad(&db, reserved.id()).await,
        Err(Error::ReservedSquad)
      ));
      assert!(matches!(
        update(&db, reserved.id(), "Renamed", None, None).await,
        Err(Error::ReservedSquad)
      ));
      assert!(get_squad(&db, reserved.id()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn update_refuses_renaming_a_user_squad_onto_the_reserved_name() {
      let db = store::open_test().await.unwrap();
      let squad = create(&db, "Crew", None, None).await.unwrap();

      let result = update(&db, squad.id(), RESERVED_UNASSIGNED_NAME, None, None).await;

      assert!(matches!(result, Err(Error::ReservedSquad)));
    }

    #[tokio::test]
    async fn unassigned_id_returns_none_until_the_bucket_exists() {
      let db = store::open_test().await.unwrap();

      assert_eq!(unassigned_id(&db).await.unwrap(), None);

      let reserved = get_or_create_unassigned(&db).await.unwrap();
      assert_eq!(unassigned_id(&db).await.unwrap(), Some(reserved.id()));
    }
  }

  mod unassign {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_moves_a_character_into_the_reserved_unassigned_squad() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let squad = create(&db, "Crew", None, None).await.unwrap();
      assign(&db, 12_345_678, squad.id(), 0).await.unwrap();

      unassign(&db, 12_345_678).await.unwrap();

      assert_eq!(members(&db, squad.id()).await.unwrap(), Vec::<i64>::new());
      let reserved = get_or_create_unassigned(&db).await.unwrap();
      assert_eq!(members(&db, reserved.id()).await.unwrap(), vec![12_345_678]);
      let row = memberships(&db)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.character_id() == 12_345_678)
        .unwrap();
      assert_eq!(row.squad_id(), reserved.id());
      assert_eq!(row.position(), 0);
    }

    #[tokio::test]
    async fn it_appends_each_unassigned_character_with_an_increasing_position() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 100).await;
      seed_character(&db, 200).await;

      unassign(&db, 100).await.unwrap();
      unassign(&db, 200).await.unwrap();

      let reserved = get_or_create_unassigned(&db).await.unwrap();
      assert_eq!(members(&db, reserved.id()).await.unwrap(), vec![100, 200]);
    }

    #[tokio::test]
    async fn assigning_to_a_user_squad_moves_a_character_out_of_the_reserved_one() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 12_345_678).await;
      let squad = create(&db, "Crew", None, None).await.unwrap();
      unassign(&db, 12_345_678).await.unwrap();
      let reserved = get_or_create_unassigned(&db).await.unwrap();

      assign(&db, 12_345_678, squad.id(), 0).await.unwrap();

      assert_eq!(members(&db, reserved.id()).await.unwrap(), Vec::<i64>::new());
      assert_eq!(members(&db, squad.id()).await.unwrap(), vec![12_345_678]);
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_name_description_and_color() {
      let db = store::open_test().await.unwrap();
      let squad = create(&db, "Old", None, None).await.unwrap();

      update(&db, squad.id(), "New", Some("renamed"), Some("#E07559"))
        .await
        .unwrap();

      let updated = get_squad(&db, squad.id()).await.unwrap().unwrap();
      assert_eq!(updated.name(), "New");
      assert_eq!(updated.description().as_deref(), Some("renamed"));
      assert_eq!(updated.color().as_deref(), Some("#E07559"));
    }
  }
}

#[cfg(test)]
mod standing_tests {
  use super::*;
  use crate::store::{
    self, Database,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
    repo::character,
  };

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_standing(db: &Database, character_id: i64, from_id: i64, from_type: &str, standing: f64, name: &str) {
    sqlx::query(
      "INSERT INTO character_standings (character_id, from_id, from_type, standing, from_name) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(character_id)
    .bind(from_id)
    .bind(from_type)
    .bind(standing)
    .bind(name)
    .execute(&db.0)
    .await
    .unwrap();
  }

  mod standings {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_an_empty_vec_when_no_standings_exist() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;

      assert_eq!(super::standings(&db, 42).await.unwrap(), Vec::new());
    }

    #[tokio::test]
    async fn it_returns_standings_grouped_by_type_then_id_with_resolved_names() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_standing(&db, 42, 1_000_125, "npc_corp", -2.5, "Concord").await;
      seed_standing(&db, 42, 500_003, "faction", 7.5, "Amarr Empire").await;
      seed_standing(&db, 42, 3_018_900, "agent", 1.25, "Some Agent").await;

      let result = super::standings(&db, 42).await.unwrap();

      assert_eq!(
        result.iter().map(|s| s.from_type().as_str()).collect::<Vec<_>>(),
        ["agent", "faction", "npc_corp"]
      );
      let faction = result.iter().find(|s| s.from_type() == "faction").unwrap();
      assert_eq!(faction.from_name(), "Amarr Empire");
      assert!((faction.standing() - 7.5).abs() < f64::EPSILON);
    }
  }

  mod replace_for_character {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::CharacterStanding;

    fn standing(character_id: i64, from_id: i64, from_type: &str, standing: f64, name: &str) -> CharacterStanding {
      CharacterStanding {
        character_id,
        from_id,
        from_name: name.to_owned(),
        from_type: from_type.to_owned(),
        standing,
      }
    }

    #[tokio::test]
    async fn it_replaces_the_whole_set_atomically() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_standing(&db, 42, 500_003, "faction", 1.0, "Stale Faction").await;

      super::replace_standings_for_character(
        &db,
        42,
        &[
          standing(42, 500_001, "faction", 7.5, "Caldari State"),
          standing(42, 1_000_125, "npc_corp", -2.5, "Concord"),
        ],
      )
      .await
      .unwrap();

      let result = super::standings(&db, 42).await.unwrap();
      assert_eq!(
        result.iter().map(|s| s.from_id()).collect::<Vec<_>>(),
        [500_001, 1_000_125]
      );
      assert!(result.iter().all(|s| s.from_id() != 500_003));
    }

    #[tokio::test]
    async fn it_clears_existing_rows_when_given_an_empty_set() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_standing(&db, 42, 500_003, "faction", 1.0, "Amarr Empire").await;

      super::replace_standings_for_character(&db, 42, &[]).await.unwrap();

      assert!(super::standings(&db, 42).await.unwrap().is_empty());
    }
  }
}
