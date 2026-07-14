pub const CHARACTER_ASSETS: &str = "esi-assets.read_assets.v1";
pub const CHARACTER_BLUEPRINTS: &str = "esi-characters.read_blueprints.v1";
pub const CHARACTER_CALENDAR_READ: &str = "esi-calendar.read_calendar_events.v1";
pub const CHARACTER_CALENDAR_RESPOND: &str = "esi-calendar.respond_calendar_events.v1";
pub const CHARACTER_CLONES: &str = "esi-clones.read_clones.v1";
pub const CHARACTER_CONTACTS: &str = "esi-characters.read_contacts.v1";
pub const CHARACTER_CONTACTS_WRITE: &str = "esi-characters.write_contacts.v1";
pub const CHARACTER_CONTRACTS: &str = "esi-contracts.read_character_contracts.v1";
#[expect(dead_code)]
pub const CHARACTER_CORPORATION_HISTORY: &str = "esi-characters.read_corporation_history.v1";
#[expect(dead_code)]
pub const CHARACTER_FATIGUE: &str = "esi-characters.read_fatigue.v1";
#[expect(dead_code)]
pub const CHARACTER_FITTINGS: &str = "esi-fittings.read_fittings.v1";
pub const CHARACTER_IMPLANTS: &str = "esi-clones.read_implants.v1";
pub const CHARACTER_INDUSTRY_JOBS: &str = "esi-industry.read_character_jobs.v1";
pub const CHARACTER_KILLMAILS: &str = "esi-killmails.read_killmails.v1";
pub const CHARACTER_LOCATION: &str = "esi-location.read_location.v1";
pub const CHARACTER_MAIL: &str = "esi-mail.read_mail.v1";
pub const CHARACTER_MAIL_ORGANIZE: &str = "esi-mail.organize_mail.v1";
pub const CHARACTER_MAIL_SEND: &str = "esi-mail.send_mail.v1";
#[expect(dead_code)]
pub const CHARACTER_MEDALS: &str = "esi-characters.read_medals.v1";
pub const CHARACTER_NOTIFICATIONS: &str = "esi-characters.read_notifications.v1";
pub const CHARACTER_ONLINE: &str = "esi-location.read_online.v1";
pub const CHARACTER_ORDERS: &str = "esi-markets.read_character_orders.v1";
#[expect(dead_code)]
pub const CHARACTER_PLANETS: &str = "esi-planets.manage_planets.v1";
pub const CHARACTER_SEARCH: &str = "esi-search.search_structures.v1";
pub const CHARACTER_SHIP: &str = "esi-location.read_ship_type.v1";
pub const CHARACTER_SKILLQUEUE: &str = "esi-skills.read_skillqueue.v1";
pub const CHARACTER_SKILLS: &str = "esi-skills.read_skills.v1";
pub const CHARACTER_STANDINGS: &str = "esi-characters.read_standings.v1";
#[expect(dead_code)]
pub const CHARACTER_TITLES: &str = "esi-characters.read_titles.v1";
pub const CHARACTER_WALLET: &str = "esi-wallet.read_character_wallet.v1";
#[expect(dead_code)]
pub const CHARACTER_WALLET_JOURNAL: &str = "esi-wallet.read_character_wallet.v1";
#[expect(dead_code)]
pub const CHARACTER_WALLET_TRANSACTIONS: &str = "esi-wallet.read_character_wallet.v1";
pub const CORPORATION_ASSETS: &str = "esi-assets.read_corporation_assets.v1";
pub const CORPORATION_BLUEPRINTS: &str = "esi-corporations.read_blueprints.v1";
pub const CORPORATION_CONTACTS: &str = "esi-corporations.read_contacts.v1";
pub const CORPORATION_CONTRACTS: &str = "esi-contracts.read_corporation_contracts.v1";
#[expect(dead_code)]
pub const CORPORATION_CONTAINERS: &str = "esi-corporations.read_container_logs.v1";
pub const CORPORATION_DIVISIONS: &str = "esi-corporations.read_divisions.v1";
#[expect(dead_code)]
pub const CORPORATION_FACILITIES: &str = "esi-corporations.read_facilities.v1";
#[expect(dead_code)]
pub const CORPORATION_FW_STATS: &str = "esi-corporations.read_fw_stats.v1";
pub const CORPORATION_INDUSTRY_JOBS: &str = "esi-industry.read_corporation_jobs.v1";
pub const CORPORATION_KILLMAILS: &str = "esi-killmails.read_corporation_killmails.v1";
#[expect(dead_code)]
pub const CORPORATION_MEDALS: &str = "esi-corporations.read_medals.v1";
pub const CORPORATION_MEMBERS: &str = "esi-corporations.read_corporation_membership.v1";
pub const CORPORATION_MINING_EXTRACTIONS: &str = "esi-industry.read_corporation_mining.v1";
#[expect(dead_code)]
pub const CORPORATION_MINING_OBSERVERS: &str = "esi-industry.read_corporation_mining.v1";
pub const CORPORATION_ORDERS: &str = "esi-markets.read_corporation_orders.v1";
pub const CORPORATION_ROLES: &str = "esi-characters.read_corporation_roles.v1";
pub const CORPORATION_STANDINGS: &str = "esi-corporations.read_standings.v1";
pub const CORPORATION_STRUCTURES: &str = "esi-corporations.read_structures.v1";
#[expect(dead_code)]
pub const CORPORATION_TITLES: &str = "esi-corporations.read_titles.v1";
pub const CORPORATION_WALLET: &str = "esi-wallet.read_corporation_wallets.v1";
#[expect(dead_code)]
pub const CORPORATION_WALLET_DIVISION: &str = "esi-wallet.read_corporation_wallets.v1";
#[expect(dead_code)]
pub const CORPORATION_WALLET_JOURNAL: &str = "esi-wallet.read_corporation_wallets.v1";
#[expect(dead_code)]
pub const CORPORATION_WALLET_TRANSACTIONS: &str = "esi-wallet.read_corporation_wallets.v1";
pub const MARKET_STRUCTURES: &str = "esi-markets.structure_markets.v1";
pub const UI_OPEN_WINDOW: &str = "esi-ui.open_window.v1";
pub const UNIVERSE_STRUCTURES: &str = "esi-universe.read_structures.v1";

pub const BASELINE_CORP_SCOPES: &[&str] = &[CORPORATION_DIVISIONS, CORPORATION_MEMBERS, CORPORATION_ROLES];

/// The set of scopes read-gating must NOT treat as a required read wall: write scopes plus supplemental
/// read scopes that back only an optional sub-surface.
///
/// Read-gating treats a missing scope as a forbidden wall, so a scope that is requested for re-auth but is
/// not required for the base read (a write grant, or `MARKET_STRUCTURES` which only feeds the optional
/// structure order book) must be excluded here to avoid blocking read access when it is absent.
pub const WRITE_SCOPES: &[&str] = &[CHARACTER_CONTACTS_WRITE, MARKET_STRUCTURES, UI_OPEN_WINDOW];

pub fn is_write_scope(scope: &str) -> bool {
  WRITE_SCOPES.contains(&scope)
}

#[cfg(test)]
mod tests {
  mod baseline_corp_scopes {
    use super::super::*;

    #[test]
    fn it_keeps_members_for_the_corp_card_member_section() {
      assert!(BASELINE_CORP_SCOPES.contains(&CORPORATION_MEMBERS));
    }

    #[test]
    fn it_requests_divisions_so_corp_wallet_sync_does_not_401() {
      assert!(BASELINE_CORP_SCOPES.contains(&CORPORATION_DIVISIONS));
    }
  }

  mod is_write_scope {
    use super::super::*;

    #[test]
    fn it_flags_the_contacts_write_scope() {
      assert!(is_write_scope(CHARACTER_CONTACTS_WRITE));
    }

    #[test]
    fn it_excludes_the_structure_markets_scope_from_read_gating() {
      assert!(is_write_scope(MARKET_STRUCTURES));
    }

    #[test]
    fn it_excludes_the_ui_open_window_scope_from_read_gating() {
      assert!(is_write_scope(UI_OPEN_WINDOW));
    }

    #[test]
    fn it_leaves_read_scopes_unflagged() {
      assert!(!is_write_scope(CHARACTER_CONTACTS));
      assert!(!is_write_scope(CHARACTER_STANDINGS));
    }
  }
}
