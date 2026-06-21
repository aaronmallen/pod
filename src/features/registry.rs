use crate::{
  clients::esi::scopes,
  config::{Feature, SubFeature},
  features::character_detail::Tab,
  sync::JobKind,
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

/// The registry entry for a single sub-feature; the group [`Descriptor`] is the roll-up over the
/// children returned by [`Feature::sub_features`].
///
/// The granular grain is consumed by sibling tasks B (scope/job derivation) and D (rail/tab/catalog);
/// until those land it is exercised only by this module's roll-up invariant tests.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubDescriptor {
  pub jobs: &'static [JobKind],
  pub rail: Option<Destination>,
  pub scopes: &'static [&'static str],
  pub tab: Option<Tab>,
}

pub fn descriptor(feature: Feature) -> Descriptor {
  match feature {
    Feature::AssetTracking => Descriptor {
      jobs: &[
        JobKind::AssetSync,
        JobKind::CharacterAbyssals,
        JobKind::CorporationAbyssals,
      ],
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
      jobs: &[
        JobKind::CharacterKillmails,
        JobKind::CorporationKillmails,
        JobKind::KillmailDetailBackfill,
        JobKind::KillmailReconcile,
      ],
      rail: None,
      scopes: &[scopes::CHARACTER_KILLMAILS],
      tab: Some(Tab::Killlog),
    },
    Feature::Contacts => Descriptor {
      jobs: &[JobKind::CharacterContacts, JobKind::CorporationContacts],
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
        JobKind::CorporationMiningExtractions,
        JobKind::CorporationStructures,
      ],
      rail: Some(Destination::Industry),
      scopes: &[
        scopes::CHARACTER_BLUEPRINTS,
        scopes::CHARACTER_INDUSTRY_JOBS,
        scopes::CHARACTER_SEARCH,
        scopes::CORPORATION_BLUEPRINTS,
        scopes::CORPORATION_INDUSTRY_JOBS,
        scopes::CORPORATION_STRUCTURES,
        scopes::UNIVERSE_STRUCTURES,
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
      jobs: &[JobKind::CharacterStandings, JobKind::CorporationStandings],
      rail: None,
      scopes: &[scopes::CHARACTER_STANDINGS],
      tab: Some(Tab::Standings),
    },
    Feature::Wallet => Descriptor {
      jobs: &[
        JobKind::CharacterContracts,
        JobKind::CharacterMarketOrders,
        JobKind::CharacterWallet,
        JobKind::CorporationContracts,
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

#[allow(dead_code)]
pub fn sub_descriptor(sub: SubFeature) -> SubDescriptor {
  match sub {
    SubFeature::Abyssals => SubDescriptor {
      jobs: &[JobKind::CharacterAbyssals, JobKind::CorporationAbyssals],
      rail: None,
      scopes: &[scopes::CHARACTER_ASSETS],
      tab: None,
    },
    SubFeature::Blueprints => SubDescriptor {
      jobs: &[JobKind::CharacterBlueprints, JobKind::CorporationBlueprints],
      rail: None,
      scopes: &[scopes::CHARACTER_BLUEPRINTS, scopes::CORPORATION_BLUEPRINTS],
      tab: None,
    },
    SubFeature::Budget => SubDescriptor {
      jobs: &[],
      rail: None,
      scopes: &[],
      tab: None,
    },
    SubFeature::Calendar => SubDescriptor {
      jobs: &[JobKind::CharacterCalendar],
      rail: Some(Destination::Calendar),
      scopes: &[scopes::CHARACTER_CALENDAR_READ, scopes::CHARACTER_CALENDAR_RESPOND],
      tab: None,
    },
    SubFeature::CloneMonitoring => SubDescriptor {
      jobs: &[JobKind::CharacterClones],
      rail: None,
      scopes: &[scopes::CHARACTER_CLONES],
      tab: Some(Tab::Clones),
    },
    SubFeature::Contacts => SubDescriptor {
      jobs: &[JobKind::CharacterContacts, JobKind::CorporationContacts],
      rail: None,
      scopes: &[scopes::CHARACTER_CONTACTS, scopes::CHARACTER_CONTACTS_WRITE],
      tab: Some(Tab::Contacts),
    },
    SubFeature::Contracts => SubDescriptor {
      jobs: &[JobKind::CharacterContracts, JobKind::CorporationContracts],
      rail: None,
      scopes: &[scopes::CHARACTER_CONTRACTS],
      tab: None,
    },
    SubFeature::Extractions => SubDescriptor {
      jobs: &[JobKind::CorporationMiningExtractions, JobKind::CorporationStructures],
      rail: None,
      scopes: &[scopes::CORPORATION_STRUCTURES],
      tab: None,
    },
    SubFeature::Inventory => SubDescriptor {
      jobs: &[JobKind::AssetSync],
      rail: Some(Destination::Assets),
      scopes: &[scopes::CHARACTER_ASSETS],
      tab: None,
    },
    SubFeature::JobMonitoring => SubDescriptor {
      jobs: &[JobKind::CharacterIndustryJobs, JobKind::CorporationIndustryJobs],
      rail: Some(Destination::Industry),
      scopes: &[scopes::CHARACTER_INDUSTRY_JOBS, scopes::CORPORATION_INDUSTRY_JOBS],
      tab: None,
    },
    SubFeature::Journal => SubDescriptor {
      jobs: &[JobKind::CharacterWallet, JobKind::CorporationWallet],
      rail: Some(Destination::Wallet),
      scopes: &[scopes::CHARACTER_WALLET],
      tab: None,
    },
    SubFeature::KillLog => SubDescriptor {
      jobs: &[
        JobKind::CharacterKillmails,
        JobKind::CorporationKillmails,
        JobKind::KillmailDetailBackfill,
        JobKind::KillmailReconcile,
      ],
      rail: None,
      scopes: &[scopes::CHARACTER_KILLMAILS],
      tab: Some(Tab::Killlog),
    },
    SubFeature::LocationTracking => SubDescriptor {
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
    SubFeature::Mail => SubDescriptor {
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
    SubFeature::Notifications => SubDescriptor {
      jobs: &[JobKind::CharacterNotifications],
      rail: None,
      scopes: &[scopes::CHARACTER_NOTIFICATIONS],
      tab: Some(Tab::Notifications),
    },
    SubFeature::Planner => SubDescriptor {
      jobs: &[],
      rail: None,
      scopes: &[scopes::CHARACTER_SEARCH, scopes::UNIVERSE_STRUCTURES],
      tab: None,
    },
    SubFeature::SkillQueue => SubDescriptor {
      jobs: &[JobKind::CharacterSkills],
      rail: Some(Destination::Skills),
      scopes: &[
        scopes::CHARACTER_SKILLS,
        scopes::CHARACTER_SKILLQUEUE,
        scopes::CHARACTER_IMPLANTS,
      ],
      tab: None,
    },
    SubFeature::Standings => SubDescriptor {
      jobs: &[JobKind::CharacterStandings, JobKind::CorporationStandings],
      rail: None,
      scopes: &[scopes::CHARACTER_STANDINGS],
      tab: Some(Tab::Standings),
    },
    SubFeature::Stockpiles | SubFeature::Tracker | SubFeature::Values => SubDescriptor {
      jobs: &[],
      rail: Some(Destination::Assets),
      scopes: &[scopes::CHARACTER_ASSETS],
      tab: None,
    },
    SubFeature::Transactions => SubDescriptor {
      jobs: &[JobKind::CharacterMarketOrders],
      rail: Some(Destination::Wallet),
      scopes: &[scopes::CHARACTER_WALLET],
      tab: None,
    },
    SubFeature::Wallets => SubDescriptor {
      jobs: &[JobKind::MarketPrices, JobKind::NetWorthSnapshot],
      rail: Some(Destination::Wallet),
      scopes: &[scopes::CHARACTER_WALLET],
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
  use std::collections::HashSet;

  use super::*;

  fn rolled_up_jobs(feature: Feature) -> HashSet<JobKind> {
    feature
      .sub_features()
      .iter()
      .flat_map(|&sub| sub_descriptor(sub).jobs.iter().copied())
      .collect()
  }

  fn rolled_up_scopes(feature: Feature) -> HashSet<&'static str> {
    feature
      .sub_features()
      .iter()
      .flat_map(|&sub| sub_descriptor(sub).scopes.iter().copied())
      .collect()
  }

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

    #[test]
    fn the_industry_feature_requests_the_facility_search_scopes() {
      let scopes = descriptor(Feature::Industry).scopes;

      assert!(
        scopes.contains(&scopes::CHARACTER_SEARCH),
        "Industry must request CHARACTER_SEARCH for live facility search"
      );
      assert!(
        scopes.contains(&scopes::UNIVERSE_STRUCTURES),
        "Industry must request UNIVERSE_STRUCTURES to resolve structure hits"
      );
    }
  }

  mod feature_for_destination {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_leaves_always_on_destinations_unmapped() {
      assert_eq!(feature_for_destination(Destination::Characters), None);
      assert_eq!(feature_for_destination(Destination::Settings), None);
    }

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

  mod sub_descriptor {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn its_jobs_roll_up_to_exactly_the_group_descriptor() {
      for feature in Feature::ALL {
        let group: HashSet<JobKind> = descriptor(feature).jobs.iter().copied().collect();

        assert_eq!(
          rolled_up_jobs(feature),
          group,
          "{feature:?} sub-feature jobs must union to the group descriptor"
        );
      }
    }

    #[test]
    fn its_scopes_roll_up_to_exactly_the_group_descriptor() {
      for feature in Feature::ALL {
        let group: HashSet<&str> = descriptor(feature).scopes.iter().copied().collect();

        assert_eq!(
          rolled_up_scopes(feature),
          group,
          "{feature:?} sub-feature scopes must union to the group descriptor"
        );
      }
    }

    #[test]
    fn its_rail_rolls_up_to_the_group_descriptor() {
      for feature in Feature::ALL {
        let rolled = feature.sub_features().iter().find_map(|&sub| sub_descriptor(sub).rail);

        assert_eq!(
          rolled,
          descriptor(feature).rail,
          "{feature:?} rail must match the group"
        );
      }
    }

    #[test]
    fn its_tab_rolls_up_to_the_group_descriptor() {
      for feature in Feature::ALL {
        let rolled = feature.sub_features().iter().find_map(|&sub| sub_descriptor(sub).tab);

        assert_eq!(rolled, descriptor(feature).tab, "{feature:?} tab must match the group");
      }
    }

    #[test]
    fn no_job_is_claimed_by_two_sub_features() {
      let mut seen: HashSet<JobKind> = HashSet::new();

      for sub in SubFeature::ALL {
        for &job in sub_descriptor(sub).jobs {
          assert!(seen.insert(job), "{job:?} is claimed by more than one sub-feature");
        }
      }
    }
  }
}
