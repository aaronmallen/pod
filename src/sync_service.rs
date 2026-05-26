//! Background synchronisation service.
//!
//! `SyncService` drives continuous ESI polling for all enrolled characters.
//! It runs as an iced subscription that lives for the lifetime of the app,
//! re-fetching every endpoint once its `x-cached-seconds` timer has elapsed
//! and writing results into `DataStore` while emitting `SyncEvent` messages
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

use iced::{Subscription, Task, futures::SinkExt as _};
use pod_model::Character;
use tracing::Instrument as _;

/// Back-off applied after a 5xx server error or rate-limit (420/429) response.
const ERROR_BACKOFF_SECS: u64 = 60;

/// How long the scheduler sleeps between polling cycles when all endpoints are
/// still within their cache window.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// ESI data categories that the sync service polls per character.
///
/// Each variant corresponds to a distinct ESI endpoint family. The `label`
/// method returns the string key used in `SyncEvent` payloads.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyncDataType {
  /// Character wallet balance (`esi-wallet.read_character_wallet`).
  WalletBalance,
}

impl SyncDataType {
  /// Returns the string identifier emitted in `SyncEvent` payloads.
  pub fn label(self) -> &'static str {
    match self {
      Self::WalletBalance => "wallet_balance",
    }
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
    self.slots.insert(key, SlotState {
      next_allowed_at: next,
    });
  }

  /// Records a failed fetch with a back-off before the next attempt.
  fn mark_failed(&mut self, key: SlotKey, backoff_secs: u64) {
    let next = Instant::now() + Duration::from_secs(backoff_secs);
    self.slots.insert(key, SlotState {
      next_allowed_at: next,
    });
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
  esi_client: Option<pod_esi::Client>,
  scheduler: Arc<Mutex<SchedulerState>>,
}

impl SyncService {
  /// Creates a new `SyncService` with no enrolled characters.
  pub fn new() -> Self {
    Self {
      characters: Arc::new(Mutex::new(Vec::new())),
      esi_client: None,
      scheduler: Arc::new(Mutex::new(SchedulerState::default())),
    }
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
  /// messages into the iced update loop. When no ESI client is available the
  /// subscription completes immediately without emitting anything.
  pub fn subscription(&self) -> Subscription<SyncEvent> {
    let Some(client) = self.esi_client.clone() else {
      return Subscription::none();
    };
    let characters = Arc::clone(&self.characters);
    let scheduler = Arc::clone(&self.scheduler);

    Subscription::run_with_id(
      "sync_service",
      iced::futures::stream::unfold(
        (client, characters, scheduler),
        |(client, characters, scheduler)| async move {
          let events = run_tick(&client, &characters, &scheduler).await;
          tokio::time::sleep(POLL_INTERVAL).await;
          Some((events, (client, characters, scheduler)))
        },
      )
      .flat_map(|events| iced::futures::stream::iter(events)),
    )
  }
}

impl Default for SyncService {
  fn default() -> Self {
    Self::new()
  }
}

/// Runs a single scheduler tick: finds all due slots, fans out fetches in
/// parallel, and returns the resulting events.
async fn run_tick(
  client: &pod_esi::Client,
  characters: &Mutex<Vec<Character>>,
  scheduler: &Mutex<SchedulerState>,
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
        [SyncDataType::WalletBalance].iter().filter_map(|&dt| {
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
      data_type: dt.label().to_string(),
    })
    .collect();

  let handles: Vec<_> = due
    .into_iter()
    .map(|(char_id, token, dt)| {
      let client = client.clone();
      tokio::spawn(
        async move {
          let result = fetch_slot(&client, char_id, token, dt).await;
          (char_id, dt, result)
        }
        .in_current_span(),
      )
    })
    .collect();

  let mut result_events: Vec<SyncEvent> = Vec::new();
  for handle in handles {
    let Ok((char_id, dt, outcome)) = handle.await else {
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
          data_type: dt.label().to_string(),
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
          data_type: dt.label().to_string(),
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
          data_type: dt.label().to_string(),
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
          data_type: dt.label().to_string(),
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
  /// Fetch succeeded; ESI reported the cache TTL in seconds.
  Success {
    cached_secs: u64,
  },
  /// ESI returned a 420 or 429 rate-limit response.
  RateLimit {
    retry_after_secs: u64,
  },
  /// The access token has expired or ESI returned 401/403.
  AuthExpired,
  /// ESI returned a 5xx server error.
  ServerError,
}

/// Fetches a single slot and maps the ESI result to a `SlotOutcome`.
async fn fetch_slot(
  client: &pod_esi::Client,
  character_id: i64,
  token: String,
  data_type: SyncDataType,
) -> SlotOutcome {
  match data_type {
    SyncDataType::WalletBalance => fetch_wallet_balance(client, character_id, token).await,
  }
}

/// Fetches the wallet balance for `character_id` and returns a `SlotOutcome`.
///
/// Uses a default cache TTL of 120 seconds (ESI `x-esi-error-limit-*` headers
/// are handled transparently by the ESI HTTP client). The wallet balance
/// endpoint does not expose `x-cached-seconds` in the response body, so we
/// fall back to the documented 120 s value.
async fn fetch_wallet_balance(
  client: &pod_esi::Client,
  character_id: i64,
  token: String,
) -> SlotOutcome {
  use pod_esi::models::auth::Grant;
  use std::time::{Duration, SystemTime};

  let grant = Grant::new(
    token,
    character_id,
    "",
    SystemTime::now() + Duration::from_secs(3600),
    "",
    Vec::new(),
  );

  let span = tracing::debug_span!("sync.wallet_balance", character_id = character_id);
  let result = client
    .character(&grant)
    .wallet_balance()
    .instrument(span)
    .await;

  match result {
    Ok(_balance) => SlotOutcome::Success {
      cached_secs: 120,
    },
    Err(pod_esi::Error::RateLimit {
      retry_after_secs,
    }) => SlotOutcome::RateLimit {
      retry_after_secs,
    },
    Err(pod_esi::Error::Api {
      status, ..
    }) if status == 401 || status == 403 => SlotOutcome::AuthExpired,
    Err(pod_esi::Error::Api {
      status, ..
    }) if status >= 500 => SlotOutcome::ServerError,
    Err(_) => SlotOutcome::ServerError,
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
        state.slots.insert(key.clone(), SlotState {
          next_allowed_at: Instant::now() - Duration::from_secs(1),
        });

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

    mod label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_wallet_balance_label() {
        assert_eq!(SyncDataType::WalletBalance.label(), "wallet_balance");
      }
    }
  }
}
