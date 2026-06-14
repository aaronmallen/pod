use crate::{
  clients::esi::scopes, config::Feature, features::character_detail::Tab, sync::JobKind,
  ui::components::rail::Destination,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Descriptor {
  pub jobs: &'static [JobKind],
  /// `None` when the feature has no top-level nav-rail entry (lives inside character detail only).
  pub rail: Option<Destination>,
  pub scopes: &'static [&'static str],
  /// `None` when the feature is a top-level screen rather than a character-detail tab.
  pub tab: Option<Tab>,
}

pub fn descriptor(feature: Feature) -> Descriptor {
  match feature {
    Feature::AssetTracking => Descriptor {
      jobs: &[JobKind::AssetSync, JobKind::CharacterAbyssals],
      rail: Some(Destination::Assets),
      scopes: &[scopes::CHARACTER_ASSETS],
      tab: None,
    },
    Feature::Calendar => Descriptor {
      jobs: &[JobKind::CharacterCalendar],
      rail: Some(Destination::Calendar),
      scopes: &[scopes::CHARACTER_CALENDAR_READ, scopes::CHARACTER_CALENDAR_RESPOND],
      tab: None,
    },
    Feature::CloneMonitoring => Descriptor {
      jobs: &[JobKind::CharacterClones],
      rail: None,
      scopes: &[scopes::CHARACTER_CLONES],
      tab: Some(Tab::Clones),
    },
    Feature::CombatLog => Descriptor {
      jobs: &[JobKind::CharacterKillmails, JobKind::KillmailReconcile],
      rail: None,
      scopes: &[scopes::CHARACTER_KILLMAILS],
      tab: Some(Tab::Killlog),
    },
    Feature::Contacts => Descriptor {
      jobs: &[JobKind::CharacterContacts],
      rail: None,
      scopes: &[scopes::CHARACTER_CONTACTS, scopes::CHARACTER_CONTACTS_WRITE],
      tab: Some(Tab::Contacts),
    },
    Feature::EveNotifications => Descriptor {
      jobs: &[JobKind::CharacterNotifications],
      rail: None,
      scopes: &[scopes::CHARACTER_NOTIFICATIONS],
      tab: Some(Tab::Notifications),
    },
    Feature::Industry => Descriptor {
      jobs: &[
        JobKind::CharacterBlueprints,
        JobKind::CharacterIndustryJobs,
        JobKind::CorporationBlueprints,
        JobKind::CorporationIndustryJobs,
      ],
      rail: Some(Destination::Industry),
      scopes: &[
        scopes::CHARACTER_BLUEPRINTS,
        scopes::CHARACTER_INDUSTRY_JOBS,
        scopes::CORPORATION_BLUEPRINTS,
        scopes::CORPORATION_INDUSTRY_JOBS,
      ],
      tab: None,
    },
    Feature::LocationTracking => Descriptor {
      jobs: &[JobKind::CharacterTelemetry],
      rail: None,
      scopes: &[
        scopes::CHARACTER_LOCATION,
        scopes::CHARACTER_ONLINE,
        scopes::CHARACTER_SHIP,
        scopes::UNIVERSE_STRUCTURES,
      ],
      tab: None,
    },
    Feature::Mail => Descriptor {
      jobs: &[JobKind::CharacterMail],
      rail: Some(Destination::Mail),
      scopes: &[
        scopes::CHARACTER_MAIL,
        scopes::CHARACTER_MAIL_SEND,
        scopes::CHARACTER_MAIL_ORGANIZE,
        scopes::CHARACTER_SEARCH,
      ],
      tab: None,
    },
    Feature::SkillMonitoring => Descriptor {
      jobs: &[JobKind::CharacterSkills],
      rail: Some(Destination::Skills),
      scopes: &[
        scopes::CHARACTER_SKILLS,
        scopes::CHARACTER_SKILLQUEUE,
        scopes::CHARACTER_IMPLANTS,
      ],
      tab: None,
    },
    Feature::Standings => Descriptor {
      jobs: &[JobKind::CharacterStandings],
      rail: None,
      scopes: &[scopes::CHARACTER_STANDINGS],
      tab: Some(Tab::Standings),
    },
    Feature::Wallet => Descriptor {
      jobs: &[
        JobKind::CharacterContracts,
        JobKind::CharacterMarketOrders,
        JobKind::CharacterWallet,
        JobKind::CorporationWallet,
        JobKind::MarketPrices,
        JobKind::NetWorthSnapshot,
      ],
      rail: Some(Destination::Wallet),
      scopes: &[scopes::CHARACTER_WALLET, scopes::CHARACTER_CONTRACTS],
      tab: None,
    },
  }
}

pub fn feature_for_destination(destination: Destination) -> Option<Feature> {
  Feature::ALL
    .into_iter()
    .find(|&feature| descriptor(feature).rail == Some(destination))
}

pub fn feature_for_job(job: JobKind) -> Option<Feature> {
  Feature::ALL
    .into_iter()
    .find(|&feature| descriptor(feature).jobs.contains(&job))
}

pub fn feature_for_tab(tab: Tab) -> Option<Feature> {
  Feature::ALL
    .into_iter()
    .find(|&feature| descriptor(feature).tab == Some(tab))
}

#[cfg(test)]
mod tests {
  use super::*;

  mod descriptor {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_feature_maps_to_a_nonempty_scope_set() {
      for feature in Feature::ALL {
        assert!(
          !descriptor(feature).scopes.is_empty(),
          "{feature:?} must map to at least one scope"
        );
      }
    }

    #[test]
    fn every_feature_maps_to_at_least_one_job() {
      for feature in Feature::ALL {
        assert!(
          !descriptor(feature).jobs.is_empty(),
          "{feature:?} must drive at least one sync job"
        );
      }
    }

    #[test]
    fn no_job_is_owned_by_two_features() {
      let mut seen: HashSet<JobKind> = HashSet::new();

      for feature in Feature::ALL {
        for &job in descriptor(feature).jobs {
          assert!(seen.insert(job), "{job:?} is claimed by more than one feature");
        }
      }
    }

    #[test]
    fn the_featureless_jobs_belong_to_no_feature() {
      for job in [JobKind::CharacterProfile, JobKind::CorporationProfile] {
        assert_eq!(
          feature_for_job(job),
          None,
          "{job:?} must always run regardless of features"
        );
      }
    }
  }

  mod feature_for_destination {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_the_feature_backed_destinations() {
      assert_eq!(
        feature_for_destination(Destination::Assets),
        Some(Feature::AssetTracking)
      );
      assert_eq!(feature_for_destination(Destination::Calendar), Some(Feature::Calendar));
      assert_eq!(feature_for_destination(Destination::Industry), Some(Feature::Industry));
      assert_eq!(feature_for_destination(Destination::Mail), Some(Feature::Mail));
      assert_eq!(
        feature_for_destination(Destination::Skills),
        Some(Feature::SkillMonitoring)
      );
      assert_eq!(feature_for_destination(Destination::Wallet), Some(Feature::Wallet));
    }

    #[test]
    fn it_leaves_always_on_destinations_unmapped() {
      assert_eq!(feature_for_destination(Destination::Characters), None);
      assert_eq!(feature_for_destination(Destination::Settings), None);
    }
  }

  mod feature_for_tab {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn every_tab_maps_to_a_feature() {
      for tab in [
        Tab::Clones,
        Tab::Contacts,
        Tab::Killlog,
        Tab::Notifications,
        Tab::Standings,
      ] {
        assert!(feature_for_tab(tab).is_some(), "{tab:?} must belong to a feature");
      }
    }

    #[test]
    fn it_maps_each_tab_to_its_owning_feature() {
      assert_eq!(feature_for_tab(Tab::Clones), Some(Feature::CloneMonitoring));
      assert_eq!(feature_for_tab(Tab::Contacts), Some(Feature::Contacts));
      assert_eq!(feature_for_tab(Tab::Killlog), Some(Feature::CombatLog));
      assert_eq!(feature_for_tab(Tab::Notifications), Some(Feature::EveNotifications));
      assert_eq!(feature_for_tab(Tab::Standings), Some(Feature::Standings));
    }
  }
}
