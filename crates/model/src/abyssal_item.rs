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
  /// The character that owns this item.
  pub character_id: i64,
  /// Resolved location string (station or structure name).
  pub location: String,
  /// Estimated MutaMarket price, or None if not yet priced.
  pub muta_price_isk: Option<f64>,
  /// HSL hue mapped from the mutaplasmid tier for badge coloring.
  pub mutaplasmid_color_hue: u16,
  /// Tier string extracted from the mutaplasmid item name
  /// (e.g. "Decayed", "Gravid", "Unstable", "Glorified Decayed").
  pub mutaplasmid_tier: String,
  /// The EVE item ID for this specific singleton.
  pub item_id: i64,
  /// Stats for this item, sorted alphabetically by display_name.
  pub stats: Vec<AbyssalStatViewModel>,
  /// Display name of the source (base) item type.
  pub base_type_name: String,
  /// Type ID of this abyssal item (the rolled/mutated type).
  pub type_id: i32,
}
