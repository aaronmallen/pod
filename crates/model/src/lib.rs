//! Domain models shared across the Pod crate ecosystem.

mod asset_sync_state;
mod bloodline;
mod certificate;
mod character;
mod character_asset;
mod character_attributes;
mod character_contract;
mod character_detail;
mod character_skill;
mod clone;
mod constellation;
mod contact;
mod corporation;
mod corporation_asset;
mod faction;
mod item_category;
mod item_group;
mod item_type;
mod item_type_summary;
mod kill_entry;
mod mail_header;
mod market_group;
mod neural_attributes;
mod notification;
mod planet;
mod race;
mod region;
mod skill_group;
mod skill_plan;
mod skill_plan_entry;
mod solar_system;
mod standing;
mod star;
mod stargate;
mod station;
mod wallet_journal_entry;
mod wallet_transaction;

pub use asset_sync_state::Model as AssetSyncState;
pub use bloodline::Model as Bloodline;
pub use certificate::Certificate;
pub use character::{Model as Character, TrainingQueueEntry};
pub use character_asset::Model as CharacterAsset;
pub use character_attributes::Model as CharacterAttributes;
pub use character_contract::Model as CharacterContract;
pub use character_detail::{
  CharacterClone, CharacterContact, CharacterContactLabel, CharacterImplant, CharacterKillEntry, CharacterNotification,
  CharacterStanding,
};
pub use character_skill::Model as CharacterSkill;
pub use clone::{Clone, CloneImplant};
pub use constellation::Model as Constellation;
pub use contact::{Contact, ContactLabel, ContactType};
pub use corporation::Model as Corporation;
pub use corporation_asset::Model as CorporationAsset;
pub use faction::Model as Faction;
pub use item_category::Model as ItemCategory;
pub use item_group::Model as ItemGroup;
pub use item_type::{DogmaAttributeEntry, DogmaEffectEntry, Model as ItemType};
pub use item_type_summary::ItemTypeSummary;
pub use kill_entry::KillEntry;
pub use mail_header::Model as MailHeader;
pub use market_group::Model as MarketGroup;
pub use neural_attributes::NeuralAttributes;
pub use notification::{Notification, NotificationCategory, categorize_notif};
pub use planet::Model as Planet;
pub use race::Model as Race;
pub use region::Model as Region;
pub use skill_group::{AttrKey, SkillDef, SkillGroupDef};
pub use skill_plan::SkillPlan;
pub use skill_plan_entry::SkillPlanEntry;
pub use solar_system::Model as SolarSystem;
pub use standing::{FromType, Standing};
pub use star::Model as Star;
pub use stargate::Model as Stargate;
pub use station::Model as Station;
pub use wallet_journal_entry::Model as WalletJournalEntry;
pub use wallet_transaction::Model as WalletTransaction;

/// Returns the subset of `required` scopes that are absent from `granted`.
pub fn missing_scopes(granted: &[&str], required: &[&'static str]) -> Vec<&'static str> {
  required.iter().copied().filter(|s| !granted.contains(s)).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod missing_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_all_required_when_granted_is_empty() {
      let granted: &[&str] = &[];
      let required: &[&'static str] = &["esi-mail.read", "esi-skills.read"];

      assert_eq!(
        missing_scopes(granted, required),
        vec!["esi-mail.read", "esi-skills.read"]
      );
    }

    #[test]
    fn it_returns_empty_when_all_granted() {
      let granted = &["esi-mail.read", "esi-skills.read"];
      let required: &[&'static str] = &["esi-mail.read", "esi-skills.read"];

      assert_eq!(missing_scopes(granted, required), Vec::<&str>::new());
    }

    #[test]
    fn it_returns_only_missing_scopes() {
      let granted = &["esi-mail.read"];
      let required: &[&'static str] = &["esi-mail.read", "esi-skills.read"];

      assert_eq!(missing_scopes(granted, required), vec!["esi-skills.read"]);
    }
  }
}
