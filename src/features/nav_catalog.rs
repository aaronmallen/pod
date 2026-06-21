use crate::{config::Feature, features::registry, ui::components::rail::Destination};

static ABYSSALS_ICON: &[u8] = include_bytes!("../../assets/images/icons/abyssals.svg");
static ARCHIVE_ICON: &[u8] = include_bytes!("../../assets/images/icons/archive.svg");
static BUDGET_ICON: &[u8] = include_bytes!("../../assets/images/icons/budget.svg");
static CALENDAR_ICON: &[u8] = include_bytes!("../../assets/images/icons/calendar.svg");
static CHARACTERS_ICON: &[u8] = include_bytes!("../../assets/images/icons/characters.svg");
static CLOCK_ICON: &[u8] = include_bytes!("../../assets/images/icons/clock.svg");
static COMPARE_ICON: &[u8] = include_bytes!("../../assets/images/icons/compare.svg");
static CONTRACTS_ICON: &[u8] = include_bytes!("../../assets/images/icons/contracts.svg");
static CORP_ICON: &[u8] = include_bytes!("../../assets/images/icons/corp.svg");
static DOC_ICON: &[u8] = include_bytes!("../../assets/images/icons/doc.svg");
static FLASK_ICON: &[u8] = include_bytes!("../../assets/images/icons/flask.svg");
static INDUSTRY_ICON: &[u8] = include_bytes!("../../assets/images/icons/industry.svg");
static INVENTORY_ICON: &[u8] = include_bytes!("../../assets/images/icons/inventory.svg");
static JOURNAL_ICON: &[u8] = include_bytes!("../../assets/images/icons/journal.svg");
static LAYOUT_ICON: &[u8] = include_bytes!("../../assets/images/icons/layout.svg");
static MARKET_ICON: &[u8] = include_bytes!("../../assets/images/icons/market.svg");
static MOON_ICON: &[u8] = include_bytes!("../../assets/images/icons/moon.svg");
static SETTINGS_ICON: &[u8] = include_bytes!("../../assets/images/icons/settings.svg");
static SKILLS_ICON: &[u8] = include_bytes!("../../assets/images/icons/skills.svg");
static STAR_ICON: &[u8] = include_bytes!("../../assets/images/icons/star.svg");
static STOCKPILES_ICON: &[u8] = include_bytes!("../../assets/images/icons/stockpiles.svg");
static TRACKER_ICON: &[u8] = include_bytes!("../../assets/images/icons/tracker.svg");
static USERS_ICON: &[u8] = include_bytes!("../../assets/images/icons/users.svg");
static VALUES_ICON: &[u8] = include_bytes!("../../assets/images/icons/values.svg");
static WALLET_ICON: &[u8] = include_bytes!("../../assets/images/icons/wallet.svg");

static SECTIONS: &[Section] = &[
  Section {
    destination: Destination::Characters,
    // The rail icon keeps the Characters identity, but the cascade header reads "Roster".
    label_override: Some("Roster"),
    kicker: "Pilots & corporations",
    sub_sections: &[
      SubSection {
        icon: CHARACTERS_ICON,
        id: "characters",
        label: "Characters",
        route: None,
      },
      SubSection {
        icon: CORP_ICON,
        id: "corporations",
        label: "Corporations",
        route: None,
      },
    ],
  },
  Section {
    destination: Destination::Skills,
    label_override: None,
    kicker: "Training & planning",
    sub_sections: &[
      SubSection {
        icon: SKILLS_ICON,
        id: "queue",
        label: "Queue",
        route: None,
      },
      SubSection {
        icon: COMPARE_ICON,
        id: "compare",
        label: "Compare",
        route: None,
      },
    ],
  },
  Section {
    destination: Destination::Industry,
    label_override: None,
    kicker: "Manufacturing & planning",
    sub_sections: &[
      SubSection {
        icon: INDUSTRY_ICON,
        id: "jobs",
        label: "Jobs",
        route: None,
      },
      SubSection {
        icon: DOC_ICON,
        id: "blueprints",
        label: "Blueprints",
        route: None,
      },
      SubSection {
        icon: FLASK_ICON,
        id: "planner",
        label: "Planner",
        route: None,
      },
      SubSection {
        icon: MOON_ICON,
        id: "extractions",
        label: "Extractions",
        route: None,
      },
    ],
  },
  Section {
    destination: Destination::Mail,
    label_override: None,
    kicker: "Correspondence",
    sub_sections: &[],
  },
  Section {
    destination: Destination::Calendar,
    label_override: None,
    kicker: "In-game schedule",
    sub_sections: &[
      SubSection {
        icon: JOURNAL_ICON,
        id: "agenda",
        label: "Agenda",
        route: None,
      },
      SubSection {
        icon: CLOCK_ICON,
        id: "day",
        label: "Day",
        route: None,
      },
      SubSection {
        icon: CALENDAR_ICON,
        id: "week",
        label: "Week",
        route: None,
      },
      SubSection {
        icon: INVENTORY_ICON,
        id: "month",
        label: "Month",
        route: None,
      },
      SubSection {
        icon: TRACKER_ICON,
        id: "year",
        label: "Year",
        route: None,
      },
    ],
  },
  Section {
    destination: Destination::Wallet,
    label_override: None,
    kicker: "Ledger & budget",
    sub_sections: &[
      SubSection {
        icon: WALLET_ICON,
        id: "wallets",
        label: "Wallets",
        route: None,
      },
      SubSection {
        icon: JOURNAL_ICON,
        id: "journal",
        label: "Journal",
        route: None,
      },
      SubSection {
        icon: MARKET_ICON,
        id: "market",
        label: "Transactions",
        route: None,
      },
      SubSection {
        icon: CONTRACTS_ICON,
        id: "contracts",
        label: "Contracts",
        route: None,
      },
      SubSection {
        icon: BUDGET_ICON,
        id: "budget",
        label: "Budget",
        route: None,
      },
    ],
  },
  Section {
    destination: Destination::Assets,
    label_override: None,
    kicker: "Holdings across space",
    sub_sections: &[
      SubSection {
        icon: INVENTORY_ICON,
        id: "inventory",
        label: "Inventory",
        route: None,
      },
      SubSection {
        icon: ABYSSALS_ICON,
        id: "abyssals",
        label: "Abyssals",
        route: None,
      },
      SubSection {
        icon: STOCKPILES_ICON,
        id: "stockpiles",
        label: "Stockpiles",
        route: None,
      },
      SubSection {
        icon: VALUES_ICON,
        id: "values",
        label: "Values",
        route: None,
      },
      SubSection {
        icon: TRACKER_ICON,
        id: "tracker",
        label: "Tracker",
        route: None,
      },
    ],
  },
  Section {
    destination: Destination::Settings,
    label_override: None,
    kicker: "Preferences",
    // About is a real Settings tab but is deliberately excluded from the cascade catalog.
    sub_sections: &[
      SubSection {
        icon: USERS_ICON,
        id: "accessibility",
        label: "Accessibility",
        route: None,
      },
      SubSection {
        icon: SETTINGS_ICON,
        id: "features",
        label: "Features",
        route: None,
      },
      SubSection {
        icon: INDUSTRY_ICON,
        id: "industry",
        label: "Industry",
        route: None,
      },
      SubSection {
        icon: ARCHIVE_ICON,
        id: "storage",
        label: "Storage",
        route: None,
      },
      SubSection {
        icon: STAR_ICON,
        id: "tags",
        label: "Tags",
        route: None,
      },
      SubSection {
        icon: LAYOUT_ICON,
        id: "ui",
        label: "User Interface",
        route: None,
      },
    ],
  },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Section {
  pub destination: Destination,
  // Overrides the cascade header text without touching the rail icon's `Destination` identity.
  // `None` falls back to `Destination::label()`.
  pub label_override: Option<&'static str>,
  pub kicker: &'static str,
  pub sub_sections: &'static [SubSection],
}

impl Section {
  pub fn icon(&self) -> &'static [u8] {
    self.destination.icon()
  }

  pub fn is_enabled(&self, enabled_features: &[Feature]) -> bool {
    registry::feature_for_destination(self.destination).is_none_or(|feature| enabled_features.contains(&feature))
  }

  pub fn label(&self) -> &'static str {
    self.label_override.unwrap_or_else(|| self.destination.label())
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubSection {
  pub icon: &'static [u8],
  pub id: &'static str,
  pub label: &'static str,
  // A sub-section that is really its own top-level route rather than an in-view tab. The mechanism
  // exists for the deep-nav consumer to special-case; no entry uses it today.
  pub route: Option<Destination>,
}

pub fn section(destination: Destination) -> Option<&'static Section> {
  SECTIONS.iter().find(|section| section.destination == destination)
}

pub fn sections() -> &'static [Section] {
  SECTIONS
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
        Destination::Characters,
        Destination::Industry,
        Destination::Mail,
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
      let section = section(Destination::Characters).expect("characters section");

      // The cascade header reads "Roster" while the rail icon keeps the Characters identity.
      assert_eq!(section.label(), "Roster");
      assert_eq!(section.icon(), Destination::Characters.icon());
    }
  }

  mod sub_section {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn no_sub_section_carries_a_route_today() {
      // The route marker is a mechanism for the deep-nav consumer; no sub-section is its own
      // top-level route yet, so every entry must stay `None` until one genuinely is.
      for section in sections() {
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
    fn sub_section_ids_are_unique_within_a_section() {
      for section in sections() {
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

      assert!(visible.contains(&Destination::Characters));
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
      assets, calendar, character_manager, industry,
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

    // Each `catalog_id` match below is exhaustive over its enum, so adding or removing a variant
    // fails to compile until the catalog is updated in lock-step. The accompanying assertion proves
    // the catalog lists exactly those ids in order.

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
      fn catalog_id(pane: character_manager::Pane) -> &'static str {
        match pane {
          character_manager::Pane::Characters => "characters",
          character_manager::Pane::Corporations => "corporations",
        }
      }

      let order = [
        character_manager::Pane::Characters,
        character_manager::Pane::Corporations,
      ];
      let expected: Vec<&str> = order.into_iter().map(catalog_id).collect();

      assert_eq!(ids(Destination::Characters), expected);
    }

    #[test]
    fn industry_tabs_match_one_to_one() {
      fn catalog_id(tab: industry::Tab) -> &'static str {
        match tab {
          industry::Tab::Blueprints => "blueprints",
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
      // About is a genuine Settings tab but is deliberately excluded from the cascade catalog, so
      // the catalog ids are the `Category` variants MINUS About. The exhaustive match still forces a
      // catalog review whenever a variant is added or removed.
      fn catalog_id(category: settings::Category) -> Option<&'static str> {
        match category {
          settings::Category::About => None,
          settings::Category::Accessibility => Some("accessibility"),
          settings::Category::Features => Some("features"),
          settings::Category::Industry => Some("industry"),
          settings::Category::Storage => Some("storage"),
          settings::Category::Tags => Some("tags"),
          settings::Category::Ui => Some("ui"),
        }
      }

      let order = [
        settings::Category::Accessibility,
        settings::Category::Features,
        settings::Category::Industry,
        settings::Category::Storage,
        settings::Category::Tags,
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
