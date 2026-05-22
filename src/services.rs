//! Application service layer: business-logic helpers shared across controllers.

pub mod bootstrap;
pub mod cache_cleaner;
pub mod character;
pub mod corporation;
pub mod menu;
pub mod portraits;
pub mod prices;
pub mod sde;
pub mod updater;
pub mod window_state;

/// Shared database, ESI client, and application config handles passed
/// to controllers.
#[derive(Clone)]
pub struct Services {
  pub config: crate::config::Settings,
  pub db: Option<pod_db::Repo>,
  pub esi_client: Option<pod_esi::Client>,
}
