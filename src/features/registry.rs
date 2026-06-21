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

/// The set of sub-features that depend on `job`: a shared sync job (e.g. `AssetSync` feeds every
/// asset reader, `CharacterWallet` feeds every wallet reader) is owned by several sub-features, and
/// the engine keeps it scheduled while ANY owner is enabled — dropping it only when the last owner is
/// off. Distinct from [`SubDescriptor::jobs`], which partitions jobs one-to-one for the group
/// roll-up; ownership for scheduling follows real data dependencies and so a job may be shared.
///
/// Returns empty for the feature-less maintenance jobs that always run (character/corp profiles).
pub fn sub_features_for_job(job: JobKind) -> Vec<SubFeature> {
  // Every asset reader needs the asset table the AssetSync job populates.
  const ASSET_READERS: &[SubFeature] = &[
    SubFeature::Inventory,
    SubFeature::Abyssals,
    SubFeature::Stockpiles,
    SubFeature::Values,
    SubFeature::Tracker,
  ];
  // Every wallet reader needs the wallet/journal the wallet jobs populate.
  const CHAR_WALLET_READERS: &[SubFeature] = &[SubFeature::Wallets, SubFeature::Journal, SubFeature::Transactions];
  const CORP_WALLET_READERS: &[SubFeature] = &[SubFeature::Wallets, SubFeature::Journal];
  // Valuation maintenance feeds both the asset Values surface and the wallet net-worth surfaces.
  const VALUATION_READERS: &[SubFeature] = &[
    SubFeature::Wallets,
    SubFeature::Journal,
    SubFeature::Transactions,
    SubFeature::Values,
    SubFeature::Tracker,
  ];

  let owners: &[SubFeature] = match job {
    JobKind::AssetSync => ASSET_READERS,
    JobKind::CharacterAbyssals | JobKind::CorporationAbyssals => &[SubFeature::Abyssals],
    JobKind::CharacterBlueprints | JobKind::CorporationBlueprints => &[SubFeature::Blueprints],
    JobKind::CharacterCalendar => &[SubFeature::Calendar],
    JobKind::CharacterClones => &[SubFeature::CloneMonitoring],
    JobKind::CharacterContacts | JobKind::CorporationContacts => &[SubFeature::Contacts],
    JobKind::CharacterContracts | JobKind::CorporationContracts => &[SubFeature::Contracts],
    JobKind::CharacterIndustryJobs | JobKind::CorporationIndustryJobs => &[SubFeature::JobMonitoring],
    JobKind::CharacterKillmails
    | JobKind::CorporationKillmails
    | JobKind::KillmailDetailBackfill
    | JobKind::KillmailReconcile => &[SubFeature::KillLog],
    JobKind::CharacterMail => &[SubFeature::Mail],
    JobKind::CharacterMarketOrders => &[SubFeature::Transactions],
    JobKind::CharacterNotifications => &[SubFeature::Notifications],
    JobKind::CharacterSkills => &[SubFeature::SkillQueue],
    JobKind::CharacterStandings | JobKind::CorporationStandings => &[SubFeature::Standings],
    JobKind::CharacterTelemetry => &[SubFeature::LocationTracking],
    JobKind::CharacterWallet => CHAR_WALLET_READERS,
    JobKind::CorporationWallet => CORP_WALLET_READERS,
    JobKind::CorporationMiningExtractions | JobKind::CorporationStructures => &[SubFeature::Extractions],
    JobKind::MarketPrices | JobKind::NetWorthSnapshot => VALUATION_READERS,
    // The maintenance jobs that always run belong to no sub-feature: the character/corp profiles, the
    // global industry cost-index refresh (a cheap public sync feeding the planner on demand), and the
    // global token audit.
    JobKind::CharacterProfile | JobKind::CorporationProfile | JobKind::IndustryCostIndices | JobKind::TokenAudit => &[],
  };
  owners.to_vec()
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
    fn the_descriptor_jobs_partition_one_to_one_for_the_group_roll_up() {
      // The SubDescriptor::jobs field exists only to roll the group descriptor up; it stays a
      // one-to-one partition. Shared ownership for scheduling lives in `sub_features_for_job`, which
      // the next module relaxes to a SET.
      let mut seen: HashSet<JobKind> = HashSet::new();

      for sub in SubFeature::ALL {
        for &job in sub_descriptor(sub).jobs {
          assert!(
            seen.insert(job),
            "{job:?} is claimed by more than one sub-feature descriptor"
          );
        }
      }
    }
  }

  mod sub_features_for_job {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::config::FeatureFlags;

    const FEATURELESS: [JobKind; 4] = [
      JobKind::CharacterProfile,
      JobKind::CorporationProfile,
      JobKind::IndustryCostIndices,
      JobKind::TokenAudit,
    ];

    /// Flags with exactly `sub` enabled and every other sub-feature off.
    fn only(sub: SubFeature) -> FeatureFlags {
      let mut flags = FeatureFlags::default();
      for candidate in SubFeature::ALL {
        flags.set_sub_enabled(candidate, candidate == sub);
      }
      flags
    }

    #[test]
    fn every_gated_job_is_owned_by_at_least_one_sub_feature() {
      for &job in JobKind::ALL {
        if FEATURELESS.contains(&job) {
          continue;
        }
        assert!(
          !sub_features_for_job(job).is_empty(),
          "{job:?} must be owned by at least one sub-feature so a config can gate it"
        );
      }
    }

    #[test]
    fn the_maintenance_jobs_belong_to_no_sub_feature() {
      for job in FEATURELESS {
        assert!(
          sub_features_for_job(job).is_empty(),
          "{job:?} must always run regardless of the enabled sub-features"
        );
      }
    }

    #[test]
    fn a_per_group_jobs_owners_all_live_in_that_group() {
      // Per-feature jobs (everything but the cross-group valuation maintenance) must have owners that
      // all roll up to the group its descriptor names, so the finer ownership never contradicts the
      // coarse `feature_for_job` mapping.
      const CROSS_GROUP: [JobKind; 2] = [JobKind::MarketPrices, JobKind::NetWorthSnapshot];

      for &job in JobKind::ALL {
        if CROSS_GROUP.contains(&job) {
          continue;
        }
        let Some(group) = feature_for_job(job) else {
          continue;
        };
        for owner in sub_features_for_job(job) {
          assert_eq!(
            owner.group(),
            group,
            "{job:?} owner {owner:?} must live in the job's group {group:?}"
          );
        }
      }
    }

    #[test]
    fn the_valuation_jobs_feed_both_assets_and_wallet() {
      // The valuation maintenance jobs are genuinely cross-group: they keep running while EITHER an
      // asset valuation surface or a wallet net-worth surface is enabled.
      for job in [JobKind::MarketPrices, JobKind::NetWorthSnapshot] {
        let groups: HashSet<Feature> = sub_features_for_job(job).into_iter().map(SubFeature::group).collect();
        assert!(
          groups.contains(&Feature::AssetTracking),
          "{job:?} feeds asset valuation"
        );
        assert!(groups.contains(&Feature::Wallet), "{job:?} feeds wallet net worth");
      }
    }

    #[test]
    fn the_asset_sync_job_is_shared_by_every_asset_reader() {
      let owners: HashSet<SubFeature> = sub_features_for_job(JobKind::AssetSync).into_iter().collect();

      let expected: HashSet<SubFeature> = Feature::AssetTracking.sub_features().iter().copied().collect();
      assert_eq!(
        owners, expected,
        "AssetSync feeds every asset sub-feature, so all of them must own it"
      );
    }

    #[test]
    fn a_shared_job_runs_until_its_last_owner_is_disabled() {
      // AssetSync survives as long as ANY asset reader is on, and stops only when all are off.
      let mut flags = only(SubFeature::Stockpiles);
      assert!(
        JobKind::AssetSync.is_feature_enabled(&flags),
        "one surviving asset sub-feature keeps the shared AssetSync job scheduled"
      );

      flags.set_sub_enabled(SubFeature::Stockpiles, false);
      assert!(
        !JobKind::AssetSync.is_feature_enabled(&flags),
        "with every asset sub-feature off, the shared AssetSync job stops"
      );
    }

    #[test]
    fn the_wallet_job_is_shared_across_the_wallet_readers() {
      let owners: HashSet<SubFeature> = sub_features_for_job(JobKind::CharacterWallet).into_iter().collect();

      assert!(owners.contains(&SubFeature::Wallets));
      assert!(owners.contains(&SubFeature::Journal));
      assert!(owners.contains(&SubFeature::Transactions));

      // Disabling Journal alone leaves the wallet job running for the other readers.
      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::Journal, false);
      flags.set_sub_enabled(SubFeature::Transactions, false);
      assert!(
        JobKind::CharacterWallet.is_feature_enabled(&flags),
        "the Wallets balances sub-feature still needs the wallet job"
      );
    }

    #[test]
    fn the_market_orders_job_is_owned_only_by_transactions() {
      assert_eq!(
        sub_features_for_job(JobKind::CharacterMarketOrders),
        vec![SubFeature::Transactions]
      );

      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::Transactions, false);
      assert!(
        !JobKind::CharacterMarketOrders.is_feature_enabled(&flags),
        "disabling Transactions stops the market-orders job even with other wallet readers on"
      );
    }

    #[test]
    fn the_industry_jobs_drop_independently_by_sub_feature() {
      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::Blueprints, false);
      assert!(
        !JobKind::CharacterBlueprints.is_feature_enabled(&flags),
        "disabling Blueprints stops the blueprints job"
      );
      assert!(
        JobKind::CharacterIndustryJobs.is_feature_enabled(&flags),
        "Job Monitoring's job survives when only Blueprints is off"
      );

      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::Extractions, false);
      assert!(
        !JobKind::CorporationMiningExtractions.is_feature_enabled(&flags),
        "disabling Extractions stops the mining-extractions job"
      );
    }

    #[test]
    fn every_sub_feature_has_a_scope_or_is_explicitly_scope_free() {
      // Budget derives everything from already-synced local data, so it requests no scope; every
      // other sub-feature (the asset Tracker still shares CHARACTER_ASSETS) must request at least one.
      const SCOPE_FREE: [SubFeature; 1] = [SubFeature::Budget];

      for sub in SubFeature::ALL {
        let has_scope = !sub_descriptor(sub).scopes.is_empty();
        let is_scope_free = SCOPE_FREE.contains(&sub);
        assert!(
          has_scope || is_scope_free,
          "{sub:?} must request a scope or be an explicit scope-free sub-feature"
        );
      }
    }
  }
}
