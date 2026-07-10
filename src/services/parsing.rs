//! Pure, synchronous parsers for the paste formats the app ingests (EFT fits, ship and cargo
//! scans, skill plans, multibuy lists), plus a backend-agnostic name-to-type_id `Resolver` for
//! the one step that needs IO. Mirrors `pod_pack`: stateless, no DB or ESI in the parse path.
//! `dispatch::try_parse` sniffs a blob and routes it to the right format.

#[cfg_attr(not(test), allow(dead_code))]
pub mod dispatch;
pub mod eft;
pub mod level;
pub mod quantity;
pub mod resolve;
pub mod sanitize;
