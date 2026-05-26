//! Application service layer: business-logic helpers shared across controllers.

pub mod abyssals;
pub mod bootstrap;
pub mod cache_cleaner;
pub mod character;
pub mod corporation;
pub mod menu;
pub mod muta_market;
pub mod oauth_callback;
pub mod portraits;
pub mod prices;
pub mod sde;
pub mod sync_service;
pub mod sync_state;
pub mod updater;
pub mod window_state;

/// Shared database, ESI client, and application config handles passed
/// to controllers.
#[derive(Clone)]
pub struct Services {
  pub config: crate::config::Settings,
  pub db: Option<pod_db::Repo>,
  pub esi_client: Option<pod_esi::Client>,
  pub oauth_callback_tx: tokio::sync::broadcast::Sender<(String, String)>,
}
