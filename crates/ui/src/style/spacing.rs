/// Base grid unit — 4px. All spacing values are multiples of this unit.
pub const UNIT: f32 = 4.0;

/// 4px — tightest gap. Icon-to-label, tag clusters.
pub const SPACE_1: f32 = 4.0;
/// 8px — compact gap. Related list items, inline elements.
pub const SPACE_2: f32 = 8.0;
/// 10px — between-element gap. Slightly looser than compact.
pub const SPACE_2_5: f32 = 10.0;
/// 12px — default component inner spacing.
pub const SPACE_3: f32 = 12.0;
/// 14px — between-section gap. Slightly looser than default.
pub const SPACE_3_5: f32 = 14.0;
/// 16px — standard section gap. Card grid spacing, content rows.
pub const SPACE_4: f32 = 16.0;
/// 20px — medium-large gap.
pub const SPACE_5: f32 = 20.0;
/// 24px — large section spacing. Container inner padding.
pub const SPACE_6: f32 = 24.0;
/// 28px — extra-large gap.
pub const SPACE_7: f32 = 28.0;
/// 32px — page-level margins and gutters.
pub const SPACE_8: f32 = 32.0;

/// Distance from the top or bottom edge of the scroll area that
/// triggers auto-scroll during a drag operation.
pub const SCROLL_EDGE_THRESHOLD: f32 = 60.0;
/// Number of pixels to nudge the scroll position per drag-move event
/// when the cursor is within SCROLL_EDGE_THRESHOLD of an edge.
pub const SCROLL_NUDGE_PX: f32 = 20.0;

/// Layout dimension tokens — fixed structural sizes that are not part of the spacing scale.
pub mod layout {
  /// Height of the top page header bar.
  pub const HEADER_HEIGHT: f32 = 92.0;
  /// Width of the left navigation rail.
  pub const RAIL_WIDTH: f32 = 68.0;
  /// Touch target height for navigation rail items.
  pub const NAV_ITEM_HEIGHT: f32 = 44.0;
  /// Fixed height of a character card.
  pub const CHARACTER_CARD_HEIGHT: f32 = 400.0;
  /// Fixed height of the character portrait area.
  pub const CHARACTER_PORTRAIT_HEIGHT: f32 = 140.0;
  /// Fixed height of a corporation card.
  pub const CORPORATION_CARD_HEIGHT: f32 = 400.0;
  /// Default main window width.
  pub const WINDOW_DEFAULT_WIDTH: f32 = 1200.0;
  /// Default main window height.
  pub const WINDOW_DEFAULT_HEIGHT: f32 = 800.0;
  /// Minimum main window width.
  pub const WINDOW_MIN_WIDTH: f32 = 900.0;
  /// Minimum main window height.
  pub const WINDOW_MIN_HEIGHT: f32 = 600.0;
  /// Splash / auth window width.
  pub const SPLASH_WIDTH: f32 = 480.0;
  /// Splash / auth window height.
  pub const SPLASH_HEIGHT: f32 = 320.0;
  /// Maximum width of the character card grid.
  pub const GRID_MAX_WIDTH: f32 = 1280.0;
}
