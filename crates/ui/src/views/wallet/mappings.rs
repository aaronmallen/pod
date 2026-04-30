//! Pure display-mapping functions for wallet domain types.

use iced::Color;

use crate::style::color;

/// Return a glyph character and income direction for a journal entry type.
pub fn journal_type_glyph(t: &str) -> (&'static str, bool) {
  match t {
    "bounty_prizes"
    | "agent_mission_reward"
    | "agent_mission_time_bonus_reward"
    | "market_transaction"
    | "insurance"
    | "player_donation"
    | "contract_price_payment_corp"
    | "lp_store"
    | "project_reward"
    | "industry_job_tax" => ("↑", true),
    _ if !t.contains("fee") && !t.contains("tax") && !t.contains("cost") => ("↑", true),
    _ => ("↓", false),
  }
}

/// Map a contract status string to its display color.
pub fn status_color_for(status: &str) -> Color {
  match status {
    "finished" => color::status::ONLINE,
    "in_progress" => color::accent::PLASMA,
    "outstanding" => color::status::CAUTION,
    "outbid" | "failed" => color::status::DANGER,
    _ => color::text::SECONDARY,
  }
}

/// Map a contract status string to a human-readable label.
pub fn status_label_for(status: &str) -> String {
  match status {
    "finished" => "Finished".to_string(),
    "in_progress" => "In Progress".to_string(),
    "outstanding" => "Outstanding".to_string(),
    "outbid" => "Outbid".to_string(),
    "failed" => "Failed".to_string(),
    "expired" => "Expired".to_string(),
    other => other.replace('_', " "),
  }
}
