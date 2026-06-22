use crate::{
  clients::eve_image::Size,
  features::killmail_detail::{AttackerView, EntityView, ItemView, KillmailDetail, SlotGroupView},
  store::{
    Database, images,
    killmail_slot::SlotGroup,
    model::{CorporationKillEntry, CorporationKillmailAttacker},
    repo::{character, org, sde},
  },
};

const ITEM_ICON_SIZE: Size = Size::S64;

pub async fn load(db: &Database, corporation_id: i64, killmail_id: i64) -> Option<KillmailDetail> {
  let rows = org::corporation_killmails(db, corporation_id).await.ok()?;
  let row = rows.into_iter().find(|row| row.killmail_id() == killmail_id)?;

  let ship_name = type_name(db, row.ship_type_id()).await;
  let ship_icon = images::default_store().resolve_type_icon(row.ship_type_id(), None, ITEM_ICON_SIZE);

  let (system_name, system_security) = match sde::get_solar_system(db, row.system_id()).await.ok().flatten() {
    Some(system) => (Some(system.name().clone()), system.security_status()),
    None => (None, 0.0),
  };

  let victim_name = victim_name(db, row.victim_id()).await;
  let victim_portrait = victim_portrait(row.victim_id());
  let victim_corp = corporation_view(db, row.victim_corp_id()).await;
  let victim_alliance = alliance_view(db, row.victim_alliance_id()).await;

  let slots = load_slots(db, corporation_id, killmail_id).await;
  let attackers = load_attackers(db, corporation_id, killmail_id).await;

  Some(KillmailDetail {
    attackers,
    damage_taken: row.victim_damage_taken(),
    dropped_isk: dropped_isk(&row),
    is_kill: row.is_kill(),
    kill_time: row.kill_time().clone(),
    killmail_id,
    ship_icon,
    ship_name,
    slots,
    system_name,
    system_security,
    value_destroyed_isk: row.value_destroyed_isk(),
    value_isk: row.value_isk(),
    victim_alliance,
    victim_corp,
    victim_name,
    victim_portrait,
  })
}

fn dropped_isk(row: &CorporationKillEntry) -> f64 {
  (row.value_isk() - row.value_destroyed_isk()).max(0.0)
}

fn victim_portrait(victim_id: Option<i64>) -> images::ImageState {
  match victim_id {
    Some(id) => images::resolve(&images::default_store(), images::ImageKind::CharacterPortrait, id),
    None => images::ImageState::Stale {
      id: 0,
      kind: images::ImageKind::CharacterPortrait,
    },
  }
}

async fn alliance_view(db: &Database, alliance_id: Option<i64>) -> Option<EntityView> {
  let id = alliance_id?;
  let name = org::get_alliance(db, id)
    .await
    .ok()
    .flatten()
    .map(|alliance| alliance.name().clone())
    .unwrap_or_else(|| format!("Alliance {id}"));
  Some(EntityView {
    logo: images::resolve(&images::default_store(), images::ImageKind::AllianceLogo, id),
    name,
  })
}

async fn character_name(db: &Database, id: i64) -> String {
  character::get(db, id)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().to_owned())
    .unwrap_or_else(|| format!("Pilot {id}"))
}

async fn corporation_name(db: &Database, id: i64) -> String {
  org::get_corporation(db, id)
    .await
    .ok()
    .flatten()
    .map(|corp| corp.name().to_owned())
    .unwrap_or_else(|| format!("Corp {id}"))
}

async fn corporation_view(db: &Database, corp_id: Option<i64>) -> Option<EntityView> {
  let id = corp_id?;
  Some(EntityView {
    logo: images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, id),
    name: corporation_name(db, id).await,
  })
}

async fn load_attackers(db: &Database, corporation_id: i64, killmail_id: i64) -> Vec<AttackerView> {
  let rows = org::corporation_killmail_attackers(db, corporation_id, killmail_id)
    .await
    .unwrap_or_default();
  let total_damage: f64 = rows.iter().map(|row| row.damage_done() as f64).sum();

  let mut attackers = Vec::with_capacity(rows.len());
  for row in &rows {
    attackers.push(attacker_view(db, row, total_damage).await);
  }

  attackers.sort_by(|a, b| {
    b.final_blow
      .cmp(&a.final_blow)
      .then(b.damage_share.total_cmp(&a.damage_share))
  });
  attackers
}

async fn attacker_view(db: &Database, row: &CorporationKillmailAttacker, total_damage: f64) -> AttackerView {
  let name = match row.attacker_character_id() {
    Some(id) => character_name(db, id).await,
    None => "Unknown".to_owned(),
  };
  let corp_name = match row.attacker_corporation_id() {
    Some(id) => corporation_name(db, id).await,
    None => String::new(),
  };
  let (ship_name, ship_icon) = attacker_ship(db, row.ship_type_id()).await;
  let damage_share = if total_damage > 0.0 {
    row.damage_done() as f64 / total_damage
  } else {
    0.0
  };

  AttackerView {
    corp_name,
    damage_share,
    final_blow: row.final_blow(),
    is_self: false,
    name,
    ship_icon,
    ship_name,
  }
}

async fn attacker_ship(db: &Database, ship_type_id: Option<i64>) -> (String, images::IconResolution) {
  match ship_type_id {
    Some(type_id) => (
      type_name(db, type_id).await,
      images::default_store().resolve_type_icon(type_id, None, ITEM_ICON_SIZE),
    ),
    None => ("Unknown".to_owned(), images::IconResolution::Missing),
  }
}

async fn load_slots(db: &Database, corporation_id: i64, killmail_id: i64) -> Vec<SlotGroupView> {
  let rows = org::corporation_killmail_items(db, corporation_id, killmail_id)
    .await
    .unwrap_or_default();

  let mut groups: Vec<SlotGroupView> = Vec::new();
  for &group in SlotGroup::display_order() {
    let mut items = Vec::new();
    for row in rows.iter().filter(|row| SlotGroup::from_flag(row.flag()) == group) {
      // An entry is flagged green (dropped) when any of its stack survived, red otherwise; the displayed count is the
      // whole stack so a partially-looted stack still reads honestly.
      items.push(ItemView {
        dropped: row.quantity_dropped() > 0,
        icon: images::default_store().resolve_type_icon(row.type_id(), None, ITEM_ICON_SIZE),
        name: type_name(db, row.type_id()).await,
        quantity: row.quantity_destroyed() + row.quantity_dropped(),
        value_isk: row.value_isk(),
      });
    }
    if !items.is_empty() {
      groups.push(SlotGroupView {
        items,
        label: group.label(),
      });
    }
  }
  groups
}

async fn type_name(db: &Database, type_id: i64) -> String {
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
    .unwrap_or_else(|| format!("Type {type_id}"))
}

async fn victim_name(db: &Database, victim_id: Option<i64>) -> String {
  match victim_id {
    Some(id) => character_name(db, id).await,
    None => "Unknown".to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
  };

  const CEO_ID: i64 = 7001;

  const CORP_ID: i64 = 98_000_001;

  const KILLMAIL_ID: i64 = 555;

  async fn seed_corporation(db: &Database) {
    let alliance = Alliance::new(CORP_ID, CORP_ID, CEO_ID, "2020-01-01", "Test Alliance", "TST");
    let mut corp = Corporation::new(CORP_ID, "Cobalt Syndicate", "COBSY");
    corp.set_ceo_id(CEO_ID);
    corp.set_creator_id(CEO_ID);
    corp.set_member_count(100);
    corp.set_tax_rate(0.05);
    let race = Race::new(1, CORP_ID, "A race.", "Test Race");
    let bloodline = Bloodline::new(1, CORP_ID, 1, 3, "A bloodline.", 7, 5, "Test", 6, 4);
    let char = Character::new(CEO_ID, 1, CORP_ID, 1, "1990-01-01", Gender::Male, "Test CEO");
    character::insert_with_org(db, &char, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn kill_entry() -> CorporationKillEntry {
    CorporationKillEntry {
      attacker_count: 2,
      corporation_id: CORP_ID,
      final_blow: true,
      is_kill: true,
      kill_hash: "hash555".to_owned(),
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id: KILLMAIL_ID,
      ship_type_id: 587,
      synced_at: "2024-01-02T00:00:00Z".to_owned(),
      system_id: 30_000_142,
      value_destroyed_isk: 100.0,
      value_final: false,
      value_isk: 250.0,
      value_recheck_count: 0,
      value_source: "local".to_owned(),
      victim_alliance_id: None,
      victim_corp_id: None,
      victim_damage_taken: 42,
      victim_id: None,
    }
  }

  fn attacker(
    ordinal: i64,
    character_id: Option<i64>,
    ship_type_id: Option<i64>,
    damage: i64,
    final_blow: bool,
  ) -> CorporationKillmailAttacker {
    CorporationKillmailAttacker {
      alliance_id: None,
      attacker_character_id: character_id,
      attacker_corporation_id: None,
      corporation_id: CORP_ID,
      damage_done: damage,
      final_blow,
      killmail_id: KILLMAIL_ID,
      ordinal,
      ship_type_id,
    }
  }

  mod load {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assembles_a_detail_from_the_stored_killmail() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db).await;
      org::upsert_corporation_killmail(&db, &kill_entry()).await.unwrap();

      let detail = load(&db, CORP_ID, KILLMAIL_ID).await.unwrap();

      assert_eq!(detail.killmail_id, KILLMAIL_ID);
      assert_eq!(detail.ship_name, "Type 587");
      assert_eq!(detail.system_name, None);
      assert_eq!(detail.victim_name, "Unknown");
      assert_eq!(detail.dropped_isk, 150.0);
    }

    #[tokio::test]
    async fn it_returns_none_when_the_killmail_is_absent() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db).await;

      let detail = load(&db, CORP_ID, KILLMAIL_ID).await;

      assert!(detail.is_none());
    }
  }

  mod load_attackers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_is_empty_when_no_attackers_are_stored() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db).await;

      let attackers = load_attackers(&db, CORP_ID, KILLMAIL_ID).await;

      assert!(attackers.is_empty());
    }

    #[tokio::test]
    async fn it_orders_final_blow_first_then_by_damage_share() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db).await;
      org::upsert_corporation_killmail(&db, &kill_entry()).await.unwrap();
      org::upsert_corporation_killmail_detail(
        &db,
        CORP_ID,
        KILLMAIL_ID,
        &[
          attacker(0, Some(2001), Some(587), 100, false),
          attacker(1, None, None, 300, true),
        ],
        &[],
      )
      .await
      .unwrap();

      let attackers = load_attackers(&db, CORP_ID, KILLMAIL_ID).await;

      assert_eq!(attackers.len(), 2);
      assert!(attackers[0].final_blow);
      assert_eq!(attackers[0].name, "Unknown");
      assert_eq!(attackers[1].name, "Pilot 2001");
      assert_eq!(attackers[1].ship_name, "Type 587");
    }

    #[tokio::test]
    async fn it_yields_zero_damage_share_when_total_damage_is_zero() {
      let db = store::open_test().await.unwrap();
      seed_corporation(&db).await;
      org::upsert_corporation_killmail(&db, &kill_entry()).await.unwrap();
      org::upsert_corporation_killmail_detail(&db, CORP_ID, KILLMAIL_ID, &[attacker(0, None, None, 0, true)], &[])
        .await
        .unwrap();

      let attackers = load_attackers(&db, CORP_ID, KILLMAIL_ID).await;

      assert_eq!(attackers[0].damage_share, 0.0);
    }
  }
}
