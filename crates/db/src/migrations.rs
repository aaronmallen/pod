//! SeaORM migration runner for all database schema versions.

// SDE item data
pub mod m0000000001_create_item_categories;
pub mod m0000000002_create_item_groups;
pub mod m0000000003_create_market_groups;
pub mod m0000000004_create_item_types;
pub mod m0000000005_create_dogma_attributes;
pub mod m0000000006_create_type_icons;

// Universe geography
pub mod m0000000007_create_regions;
pub mod m0000000008_create_constellations;
pub mod m0000000009_create_solar_systems;
pub mod m0000000010_create_stars;
pub mod m0000000011_create_planets;
pub mod m0000000012_create_stargates;

// Lore / civilization
pub mod m0000000013_create_races;
pub mod m0000000014_create_factions;
pub mod m0000000015_create_bloodlines;
pub mod m0000000016_create_stations;

// Characters
pub mod m0000000017_create_characters;
pub mod m0000000018_create_character_skills;
pub mod m0000000019_create_character_assets;

// Tags
pub mod m0000000020_create_tags;
pub mod m0000000021_create_entity_tags;
pub mod m0000000050_add_color_and_sort_order_to_tags;

// Wallet
pub mod m0000000022_create_wallet_journal_entries;
pub mod m0000000023_create_wallet_transactions;

// Corporations
pub mod m0000000024_create_corporations;

// Mail
pub mod m0000000025_create_mail_headers;
pub mod m0000000026_create_snoozed_mails;

// Character extended data
pub mod m0000000027_create_character_clones;
pub mod m0000000028_create_character_clone_implants;
pub mod m0000000029_create_character_standings;
pub mod m0000000030_create_character_contacts;
pub mod m0000000031_create_character_contact_labels;
pub mod m0000000032_create_character_notifications;
pub mod m0000000033_create_character_killmails;
pub mod m0000000034_create_character_contracts;

// Skill plans
pub mod m0000000035_create_skill_plans;
pub mod m0000000036_create_skill_plan_entries;

// Certificates
pub mod m0000000037_create_certificates;
pub mod m0000000038_create_ship_mastery_certs;

// Price tracking
pub mod m0000000039_create_type_prices;
pub mod m0000000040_create_type_price_histories;

// Stockpiles
pub mod m0000000041_create_stockpiles;
pub mod m0000000042_create_stockpile_items;

// Universe cache
pub mod m0000000043_create_structure_cache;
pub mod m0000000044_add_solar_system_id_to_structure_cache;

// Character auth
pub mod m0000000045_add_granted_scopes_to_characters;

// Offline-first asset sync
pub mod m0000000046_add_active_ship_to_character_assets;
pub mod m0000000047_create_corporation_assets;
pub mod m0000000048_create_asset_sync_state;

// Canonical asset valuation
pub mod m0000000049_add_adjusted_price_to_type_prices;

// Mail body caching
pub mod m0000000051_add_body_and_preview_to_mail_headers;

// Abyssal items
pub mod m0000000052_add_is_abyssal_to_item_types;
pub mod m0000000053_create_abyssal_module_stats;
pub mod m0000000054_create_abyssal_items;
pub mod m0000000055_create_abyssal_source_types;

use sea_orm::DatabaseConnection;
use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      // SDE item data
      Box::new(m0000000001_create_item_categories::Migration),
      Box::new(m0000000002_create_item_groups::Migration),
      Box::new(m0000000003_create_market_groups::Migration),
      Box::new(m0000000004_create_item_types::Migration),
      Box::new(m0000000005_create_dogma_attributes::Migration),
      Box::new(m0000000006_create_type_icons::Migration),
      // Universe geography
      Box::new(m0000000007_create_regions::Migration),
      Box::new(m0000000008_create_constellations::Migration),
      Box::new(m0000000009_create_solar_systems::Migration),
      Box::new(m0000000010_create_stars::Migration),
      Box::new(m0000000011_create_planets::Migration),
      Box::new(m0000000012_create_stargates::Migration),
      // Lore / civilization
      Box::new(m0000000013_create_races::Migration),
      Box::new(m0000000014_create_factions::Migration),
      Box::new(m0000000015_create_bloodlines::Migration),
      Box::new(m0000000016_create_stations::Migration),
      // Characters
      Box::new(m0000000017_create_characters::Migration),
      Box::new(m0000000018_create_character_skills::Migration),
      Box::new(m0000000019_create_character_assets::Migration),
      // Tags
      Box::new(m0000000020_create_tags::Migration),
      Box::new(m0000000021_create_entity_tags::Migration),
      Box::new(m0000000050_add_color_and_sort_order_to_tags::Migration),
      // Wallet
      Box::new(m0000000022_create_wallet_journal_entries::Migration),
      Box::new(m0000000023_create_wallet_transactions::Migration),
      // Corporations
      Box::new(m0000000024_create_corporations::Migration),
      // Mail
      Box::new(m0000000025_create_mail_headers::Migration),
      Box::new(m0000000026_create_snoozed_mails::Migration),
      // Character extended data
      Box::new(m0000000027_create_character_clones::Migration),
      Box::new(m0000000028_create_character_clone_implants::Migration),
      Box::new(m0000000029_create_character_standings::Migration),
      Box::new(m0000000030_create_character_contacts::Migration),
      Box::new(m0000000031_create_character_contact_labels::Migration),
      Box::new(m0000000032_create_character_notifications::Migration),
      Box::new(m0000000033_create_character_killmails::Migration),
      Box::new(m0000000034_create_character_contracts::Migration),
      // Skill plans
      Box::new(m0000000035_create_skill_plans::Migration),
      Box::new(m0000000036_create_skill_plan_entries::Migration),
      // Certificates
      Box::new(m0000000037_create_certificates::Migration),
      Box::new(m0000000038_create_ship_mastery_certs::Migration),
      // Price tracking
      Box::new(m0000000039_create_type_prices::Migration),
      Box::new(m0000000040_create_type_price_histories::Migration),
      // Stockpiles
      Box::new(m0000000041_create_stockpiles::Migration),
      Box::new(m0000000042_create_stockpile_items::Migration),
      // Universe cache
      Box::new(m0000000043_create_structure_cache::Migration),
      Box::new(m0000000044_add_solar_system_id_to_structure_cache::Migration),
      // Character auth
      Box::new(m0000000045_add_granted_scopes_to_characters::Migration),
      // Offline-first asset sync
      Box::new(m0000000046_add_active_ship_to_character_assets::Migration),
      Box::new(m0000000047_create_corporation_assets::Migration),
      Box::new(m0000000048_create_asset_sync_state::Migration),
      // Canonical asset valuation
      Box::new(m0000000049_add_adjusted_price_to_type_prices::Migration),
      // Mail body caching
      Box::new(m0000000051_add_body_and_preview_to_mail_headers::Migration),
      // Abyssal items
      Box::new(m0000000052_add_is_abyssal_to_item_types::Migration),
      Box::new(m0000000053_create_abyssal_module_stats::Migration),
      Box::new(m0000000054_create_abyssal_items::Migration),
      Box::new(m0000000055_create_abyssal_source_types::Migration),
    ]
  }
}

pub async fn run(db: &DatabaseConnection) -> Result<(), DbErr> {
  Migrator::up(db, None).await
}
