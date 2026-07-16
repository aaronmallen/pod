use std::collections::{BTreeSet, HashMap, HashSet};

use sqlx::{QueryBuilder, Sqlite};

use crate::store::{
  Database, Error,
  model::{
    Alliance, Character, ContactCursor, ContactSortColumn, ContactSortDir, Corporation, CorporationContact,
    CorporationContactLabel, CorporationKillEntry, CorporationKillmailAttacker, CorporationKillmailItem,
    CorporationMemberRole, CorporationMiningExtraction, CorporationStanding, Faction, OwnedCorporation,
    SeedCorporation, Station,
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
// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn insert_corporation_with_org(
  db: &Database,
  corp: &Corporation,
  alliance: Option<&Alliance>,
  faction: Option<&Faction>,
  ceo_char: &Character,
  creator_char: &Character,
  home_station: Option<&Station>,
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

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
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn corporation_names(db: &Database) -> Result<HashMap<i64, String>, Error> {
  let rows = sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM corporations")
    .fetch_all(&db.0)
    .await?;
  Ok(rows.into_iter().collect())
}

pub async fn upsert_many_seed_corporations(db: &Database, corporations: &[SeedCorporation]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;

  for corporation in corporations {
    sqlx::query(
      "INSERT INTO corporations \
        (id, ceo_id, creator_id, faction_id, home_station_id, member_count, name, tax_rate, ticker) \
      VALUES (?, 0, 0, ?, ?, 0, ?, 0.0, ?) \
      ON CONFLICT(id) DO UPDATE SET \
        faction_id      = excluded.faction_id, \
        home_station_id = excluded.home_station_id, \
        name            = excluded.name, \
        ticker          = excluded.ticker",
    )
    .bind(corporation.id)
    .bind(corporation.faction_id)
    .bind(corporation.home_station_id)
    .bind(corporation.name.as_str())
    .bind(corporation.ticker.as_str())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

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

// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn insert_alliance_with_org(
  db: &Database,
  alliance: &Alliance,
  creator_corp: &Corporation,
  creator_char: &Character,
  executor_corp: Option<&Corporation>,
  faction: Option<&Faction>,
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

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
  .execute(db.writer())
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
  let mut tx = db.writer().begin().await?;
  let character_ids: BTreeSet<i64> = roles.iter().map(CorporationMemberRole::character_id).collect();
  for character_id in character_ids {
    sqlx::query("DELETE FROM corporation_member_roles WHERE corporation_id = ? AND character_id = ?")
      .bind(corporation_id)
      .bind(character_id)
      .execute(&mut *tx)
      .await?;
  }
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

pub async fn replace_contacts_for_corporation(
  db: &Database,
  corporation_id: i64,
  contacts: &[CorporationContact],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_contacts WHERE corporation_id = ?")
    .bind(corporation_id)
    .execute(&mut *tx)
    .await?;

  for contact in contacts {
    sqlx::query(
      "INSERT INTO corporation_contacts \
        (corporation_id, contact_id, contact_type, standing, is_watched, is_blocked, label_ids, contact_name) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(contact.corporation_id())
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

pub async fn replace_labels_for_corporation(
  db: &Database,
  corporation_id: i64,
  labels: &[CorporationContactLabel],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_contact_labels WHERE corporation_id = ?")
    .bind(corporation_id)
    .execute(&mut *tx)
    .await?;

  for label in labels {
    sqlx::query("INSERT INTO corporation_contact_labels (corporation_id, label_id, label_name) VALUES (?, ?, ?)")
      .bind(label.corporation_id())
      .bind(label.label_id())
      .bind(label.label_name())
      .execute(&mut *tx)
      .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn corporation_contact_labels(
  db: &Database,
  corporation_id: i64,
) -> Result<Vec<CorporationContactLabel>, Error> {
  let labels = sqlx::query_as::<_, CorporationContactLabel>(
    "SELECT corporation_id, label_id, label_name FROM corporation_contact_labels \
    WHERE corporation_id = ? ORDER BY label_id",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(labels)
}

// Filter, cursor, and limit parameters of a keyset-paginated query; bundling them would only move the fields.
#[allow(clippy::too_many_arguments)]
pub async fn corporation_contacts_page(
  db: &Database,
  corporation_id: i64,
  contact_type: Option<&str>,
  query: Option<&str>,
  sort: ContactSortColumn,
  dir: ContactSortDir,
  cursor: Option<&ContactCursor>,
  limit: i64,
) -> Result<Vec<CorporationContact>, Error> {
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
    "SELECT corporation_id, contact_id, contact_name, contact_type, is_blocked, is_watched, label_ids, standing \
    FROM corporation_contacts WHERE corporation_id = ",
  );
  builder.push_bind(corporation_id);
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

  let rows = builder.build_query_as::<CorporationContact>().fetch_all(&db.0).await?;
  Ok(rows)
}

pub async fn replace_standings_for_corporation(
  db: &Database,
  corporation_id: i64,
  standings: &[CorporationStanding],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_standings WHERE corporation_id = ?")
    .bind(corporation_id)
    .execute(&mut *tx)
    .await?;

  for standing in standings {
    sqlx::query(
      "INSERT INTO corporation_standings (corporation_id, from_id, from_type, standing, from_name) \
      VALUES (?, ?, ?, ?, ?)",
    )
    .bind(standing.corporation_id())
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

pub async fn corporation_mining_extractions(
  db: &Database,
  corporation_id: i64,
) -> Result<Vec<CorporationMiningExtraction>, Error> {
  let rows = sqlx::query_as::<_, CorporationMiningExtraction>(
    "SELECT \
      cme.corporation_id, \
      cme.structure_id, \
      cme.moon_id, \
      cme.chunk_arrival_time, \
      cme.extraction_start_time, \
      cme.natural_decay_time, \
      moons.name AS moon_name, \
      moons.solar_system_id AS solar_system_id, \
      solar_systems.security_status AS security_status \
    FROM corporation_mining_extractions cme \
    LEFT JOIN moons ON moons.id = cme.moon_id \
    LEFT JOIN solar_systems ON solar_systems.id = moons.solar_system_id \
    WHERE cme.corporation_id = ? \
    ORDER BY cme.chunk_arrival_time",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn replace_extractions_for_corporation(
  db: &Database,
  corporation_id: i64,
  extractions: &[CorporationMiningExtraction],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_mining_extractions WHERE corporation_id = ?")
    .bind(corporation_id)
    .execute(&mut *tx)
    .await?;

  for extraction in extractions {
    sqlx::query(
      "INSERT INTO corporation_mining_extractions \
        (corporation_id, structure_id, moon_id, chunk_arrival_time, extraction_start_time, natural_decay_time) \
      VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(extraction.corporation_id())
    .bind(extraction.structure_id())
    .bind(extraction.moon_id())
    .bind(extraction.chunk_arrival_time())
    .bind(extraction.extraction_start_time())
    .bind(extraction.natural_decay_time())
    .execute(&mut *tx)
    .await?;
  }

  tx.commit().await?;
  Ok(())
}

pub async fn upsert_corporation_killmail(db: &Database, killmail: &CorporationKillEntry) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO corporation_killmails \
      (corporation_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, victim_corp_id, \
      victim_alliance_id, victim_damage_taken, system_id, \
      value_isk, value_destroyed_isk, value_source, value_recheck_count, value_final, \
      attacker_count, final_blow, kill_time, synced_at) \
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (corporation_id, killmail_id) DO UPDATE SET \
      kill_hash = excluded.kill_hash, is_kill = excluded.is_kill, ship_type_id = excluded.ship_type_id, \
      victim_id = excluded.victim_id, victim_corp_id = excluded.victim_corp_id, \
      victim_alliance_id = excluded.victim_alliance_id, victim_damage_taken = excluded.victim_damage_taken, \
      system_id = excluded.system_id, \
      value_isk = excluded.value_isk, value_destroyed_isk = excluded.value_destroyed_isk, \
      value_source = excluded.value_source, value_recheck_count = excluded.value_recheck_count, \
      value_final = excluded.value_final, attacker_count = excluded.attacker_count, \
      final_blow = excluded.final_blow, kill_time = excluded.kill_time, synced_at = excluded.synced_at",
  )
  .bind(killmail.corporation_id())
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
  .execute(db.writer())
  .await?;
  Ok(())
}

pub async fn corporation_killmail_ids(db: &Database, corporation_id: i64) -> Result<HashSet<i64>, Error> {
  let ids = sqlx::query_scalar::<_, i64>("SELECT killmail_id FROM corporation_killmails WHERE corporation_id = ?")
    .bind(corporation_id)
    .fetch_all(&db.0)
    .await?;
  Ok(ids.into_iter().collect())
}

pub async fn upsert_corporation_killmail_detail(
  db: &Database,
  corporation_id: i64,
  killmail_id: i64,
  attackers: &[CorporationKillmailAttacker],
  items: &[CorporationKillmailItem],
) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;

  sqlx::query("DELETE FROM corporation_killmail_attackers WHERE corporation_id = ? AND killmail_id = ?")
    .bind(corporation_id)
    .bind(killmail_id)
    .execute(&mut *tx)
    .await?;
  sqlx::query("DELETE FROM corporation_killmail_items WHERE corporation_id = ? AND killmail_id = ?")
    .bind(corporation_id)
    .bind(killmail_id)
    .execute(&mut *tx)
    .await?;

  for attacker in attackers {
    sqlx::query(
      "INSERT INTO corporation_killmail_attackers \
        (corporation_id, killmail_id, ordinal, attacker_character_id, attacker_corporation_id, alliance_id, \
        ship_type_id, damage_done, final_blow) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(attacker.corporation_id())
    .bind(attacker.killmail_id())
    .bind(attacker.ordinal())
    .bind(attacker.attacker_character_id())
    .bind(attacker.attacker_corporation_id())
    .bind(attacker.alliance_id())
    .bind(attacker.ship_type_id())
    .bind(attacker.damage_done())
    .bind(attacker.final_blow())
    .execute(&mut *tx)
    .await?;
  }

  for item in items {
    sqlx::query(
      "INSERT INTO corporation_killmail_items \
        (corporation_id, killmail_id, ordinal, type_id, flag, quantity_destroyed, quantity_dropped, value_isk) \
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(item.corporation_id())
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

pub async fn corporation_killmail_attackers(
  db: &Database,
  corporation_id: i64,
  killmail_id: i64,
) -> Result<Vec<CorporationKillmailAttacker>, Error> {
  let rows = sqlx::query_as::<_, CorporationKillmailAttacker>(
    "SELECT alliance_id, attacker_character_id, attacker_corporation_id, corporation_id, damage_done, final_blow, \
      killmail_id, ordinal, ship_type_id FROM corporation_killmail_attackers \
    WHERE corporation_id = ? AND killmail_id = ? ORDER BY ordinal",
  )
  .bind(corporation_id)
  .bind(killmail_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corporation_killmail_items(
  db: &Database,
  corporation_id: i64,
  killmail_id: i64,
) -> Result<Vec<CorporationKillmailItem>, Error> {
  let rows = sqlx::query_as::<_, CorporationKillmailItem>(
    "SELECT corporation_id, flag, killmail_id, ordinal, quantity_destroyed, quantity_dropped, type_id, value_isk \
    FROM corporation_killmail_items WHERE corporation_id = ? AND killmail_id = ? ORDER BY ordinal",
  )
  .bind(corporation_id)
  .bind(killmail_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corporation_killmails(db: &Database, corporation_id: i64) -> Result<Vec<CorporationKillEntry>, Error> {
  let rows = sqlx::query_as::<_, CorporationKillEntry>(
    "SELECT corporation_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, victim_corp_id, \
      victim_alliance_id, victim_damage_taken, system_id, \
      value_isk, value_destroyed_isk, value_source, value_recheck_count, value_final, \
      attacker_count, final_blow, kill_time, synced_at FROM corporation_killmails \
    WHERE corporation_id = ? ORDER BY kill_time DESC, killmail_id DESC",
  )
  .bind(corporation_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

pub async fn corporation_killmails_page(
  db: &Database,
  corporation_id: i64,
  after: Option<(String, i64)>,
  limit: i64,
) -> Result<Vec<CorporationKillEntry>, Error> {
  let mut builder = QueryBuilder::<Sqlite>::new(
    "SELECT corporation_id, killmail_id, kill_hash, is_kill, ship_type_id, victim_id, victim_corp_id, \
      victim_alliance_id, victim_damage_taken, system_id, \
      value_isk, value_destroyed_isk, value_source, value_recheck_count, value_final, \
      attacker_count, final_blow, kill_time, synced_at FROM corporation_killmails WHERE corporation_id = ",
  );
  builder.push_bind(corporation_id);
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

  let rows = builder
    .build_query_as::<CorporationKillEntry>()
    .fetch_all(&db.0)
    .await?;
  Ok(rows)
}

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

  mod corporation_contact_labels {
    use pretty_assertions::assert_eq;

    use super::*;

    const CORP_ID: i64 = 5001;

    #[tokio::test]
    async fn it_returns_the_labels_ordered_by_label_id() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CORP_ID, 8001).await;
      let labels = vec![
        CorporationContactLabel {
          corporation_id: CORP_ID,
          label_id: 20,
          label_name: "Allies".to_owned(),
        },
        CorporationContactLabel {
          corporation_id: CORP_ID,
          label_id: 10,
          label_name: "Hostiles".to_owned(),
        },
      ];
      replace_labels_for_corporation(&db, CORP_ID, &labels).await.unwrap();

      let fetched = corporation_contact_labels(&db, CORP_ID).await.unwrap();

      assert_eq!(fetched.iter().map(|l| l.label_id()).collect::<Vec<_>>(), vec![10, 20]);
    }
  }

  mod corporation_contacts_page {
    use pretty_assertions::assert_eq;

    use super::*;

    const CORP_ID: i64 = 5002;

    fn contact(id: i64, kind: &str, standing: f64, name: &str) -> CorporationContact {
      CorporationContact {
        contact_id: id,
        contact_name: name.to_owned(),
        contact_type: kind.to_owned(),
        corporation_id: CORP_ID,
        is_blocked: false,
        is_watched: false,
        label_ids: "[]".to_owned(),
        standing,
      }
    }

    async fn seed_contacts(db: &store::Database) {
      seed_character(db, CORP_ID, 8002).await;
      let contacts = vec![
        contact(100, "character", 8.5, "Wingmate"),
        contact(200, "corporation", -5.0, "Hostile Corp"),
        contact(300, "alliance", 0.0, "Neutral Alliance"),
      ];
      replace_contacts_for_corporation(db, CORP_ID, &contacts).await.unwrap();
    }

    fn ids(rows: &[CorporationContact]) -> Vec<i64> {
      rows.iter().map(|row| row.contact_id()).collect()
    }

    #[tokio::test]
    async fn it_filters_by_contact_type() {
      let db = store::open_test().await.unwrap();
      seed_contacts(&db).await;

      let rows = corporation_contacts_page(
        &db,
        CORP_ID,
        Some("corporation"),
        None,
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        None,
        100,
      )
      .await
      .unwrap();

      assert_eq!(ids(&rows), vec![200]);
    }

    #[tokio::test]
    async fn it_matches_a_name_query() {
      let db = store::open_test().await.unwrap();
      seed_contacts(&db).await;

      let rows = corporation_contacts_page(
        &db,
        CORP_ID,
        None,
        Some("wing"),
        ContactSortColumn::Name,
        ContactSortDir::Asc,
        None,
        100,
      )
      .await
      .unwrap();

      assert_eq!(ids(&rows), vec![100]);
    }

    #[tokio::test]
    async fn it_orders_by_standing_descending() {
      let db = store::open_test().await.unwrap();
      seed_contacts(&db).await;

      let rows = corporation_contacts_page(
        &db,
        CORP_ID,
        None,
        None,
        ContactSortColumn::Standing,
        ContactSortDir::Desc,
        None,
        100,
      )
      .await
      .unwrap();

      assert_eq!(ids(&rows), vec![100, 300, 200]);
    }

    #[tokio::test]
    async fn it_pages_through_contacts_without_overlap_via_the_cursor() {
      let db = store::open_test().await.unwrap();
      seed_contacts(&db).await;

      let first = corporation_contacts_page(
        &db,
        CORP_ID,
        None,
        None,
        ContactSortColumn::Standing,
        ContactSortDir::Desc,
        None,
        2,
      )
      .await
      .unwrap();
      assert_eq!(ids(&first), vec![100, 300]);

      let last = first.last().unwrap();
      let cursor = ContactCursor::Number(last.standing(), last.contact_id());
      let second = corporation_contacts_page(
        &db,
        CORP_ID,
        None,
        None,
        ContactSortColumn::Standing,
        ContactSortDir::Desc,
        Some(&cursor),
        2,
      )
      .await
      .unwrap();

      assert_eq!(ids(&second), vec![200]);
    }
  }

  mod corporation_killmails_page {
    use pretty_assertions::assert_eq;

    use super::*;

    const CORP_ID: i64 = 4001;

    fn kill(killmail_id: i64, kill_time: &str) -> CorporationKillEntry {
      CorporationKillEntry {
        attacker_count: 1,
        corporation_id: CORP_ID,
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

    fn ids(rows: &[CorporationKillEntry]) -> Vec<i64> {
      rows.iter().map(|row| row.killmail_id()).collect()
    }

    async fn seed_kills(db: &store::Database) {
      seed_character(db, CORP_ID, 7001).await;

      upsert_corporation_killmail(db, &kill(100, "2024-03-01T00:00:00Z"))
        .await
        .unwrap();
      upsert_corporation_killmail(db, &kill(101, "2024-02-01T00:00:00Z"))
        .await
        .unwrap();
      upsert_corporation_killmail(db, &kill(102, "2024-02-01T00:00:00Z"))
        .await
        .unwrap();
      upsert_corporation_killmail(db, &kill(103, "2024-01-01T00:00:00Z"))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_orders_by_kill_time_then_killmail_id_descending() {
      let db = store::open_test().await.unwrap();
      seed_kills(&db).await;

      let rows = corporation_killmails_page(&db, CORP_ID, None, 100).await.unwrap();

      assert_eq!(ids(&rows), vec![100, 102, 101, 103]);
    }

    #[tokio::test]
    async fn it_pages_through_kills_without_overlap_via_the_cursor() {
      let db = store::open_test().await.unwrap();
      seed_kills(&db).await;

      let first = corporation_killmails_page(&db, CORP_ID, None, 2).await.unwrap();
      assert_eq!(ids(&first), vec![100, 102]);

      let last = first.last().unwrap();
      let cursor = Some((last.kill_time().clone(), last.killmail_id()));
      let second = corporation_killmails_page(&db, CORP_ID, cursor, 2).await.unwrap();

      assert_eq!(ids(&second), vec![101, 103]);
    }

    #[tokio::test]
    async fn it_returns_an_empty_page_past_the_last_kill() {
      let db = store::open_test().await.unwrap();
      seed_kills(&db).await;

      let beyond = corporation_killmails_page(&db, CORP_ID, Some(("2023-01-01T00:00:00Z".to_owned(), 0)), 10)
        .await
        .unwrap();

      assert!(beyond.is_empty());
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

    // Test seeder whose arguments mirror the owned-corporation columns.
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
    async fn it_ands_multiple_tokens() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "cobalt tag:mining").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "cobalt tag:pvp").await, Vec::<i64>::new());
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

    #[tokio::test]
    async fn it_matches_a_quoted_phrase() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "\"jita trade hub\"").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "tag:\"Mining\"").await, vec![COBALT_INDUSTRIES]);
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
    async fn it_matches_free_text_on_a_tag_name() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "mining").await, vec![COBALT_INDUSTRIES]);
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
    async fn it_matches_free_text_on_corp_name_or_ticker() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "cobalt").await, vec![COBALT_INDUSTRIES]);
      assert_eq!(matching(&db, "reds").await, vec![RED_SYNDICATE]);
    }

    #[tokio::test]
    async fn it_matches_free_text_on_hq_station_name() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "trade hub").await, vec![COBALT_INDUSTRIES]);
    }

    #[tokio::test]
    async fn it_negates_a_tag_filter() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "-tag:pvp").await, vec![COBALT_INDUSTRIES]);
    }

    #[tokio::test]
    async fn it_returns_all_owned_corps_for_an_empty_query() {
      let db = store::open_test().await.unwrap();
      seed_owned_corps(&db).await;

      assert_eq!(matching(&db, "").await, vec![COBALT_INDUSTRIES, RED_SYNDICATE]);
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
    async fn it_excludes_reference_corporations_without_a_credential() {
      let db = store::open_test().await.unwrap();
      seed_owned_character(&db, 2001, 8001).await;

      let result = all_owned_corporations(&db).await.unwrap();

      assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn it_returns_empty_vec_when_no_corporations_are_owned() {
      let db = store::open_test().await.unwrap();

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
    async fn it_rejects_a_corp_with_no_credential() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Director")])
        .await
        .unwrap();

      assert!(!corp_is_authorized(&db, CORP_ID).await.unwrap());
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

  mod replace_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_leaves_existing_roles_untouched_for_an_empty_slice() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Director")])
        .await
        .unwrap();

      replace_for_corporation(&db, CORP_ID, &[]).await.unwrap();

      let rows = for_corporation(&db, CORP_ID).await.unwrap();
      assert_eq!(rows, vec![role(CORP_ID, DIRECTOR_ID, "Director")]);
    }

    #[tokio::test]
    async fn it_replaces_only_the_named_character_without_clobbering_others() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, DIRECTOR_ID, "Director")])
        .await
        .unwrap();
      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, 999, "Station_Manager")])
        .await
        .unwrap();

      replace_for_corporation(&db, CORP_ID, &[role(CORP_ID, 999, "Accountant")])
        .await
        .unwrap();

      let rows = for_corporation(&db, CORP_ID).await.unwrap();
      assert_eq!(
        rows,
        vec![role(CORP_ID, DIRECTOR_ID, "Director"), role(CORP_ID, 999, "Accountant"),]
      );
    }

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
  }
}

#[cfg(test)]
mod mining_extraction_tests {
  use super::*;
  use crate::store::{
    self,
    model::{Constellation, Corporation, Moon, Region, SolarSystem},
    repo::{org, sde},
  };

  const CORP_ID: i64 = 90_000_001;

  fn extraction(corporation_id: i64, structure_id: i64, moon_id: i64) -> CorporationMiningExtraction {
    CorporationMiningExtraction {
      chunk_arrival_time: Some("2026-06-20T00:00:00Z".to_owned()),
      corporation_id,
      extraction_start_time: Some("2026-06-13T00:00:00Z".to_owned()),
      moon_id,
      moon_name: None,
      natural_decay_time: Some("2026-06-21T00:00:00Z".to_owned()),
      security_status: None,
      solar_system_id: None,
      structure_id,
    }
  }

  async fn seed_corp(db: &store::Database) {
    let mut corp = Corporation::new(CORP_ID, "Test Corp", "TST");
    corp.set_ceo_id(100);
    corp.set_creator_id(100);
    corp.set_member_count(1);
    corp.set_tax_rate(0.1);
    org::upsert_corporation(db, &corp).await.unwrap();
  }

  async fn seed_moon(db: &store::Database, moon_id: i64, solar_system_id: i64) {
    sde::upsert_region(
      db,
      &Region {
        description: None,
        id: 10_000_001,
        name: "Test Region".to_owned(),
      },
    )
    .await
    .unwrap();
    sde::upsert_constellation(
      db,
      &Constellation {
        id: 20_000_001,
        name: "Test Constellation".to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        region_id: 10_000_001,
      },
    )
    .await
    .unwrap();
    sde::upsert_solar_system(
      db,
      &SolarSystem {
        constellation_id: 20_000_001,
        id: solar_system_id,
        name: "Test System".to_owned(),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        security_class: None,
        security_status: 0.5,
        star_id: None,
      },
    )
    .await
    .unwrap();
    sde::upsert_many_moons(
      db,
      &[Moon {
        id: moon_id,
        name: "Test System I - Moon 1".to_owned(),
        orbit_index: Some(1),
        planet_id: Some(40_000_001),
        position_x: 0.0,
        position_y: 0.0,
        position_z: 0.0,
        radius: None,
        solar_system_id,
        type_id: Some(14),
      }],
    )
    .await
    .unwrap();
  }

  mod corporation_mining_extractions {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_joins_the_moon_name_system_and_security_status() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      seed_moon(&db, 40_000_001, 30_000_001).await;
      replace_extractions_for_corporation(&db, CORP_ID, &[extraction(CORP_ID, 1_021_000_000_001, 40_000_001)])
        .await
        .unwrap();

      let rows = corporation_mining_extractions(&db, CORP_ID).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].moon_name().as_deref(), Some("Test System I - Moon 1"));
      assert_eq!(rows[0].solar_system_id(), Some(30_000_001));
      assert_eq!(rows[0].security_status(), Some(0.5));
    }

    #[tokio::test]
    async fn it_returns_a_row_with_null_geo_when_the_moon_is_not_in_the_sde() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_extractions_for_corporation(&db, CORP_ID, &[extraction(CORP_ID, 1_021_000_000_001, 40_000_999)])
        .await
        .unwrap();

      let rows = corporation_mining_extractions(&db, CORP_ID).await.unwrap();

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].moon_name(), &None);
      assert_eq!(rows[0].moon_id(), 40_000_999);
    }
  }

  mod replace_extractions_for_corporation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_clears_extractions_with_an_empty_slice() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_extractions_for_corporation(&db, CORP_ID, &[extraction(CORP_ID, 1_021_000_000_001, 40_000_001)])
        .await
        .unwrap();

      replace_extractions_for_corporation(&db, CORP_ID, &[]).await.unwrap();

      assert!(corporation_mining_extractions(&db, CORP_ID).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_full_replaces_the_prior_extraction_set() {
      let db = store::open_test().await.unwrap();
      seed_corp(&db).await;
      replace_extractions_for_corporation(&db, CORP_ID, &[extraction(CORP_ID, 1_021_000_000_001, 40_000_001)])
        .await
        .unwrap();

      replace_extractions_for_corporation(&db, CORP_ID, &[extraction(CORP_ID, 1_021_000_000_002, 40_000_002)])
        .await
        .unwrap();

      let rows = corporation_mining_extractions(&db, CORP_ID).await.unwrap();
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].structure_id(), 1_021_000_000_002);
    }
  }
}
