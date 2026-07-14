use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};

use crate::{
  clients::{self, esi, esi::models::market::RegionOrder, http},
  config::FeatureFlags,
  features::market::{outbid, watch_eval},
  store::{
    Database,
    model::{
      MarketAlertKind, MarketOrder, MarketWatch, NewNotification, Notification, NotificationDestination,
      NotificationKind, NotificationOwner, NotificationTarget, WatchDirection,
    },
    repo::{
      calendar, character, finance, industry, mail, market_alert_state, market_watchlist, notifications, org, sde,
      skill_completion,
    },
  },
  sync::JobKind,
};

const LIST_LIMIT: i64 = 200;

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
  pub list: Vec<Notification>,
  pub outbid: i64,
  pub surfaced: Vec<Notification>,
  pub unread: i64,
  pub who: HashMap<NotificationOwner, String>,
}

pub async fn refresh(
  db: &Database,
  now: DateTime<Utc>,
  characters: &[i64],
  corporations: &[i64],
  features: &FeatureFlags,
  run_detectors: bool,
) -> Snapshot {
  let surfaced = if run_detectors {
    detect(db, now, characters, corporations, features).await
  } else {
    Vec::new()
  };

  let list = notifications::list(db, LIST_LIMIT).await.unwrap_or_default();
  let unread = notifications::unread_count(db).await.unwrap_or(0);
  let outbid = market_alert_state::count_alerted(db, MarketAlertKind::Outbid)
    .await
    .unwrap_or(0);
  let who = resolve_owner_names(db, &list, &surfaced).await;

  Snapshot {
    list,
    outbid,
    surfaced,
    unread,
    who,
  }
}

pub async fn detect(
  db: &Database,
  now: DateTime<Utc>,
  characters: &[i64],
  corporations: &[i64],
  features: &FeatureFlags,
) -> Vec<Notification> {
  let mut surfaced = Vec::new();
  for &character_id in characters {
    run(mail_detector(db, character_id, features), "mail", &mut surfaced).await;
    run(calendar_detector(db, character_id, features), "calendar", &mut surfaced).await;
    run(
      killmail_detector(db, NotificationOwner::Character(character_id), features),
      "killmail",
      &mut surfaced,
    )
    .await;
    run(skill_detector(db, character_id, now, features), "skill", &mut surfaced).await;
    run(
      industry_detector(db, NotificationOwner::Character(character_id), features),
      "industry",
      &mut surfaced,
    )
    .await;
    run(outbid_detector(db, character_id, features), "outbid", &mut surfaced).await;
    run(
      watchlist_target_detector(db, character_id, features),
      "watchlist target",
      &mut surfaced,
    )
    .await;
  }
  for &corporation_id in corporations {
    run(
      killmail_detector(db, NotificationOwner::Corporation(corporation_id), features),
      "corp killmail",
      &mut surfaced,
    )
    .await;
    run(
      industry_detector(db, NotificationOwner::Corporation(corporation_id), features),
      "corp industry",
      &mut surfaced,
    )
    .await;
    run(
      extraction_scheduled_detector(db, corporation_id, features),
      "extraction scheduled",
      &mut surfaced,
    )
    .await;
    run(
      extraction_cracked_detector(db, corporation_id, now, features),
      "extraction cracked",
      &mut surfaced,
    )
    .await;
  }
  surfaced
}

async fn run(
  detector: impl std::future::Future<Output = Result<Vec<Notification>, crate::store::Error>>,
  label: &str,
  surfaced: &mut Vec<Notification>,
) {
  match detector.await {
    Ok(mut emitted) => surfaced.append(&mut emitted),
    Err(error) => {
      tracing::warn!(target: "pod::notifications", %error, detector = label, "notification detector failed");
    }
  }
}

async fn resolve_owner_names(
  db: &Database,
  list: &[Notification],
  surfaced: &[Notification],
) -> HashMap<NotificationOwner, String> {
  let mut names = HashMap::new();
  resolve_into(db, list, &mut names).await;
  resolve_into(db, surfaced, &mut names).await;
  names
}

pub async fn resolve_names(db: &Database, notifications: &[Notification]) -> HashMap<NotificationOwner, String> {
  let mut names = HashMap::new();
  resolve_into(db, notifications, &mut names).await;
  names
}

async fn resolve_into(db: &Database, notifications: &[Notification], names: &mut HashMap<NotificationOwner, String>) {
  for owner in notifications.iter().map(Notification::owner) {
    if names.contains_key(&owner) {
      continue;
    }
    let name = match owner {
      NotificationOwner::Character(id) => character_name(db, id).await,
      NotificationOwner::Corporation(id) => corporation_name(db, id).await,
    };
    names.insert(owner, name);
  }
}

async fn character_name(db: &Database, character_id: i64) -> String {
  character::get(db, character_id)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().clone())
    .unwrap_or_default()
}

/// Whether this is the first scan of `(owner, kind)`: the notifications table holds no row — surfaced or
/// suppressed watermark — for it yet. Deliberately independent of the sync ledger: the sync engine writes
/// the ledger's `last_success_at` BEFORE the detector pulse, so a ledger-based check reads false on the
/// very first sync and would flood the whole pre-existing history. On a true first scan the caller
/// watermarks the current items (and a sentinel) so later passes only surface genuinely new items.
async fn is_first_scan(
  db: &Database,
  owner: NotificationOwner,
  kind: NotificationKind,
) -> Result<bool, crate::store::Error> {
  Ok(!notifications::has_any(db, &owner, kind).await?)
}

async fn corporation_name(db: &Database, corporation_id: i64) -> String {
  org::get_corporation(db, corporation_id)
    .await
    .ok()
    .flatten()
    .map(|corporation| corporation.name().clone())
    .unwrap_or_default()
}

async fn type_name(db: &Database, type_id: i64) -> Option<String> {
  sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item| item.name().clone())
}

async fn mail_detector(
  db: &Database,
  character_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterMail.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::Mail).await?;
  let headers = mail::headers(db, character_id).await?;
  // Default: received mail not authored by the owner (drop self-sent copies in the Sent box).
  let received: Vec<_> = headers.into_iter().filter(|m| m.from_id() != character_id).collect();
  let watermarks: Vec<String> = received.iter().map(|m| mail_key(character_id, m.mail_id())).collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Mail, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for header in received {
    let title = header
      .subject()
      .clone()
      .filter(|s| !s.is_empty())
      .unwrap_or_else(|| t!("shell.notification.mail_title_fallback").into_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.mail_body", name => header.from_name()).into_owned(),
        dedup_key: mail_key(character_id, header.mail_id()),
        kind: NotificationKind::Mail,
        owner: NotificationOwner::Character(character_id),
        target: NotificationTarget {
          character: Some(character_id),
          destination: NotificationDestination::Mail,
          sub: None,
        },
        title,
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn calendar_detector(
  db: &Database,
  character_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterCalendar.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::Calendar).await?;
  // The table holds only real ESI events; Pod-derived overlays live in memory, so reading the table
  // already excludes synthetic ones.
  let events = calendar::events(db, character_id).await?;
  let watermarks: Vec<String> = events
    .iter()
    .map(|e| calendar_key(character_id, e.event_id()))
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Calendar, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for event in events {
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: event.owner_name().clone(),
        dedup_key: calendar_key(character_id, event.event_id()),
        kind: NotificationKind::Calendar,
        owner: NotificationOwner::Character(character_id),
        target: NotificationTarget {
          character: Some(character_id),
          destination: NotificationDestination::Calendar,
          sub: None,
        },
        title: event.title().clone(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn killmail_detector(
  db: &Database,
  owner: NotificationOwner,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  let job = match owner {
    NotificationOwner::Character(_) => JobKind::CharacterKillmails,
    NotificationOwner::Corporation(_) => JobKind::CorporationKillmails,
  };
  if !job.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let first = is_first_scan(db, owner, NotificationKind::Killmail).await?;
  let rows: Vec<(i64, i64)> = match owner {
    NotificationOwner::Character(id) => character::killmails(db, id)
      .await?
      .into_iter()
      .map(|k| (k.killmail_id(), k.ship_type_id()))
      .collect(),
    NotificationOwner::Corporation(id) => org::corporation_killmails(db, id)
      .await?
      .into_iter()
      .map(|k| (k.killmail_id(), k.ship_type_id()))
      .collect(),
  };
  let watermarks: Vec<String> = rows.iter().map(|(killmail_id, _)| killmail_key(*killmail_id)).collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Killmail, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (killmail_id, ship_type_id) in rows {
    let ship = type_name(db, ship_type_id)
      .await
      .unwrap_or_else(|| t!("shell.notification.killmail_ship_fallback").into_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.killmail_destroyed", ship => ship).into_owned(),
        dedup_key: killmail_key(killmail_id),
        kind: NotificationKind::Killmail,
        owner,
        target: NotificationTarget::killmail(owner, killmail_id),
        title: t!("shell.notification.killmail_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn skill_detector(
  db: &Database,
  character_id: i64,
  now: DateTime<Utc>,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterSkills.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::Skill).await?;
  // A finished skill leaves no completion row; "new" is a finish_date crossing wall-clock. Scan the
  // whole queue so a multi-skill burst that matured while idle each fires once.
  let now_rfc = now.to_rfc3339();
  let matured: Vec<(i64, i64, String)> = character::skillqueue(db, character_id)
    .await?
    .into_iter()
    .filter_map(|entry| {
      entry
        .finish_date()
        .clone()
        .map(|finish| (entry.skill_id(), entry.finished_level(), finish))
    })
    .filter(|(_, _, finish)| crossed(finish, &now_rfc))
    .collect();
  let watermarks: Vec<String> = matured
    .iter()
    .map(|(skill_id, _, finish)| skill_key(character_id, *skill_id, finish))
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Skill, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (skill_id, level, finish) in matured {
    capture_completion(db, character_id, skill_id, level, &finish).await?;
    let skill = type_name(db, skill_id)
      .await
      .unwrap_or_else(|| t!("shell.notification.skill_fallback").into_owned());
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.skill_body").into_owned(),
        dedup_key: skill_key(character_id, skill_id, &finish),
        kind: NotificationKind::Skill,
        owner: NotificationOwner::Character(character_id),
        target: NotificationTarget {
          character: Some(character_id),
          destination: NotificationDestination::Skills,
          sub: None,
        },
        title: t!("shell.notification.skill_title", skill => skill).into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

/// Called once per matured entry, independent of whether the paired notification is deduped.
///
/// `insert_if_absent` keeps repeated calls idempotent across reruns; entries watermarked on the
/// first scan never reach this function, so pre-existing completion history is deliberately not
/// backfilled.
async fn capture_completion(
  db: &Database,
  character_id: i64,
  skill_id: i64,
  level: i64,
  completed_at: &str,
) -> Result<(), crate::store::Error> {
  skill_completion::insert_if_absent(db, character_id, skill_id, level, completed_at).await?;
  Ok(())
}

async fn industry_detector(
  db: &Database,
  owner: NotificationOwner,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  let (job, subject_id) = match owner {
    NotificationOwner::Character(id) => (JobKind::CharacterIndustryJobs, id),
    NotificationOwner::Corporation(id) => (JobKind::CorporationIndustryJobs, id),
  };
  if !job.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let first = is_first_scan(db, owner, NotificationKind::Industry).await?;
  let jobs: Vec<(i64, Option<i64>)> = match owner {
    NotificationOwner::Character(id) => industry::list_for_character(db, id)
      .await?
      .into_iter()
      .filter(|j| is_industry_done(j.status()))
      .map(|j| (j.job_id(), j.product_type_id()))
      .collect(),
    NotificationOwner::Corporation(id) => industry::list_for_corporation(db, id)
      .await?
      .into_iter()
      .filter(|j| is_industry_done(j.status()))
      .map(|j| (j.job_id(), j.product_type_id()))
      .collect(),
  };
  let watermarks: Vec<String> = jobs.iter().map(|(job_id, _)| industry_key(*job_id)).collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::Industry, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (job_id, product_type_id) in jobs {
    let product = match product_type_id {
      Some(id) => type_name(db, id)
        .await
        .unwrap_or_else(|| t!("shell.notification.industry_fallback").into_owned()),
      None => t!("shell.notification.industry_fallback").into_owned(),
    };
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: t!("shell.notification.industry_ready", product => product).into_owned(),
        dedup_key: industry_key(job_id),
        kind: NotificationKind::Industry,
        owner,
        target: NotificationTarget {
          character: matches!(owner, NotificationOwner::Character(_)).then_some(subject_id),
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: t!("shell.notification.industry_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn extraction_scheduled_detector(
  db: &Database,
  corporation_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CorporationMiningExtractions.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Corporation(corporation_id);
  let first = is_first_scan(db, owner, NotificationKind::ExtractionScheduled).await?;
  let scheduled: Vec<_> = org::corporation_mining_extractions(db, corporation_id)
    .await?
    .into_iter()
    .filter_map(|e| {
      e.chunk_arrival_time()
        .clone()
        .map(|arrival| (e.structure_id(), e.moon_id(), arrival, e.moon_name().clone()))
    })
    .collect();
  let watermarks: Vec<String> = scheduled
    .iter()
    .map(|(structure_id, moon_id, arrival, _)| {
      extraction_scheduled_key(corporation_id, *structure_id, *moon_id, arrival)
    })
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::ExtractionScheduled, &watermarks).await?;
    return Ok(Vec::new());
  }

  let who = corporation_name(db, corporation_id).await;
  let mut surfaced = Vec::new();
  for (structure_id, moon_id, arrival, moon_name) in scheduled {
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: moon_name.unwrap_or_else(|| who.clone()),
        dedup_key: extraction_scheduled_key(corporation_id, structure_id, moon_id, &arrival),
        kind: NotificationKind::ExtractionScheduled,
        owner: NotificationOwner::Corporation(corporation_id),
        target: NotificationTarget {
          character: None,
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: t!("shell.notification.extraction_scheduled_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

async fn extraction_cracked_detector(
  db: &Database,
  corporation_id: i64,
  now: DateTime<Utc>,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CorporationMiningExtractions.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let owner = NotificationOwner::Corporation(corporation_id);
  let first = is_first_scan(db, owner, NotificationKind::ExtractionCracked).await?;
  let now_rfc = now.to_rfc3339();
  let cracked: Vec<_> = org::corporation_mining_extractions(db, corporation_id)
    .await?
    .into_iter()
    .filter_map(|e| {
      e.chunk_arrival_time()
        .clone()
        .map(|arrival| (e.structure_id(), e.moon_id(), arrival, e.moon_name().clone()))
    })
    .filter(|(_, _, arrival, _)| crossed(arrival, &now_rfc))
    .collect();
  let watermarks: Vec<String> = cracked
    .iter()
    .map(|(structure_id, moon_id, arrival, _)| extraction_cracked_key(corporation_id, *structure_id, *moon_id, arrival))
    .collect();

  if first {
    notifications::watermark(db, &owner, NotificationKind::ExtractionCracked, &watermarks).await?;
    return Ok(Vec::new());
  }

  let mut surfaced = Vec::new();
  for (structure_id, moon_id, arrival, moon_name) in cracked {
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: moon_name.unwrap_or_else(|| t!("shell.notification.extraction_cracked_fallback").into_owned()),
        dedup_key: extraction_cracked_key(corporation_id, structure_id, moon_id, &arrival),
        kind: NotificationKind::ExtractionCracked,
        owner: NotificationOwner::Corporation(corporation_id),
        target: NotificationTarget {
          character: None,
          destination: NotificationDestination::Industry,
          sub: None,
        },
        title: t!("shell.notification.extraction_cracked_title").into_owned(),
      },
    )
    .await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

// Edge-triggered outbid alerts. Freshly-synced open orders are annotated against the live public
// region book (S3-T4); the transition not-outbid -> outbid fires exactly once. The per-order
// alert-state (S6-T1) freezes a price-derived marker while the order stays outbid, so its dedup_key is
// stable and the emit dedup swallows every rerun; regaining best price clears the state so the next
// undercut mints a fresh marker and a fresh notification. Wired into `detect()` separately (S6-T5).
async fn outbid_detector(
  db: &Database,
  character_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterMarketOrders.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let orders = finance::open_for_character(db, character_id).await?;
  let book = outbid_book(db, &orders).await;
  reconcile_outbid(db, character_id, &orders, &book).await
}

async fn reconcile_outbid<Q: outbid::BookQuote>(
  db: &Database,
  character_id: i64,
  orders: &[MarketOrder],
  book: &[Q],
) -> Result<Vec<Notification>, crate::store::Error> {
  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::Outbid).await?;
  let annotations = outbid::annotate_all(orders, book);

  if first {
    return watermark_outbid(db, character_id, orders, &annotations).await;
  }

  let mut surfaced = Vec::new();
  for (order, annotation) in orders.iter().zip(&annotations) {
    let emitted = step_outbid(db, character_id, order, annotation).await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

// First scan: watermark every already-outbid order (mark its alert-state and suppress the matching
// dedup_key) so a character deep in an undercut war at first sync is not flooded.
async fn watermark_outbid(
  db: &Database,
  character_id: i64,
  orders: &[MarketOrder],
  annotations: &[outbid::Annotation],
) -> Result<Vec<Notification>, crate::store::Error> {
  let owner = NotificationOwner::Character(character_id);
  let mut keys = Vec::new();
  for (order, annotation) in orders.iter().zip(annotations) {
    if let Some(best) = outbid_best(annotation) {
      let state = market_alert_state::mark(
        db,
        MarketAlertKind::Outbid,
        character_id,
        order.order_id(),
        &outbid_marker(best),
      )
      .await?;
      keys.push(state.dedup_key());
    }
  }
  notifications::watermark(db, &owner, NotificationKind::Outbid, &keys).await?;
  Ok(Vec::new())
}

async fn step_outbid(
  db: &Database,
  character_id: i64,
  order: &MarketOrder,
  annotation: &outbid::Annotation,
) -> Result<Vec<Notification>, crate::store::Error> {
  let Some(best) = outbid_best(annotation) else {
    market_alert_state::clear(db, MarketAlertKind::Outbid, character_id, order.order_id()).await?;
    return Ok(Vec::new());
  };
  let state = market_alert_state::mark(
    db,
    MarketAlertKind::Outbid,
    character_id,
    order.order_id(),
    &outbid_marker(best),
  )
  .await?;
  emit_outbid(db, character_id, order, &state.dedup_key()).await
}

async fn emit_outbid(
  db: &Database,
  character_id: i64,
  order: &MarketOrder,
  dedup_key: &str,
) -> Result<Vec<Notification>, crate::store::Error> {
  let item = type_name(db, order.type_id())
    .await
    .unwrap_or_else(|| t!("shell.notification.outbid_fallback").into_owned());
  let emitted = notifications::emit(
    db,
    &NewNotification {
      body: t!("shell.notification.outbid_body", item => item).into_owned(),
      dedup_key: dedup_key.to_owned(),
      kind: NotificationKind::Outbid,
      owner: NotificationOwner::Character(character_id),
      target: NotificationTarget::market_outbid(Some(character_id), order.order_id()),
      title: t!("shell.notification.outbid_title").into_owned(),
    },
  )
  .await?;
  Ok(emitted.into_iter().collect())
}

async fn outbid_book(db: &Database, orders: &[MarketOrder]) -> Vec<outbid::Quote> {
  let Ok(esi) = public_market_client(db) else {
    return Vec::new();
  };
  let mut seen: HashSet<(i64, i64)> = HashSet::new();
  let mut book = Vec::new();
  for order in orders {
    let key = (order.region_id(), order.type_id());
    if !seen.insert(key) {
      continue;
    }
    let sells = esi.market().sell_orders(key.0, key.1).await.unwrap_or_default();
    let buys = esi.market().buy_orders(key.0, key.1).await.unwrap_or_default();
    push_outbid_quotes(&mut book, key.1, false, sells);
    push_outbid_quotes(&mut book, key.1, true, buys);
  }
  book
}

fn push_outbid_quotes(book: &mut Vec<outbid::Quote>, type_id: i64, is_buy_order: bool, orders: Vec<RegionOrder>) {
  for order in orders {
    book.push(outbid::Quote {
      is_buy_order,
      location_id: order.location_id,
      price: order.price,
      type_id,
    });
  }
}

fn public_market_client(db: &Database) -> Result<esi::Client, clients::Error> {
  let http = http::Client::builder(http::Cache::new(db.clone())).build();
  esi::Client::builder(http).user_agent(clients::user_agent()).build()
}

fn outbid_best(annotation: &outbid::Annotation) -> Option<f64> {
  annotation.outbid.then_some(annotation.best).flatten()
}

fn outbid_marker(best: f64) -> String {
  format!("{best:.2}")
}

// Edge-triggered watchlist target alerts. Each owned character's watches (S4-T6) are evaluated over the
// live public region book; the transition not-met -> met fires exactly once. The per-watch alert-state
// (S6-T1) freezes a crossing-derived marker while the target stays met, so its dedup_key is stable and
// the emit dedup swallows every rerun; a retreat back across the target clears the state so the next
// crossing mints a fresh marker and a fresh notification. Wired into `detect()` separately (S6-T5).
async fn watchlist_target_detector(
  db: &Database,
  character_id: i64,
  features: &FeatureFlags,
) -> Result<Vec<Notification>, crate::store::Error> {
  if !JobKind::CharacterMarketOrders.is_feature_enabled(features) {
    return Ok(Vec::new());
  }

  let watches = market_watchlist::list_for_character(db, character_id).await?;
  let prices = target_book(db, &watches).await;
  reconcile_target(db, character_id, &watches, &prices).await
}

async fn reconcile_target(
  db: &Database,
  character_id: i64,
  watches: &[MarketWatch],
  prices: &watch_eval::PriceMap,
) -> Result<Vec<Notification>, crate::store::Error> {
  let owner = NotificationOwner::Character(character_id);
  let first = is_first_scan(db, owner, NotificationKind::WatchlistTarget).await?;

  if first {
    return watermark_target(db, character_id, watches, prices).await;
  }

  let mut surfaced = Vec::new();
  for watch in watches {
    let emitted = step_target(db, character_id, watch, prices).await?;
    surfaced.extend(emitted);
  }
  Ok(surfaced)
}

// First scan: mark every already-met watch (freeze its alert-state and suppress the matching dedup_key)
// so a character whose targets are already met at first sync is not flooded.
async fn watermark_target(
  db: &Database,
  character_id: i64,
  watches: &[MarketWatch],
  prices: &watch_eval::PriceMap,
) -> Result<Vec<Notification>, crate::store::Error> {
  let owner = NotificationOwner::Character(character_id);
  let mut keys = Vec::new();
  for watch in watches {
    if let Some(marker) = target_met_marker(watch, prices) {
      let state = market_alert_state::mark(db, MarketAlertKind::Target, character_id, watch.id, &marker).await?;
      keys.push(state.dedup_key());
    }
  }
  notifications::watermark(db, &owner, NotificationKind::WatchlistTarget, &keys).await?;
  Ok(Vec::new())
}

async fn step_target(
  db: &Database,
  character_id: i64,
  watch: &MarketWatch,
  prices: &watch_eval::PriceMap,
) -> Result<Vec<Notification>, crate::store::Error> {
  let Some(marker) = target_met_marker(watch, prices) else {
    market_alert_state::clear(db, MarketAlertKind::Target, character_id, watch.id).await?;
    return Ok(Vec::new());
  };
  let state = market_alert_state::mark(db, MarketAlertKind::Target, character_id, watch.id, &marker).await?;
  emit_target(db, character_id, watch, &state.dedup_key()).await
}

async fn emit_target(
  db: &Database,
  character_id: i64,
  watch: &MarketWatch,
  dedup_key: &str,
) -> Result<Vec<Notification>, crate::store::Error> {
  let item = type_name(db, watch.type_id)
    .await
    .unwrap_or_else(|| t!("shell.notification.watchlist_target_fallback").into_owned());
  let location = target_location_name(db, watch.region_id).await;
  let direction = target_direction_label(watch);
  let emitted = notifications::emit(
    db,
    &NewNotification {
      body: t!(
        "shell.notification.watchlist_target_body",
        item => item,
        direction => direction,
        location => location
      )
      .into_owned(),
      dedup_key: dedup_key.to_owned(),
      kind: NotificationKind::WatchlistTarget,
      owner: NotificationOwner::Character(character_id),
      target: NotificationTarget::market_watchlist_target(Some(character_id), watch.type_id),
      title: t!("shell.notification.watchlist_target_title").into_owned(),
    },
  )
  .await?;
  Ok(emitted.into_iter().collect())
}

// One live region-book read per distinct (type_id, region) pair so N watches on the same item and
// region trigger a single price read. Watches with no region resolve to nothing and are skipped.
async fn target_book(db: &Database, watches: &[MarketWatch]) -> watch_eval::PriceMap {
  let mut prices = watch_eval::PriceMap::new();
  let Ok(esi) = public_market_client(db) else {
    return prices;
  };
  let mut seen: HashSet<(i64, i64)> = HashSet::new();
  for watch in watches {
    let Some(region_id) = watch.region_id else {
      continue;
    };
    let key = (watch.type_id, region_id);
    if !seen.insert(key) {
      continue;
    }
    let best = target_best_prices(&esi, region_id, watch.type_id).await;
    prices.insert(key, best);
  }
  prices
}

async fn target_best_prices(esi: &esi::Client, region_id: i64, type_id: i64) -> watch_eval::BestPrices {
  let sells = esi.market().sell_orders(region_id, type_id).await.unwrap_or_default();
  let buys = esi.market().buy_orders(region_id, type_id).await.unwrap_or_default();
  watch_eval::BestPrices {
    best_buy: buys.iter().map(|order| order.price).max_by(f64::total_cmp),
    best_sell: sells.iter().map(|order| order.price).min_by(f64::total_cmp),
  }
}

// Reuses S4-T6's met rule (Buy watches best_sell, Sell watches best_buy; met = current crosses target):
// returns Some(marker) only when the target is met. The marker folds the crossing side and price so a
// fresh crossing after a retreat mints a distinct dedup_key.
fn target_met_marker(watch: &MarketWatch, prices: &watch_eval::PriceMap) -> Option<String> {
  let direction = WatchDirection::parse(&watch.direction)?;
  let region_id = watch.region_id?;
  let best = prices.get(&(watch.type_id, region_id))?;
  let outcome = watch_eval::evaluate(direction, watch.target_price, best);
  let current = outcome.current?;
  outcome.met.then(|| target_marker(direction, current))
}

fn target_marker(direction: WatchDirection, current: f64) -> String {
  format!("{}:{current:.2}", direction.as_str())
}

fn target_direction_label(watch: &MarketWatch) -> String {
  match WatchDirection::parse(&watch.direction) {
    Some(WatchDirection::Sell) => t!("shell.notification.watchlist_target_sell").into_owned(),
    _ => t!("shell.notification.watchlist_target_buy").into_owned(),
  }
}

async fn target_location_name(db: &Database, region_id: Option<i64>) -> String {
  let fallback = || t!("shell.notification.watchlist_target_location_fallback").into_owned();
  let Some(region_id) = region_id else {
    return fallback();
  };
  sde::get_region(db, region_id)
    .await
    .ok()
    .flatten()
    .map(|region| region.name().clone())
    .unwrap_or_else(fallback)
}

fn crossed(timestamp: &str, now_rfc: &str) -> bool {
  match (
    DateTime::parse_from_rfc3339(timestamp),
    DateTime::parse_from_rfc3339(now_rfc),
  ) {
    (Ok(when), Ok(now)) => when <= now,
    _ => false,
  }
}

fn is_industry_done(status: &str) -> bool {
  matches!(status, "ready" | "delivered")
}

fn calendar_key(character_id: i64, event_id: i64) -> String {
  format!("calendar:{character_id}:{event_id}")
}

fn extraction_cracked_key(corporation_id: i64, structure_id: i64, moon_id: i64, arrival: &str) -> String {
  format!("extraction_cracked:{corporation_id}:{structure_id}:{moon_id}:{arrival}")
}

fn extraction_scheduled_key(corporation_id: i64, structure_id: i64, moon_id: i64, arrival: &str) -> String {
  format!("extraction_scheduled:{corporation_id}:{structure_id}:{moon_id}:{arrival}")
}

fn industry_key(job_id: i64) -> String {
  format!("industry:{job_id}")
}

// Keyed on the killmail id ALONE (no owner): a kill attributable to both an owned character and its
// owned corporation must notify exactly once, so whichever owner's detector reaches it first wins and
// the other's emit is an INSERT OR IGNORE no-op.
fn killmail_key(killmail_id: i64) -> String {
  format!("killmail:{killmail_id}")
}

fn mail_key(character_id: i64, mail_id: i64) -> String {
  format!("mail:{character_id}:{mail_id}")
}

fn skill_key(character_id: i64, skill_id: i64, finish_date: &str) -> String {
  format!("skill:{character_id}:{skill_id}:{finish_date}")
}

#[cfg(test)]
mod tests {
  use super::*;

  mod crossed {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flags_a_past_timestamp_as_matured() {
      assert!(crossed("2026-01-01T00:00:00+00:00", "2026-06-01T00:00:00+00:00"));
    }

    #[test]
    fn it_does_not_flag_a_future_timestamp() {
      assert!(!crossed("2026-12-01T00:00:00+00:00", "2026-06-01T00:00:00+00:00"));
    }

    #[test]
    fn it_treats_an_unparseable_timestamp_as_not_matured() {
      assert!(!crossed("not-a-date", "2026-06-01T00:00:00+00:00"));
    }

    #[test]
    fn it_treats_the_exact_boundary_as_matured() {
      assert_eq!(crossed("2026-06-01T00:00:00+00:00", "2026-06-01T00:00:00+00:00"), true);
    }
  }

  mod is_industry_done {
    use super::*;

    #[test]
    fn it_only_fires_on_ready_or_delivered() {
      assert!(is_industry_done("ready"));
      assert!(is_industry_done("delivered"));
      assert!(!is_industry_done("active"));
      assert!(!is_industry_done("paused"));
    }
  }

  mod keys {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_stable_kind_prefixed_keys() {
      assert_eq!(mail_key(1, 2), "mail:1:2");
      assert_eq!(calendar_key(1, 2), "calendar:1:2");
      assert_eq!(killmail_key(7), "killmail:7");
      assert_eq!(
        skill_key(1, 3300, "2026-06-22T00:00:00+00:00"),
        "skill:1:3300:2026-06-22T00:00:00+00:00"
      );
      assert_eq!(industry_key(55), "industry:55");
      assert_eq!(
        extraction_scheduled_key(98, 1001, 40_000, "2026-06-22T00:00:00+00:00"),
        "extraction_scheduled:98:1001:40000:2026-06-22T00:00:00+00:00"
      );
      assert_eq!(
        extraction_cracked_key(98, 1001, 40_000, "2026-06-22T00:00:00+00:00"),
        "extraction_cracked:98:1001:40000:2026-06-22T00:00:00+00:00"
      );
    }
  }

  mod killmail_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{
        self,
        model::{CharacterKillEntry, CorporationKillEntry},
      },
    };

    const CHARACTER: i64 = 95_465_499;

    fn flags() -> FeatureFlags {
      FeatureFlags::default()
    }

    fn killmails_disabled() -> FeatureFlags {
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::CombatLog, false);
      flags
    }

    async fn seed_character(db: &Database) {
      sqlx::query("INSERT INTO races (id, alliance_id, description, name) VALUES (1, 1, '', 'Caldari')")
        .execute(db.writer())
        .await
        .unwrap();
      let bloodline = "INSERT INTO bloodlines \
        (id, corporation_id, race_id, charisma, description, intelligence, memory, name, perception, willpower) \
        VALUES (1, 1, 1, 1, '', 1, 1, 'Civire', 1, 1)";
      sqlx::query(bloodline).execute(db.writer()).await.unwrap();
      let corporation = "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
        VALUES (1, 1, 1, 1, 'Test Corp', 0.0, 'TEST')";
      sqlx::query(corporation).execute(db.writer()).await.unwrap();
      let character = "INSERT INTO characters (id, bloodline_id, corporation_id, race_id, birthday, gender, name) \
        VALUES (?, 1, 1, 1, '2020-01-01T00:00:00Z', 'male', 'Test Pilot')";
      sqlx::query(character)
        .bind(CHARACTER)
        .execute(db.writer())
        .await
        .unwrap();
    }

    fn kill(killmail_id: i64) -> CharacterKillEntry {
      CharacterKillEntry {
        attacker_count: 1,
        character_id: CHARACTER,
        final_blow: true,
        is_kill: true,
        kill_hash: format!("hash-{killmail_id}"),
        kill_time: "2026-06-20T00:00:00+00:00".to_owned(),
        killmail_id,
        ship_type_id: 587,
        synced_at: "2026-06-22T00:00:00+00:00".to_owned(),
        system_id: 30_000_142,
        value_destroyed_isk: 0.0,
        value_final: true,
        value_isk: 0.0,
        value_recheck_count: 0,
        value_source: "zkill".to_owned(),
        victim_alliance_id: None,
        victim_corp_id: None,
        victim_damage_taken: 100,
        victim_id: None,
      }
    }

    const CORPORATION: i64 = 1;

    fn corp_kill(killmail_id: i64) -> CorporationKillEntry {
      CorporationKillEntry {
        attacker_count: 1,
        corporation_id: CORPORATION,
        final_blow: true,
        is_kill: true,
        kill_hash: format!("corp-hash-{killmail_id}"),
        kill_time: "2026-06-20T00:00:00+00:00".to_owned(),
        killmail_id,
        ship_type_id: 587,
        synced_at: "2026-06-22T00:00:00+00:00".to_owned(),
        system_id: 30_000_142,
        value_destroyed_isk: 0.0,
        value_final: true,
        value_isk: 0.0,
        value_recheck_count: 0,
        value_source: "zkill".to_owned(),
        victim_alliance_id: None,
        victim_corp_id: None,
        victim_damage_taken: 100,
        victim_id: None,
      }
    }

    #[tokio::test]
    async fn it_watermarks_existing_history_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();

      let surfaced = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "pre-existing history is watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_exactly_one_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      character::upsert_killmail(&db, &kill(2)).await.unwrap();

      let surfaced = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "killmail:2");
      assert_eq!(notifications::unread_count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_is_a_no_op_on_a_rerun_over_unchanged_data() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      character::upsert_killmail(&db, &kill(2)).await.unwrap();
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      let rerun = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();

      assert!(rerun.is_empty(), "re-running over unchanged data surfaces nothing new");
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_notifies_once_when_owned_by_both_a_character_and_its_corporation() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      killmail_detector(&db, NotificationOwner::Corporation(CORPORATION), &flags())
        .await
        .unwrap();
      character::upsert_killmail(&db, &kill(42)).await.unwrap();
      org::upsert_corporation_killmail(&db, &corp_kill(42)).await.unwrap();

      let from_char = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &flags())
        .await
        .unwrap();
      let from_corp = killmail_detector(&db, NotificationOwner::Corporation(CORPORATION), &flags())
        .await
        .unwrap();

      assert_eq!(from_char.len(), 1, "the character detector surfaces the kill once");
      assert!(
        from_corp.is_empty(),
        "the corp detector sees the same key already taken"
      );
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db).await;
      character::upsert_killmail(&db, &kill(1)).await.unwrap();

      let surfaced = killmail_detector(&db, NotificationOwner::Character(CHARACTER), &killmails_disabled())
        .await
        .unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  async fn seed_character(db: &Database, character_id: i64) {
    sqlx::query(
      "INSERT INTO races (id, alliance_id, description, name) VALUES (1, 1, '', 'Caldari') ON CONFLICT DO NOTHING",
    )
    .execute(db.writer())
    .await
    .unwrap();
    let bloodline = "INSERT INTO bloodlines \
      (id, corporation_id, race_id, charisma, description, intelligence, memory, name, perception, willpower) \
      VALUES (1, 1, 1, 1, '', 1, 1, 'Civire', 1, 1) ON CONFLICT DO NOTHING";
    sqlx::query(bloodline).execute(db.writer()).await.unwrap();
    let corporation = "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (1, 1, 1, 1, 'Test Corp', 0.0, 'TEST') ON CONFLICT DO NOTHING";
    sqlx::query(corporation).execute(db.writer()).await.unwrap();
    let character = "INSERT INTO characters (id, bloodline_id, corporation_id, race_id, birthday, gender, name) \
      VALUES (?, 1, 1, 1, '2020-01-01T00:00:00Z', 'male', 'Test Pilot')";
    sqlx::query(character)
      .bind(character_id)
      .execute(db.writer())
      .await
      .unwrap();
  }

  async fn seed_corporation(db: &Database, corporation_id: i64) {
    sqlx::query(
      "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
      VALUES (?, 1, 1, 1, 'Test Corp', 0.0, 'TEST') ON CONFLICT DO NOTHING",
    )
    .bind(corporation_id)
    .execute(db.writer())
    .await
    .unwrap();
  }

  mod calendar_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CharacterCalendarEvent},
    };

    const CHARACTER: i64 = 7001;

    fn event(event_id: i64) -> CharacterCalendarEvent {
      CharacterCalendarEvent {
        character_id: CHARACTER,
        event_id,
        owner_name: "Fleet Command".to_owned(),
        timestamp: "2026-06-20T00:00:00+00:00".to_owned(),
        title: "Op briefing".to_owned(),
        ..Default::default()
      }
    }

    async fn seed_event(db: &Database, event_id: i64) {
      calendar::upsert_complete(db, &event(event_id), &[]).await.unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_existing_history_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;

      let surfaced = calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "pre-existing events are watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_a_new_event_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;
      calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();
      seed_event(&db, 2).await;

      let surfaced = calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "calendar:7001:2");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].title(), "Op briefing");
      assert_eq!(surfaced[0].body(), "Fleet Command");
      assert_eq!(
        surfaced[0].target().destination,
        crate::store::model::NotificationDestination::Calendar
      );
    }

    #[tokio::test]
    async fn it_is_a_no_op_on_a_rerun_over_unchanged_data() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;
      calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();
      seed_event(&db, 2).await;
      calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      let rerun = calendar_detector(&db, CHARACTER, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(rerun.is_empty());
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_event(&db, 1).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Calendar, false);

      let surfaced = calendar_detector(&db, CHARACTER, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod extraction_cracked_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CorporationMiningExtraction},
    };

    const CORPORATION: i64 = 98_000_001;

    const NOW: &str = "2026-06-21T00:00:00+00:00";

    fn now() -> DateTime<Utc> {
      DateTime::parse_from_rfc3339(NOW).unwrap().with_timezone(&Utc)
    }

    fn extraction(structure_id: i64, moon_id: i64, arrival: &str) -> CorporationMiningExtraction {
      CorporationMiningExtraction {
        chunk_arrival_time: Some(arrival.to_owned()),
        corporation_id: CORPORATION,
        extraction_start_time: None,
        moon_id,
        moon_name: None,
        natural_decay_time: None,
        security_status: None,
        solar_system_id: None,
        structure_id,
      }
    }

    async fn seed(db: &Database, extractions: &[CorporationMiningExtraction]) {
      seed_corporation(db, CORPORATION).await;
      org::replace_extractions_for_corporation(db, CORPORATION, extractions)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_already_cracked_chunks_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-06-20T00:00:00+00:00")]).await;

      let surfaced = extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "a past arrival is watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_ignores_a_future_arrival() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();

      let surfaced = extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "a chunk whose arrival is still in the future never cracks"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_when_a_chunk_matures_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      extraction_cracked_detector(&db, CORPORATION, now(), &FeatureFlags::default())
        .await
        .unwrap();
      let later = DateTime::parse_from_rfc3339("2026-12-02T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc);

      let surfaced = extraction_cracked_detector(&db, CORPORATION, later, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(
        surfaced[0].dedup_key(),
        "extraction_cracked:98000001:1001:40000:2026-12-01T00:00:00+00:00"
      );
      assert_eq!(surfaced[0].owner(), NotificationOwner::Corporation(CORPORATION));
      assert_eq!(surfaced[0].title(), "Moon chunk fractured");
      assert_eq!(surfaced[0].body(), "Ready to mine");
      assert_eq!(surfaced[0].target().character, None);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-06-20T00:00:00+00:00")]).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Industry, false);

      let surfaced = extraction_cracked_detector(&db, CORPORATION, now(), &flags)
        .await
        .unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod extraction_scheduled_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CorporationMiningExtraction},
    };

    const CORPORATION: i64 = 98_000_002;

    fn extraction(structure_id: i64, moon_id: i64, arrival: &str) -> CorporationMiningExtraction {
      CorporationMiningExtraction {
        chunk_arrival_time: Some(arrival.to_owned()),
        corporation_id: CORPORATION,
        extraction_start_time: None,
        moon_id,
        moon_name: None,
        natural_decay_time: None,
        security_status: None,
        solar_system_id: None,
        structure_id,
      }
    }

    fn no_arrival(structure_id: i64, moon_id: i64) -> CorporationMiningExtraction {
      CorporationMiningExtraction {
        chunk_arrival_time: None,
        corporation_id: CORPORATION,
        extraction_start_time: None,
        moon_id,
        moon_name: None,
        natural_decay_time: None,
        security_status: None,
        solar_system_id: None,
        structure_id,
      }
    }

    async fn seed(db: &Database, extractions: &[CorporationMiningExtraction]) {
      seed_corporation(db, CORPORATION).await;
      org::replace_extractions_for_corporation(db, CORPORATION, extractions)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_existing_schedules_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "pre-existing schedules are watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_skips_an_extraction_without_an_arrival_time() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[no_arrival(1001, 40_000)]).await;
      extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();
      seed(&db, &[no_arrival(1001, 40_000), no_arrival(1002, 40_001)]).await;

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "an extraction with no chunk arrival has nothing to schedule"
      );
    }

    #[tokio::test]
    async fn it_emits_a_new_schedule_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();
      seed(
        &db,
        &[
          extraction(1001, 40_000, "2026-12-01T00:00:00+00:00"),
          extraction(1002, 40_001, "2026-12-05T00:00:00+00:00"),
        ],
      )
      .await;

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(
        surfaced[0].dedup_key(),
        "extraction_scheduled:98000002:1002:40001:2026-12-05T00:00:00+00:00"
      );
      assert_eq!(surfaced[0].owner(), NotificationOwner::Corporation(CORPORATION));
      assert_eq!(surfaced[0].title(), "Extraction scheduled");
      assert_eq!(
        surfaced[0].target().destination,
        crate::store::model::NotificationDestination::Industry
      );
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed(&db, &[extraction(1001, 40_000, "2026-12-01T00:00:00+00:00")]).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Industry, false);

      let surfaced = extraction_scheduled_detector(&db, CORPORATION, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod industry_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{
        self,
        model::{CharacterIndustryJob, CorporationIndustryJob, CorporationMemberRole, OwnerType},
        repo::infra,
      },
    };

    const CHARACTER: i64 = 7100;

    const CORPORATION: i64 = 1;

    const DIRECTOR: i64 = 7100;

    fn job(job_id: i64, status: &str) -> CharacterIndustryJob {
      CharacterIndustryJob {
        activity_id: 1,
        blueprint_id: 100,
        blueprint_location_id: 60_003_760,
        blueprint_type_id: 12_345,
        character_id: CHARACTER,
        completed_character_id: None,
        completed_date: None,
        cost: None,
        duration: 3600,
        end_date: "2026-06-20T00:00:00+00:00".to_owned(),
        facility_id: 60_003_760,
        installer_id: CHARACTER,
        job_id,
        licensed_runs: None,
        output_location_id: 60_003_760,
        pause_date: None,
        probability: None,
        product_type_id: Some(587),
        runs: 1,
        start_date: "2026-06-19T00:00:00+00:00".to_owned(),
        station_id: None,
        status: status.to_owned(),
        successful_runs: None,
      }
    }

    fn corp_job(job_id: i64, status: &str) -> CorporationIndustryJob {
      CorporationIndustryJob {
        activity_id: 1,
        blueprint_id: 100,
        blueprint_location_id: 60_003_760,
        blueprint_type_id: 12_345,
        completed_character_id: None,
        completed_date: None,
        corporation_id: CORPORATION,
        cost: None,
        duration: 3600,
        end_date: "2026-06-20T00:00:00+00:00".to_owned(),
        facility_id: 60_003_760,
        installer_id: DIRECTOR,
        job_id,
        licensed_runs: None,
        output_location_id: 60_003_760,
        pause_date: None,
        probability: None,
        product_type_id: Some(587),
        runs: 1,
        start_date: "2026-06-19T00:00:00+00:00".to_owned(),
        station_id: None,
        status: status.to_owned(),
        successful_runs: None,
      }
    }

    // The corp industry reader gates on `corp_is_authorized`: an owned corp whose Director-roled
    // credential authorizer is a member holding the Director role. Seed that full chain.
    async fn authorize_corp(db: &Database) {
      seed_character(db, DIRECTOR).await;
      infra::upsert(
        db,
        CORPORATION,
        OwnerType::Corporation,
        "tok",
        "rt",
        9999,
        Some(DIRECTOR),
        None,
      )
      .await
      .unwrap();
      org::replace_for_corporation(
        db,
        CORPORATION,
        &[CorporationMemberRole::from((
          CORPORATION,
          DIRECTOR,
          "Director".to_owned(),
        ))],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_finished_jobs_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      industry::replace_for_character(&db, CHARACTER, &[job(1, "ready")])
        .await
        .unwrap();

      let surfaced = industry_detector(&db, NotificationOwner::Character(CHARACTER), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "pre-existing finished jobs are watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_only_surfaces_done_jobs() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      industry::replace_for_character(&db, CHARACTER, &[job(1, "ready")])
        .await
        .unwrap();
      industry_detector(&db, NotificationOwner::Character(CHARACTER), &FeatureFlags::default())
        .await
        .unwrap();
      industry::replace_for_character(
        &db,
        CHARACTER,
        &[job(1, "ready"), job(2, "active"), job(3, "delivered")],
      )
      .await
      .unwrap();

      let surfaced = industry_detector(&db, NotificationOwner::Character(CHARACTER), &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1, "only the newly-delivered job surfaces");
      assert_eq!(surfaced[0].dedup_key(), "industry:3");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].title(), "Industry job complete");
      assert_eq!(surfaced[0].target().character, Some(CHARACTER));
    }

    #[tokio::test]
    async fn it_emits_for_a_corporation_owner() {
      let db = store::open_test().await.unwrap();
      authorize_corp(&db).await;
      industry::replace_for_corporation(&db, CORPORATION, &[corp_job(10, "ready")])
        .await
        .unwrap();
      industry_detector(
        &db,
        NotificationOwner::Corporation(CORPORATION),
        &FeatureFlags::default(),
      )
      .await
      .unwrap();
      industry::replace_for_corporation(&db, CORPORATION, &[corp_job(10, "ready"), corp_job(11, "delivered")])
        .await
        .unwrap();

      let surfaced = industry_detector(
        &db,
        NotificationOwner::Corporation(CORPORATION),
        &FeatureFlags::default(),
      )
      .await
      .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "industry:11");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Corporation(CORPORATION));
      assert_eq!(
        surfaced[0].target().character,
        None,
        "a corp job carries no character target"
      );
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      industry::replace_for_character(&db, CHARACTER, &[job(1, "ready")])
        .await
        .unwrap();
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Industry, false);

      let surfaced = industry_detector(&db, NotificationOwner::Character(CHARACTER), &flags)
        .await
        .unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod outbid_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      clients::esi::models::character::MarketOrder as EsiMarketOrder,
      config::SubFeature,
      store::{self, repo::market_alert_state},
    };

    const CHARACTER: i64 = 7300;

    const STATION: i64 = 60_003_760;

    const REGION: i64 = 10_000_002;

    const TYPE: i64 = 34;

    fn order(order_id: i64, price: f64) -> MarketOrder {
      MarketOrder::from((
        CHARACTER,
        EsiMarketOrder {
          duration: 90,
          escrow: 0.0,
          is_buy_order: false,
          issued: "2026-07-13T00:00:00Z".to_owned(),
          location_id: STATION,
          min_volume: Some(1),
          order_id,
          price,
          range: "station".to_owned(),
          region_id: REGION,
          type_id: TYPE,
          volume_remain: 100,
          volume_total: 100,
        },
      ))
    }

    fn competitor(price: f64) -> outbid::Quote {
      outbid::Quote {
        is_buy_order: false,
        location_id: STATION,
        price,
        type_id: TYPE,
      }
    }

    async fn seed_order(db: &Database, order_id: i64, price: f64) {
      finance::replace(db, CHARACTER, &[order(order_id, price)])
        .await
        .unwrap();
    }

    async fn orders(db: &Database) -> Vec<MarketOrder> {
      finance::open_for_character(db, CHARACTER).await.unwrap()
    }

    #[tokio::test]
    async fn it_watermarks_an_already_outbid_order_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_order(&db, 5001, 100.0).await;

      let surfaced = reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(90.0)])
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "a first-scan undercut is watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
      let state = market_alert_state::read(&db, MarketAlertKind::Outbid, CHARACTER, 5001)
        .await
        .unwrap()
        .unwrap();
      assert!(state.alerted);
      assert_eq!(state.marker, "90.00");
    }

    #[tokio::test]
    async fn it_does_not_re_emit_a_watermarked_order_on_a_later_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_order(&db, 5001, 100.0).await;
      reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(90.0)])
        .await
        .unwrap();

      let again = reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(90.0)])
        .await
        .unwrap();

      assert!(again.is_empty(), "an order already outbid at first scan never surfaces");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_exactly_one_on_the_not_outbid_to_outbid_transition() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_order(&db, 5001, 100.0).await;
      // First scan while holding best price: nothing outbid, so only the first-scan sentinel lands.
      reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[] as &[outbid::Quote])
        .await
        .unwrap();

      let surfaced = reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(90.0)])
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), &format!("outbid:{CHARACTER}:5001:90.00"));
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].title(), "You have been outbid");
      assert_eq!(surfaced[0].target().destination, NotificationDestination::Market);
      assert_eq!(surfaced[0].target().character, Some(CHARACTER));
    }

    #[tokio::test]
    async fn it_never_re_fires_while_the_order_stays_outbid() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_order(&db, 5001, 100.0).await;
      reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[] as &[outbid::Quote])
        .await
        .unwrap();
      reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(90.0)])
        .await
        .unwrap();

      // Still outbid, competitor drops further: the marker is frozen so the dedup_key holds.
      let rerun = reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(80.0)])
        .await
        .unwrap();

      assert!(rerun.is_empty(), "re-running while still outbid surfaces nothing");
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_re_fires_with_a_fresh_dedup_key_after_regaining_best_then_being_undercut() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_order(&db, 5001, 100.0).await;
      reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[] as &[outbid::Quote])
        .await
        .unwrap();
      let first = reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(90.0)])
        .await
        .unwrap();
      // Order regains best price (no competing undercut): alert-state clears.
      reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[] as &[outbid::Quote])
        .await
        .unwrap();

      let refire = reconcile_outbid(&db, CHARACTER, &orders(&db).await, &[competitor(80.0)])
        .await
        .unwrap();

      assert_eq!(refire.len(), 1, "a fresh undercut after regaining best fires again");
      assert_eq!(refire[0].dedup_key(), &format!("outbid:{CHARACTER}:5001:80.00"));
      assert_ne!(refire[0].dedup_key(), first[0].dedup_key());
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_order(&db, 5001, 100.0).await;
      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::MarketOrders, false);

      let surfaced = outbid_detector(&db, CHARACTER, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    mod helpers {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_derives_a_two_decimal_marker_from_the_best_price() {
        assert_eq!(outbid_marker(90.0), "90.00");
        assert_eq!(outbid_marker(90.055), "90.06");
        assert_eq!(outbid_marker(1_234.5), "1234.50");
      }

      #[test]
      fn it_reads_the_best_price_only_when_outbid() {
        let hit = outbid::Annotation {
          best: Some(90.0),
          gap: Some(10.0),
          gap_pct: Some(10.0),
          outbid: true,
        };
        assert_eq!(outbid_best(&hit), Some(90.0));
        assert_eq!(outbid_best(&outbid::Annotation::default()), None);
      }

      #[test]
      fn it_maps_region_orders_into_side_tagged_quotes() {
        let mut book = Vec::new();
        push_outbid_quotes(
          &mut book,
          TYPE,
          false,
          vec![RegionOrder {
            duration: 90,
            is_buy_order: true,
            issued: String::new(),
            location_id: STATION,
            min_volume: 1,
            order_id: 1,
            price: 42.0,
            range: "station".to_owned(),
            system_id: 30_000_142,
            type_id: 999,
            volume_remain: 5,
          }],
        );

        assert_eq!(book.len(), 1);
        assert_eq!(
          book[0].is_buy_order, false,
          "the side is stamped by the caller, not the row"
        );
        assert_eq!(book[0].type_id, TYPE);
        assert_eq!(book[0].location_id, STATION);
        assert_eq!(book[0].price, 42.0);
      }
    }
  }

  mod mail_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{
        self,
        model::{CharacterMail, CharacterMailBody, CharacterMailRecipient},
      },
    };

    const CHARACTER: i64 = 7200;

    fn header(mail_id: i64, from_id: i64, subject: Option<&str>) -> CharacterMail {
      CharacterMail {
        character_id: CHARACTER,
        from_id,
        from_name: "Sender".to_owned(),
        mail_id,
        subject: subject.map(str::to_owned),
        timestamp: "2026-06-20T00:00:00+00:00".to_owned(),
        ..Default::default()
      }
    }

    async fn seed_mail(db: &Database, mail_id: i64, from_id: i64, subject: Option<&str>) {
      let body = CharacterMailBody {
        body: "<p>hi</p>".to_owned(),
        character_id: CHARACTER,
        mail_id,
      };
      mail::upsert_complete(
        db,
        &header(mail_id, from_id, subject),
        &body,
        &[] as &[CharacterMailRecipient],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_existing_mail_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("Hello")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert!(surfaced.is_empty(), "pre-existing mail is watermarked, not surfaced");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_a_new_received_mail_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("Hello")).await;
      mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();
      seed_mail(&db, 2, 888, Some("Re: contract")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "mail:7200:2");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].title(), "Re: contract");
      assert_eq!(surfaced[0].body(), "From Sender");
    }

    #[tokio::test]
    async fn it_titles_an_empty_subject_mail_with_a_fallback() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("seed")).await;
      mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();
      seed_mail(&db, 2, 888, Some("")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].title(), "New EVE mail");
    }

    #[tokio::test]
    async fn it_drops_self_sent_mail() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("seed")).await;
      mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();
      seed_mail(&db, 2, CHARACTER, Some("My own message")).await;

      let surfaced = mail_detector(&db, CHARACTER, &FeatureFlags::default()).await.unwrap();

      assert!(surfaced.is_empty(), "mail authored by the owner is filtered out");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_mail(&db, 1, 999, Some("Hello")).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::Mail, false);

      let surfaced = mail_detector(&db, CHARACTER, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }

  mod skill_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::Feature,
      store::{self, model::CharacterSkillqueue},
    };

    const CHARACTER: i64 = 7300;

    const NOW: &str = "2026-06-21T00:00:00+00:00";

    fn now() -> DateTime<Utc> {
      DateTime::parse_from_rfc3339(NOW).unwrap().with_timezone(&Utc)
    }

    fn entry(skill_id: i64, queue_position: i64, finish: Option<&str>) -> CharacterSkillqueue {
      CharacterSkillqueue {
        character_id: CHARACTER,
        finish_date: finish.map(str::to_owned),
        finished_level: 5,
        level_end_sp: None,
        level_start_sp: None,
        queue_position,
        skill_id,
        start_date: None,
        training_start_sp: None,
      }
    }

    async fn seed_queue(db: &Database, entries: &[CharacterSkillqueue]) {
      character::replace_skillqueue(db, CHARACTER, entries).await.unwrap();
    }

    #[tokio::test]
    async fn it_watermarks_already_matured_skills_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-06-20T00:00:00+00:00"))]).await;

      let surfaced = skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "a skill already finished is watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_ignores_a_skill_still_training_and_one_without_a_finish_date() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(
        &db,
        &[entry(3300, 0, Some("2026-12-01T00:00:00+00:00")), entry(3301, 1, None)],
      )
      .await;
      skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();

      let surfaced = skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();

      assert!(surfaced.is_empty(), "future and not-yet-started skills never mature");
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_when_a_skill_matures_after_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-12-01T00:00:00+00:00"))]).await;
      skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();
      let later = DateTime::parse_from_rfc3339("2026-12-02T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc);

      let surfaced = skill_detector(&db, CHARACTER, later, &FeatureFlags::default())
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(surfaced[0].dedup_key(), "skill:7300:3300:2026-12-01T00:00:00+00:00");
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(surfaced[0].body(), "Training complete");
      assert_eq!(surfaced[0].target().character, Some(CHARACTER));
      assert_eq!(
        surfaced[0].target().destination,
        crate::store::model::NotificationDestination::Skills
      );
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-06-20T00:00:00+00:00"))]).await;
      let mut flags = FeatureFlags::default();
      flags.set_enabled(Feature::SkillMonitoring, false);

      let surfaced = skill_detector(&db, CHARACTER, now(), &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_captures_a_detected_completion_in_the_table_exactly_once() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-12-01T00:00:00+00:00"))]).await;
      skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();
      let later = DateTime::parse_from_rfc3339("2026-12-02T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc);

      skill_detector(&db, CHARACTER, later, &FeatureFlags::default())
        .await
        .unwrap();
      skill_detector(&db, CHARACTER, later, &FeatureFlags::default())
        .await
        .unwrap();

      let rows = skill_completion::unverified(&db, CHARACTER).await.unwrap();
      assert_eq!(
        rows.len(),
        1,
        "a repeated detection captures the completion exactly once"
      );
      assert_eq!(rows[0].skill_id, 3300);
      assert_eq!(rows[0].level, 5);
      assert_eq!(rows[0].completed_at, "2026-12-01T00:00:00+00:00");
    }

    #[tokio::test]
    async fn it_captures_even_when_the_notification_is_deduped() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-12-01T00:00:00+00:00"))]).await;
      skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();
      let later = DateTime::parse_from_rfc3339("2026-12-02T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc);
      skill_detector(&db, CHARACTER, later, &FeatureFlags::default())
        .await
        .unwrap();

      let rerun = skill_detector(&db, CHARACTER, later, &FeatureFlags::default())
        .await
        .unwrap();

      assert!(rerun.is_empty(), "the notification is deduped on the rerun");
      assert_eq!(skill_completion::unverified(&db, CHARACTER).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_capture_watermarked_history_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_queue(&db, &[entry(3300, 0, Some("2026-06-20T00:00:00+00:00"))]).await;

      skill_detector(&db, CHARACTER, now(), &FeatureFlags::default())
        .await
        .unwrap();

      let rows = skill_completion::unverified(&db, CHARACTER).await.unwrap();
      assert!(rows.is_empty(), "pre-existing history is watermarked, not captured");
    }
  }

  mod target_marker {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_folds_the_crossing_side_and_price() {
      assert_eq!(target_marker(WatchDirection::Buy, 9.0), "buy:9.00");
      assert_eq!(target_marker(WatchDirection::Sell, 12.5), "sell:12.50");
    }
  }

  mod target_met_marker {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::model::WatchDirection;

    const TYPE_ID: i64 = 34;

    const REGION: i64 = 10_000_002;

    fn watch(direction: WatchDirection, target: Option<f64>) -> MarketWatch {
      MarketWatch {
        character_id: 1,
        created_at: String::new(),
        direction: direction.as_str().to_owned(),
        id: 7,
        location_id: None,
        region_id: Some(REGION),
        target_price: target,
        type_id: TYPE_ID,
        updated_at: String::new(),
      }
    }

    fn price_map(best_buy: Option<f64>, best_sell: Option<f64>) -> watch_eval::PriceMap {
      let mut map = watch_eval::PriceMap::new();
      map.insert(
        (TYPE_ID, REGION),
        watch_eval::BestPrices {
          best_buy,
          best_sell,
        },
      );
      map
    }

    #[test]
    fn it_marks_a_met_buy_watch_from_the_best_sell() {
      let marker = target_met_marker(
        &watch(WatchDirection::Buy, Some(10.0)),
        &price_map(Some(8.0), Some(9.0)),
      );
      assert_eq!(marker, Some("buy:9.00".to_owned()));
    }

    #[test]
    fn it_marks_a_met_sell_watch_from_the_best_buy() {
      let marker = target_met_marker(
        &watch(WatchDirection::Sell, Some(10.0)),
        &price_map(Some(12.0), Some(13.0)),
      );
      assert_eq!(marker, Some("sell:12.00".to_owned()));
    }

    #[test]
    fn it_yields_nothing_when_the_target_is_unmet() {
      let marker = target_met_marker(
        &watch(WatchDirection::Buy, Some(10.0)),
        &price_map(Some(8.0), Some(11.0)),
      );
      assert_eq!(marker, None);
    }

    #[test]
    fn it_yields_nothing_when_the_region_book_is_absent() {
      let marker = target_met_marker(&watch(WatchDirection::Buy, Some(10.0)), &watch_eval::PriceMap::new());
      assert_eq!(marker, None);
    }
  }

  mod watchlist_target_detector {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      config::SubFeature,
      store::{
        self,
        model::{NewWatch, WatchDirection},
      },
    };

    const CHARACTER: i64 = 91_000_001;

    const TYPE_ID: i64 = 34;

    const REGION: i64 = 10_000_002;

    async fn seed_region(db: &Database) {
      sqlx::query("INSERT INTO regions (id, description, name) VALUES (?, '', 'The Forge') ON CONFLICT DO NOTHING")
        .bind(REGION)
        .execute(db.writer())
        .await
        .unwrap();
    }

    async fn seed_watch(db: &Database, direction: WatchDirection, target: f64) -> i64 {
      let created = market_watchlist::create(
        db,
        &NewWatch {
          character_id: CHARACTER,
          direction,
          location_id: None,
          region_id: Some(REGION),
          target_price: Some(target),
          type_id: TYPE_ID,
        },
      )
      .await
      .unwrap();
      created.id
    }

    fn price_map(best_buy: Option<f64>, best_sell: Option<f64>) -> watch_eval::PriceMap {
      let mut map = watch_eval::PriceMap::new();
      map.insert(
        (TYPE_ID, REGION),
        watch_eval::BestPrices {
          best_buy,
          best_sell,
        },
      );
      map
    }

    async fn watches(db: &Database) -> Vec<MarketWatch> {
      market_watchlist::list_for_character(db, CHARACTER).await.unwrap()
    }

    #[tokio::test]
    async fn it_watermarks_already_met_watches_on_the_first_scan() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_watch(&db, WatchDirection::Buy, 10.0).await;

      let surfaced = reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(9.0)))
        .await
        .unwrap();

      assert!(
        surfaced.is_empty(),
        "an already-met target is watermarked, not surfaced"
      );
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_emits_exactly_one_on_the_not_met_to_met_transition() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_region(&db).await;
      let watch_id = seed_watch(&db, WatchDirection::Buy, 10.0).await;
      // First scan while unmet: watermark nothing, so a later crossing is a genuine transition.
      reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(11.0)))
        .await
        .unwrap();

      let surfaced = reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(9.0)))
        .await
        .unwrap();

      assert_eq!(surfaced.len(), 1);
      assert_eq!(
        surfaced[0].dedup_key(),
        &format!("target:{CHARACTER}:{watch_id}:buy:9.00")
      );
      assert_eq!(surfaced[0].owner(), NotificationOwner::Character(CHARACTER));
      assert_eq!(
        surfaced[0].target().destination,
        crate::store::model::NotificationDestination::Market
      );
      assert_eq!(notifications::unread_count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn it_is_a_no_op_while_the_target_stays_met() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_region(&db).await;
      seed_watch(&db, WatchDirection::Buy, 10.0).await;
      reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(11.0)))
        .await
        .unwrap();
      reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(9.0)))
        .await
        .unwrap();

      let rerun = reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(8.0)))
        .await
        .unwrap();

      assert!(
        rerun.is_empty(),
        "the frozen marker keeps the dedup_key stable while met"
      );
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_clears_and_re_fires_a_fresh_key_when_price_retreats_then_recrosses() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_region(&db).await;
      let watch_id = seed_watch(&db, WatchDirection::Buy, 10.0).await;
      reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(11.0)))
        .await
        .unwrap();
      let first = reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(9.0)))
        .await
        .unwrap();
      // Retreat above the target clears the alert-state.
      reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(12.0)))
        .await
        .unwrap();

      let second = reconcile_target(&db, CHARACTER, &watches(&db).await, &price_map(None, Some(7.0)))
        .await
        .unwrap();

      assert_eq!(first.len(), 1);
      assert_eq!(second.len(), 1);
      assert_eq!(first[0].dedup_key(), &format!("target:{CHARACTER}:{watch_id}:buy:9.00"));
      assert_eq!(
        second[0].dedup_key(),
        &format!("target:{CHARACTER}:{watch_id}:buy:7.00")
      );
      assert_ne!(first[0].dedup_key(), second[0].dedup_key());
      assert_eq!(notifications::list(&db, 50).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn it_does_not_run_for_a_disabled_feature() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHARACTER).await;
      seed_watch(&db, WatchDirection::Buy, 10.0).await;
      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::MarketOrders, false);

      let surfaced = watchlist_target_detector(&db, CHARACTER, &flags).await.unwrap();

      assert!(surfaced.is_empty());
      assert!(notifications::list(&db, 50).await.unwrap().is_empty());
    }
  }
}
