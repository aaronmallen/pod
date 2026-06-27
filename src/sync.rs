mod calendar_handlers;
mod command;
mod contact_handlers;
mod drain;
mod engine;
mod event;
mod freshness;
mod handle;
mod job;
mod jobs;
mod language_refresh;
mod mail_handlers;
mod outbox;
mod outcome;
mod schedule;
mod status;
mod structure_resolution;
mod subject;
pub mod token;

#[cfg(test)]
pub use command::Command;
pub use engine::spawn;
pub use event::Event;
pub use freshness::{Freshness, FreshnessSummary, freshness_of};
pub use handle::Handle;
pub use job::{JobKey, JobKind};
// The boot-time language-switch re-sync hook is consumed by the app's LanguageChanged restart flow
// (sibling task wzkmsqyk); it reads as unused until that wiring lands.
#[allow(unused_imports)]
pub use language_refresh::{Refresh, refresh_for_language_switch};
// Outcome is re-exported only for test fixtures (`outcome: sync::Outcome::synced()`); no production reader yet.
#[allow(unused_imports)]
pub use outcome::Outcome;
pub use status::{OutboxStatus, Phase, SyncStatus};
pub use structure_resolution::resolve_stockpile_location;
pub use subject::Subject;
