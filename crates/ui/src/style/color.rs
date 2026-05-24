use iced::Color;

/// Transparent — convenience alias for `Color::TRANSPARENT`.
pub const TRANSPARENT: Color = Color::TRANSPARENT;

/// Text and foreground color tokens.
pub mod text {
  use iced::Color;

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
  /// Dimmed text — 45% opacity. Used for secondary chart labels and status indicators.
  pub const DIM: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.45,
  };
  /// Ghost text — 18% opacity. Used for near-invisible placeholders.
  pub const GHOST: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.18,
  };
  /// Medium text — 65% opacity. Used for mid-tier hierarchy labels.
  pub const MEDIUM: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.65,
  };
  /// Muted text — 60% opacity. Used for lower-tier hierarchy labels.
  pub const MUTED: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.60,
  };
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
  /// Strong text — 78% opacity. Used for location and system names in hierarchy trees.
  pub const STRONG: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.78,
  };
  /// Success / online state text.
  pub const SUCCESS: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 1.0,
  };
  /// Tertiary / placeholder / timestamp text — 35% opacity.
  pub const TERTIARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.35,
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

  /// Cobalt blue — used for contract, clone, and contact notification types.
  pub const COBALT: Color = Color {
    r: 0.376,
    g: 0.647,
    b: 0.902,
    a: 1.0,
  };
  /// Danger red hover — 85% opacity danger red for destructive button hover state.
  pub const DANGER_HOVER: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 0.85,
  };
  /// Omega gold — premium clone / subscription indicator.
  pub const GOLD: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 1.0,
  };
  /// Omega gold at 40% opacity — used for warning text labels.
  pub const GOLD_DIM: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 0.40,
  };
  /// Omega gold at 6% opacity — subtle warning background fill.
  pub const GOLD_FAINT: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 0.06,
  };
  /// Omega gold at 20% opacity — warning border color.
  pub const GOLD_MUTED: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 0.20,
  };
  /// Omega gold at 10% opacity — skill category chip background.
  pub const GOLD_SUBTLE: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 0.10,
  };
  /// Plasma cyan — primary brand accent.
  pub const PLASMA: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  };
  /// Plasma cyan at 15% opacity — selected conversation or item background.
  pub const PLASMA_ACTIVE: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.15,
  };
  /// Plasma cyan at 18% opacity — ESI status banner background.
  pub const PLASMA_BANNER: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.18,
  };
  /// Plasma cyan at 35% opacity — accent border and active indicator line.
  pub const PLASMA_BORDER: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.35,
  };
  /// Plasma cyan at 50% opacity — filled pip or bar segment.
  pub const PLASMA_HALF: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.50,
  };
  /// Plasma cyan at 12% opacity — tracker or tag selection background.
  pub const PLASMA_HIGHLIGHT: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.12,
  };
  /// Plasma cyan hover — 85% opacity for primary button hover and send button hover.
  pub const PLASMA_HOVER: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.85,
  };
  /// Plasma cyan at 25% opacity — accent border.
  pub const PLASMA_MUTED: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.25,
  };
  /// Plasma cyan at 8% opacity — very subtle selection background.
  pub const PLASMA_SELECTED: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.08,
  };
  /// Plasma cyan at 10% opacity — subtle accent fill.
  pub const PLASMA_SUBTLE: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.10,
  };
}

/// Semantic status color tokens.
pub mod status {
  use iced::Color;

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
  /// Danger red at 30% opacity — used for destructive state borders.
  pub const DANGER_BORDER: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 0.30,
  };
  /// Danger red at 8% opacity — very subtle danger hover background.
  pub const DANGER_FAINT: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 0.08,
  };
  /// Danger red at 35% opacity — muted danger indicator color.
  pub const DANGER_MUTED: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 0.35,
  };
  /// Danger red at 65% opacity — strong danger standing or dot color.
  pub const DANGER_STRONG: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 0.65,
  };
  /// Danger red at 12% opacity — subtle danger background fill.
  pub const DANGER_SUBTLE: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 0.12,
  };
  /// In-space / undocked state.
  pub const IN_SPACE: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  };
  /// Online / docked / success state.
  pub const ONLINE: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 1.0,
  };
  /// Online green at 35% opacity — muted success fill for stockpile borders and progress bars.
  pub const ONLINE_MUTED: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 0.35,
  };
  /// Online green at 65% opacity — strong positive standing or dot color.
  pub const ONLINE_STRONG: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 0.65,
  };
  /// Online green at 12% opacity — subtle success background fill.
  pub const ONLINE_SUBTLE: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 0.12,
  };
  /// Bright green at 12% opacity — killlog victory background.
  pub const VICTORY_SUBTLE: Color = Color {
    r: 0.275,
    g: 0.788,
    b: 0.431,
    a: 0.12,
  };
}

/// Interactive state overlay and fill color tokens.
pub mod state {
  use iced::Color;

  /// Nav-button active background — 10% warm-white overlay.
  pub const ACTIVE_OVERLAY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.10,
  };
  /// Danger background fill — dark red used for destructive action backgrounds.
  pub const DANGER_FILL: Color = Color {
    r: 0.102,
    g: 0.039,
    b: 0.035,
    a: 1.0,
  };
  /// Hover background overlay — 4% warm-white tint.
  pub const HOVER_OVERLAY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.04,
  };
  /// Light scrim over content — 40% black.
  pub const OVERLAY_DARK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.40,
  };
  /// Heavy scrim — 60% black.
  pub const OVERLAY_DARKER: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.60,
  };
  /// Medium scrim — 50% black.
  pub const OVERLAY_MEDIUM: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.50,
  };
  /// Pressed / active background overlay — 8% warm-white tint.
  pub const PRESSED_OVERLAY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.08,
  };
  /// Text-input selection highlight — plasma cyan at 30% opacity.
  pub const SELECTION: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.30,
  };
  /// Faint filled surface — 6% warm-white; used for subtle container backgrounds.
  pub const SUBTLE_FILL: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.06,
  };
  /// Tag chip fill — neutral grey at 8% opacity.
  pub const TAG_FILL: Color = Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 0.08,
  };
  /// Toggle thumb color (on state) — deep teal.
  pub const TOGGLE_THUMB: Color = Color {
    r: 0.039,
    g: 0.106,
    b: 0.133,
    a: 1.0,
  };
}

/// Chart palette — cycled colors for data visualizations.
pub mod chart {
  use iced::Color;

  /// Palette slot 3 — deep coral red.
  pub const P3: Color = Color {
    r: 0.843,
    g: 0.459,
    b: 0.349,
    a: 1.0,
  };
  /// Palette slot 4 — bright green.
  pub const P4: Color = Color {
    r: 0.498,
    g: 0.710,
    b: 0.353,
    a: 1.0,
  };
  /// Palette slot 5 — violet purple.
  pub const P5: Color = Color {
    r: 0.655,
    g: 0.498,
    b: 0.847,
    a: 1.0,
  };
  /// Palette slot 6 — teal green.
  pub const P6: Color = Color {
    r: 0.353,
    g: 0.722,
    b: 0.627,
    a: 1.0,
  };
  /// Palette slot 7 — salmon rose.
  pub const P7: Color = Color {
    r: 0.847,
    g: 0.400,
    b: 0.431,
    a: 1.0,
  };
}

/// Returns plasma cyan with heat-mapped opacity for the asset values matrix (4 %–20 %).
pub fn plasma_heat(intensity: f32) -> Color {
  Color {
    a: 0.04 + 0.16 * intensity,
    ..accent::PLASMA
  }
}

/// Returns `base` with its alpha channel replaced by `alpha`.
pub fn with_alpha(base: Color, alpha: f32) -> Color {
  Color {
    a: alpha,
    ..base
  }
}
