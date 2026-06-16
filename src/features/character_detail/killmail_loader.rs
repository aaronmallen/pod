use crate::{
  clients::eve_image::Size,
  features::killmail_detail::{AttackerView, EntityView, ItemView, KillmailDetail, SlotGroupView},
  store::{
    Database, images,
    killmail_slot::SlotGroup,
    model::CharacterKillEntry,
    repo::{character, org, sde},
  },
};

const ITEM_ICON_SIZE: Size = Size::S64;

/// `viewing_character_id` only flags the matching attacker row as `is_self`; `character_id` scopes every query.
pub(super) async fn load(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
  viewing_character_id: i64,
) -> Option<KillmailDetail> {
  let rows = character::killmails(db, character_id).await.ok()?;
  let row = rows.into_iter().find(|row| row.killmail_id() == killmail_id)?;

  let ship_name = type_name(db, row.ship_type_id()).await;
  let ship_icon = images::default_store().resolve_type_icon(row.ship_type_id(), None, ITEM_ICON_SIZE);

  let (system_name, system_security) = match sde::get_solar_system(db, row.system_id()).await.ok().flatten() {
    Some(system) => (Some(system.name().clone()), system.security_status()),
    None => (None, 0.0),
  };

  let victim_name = victim_name(db, row.victim_id()).await;
  let victim_portrait = match row.victim_id() {
    Some(id) => images::resolve(&images::default_store(), images::ImageKind::CharacterPortrait, id),
    None => images::ImageState::Stale {
      id: 0,
      kind: images::ImageKind::CharacterPortrait,
    },
  };
  let victim_corp = corporation_view(db, row.victim_corp_id()).await;
  let victim_alliance = alliance_view(db, row.victim_alliance_id()).await;

  let slots = load_slots(db, character_id, killmail_id).await;
  let attackers = load_attackers(db, character_id, killmail_id, viewing_character_id).await;

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

fn dropped_isk(row: &CharacterKillEntry) -> f64 {
  (row.value_isk() - row.value_destroyed_isk()).max(0.0)
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

async fn load_attackers(
  db: &Database,
  character_id: i64,
  killmail_id: i64,
  viewing_character_id: i64,
) -> Vec<AttackerView> {
  let rows = character::killmail_attackers(db, character_id, killmail_id)
    .await
    .unwrap_or_default();
  let total_damage: f64 = rows.iter().map(|row| row.damage_done() as f64).sum();

  let mut attackers = Vec::with_capacity(rows.len());
  for row in &rows {
    let name = match row.attacker_character_id() {
      Some(id) => character_name(db, id).await,
      None => "Unknown".to_owned(),
    };
    let corp_name = match row.corporation_id() {
      Some(id) => corporation_name(db, id).await,
      None => String::new(),
    };
    let (ship_name, ship_icon) = match row.ship_type_id() {
      Some(type_id) => (
        type_name(db, type_id).await,
        images::default_store().resolve_type_icon(type_id, None, ITEM_ICON_SIZE),
      ),
      None => ("Unknown".to_owned(), images::IconResolution::Missing),
    };
    let damage_share = if total_damage > 0.0 {
      row.damage_done() as f64 / total_damage
    } else {
      0.0
    };

    attackers.push(AttackerView {
      corp_name,
      damage_share,
      final_blow: row.final_blow(),
      is_self: row.attacker_character_id() == Some(viewing_character_id),
      name,
      ship_icon,
      ship_name,
    });
  }

  attackers.sort_by(|a, b| {
    b.final_blow
      .cmp(&a.final_blow)
      .then(b.damage_share.total_cmp(&a.damage_share))
  });
  attackers
}

async fn load_slots(db: &Database, character_id: i64, killmail_id: i64) -> Vec<SlotGroupView> {
  let rows = character::killmail_items(db, character_id, killmail_id)
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
