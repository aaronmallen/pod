//! Feature flags that gate which ESI scopes Pod requests.

use getset::Getters;
use pod_esi::scopes::Scopes;
use serde::{Deserialize, Serialize};

/// Feature flags controlling which ESI scopes are requested.
///
/// Every flag defaults to `true`. Disabling a flag removes the
/// corresponding scope(s) from the OAuth sign-in URL so the user is
/// never prompted for permissions the feature won't use.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize)]
#[serde(default)]
pub struct Settings {
  /// Sync jump-clone locations and active-clone implants.
  #[get = "pub"]
  clone_monitoring: bool,

  /// Read character contacts and contact labels.
  #[get = "pub"]
  contacts: bool,

  /// Read recent character killmails.
  #[get = "pub"]
  combat_log: bool,

  /// Read EVE notification feed.
  #[get = "pub"]
  eve_notifications: bool,

  /// Read character standings toward NPCs and other players.
  #[get = "pub"]
  standings: bool,

  /// Poll the character's current solar-system location.
  #[get = "pub"]
  location_tracking: bool,

  /// Sync skill levels and active skill-training queue.
  #[get = "pub"]
  skill_monitoring: bool,

  /// Read, organise, and send EVE mail.
  #[get = "pub"]
  mail: bool,

  /// Read character wallet balance, journal, and transactions.
  #[get = "pub"]
  wallet: bool,

  /// Read character assets and resolve player-owned structure names.
  #[get = "pub"]
  asset_tracking: bool,
}

impl Settings {
  /// Constructs `Settings` from individual flag values.
  #[allow(clippy::too_many_arguments)]
  pub fn from_flags(
    asset_tracking: bool,
    clone_monitoring: bool,
    combat_log: bool,
    contacts: bool,
    eve_notifications: bool,
    location_tracking: bool,
    mail: bool,
    skill_monitoring: bool,
    standings: bool,
    wallet: bool,
  ) -> Self {
    Self {
      asset_tracking,
      clone_monitoring,
      combat_log,
      contacts,
      eve_notifications,
      location_tracking,
      mail,
      skill_monitoring,
      standings,
      wallet,
    }
  }

  /// ESI scopes to request when a **character** authenticates.
  pub fn required_scopes_for_character(&self) -> Vec<&'static str> {
    let mut scopes: Vec<&'static str> = Vec::new();

    if self.clone_monitoring || self.skill_monitoring {
      scopes.push(Scopes::CLONES_READ_CLONES);
      scopes.push(Scopes::CLONES_READ_IMPLANTS);
    }

    if self.contacts {
      scopes.push(Scopes::CHARACTERS_READ_CONTACTS);
    }

    if self.combat_log {
      scopes.push(Scopes::KILLMAILS_READ_KILLMAILS);
    }

    if self.eve_notifications {
      scopes.push(Scopes::CHARACTERS_READ_NOTIFICATIONS);
    }

    if self.standings {
      scopes.push(Scopes::CHARACTERS_READ_STANDINGS);
    }

    if self.location_tracking {
      scopes.push(Scopes::LOCATION_READ_LOCATION);
    }

    if self.skill_monitoring {
      scopes.push(Scopes::SKILLS_READ_SKILLS);
      scopes.push(Scopes::SKILLS_READ_SKILLQUEUE);
    }

    if self.mail {
      scopes.push(Scopes::MAIL_READ_MAIL);
      scopes.push(Scopes::MAIL_ORGANIZE_MAIL);
      scopes.push(Scopes::MAIL_SEND_MAIL);
    }

    if self.wallet {
      scopes.push(Scopes::WALLET_READ_CHARACTER_WALLET);
    }

    if self.asset_tracking {
      scopes.push(Scopes::ASSETS_READ_ASSETS);
      scopes.push(Scopes::UNIVERSE_READ_STRUCTURES);
    }

    scopes
  }

  /// ESI scopes to request when a **corporation** authenticates.
  ///
  /// Includes base corp scopes that are always needed plus feature-gated
  /// corp scopes for wallet and assets.
  pub fn required_scopes_for_corporation(&self) -> Vec<&'static str> {
    let mut scopes: Vec<&'static str> = vec![Scopes::CORPORATIONS_READ_CORPORATION_MEMBERSHIP];

    if self.wallet {
      scopes.push(Scopes::CONTRACTS_READ_CORPORATION_CONTRACTS);
      scopes.push(Scopes::MARKETS_READ_CORPORATION_ORDERS);
      scopes.push(Scopes::WALLET_READ_CORPORATION_WALLETS);
    }

    if self.asset_tracking {
      scopes.push(Scopes::ASSETS_READ_CORPORATION_ASSETS);
    }

    scopes
  }
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      clone_monitoring: true,
      contacts: true,
      combat_log: true,
      eve_notifications: true,
      standings: true,
      location_tracking: true,
      skill_monitoring: true,
      mail: true,
      wallet: true,
      asset_tracking: true,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn all_disabled() -> Settings {
    Settings {
      clone_monitoring: false,
      contacts: false,
      combat_log: false,
      eve_notifications: false,
      standings: false,
      location_tracking: false,
      skill_monitoring: false,
      mail: false,
      wallet: false,
      asset_tracking: false,
    }
  }

  mod required_scopes_for_character {
    use super::*;

    #[test]
    fn it_returns_empty_when_all_flags_disabled() {
      let settings = all_disabled();

      assert!(settings.required_scopes_for_character().is_empty());
    }

    #[test]
    fn it_includes_clone_scopes_when_clone_monitoring_enabled() {
      let mut settings = all_disabled();
      settings.clone_monitoring = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::CLONES_READ_CLONES));
      assert!(scopes.contains(&Scopes::CLONES_READ_IMPLANTS));
    }

    #[test]
    fn it_includes_contacts_scope_when_contacts_enabled() {
      let mut settings = all_disabled();
      settings.contacts = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::CHARACTERS_READ_CONTACTS));
    }

    #[test]
    fn it_includes_killmails_scope_when_combat_log_enabled() {
      let mut settings = all_disabled();
      settings.combat_log = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::KILLMAILS_READ_KILLMAILS));
    }

    #[test]
    fn it_includes_notifications_scope_when_eve_notifications_enabled() {
      let mut settings = all_disabled();
      settings.eve_notifications = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::CHARACTERS_READ_NOTIFICATIONS));
    }

    #[test]
    fn it_includes_standings_scope_when_standings_enabled() {
      let mut settings = all_disabled();
      settings.standings = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::CHARACTERS_READ_STANDINGS));
    }

    #[test]
    fn it_includes_location_scope_when_location_tracking_enabled() {
      let mut settings = all_disabled();
      settings.location_tracking = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::LOCATION_READ_LOCATION));
    }

    #[test]
    fn it_includes_skill_scopes_when_skill_monitoring_enabled() {
      let mut settings = all_disabled();
      settings.skill_monitoring = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::SKILLS_READ_SKILLS));
      assert!(scopes.contains(&Scopes::SKILLS_READ_SKILLQUEUE));
    }

    #[test]
    fn it_includes_mail_scopes_when_mail_enabled() {
      let mut settings = all_disabled();
      settings.mail = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::MAIL_READ_MAIL));
      assert!(scopes.contains(&Scopes::MAIL_ORGANIZE_MAIL));
      assert!(scopes.contains(&Scopes::MAIL_SEND_MAIL));
    }

    #[test]
    fn it_includes_wallet_scopes_when_wallet_enabled() {
      let mut settings = all_disabled();
      settings.wallet = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::WALLET_READ_CHARACTER_WALLET));
      assert!(!scopes.contains(&Scopes::WALLET_READ_CORPORATION_WALLETS));
    }

    #[test]
    fn it_includes_asset_scopes_when_asset_tracking_enabled() {
      let mut settings = all_disabled();
      settings.asset_tracking = true;

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::ASSETS_READ_ASSETS));
      assert!(scopes.contains(&Scopes::UNIVERSE_READ_STRUCTURES));
    }

    #[test]
    fn it_includes_all_scopes_when_all_flags_enabled() {
      let settings = Settings::default();

      let scopes = settings.required_scopes_for_character();

      assert!(scopes.contains(&Scopes::CLONES_READ_CLONES));
      assert!(scopes.contains(&Scopes::CLONES_READ_IMPLANTS));
      assert!(scopes.contains(&Scopes::CHARACTERS_READ_CONTACTS));
      assert!(scopes.contains(&Scopes::KILLMAILS_READ_KILLMAILS));
      assert!(scopes.contains(&Scopes::CHARACTERS_READ_NOTIFICATIONS));
      assert!(scopes.contains(&Scopes::CHARACTERS_READ_STANDINGS));
      assert!(scopes.contains(&Scopes::LOCATION_READ_LOCATION));
      assert!(scopes.contains(&Scopes::SKILLS_READ_SKILLS));
      assert!(scopes.contains(&Scopes::SKILLS_READ_SKILLQUEUE));
      assert!(scopes.contains(&Scopes::MAIL_READ_MAIL));
      assert!(scopes.contains(&Scopes::MAIL_ORGANIZE_MAIL));
      assert!(scopes.contains(&Scopes::MAIL_SEND_MAIL));
      assert!(scopes.contains(&Scopes::WALLET_READ_CHARACTER_WALLET));
      assert!(!scopes.contains(&Scopes::WALLET_READ_CORPORATION_WALLETS));
      assert!(scopes.contains(&Scopes::ASSETS_READ_ASSETS));
      assert!(scopes.contains(&Scopes::UNIVERSE_READ_STRUCTURES));
    }
  }

  mod required_scopes_for_corporation {
    use super::*;

    #[test]
    fn it_always_includes_membership_scope() {
      let settings = all_disabled();

      let scopes = settings.required_scopes_for_corporation();

      assert!(scopes.contains(&Scopes::CORPORATIONS_READ_CORPORATION_MEMBERSHIP));
    }

    #[test]
    fn it_excludes_wallet_and_asset_scopes_when_disabled() {
      let settings = all_disabled();

      let scopes = settings.required_scopes_for_corporation();

      assert!(!scopes.contains(&Scopes::CONTRACTS_READ_CORPORATION_CONTRACTS));
      assert!(!scopes.contains(&Scopes::MARKETS_READ_CORPORATION_ORDERS));
      assert!(!scopes.contains(&Scopes::WALLET_READ_CORPORATION_WALLETS));
      assert!(!scopes.contains(&Scopes::ASSETS_READ_CORPORATION_ASSETS));
    }

    #[test]
    fn it_includes_corp_wallet_scopes_when_wallet_enabled() {
      let mut settings = all_disabled();
      settings.wallet = true;

      let scopes = settings.required_scopes_for_corporation();

      assert!(scopes.contains(&Scopes::CONTRACTS_READ_CORPORATION_CONTRACTS));
      assert!(scopes.contains(&Scopes::MARKETS_READ_CORPORATION_ORDERS));
      assert!(scopes.contains(&Scopes::WALLET_READ_CORPORATION_WALLETS));
      assert!(!scopes.contains(&Scopes::ASSETS_READ_CORPORATION_ASSETS));
    }

    #[test]
    fn it_includes_corp_asset_scope_when_asset_tracking_enabled() {
      let mut settings = all_disabled();
      settings.asset_tracking = true;

      let scopes = settings.required_scopes_for_corporation();

      assert!(scopes.contains(&Scopes::ASSETS_READ_CORPORATION_ASSETS));
      assert!(!scopes.contains(&Scopes::WALLET_READ_CORPORATION_WALLETS));
    }

    #[test]
    fn it_includes_all_corp_scopes_when_all_flags_enabled() {
      let settings = Settings::default();

      let scopes = settings.required_scopes_for_corporation();

      assert!(scopes.contains(&Scopes::CORPORATIONS_READ_CORPORATION_MEMBERSHIP));
      assert!(scopes.contains(&Scopes::CONTRACTS_READ_CORPORATION_CONTRACTS));
      assert!(scopes.contains(&Scopes::MARKETS_READ_CORPORATION_ORDERS));
      assert!(scopes.contains(&Scopes::WALLET_READ_CORPORATION_WALLETS));
      assert!(scopes.contains(&Scopes::ASSETS_READ_CORPORATION_ASSETS));
    }
  }
}
