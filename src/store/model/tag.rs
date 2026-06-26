use getset::{CopyGetters, Getters};
use sqlx::FromRow;

/// Canonical tag-name key for case-insensitive, trimmed comparison. This mirrors the store's actual
/// source of truth: the `uq_tags_scope_lower_name` unique index and the repo's find-or-create /
/// rename guard both key on `(scope, lower(name))`, where `lower` is SQLite's built-in — which folds
/// ASCII case only. Matching that exactly (trim + ASCII case-fold) is what keeps every in-memory
/// fast-path guard in lock-step with what the database will accept or reject; a Unicode `to_lowercase`
/// here would re-introduce drift in the opposite direction (the guard rejecting a name the store would
/// happily insert as a distinct row). The single shared helper also replaces the previous mix of
/// ad-hoc `eq_ignore_ascii_case` and `to_lowercase` call sites so they can no longer disagree.
pub fn normalize_name(name: &str) -> String {
  name.trim().to_ascii_lowercase()
}

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  color: Option<String>,
  #[getset(get_copy = "pub")]
  created_at: i64,
  #[getset(get = "pub")]
  description: Option<String>,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  name: String,
  #[getset(get_copy = "pub")]
  position: i64,
  #[getset(get_copy = "pub")]
  updated_at: i64,
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::normalize_name;

  #[test]
  fn normalize_name_trims_and_ascii_lowercases() {
    assert_eq!(normalize_name("  Hauler  "), "hauler");
    assert_eq!(normalize_name("ROLLER"), "roller");
  }

  #[test]
  fn normalize_name_matches_repo_find_or_create_semantics() {
    // Mirrors `WHERE lower(name) = lower(?)` (SQLite ASCII `lower`) over a trimmed UI input:
    // ASCII-case- and whitespace-insensitive, so "Roller" and "  roller " collapse to one key.
    assert_eq!(normalize_name("Roller"), normalize_name("  roller "));
  }

  #[test]
  fn normalize_name_folds_only_ascii_to_stay_in_lock_step_with_sqlite_lower() {
    // SQLite's `lower()` (and thus the unique index + repo guard) leaves non-ASCII bytes untouched, so
    // the helper must NOT Unicode-fold — otherwise it would treat "Étagère"/"étagère" as one name while
    // the store stores them as two distinct rows, and the in-memory guard would diverge from reality.
    assert_ne!(normalize_name("Étagère"), normalize_name("étagère"));
  }
}
