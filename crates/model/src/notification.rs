//! Domain models and categorization logic for character notifications.

use getset::Getters;

/// High-level category for an EVE notification, derived from the notification type string.
#[derive(Clone, Debug, PartialEq)]
pub enum NotificationCategory {
  /// Alliance-level events.
  Alliance,
  /// Cloning and jump-clone events.
  Clone,
  /// NPC combat and kill/loss events.
  Combat,
  /// Contact list changes.
  Contact,
  /// In-game contract events.
  Contract,
  /// Corporation events.
  Corp,
  /// Factional warfare events.
  Fw,
  /// Incursion and invasion events.
  Incursion,
  /// Industry, manufacturing, and reactions.
  Industry,
  /// Insurance claim events.
  Insurance,
  /// Market and billing events.
  Market,
  /// Agent mission events.
  Mission,
  /// Reward and daily-challenge events.
  Reward,
  /// Standing changes.
  Standing,
  /// Structure and sovereignty events.
  Structure,
  /// Generic or unrecognized system messages.
  System,
  /// War declarations and updates.
  War,
}

/// Derives the [`NotificationCategory`] for a raw EVE notification type string.
///
/// Checks an explicit override map first, then applies ordered prefix/contains
/// rules ported from `tmp/design/character-detail-data.jsx`. Returns
/// [`NotificationCategory::System`] for any unrecognized type.
pub fn categorize_notif(notif_type: &str) -> NotificationCategory {
  // Explicit overrides win before any prefix rule.
  match notif_type {
    "StructureCourierContractChanged" => return NotificationCategory::Contract,
    "AllianceCapitalChanged" => return NotificationCategory::Alliance,
    "AllWarCorpJoinedAllianceMsg" => return NotificationCategory::War,
    "BattlePunishFriendlyFire" => return NotificationCategory::Combat,
    "CombatOperationFinished" => return NotificationCategory::Combat,
    "ContainerPasswordMsg" => return NotificationCategory::System,
    "OldLscMessages" => return NotificationCategory::System,
    "OperationFinished" => return NotificationCategory::Combat,
    "TutorialMsg" => return NotificationCategory::System,
    _ => {}
  }

  // Specifics before generic prefixes.
  if notif_type.starts_with("StructureCourier") {
    return NotificationCategory::Contract;
  }

  if notif_type.starts_with("Insurance") {
    return NotificationCategory::Insurance;
  }

  if notif_type.starts_with("Clone") || notif_type.starts_with("JumpClone") {
    return NotificationCategory::Clone;
  }

  if notif_type.starts_with("FW") || notif_type.starts_with("FacWar") {
    return NotificationCategory::Fw;
  }

  if notif_type.starts_with("NPCStandings") {
    return NotificationCategory::Standing;
  }

  if notif_type.starts_with("Bounty") || notif_type.starts_with("KillReport") || notif_type.starts_with("KillRight") {
    return NotificationCategory::Combat;
  }

  if notif_type.starts_with("Mission")
    || notif_type.starts_with("ResearchMission")
    || notif_type.starts_with("StoryLine")
    || notif_type.starts_with("AgentRetired")
  {
    return NotificationCategory::Mission;
  }

  if notif_type.starts_with("Moonmining")
    || notif_type.starts_with("IndustryOperation")
    || notif_type.starts_with("IndustryTeam")
    || notif_type.starts_with("StructuresJobs")
  {
    return NotificationCategory::Industry;
  }

  if notif_type.starts_with("Incursion")
    || notif_type.starts_with("Invasion")
    || notif_type.starts_with("DistrictAttacked")
    || notif_type.starts_with("DustApp")
    || notif_type.starts_with("ContractRegionChanged")
  {
    return NotificationCategory::Incursion;
  }

  if notif_type.starts_with("Contact") || notif_type.starts_with("BuddyConnect") || notif_type.starts_with("LocateChar")
  {
    return NotificationCategory::Contact;
  }

  if notif_type.starts_with("Bill")
    || notif_type.starts_with("Customs")
    || notif_type.starts_with("Raffle")
    || notif_type.starts_with("Reimbursement")
    || notif_type.starts_with("Transaction")
    || notif_type.starts_with("StructurePaint")
  {
    return NotificationCategory::Market;
  }

  if notif_type.starts_with("GameTime")
    || notif_type.starts_with("Gift")
    || notif_type.starts_with("DailyItemReward")
    || notif_type.starts_with("LPAutoRedeemed")
    || notif_type.starts_with("SkinSequencing")
    || notif_type.starts_with("SPAutoRedeemed")
    || notif_type.starts_with("SeasonalChallenge")
    || notif_type.starts_with("ExpertSystem")
  {
    return NotificationCategory::Reward;
  }

  if notif_type.starts_with("FreelanceProject") {
    return NotificationCategory::System;
  }

  // War — contains "War", starts with "Ally" or "MercOffer", or contains "Surrender".
  if notif_type.contains("War")
    || notif_type.starts_with("Ally")
    || notif_type.contains("Surrender")
    || notif_type.starts_with("MercOffer")
  {
    return NotificationCategory::War;
  }

  // Structure / sovereignty / station / tower.
  if notif_type.starts_with("Sov")
    || notif_type.starts_with("Sovereignty")
    || notif_type.starts_with("Tower")
    || notif_type.starts_with("Skyhook")
    || notif_type.starts_with("Orbital")
    || notif_type.starts_with("Mercenary")
    || notif_type.starts_with("IHub")
    || notif_type.starts_with("Infrastructure")
    || notif_type.starts_with("Structure")
    || notif_type.starts_with("Station")
    || notif_type.starts_with("OwnershipTransferred")
    || notif_type.starts_with("EntosisCapture")
    || notif_type.starts_with("AllAnchoring")
    || notif_type.starts_with("AllMaintenance")
    || notif_type.starts_with("AllStruc")
  {
    return NotificationCategory::Structure;
  }

  // Corp catch-all (after war).
  if notif_type.starts_with("CorporationGoal") || notif_type.starts_with("CorporationLeft") {
    return NotificationCategory::Corp;
  }

  if notif_type.starts_with("Char") || notif_type.starts_with("Corp") {
    return NotificationCategory::Corp;
  }

  NotificationCategory::System
}

/// A notification received by a character from the EVE notification feed.
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct Notification {
  /// Optional YAML/text body attached to the notification.
  #[get = "pub"]
  body: Option<String>,
  /// High-level category derived from the notification type.
  #[get = "pub"]
  category: NotificationCategory,
  /// Whether the character has read this notification.
  #[get = "pub"]
  is_read: bool,
  /// Unique notification identifier.
  #[get = "pub"]
  notification_id: i64,
  /// Sender entity ID, if available.
  #[get = "pub"]
  sender_id: Option<i64>,
  /// Sender entity type string, if available.
  #[get = "pub"]
  sender_type: Option<String>,
  /// Raw EVE notification type string.
  #[get = "pub"]
  notif_type: String,
  /// ISO-8601 timestamp when the notification was sent.
  #[get = "pub"]
  timestamp: String,
}

impl Notification {
  /// Creates a new notification, deriving the category from `notif_type`.
  pub fn new(notification_id: i64, notif_type: impl Into<String>, timestamp: impl Into<String>) -> Self {
    let notif_type = notif_type.into();
    let category = categorize_notif(&notif_type);
    Self {
      body: None,
      category,
      is_read: false,
      notification_id,
      notif_type,
      sender_id: None,
      sender_type: None,
      timestamp: timestamp.into(),
    }
  }

  /// Sets the notification body text.
  pub fn set_body(&mut self, body: Option<String>) -> &mut Self {
    self.body = body;
    self
  }

  /// Sets the category, overriding the value derived at construction.
  pub fn set_category(&mut self, category: NotificationCategory) -> &mut Self {
    self.category = category;
    self
  }

  /// Sets whether the notification has been read.
  pub fn set_is_read(&mut self, is_read: bool) -> &mut Self {
    self.is_read = is_read;
    self
  }

  /// Sets the notification ID.
  pub fn set_notification_id(&mut self, notification_id: i64) -> &mut Self {
    self.notification_id = notification_id;
    self
  }

  /// Sets the raw notification type string.
  pub fn set_notif_type(&mut self, notif_type: impl Into<String>) -> &mut Self {
    self.notif_type = notif_type.into();
    self
  }

  /// Sets the sender entity ID.
  pub fn set_sender_id(&mut self, sender_id: Option<i64>) -> &mut Self {
    self.sender_id = sender_id;
    self
  }

  /// Sets the sender entity type.
  pub fn set_sender_type(&mut self, sender_type: Option<String>) -> &mut Self {
    self.sender_type = sender_type;
    self
  }

  /// Sets the notification timestamp.
  pub fn set_timestamp(&mut self, timestamp: impl Into<String>) -> &mut Self {
    self.timestamp = timestamp.into();
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod categorize_notif {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_war_for_war_declared() {
      assert_eq!(categorize_notif("WarDeclared"), NotificationCategory::War);
    }

    #[test]
    fn it_returns_contract_for_structure_courier_contract_changed() {
      assert_eq!(
        categorize_notif("StructureCourierContractChanged"),
        NotificationCategory::Contract
      );
    }

    #[test]
    fn it_returns_system_for_tutorial_msg() {
      assert_eq!(categorize_notif("TutorialMsg"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_combat_for_kill_report_final_blow() {
      assert_eq!(categorize_notif("KillReportFinalBlow"), NotificationCategory::Combat);
    }

    #[test]
    fn it_returns_standing_for_npc_standings_gained() {
      assert_eq!(categorize_notif("NPCStandingsGained"), NotificationCategory::Standing);
    }

    #[test]
    fn it_returns_system_for_unknown_future_type() {
      assert_eq!(categorize_notif("UnknownFutureType"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_alliance_for_alliance_capital_changed() {
      assert_eq!(
        categorize_notif("AllianceCapitalChanged"),
        NotificationCategory::Alliance
      );
    }

    #[test]
    fn it_returns_combat_for_combat_operation_finished() {
      assert_eq!(
        categorize_notif("CombatOperationFinished"),
        NotificationCategory::Combat
      );
    }

    #[test]
    fn it_returns_system_for_old_lsc_messages() {
      assert_eq!(categorize_notif("OldLscMessages"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_insurance_for_insurance_prefix() {
      assert_eq!(
        categorize_notif("InsuranceExpirationMsg"),
        NotificationCategory::Insurance
      );
    }

    #[test]
    fn it_returns_clone_for_clone_prefix() {
      assert_eq!(categorize_notif("CloneActivationMsg"), NotificationCategory::Clone);
    }

    #[test]
    fn it_returns_clone_for_jump_clone_prefix() {
      assert_eq!(categorize_notif("JumpCloneDeletedMsg"), NotificationCategory::Clone);
    }

    #[test]
    fn it_returns_fw_for_fw_prefix() {
      assert_eq!(categorize_notif("FWAllianceWarningMsg"), NotificationCategory::Fw);
    }

    #[test]
    fn it_returns_fw_for_facwar_prefix() {
      assert_eq!(categorize_notif("FacWarLPPayoutKill"), NotificationCategory::Fw);
    }

    #[test]
    fn it_returns_combat_for_bounty_prefix() {
      assert_eq!(categorize_notif("BountyPlacedChar"), NotificationCategory::Combat);
    }

    #[test]
    fn it_returns_combat_for_kill_right_prefix() {
      assert_eq!(categorize_notif("KillRightAvailable"), NotificationCategory::Combat);
    }

    #[test]
    fn it_returns_mission_for_mission_prefix() {
      assert_eq!(
        categorize_notif("MissionOfferExpirationMsg"),
        NotificationCategory::Mission
      );
    }

    #[test]
    fn it_returns_industry_for_moonmining_prefix() {
      assert_eq!(
        categorize_notif("MoonminingAutomaticFracture"),
        NotificationCategory::Industry
      );
    }

    #[test]
    fn it_returns_incursion_for_incursion_prefix() {
      assert_eq!(
        categorize_notif("IncursionCompletedMsg"),
        NotificationCategory::Incursion
      );
    }

    #[test]
    fn it_returns_contact_for_contact_prefix() {
      assert_eq!(categorize_notif("ContactAdd"), NotificationCategory::Contact);
    }

    #[test]
    fn it_returns_market_for_bill_prefix() {
      assert_eq!(categorize_notif("BillOutOfMoneyMsg"), NotificationCategory::Market);
    }

    #[test]
    fn it_returns_reward_for_daily_item_reward_prefix() {
      assert_eq!(categorize_notif("DailyItemRewardMsg"), NotificationCategory::Reward);
    }

    #[test]
    fn it_returns_structure_for_structure_prefix() {
      assert_eq!(
        categorize_notif("StructureUnderAttack"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_corp_for_corp_prefix() {
      assert_eq!(categorize_notif("CorpAppNewMsg"), NotificationCategory::Corp);
    }

    #[test]
    fn it_returns_war_for_surrender_suffix() {
      assert_eq!(categorize_notif("WarSurrenderDeclinedMsg"), NotificationCategory::War);
    }

    #[test]
    fn it_returns_structure_for_contract_on_structure_courier_prefix() {
      assert_eq!(
        categorize_notif("StructureCourierContractFailed"),
        NotificationCategory::Contract
      );
    }
  }
}
