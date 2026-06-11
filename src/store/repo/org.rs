use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{
    Alliance, Character, Corporation, CorporationMemberRole, Faction, OwnedCorporation, Station,
    corporation_card::{CardRow, CardRowSql, CardTag, TagRowSql},
  },
  repo::infra::like_pattern,
  search::{FilterToken, ParsedQuery},
};

const CHARACTER_ONLY_KEYS: &[&str] = &["clone", "corp", "loc", "name", "status", "training"];

pub const REQUIRED_ASSET_ROLE: &str = "Director";

const SEARCH_SELECT: &str = "\
  SELECT \
    oc.id AS corporation_id, \
    oc.name AS name, \
    oc.ticker AS ticker, \
    oc.member_count AS member_count, \
    oc.tax_rate AS tax_rate, \
    al.name AS alliance_name, \
    al.ticker AS alliance_ticker, \
    ceo.name AS ceo_name, \
    hq.name AS hq_name \
  FROM owned_corporations oc \
  LEFT JOIN alliances al ON al.id = oc.alliance_id \
  LEFT JOIN characters ceo ON ceo.id = oc.ceo_id \
  LEFT JOIN stations hq ON hq.id = oc.home_station_id";

pub async fn all_corporations(db: &Database) -> Result<Vec<Corporation>, Error> {
  let rows = sqlx::query_as::<_, Corporation>(
    "SELECT alliance_id, ceo_id, creator_id, date_founded AS creation_date, description, \
    faction_id, home_station_id, id, member_count, name, shares, tax_rate, ticker, url, \
    war_eligible FROM corporations",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn search_corporations(db: &Database, query: &ParsedQuery) -> Result<Vec<CardRow>, Error> {
  let mut builder = QueryBuilder::<Sqlite>::new(SEARCH_SELECT);

  for (index, token) in query.tokens.iter().enumerate() {
    builder.push(if index == 0 { " WHERE " } else { " AND " });
    push_token_predicate(&mut builder, token);
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
    "SELECT et.entity_id AS corporation_id, tg.color AS color, tg.id AS id, tg.name AS name \
    FROM entity_tags et \
    JOIN tags tg ON tg.id = et.tag_id \
    WHERE et.entity_type = 'corporation' AND et.entity_id IN (",
  );
  let mut separated = builder.separated(", ");
  for row in rows.iter() {
    separated.push_bind(row.corporation_id);
  }
  separated.push_unseparated(") ORDER BY tg.position, tg.id");

  let tag_rows = builder.build_query_as::<TagRowSql>().fetch_all(&db.0).await?;

  let mut by_corp: HashMap<i64, Vec<CardTag>> = HashMap::new();
  for tag in tag_rows {
    by_corp.entry(tag.corporation_id).or_default().push(CardTag {
      color_hex: tag.color,
      id: tag.id,
      name: tag.name,
    });
  }
  for row in rows.iter_mut() {
    if let Some(tags) = by_corp.remove(&row.corporation_id) {
      row.tags = tags;
    }
  }
  Ok(())
}

pub async fn get_corporation(db: &Database, id: i64) -> Result<Option<Corporation>, Error> {
  let row = sqlx::query_as::<_, Corporation>(
    "SELECT alliance_id, ceo_id, creator_id, date_founded AS creation_date, description, \
    faction_id, home_station_id, id, member_count, name, shares, tax_rate, ticker, url, \
    war_eligible FROM corporations WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Like the character `*_with_org` writers, this inserts the CEO and creator characters under
// `defer_foreign_keys = ON` and relies on the deferred characters.corporation_id FK being satisfied
// at commit. It only inserts `corp`, so it is safe ONLY when `ceo_char.corporation_id()` and
// `creator_char.corporation_id()` both equal `corp.id()` (i.e. the CEO/creator are members of the
// corp being inserted). It is currently test-only (no production caller); any future production use
// that persists a CEO/creator belonging to a different corp must first ensure that corp's row
// exists, exactly as sync::structure_resolution::resolve_owner_corporation does for the reference
// CEO. See the deferred FK on migrations/0003_create_orgs.sql.
#[allow(dead_code)]
pub async fn insert_corporation_with_org(
  db: &Database,
  corp: &Corporation,
  alliance: Option<&Alliance>,
  faction: Option<&Faction>,
  ceo_char: &Character,
  creator_char: &Character,
  home_station: Option<&Station>,
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  if let Some(faction) = faction {
    sqlx::query(
      "INSERT OR IGNORE INTO factions \
      (id, corporation_id, description, is_unique, militia_corporation_id, name, \
      size_factor, solar_system_id, station_count, station_system_count) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(faction.id())
    .bind(faction.corporation_id())
    .bind(faction.description())
    .bind(faction.is_unique())
    .bind(faction.militia_corporation_id())
    .bind(faction.name())
    .bind(faction.size_factor())
    .bind(faction.solar_system_id())
    .bind(faction.station_count())
    .bind(faction.station_system_count())
    .execute(&mut *tx)
    .await?;
  }

  if let Some(alliance) = alliance {
    sqlx::query(
      "INSERT OR IGNORE INTO alliances \
      (id, creator_corporation_id, creator_id, date_founded, executor_corporation_id, \
      faction_id, name, ticker) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(alliance.id())
    .bind(alliance.creator_corporation_id())
    .bind(alliance.creator_id())
    .bind(alliance.date_founded())
    .bind(alliance.executor_corporation_id())
    .bind(alliance.faction_id())
    .bind(alliance.name())
    .bind(alliance.ticker())
    .execute(&mut *tx)
    .await?;
  }

  if let Some(station) = home_station {
    sqlx::query(
      "INSERT OR IGNORE INTO stations \
      (id, max_dockable_ship_volume, name, office_rental_cost, owner, \
      position_x, position_y, position_z, race_id, reprocessing_efficiency, \
      reprocessing_stations_take, services, system_id, type_id) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(station.id())
    .bind(station.max_dockable_ship_volume())
    .bind(station.name())
    .bind(station.office_rental_cost())
    .bind(station.owner())
    .bind(station.position_x())
    .bind(station.position_y())
    .bind(station.position_z())
    .bind(station.race_id())
    .bind(station.reprocessing_efficiency())
    .bind(station.reprocessing_stations_take())
    .bind(station.services())
    .bind(station.system_id())
    .bind(station.type_id())
    .execute(&mut *tx)
    .await?;
  }

  sqlx::query(
    "INSERT OR IGNORE INTO characters \
      (id, alliance_id, birthday, bloodline_id, corporation_id, description, faction_id, \
      gender, name, race_id, security_status, title) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(ceo_char.id())
  .bind(ceo_char.alliance_id())
  .bind(ceo_char.birthday())
  .bind(ceo_char.bloodline_id())
  .bind(ceo_char.corporation_id())
  .bind(ceo_char.description())
  .bind(ceo_char.faction_id())
  .bind(ceo_char.gender())
  .bind(ceo_char.name())
  .bind(ceo_char.race_id())
  .bind(ceo_char.security_status())
  .bind(ceo_char.title())
  .execute(&mut *tx)
  .await?;

  if creator_char.id() != ceo_char.id() {
    sqlx::query(
      "INSERT OR IGNORE INTO characters \
      (id, alliance_id, birthday, bloodline_id, corporation_id, description, faction_id, \
      gender, name, race_id, security_status, title) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(creator_char.id())
    .bind(creator_char.alliance_id())
    .bind(creator_char.birthday())
    .bind(creator_char.bloodline_id())
    .bind(creator_char.corporation_id())
    .bind(creator_char.description())
    .bind(creator_char.faction_id())
    .bind(creator_char.gender())
    .bind(creator_char.name())
    .bind(creator_char.race_id())
    .bind(creator_char.security_status())
    .bind(creator_char.title())
    .execute(&mut *tx)
    .await?;
  }

  sqlx::query(
    "INSERT INTO corporations \
      (id, alliance_id, ceo_id, creator_id, date_founded, description, faction_id, \
      home_station_id, member_count, name, shares, tax_rate, ticker, url, war_eligible) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(corp.id())
  .bind(corp.alliance_id())
  .bind(corp.ceo_id())
  .bind(corp.creator_id())
  .bind(corp.creation_date())
  .bind(corp.description())
  .bind(corp.faction_id())
  .bind(corp.home_station_id())
  .bind(corp.member_count())
  .bind(corp.name())
  .bind(corp.shares())
  .bind(corp.tax_rate())
  .bind(corp.ticker())
  .bind(corp.url())
  .bind(corp.war_eligible())
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;
  Ok(())
}

fn into_card_row(sql: CardRowSql) -> CardRow {
  CardRow {
    alliance_name: sql.alliance_name,
    alliance_ticker: sql.alliance_ticker,
    ceo_name: sql.ceo_name,
    corporation_id: sql.corporation_id,
    hq_name: sql.hq_name,
    member_count: sql.member_count,
    name: sql.name,
    tags: Vec::new(),
    tax_rate: sql.tax_rate,
    ticker: sql.ticker,
  }
}

fn push_free_text_predicate(builder: &mut QueryBuilder<Sqlite>, text: &str) {
  let pattern = like_pattern(text);

  builder.push("(oc.name LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR oc.ticker LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR al.name LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR al.ticker LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR ceo.name LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(" ESCAPE '\\' OR hq.name LIKE ");
  builder.push_bind(pattern.clone());
  builder.push(
    " ESCAPE '\\' OR EXISTS (SELECT 1 FROM entity_tags et \
    JOIN tags tg ON tg.id = et.tag_id \
    WHERE et.entity_type = 'corporation' AND et.entity_id = oc.id AND tg.name LIKE ",
  );
  builder.push_bind(pattern);
  builder.push(" ESCAPE '\\'))");
}

fn push_key_value_predicate(builder: &mut QueryBuilder<Sqlite>, key: &str, value: &str) {
  match key {
    "tag" => {
      builder.push(
        "EXISTS (SELECT 1 FROM entity_tags et JOIN tags tg ON tg.id = et.tag_id \
        WHERE et.entity_type = 'corporation' AND et.entity_id = oc.id AND tg.name = ",
      );
      builder.push_bind(value.to_string());
      builder.push(" COLLATE NOCASE)");
    }
    _ => {
      // Unrecognized corp facet key compiles to a predicate that matches no rows.
      builder.push("0 = 1");
    }
  }
}

fn push_token_predicate(builder: &mut QueryBuilder<Sqlite>, token: &FilterToken) {
  if let FilterToken::KeyValue {
    key, ..
  } = token
    && CHARACTER_ONLY_KEYS.contains(&key.as_str())
  {
    // Character-only filter keys don't apply to corporations: emit a true predicate so the token is a no-op.
    builder.push("1 = 1");
    return;
  }

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
        push_key_value_predicate(builder, key, value);
      }
      builder.push(")");
    }
  }
}

pub async fn upsert_corporation(db: &Database, corp: &Corporation) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO corporations \
      (id, alliance_id, ceo_id, creator_id, date_founded, description, faction_id, \
      home_station_id, member_count, name, shares, tax_rate, ticker, url, war_eligible) \
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
  .bind(corp.id())
  .bind(corp.alliance_id())
  .bind(corp.ceo_id())
  .bind(corp.creator_id())
  .bind(corp.creation_date())
  .bind(corp.description())
  .bind(corp.faction_id())
  .bind(corp.home_station_id())
  .bind(corp.member_count())
  .bind(corp.name())
  .bind(corp.shares())
  .bind(corp.tax_rate())
  .bind(corp.ticker())
  .bind(corp.url())
  .bind(corp.war_eligible())
  .execute(&db.0)
  .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn all_alliances(db: &Database) -> Result<Vec<Alliance>, Error> {
  let rows = sqlx::query_as::<_, Alliance>(
    "SELECT creator_corporation_id, creator_id, date_founded, executor_corporation_id, \
    faction_id, id, name, ticker FROM alliances",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn get_alliance(db: &Database, id: i64) -> Result<Option<Alliance>, Error> {
  let row = sqlx::query_as::<_, Alliance>(
    "SELECT creator_corporation_id, creator_id, date_founded, executor_corporation_id, \
    faction_id, id, name, ticker FROM alliances WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn insert_alliance_with_org(
  db: &Database,
  alliance: &Alliance,
  creator_corp: &Corporation,
  creator_char: &Character,
  executor_corp: Option<&Corporation>,
  faction: Option<&Faction>,
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;

  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  if let Some(faction) = faction {
    sqlx::query(
      "INSERT OR IGNORE INTO factions \
      (id, corporation_id, description, is_unique, militia_corporation_id, name, \
      size_factor, solar_system_id, station_count, station_system_count) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(faction.id())
    .bind(faction.corporation_id())
    .bind(faction.description())
    .bind(faction.is_unique())
    .bind(faction.militia_corporation_id())
    .bind(faction.name())
    .bind(faction.size_factor())
    .bind(faction.solar_system_id())
    .bind(faction.station_count())
    .bind(faction.station_system_count())
    .execute(&mut *tx)
    .await?;
  }

  sqlx::query(
    "INSERT INTO alliances \
      (id, creator_corporation_id, creator_id, date_founded, executor_corporation_id, \
      faction_id, name, ticker) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(alliance.id())
  .bind(alliance.creator_corporation_id())
  .bind(alliance.creator_id())
  .bind(alliance.date_founded())
  .bind(alliance.executor_corporation_id())
  .bind(alliance.faction_id())
  .bind(alliance.name())
  .bind(alliance.ticker())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT INTO corporations \
      (id, alliance_id, ceo_id, creator_id, date_founded, description, faction_id, \
      home_station_id, member_count, name, shares, tax_rate, ticker, url, war_eligible) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(creator_corp.id())
  .bind(creator_corp.alliance_id())
  .bind(creator_corp.ceo_id())
  .bind(creator_corp.creator_id())
  .bind(creator_corp.creation_date())
  .bind(creator_corp.description())
  .bind(creator_corp.faction_id())
  .bind(creator_corp.home_station_id())
  .bind(creator_corp.member_count())
  .bind(creator_corp.name())
  .bind(creator_corp.shares())
  .bind(creator_corp.tax_rate())
  .bind(creator_corp.ticker())
  .bind(creator_corp.url())
  .bind(creator_corp.war_eligible())
  .execute(&mut *tx)
  .await?;

  sqlx::query(
    "INSERT INTO characters \
      (id, alliance_id, birthday, bloodline_id, corporation_id, description, faction_id, \
      gender, name, race_id, security_status, title) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(creator_char.id())
  .bind(creator_char.alliance_id())
  .bind(creator_char.birthday())
  .bind(creator_char.bloodline_id())
  .bind(creator_char.corporation_id())
  .bind(creator_char.description())
  .bind(creator_char.faction_id())
  .bind(creator_char.gender())
  .bind(creator_char.name())
  .bind(creator_char.race_id())
  .bind(creator_char.security_status())
  .bind(creator_char.title())
  .execute(&mut *tx)
  .await?;

  if let Some(executor_corp) = executor_corp {
    sqlx::query(
      "INSERT OR IGNORE INTO corporations \
      (id, alliance_id, ceo_id, creator_id, date_founded, description, faction_id, \
      home_station_id, member_count, name, shares, tax_rate, ticker, url, war_eligible) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(executor_corp.id())
    .bind(executor_corp.alliance_id())
    .bind(executor_corp.ceo_id())
    .bind(executor_corp.creator_id())
    .bind(executor_corp.creation_date())
    .bind(executor_corp.description())
    .bind(executor_corp.faction_id())
    .bind(executor_corp.home_station_id())
    .bind(executor_corp.member_count())
    .bind(executor_corp.name())
    .bind(executor_corp.shares())
    .bind(executor_corp.tax_rate())
    .bind(executor_corp.ticker())
    .bind(executor_corp.url())
    .bind(executor_corp.war_eligible())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_alliance(db: &Database, alliance: &Alliance) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO alliances \
      (id, creator_corporation_id, creator_id, date_founded, executor_corporation_id, \
      faction_id, name, ticker) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(id) DO UPDATE SET \
      creator_corporation_id = excluded.creator_corporation_id, \
      creator_id            = excluded.creator_id, \
      date_founded          = excluded.date_founded, \
      executor_corporation_id = excluded.executor_corporation_id, \
      faction_id            = excluded.faction_id, \
      name                  = excluded.name, \
      ticker                = excluded.ticker",
  )
  .bind(alliance.id())
  .bind(alliance.creator_corporation_id())
  .bind(alliance.creator_id())
  .bind(alliance.date_founded())
  .bind(alliance.executor_corporation_id())
  .bind(alliance.faction_id())
  .bind(alliance.name())
  .bind(alliance.ticker())
  .execute(&db.0)
  .await?;
  Ok(())
}

pub async fn all_owned_corporations(db: &Database) -> Result<Vec<OwnedCorporation>, Error> {
  let rows = sqlx::query_as::<_, OwnedCorporation>(
    "SELECT alliance_id, authorized_by, ceo_id, date_founded, description, home_station_id, id, \
    member_count, name, tax_rate, ticker, url, war_eligible FROM owned_corporations \
    ORDER BY name",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_for_corporation(
  db: &Database,
  corporation_id: i64,
  roles: &[CorporationMemberRole],
) -> Result<(), Error> {
  let mut tx = db.0.begin().await?;
  sqlx::query("DELETE FROM corporation_member_roles WHERE corporation_id = ?")
    .bind(corporation_id)
    .execute(&mut *tx)
    .await?;
  for entry in roles {
    sqlx::query(
      "INSERT INTO corporation_member_roles (corporation_id, character_id, role) VALUES (?, ?, ?) \
      ON CONFLICT(corporation_id, character_id, role) DO NOTHING",
    )
    .bind(entry.corporation_id())
    .bind(entry.character_id())
    .bind(entry.role())
    .execute(&mut *tx)
    .await?;
  }
  tx.commit().await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn for_corporation(db: &Database, corporation_id: i64) -> Result<Vec<CorporationMemberRole>, Error> {
  let rows = sqlx::query_as::<_, CorporationMemberRole>(
    "SELECT character_id, corporation_id, role FROM corporation_member_roles \
    WHERE corporation_id = ? ORDER BY character_id, role",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corp_is_authorized(db: &Database, corporation_id: i64) -> Result<bool, Error> {
  let authorized = sqlx::query_scalar::<_, i64>(
    "SELECT 1 FROM owned_corporations oc \
    JOIN corporation_member_roles cmr \
      ON cmr.corporation_id = oc.id \
    AND cmr.character_id = oc.authorized_by \
    AND cmr.role = ? \
    WHERE oc.id = ? \
    LIMIT 1",
  )
  .bind(REQUIRED_ASSET_ROLE)
  .bind(corporation_id)
  .fetch_optional(&db.0)
  .await?;
  Ok(authorized.is_some())
}

#[cfg(test)]
mod corporation_tests {
  use super::*;
  use crate::{
    store,
    store::{
      model::{Bloodline, Gender, Race},
      repo::character,
    },
  };

  fn make_alliance(id: i64, corp_id: i64, creator_id: i64) -> Alliance {
    Alliance::new(id, corp_id, creator_id, "2020-01-01", "Test Alliance", "TST")
  }

  fn make_bloodline(corp_id: i64) -> Bloodline {
    Bloodline::new(1, corp_id, 1, 3, "A bloodline.", 7, 5, "Test", 6, 4)
  }

  fn make_character(id: i64, corp_id: i64) -> Character {
    Character::new(id, 1, corp_id, 1, "1990-01-01", Gender::Male, "Test Char")
  }

  fn make_corporation(id: i64, ceo_id: i64) -> Corporation {
    let mut corp = Corporation::new(id, "Test Corp", "TSC");
    corp.set_ceo_id(ceo_id);
    corp.set_creator_id(ceo_id);
    corp.set_member_count(100);
    corp.set_tax_rate(0.05);
    corp
  }

  fn make_race(alliance_id: i64) -> Race {
    Race::new(1, alliance_id, "A race.", "Test Race")
  }

  async fn seed_character(db: &store::Database, corp_id: i64, char_id: i64) {
    let alliance = make_alliance(corp_id, corp_id, char_id);
    let corporation = make_corporation(corp_id, char_id);
    let race = make_race(corp_id);
    let bloodline = make_bloodline(corp_id);
    let char = make_character(char_id, corp_id);
    character::insert_with_org(db, &char, &bloodline, &race, &corporation, Some(&alliance), None)
      .await
      .unwrap();
  }

  mod search {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{
        Constellation, ENTITY_TYPE_CHARACTER, ENTITY_TYPE_CORPORATION, ItemCategory, ItemGroup, ItemType, OwnerType,
        Region, SolarSystem, Station,
      },
      repo::{infra, sde},
      search::parse,
    };

    const COBALT_CEO: i64 = 7001;
    const COBALT_INDUSTRIES: i64 = 2001;
    const RED_CEO: i64 = 7002;
    const RED_SYNDICATE: i64 = 2002;

    fn make_constellation() -> Constellation {
      Constellation {
        id: 20_000_001,
        name: "Test Constellation".to_string(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        region_id: 10_000_001,
      }
    }

    fn make_item_category() -> ItemCategory {
      ItemCategory {
        icon_id: None,
        id: 1,
        name: "Station".to_string(),
        published: true,
      }
    }

    fn make_item_group() -> ItemGroup {
      ItemGroup {
        category_id: 1,
        icon_id: None,
        id: 1,
        name: "Station".to_string(),
        published: true,
      }
    }

    fn make_item_type(id: i64) -> ItemType {
      ItemType {
        capacity: None,
        description: Some("Test Item Type".to_string()),
        dogma_attributes: "[]".to_string(),
        group_id: 1,
        icon_id: None,
        id,
        market_group_id: None,
        name: "Caldari Station".to_string(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      }
    }

    fn make_region() -> Region {
      Region {
        description: None,
        id: 10_000_001,
        name: "Test Region".to_string(),
      }
    }

    fn make_solar_system() -> SolarSystem {
      SolarSystem {
        constellation_id: 20_000_001,
        id: 30_000_142,
        name: "Jita".to_string(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        security_class: None,
        security_status: 0.9,
        star_id: None,
      }
    }

    fn make_station(id: i64, name: &str) -> Station {
      Station {
        id,
        max_dockable_ship_volume: 1_000_000.0,
        name: name.to_string(),
        office_rental_cost: 10_000.0,
        owner: None,
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        race_id: None,
        reprocessing_efficiency: 0.5,
        reprocessing_stations_take: 0.05,
        services: "[]".to_string(),
        system_id: 30_000_142,
        type_id: 52678,
      }
    }

    async fn seed_hq_station(db: &Database, station_id: i64, station_name: &str) {
      sde::upsert_item_category(db, &make_item_category()).await.unwrap();
      sde::upsert_item_group(db, &make_item_group()).await.unwrap();
      sde::upsert_item_type(db, &make_item_type(52678)).await.unwrap();
      sde::insert_station_with_geography(
        db,
        &make_station(station_id, station_name),
        &make_solar_system(),
        &make_constellation(),
        &make_region(),
      )
      .await
      .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    async fn seed_owned_corp(
      db: &Database,
      corp_id: i64,
      name: &str,
      ticker: &str,
      ceo_id: i64,
      ceo_name: &str,
      alliance: Option<(&str, &str)>,
      hq: Option<(i64, &str)>,
    ) {
      let mut corporation = Corporation::new(corp_id, name, ticker);
      corporation.set_ceo_id(ceo_id);
      corporation.set_creator_id(ceo_id);
      corporation.set_member_count(100);
      corporation.set_tax_rate(0.1);

      let alliance_model = alliance.map(|(alliance_name, alliance_ticker)| {
        let alliance_id = corp_id + 90_000;
        corporation.set_alliance_id(alliance_id);
        Alliance::new(
          alliance_id,
          corp_id,
          ceo_id,
          "2020-01-01",
          alliance_name,
          alliance_ticker,
        )
      });

      if let Some((station_id, station_name)) = hq {
        corporation.set_home_station_id(station_id);
        seed_hq_station(db, station_id, station_name).await;
      }

      let ceo = Character::new(ceo_id, 1, corp_id, 1, "1990-01-01", Gender::Male, ceo_name);
      let bloodline = make_bloodline(corp_id);
      let race = make_race(corp_id);

      character::insert_with_org(db, &ceo, &bloodline, &race, &corporation, alliance_model.as_ref(), None)
        .await
        .unwrap();
      infra::upsert(
        db,
        corp_id,
        OwnerType::Corporation,
        "tok",
        "rt",
        9999,
        Some(ceo_id),
        None,
      )
      .await
      .unwrap();
    }

    async fn seed_owned_corps(db: &Database) {
      seed_owned_corp(
        db,
        COBALT_INDUSTRIES,
        "Cobalt Industries",
        "CBLT",
        COBALT_CEO,
        "Cobalt Director",
        Some(("Brave Collective", "BRAVE")),
        Some((60_003_760, "Jita Trade Hub")),
      )
      .await;
      seed_owned_corp(
        db,
        RED_SYNDICATE,
        "Red Syndicate",
        "REDS",
        RED_CEO,
        "Red Overlord",
        None,
        None,
      )
      .await;

      let mining = infra::create(db, "Mining", None, Some("#00ccff")).await.unwrap();
      let pvp = infra::create(db, "PvP", None, None).await.unwrap();
      infra::assign(db, ENTITY_TYPE_CORPORATION, COBALT_INDUSTRIES, mining.id())
        .await
        .unwrap();
      infra::assign(db, ENTITY_TYPE_CORPORATION, RED_SYNDICATE, pvp.id())
        .await
        .unwrap();
    }

    async fn matching(db: &Database, query: &str) -> Vec<i64> {
      search_corporations(db, &parse(query))
        .await
        .unwrap()
        .iter()
        .map(|row| row.corporation_id)
        .collect()
    }

    #[tokio::test]
    async fn it_returns_all_owned_corps_for_an_empty_query() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "").await, vec![COBALT_INDUSTRIES, RED_SYNDICATE]);
    }

    #[tokio::test]
    async fn it_matches_free_text_on_corp_name_or_ticker() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "cobalt").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "reds").await, vec![RED_SYNDICATE]);
    }

    #[tokio::test]
    async fn it_matches_free_text_on_alliance_name_or_ticker() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "brave collective").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "brave").await, vec![COBALT_INDUSTRIES]);
    }

    #[tokio::test]
    async fn it_matches_free_text_on_ceo_character_name() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "director").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "overlord").await, vec![RED_SYNDICATE]);
    }

    #[tokio::test]
    async fn it_matches_free_text_on_hq_station_name() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "trade hub").await, vec![COBALT_INDUSTRIES]);
    }

    #[tokio::test]
    async fn it_matches_free_text_on_a_tag_name() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "mining").await, vec![COBALT_INDUSTRIES]);
    }

    #[tokio::test]
    async fn it_matches_a_tag_key_with_or_within_the_values() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "tag:mining").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(
        matching(&db, "tag:mining,pvp").await,
        vec![COBALT_INDUSTRIES, RED_SYNDICATE]
      );
    }

    #[tokio::test]
    async fn it_negates_a_tag_filter() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "-tag:pvp").await, vec![COBALT_INDUSTRIES]);
    }

    #[tokio::test]
    async fn it_matches_a_quoted_phrase() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "\"jita trade hub\"").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "tag:\"Mining\"").await, vec![COBALT_INDUSTRIES]);
    }

    #[tokio::test]
    async fn it_ands_multiple_tokens() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "cobalt tag:mining").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "cobalt tag:pvp").await, Vec::<i64>::new());
    }

    #[tokio::test]
    async fn it_treats_character_only_keys_as_no_ops() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      for query in [
        "status:docked",
        "training:active",
        "loc:jita",
        "name:cobalt",
        "corp:cobalt",
      ] {
        assert_eq!(
          matching(&db, query).await,
          vec![COBALT_INDUSTRIES, RED_SYNDICATE],
          "expected `{query}` to be a no-op"
        );
      }

      assert_eq!(
        matching(&db, "-status:docked").await,
        vec![COBALT_INDUSTRIES, RED_SYNDICATE]
      );
    }

    #[tokio::test]
    async fn it_carries_the_corp_card_projection_fields() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      let rows = search_corporations(&db, &parse("tag:mining")).await.unwrap();

      assert_eq!(rows.len(), 1);
      let cobalt = &rows[0];
      assert_eq!(cobalt.name, "Cobalt Industries");
      assert_eq!(cobalt.ticker, "CBLT");
      assert_eq!(cobalt.alliance_name.as_deref(), Some("Brave Collective"));
      assert_eq!(cobalt.alliance_ticker.as_deref(), Some("BRAVE"));
      assert_eq!(cobalt.ceo_name.as_deref(), Some("Cobalt Director"));
      assert_eq!(cobalt.hq_name.as_deref(), Some("Jita Trade Hub"));
      assert_eq!(cobalt.member_count, 100);

      assert_eq!(cobalt.tags.len(), 1);
      assert_eq!(cobalt.tags[0].name, "Mining");
      assert_eq!(cobalt.tags[0].color_hex.as_deref(), Some("#00ccff"));
    }

    #[tokio::test]
    async fn it_does_not_match_corp_only_tags_against_character_tags() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;
      let leak = infra::create(&db, "Leak", None, None).await.unwrap();
      infra::assign(&db, ENTITY_TYPE_CHARACTER, COBALT_CEO, leak.id())
        .await
        .unwrap();

      assert_eq!(matching(&db, "tag:leak").await, Vec::<i64>::new());
      assert_eq!(matching(&db, "leak").await, Vec::<i64>::new());
    }
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_corporations_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_corporations(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_corporation(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }
  }

  mod insert_with_org {
    use super::*;

    #[tokio::test]
    async fn it_inserts_corporation_without_optional_org_members() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 2001, 8001).await;
      let ceo = make_character(8001, 2001);
      let corp = make_corporation(20002, 8001);

      insert_corporation_with_org(&db, &corp, None, None, &ceo, &ceo, None)
        .await
        .unwrap();

      let result = get_corporation(&db, 20002).await.unwrap();
      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_inserts_with_separate_ceo_and_creator() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 3001, 9001).await;
      let ceo = make_character(9001, 3001);
      let corp = make_corporation(30002, 9001);

      insert_corporation_with_org(&db, &corp, None, None, &ceo, &ceo, None)
        .await
        .unwrap();

      let result = get_corporation(&db, 30002).await.unwrap();
      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_inserts_the_alliance_when_present() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 5001, 9501).await;
      let alliance = make_alliance(99_000_004, 5001, 9501);
      let ceo = make_character(9501, 5001);
      let corp = make_corporation(50002, 9501);

      insert_corporation_with_org(&db, &corp, Some(&alliance), None, &ceo, &ceo, None)
        .await
        .unwrap();

      assert!(get_corporation(&db, 50002).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_inserts_the_faction_when_present() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 6001, 9601).await;
      let ceo = make_character(9601, 6001);
      let corp = make_corporation(60002, 9601);
      let faction = Faction::new(500_002, "Test Faction", true, 1.0, 100, 50);

      insert_corporation_with_org(&db, &corp, None, Some(&faction), &ceo, &ceo, None)
        .await
        .unwrap();

      assert!(get_corporation(&db, 60002).await.unwrap().is_some());
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_updates_an_existing_corporation() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 4001, 7001).await;
      let ceo = make_character(7001, 4001);
      let corp = make_corporation(40002, 7001);
      insert_corporation_with_org(&db, &corp, None, None, &ceo, &ceo, None)
        .await
        .unwrap();

      let mut updated = make_corporation(40002, 7001);
      updated.set_member_count(999);
      upsert_corporation(&db, &updated).await.unwrap();

      let result = get_corporation(&db, 40002).await.unwrap().unwrap();
      assert_eq!(result.member_count(), Some(999));
    }
  }
}

#[cfg(test)]
mod alliance_tests {
  use super::*;
  use crate::store;

  fn make_alliance(id: i64, creator_corp_id: i64, creator_id: i64) -> Alliance {
    Alliance::new(id, creator_corp_id, creator_id, "2020-01-01", "Test Alliance", "TST")
  }

  fn make_character(id: i64, corp_id: i64) -> Character {
    use crate::store::model::Gender;
    Character::new(id, 1, corp_id, 1, "1990-01-01", Gender::Male, "Test Character")
  }

  fn make_corporation(id: i64, ceo_id: i64, creator_id: i64) -> Corporation {
    let mut corp = Corporation::new(id, "Test Corp", "TSC");
    corp.set_ceo_id(ceo_id);
    corp.set_creator_id(creator_id);
    corp.set_member_count(100);
    corp.set_tax_rate(0.05);
    corp
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_alliances_exist() {
      let db = store::open_test().await.unwrap();

      let result = all_alliances(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_id() {
      let db = store::open_test().await.unwrap();

      let result = get_alliance(&db, 9999).await.unwrap();

      assert_eq!(result, None);
    }
  }

  mod insert_with_org {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{Bloodline, Race},
      repo::sde,
    };

    async fn seed_universe(db: &store::Database) {
      sde::upsert_race(db, &Race::new(1, 500_001, "Caldari", "Caldari"))
        .await
        .unwrap();
      sde::upsert_bloodline(db, &Bloodline::new(1, 1_000_001, 1, 5, "Civire", 6, 7, "Civire", 8, 9))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_inserts_the_full_org_stack() {
      let db = store::open_test().await.unwrap();
      seed_universe(&db).await;
      let alliance = make_alliance(99_000_001, 90_000_001, 12_345_678);
      let creator_corp = make_corporation(90_000_001, 12_345_678, 12_345_678);
      let creator_char = make_character(12_345_678, 90_000_001);

      insert_alliance_with_org(&db, &alliance, &creator_corp, &creator_char, None, None)
        .await
        .unwrap();

      let found = get_alliance(&db, 99_000_001).await.unwrap().unwrap();
      assert_eq!(found.id(), 99_000_001);
      assert_eq!(found.ticker(), "TST");
    }

    #[tokio::test]
    async fn it_inserts_with_an_executor_corporation() {
      let db = store::open_test().await.unwrap();
      seed_universe(&db).await;
      let alliance = make_alliance(99_000_002, 90_000_002, 12_345_679);
      let creator_corp = make_corporation(90_000_002, 12_345_679, 12_345_679);
      let executor_corp = make_corporation(90_000_003, 12_345_680, 12_345_680);
      let creator_char = make_character(12_345_679, 90_000_002);

      insert_alliance_with_org(&db, &alliance, &creator_corp, &creator_char, Some(&executor_corp), None)
        .await
        .unwrap();

      assert!(get_alliance(&db, 99_000_002).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_inserts_with_a_faction() {
      let db = store::open_test().await.unwrap();
      seed_universe(&db).await;
      let alliance = make_alliance(99_000_004, 90_000_004, 12_345_681);
      let creator_corp = make_corporation(90_000_004, 12_345_681, 12_345_681);
      let creator_char = make_character(12_345_681, 90_000_004);
      let faction = Faction::new(500_001, "Test Faction", true, 1.0, 100, 50);

      insert_alliance_with_org(&db, &alliance, &creator_corp, &creator_char, None, Some(&faction))
        .await
        .unwrap();

      assert!(get_alliance(&db, 99_000_004).await.unwrap().is_some());
    }
  }
}

#[cfg(test)]
mod owned_corporation_tests {
  use super::*;
  use crate::{
    store,
    store::{
      model::{Bloodline, Gender, OwnerType, Race},
      repo::{character, infra},
    },
  };

  fn make_corporation(id: i64, ceo_id: i64) -> Corporation {
    let mut corp = Corporation::new(id, "Test Corp", "TSC");
    corp.set_ceo_id(ceo_id);
    corp.set_creator_id(ceo_id);
    corp.set_member_count(100);
    corp.set_tax_rate(0.05);
    corp
  }

  async fn seed_owned_character(db: &store::Database, corp_id: i64, char_id: i64) {
    let alliance = Alliance::new(corp_id, corp_id, char_id, "2020-01-01", "Test Alliance", "TST");
    let corporation = make_corporation(corp_id, char_id);
    let race = Race::new(1, corp_id, "A race.", "Test Race");
    let bloodline = Bloodline::new(1, corp_id, 1, 3, "A bloodline.", 7, 5, "Test", 6, 4);
    let char = Character::new(char_id, 1, corp_id, 1, "1990-01-01", Gender::Male, "Test Char");
    character::insert_with_org(db, &char, &bloodline, &race, &corporation, Some(&alliance), None)
      .await
      .unwrap();
    infra::upsert(db, char_id, OwnerType::Character, "tok", "rt", 9999, None, None)
      .await
      .unwrap();
  }

  async fn own_corporation(db: &store::Database, corp_id: i64, authorized_by: i64) {
    infra::upsert(
      db,
      corp_id,
      OwnerType::Corporation,
      "tok",
      "rt",
      9999,
      Some(authorized_by),
      None,
    )
    .await
    .unwrap();
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_corporations_are_owned() {
      let db = store::open_test().await.unwrap();

      let result = all_owned_corporations(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_excludes_reference_corporations_without_a_credential() {
      let db = store::open_test().await.unwrap();
      seed_owned_character(&db, 2001, 8001).await;

      let result = all_owned_corporations(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_returns_only_director_added_corporations_with_the_authorizer() {
      let db = store::open_test().await.unwrap();
      seed_owned_character(&db, 2001, 8001).await;
      own_corporation(&db, 2001, 8001).await;

      let result = all_owned_corporations(&db).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].id(), 2001);
      assert_eq!(result[0].authorized_by(), Some(8001));
    }
  }
}

#[cfg(test)]
mod member_role_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Corporation, OwnerType},
    repo::{infra, org},
  };

  const CORP_ID: i64 = 90_000_001;
  const DIRECTOR_ID: i64 = 100;

  fn role(corporation_id: i64, character_id: i64, role: &str) -> CorporationMemberRole {
    CorporationMemberRole::from((corporation_id, character_id, role.to_string()))
  }

  async fn seed_corp(db: &store::Database) {
    let mut corp = Corporation::new(CORP_ID, "Test Corp", "TST");
    corp.set_ceo_id(DIRECTOR_ID);
    corp.set_creator_id(DIRECTOR_ID);
    corp.set_member_count(1);
    corp.set_tax_rate(0.1);
    org::upsert_corporation(db, &corp).await.unwrap();
  }

  async fn seed_credential(db: &store::Database, authorized_by: Option<i64>) {
    infra::upsert(
      db,
      CORP_ID,
      OwnerType::Corporation,
      "tok",
      "rt",
      4_102_444_800,
      authorized_by,
      None,
    )
    .await
    .unwrap();
  }

  mod replace_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_persists_a_characters_role_set() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;

      replace_for_corporation(
        &db,
        CORP_ID,
        &[
          role(CORP_ID, DIRECTOR_ID, "Director"),
          role(CORP_ID, DIRECTOR_ID, "Accountant"),
        ],
      )
      .await
      .unwrap();

      let rows = for_corporation(&db, CORP_ID).await.unwrap();
      assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn it_replaces_the_prior_role_set() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Director")])
        .await
        .unwrap();

      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Accountant")])
        .await
        .unwrap();

      let rows = for_corporation(&db, CORP_ID).await.unwrap();
      assert_eq!(rows, vec![role(CORP_ID, DIRECTOR_ID, "Accountant")]);
    }

    #[tokio::test]
    async fn it_clears_roles_with_an_empty_slice() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Director")])
        .await
        .unwrap();

      replace_for_corporation(&db, CORP_ID, &[]).await.unwrap();

      assert!(for_corporation(&db, CORP_ID).await.unwrap().is_empty());
    }
  }

  mod corp_is_authorized {
    use super::*;

    #[tokio::test]
    async fn it_authorizes_an_owned_corp_whose_director_holds_the_role() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db, Some(DIRECTOR_ID)).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Director")])
        .await
        .unwrap();

      assert!(corp_is_authorized(&db, CORP_ID).await.unwrap());
    }

    #[tokio::test]
    async fn it_rejects_an_owned_corp_whose_authorizer_lacks_the_role() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db, Some(DIRECTOR_ID)).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Accountant")])
        .await
        .unwrap();

      assert!(!corp_is_authorized(&db, CORP_ID).await.unwrap());
    }

    #[tokio::test]
    async fn it_rejects_a_corp_with_no_credential() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Director")])
        .await
        .unwrap();

      assert!(!corp_is_authorized(&db, CORP_ID).await.unwrap());
    }

    #[tokio::test]
    async fn it_rejects_when_a_different_character_holds_the_role() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_credential(&db, Some(DIRECTOR_ID)).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, 999, "Director")])
        .await
        .unwrap();

      assert!(!corp_is_authorized(&db, CORP_ID).await.unwrap());
    }
  }
}
