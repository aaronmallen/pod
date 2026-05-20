//! Application service layer: business-logic helpers shared across controllers.

pub mod bootstrap;
pub mod cache_cleaner;
pub mod character;
pub mod corporation;
pub mod menu;
pub mod sde;
pub mod updater;
pub mod window_state;

/// Shared database and ESI client handles passed to controllers.
#[derive(Clone)]
pub struct Services {
  pub db: Option<pod_db::Repo>,
  pub esi_client: Option<pod_esi::Client>,
}
