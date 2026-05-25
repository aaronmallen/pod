//! View model types for abyssal (mutated) modules.

/// A single rolled stat on an abyssal item, enriched with display metadata.
#[derive(Clone, Debug)]
pub struct AbyssalStatViewModel {
  /// EVE dogma attribute ID.
  pub attribute_id: i32,
  /// Base (unmutated) value of this stat on the source item type.
  pub base_value: f64,
  /// Human-readable stat name from the dogma_attributes table.
  pub display_name: String,
  /// Whether a higher value is better for this stat.
  pub high_is_good: bool,
  /// EVE icon ID for the attribute, used to fetch the attribute icon image.
  pub icon_id: Option<i32>,
  /// Maximum multiplier from the abyssal_module_stats table.
  pub max_mult: f64,
  /// Minimum multiplier from the abyssal_module_stats table.
  pub min_mult: f64,
  /// The actual rolled value on this specific item.
  pub rolled_value: f64,
  /// Unit suffix string derived from the DOGMA_UNITS map (e.g. " tf", " GJ", "%").
  pub unit_suffix: String,
}

/// Denormalized view model for rendering an abyssal item card in the UI.
#[derive(Clone, Debug)]
pub struct AbyssalViewModel {
  /// Display name of the source (base) item type.
  pub base_type_name: String,
  /// The character that owns this item.
  pub character_id: i64,
  /// The EVE item ID for this specific singleton.
  pub item_id: i64,
  /// Resolved location string (station or structure name).
  pub location: String,
  /// Estimated MutaMarket price, or None if not yet priced.
  pub muta_price_isk: Option<f64>,
  /// HSL hue mapped from the mutaplasmid tier for badge coloring.
  pub mutaplasmid_color_hue: u16,
  /// Tier string extracted from the mutaplasmid item name
  /// (e.g. "Decayed", "Gravid", "Unstable", "Glorified Decayed").
  pub mutaplasmid_tier: String,
  /// Type ID of the source (base) module used as mutaplasmid input.
  pub source_type_id: i32,
  /// Stats for this item, sorted alphabetically by display_name.
  pub stats: Vec<AbyssalStatViewModel>,
  /// Type ID of this abyssal item (the rolled/mutated type).
  pub type_id: i32,
}

/// A single source module type within a category.
///
/// Each EVE source type ID (T1, T2, or faction variant) gets its own entry.
#[derive(Clone, Debug)]
pub struct AbyssalSourceType {
  /// Display name shown to the user (e.g. "Small Shield Booster II").
  pub name: String,
  /// EVE source type ID for this specific variant.
  pub type_id: i32,
  /// Stat bounds for this source type, used to render slider ranges.
  /// Empty when no stat bound data is available for this type.
  pub stat_templates: Vec<AbyssalStatViewModel>,
}

/// A top-level filter category containing source module type entries.
#[derive(Clone, Debug)]
pub struct AbyssalCategory {
  /// Category display name (e.g. "Shield", "Propulsion").
  pub name: String,
  /// Source type entries within this category, in definition order.
  pub source_types: Vec<AbyssalSourceType>,
}

/// All data needed to render the abyssals tab on initial load.
#[derive(Clone, Debug, Default)]
pub struct AbyssalsData {
  /// All available filter categories derived from the SDE source types.
  pub categories: Vec<AbyssalCategory>,
  /// All owned abyssal items.
  pub items: Vec<AbyssalViewModel>,
  /// Raw icon bytes for each source type, keyed by source_type_id.
  pub type_icons: Vec<(i32, Vec<u8>)>,
}
