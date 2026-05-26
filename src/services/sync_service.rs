//! Background synchronisation service.
//!
//! `SyncService` drives continuous ESI polling for all enrolled characters.
//! It runs as an iced subscription that lives for the lifetime of the app,
//! re-fetching every endpoint once its `x-cached-seconds` timer has elapsed
//! and writing results to the database while emitting `SyncEvent` messages
//! into the iced update loop.
//!
//! Scheduling is rate-limit-aware: each `(SyncDataType, character_id)` pair
//! tracks a `next_allowed_at` instant derived from the ESI
//! `x-cached-seconds` response header. An endpoint is never called before
//! that instant elapses.

use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use iced::{Subscription, futures::SinkExt as _};
use pod_model::Character;
use tracing::Instrument as _;

/// Back-off applied after a 5xx server error or rate-limit (420/429) response.
const ERROR_BACKOFF_SECS: u64 = 60;

/// How long the scheduler sleeps between polling cycles when all endpoints are
/// still within their cache window.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// ESI data categories that the sync service polls per character.
///
/// Each variant corresponds to a distinct ESI endpoint family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyncDataType {
  /// Character assets list (`esi-assets.read_assets`).
  CharacterAssets,
  /// Character location (`esi-location.read_location`).
  CharacterLocation,
  /// Character contracts (`esi-contracts.read_character_contracts`).
  Contracts,
  /// Character mail (`esi-mail.read_mail`).
  Mail,
  /// Character skill queue and trained skills (`esi-skills.read_skills`).
  Skills,
  /// Character wallet balance (`esi-wallet.read_character_wallet`).
  WalletBalance,
  /// Character wallet journal (`esi-wallet.read_character_wallet`).
  WalletJournal,
  /// Character wallet transactions (`esi-wallet.read_character_wallet`).
  WalletTransactions,
}

impl SyncDataType {
  /// Returns the human-readable display name emitted in `SyncEvent` payloads.
  pub fn human_label(self) -> &'static str {
    match self {
      Self::CharacterAssets => "Assets",
      Self::CharacterLocation => "Location",
      Self::Contracts => "Contracts",
      Self::Mail => "Mail",
      Self::Skills => "Skills",
      Self::WalletBalance => "Wallet Balance",
      Self::WalletJournal => "Wallet Journal",
      Self::WalletTransactions => "Wallet Transactions",
    }
  }

  /// Returns the string identifier emitted in `SyncEvent` payloads.
  pub fn label(self) -> &'static str {
    match self {
      Self::CharacterAssets => "character_assets",
      Self::CharacterLocation => "character_location",
      Self::Contracts => "contracts",
      Self::Mail => "mail",
      Self::Skills => "skills",
      Self::WalletBalance => "wallet_balance",
      Self::WalletJournal => "wallet_journal",
      Self::WalletTransactions => "wallet_transactions",
    }
  }

  /// Returns all data types that the scheduler polls each tick.
  fn all() -> &'static [SyncDataType] {
    &[
      SyncDataType::CharacterAssets,
      SyncDataType::CharacterLocation,
      SyncDataType::Contracts,
      SyncDataType::Mail,
      SyncDataType::Skills,
      SyncDataType::WalletBalance,
      SyncDataType::WalletJournal,
      SyncDataType::WalletTransactions,
    ]
  }
}

/// Events emitted by `SyncService` and routed through the iced message loop.
#[derive(Clone, Debug)]
pub enum SyncEvent {
  /// A sync operation completed successfully.
  Completed {
    /// Character whose data was synced.
    character_id: i64,
    /// Timestamp at which the operation finished.
    completed_at: Instant,
    /// ESI data category that was synced (e.g. `"wallet_balance"`).
    data_type: String,
    /// When the next sync for this endpoint is allowed.
    next_allowed_at: Instant,
  },
  /// A sync operation failed with an ESI error.
  Error {
    /// Character the sync was attempted for.
    character_id: i64,
    /// ESI data category that failed.
    data_type: String,
    /// `true` when the failure was an ESI 5xx server error;
    /// `false` for rate-limit or client errors.
    is_server_error: bool,
  },
  /// A sync operation is currently in progress.
  InFlight {
    /// Character the sync is running for.
    character_id: i64,
    /// ESI data category currently being fetched.
    data_type: String,
  },
}

/// Tracks live sync activity for display in the status bar.
pub struct SyncRegistry {
  /// Whether the most recent error was an ESI 5xx server error.
  pub has_server_error: bool,
  /// ESI data type names currently being fetched.
  pub in_flight: Vec<String>,
  /// Timestamp of the most recent successful sync completion.
  pub last_synced_at: Option<Instant>,
}

impl SyncRegistry {
  /// Creates a new empty registry with no recorded activity.
  pub fn new() -> Self {
    Self {
      has_server_error: false,
      in_flight: Vec::new(),
      last_synced_at: None,
    }
  }

  /// Applies a `SyncEvent` to update the registry state.
  pub fn update(&mut self, event: SyncEvent) {
    match event {
      SyncEvent::Completed {
        completed_at,
        data_type,
        ..
      } => {
        self.in_flight.retain(|t| *t != data_type);
        self.last_synced_at = Some(completed_at);
      }
      SyncEvent::Error {
        data_type,
        is_server_error,
        ..
      } => {
        self.in_flight.retain(|t| *t != data_type);
        if is_server_error {
          self.has_server_error = true;
        }
      }
      SyncEvent::InFlight {
        data_type, ..
      } => {
        if !self.in_flight.contains(&data_type) {
          self.in_flight.push(data_type);
        }
      }
    }
  }
}

impl Default for SyncRegistry {
  fn default() -> Self {
    Self::new()
  }
}

/// Key identifying a single (endpoint, character) polling slot.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SlotKey {
  character_id: i64,
  data_type: SyncDataType,
}

/// Per-slot scheduling state.
#[derive(Debug)]
struct SlotState {
  /// Earliest instant at which the next fetch is allowed.
  next_allowed_at: Instant,
}

/// Shared scheduler state, wrapped in `Arc<Mutex<…>>` so the subscription
/// future and `force_refresh_all` can both access it.
#[derive(Debug, Default)]
struct SchedulerState {
  slots: HashMap<SlotKey, SlotState>,
}

/// Bundle of shared data passed into the scheduler subscription recipe.
#[derive(Clone)]
struct WorkerBundle {
  characters: Arc<Mutex<Vec<Character>>>,
  client: pod_esi::Client,
  db: Arc<pod_db::Repo>,
  scheduler: Arc<Mutex<SchedulerState>>,
}

impl SchedulerState {
  /// Returns `true` when `key` is due for a fetch (or has never been fetched).
  fn is_due(&self, key: &SlotKey) -> bool {
    self
      .slots
      .get(key)
      .map(|s| Instant::now() >= s.next_allowed_at)
      .unwrap_or(true)
  }

  /// Records a successful fetch, setting `next_allowed_at` from the ESI
  /// `x-cached-seconds` value.
  fn mark_completed(&mut self, key: SlotKey, cached_secs: u64) {
    let next = Instant::now() + Duration::from_secs(cached_secs);
    self.slots.insert(
      key,
      SlotState {
        next_allowed_at: next,
      },
    );
  }

  /// Records a failed fetch with a back-off before the next attempt.
  fn mark_failed(&mut self, key: SlotKey, backoff_secs: u64) {
    let next = Instant::now() + Duration::from_secs(backoff_secs);
    self.slots.insert(
      key,
      SlotState {
        next_allowed_at: next,
      },
    );
  }

  /// Resets all `next_allowed_at` timestamps to `now`, making every slot
  /// immediately due on the next scheduler tick.
  fn reset_all(&mut self) {
    let now = Instant::now();
    for slot in self.slots.values_mut() {
      slot.next_allowed_at = now;
    }
  }
}

/// Drives background ESI synchronisation for enrolled entities.
///
/// Construct once at app startup and call `subscription()` from the iced
/// `subscription` function. Use `force_refresh_all()` when the user requests
/// an immediate re-sync.
pub struct SyncService {
  characters: Arc<Mutex<Vec<Character>>>,
  db: Option<Arc<pod_db::Repo>>,
  esi_client: Option<pod_esi::Client>,
  scheduler: Arc<Mutex<SchedulerState>>,
}

impl SyncService {
  /// Creates a new `SyncService` with no enrolled characters.
  pub fn new() -> Self {
    Self {
      characters: Arc::new(Mutex::new(Vec::new())),
      db: None,
      esi_client: None,
      scheduler: Arc::new(Mutex::new(SchedulerState::default())),
    }
  }

  /// Attaches the database repository, enabling DB writes from sync tasks.
  ///
  /// Called after bootstrap finishes opening the database.
  pub fn set_db(&mut self, db: pod_db::Repo) {
    self.db = Some(Arc::new(db));
  }

  /// Replaces the enrolled character list.
  ///
  /// Called after bootstrap finishes loading characters from the database so
  /// that the scheduler knows which characters to poll.
  pub fn set_characters(&mut self, characters: Vec<Character>) {
    let mut guard = self.characters.lock().expect("characters mutex poisoned");
    *guard = characters;
  }

  /// Attaches an ESI client, enabling live network calls.
  ///
  /// Without a client the subscription emits no events.
  pub fn set_esi_client(&mut self, client: pod_esi::Client) {
    self.esi_client = Some(client);
  }

  /// Requests an immediate re-sync of all enrolled entities by resetting
  /// every `next_allowed_at` timestamp to now.
  pub fn force_refresh_all(&self) {
    let mut guard = self.scheduler.lock().expect("scheduler mutex poisoned");
    guard.reset_all();
  }

  /// Returns an iced subscription that continuously polls ESI endpoints.
  ///
  /// The subscription runs for the lifetime of the app and emits `SyncEvent`
  /// messages into the iced update loop. When no ESI client or DB is
  /// available the subscription completes immediately without emitting anything.
  pub fn subscription(&self) -> Subscription<SyncEvent> {
    let Some(client) = self.esi_client.clone() else {
      return Subscription::none();
    };
    let Some(db) = self.db.clone() else {
      return Subscription::none();
    };
    iced::advanced::subscription::from_recipe(SyncRecipe {
      bundle: WorkerBundle {
        characters: Arc::clone(&self.characters),
        client,
        db,
        scheduler: Arc::clone(&self.scheduler),
      },
    })
  }
}

impl Default for SyncService {
  fn default() -> Self {
    Self::new()
  }
}

/// Iced subscription recipe that drives the background scheduler loop.
struct SyncRecipe {
  bundle: WorkerBundle,
}

impl iced::advanced::subscription::Recipe for SyncRecipe {
  type Output = SyncEvent;

  fn hash(&self, state: &mut iced::advanced::subscription::Hasher) {
    use std::hash::Hash as _;
    "sync_service_recipe".hash(state);
  }

  fn stream(
    self: Box<Self>,
    _input: iced::advanced::subscription::EventStream,
  ) -> iced::futures::stream::BoxStream<'static, SyncEvent> {
    use iced::futures::StreamExt as _;
    let bundle = self.bundle;
    iced::stream::channel(256, async move |mut tx| {
      loop {
        let events = run_tick(&bundle.client, &bundle.characters, &bundle.scheduler, &bundle.db).await;
        for event in events {
          if tx.send(event).await.is_err() {
            return;
          }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
      }
    })
    .boxed()
  }
}

/// Runs a single scheduler tick: finds all due slots, fans out fetches in
/// parallel, and returns the resulting events.
async fn run_tick(
  client: &pod_esi::Client,
  characters: &Mutex<Vec<Character>>,
  scheduler: &Mutex<SchedulerState>,
  db: &pod_db::Repo,
) -> Vec<SyncEvent> {
  let chars = characters.lock().expect("characters mutex poisoned").clone();
  if chars.is_empty() {
    return Vec::new();
  }

  let due: Vec<(i64, String, SyncDataType)> = {
    let sched = scheduler.lock().expect("scheduler mutex poisoned");
    chars
      .iter()
      .filter(|c| !c.access_token().is_empty())
      .flat_map(|c| {
        SyncDataType::all().iter().filter_map(|&dt| {
          let key = SlotKey {
            character_id: *c.id(),
            data_type: dt,
          };
          if sched.is_due(&key) {
            Some((*c.id(), c.access_token().clone(), dt))
          } else {
            None
          }
        })
      })
      .collect()
  };

  if due.is_empty() {
    return Vec::new();
  }

  let mut in_flight_events: Vec<SyncEvent> = due
    .iter()
    .map(|(char_id, _, dt)| SyncEvent::InFlight {
      character_id: *char_id,
      data_type: dt.human_label().to_string(),
    })
    .collect();

  let handles: Vec<_> = due
    .into_iter()
    .map(|(char_id, token, dt)| {
      let client = client.clone();
      let db = db.clone();
      tokio::spawn(
        async move {
          let result = fetch_slot(&client, &db, char_id, token, dt).await;
          (char_id, dt, result)
        }
        .in_current_span(),
      )
    })
    .collect();

  let mut result_events: Vec<SyncEvent> = Vec::new();
  for handle in handles {
    let Ok((char_id, dt, outcome)) = handle.await else {
      tracing::error!("sync: fetch task panicked or was cancelled");
      continue;
    };
    let key = SlotKey {
      character_id: char_id,
      data_type: dt,
    };
    match outcome {
      SlotOutcome::Success {
        cached_secs,
      } => {
        let next = Instant::now() + Duration::from_secs(cached_secs);
        {
          let mut sched = scheduler.lock().expect("scheduler mutex poisoned");
          sched.mark_completed(key, cached_secs);
        }
        result_events.push(SyncEvent::Completed {
          character_id: char_id,
          completed_at: Instant::now(),
          data_type: dt.human_label().to_string(),
          next_allowed_at: next,
        });
      }
      SlotOutcome::RateLimit {
        retry_after_secs,
      } => {
        {
          let mut sched = scheduler.lock().expect("scheduler mutex poisoned");
          sched.mark_failed(key, retry_after_secs);
        }
        result_events.push(SyncEvent::Error {
          character_id: char_id,
          data_type: dt.human_label().to_string(),
          is_server_error: false,
        });
      }
      SlotOutcome::AuthExpired => {
        {
          let mut sched = scheduler.lock().expect("scheduler mutex poisoned");
          sched.mark_failed(key, ERROR_BACKOFF_SECS);
        }
        result_events.push(SyncEvent::Error {
          character_id: char_id,
          data_type: dt.human_label().to_string(),
          is_server_error: false,
        });
      }
      SlotOutcome::ServerError => {
        {
          let mut sched = scheduler.lock().expect("scheduler mutex poisoned");
          sched.mark_failed(key, ERROR_BACKOFF_SECS);
        }
        result_events.push(SyncEvent::Error {
          character_id: char_id,
          data_type: dt.human_label().to_string(),
          is_server_error: true,
        });
      }
    }
  }

  in_flight_events.extend(result_events);
  in_flight_events
}

/// Outcome of fetching a single (endpoint, character) slot.
#[derive(Debug)]
enum SlotOutcome {
  /// The access token has expired or ESI returned 401/403.
  AuthExpired,
  /// ESI returned a 420 or 429 rate-limit response.
  RateLimit { retry_after_secs: u64 },
  /// ESI returned a 5xx server error.
  ServerError,
  /// Fetch succeeded; ESI reported the cache TTL in seconds.
  Success { cached_secs: u64 },
}

/// Fetches a single slot and maps the ESI result to a `SlotOutcome`.
async fn fetch_slot(
  client: &pod_esi::Client,
  db: &pod_db::Repo,
  character_id: i64,
  token: String,
  data_type: SyncDataType,
) -> SlotOutcome {
  match data_type {
    SyncDataType::CharacterAssets => fetch_character_assets(client, db, character_id, token).await,
    SyncDataType::CharacterLocation => fetch_character_location(client, character_id, token).await,
    SyncDataType::Contracts => fetch_contracts(client, db, character_id, token).await,
    SyncDataType::Mail => fetch_mail(client, db, character_id, token).await,
    SyncDataType::Skills => fetch_skills(client, db, character_id, token).await,
    SyncDataType::WalletBalance => fetch_wallet_balance(client, db, character_id, token).await,
    SyncDataType::WalletJournal => fetch_wallet_journal(client, db, character_id, token).await,
    SyncDataType::WalletTransactions => fetch_wallet_transactions(client, db, character_id, token).await,
  }
}

/// Builds a [`pod_esi::models::auth::Grant`] for the given character and token.
fn make_grant(token: String, character_id: i64) -> pod_esi::models::auth::Grant {
  use std::time::{Duration, SystemTime};

  pod_esi::models::auth::Grant::new(
    token,
    character_id,
    "",
    SystemTime::now() + Duration::from_secs(3600),
    "",
    Vec::new(),
  )
}

/// Maps a `pod_esi::Error` to a `SlotOutcome`.
fn map_esi_error(err: pod_esi::Error) -> SlotOutcome {
  match err {
    pod_esi::Error::RateLimit {
      retry_after_secs,
    } => SlotOutcome::RateLimit {
      retry_after_secs,
    },
    pod_esi::Error::Api {
      status, ..
    } if status == 401 || status == 403 => SlotOutcome::AuthExpired,
    pod_esi::Error::Api {
      status, ..
    } if status >= 500 => SlotOutcome::ServerError,
    _ => SlotOutcome::ServerError,
  }
}

/// Fetches character assets and writes them to the DB.
async fn fetch_character_assets(
  client: &pod_esi::Client,
  db: &pod_db::Repo,
  character_id: i64,
  token: String,
) -> SlotOutcome {
  use pod_model::CharacterAsset;

  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.character_assets", character_id = character_id);
  let result = client.character(&grant).assets().instrument(span).await;

  match result {
    Ok(raw) => {
      let assets: Vec<CharacterAsset> = raw
        .into_iter()
        .map(|a| CharacterAsset {
          character_id,
          is_blueprint_copy: a.is_blueprint_copy,
          is_singleton: a.is_singleton,
          item_id: a.item_id,
          location_flag: a.location_flag,
          location_id: a.location_id,
          location_type: a.location_type,
          quantity: a.quantity,
          type_id: a.type_id,
          ..Default::default()
        })
        .collect();
      let keep_ids: Vec<i64> = assets.iter().map(|a| a.item_id).collect();
      if let Err(e) = db
        .character_assets()
        .upsert_character_assets(character_id, &assets)
        .await
      {
        tracing::warn!("sync: failed to persist assets for character {character_id}: {e}");
      }
      if let Err(e) = db
        .character_assets()
        .delete_stale_character_assets(character_id, &keep_ids)
        .await
      {
        tracing::warn!("sync: failed to delete stale assets for character {character_id}: {e}");
      }
      SlotOutcome::Success {
        cached_secs: 3600,
      }
    }
    Err(e) => map_esi_error(e),
  }
}

/// Fetches the character's current location (read-only; no DB write for now).
async fn fetch_character_location(client: &pod_esi::Client, character_id: i64, token: String) -> SlotOutcome {
  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.character_location", character_id = character_id);
  let result = client.character(&grant).location().instrument(span).await;

  match result {
    Ok(_location) => SlotOutcome::Success {
      cached_secs: 5,
    },
    Err(e) => map_esi_error(e),
  }
}

/// Fetches character contracts and writes them to the DB.
async fn fetch_contracts(client: &pod_esi::Client, db: &pod_db::Repo, character_id: i64, token: String) -> SlotOutcome {
  use pod_model::CharacterContract;

  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.contracts", character_id = character_id);
  let result = client.character(&grant).contracts().instrument(span).await;

  match result {
    Ok(raw) => {
      let contracts: Vec<CharacterContract> = raw
        .into_iter()
        .map(|c| CharacterContract {
          acceptor_id: c.acceptor_id,
          assignee_id: c.assignee_id,
          character_id,
          collateral: c.collateral,
          contract_id: c.contract_id,
          contract_type: c.r#type,
          date_expired: c.date_expired,
          date_issued: c.date_issued,
          issuer_id: c.issuer_id,
          price: c.price,
          start_location_id: c.start_location_id,
          status: c.status,
          title: c.title.unwrap_or_default(),
        })
        .collect();
      if let Err(e) = db.wallet().upsert_contracts(character_id, &contracts).await {
        tracing::warn!("sync: failed to persist contracts for character {character_id}: {e}");
      }
      SlotOutcome::Success {
        cached_secs: 300,
      }
    }
    Err(e) => map_esi_error(e),
  }
}

/// Fetches mail headers and writes them to the DB.
async fn fetch_mail(client: &pod_esi::Client, db: &pod_db::Repo, character_id: i64, token: String) -> SlotOutcome {
  use pod_model::MailHeader;

  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.mail", character_id = character_id);
  let result = client.character(&grant).mail().instrument(span).await;

  match result {
    Ok(raw) => {
      let headers: Vec<MailHeader> = raw
        .into_iter()
        .filter_map(|h| {
          let mail_id = h.mail_id?;
          let timestamp = h.timestamp?;
          Some(MailHeader {
            body: None,
            character_id,
            from_id: h.from,
            is_read: h.is_read.unwrap_or(false),
            mail_id,
            preview: None,
            recipients_display: h
              .recipients
              .as_deref()
              .unwrap_or_default()
              .iter()
              .map(|r| r.recipient_id.to_string())
              .collect::<Vec<_>>()
              .join(", "),
            subject: h.subject.unwrap_or_default(),
            timestamp,
          })
        })
        .collect();
      if let Err(e) = db.mail().upsert_mail_headers(character_id, &headers).await {
        tracing::warn!("sync: failed to persist mail headers for character {character_id}: {e}");
      }
      SlotOutcome::Success {
        cached_secs: 30,
      }
    }
    Err(e) => map_esi_error(e),
  }
}

/// Fetches character skills and writes them to the DB.
async fn fetch_skills(client: &pod_esi::Client, db: &pod_db::Repo, character_id: i64, token: String) -> SlotOutcome {
  use crate::services::character as character_service;

  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.skills", character_id = character_id);
  let result = client.character(&grant).skills().instrument(span).await;

  match result {
    Ok(esi_skills) => {
      let skills = character_service::build_character_skills(character_id, esi_skills.skills, vec![]);
      if let Err(e) = db.skills().upsert_character_skills(character_id, &skills).await {
        tracing::warn!("sync: failed to persist skills for character {character_id}: {e}");
      }
      SlotOutcome::Success {
        cached_secs: 120,
      }
    }
    Err(e) => map_esi_error(e),
  }
}

/// Fetches the wallet balance for `character_id` and writes it to the DB.
///
/// Uses a default cache TTL of 120 seconds (the documented ESI value for the
/// wallet balance endpoint).
async fn fetch_wallet_balance(
  client: &pod_esi::Client,
  db: &pod_db::Repo,
  character_id: i64,
  token: String,
) -> SlotOutcome {
  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.wallet_balance", character_id = character_id);
  let result = client.character(&grant).wallet_balance().instrument(span).await;

  match result {
    Ok(balance) => {
      if let Err(e) = db.characters().update_wallet(character_id, Some(balance.0)).await {
        tracing::warn!("sync: failed to persist wallet balance for character {character_id}: {e}");
      }
      SlotOutcome::Success {
        cached_secs: 120,
      }
    }
    Err(e) => map_esi_error(e),
  }
}

/// Fetches the wallet journal for `character_id` and writes entries to the DB.
async fn fetch_wallet_journal(
  client: &pod_esi::Client,
  db: &pod_db::Repo,
  character_id: i64,
  token: String,
) -> SlotOutcome {
  use pod_model::WalletJournalEntry;

  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.wallet_journal", character_id = character_id);
  let result = client.character(&grant).wallet_journal().instrument(span).await;

  match result {
    Ok(raw) => {
      let entries: Vec<WalletJournalEntry> = raw
        .into_iter()
        .map(|e| WalletJournalEntry {
          amount: e.amount,
          balance: e.balance,
          character_id,
          date: e.date,
          description: e.description,
          entry_id: e.id,
          first_party_id: e.first_party_id,
          ref_type: e.ref_type,
          second_party_id: e.second_party_id,
        })
        .collect();
      if let Err(e) = db.wallet().upsert_journal_entries(character_id, &entries).await {
        tracing::warn!("sync: failed to persist wallet journal for character {character_id}: {e}");
      }
      SlotOutcome::Success {
        cached_secs: 3600,
      }
    }
    Err(e) => map_esi_error(e),
  }
}

/// Fetches wallet transactions for `character_id` and writes them to the DB.
async fn fetch_wallet_transactions(
  client: &pod_esi::Client,
  db: &pod_db::Repo,
  character_id: i64,
  token: String,
) -> SlotOutcome {
  use pod_model::WalletTransaction;

  let grant = make_grant(token, character_id);
  let span = tracing::debug_span!("sync.wallet_transactions", character_id = character_id);
  let result = client.character(&grant).wallet_transactions().instrument(span).await;

  match result {
    Ok(raw) => {
      let txns: Vec<WalletTransaction> = raw
        .into_iter()
        .map(|t| WalletTransaction {
          character_id,
          client_id: t.client_id,
          date: t.date,
          is_buy: t.is_buy,
          location_id: t.location_id,
          quantity: t.quantity,
          transaction_id: t.transaction_id,
          type_id: t.type_id,
          unit_price: t.unit_price,
        })
        .collect();
      if let Err(e) = db.wallet().upsert_wallet_transactions(character_id, &txns).await {
        tracing::warn!("sync: failed to persist wallet transactions for character {character_id}: {e}");
      }
      SlotOutcome::Success {
        cached_secs: 3600,
      }
    }
    Err(e) => map_esi_error(e),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod scheduler_state {
    use super::*;

    mod is_due {
      use super::*;

      #[test]
      fn it_returns_true_for_unknown_slot() {
        let state = SchedulerState::default();
        let key = SlotKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };

        assert!(state.is_due(&key));
      }

      #[test]
      fn it_returns_false_when_next_allowed_at_is_in_the_future() {
        let mut state = SchedulerState::default();
        let key = SlotKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        state.mark_completed(key.clone(), 3600);

        assert!(!state.is_due(&key));
      }

      #[test]
      fn it_returns_true_when_next_allowed_at_has_passed() {
        let mut state = SchedulerState::default();
        let key = SlotKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        state.slots.insert(
          key.clone(),
          SlotState {
            next_allowed_at: Instant::now() - Duration::from_secs(1),
          },
        );

        assert!(state.is_due(&key));
      }
    }

    mod mark_completed {
      use super::*;

      #[test]
      fn it_sets_next_allowed_at_in_the_future() {
        let mut state = SchedulerState::default();
        let key = SlotKey {
          character_id: 2,
          data_type: SyncDataType::WalletBalance,
        };

        state.mark_completed(key.clone(), 120);

        assert!(!state.is_due(&key));
      }
    }

    mod mark_failed {
      use super::*;

      #[test]
      fn it_sets_next_allowed_at_with_backoff() {
        let mut state = SchedulerState::default();
        let key = SlotKey {
          character_id: 3,
          data_type: SyncDataType::WalletBalance,
        };

        state.mark_failed(key.clone(), 60);

        assert!(!state.is_due(&key));
      }
    }

    mod reset_all {
      use super::*;

      #[test]
      fn it_makes_all_slots_immediately_due() {
        let mut state = SchedulerState::default();
        let key1 = SlotKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        let key2 = SlotKey {
          character_id: 2,
          data_type: SyncDataType::WalletBalance,
        };
        state.mark_completed(key1.clone(), 3600);
        state.mark_completed(key2.clone(), 3600);

        state.reset_all();

        assert!(state.is_due(&key1));
        assert!(state.is_due(&key2));
      }
    }
  }

  mod sync_registry {
    use super::*;

    mod update {
      use super::*;

      #[test]
      fn it_removes_data_type_from_in_flight_on_completed() {
        let mut reg = SyncRegistry::new();
        reg.in_flight.push("wallet_balance".to_string());

        reg.update(SyncEvent::Completed {
          character_id: 1,
          completed_at: Instant::now(),
          data_type: "wallet_balance".to_string(),
          next_allowed_at: Instant::now(),
        });

        assert!(reg.in_flight.is_empty());
      }

      #[test]
      fn it_sets_last_synced_at_on_completed() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Completed {
          character_id: 1,
          completed_at: Instant::now(),
          data_type: "wallet_balance".to_string(),
          next_allowed_at: Instant::now(),
        });

        assert!(reg.last_synced_at.is_some());
      }

      #[test]
      fn it_removes_data_type_from_in_flight_on_error() {
        let mut reg = SyncRegistry::new();
        reg.in_flight.push("wallet_balance".to_string());

        reg.update(SyncEvent::Error {
          character_id: 1,
          data_type: "wallet_balance".to_string(),
          is_server_error: false,
        });

        assert!(reg.in_flight.is_empty());
      }

      #[test]
      fn it_sets_has_server_error_on_server_error() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Error {
          character_id: 1,
          data_type: "wallet_balance".to_string(),
          is_server_error: true,
        });

        assert!(reg.has_server_error);
      }

      #[test]
      fn it_does_not_set_has_server_error_on_non_server_error() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Error {
          character_id: 1,
          data_type: "wallet_balance".to_string(),
          is_server_error: false,
        });

        assert!(!reg.has_server_error);
      }

      #[test]
      fn it_adds_data_type_to_in_flight() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::InFlight {
          character_id: 1,
          data_type: "wallet_balance".to_string(),
        });

        assert_eq!(reg.in_flight, vec!["wallet_balance"]);
      }

      #[test]
      fn it_does_not_add_duplicate_to_in_flight() {
        let mut reg = SyncRegistry::new();
        reg.in_flight.push("wallet_balance".to_string());

        reg.update(SyncEvent::InFlight {
          character_id: 1,
          data_type: "wallet_balance".to_string(),
        });

        assert_eq!(reg.in_flight.len(), 1);
      }
    }
  }

  mod sync_service {
    use super::*;

    mod force_refresh_all {
      use super::*;

      #[test]
      fn it_resets_all_scheduler_slots() {
        let service = SyncService::new();
        {
          let mut sched = service.scheduler.lock().unwrap();
          let key = SlotKey {
            character_id: 42,
            data_type: SyncDataType::WalletBalance,
          };
          sched.mark_completed(key, 3600);
        }

        service.force_refresh_all();

        let sched = service.scheduler.lock().unwrap();
        let key = SlotKey {
          character_id: 42,
          data_type: SyncDataType::WalletBalance,
        };
        assert!(sched.is_due(&key));
      }
    }
  }

  mod sync_data_type {
    use super::*;

    mod human_label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_assets_for_character_assets() {
        assert_eq!(SyncDataType::CharacterAssets.human_label(), "Assets");
      }

      #[test]
      fn it_returns_contracts_for_contracts() {
        assert_eq!(SyncDataType::Contracts.human_label(), "Contracts");
      }

      #[test]
      fn it_returns_location_for_character_location() {
        assert_eq!(SyncDataType::CharacterLocation.human_label(), "Location");
      }

      #[test]
      fn it_returns_mail_for_mail() {
        assert_eq!(SyncDataType::Mail.human_label(), "Mail");
      }

      #[test]
      fn it_returns_skills_for_skills() {
        assert_eq!(SyncDataType::Skills.human_label(), "Skills");
      }

      #[test]
      fn it_returns_wallet_balance_for_wallet_balance() {
        assert_eq!(SyncDataType::WalletBalance.human_label(), "Wallet Balance");
      }

      #[test]
      fn it_returns_wallet_journal_for_wallet_journal() {
        assert_eq!(SyncDataType::WalletJournal.human_label(), "Wallet Journal");
      }

      #[test]
      fn it_returns_wallet_transactions_for_wallet_transactions() {
        assert_eq!(SyncDataType::WalletTransactions.human_label(), "Wallet Transactions");
      }
    }

    mod label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_character_assets_label() {
        assert_eq!(SyncDataType::CharacterAssets.label(), "character_assets");
      }

      #[test]
      fn it_returns_character_location_label() {
        assert_eq!(SyncDataType::CharacterLocation.label(), "character_location");
      }

      #[test]
      fn it_returns_contracts_label() {
        assert_eq!(SyncDataType::Contracts.label(), "contracts");
      }

      #[test]
      fn it_returns_mail_label() {
        assert_eq!(SyncDataType::Mail.label(), "mail");
      }

      #[test]
      fn it_returns_skills_label() {
        assert_eq!(SyncDataType::Skills.label(), "skills");
      }

      #[test]
      fn it_returns_wallet_balance_label() {
        assert_eq!(SyncDataType::WalletBalance.label(), "wallet_balance");
      }

      #[test]
      fn it_returns_wallet_journal_label() {
        assert_eq!(SyncDataType::WalletJournal.label(), "wallet_journal");
      }

      #[test]
      fn it_returns_wallet_transactions_label() {
        assert_eq!(SyncDataType::WalletTransactions.label(), "wallet_transactions");
      }
    }

    mod all {
      use super::*;

      #[test]
      fn it_returns_all_eight_variants() {
        assert_eq!(SyncDataType::all().len(), 8);
      }
    }
  }
}
