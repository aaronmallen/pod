#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
  Blocked { reason: String },
  Empty,
  Failed { reason: String },
  NotReady,
  Skipped { reason: String },
  Synced { rows_touched: i64 },
}

impl Outcome {
  pub fn from_rows(rows: usize) -> Self {
    if rows == 0 {
      Self::Empty
    } else {
      Self::Synced {
        rows_touched: rows as i64,
      }
    }
  }

  pub fn synced() -> Self {
    Self::Synced {
      rows_touched: 0,
    }
  }

  pub fn label(&self) -> &'static str {
    match self {
      Self::Blocked {
        ..
      } => "blocked",
      Self::Empty => "empty",
      Self::Failed {
        ..
      } => "failed",
      Self::NotReady => "not_ready",
      Self::Skipped {
        ..
      } => "skipped",
      Self::Synced {
        ..
      } => "synced",
    }
  }

  pub fn reason(&self) -> Option<&str> {
    match self {
      Self::Blocked {
        reason,
      }
      | Self::Failed {
        reason,
      }
      | Self::Skipped {
        reason,
      } => Some(reason),
      Self::Empty
      | Self::NotReady
      | Self::Synced {
        ..
      } => None,
    }
  }

  pub fn rows_touched(&self) -> i64 {
    match self {
      Self::Synced {
        rows_touched,
      } => *rows_touched,
      Self::Blocked {
        ..
      }
      | Self::Empty
      | Self::Failed {
        ..
      }
      | Self::NotReady
      | Self::Skipped {
        ..
      } => 0,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_variant_to_its_ledger_token() {
      assert_eq!(
        Outcome::Synced {
          rows_touched: 3
        }
        .label(),
        "synced"
      );
      assert_eq!(Outcome::Empty.label(), "empty");
      assert_eq!(
        Outcome::Blocked {
          reason: "x".into()
        }
        .label(),
        "blocked"
      );
      assert_eq!(Outcome::NotReady.label(), "not_ready");
      assert_eq!(
        Outcome::Failed {
          reason: "x".into()
        }
        .label(),
        "failed"
      );
      assert_eq!(
        Outcome::Skipped {
          reason: "x".into()
        }
        .label(),
        "skipped"
      );
    }
  }

  mod reason {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_a_reason_only_for_the_explanatory_variants() {
      assert_eq!(
        Outcome::Blocked {
          reason: "no scope".into()
        }
        .reason(),
        Some("no scope")
      );
      assert_eq!(
        Outcome::Failed {
          reason: "timeout".into()
        }
        .reason(),
        Some("timeout")
      );
      assert_eq!(
        Outcome::Skipped {
          reason: "feature off".into()
        }
        .reason(),
        Some("feature off")
      );

      assert_eq!(
        Outcome::Synced {
          rows_touched: 1
        }
        .reason(),
        None
      );
      assert_eq!(Outcome::Empty.reason(), None);
      assert_eq!(Outcome::NotReady.reason(), None);
    }
  }

  mod rows_touched {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_rows_only_for_a_synced_outcome() {
      assert_eq!(
        Outcome::Synced {
          rows_touched: 42
        }
        .rows_touched(),
        42
      );

      assert_eq!(Outcome::Empty.rows_touched(), 0);
      assert_eq!(
        Outcome::Blocked {
          reason: "x".into()
        }
        .rows_touched(),
        0
      );
      assert_eq!(Outcome::NotReady.rows_touched(), 0);
      assert_eq!(
        Outcome::Failed {
          reason: "x".into()
        }
        .rows_touched(),
        0
      );
      assert_eq!(
        Outcome::Skipped {
          reason: "x".into()
        }
        .rows_touched(),
        0
      );
    }
  }
}
