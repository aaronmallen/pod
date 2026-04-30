//! ESI OAuth2 scope constants.

/// Namespace for ESI OAuth2 scope string constants.
pub struct Scopes;

impl Scopes {
  /// All 63 ESI OAuth2 scope strings.
  pub const ALL: &[&str] = &[
    Self::ALLIANCES_READ_CONTACTS,
    Self::ASSETS_READ_ASSETS,
    Self::ASSETS_READ_CORPORATION_ASSETS,
    Self::CALENDAR_READ_CALENDAR_EVENTS,
    Self::CALENDAR_RESPOND_CALENDAR_EVENTS,
    Self::CHARACTERS_READ_AGENTS_RESEARCH,
    Self::CHARACTERS_READ_BLUEPRINTS,
    Self::CHARACTERS_READ_CONTACTS,
    Self::CHARACTERS_READ_CORPORATION_ROLES,
    Self::CHARACTERS_READ_FATIGUE,
    Self::CHARACTERS_READ_FW_STATS,
    Self::CHARACTERS_READ_LOYALTY,
    Self::CHARACTERS_READ_MEDALS,
    Self::CHARACTERS_READ_NOTIFICATIONS,
    Self::CHARACTERS_READ_STANDINGS,
    Self::CHARACTERS_READ_TITLES,
    Self::CHARACTERS_WRITE_CONTACTS,
    Self::CLONES_READ_CLONES,
    Self::CLONES_READ_IMPLANTS,
    Self::CONTRACTS_READ_CHARACTER_CONTRACTS,
    Self::CONTRACTS_READ_CORPORATION_CONTRACTS,
    Self::CORPORATIONS_READ_BLUEPRINTS,
    Self::CORPORATIONS_READ_CONTACTS,
    Self::CORPORATIONS_READ_CONTAINER_LOGS,
    Self::CORPORATIONS_READ_CORPORATION_MEMBERSHIP,
    Self::CORPORATIONS_READ_DIVISIONS,
    Self::CORPORATIONS_READ_FACILITIES,
    Self::CORPORATIONS_READ_FW_STATS,
    Self::CORPORATIONS_READ_MEDALS,
    Self::CORPORATIONS_READ_STANDINGS,
    Self::CORPORATIONS_READ_STARBASES,
    Self::CORPORATIONS_READ_STRUCTURES,
    Self::CORPORATIONS_READ_TITLES,
    Self::CORPORATIONS_TRACK_MEMBERS,
    Self::FITTINGS_READ_FITTINGS,
    Self::FITTINGS_WRITE_FITTINGS,
    Self::FLEETS_READ_FLEET,
    Self::FLEETS_WRITE_FLEET,
    Self::INDUSTRY_READ_CHARACTER_JOBS,
    Self::INDUSTRY_READ_CHARACTER_MINING,
    Self::INDUSTRY_READ_CORPORATION_JOBS,
    Self::INDUSTRY_READ_CORPORATION_MINING,
    Self::KILLMAILS_READ_CORPORATION_KILLMAILS,
    Self::KILLMAILS_READ_KILLMAILS,
    Self::LOCATION_READ_LOCATION,
    Self::LOCATION_READ_ONLINE,
    Self::LOCATION_READ_SHIP_TYPE,
    Self::MAIL_ORGANIZE_MAIL,
    Self::MAIL_READ_MAIL,
    Self::MAIL_SEND_MAIL,
    Self::MARKETS_READ_CHARACTER_ORDERS,
    Self::MARKETS_READ_CORPORATION_ORDERS,
    Self::MARKETS_STRUCTURE_MARKETS,
    Self::PLANETS_MANAGE_PLANETS,
    Self::PLANETS_READ_CUSTOMS_OFFICES,
    Self::SEARCH_SEARCH_STRUCTURES,
    Self::SKILLS_READ_SKILLQUEUE,
    Self::SKILLS_READ_SKILLS,
    Self::UI_OPEN_WINDOW,
    Self::UI_WRITE_WAYPOINT,
    Self::UNIVERSE_READ_STRUCTURES,
    Self::WALLET_READ_CHARACTER_WALLET,
    Self::WALLET_READ_CORPORATION_WALLETS,
  ];
  /// `esi-alliances.read_contacts.v1`
  pub const ALLIANCES_READ_CONTACTS: &str = "esi-alliances.read_contacts.v1";
  /// `esi-assets.read_assets.v1`
  pub const ASSETS_READ_ASSETS: &str = "esi-assets.read_assets.v1";
  /// `esi-assets.read_corporation_assets.v1`
  pub const ASSETS_READ_CORPORATION_ASSETS: &str = "esi-assets.read_corporation_assets.v1";
  /// `esi-calendar.read_calendar_events.v1`
  pub const CALENDAR_READ_CALENDAR_EVENTS: &str = "esi-calendar.read_calendar_events.v1";
  /// `esi-calendar.respond_calendar_events.v1`
  pub const CALENDAR_RESPOND_CALENDAR_EVENTS: &str = "esi-calendar.respond_calendar_events.v1";
  /// `esi-characters.read_agents_research.v1`
  pub const CHARACTERS_READ_AGENTS_RESEARCH: &str = "esi-characters.read_agents_research.v1";
  /// `esi-characters.read_blueprints.v1`
  pub const CHARACTERS_READ_BLUEPRINTS: &str = "esi-characters.read_blueprints.v1";
  /// `esi-characters.read_contacts.v1`
  pub const CHARACTERS_READ_CONTACTS: &str = "esi-characters.read_contacts.v1";
  /// `esi-characters.read_corporation_roles.v1`
  pub const CHARACTERS_READ_CORPORATION_ROLES: &str = "esi-characters.read_corporation_roles.v1";
  /// `esi-characters.read_fatigue.v1`
  pub const CHARACTERS_READ_FATIGUE: &str = "esi-characters.read_fatigue.v1";
  /// `esi-characters.read_fw_stats.v1`
  pub const CHARACTERS_READ_FW_STATS: &str = "esi-characters.read_fw_stats.v1";
  /// `esi-characters.read_loyalty.v1`
  pub const CHARACTERS_READ_LOYALTY: &str = "esi-characters.read_loyalty.v1";
  /// `esi-characters.read_medals.v1`
  pub const CHARACTERS_READ_MEDALS: &str = "esi-characters.read_medals.v1";
  /// `esi-characters.read_notifications.v1`
  pub const CHARACTERS_READ_NOTIFICATIONS: &str = "esi-characters.read_notifications.v1";
  /// `esi-characters.read_standings.v1`
  pub const CHARACTERS_READ_STANDINGS: &str = "esi-characters.read_standings.v1";
  /// `esi-characters.read_titles.v1`
  pub const CHARACTERS_READ_TITLES: &str = "esi-characters.read_titles.v1";
  /// `esi-characters.write_contacts.v1`
  pub const CHARACTERS_WRITE_CONTACTS: &str = "esi-characters.write_contacts.v1";
  /// `esi-clones.read_clones.v1`
  pub const CLONES_READ_CLONES: &str = "esi-clones.read_clones.v1";
  /// `esi-clones.read_implants.v1`
  pub const CLONES_READ_IMPLANTS: &str = "esi-clones.read_implants.v1";
  /// `esi-contracts.read_character_contracts.v1`
  pub const CONTRACTS_READ_CHARACTER_CONTRACTS: &str = "esi-contracts.read_character_contracts.v1";
  /// `esi-contracts.read_corporation_contracts.v1`
  pub const CONTRACTS_READ_CORPORATION_CONTRACTS: &str = "esi-contracts.read_corporation_contracts.v1";
  /// `esi-corporations.read_blueprints.v1`
  pub const CORPORATIONS_READ_BLUEPRINTS: &str = "esi-corporations.read_blueprints.v1";
  /// `esi-corporations.read_contacts.v1`
  pub const CORPORATIONS_READ_CONTACTS: &str = "esi-corporations.read_contacts.v1";
  /// `esi-corporations.read_container_logs.v1`
  pub const CORPORATIONS_READ_CONTAINER_LOGS: &str = "esi-corporations.read_container_logs.v1";
  /// `esi-corporations.read_corporation_membership.v1`
  pub const CORPORATIONS_READ_CORPORATION_MEMBERSHIP: &str = "esi-corporations.read_corporation_membership.v1";
  /// `esi-corporations.read_divisions.v1`
  pub const CORPORATIONS_READ_DIVISIONS: &str = "esi-corporations.read_divisions.v1";
  /// `esi-corporations.read_facilities.v1`
  pub const CORPORATIONS_READ_FACILITIES: &str = "esi-corporations.read_facilities.v1";
  /// `esi-corporations.read_fw_stats.v1`
  pub const CORPORATIONS_READ_FW_STATS: &str = "esi-corporations.read_fw_stats.v1";
  /// `esi-corporations.read_medals.v1`
  pub const CORPORATIONS_READ_MEDALS: &str = "esi-corporations.read_medals.v1";
  /// `esi-corporations.read_standings.v1`
  pub const CORPORATIONS_READ_STANDINGS: &str = "esi-corporations.read_standings.v1";
  /// `esi-corporations.read_starbases.v1`
  pub const CORPORATIONS_READ_STARBASES: &str = "esi-corporations.read_starbases.v1";
  /// `esi-corporations.read_structures.v1`
  pub const CORPORATIONS_READ_STRUCTURES: &str = "esi-corporations.read_structures.v1";
  /// `esi-corporations.read_titles.v1`
  pub const CORPORATIONS_READ_TITLES: &str = "esi-corporations.read_titles.v1";
  /// `esi-corporations.track_members.v1`
  pub const CORPORATIONS_TRACK_MEMBERS: &str = "esi-corporations.track_members.v1";
  /// `esi-fittings.read_fittings.v1`
  pub const FITTINGS_READ_FITTINGS: &str = "esi-fittings.read_fittings.v1";
  /// `esi-fittings.write_fittings.v1`
  pub const FITTINGS_WRITE_FITTINGS: &str = "esi-fittings.write_fittings.v1";
  /// `esi-fleets.read_fleet.v1`
  pub const FLEETS_READ_FLEET: &str = "esi-fleets.read_fleet.v1";
  /// `esi-fleets.write_fleet.v1`
  pub const FLEETS_WRITE_FLEET: &str = "esi-fleets.write_fleet.v1";
  /// `esi-industry.read_character_jobs.v1`
  pub const INDUSTRY_READ_CHARACTER_JOBS: &str = "esi-industry.read_character_jobs.v1";
  /// `esi-industry.read_character_mining.v1`
  pub const INDUSTRY_READ_CHARACTER_MINING: &str = "esi-industry.read_character_mining.v1";
  /// `esi-industry.read_corporation_jobs.v1`
  pub const INDUSTRY_READ_CORPORATION_JOBS: &str = "esi-industry.read_corporation_jobs.v1";
  /// `esi-industry.read_corporation_mining.v1`
  pub const INDUSTRY_READ_CORPORATION_MINING: &str = "esi-industry.read_corporation_mining.v1";
  /// `esi-killmails.read_corporation_killmails.v1`
  pub const KILLMAILS_READ_CORPORATION_KILLMAILS: &str = "esi-killmails.read_corporation_killmails.v1";
  /// `esi-killmails.read_killmails.v1`
  pub const KILLMAILS_READ_KILLMAILS: &str = "esi-killmails.read_killmails.v1";
  /// `esi-location.read_location.v1`
  pub const LOCATION_READ_LOCATION: &str = "esi-location.read_location.v1";
  /// `esi-location.read_online.v1`
  pub const LOCATION_READ_ONLINE: &str = "esi-location.read_online.v1";
  /// `esi-location.read_ship_type.v1`
  pub const LOCATION_READ_SHIP_TYPE: &str = "esi-location.read_ship_type.v1";
  /// `esi-mail.organize_mail.v1`
  pub const MAIL_ORGANIZE_MAIL: &str = "esi-mail.organize_mail.v1";
  /// `esi-mail.read_mail.v1`
  pub const MAIL_READ_MAIL: &str = "esi-mail.read_mail.v1";
  /// `esi-mail.send_mail.v1`
  pub const MAIL_SEND_MAIL: &str = "esi-mail.send_mail.v1";
  /// `esi-markets.read_character_orders.v1`
  pub const MARKETS_READ_CHARACTER_ORDERS: &str = "esi-markets.read_character_orders.v1";
  /// `esi-markets.read_corporation_orders.v1`
  pub const MARKETS_READ_CORPORATION_ORDERS: &str = "esi-markets.read_corporation_orders.v1";
  /// `esi-markets.structure_markets.v1`
  pub const MARKETS_STRUCTURE_MARKETS: &str = "esi-markets.structure_markets.v1";
  /// `esi-planets.manage_planets.v1`
  pub const PLANETS_MANAGE_PLANETS: &str = "esi-planets.manage_planets.v1";
  /// `esi-planets.read_customs_offices.v1`
  pub const PLANETS_READ_CUSTOMS_OFFICES: &str = "esi-planets.read_customs_offices.v1";
  /// `esi-search.search_structures.v1`
  pub const SEARCH_SEARCH_STRUCTURES: &str = "esi-search.search_structures.v1";
  /// `esi-skills.read_skillqueue.v1`
  pub const SKILLS_READ_SKILLQUEUE: &str = "esi-skills.read_skillqueue.v1";
  /// `esi-skills.read_skills.v1`
  pub const SKILLS_READ_SKILLS: &str = "esi-skills.read_skills.v1";
  /// `esi-ui.open_window.v1`
  pub const UI_OPEN_WINDOW: &str = "esi-ui.open_window.v1";
  /// `esi-ui.write_waypoint.v1`
  pub const UI_WRITE_WAYPOINT: &str = "esi-ui.write_waypoint.v1";
  /// `esi-universe.read_structures.v1`
  pub const UNIVERSE_READ_STRUCTURES: &str = "esi-universe.read_structures.v1";
  /// `esi-wallet.read_character_wallet.v1`
  pub const WALLET_READ_CHARACTER_WALLET: &str = "esi-wallet.read_character_wallet.v1";
  /// `esi-wallet.read_corporation_wallets.v1`
  pub const WALLET_READ_CORPORATION_WALLETS: &str = "esi-wallet.read_corporation_wallets.v1";
}
