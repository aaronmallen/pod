mod language;

// Re-exported for the sibling i18n tasks (config field, ESI injection, SDE re-seed) that consume it;
// nothing outside this module references it yet, so it reads as unused until those land.
#[allow(unused_imports)]
pub use language::Language;
