//! Dimension tokens for specific UI components.

/// Badge component dimensions.
pub mod badge {
  /// Diameter of a status indicator dot.
  pub const DOT_SIZE: f32 = 6.0;
}

/// Button component padding constants.
pub mod button {
  use iced::Padding;

  /// Standard button padding — used by `ghost`, `primary`, and `danger`.
  pub const PADDING_DEFAULT: Padding = Padding {
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
    top: 8.0,
  };

  /// Ghost/outline variant padding — slightly tighter horizontal inset.
  pub const PADDING_GHOST: Padding = Padding {
    bottom: 7.0,
    left: 10.0,
    right: 10.0,
    top: 7.0,
  };

  /// Row / table-cell button padding — compact variant.
  pub const PADDING_ROW: Padding = Padding {
    bottom: 6.0,
    left: 8.0,
    right: 8.0,
    top: 6.0,
  };
}

/// Compose-panel window dimensions.
pub mod compose_panel {
  /// Height when the panel is collapsed.
  pub const COLLAPSED_HEIGHT: f32 = 480.0;

  /// Width when the panel is collapsed.
  pub const COLLAPSED_WIDTH: f32 = 540.0;

  /// Height when the panel is fully expanded.
  pub const EXPANDED_HEIGHT: f32 = 640.0;

  /// Width when the panel is fully expanded.
  pub const EXPANDED_WIDTH: f32 = 820.0;
}

/// Toggle switch dimensions.
pub mod toggle {
  /// Left offset of the thumb when toggled off.
  pub const THUMB_OFF_OFFSET: f32 = 2.0;

  /// Left offset of the thumb when toggled on.
  pub const THUMB_ON_OFFSET: f32 = 17.0;

  /// Diameter of the thumb circle.
  pub const THUMB_SIZE: f32 = 14.0;

  /// Height of the track container.
  pub const TRACK_HEIGHT: f32 = 22.0;

  /// Width of the track container.
  pub const TRACK_WIDTH: f32 = 38.0;
}
