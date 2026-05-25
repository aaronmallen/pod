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
  categorize_early_groups(notif_type)
    .or_else(|| categorize_late_groups(notif_type))
    .unwrap_or(NotificationCategory::System)
}

/// Checks explicit overrides and the first group of domain-prefix rules.
fn categorize_early_groups(notif_type: &str) -> Option<NotificationCategory> {
  categorize_override(notif_type)
    .or_else(|| categorize_early_domain_a(notif_type))
    .or_else(|| categorize_early_domain_b(notif_type))
}

fn categorize_early_domain_a(notif_type: &str) -> Option<NotificationCategory> {
  categorize_contract_notif(notif_type)
    .or_else(|| categorize_insurance_notif(notif_type))
    .or_else(|| categorize_clone_notif(notif_type))
    .or_else(|| categorize_fw_notif(notif_type))
}

fn categorize_early_domain_b(notif_type: &str) -> Option<NotificationCategory> {
  categorize_standing_notif(notif_type)
    .or_else(|| categorize_combat_notif(notif_type))
    .or_else(|| categorize_mission_notif(notif_type))
    .or_else(|| categorize_industry_notif(notif_type))
}

/// Checks the second group of domain-prefix rules, including war and structure.
fn categorize_late_groups(notif_type: &str) -> Option<NotificationCategory> {
  categorize_late_domain_a(notif_type).or_else(|| categorize_late_domain_b(notif_type))
}

fn categorize_late_domain_a(notif_type: &str) -> Option<NotificationCategory> {
  categorize_incursion_notif(notif_type)
    .or_else(|| categorize_contact_notif(notif_type))
    .or_else(|| categorize_market_notif(notif_type))
    .or_else(|| categorize_reward_notif(notif_type))
}

fn categorize_late_domain_b(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("FreelanceProject") {
    return Some(NotificationCategory::System);
  }
  categorize_war_notif(notif_type)
    .or_else(|| categorize_structure_notif(notif_type))
    .or_else(|| categorize_corp_notif(notif_type))
}

/// Handles explicit type-string overrides that must win before any prefix rule.
fn categorize_override(notif_type: &str) -> Option<NotificationCategory> {
  const OVERRIDES: &[(&str, NotificationCategory)] = &[
    ("AllianceCapitalChanged", NotificationCategory::Alliance),
    ("AllWarCorpJoinedAllianceMsg", NotificationCategory::War),
    ("BattlePunishFriendlyFire", NotificationCategory::Combat),
    ("CombatOperationFinished", NotificationCategory::Combat),
    ("ContainerPasswordMsg", NotificationCategory::System),
    ("OldLscMessages", NotificationCategory::System),
    ("OperationFinished", NotificationCategory::Combat),
    ("StructureCourierContractChanged", NotificationCategory::Contract),
    ("TutorialMsg", NotificationCategory::System),
  ];
  OVERRIDES
    .iter()
    .find(|(key, _)| *key == notif_type)
    .map(|(_, cat)| cat.clone())
}

/// Classifies structure-courier contract notifications.
fn categorize_contract_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("StructureCourier") {
    Some(NotificationCategory::Contract)
  } else {
    None
  }
}

/// Classifies insurance notifications.
fn categorize_insurance_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("Insurance") {
    Some(NotificationCategory::Insurance)
  } else {
    None
  }
}

/// Classifies clone and jump-clone notifications.
fn categorize_clone_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("Clone") || notif_type.starts_with("JumpClone") {
    Some(NotificationCategory::Clone)
  } else {
    None
  }
}

/// Classifies factional warfare notifications.
fn categorize_fw_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("FW") || notif_type.starts_with("FacWar") {
    Some(NotificationCategory::Fw)
  } else {
    None
  }
}

/// Classifies NPC standing change notifications.
fn categorize_standing_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("NPCStandings") {
    Some(NotificationCategory::Standing)
  } else {
    None
  }
}

/// Classifies combat, kill-report, and bounty notifications.
fn categorize_combat_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("Bounty") || notif_type.starts_with("KillReport") || notif_type.starts_with("KillRight") {
    Some(NotificationCategory::Combat)
  } else {
    None
  }
}

/// Classifies agent mission and storyline notifications.
fn categorize_mission_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("Mission")
    || notif_type.starts_with("ResearchMission")
    || notif_type.starts_with("StoryLine")
    || notif_type.starts_with("AgentRetired")
  {
    Some(NotificationCategory::Mission)
  } else {
    None
  }
}

/// Classifies industry, moon-mining, and manufacturing job notifications.
fn categorize_industry_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("Moonmining")
    || notif_type.starts_with("IndustryOperation")
    || notif_type.starts_with("IndustryTeam")
    || notif_type.starts_with("StructuresJobs")
  {
    Some(NotificationCategory::Industry)
  } else {
    None
  }
}

/// Classifies incursion and invasion notifications.
fn categorize_incursion_notif(notif_type: &str) -> Option<NotificationCategory> {
  const PREFIXES: &[&str] = &[
    "ContractRegionChanged",
    "DistrictAttacked",
    "DustApp",
    "Incursion",
    "Invasion",
  ];
  if PREFIXES.iter().any(|p| notif_type.starts_with(p)) {
    Some(NotificationCategory::Incursion)
  } else {
    None
  }
}

/// Classifies contact-list change notifications.
fn categorize_contact_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("Contact") || notif_type.starts_with("BuddyConnect") || notif_type.starts_with("LocateChar")
  {
    Some(NotificationCategory::Contact)
  } else {
    None
  }
}

/// Classifies market, billing, and transaction notifications.
fn categorize_market_notif(notif_type: &str) -> Option<NotificationCategory> {
  const PREFIXES: &[&str] = &[
    "Bill",
    "Customs",
    "Raffle",
    "Reimbursement",
    "StructurePaint",
    "Transaction",
  ];
  if PREFIXES.iter().any(|p| notif_type.starts_with(p)) {
    Some(NotificationCategory::Market)
  } else {
    None
  }
}

/// Classifies reward, gift, and seasonal-challenge notifications.
fn categorize_reward_notif(notif_type: &str) -> Option<NotificationCategory> {
  const PREFIXES: &[&str] = &[
    "DailyItemReward",
    "ExpertSystem",
    "GameTime",
    "Gift",
    "LPAutoRedeemed",
    "SPAutoRedeemed",
    "SeasonalChallenge",
    "SkinSequencing",
  ];
  if PREFIXES.iter().any(|p| notif_type.starts_with(p)) {
    Some(NotificationCategory::Reward)
  } else {
    None
  }
}

/// Classifies war declaration and mercenary notifications.
fn categorize_war_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.contains("War")
    || notif_type.starts_with("Ally")
    || notif_type.contains("Surrender")
    || notif_type.starts_with("MercOffer")
  {
    Some(NotificationCategory::War)
  } else {
    None
  }
}

/// Classifies structure, sovereignty, station, and tower notifications.
fn categorize_structure_notif(notif_type: &str) -> Option<NotificationCategory> {
  const PREFIXES: &[&str] = &[
    "AllAnchoring",
    "AllMaintenance",
    "AllStruc",
    "EntosisCapture",
    "IHub",
    "Infrastructure",
    "Mercenary",
    "Orbital",
    "OwnershipTransferred",
    "Skyhook",
    "Sov",
    "Sovereignty",
    "Station",
    "Structure",
    "Tower",
  ];
  if PREFIXES.iter().any(|p| notif_type.starts_with(p)) {
    Some(NotificationCategory::Structure)
  } else {
    None
  }
}

/// Classifies corporation notifications.
fn categorize_corp_notif(notif_type: &str) -> Option<NotificationCategory> {
  if notif_type.starts_with("CorporationGoal")
    || notif_type.starts_with("CorporationLeft")
    || notif_type.starts_with("Char")
    || notif_type.starts_with("Corp")
  {
    Some(NotificationCategory::Corp)
  } else {
    None
  }
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
  /// Raw EVE notification type string.
  #[get = "pub"]
  notif_type: String,
  /// Sender entity ID, if available.
  #[get = "pub"]
  sender_id: Option<i64>,
  /// Sender entity type string, if available.
  #[get = "pub"]
  sender_type: Option<String>,
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

  mod categorize_structure_notif {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_none_for_non_structure_type() {
      assert_eq!(categorize_structure_notif("UnknownMsg"), None);
    }

    #[test]
    fn it_returns_structure_for_all_anchoring_prefix() {
      assert_eq!(
        categorize_structure_notif("AllAnchoringMsg"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_all_maintenance_prefix() {
      assert_eq!(
        categorize_structure_notif("AllMaintenanceBillMsg"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_all_struc_prefix() {
      assert_eq!(
        categorize_structure_notif("AllStructureOnline"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_entosis_capture_prefix() {
      assert_eq!(
        categorize_structure_notif("EntosisCaptureMsgStandard"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_ihub_prefix() {
      assert_eq!(
        categorize_structure_notif("IHubDestroyedByBillFailure"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_infrastructure_prefix() {
      assert_eq!(
        categorize_structure_notif("InfrastructureHubLowPower"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_mercenary_prefix() {
      assert_eq!(
        categorize_structure_notif("MercenaryDenAttacked"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_orbital_prefix() {
      assert_eq!(
        categorize_structure_notif("OrbitalAttacked"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_ownership_transferred_prefix() {
      assert_eq!(
        categorize_structure_notif("OwnershipTransferred"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_skyhook_prefix() {
      assert_eq!(
        categorize_structure_notif("SkyhookDeployed"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_sov_prefix() {
      assert_eq!(
        categorize_structure_notif("SovCommandNodeEventStarted"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_sovereignty_prefix() {
      assert_eq!(
        categorize_structure_notif("SovereigntyIHubBillLate"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_station_prefix() {
      assert_eq!(
        categorize_structure_notif("StationAggressionMsg"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_structure_prefix() {
      assert_eq!(
        categorize_structure_notif("StructureUnderAttack"),
        Some(NotificationCategory::Structure)
      );
    }

    #[test]
    fn it_returns_structure_for_tower_prefix() {
      assert_eq!(
        categorize_structure_notif("TowerAlertMsg"),
        Some(NotificationCategory::Structure)
      );
    }
  }

  mod categorize_notif {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_alliance_for_alliance_capital_changed() {
      assert_eq!(
        categorize_notif("AllianceCapitalChanged"),
        NotificationCategory::Alliance
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
    fn it_returns_combat_for_battle_punish_friendly_fire() {
      assert_eq!(
        categorize_notif("BattlePunishFriendlyFire"),
        NotificationCategory::Combat
      );
    }

    #[test]
    fn it_returns_combat_for_bounty_prefix() {
      assert_eq!(categorize_notif("BountyPlacedChar"), NotificationCategory::Combat);
    }

    #[test]
    fn it_returns_combat_for_combat_operation_finished() {
      assert_eq!(
        categorize_notif("CombatOperationFinished"),
        NotificationCategory::Combat
      );
    }

    #[test]
    fn it_returns_combat_for_kill_report_final_blow() {
      assert_eq!(categorize_notif("KillReportFinalBlow"), NotificationCategory::Combat);
    }

    #[test]
    fn it_returns_combat_for_kill_right_prefix() {
      assert_eq!(categorize_notif("KillRightAvailable"), NotificationCategory::Combat);
    }

    #[test]
    fn it_returns_combat_for_operation_finished() {
      assert_eq!(categorize_notif("OperationFinished"), NotificationCategory::Combat);
    }

    #[test]
    fn it_returns_contact_for_buddy_connect_prefix() {
      assert_eq!(
        categorize_notif("BuddyConnectContactAdd"),
        NotificationCategory::Contact
      );
    }

    #[test]
    fn it_returns_contact_for_contact_prefix() {
      assert_eq!(categorize_notif("ContactAdd"), NotificationCategory::Contact);
    }

    #[test]
    fn it_returns_contact_for_locate_char_prefix() {
      assert_eq!(categorize_notif("LocateCharMsg"), NotificationCategory::Contact);
    }

    #[test]
    fn it_returns_contract_for_structure_courier_contract_changed() {
      assert_eq!(
        categorize_notif("StructureCourierContractChanged"),
        NotificationCategory::Contract
      );
    }

    #[test]
    fn it_returns_contract_for_structure_courier_prefix() {
      assert_eq!(
        categorize_notif("StructureCourierContractFailed"),
        NotificationCategory::Contract
      );
    }

    #[test]
    fn it_returns_corp_for_char_prefix() {
      assert_eq!(categorize_notif("CharAppAcceptMsg"), NotificationCategory::Corp);
    }

    #[test]
    fn it_returns_corp_for_corp_prefix() {
      assert_eq!(categorize_notif("CorpAppNewMsg"), NotificationCategory::Corp);
    }

    #[test]
    fn it_returns_corp_for_corporation_goal_prefix() {
      assert_eq!(categorize_notif("CorporationGoalClosed"), NotificationCategory::Corp);
    }

    #[test]
    fn it_returns_corp_for_corporation_left_prefix() {
      assert_eq!(categorize_notif("CorporationLeft"), NotificationCategory::Corp);
    }

    #[test]
    fn it_returns_fw_for_facwar_prefix() {
      assert_eq!(categorize_notif("FacWarLPPayoutKill"), NotificationCategory::Fw);
    }

    #[test]
    fn it_returns_fw_for_fw_prefix() {
      assert_eq!(categorize_notif("FWAllianceWarningMsg"), NotificationCategory::Fw);
    }

    #[test]
    fn it_returns_incursion_for_contract_region_changed_prefix() {
      assert_eq!(
        categorize_notif("ContractRegionChanged"),
        NotificationCategory::Incursion
      );
    }

    #[test]
    fn it_returns_incursion_for_district_attacked_prefix() {
      assert_eq!(categorize_notif("DistrictAttackedMsg"), NotificationCategory::Incursion);
    }

    #[test]
    fn it_returns_incursion_for_dust_app_prefix() {
      assert_eq!(categorize_notif("DustAppNotification"), NotificationCategory::Incursion);
    }

    #[test]
    fn it_returns_incursion_for_incursion_prefix() {
      assert_eq!(
        categorize_notif("IncursionCompletedMsg"),
        NotificationCategory::Incursion
      );
    }

    #[test]
    fn it_returns_incursion_for_invasion_prefix() {
      assert_eq!(
        categorize_notif("InvasionCompletionMsg"),
        NotificationCategory::Incursion
      );
    }

    #[test]
    fn it_returns_industry_for_industry_operation_prefix() {
      assert_eq!(
        categorize_notif("IndustryOperationFinished"),
        NotificationCategory::Industry
      );
    }

    #[test]
    fn it_returns_industry_for_industry_team_prefix() {
      assert_eq!(
        categorize_notif("IndustryTeamAuctionWon"),
        NotificationCategory::Industry
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
    fn it_returns_industry_for_structures_jobs_prefix() {
      assert_eq!(
        categorize_notif("StructuresJobsCancelled"),
        NotificationCategory::Industry
      );
    }

    #[test]
    fn it_returns_insurance_for_insurance_prefix() {
      assert_eq!(
        categorize_notif("InsuranceExpirationMsg"),
        NotificationCategory::Insurance
      );
    }

    #[test]
    fn it_returns_market_for_bill_prefix() {
      assert_eq!(categorize_notif("BillOutOfMoneyMsg"), NotificationCategory::Market);
    }

    #[test]
    fn it_returns_market_for_customs_prefix() {
      assert_eq!(categorize_notif("CustomsMsg"), NotificationCategory::Market);
    }

    #[test]
    fn it_returns_market_for_raffle_prefix() {
      assert_eq!(categorize_notif("RaffleCreated"), NotificationCategory::Market);
    }

    #[test]
    fn it_returns_market_for_reimbursement_prefix() {
      assert_eq!(categorize_notif("ReimbursementMsg"), NotificationCategory::Market);
    }

    #[test]
    fn it_returns_market_for_structure_paint_prefix() {
      assert_eq!(
        categorize_notif("StructurePaintPurchased"),
        NotificationCategory::Market
      );
    }

    #[test]
    fn it_returns_market_for_transaction_prefix() {
      assert_eq!(categorize_notif("TransactionReversalMsg"), NotificationCategory::Market);
    }

    #[test]
    fn it_returns_mission_for_agent_retired_prefix() {
      assert_eq!(
        categorize_notif("AgentRetiredTrigravian"),
        NotificationCategory::Mission
      );
    }

    #[test]
    fn it_returns_mission_for_mission_prefix() {
      assert_eq!(
        categorize_notif("MissionOfferExpirationMsg"),
        NotificationCategory::Mission
      );
    }

    #[test]
    fn it_returns_mission_for_research_mission_prefix() {
      assert_eq!(
        categorize_notif("ResearchMissionAvailableMsg"),
        NotificationCategory::Mission
      );
    }

    #[test]
    fn it_returns_mission_for_story_line_prefix() {
      assert_eq!(
        categorize_notif("StoryLineMissionAvailableMsg"),
        NotificationCategory::Mission
      );
    }

    #[test]
    fn it_returns_reward_for_daily_item_reward_prefix() {
      assert_eq!(categorize_notif("DailyItemRewardMsg"), NotificationCategory::Reward);
    }

    #[test]
    fn it_returns_reward_for_expert_system_prefix() {
      assert_eq!(categorize_notif("ExpertSystemExpired"), NotificationCategory::Reward);
    }

    #[test]
    fn it_returns_reward_for_game_time_prefix() {
      assert_eq!(categorize_notif("GameTimeAdded"), NotificationCategory::Reward);
    }

    #[test]
    fn it_returns_reward_for_gift_prefix() {
      assert_eq!(categorize_notif("GiftReceived"), NotificationCategory::Reward);
    }

    #[test]
    fn it_returns_reward_for_lp_auto_redeemed_prefix() {
      assert_eq!(categorize_notif("LPAutoRedeemed"), NotificationCategory::Reward);
    }

    #[test]
    fn it_returns_reward_for_seasonal_challenge_prefix() {
      assert_eq!(
        categorize_notif("SeasonalChallengeCompleted"),
        NotificationCategory::Reward
      );
    }

    #[test]
    fn it_returns_reward_for_skin_sequencing_prefix() {
      assert_eq!(
        categorize_notif("SkinSequencingCompleted"),
        NotificationCategory::Reward
      );
    }

    #[test]
    fn it_returns_reward_for_sp_auto_redeemed_prefix() {
      assert_eq!(categorize_notif("SPAutoRedeemed"), NotificationCategory::Reward);
    }

    #[test]
    fn it_returns_standing_for_npc_standings_gained() {
      assert_eq!(categorize_notif("NPCStandingsGained"), NotificationCategory::Standing);
    }

    #[test]
    fn it_returns_structure_for_all_anchoring_prefix() {
      assert_eq!(categorize_notif("AllAnchoringMsg"), NotificationCategory::Structure);
    }

    #[test]
    fn it_returns_structure_for_all_maintenance_prefix() {
      assert_eq!(
        categorize_notif("AllMaintenanceBillMsg"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_all_struc_prefix() {
      assert_eq!(categorize_notif("AllStructureOnline"), NotificationCategory::Structure);
    }

    #[test]
    fn it_returns_structure_for_entosis_capture_prefix() {
      assert_eq!(
        categorize_notif("EntosisCaptureMsgStandard"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_ihub_prefix() {
      assert_eq!(
        categorize_notif("IHubDestroyedByBillFailure"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_infrastructure_prefix() {
      assert_eq!(
        categorize_notif("InfrastructureHubLowPower"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_mercenary_prefix() {
      assert_eq!(
        categorize_notif("MercenaryDenAttacked"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_orbital_prefix() {
      assert_eq!(categorize_notif("OrbitalAttacked"), NotificationCategory::Structure);
    }

    #[test]
    fn it_returns_structure_for_ownership_transferred_prefix() {
      assert_eq!(
        categorize_notif("OwnershipTransferred"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_skyhook_prefix() {
      assert_eq!(categorize_notif("SkyhookDeployed"), NotificationCategory::Structure);
    }

    #[test]
    fn it_returns_structure_for_sov_prefix() {
      assert_eq!(
        categorize_notif("SovCommandNodeEventStarted"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_sovereignty_prefix() {
      assert_eq!(
        categorize_notif("SovereigntyIHubBillLate"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_station_prefix() {
      assert_eq!(
        categorize_notif("StationAggressionMsg"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_structure_prefix() {
      assert_eq!(
        categorize_notif("StructureUnderAttack"),
        NotificationCategory::Structure
      );
    }

    #[test]
    fn it_returns_structure_for_tower_prefix() {
      assert_eq!(categorize_notif("TowerAlertMsg"), NotificationCategory::Structure);
    }

    #[test]
    fn it_returns_system_for_container_password_msg() {
      assert_eq!(categorize_notif("ContainerPasswordMsg"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_system_for_freelance_project_prefix() {
      assert_eq!(categorize_notif("FreelanceProjectMsg"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_system_for_old_lsc_messages() {
      assert_eq!(categorize_notif("OldLscMessages"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_system_for_tutorial_msg() {
      assert_eq!(categorize_notif("TutorialMsg"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_system_for_unknown_future_type() {
      assert_eq!(categorize_notif("UnknownFutureType"), NotificationCategory::System);
    }

    #[test]
    fn it_returns_war_for_all_war_corp_joined_alliance_msg() {
      assert_eq!(
        categorize_notif("AllWarCorpJoinedAllianceMsg"),
        NotificationCategory::War
      );
    }

    #[test]
    fn it_returns_war_for_ally_prefix() {
      assert_eq!(categorize_notif("AllyJoinedWarDefMsg"), NotificationCategory::War);
    }

    #[test]
    fn it_returns_war_for_merc_offer_prefix() {
      assert_eq!(categorize_notif("MercOfferMsg"), NotificationCategory::War);
    }

    #[test]
    fn it_returns_war_for_surrender_suffix() {
      assert_eq!(categorize_notif("WarSurrenderDeclinedMsg"), NotificationCategory::War);
    }

    #[test]
    fn it_returns_war_for_war_declared() {
      assert_eq!(categorize_notif("WarDeclared"), NotificationCategory::War);
    }
  }
}
