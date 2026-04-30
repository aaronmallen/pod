//! Application service layer: business-logic helpers shared across controllers.

pub mod bootstrap;
pub mod character;
pub mod corporation;
pub mod sde;
pub mod window_state;

/// Shared database and ESI client handles passed to controllers.
#[derive(Clone)]
pub struct Services {
  pub db: Option<pod_db::Repo>,
  pub esi_client: Option<pod_esi::Client>,
}
