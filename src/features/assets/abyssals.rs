mod card;
mod card_grid;
mod filter_sidebar;
mod module_type_picker;
mod stat_ranges;
mod stat_row;
mod tier_badge;
mod type_icon_tile;

use std::collections::HashMap;

use iced::{Element, Length, Padding, widget::container};

use super::{Message, RosterPilot, Scope};
pub(super) use crate::store::{
  model::{StatRange, StatTemplate, abyssal_source_type_filter::SourceTypeFilter},
  repo::assets::AbyssalCursor,
};
use crate::{
  store::{
    Database, images,
    model::AbyssalItem,
    repo::{assets, character, sde},
  },
  ui::{components::empty_state::empty_state as shared_empty_state, style::spacing},
};

/// Number of cards fetched per cursor-paginated abyssal page.
pub(super) const PAGE_SIZE: i64 = 60;

const UNIT_SUFFIX_TABLE: &[(i64, &str)] = &[
  (71, " GJ"),
  (101, " m/s"),
  (105, " HP"),
  (108, " s"),
  (114, " kg"),
  (115, " tf"),
  (116, " MW"),
  (117, " km"),
  (121, " m\u{00b3}"),
  (124, "%"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct AbyssalCard {
  pub(super) character_id: i64,
  pub(super) estimate: Option<f64>,
  pub(super) group_type_id: i64,
  pub(super) item_id: i64,
  pub(super) location: String,
  pub(super) module_name: String,
  pub(super) owner_name: String,
  pub(super) portrait: images::ImageState,
  pub(super) price_unavailable: bool,
  pub(super) stats: Vec<AbyssalStat>,
  pub(super) tier_label: String,
}

impl AbyssalCard {
  /// The keyset cursor that resumes pagination strictly after this card.
  ///
  /// Mirrors the grid's group ordering: `(source_type_id, item_id)`.
  pub(super) fn cursor(&self) -> AbyssalCursor {
    AbyssalCursor {
      item_id: self.item_id,
      source_type_id: self.group_type_id,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AbyssalStat {
  pub(super) attribute_id: i64,
  pub(super) base_value: f64,
  pub(super) bound_hi: f64,
  pub(super) bound_lo: f64,
  pub(super) display_name: String,
  pub(super) high_is_good: bool,
  pub(super) rolled: f64,
  pub(super) unit_suffix: String,
}

impl AbyssalStat {
  #[allow(dead_code)]
  pub(super) fn delta_pct(&self) -> f64 {
    if self.base_value.abs() < f64::EPSILON {
      return 0.0;
    }
    (self.rolled - self.base_value) / self.base_value * 100.0
  }
}

#[derive(Clone, Debug, Default)]
pub struct AbyssalsData {
  pub(super) cards: Vec<AbyssalCard>,
  pub(super) source_types: Vec<SourceTypeFilter>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filters {
  pub(super) source_type_id: Option<i64>,
  pub(super) stat_ranges: HashMap<i64, (f64, f64)>,
}

impl Filters {
  pub(super) fn is_active(&self) -> bool {
    self.source_type_id.is_some() || !self.stat_ranges.is_empty()
  }

  pub(super) fn stat_ranges_for_query(&self) -> HashMap<i64, StatRange> {
    self
      .stat_ranges
      .iter()
      .map(|(attribute_id, (min, max))| {
        (
          *attribute_id,
          StatRange {
            max: *max,
            min: *min,
          },
        )
      })
      .collect()
  }
}

/// Load the initial abyssals payload: the first cursor page of (unfiltered) cards
/// plus the source-type filter facets.
pub(super) async fn load_cards(db: &Database, scope: Scope, roster: &[RosterPilot]) -> AbyssalsData {
  let character_ids = scope_character_ids(db, scope, roster).await;
  let cards = load_filtered_page(db, scope, roster, &Filters::default(), None).await;
  let source_types = assets::source_type_filters(db, &character_ids)
    .await
    .unwrap_or_default();
  AbyssalsData {
    cards,
    source_types,
  }
}

/// Load the first cursor-delimited page of filtered cards.
///
/// Replaces the old full-set fetch: only [`PAGE_SIZE`] cards are materialized up
/// front, with the rest loaded on scroll via [`load_filtered_page`].
pub(super) async fn load_filtered_cards(
  db: &Database,
  scope: Scope,
  roster: &[RosterPilot],
  filters: &Filters,
) -> Vec<AbyssalCard> {
  load_filtered_page(db, scope, roster, filters, None).await
}

/// Load one cursor-delimited page of filtered cards.
///
/// `cursor` is `None` for the first page, or the [`AbyssalCard::cursor`] of the
/// last card already shown to resume strictly after it.
pub(super) async fn load_filtered_page(
  db: &Database,
  scope: Scope,
  roster: &[RosterPilot],
  filters: &Filters,
  cursor: Option<AbyssalCursor>,
) -> Vec<AbyssalCard> {
  let character_ids = scope_character_ids(db, scope, roster).await;
  let items = assets::page_for_characters(
    db,
    &character_ids,
    filters.source_type_id,
    &filters.stat_ranges_for_query(),
    cursor,
    Some(PAGE_SIZE),
  )
  .await
  .unwrap_or_default();

  cards_from_items(db, roster, &items).await
}

/// Build the display cards for a batch of items, resolving each card's stats and
/// then back-filling locations in a single batched lookup.
async fn cards_from_items(db: &Database, roster: &[RosterPilot], items: &[AbyssalItem]) -> Vec<AbyssalCard> {
  let mut base_cache: HashMap<i64, BaseModule> = HashMap::new();
  let mut cards = Vec::with_capacity(items.len());
  for item in items {
    let owner_name = owner_name_for(roster, item.character_id());
    cards.push(build_card(db, item, &owner_name, &mut base_cache).await);
  }

  let item_ids: Vec<i64> = cards.iter().map(|card| card.item_id).collect();
  let locations = assets::locations_for_items(db, &item_ids).await.unwrap_or_default();
  for card in &mut cards {
    if let Some(location) = locations.get(&card.item_id) {
      card.location = location.clone();
    }
  }
  cards
}

pub(super) async fn load_stat_templates(
  db: &Database,
  scope: Scope,
  roster: &[RosterPilot],
  type_id: i64,
) -> Vec<StatTemplate> {
  let character_ids = scope_character_ids(db, scope, roster).await;
  assets::stat_templates_for_owned_type(db, &character_ids, type_id)
    .await
    .unwrap_or_default()
}

async fn scope_character_ids(db: &Database, scope: Scope, roster: &[RosterPilot]) -> Vec<i64> {
  match scope {
    Scope::Character(id) => vec![id],
    Scope::All => owned_character_ids(db, roster).await,
    Scope::Corporation(_) => Vec::new(),
  }
}

fn owner_name_for(roster: &[RosterPilot], character_id: i64) -> String {
  roster
    .iter()
    .find(|pilot| pilot.id == character_id)
    .map(|pilot| pilot.name.clone())
    .unwrap_or_else(|| format!("Character {character_id}"))
}

async fn owned_character_ids(db: &Database, roster: &[RosterPilot]) -> Vec<i64> {
  if !roster.is_empty() {
    return roster.iter().map(|pilot| pilot.id).collect();
  }
  character::all_owned(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|c| c.id())
    .collect()
}

struct BaseModule {
  dogma: HashMap<i64, f64>,
  name: String,
}

async fn build_card(
  db: &Database,
  item: &AbyssalItem,
  owner_name: &str,
  base_cache: &mut HashMap<i64, BaseModule>,
) -> AbyssalCard {
  let source_type_id = item.source_type_id();
  if let std::collections::hash_map::Entry::Vacant(e) = base_cache.entry(source_type_id) {
    e.insert(load_base_module(db, source_type_id).await);
  }
  let base = base_cache.get(&source_type_id).expect("just inserted");

  let rolled = parse_dogma(item.dogma_attributes());
  let bounds = assets::module_stats_for_type(db, item.type_id())
    .await
    .unwrap_or_default();
  let attribute_ids: Vec<i64> = bounds.iter().map(|bound| bound.attribute_id()).collect();
  let metadata: HashMap<i64, crate::store::model::DogmaAttribute> = sde::get_dogma_attributes(db, &attribute_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|attr| (attr.attribute_id(), attr))
    .collect();

  let mut stats = bounds
    .iter()
    .map(|bound| {
      let attribute_id = bound.attribute_id();
      let base_value = base.dogma.get(&attribute_id).copied().unwrap_or(0.0);
      let lo = base_value * bound.min_mult();
      let hi = base_value * bound.max_mult();
      let meta = metadata.get(&attribute_id);
      AbyssalStat {
        attribute_id,
        base_value,
        bound_hi: lo.max(hi),
        bound_lo: lo.min(hi),
        display_name: meta
          .and_then(|m| m.display_name().clone())
          .or_else(|| meta.map(|m| m.name().to_owned()))
          .unwrap_or_else(|| format!("Attr {attribute_id}")),
        high_is_good: meta
          .map(crate::store::model::DogmaAttribute::high_is_good)
          .unwrap_or(true),
        rolled: rolled.get(&attribute_id).copied().unwrap_or(0.0),
        unit_suffix: unit_suffix_for_id(meta.and_then(crate::store::model::DogmaAttribute::unit_id)).to_owned(),
      }
    })
    .collect::<Vec<AbyssalStat>>();
  stats.sort_by(|a, b| a.display_name.cmp(&b.display_name));

  AbyssalCard {
    character_id: item.character_id(),
    estimate: item.muta_price_isk(),
    group_type_id: source_type_id,
    item_id: item.item_id(),
    location: String::new(),
    module_name: base.name.clone(),
    owner_name: owner_name.to_owned(),
    portrait: images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      item.character_id(),
    ),
    price_unavailable: item.muta_price_synced().is_some() && item.muta_price_isk().is_none(),
    stats,
    tier_label: type_name_of(db, item.mutator_type_id()).await,
  }
}

async fn load_base_module(db: &Database, type_id: i64) -> BaseModule {
  match crate::store::repo::sde::get_item_type(db, type_id).await.ok().flatten() {
    Some(item_type) => BaseModule {
      dogma: parse_dogma(item_type.dogma_attributes()),
      name: item_type.name().to_owned(),
    },
    None => BaseModule {
      dogma: HashMap::new(),
      name: format!("Type {type_id}"),
    },
  }
}

fn parse_dogma(json: &str) -> HashMap<i64, f64> {
  #[derive(serde::Deserialize)]
  struct Entry {
    attribute_id: i64,
    value: f64,
  }
  serde_json::from_str::<Vec<Entry>>(json)
    .unwrap_or_default()
    .into_iter()
    .map(|entry| (entry.attribute_id, entry.value))
    .collect()
}

async fn type_name_of(db: &Database, type_id: i64) -> String {
  crate::store::repo::sde::get_item_type(db, type_id)
    .await
    .ok()
    .flatten()
    .map(|item_type| item_type.name().to_owned())
    .unwrap_or_else(|| format!("Type {type_id}"))
}

fn unit_suffix_for_id(unit_id: Option<i64>) -> &'static str {
  unit_id
    .and_then(|id| UNIT_SUFFIX_TABLE.iter().find(|&&(k, _)| k == id).map(|&(_, v)| v))
    .unwrap_or("")
}

pub(super) fn format_stat_value(value: f64, unit_suffix: &str) -> String {
  let formatted = format!("{value:.2}");
  let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
  format!("{trimmed}{unit_suffix}")
}

pub(super) fn group_by_type<'a>(cards: &[&'a AbyssalCard]) -> Vec<(String, Vec<&'a AbyssalCard>)> {
  let mut order: Vec<i64> = Vec::new();
  let mut groups: HashMap<i64, Vec<&'a AbyssalCard>> = HashMap::new();
  for card in cards {
    groups.entry(card.group_type_id).or_insert_with(|| {
      order.push(card.group_type_id);
      Vec::new()
    });
    groups.get_mut(&card.group_type_id).expect("just inserted").push(card);
  }
  order
    .into_iter()
    .map(|type_id| {
      let members = groups.remove(&type_id).unwrap_or_default();
      let label = members
        .first()
        .map(|c| c.module_name.clone())
        .unwrap_or_else(|| format!("Type {type_id}"));
      (label, members)
    })
    .collect()
}

pub(super) fn filter_rail(state: &super::State) -> Element<'_, Message> {
  filter_sidebar::rail(state)
}

pub(super) fn picker_modal(state: &super::State) -> Element<'_, Message> {
  module_type_picker::modal(state)
}

/// Render the abyssals card grid, windowed so only the viewport's rows-of-cards
/// are materialized.
///
/// `cards` is the full loaded set (cursor pagination appends to it); `scroll_offset`
/// is the live pixel offset tracked in feature state and fed to the windowing math.
pub(super) fn body<'a>(cards: Vec<&'a AbyssalCard>, any_owned: bool, scroll_offset: f32) -> Element<'a, Message> {
  if cards.is_empty() {
    return empty_state(any_owned);
  }

  container(card_grid::windowed_grid(cards, scroll_offset))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_6,
      right: super::HEADER_SIDE_PADDING,
      bottom: spacing::SPACE_6 + spacing::SPACE_2,
      left: super::HEADER_SIDE_PADDING,
    })
    .into()
}

fn empty_state<'a>(any_owned: bool) -> Element<'a, Message> {
  let label = if any_owned {
    "No abyssal modules match the active filters."
  } else {
    "No abyssal modules synced yet."
  };
  shared_empty_state(label).render()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn stat(attribute_id: i64, base: f64, rolled: f64, bounds: (f64, f64)) -> AbyssalStat {
    AbyssalStat {
      attribute_id,
      base_value: base,
      bound_hi: bounds.1,
      bound_lo: bounds.0,
      display_name: format!("Attr {attribute_id}"),
      high_is_good: true,
      rolled,
      unit_suffix: " tf".to_owned(),
    }
  }

  fn card(item_id: i64, module: &str, group_type_id: i64, tier: &str) -> AbyssalCard {
    AbyssalCard {
      character_id: 7,
      estimate: Some(1_000_000.0),
      group_type_id,
      item_id,
      location: "Jita IV - Moon 4".to_owned(),
      module_name: module.to_owned(),
      owner_name: "Vex".to_owned(),
      portrait: images::ImageState::Stale {
        id: 7,
        kind: images::ImageKind::CharacterPortrait,
      },
      price_unavailable: false,
      stats: vec![stat(50, 47.0, 41.0, (28.0, 56.0)), stat(51, 8.5, 7.1, (5.0, 12.0))],
      tier_label: tier.to_owned(),
    }
  }

  mod parse_dogma {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_the_attribute_value_pairs() {
      let map = parse_dogma(r#"[{"attribute_id": 50, "value": 41.0}, {"attribute_id": 51, "value": 7.1}]"#);
      assert_eq!(map.get(&50), Some(&41.0));
      assert_eq!(map.get(&51), Some(&7.1));
    }

    #[test]
    fn it_yields_an_empty_map_for_a_malformed_blob() {
      assert!(parse_dogma("not json").is_empty());
      assert!(parse_dogma("[]").is_empty());
    }
  }

  mod delta_pct {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_computes_the_signed_delta_from_base() {
      assert_eq!(stat(1, 100.0, 80.0, (60.0, 140.0)).delta_pct(), -20.0);
    }

    #[test]
    fn it_is_zero_when_base_is_zero() {
      assert_eq!(stat(1, 0.0, 5.0, (0.0, 0.0)).delta_pct(), 0.0);
    }
  }

  mod format_stat_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_the_unit_suffix_and_trims_trailing_zeros() {
      assert_eq!(format_stat_value(35.5, "%"), "35.5%");
      assert_eq!(format_stat_value(50_000.0, " kg"), "50000 kg");
      assert_eq!(format_stat_value(1_500.0, " HP"), "1500 HP");
      assert_eq!(format_stat_value(25.5, " tf"), "25.5 tf");
      assert_eq!(format_stat_value(4.75, " GJ"), "4.75 GJ");
    }
  }

  mod unit_suffix_for_id {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_known_unit_ids_to_their_suffix() {
      assert_eq!(unit_suffix_for_id(Some(71)), " GJ");
      assert_eq!(unit_suffix_for_id(Some(124)), "%");
      assert_eq!(unit_suffix_for_id(Some(121)), " m\u{00b3}");
    }

    #[test]
    fn it_yields_an_empty_suffix_for_unknown_or_missing_unit_ids() {
      assert_eq!(unit_suffix_for_id(Some(99_999)), "");
      assert_eq!(unit_suffix_for_id(None), "");
    }
  }

  mod load_filtered_cards {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      model::{AbyssalModuleStat, Alliance, Bloodline, Character, Corporation, Gender, Race},
    };

    async fn seed_character(db: &Database, id: i64) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_builds_a_card_per_owned_abyssal_item_with_its_rolled_stats() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 7).await;
      let abyssal = AbyssalItem::new(
        100,
        7,
        47_297,
        47_408,
        5975,
        r#"[{"attribute_id":6,"value":450.0}]"#.to_owned(),
        1_700_000_000,
      );
      assets::upsert(&db, &abyssal).await.unwrap();
      assets::upsert_module_stats(&db, &[AbyssalModuleStat::new(47_297, 6, 0.6, 1.4)])
        .await
        .unwrap();

      let cards = load_filtered_cards(&db, Scope::Character(7), &[], &Filters::default()).await;

      assert_eq!(cards.len(), 1);
      assert_eq!(cards[0].item_id, 100);
      assert_eq!(cards[0].character_id, 7);
      assert!(cards[0].stats.iter().any(|stat| stat.attribute_id == 6));
    }

    #[tokio::test]
    async fn it_returns_no_cards_for_a_character_with_no_abyssal_items() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 7).await;

      let cards = load_filtered_cards(&db, Scope::Character(7), &[], &Filters::default()).await;

      assert!(cards.is_empty());
    }
  }

  mod group_by_type {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_cards_by_type_with_a_break_between_groups() {
      let cards = [
        card(1, "Heavy Assault Missile Launcher II", 2410, "Unstable"),
        card(2, "Adaptive Invulnerability Field II", 2281, "Gravid"),
        card(3, "Heavy Assault Missile Launcher II", 2410, "Gravid"),
      ];

      let refs: Vec<&AbyssalCard> = cards.iter().collect();
      let groups = group_by_type(&refs);

      assert_eq!(groups.len(), 2);
      assert_eq!(groups[0].0, "Heavy Assault Missile Launcher II");
      assert_eq!(groups[0].1.len(), 2);
      assert_eq!(groups[0].1[0].item_id, 1);
      assert_eq!(groups[0].1[1].item_id, 3);
      assert_eq!(groups[1].0, "Adaptive Invulnerability Field II");
      assert_eq!(groups[1].1.len(), 1);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_grouped_card_grid() {
      let cards = [
        card(1, "Heavy Assault Missile Launcher II", 2410, "Unstable"),
        card(2, "Adaptive Invulnerability Field II", 2281, "Gravid"),
      ];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();
      let _el: Element<'_, Message> = body(refs, true, 0.0);
    }

    #[test]
    fn it_renders_the_empty_state() {
      let _el: Element<'_, Message> = body(Vec::new(), false, 0.0);
    }
  }
}
