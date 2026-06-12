#![allow(dead_code)]

pub const CHARACTER_ASSETS: &str = "esi-assets.read_assets.v1";
pub const CHARACTER_BLUEPRINTS: &str = "esi-characters.read_blueprints.v1";
pub const CHARACTER_CALENDAR_READ: &str = "esi-calendar.read_calendar_events.v1";
pub const CHARACTER_CALENDAR_RESPOND: &str = "esi-calendar.respond_calendar_events.v1";
pub const CHARACTER_CLONES: &str = "esi-clones.read_clones.v1";
pub const CHARACTER_CONTACTS: &str = "esi-characters.read_contacts.v1";
pub const CHARACTER_CONTRACTS: &str = "esi-contracts.read_character_contracts.v1";
pub const CHARACTER_CORPORATION_HISTORY: &str = "esi-characters.read_corporation_history.v1";
pub const CHARACTER_FATIGUE: &str = "esi-characters.read_fatigue.v1";
pub const CHARACTER_FITTINGS: &str = "esi-fittings.read_fittings.v1";
pub const CHARACTER_IMPLANTS: &str = "esi-clones.read_implants.v1";
pub const CHARACTER_KILLMAILS: &str = "esi-killmails.read_killmails.v1";
pub const CHARACTER_LOCATION: &str = "esi-location.read_location.v1";
pub const CHARACTER_MAIL: &str = "esi-mail.read_mail.v1";
pub const CHARACTER_MAIL_ORGANIZE: &str = "esi-mail.organize_mail.v1";
pub const CHARACTER_MAIL_SEND: &str = "esi-mail.send_mail.v1";
pub const CHARACTER_MEDALS: &str = "esi-characters.read_medals.v1";
pub const CHARACTER_NOTIFICATIONS: &str = "esi-characters.read_notifications.v1";
pub const CHARACTER_ONLINE: &str = "esi-location.read_online.v1";
pub const CHARACTER_ORDERS: &str = "esi-markets.read_character_orders.v1";
pub const CHARACTER_PLANETS: &str = "esi-planets.manage_planets.v1";
pub const CHARACTER_SEARCH: &str = "esi-search.search_structures.v1";
pub const CHARACTER_SHIP: &str = "esi-location.read_ship_type.v1";
pub const CHARACTER_SKILLQUEUE: &str = "esi-skills.read_skillqueue.v1";
pub const CHARACTER_SKILLS: &str = "esi-skills.read_skills.v1";
pub const CHARACTER_STANDINGS: &str = "esi-characters.read_standings.v1";
pub const CHARACTER_TITLES: &str = "esi-characters.read_titles.v1";
pub const CHARACTER_WALLET: &str = "esi-wallet.read_character_wallet.v1";
pub const CHARACTER_WALLET_JOURNAL: &str = "esi-wallet.read_character_wallet.v1";
pub const CHARACTER_WALLET_TRANSACTIONS: &str = "esi-wallet.read_character_wallet.v1";
pub const CORPORATION_ASSETS: &str = "esi-assets.read_corporation_assets.v1";
pub const CORPORATION_BLUEPRINTS: &str = "esi-corporations.read_blueprints.v1";
pub const CORPORATION_CONTACTS: &str = "esi-corporations.read_contacts.v1";
pub const CORPORATION_CONTAINERS: &str = "esi-corporations.read_container_logs.v1";
pub const CORPORATION_DIVISIONS: &str = "esi-corporations.read_divisions.v1";
pub const CORPORATION_FACILITIES: &str = "esi-corporations.read_facilities.v1";
pub const CORPORATION_FW_STATS: &str = "esi-corporations.read_fw_stats.v1";
pub const CORPORATION_INDUSTRY_JOBS: &str = "esi-industry.read_corporation_jobs.v1";
pub const CORPORATION_KILLMAILS: &str = "esi-killmails.read_corporation_killmails.v1";
pub const CORPORATION_MEDALS: &str = "esi-corporations.read_medals.v1";
pub const CORPORATION_MEMBERS: &str = "esi-corporations.read_corporation_membership.v1";
pub const CORPORATION_MINING_EXTRACTIONS: &str = "esi-industry.read_corporation_mining.v1";
pub const CORPORATION_MINING_OBSERVERS: &str = "esi-industry.read_corporation_mining.v1";
pub const CORPORATION_ORDERS: &str = "esi-markets.read_corporation_orders.v1";
pub const CORPORATION_ROLES: &str = "esi-characters.read_corporation_roles.v1";
pub const CORPORATION_STANDINGS: &str = "esi-corporations.read_standings.v1";
pub const CORPORATION_STRUCTURES: &str = "esi-corporations.read_structures.v1";
pub const CORPORATION_TITLES: &str = "esi-corporations.read_titles.v1";
pub const CORPORATION_WALLET: &str = "esi-wallet.read_corporation_wallets.v1";
pub const CORPORATION_WALLET_DIVISION: &str = "esi-wallet.read_corporation_wallets.v1";
pub const CORPORATION_WALLET_JOURNAL: &str = "esi-wallet.read_corporation_wallets.v1";
pub const CORPORATION_WALLET_TRANSACTIONS: &str = "esi-wallet.read_corporation_wallets.v1";
pub const UNIVERSE_STRUCTURES: &str = "esi-universe.read_structures.v1";

pub const CORP_SIGN_IN_SCOPES: &[&str] = &[
  CORPORATION_ROLES,
  CORPORATION_ASSETS,
  CORPORATION_WALLET,
  CORPORATION_DIVISIONS,
  CORPORATION_MEMBERS,
];

#[cfg(test)]
mod tests {
  mod corp_sign_in_scopes {
    use super::super::*;

    #[test]
    fn it_requests_divisions_so_corp_wallet_sync_does_not_401() {
      assert!(CORP_SIGN_IN_SCOPES.contains(&CORPORATION_DIVISIONS));
    }
  }
}
