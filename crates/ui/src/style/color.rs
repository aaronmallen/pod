use iced::Color;

/// Text and foreground color tokens.
pub mod text {
  use iced::Color;

  /// Primary body text — full-opacity ink.
  pub const PRIMARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 1.0,
  };
  /// Secondary / reduced-emphasis text — 55% opacity.
  pub const SECONDARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.55,
  };
  /// Tertiary / placeholder / timestamp text — 35% opacity.
  pub const TERTIARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.35,
  };
  /// Accent-colored text — plasma cyan.
  pub const ACCENT: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  };
  /// Danger / error state text.
  pub const DANGER: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 1.0,
  };
  /// Success / online state text.
  pub const SUCCESS: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 1.0,
  };
  /// Warning / caution state text.
  pub const WARNING: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 1.0,
  };
}

/// Surface and background color tokens.
pub mod surface {
  use iced::Color;

  /// Base page / application background.
  pub const BASE: Color = Color {
    r: 0.082,
    g: 0.090,
    b: 0.106,
    a: 1.0,
  };
  /// Elevated surface — cards, panels, popovers.
  pub const RAISED: Color = Color {
    r: 0.106,
    g: 0.118,
    b: 0.137,
    a: 1.0,
  };
  /// Sunken / recessed surface — inputs, filter bars, wells.
  pub const SUNKEN: Color = Color {
    r: 0.055,
    g: 0.059,
    b: 0.071,
    a: 1.0,
  };
  /// Navigation rail background.
  pub const NAVIGATION: Color = Color {
    r: 0.039,
    g: 0.043,
    b: 0.055,
    a: 1.0,
  };
}

/// Border and divider color tokens.
pub mod border {
  use iced::Color;

  /// Subtle rule — 10% opacity. Use for low-contrast separators.
  pub const SUBTLE: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.10,
  };
  /// Default rule — 18% opacity. Use for panel and card borders.
  pub const DEFAULT: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.18,
  };
}

/// Accent and brand color tokens.
pub mod accent {
  use iced::Color;

  /// Plasma cyan — primary brand accent.
  pub const PLASMA: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  };
  /// Plasma cyan at 10% opacity — subtle accent fill.
  pub const PLASMA_SUBTLE: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.10,
  };
  /// Plasma cyan at 25% opacity — accent border.
  pub const PLASMA_MUTED: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.25,
  };
  /// Omega gold — premium clone / subscription indicator.
  pub const GOLD: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 1.0,
  };
}

/// Semantic status color tokens.
pub mod status {
  use iced::Color;

  /// Online / docked / success state.
  pub const ONLINE: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 1.0,
  };
  /// In-space / undocked state.
  pub const IN_SPACE: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  };
  /// Caution / training / warning state.
  pub const CAUTION: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 1.0,
  };
  /// Error / destructive action state.
  pub const DANGER: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 1.0,
  };
}

/// Transparent — convenience alias for `Color::TRANSPARENT`.
pub const TRANSPARENT: Color = Color::TRANSPARENT;
