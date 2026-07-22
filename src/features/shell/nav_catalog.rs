use crate::{
  config::{Feature, FeatureFlags, SubFeature},
  features::shell::registry,
  ui::components::rail::Destination,
};

static ABYSSALS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/abyssals.svg");
static ARCHIVE_ICON: &[u8] = include_bytes!("../../../assets/images/icons/archive.svg");
static BUDGET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/budget.svg");
static CALENDAR_ICON: &[u8] = include_bytes!("../../../assets/images/icons/calendar.svg");
static CAPTAINS_LOG_ICON: &[u8] = include_bytes!("../../../assets/images/icons/captains-log.svg");
static CHARACTERS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/roster.svg");
static CLOCK_ICON: &[u8] = include_bytes!("../../../assets/images/icons/clock.svg");
static CONTACT_SYNC_ICON: &[u8] = include_bytes!("../../../assets/images/icons/contact-sync.svg");
static COMPARE_ICON: &[u8] = include_bytes!("../../../assets/images/icons/compare.svg");
static CONTRACTS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/contracts.svg");
static CORP_ICON: &[u8] = include_bytes!("../../../assets/images/icons/corp.svg");
static DOC_ICON: &[u8] = include_bytes!("../../../assets/images/icons/doc.svg");
static FACILITIES_ICON: &[u8] = include_bytes!("../../../assets/images/icons/facilities.svg");
static FLASK_ICON: &[u8] = include_bytes!("../../../assets/images/icons/flask.svg");
static INDUSTRY_ICON: &[u8] = include_bytes!("../../../assets/images/icons/industry.svg");
static INVENTORY_ICON: &[u8] = include_bytes!("../../../assets/images/icons/inventory.svg");
static JOURNAL_ICON: &[u8] = include_bytes!("../../../assets/images/icons/journal.svg");
static LAYOUT_ICON: &[u8] = include_bytes!("../../../assets/images/icons/layout.svg");
static LINK_ICON: &[u8] = include_bytes!("../../../assets/images/icons/link.svg");
static MARKET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/market.svg");
static MARKET_TREE_ICON: &[u8] = include_bytes!("../../../assets/images/icons/market-tree.svg");
static MOON_ICON: &[u8] = include_bytes!("../../../assets/images/icons/moon.svg");
static PLANET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/planet.svg");
static PULSE_ICON: &[u8] = include_bytes!("../../../assets/images/icons/pulse.svg");
static SETTINGS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/settings.svg");
static SKILLS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/skills.svg");
static STAR_ICON: &[u8] = include_bytes!("../../../assets/images/icons/star.svg");
static STOCKPILES_ICON: &[u8] = include_bytes!("../../../assets/images/icons/stockpiles.svg");
static TRACKER_ICON: &[u8] = include_bytes!("../../../assets/images/icons/tracker.svg");
static USERS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/users.svg");
static VALUES_ICON: &[u8] = include_bytes!("../../../assets/images/icons/values.svg");
static WALLET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/wallet.svg");

static SECTIONS: &[Section] = &[
  Section {
    destination: Destination::Roster,
    label_override: Some("nav.roster.label"),
    kicker: "nav.roster.kicker",
    sub_sections: &[
      SubSection {
        icon: CHARACTERS_ICON,
        id: "characters",
        label: "nav.roster.characters",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: CORP_ICON,
        id: "corporations",
        label: "nav.roster.corporations",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: CAPTAINS_LOG_ICON,
        id: "captains-log",
        label: "nav.roster.captains_log",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: FACILITIES_ICON,
        id: "structures",
        label: "nav.roster.structures",
        route: None,
        sub_feature: Some(SubFeature::StructureAlerts),
      },
      SubSection {
        icon: CONTACT_SYNC_ICON,
        id: "contact-sync",
        label: "nav.roster.contact_sync",
        route: None,
        sub_feature: Some(SubFeature::Contacts),
      },
    ],
  },
  Section {
    destination: Destination::Skills,
    label_override: None,
    kicker: "nav.skills.kicker",
    sub_sections: &[
      SubSection {
        icon: SKILLS_ICON,
        id: "queue",
        label: "nav.skills.queue",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: COMPARE_ICON,
        id: "compare",
        label: "nav.skills.compare",
        route: None,
        sub_feature: None,
      },
    ],
  },
  Section {
    destination: Destination::Industry,
    label_override: None,
    kicker: "nav.industry.kicker",
    sub_sections: &[
      SubSection {
        icon: INDUSTRY_ICON,
        id: "jobs",
        label: "nav.industry.jobs",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: DOC_ICON,
        id: "blueprints",
        label: "nav.industry.blueprints",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: FLASK_ICON,
        id: "planner",
        label: "nav.industry.planner",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: PLANET_ICON,
        id: "colonies",
        label: "nav.industry.colonies",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: MOON_ICON,
        id: "extractions",
        label: "nav.industry.extractions",
        route: None,
        sub_feature: None,
      },
    ],
  },
  Section {
    destination: Destination::Mail,
    label_override: None,
    kicker: "nav.mail.kicker",
    sub_sections: &[],
  },
  Section {
    destination: Destination::Calendar,
    label_override: None,
    kicker: "nav.calendar.kicker",
    sub_sections: &[
      SubSection {
        icon: JOURNAL_ICON,
        id: "agenda",
        label: "nav.calendar.agenda",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: CLOCK_ICON,
        id: "day",
        label: "nav.calendar.day",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: CALENDAR_ICON,
        id: "week",
        label: "nav.calendar.week",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: INVENTORY_ICON,
        id: "month",
        label: "nav.calendar.month",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: TRACKER_ICON,
        id: "year",
        label: "nav.calendar.year",
        route: None,
        sub_feature: None,
      },
    ],
  },
  Section {
    destination: Destination::Wallet,
    label_override: None,
    kicker: "nav.wallet.kicker",
    sub_sections: &[
      SubSection {
        icon: WALLET_ICON,
        id: "wallets",
        label: "nav.wallet.wallets",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: JOURNAL_ICON,
        id: "journal",
        label: "nav.wallet.journal",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: MARKET_ICON,
        id: "market",
        label: "nav.wallet.market",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: CONTRACTS_ICON,
        id: "contracts",
        label: "nav.wallet.contracts",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: BUDGET_ICON,
        id: "budget",
        label: "nav.wallet.budget",
        route: None,
        sub_feature: None,
      },
    ],
  },
  Section {
    destination: Destination::Market,
    label_override: None,
    kicker: "nav.market.kicker",
    sub_sections: &[
      SubSection {
        icon: MARKET_TREE_ICON,
        id: "browse",
        label: "nav.market.browse",
        route: None,
        sub_feature: Some(SubFeature::MarketBrowse),
      },
      SubSection {
        icon: CONTRACTS_ICON,
        id: "orders",
        label: "nav.market.orders",
        route: None,
        sub_feature: Some(SubFeature::MarketOrders),
      },
      SubSection {
        icon: COMPARE_ICON,
        id: "compare",
        label: "nav.market.compare",
        route: None,
        sub_feature: Some(SubFeature::MarketCompare),
      },
      SubSection {
        icon: STAR_ICON,
        id: "watchlist",
        label: "nav.market.watchlist",
        route: None,
        sub_feature: Some(SubFeature::MarketWatchlist),
      },
    ],
  },
  Section {
    destination: Destination::Assets,
    label_override: None,
    kicker: "nav.assets.kicker",
    sub_sections: &[
      SubSection {
        icon: INVENTORY_ICON,
        id: "inventory",
        label: "nav.assets.inventory",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: ABYSSALS_ICON,
        id: "abyssals",
        label: "nav.assets.abyssals",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: STOCKPILES_ICON,
        id: "stockpiles",
        label: "nav.assets.stockpiles",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: VALUES_ICON,
        id: "values",
        label: "nav.assets.values",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: TRACKER_ICON,
        id: "tracker",
        label: "nav.assets.tracker",
        route: None,
        sub_feature: None,
      },
    ],
  },
  Section {
    destination: Destination::Settings,
    label_override: None,
    kicker: "nav.settings.kicker",
    // About is a real Settings tab but is deliberately excluded from the cascade catalog.
    sub_sections: &[
      SubSection {
        icon: USERS_ICON,
        id: "accessibility",
        label: "nav.settings.accessibility",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: CAPTAINS_LOG_ICON,
        id: "captains-log",
        label: "nav.settings.captains_log",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: FACILITIES_ICON,
        id: "facilities",
        label: "nav.settings.facility",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: SETTINGS_ICON,
        id: "features",
        label: "nav.settings.features",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: LINK_ICON,
        id: "mcp",
        label: "nav.settings.mcp",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: ARCHIVE_ICON,
        id: "storage",
        label: "nav.settings.storage",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: STAR_ICON,
        id: "tags",
        label: "nav.settings.tags",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: PULSE_ICON,
        id: "telemetry",
        label: "nav.settings.telemetry",
        route: None,
        sub_feature: None,
      },
      SubSection {
        icon: LAYOUT_ICON,
        id: "ui",
        label: "nav.settings.ui",
        route: None,
        sub_feature: None,
      },
    ],
  },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Section {
  pub destination: Destination,
  label_override: Option<&'static str>,
  kicker: &'static str,
  pub sub_sections: &'static [SubSection],
}

impl Section {
  pub fn icon(&self) -> &'static [u8] {
    self.destination.icon()
  }

  pub fn is_enabled(&self, enabled_features: &[Feature]) -> bool {
    registry::feature_for_destination(self.destination).is_none_or(|feature| enabled_features.contains(&feature))
  }

  pub fn kicker(&self) -> String {
    t!(self.kicker).into_owned()
  }

  pub fn label(&self) -> String {
    self
      .label_override
      .map_or_else(|| self.destination.label(), |key| t!(key).into_owned())
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubSection {
  pub icon: &'static [u8],
  pub id: &'static str,
  label: &'static str,
  pub route: Option<Destination>,
  pub sub_feature: Option<SubFeature>,
}

impl SubSection {
  pub fn is_enabled(&self, flags: &FeatureFlags, structures_available: bool) -> bool {
    match self.sub_feature {
      Some(SubFeature::StructureAlerts) => structures_available && flags.is_sub_enabled(SubFeature::StructureAlerts),
      Some(sub) => flags.is_sub_enabled(sub),
      None => true,
    }
  }

  pub fn label(&self) -> String {
    t!(self.label).into_owned()
  }
}

pub fn section(destination: Destination) -> Option<&'static Section> {
  SECTIONS.iter().find(|section| section.destination == destination)
}

pub fn visible_sections(enabled_features: &[Feature]) -> impl Iterator<Item = &'static Section> {
  SECTIONS
    .iter()
    .filter(move |section| section.is_enabled(enabled_features))
}

#[cfg(test)]
mod tests {
  use super::*;

  mod section {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_section_for_every_destination() {
      for destination in [
        Destination::Assets,
        Destination::Calendar,
        Destination::Roster,
        Destination::Industry,
        Destination::Mail,
        Destination::Market,
        Destination::Settings,
        Destination::Skills,
        Destination::Wallet,
      ] {
        assert!(
          section(destination).is_some(),
          "{destination:?} must have a catalog section"
        );
      }
    }

    #[test]
    fn it_borrows_label_and_icon_from_the_destination() {
      let section = section(Destination::Wallet).expect("wallet section");

      assert_eq!(section.label(), Destination::Wallet.label());
      assert_eq!(section.icon(), Destination::Wallet.icon());
    }

    #[test]
    fn it_overrides_the_characters_cascade_label_to_roster() {
      let section = section(Destination::Roster).expect("characters section");

      assert_eq!(section.label(), "Roster");
      assert_eq!(section.icon(), Destination::Roster.icon());
    }
  }

  mod sub_section {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn no_sub_section_carries_a_route_today() {
      for section in SECTIONS {
        for sub in section.sub_sections {
          assert_eq!(
            sub.route,
            None,
            "{}/{} must not carry a route yet",
            section.label(),
            sub.id
          );
        }
      }
    }

    #[test]
    fn contact_sync_is_gated_on_the_contacts_sub_feature() {
      let sub = section(Destination::Roster)
        .expect("roster section")
        .sub_sections
        .iter()
        .find(|sub| sub.id == "contact-sync")
        .expect("contact-sync sub-section");

      let mut flags = FeatureFlags::default();
      assert!(sub.is_enabled(&flags, true));

      flags.set_sub_enabled(SubFeature::Contacts, false);
      assert!(!sub.is_enabled(&flags, true));
    }

    #[test]
    fn structures_is_gated_on_both_the_feature_and_accessible_structures() {
      let sub = section(Destination::Roster)
        .expect("roster section")
        .sub_sections
        .iter()
        .find(|sub| sub.id == "structures")
        .expect("structures sub-section");

      let flags = FeatureFlags::default();
      assert!(sub.is_enabled(&flags, true));
      assert!(!sub.is_enabled(&flags, false), "hidden without an accessible structure");

      let mut disabled = FeatureFlags::default();
      disabled.set_sub_enabled(SubFeature::StructureAlerts, false);
      assert!(!sub.is_enabled(&disabled, true), "hidden when the feature is off");
    }

    #[test]
    fn ungated_sub_sections_stay_visible_with_everything_disabled() {
      let mut flags = FeatureFlags::default();
      for sub in SubFeature::ALL {
        flags.set_sub_enabled(sub, false);
      }

      for sub in section(Destination::Roster).expect("roster section").sub_sections {
        if sub.sub_feature.is_none() {
          assert!(sub.is_enabled(&flags, false), "{} must not be gated", sub.id);
        }
      }
    }

    #[test]
    fn sub_section_ids_are_unique_within_a_section() {
      for section in SECTIONS {
        let mut ids: Vec<&str> = section.sub_sections.iter().map(|sub| sub.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(count, ids.len(), "{} has duplicate sub-section ids", section.label());
      }
    }
  }

  mod visible_sections {
    use super::*;

    #[test]
    fn it_keeps_always_on_destinations_when_no_feature_is_enabled() {
      let visible: Vec<Destination> = visible_sections(&[]).map(|section| section.destination).collect();

      assert!(visible.contains(&Destination::Roster));
      assert!(visible.contains(&Destination::Settings));
    }

    #[test]
    fn it_hides_a_section_whose_feature_is_disabled() {
      let visible: Vec<Destination> = visible_sections(&[]).map(|section| section.destination).collect();

      assert!(
        !visible.contains(&Destination::Wallet),
        "Wallet is gated behind its feature"
      );
      assert!(
        !visible.contains(&Destination::Assets),
        "Assets is gated behind its feature"
      );
    }

    #[test]
    fn it_reveals_a_section_when_its_feature_is_enabled() {
      let visible: Vec<Destination> = visible_sections(&[Feature::Wallet])
        .map(|section| section.destination)
        .collect();

      assert!(visible.contains(&Destination::Wallet));
      assert!(
        !visible.contains(&Destination::Mail),
        "an unrelated gated section stays hidden"
      );
    }

    #[test]
    fn it_reuses_the_registry_feature_gate() {
      for destination in [
        Destination::Assets,
        Destination::Calendar,
        Destination::Mail,
        Destination::Wallet,
      ] {
        let feature = registry::feature_for_destination(destination).expect("a feature-backed destination");
        let visible: Vec<Destination> = visible_sections(&[feature])
          .map(|section| section.destination)
          .collect();

        assert!(
          visible.contains(&destination),
          "{destination:?} appears once its feature is on"
        );
      }
    }
  }

  mod tab_enum_parity {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::{
      assets, calendar, industry, market, roster,
      settings::{self},
      wallet,
    };

    fn ids(destination: Destination) -> Vec<&'static str> {
      section(destination)
        .expect("section")
        .sub_sections
        .iter()
        .map(|sub| sub.id)
        .collect()
    }

    #[test]
    fn assets_tabs_match_one_to_one() {
      fn catalog_id(tab: assets::Tab) -> &'static str {
        match tab {
          assets::Tab::Abyssals => "abyssals",
          assets::Tab::Inventory => "inventory",
          assets::Tab::Stockpiles => "stockpiles",
          assets::Tab::Tracker => "tracker",
          assets::Tab::Values => "values",
        }
      }

      let order = [
        assets::Tab::Inventory,
        assets::Tab::Abyssals,
        assets::Tab::Stockpiles,
        assets::Tab::Values,
        assets::Tab::Tracker,
      ];
      let expected: Vec<&str> = order.into_iter().map(catalog_id).collect();

      assert_eq!(ids(Destination::Assets), expected);
    }

    #[test]
    fn calendar_views_match_one_to_one() {
      fn catalog_id(view: calendar::View) -> &'static str {
        match view {
          calendar::View::Agenda => "agenda",
          calendar::View::Day => "day",
          calendar::View::Month => "month",
          calendar::View::Week => "week",
          calendar::View::Year => "year",
        }
      }

      let expected: Vec<&str> = calendar::View::ALL.into_iter().map(catalog_id).collect();

      assert_eq!(ids(Destination::Calendar), expected);
    }

    #[test]
    fn characters_panes_match_one_to_one() {
      fn catalog_id(pane: roster::Pane) -> &'static str {
        match pane {
          roster::Pane::Characters => "characters",
          roster::Pane::Corporations => "corporations",
        }
      }

      let order = [roster::Pane::Characters, roster::Pane::Corporations];
      let mut expected: Vec<&str> = order.into_iter().map(catalog_id).collect();
      expected.push("captains-log");
      expected.push("structures");
      expected.push("contact-sync");

      assert_eq!(ids(Destination::Roster), expected);
    }

    #[test]
    fn industry_tabs_match_one_to_one() {
      fn catalog_id(tab: industry::Tab) -> &'static str {
        match tab {
          industry::Tab::Blueprints => "blueprints",
          industry::Tab::Colonies => "colonies",
          industry::Tab::Extractions => "extractions",
          industry::Tab::Jobs => "jobs",
          industry::Tab::Planner => "planner",
        }
      }

      let expected: Vec<&str> = industry::Tab::ALL.into_iter().map(catalog_id).collect();

      assert_eq!(ids(Destination::Industry), expected);
    }

    #[test]
    fn settings_categories_match_one_to_one() {
      fn catalog_id(category: settings::Category) -> Option<&'static str> {
        match category {
          settings::Category::About => None,
          settings::Category::Accessibility => Some("accessibility"),
          settings::Category::CaptainsLog => Some("captains-log"),
          settings::Category::Facility => Some("facilities"),
          settings::Category::Features => Some("features"),
          settings::Category::Mcp => Some("mcp"),
          settings::Category::Storage => Some("storage"),
          settings::Category::Tags => Some("tags"),
          settings::Category::Telemetry => Some("telemetry"),
          settings::Category::Ui => Some("ui"),
        }
      }

      let order = [
        settings::Category::Accessibility,
        settings::Category::CaptainsLog,
        settings::Category::Facility,
        settings::Category::Features,
        settings::Category::Mcp,
        settings::Category::Storage,
        settings::Category::Tags,
        settings::Category::Telemetry,
        settings::Category::Ui,
        settings::Category::About,
      ];
      let expected: Vec<&str> = order.into_iter().filter_map(catalog_id).collect();

      assert_eq!(ids(Destination::Settings), expected);
    }

    #[test]
    fn about_is_excluded_from_the_settings_cascade() {
      assert!(
        !ids(Destination::Settings).contains(&"about"),
        "About is a real Settings tab but must not appear in the cascade catalog"
      );
    }

    #[test]
    fn market_tabs_match_one_to_one() {
      fn catalog_id(tab: market::Tab) -> &'static str {
        match tab {
          market::Tab::Browse => "browse",
          market::Tab::Compare => "compare",
          market::Tab::Orders => "orders",
          market::Tab::Watchlist => "watchlist",
        }
      }

      let expected: Vec<&str> = market::Tab::ORDER.into_iter().map(catalog_id).collect();

      assert_eq!(ids(Destination::Market), expected);
    }

    #[test]
    fn wallet_tabs_match_one_to_one() {
      fn catalog_id(tab: wallet::Tab) -> &'static str {
        match tab {
          wallet::Tab::Budget => "budget",
          wallet::Tab::Contracts => "contracts",
          wallet::Tab::Journal => "journal",
          wallet::Tab::Market => "market",
          wallet::Tab::Wallets => "wallets",
        }
      }

      let order = [
        wallet::Tab::Wallets,
        wallet::Tab::Journal,
        wallet::Tab::Market,
        wallet::Tab::Contracts,
        wallet::Tab::Budget,
      ];
      let expected: Vec<&str> = order.into_iter().map(catalog_id).collect();

      assert_eq!(ids(Destination::Wallet), expected);
    }
  }
}
