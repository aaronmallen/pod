pub mod accent {
  use iced::Color;

  pub const PLASMA: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  };
  pub const PLASMA_MUTED: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.25,
  };
}

pub mod chart {
  use iced::Color;

  pub const GOLD: Color = Color {
    r: 0.85,
    g: 0.78,
    b: 0.42,
    a: 1.0,
  };
  pub const VIOLET: Color = Color {
    r: 0.62,
    g: 0.55,
    b: 0.86,
    a: 1.0,
  };
}

pub mod state {
  use iced::Color;

  pub const OVERLAY_DARK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.4,
  };
}

pub mod status {
  use iced::Color;

  pub const DANGER: Color = Color {
    r: 0.878,
    g: 0.459,
    b: 0.349,
    a: 1.0,
  };
  pub const ONLINE: Color = Color {
    r: 0.357,
    g: 0.725,
    b: 0.494,
    a: 1.0,
  };
  pub const WARNING: Color = Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 1.0,
  };
}

pub mod surface {
  use iced::Color;

  pub const BASE: Color = Color {
    r: 0.082,
    g: 0.090,
    b: 0.106,
    a: 1.0,
  };
  pub const NAVIGATION: Color = Color {
    r: 0.039,
    g: 0.043,
    b: 0.055,
    a: 1.0,
  };
  pub const RAISED: Color = Color {
    r: 0.106,
    g: 0.118,
    b: 0.137,
    a: 1.0,
  };
  pub const SUNKEN: Color = Color {
    r: 0.055,
    g: 0.059,
    b: 0.071,
    a: 1.0,
  };
}

pub mod text {
  use iced::Color;

  pub const DIM: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.45,
  };
  pub const PRIMARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 1.0,
  };
  pub const SECONDARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.55,
  };
  pub const TERTIARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.35,
  };
}

pub fn with_alpha(base: iced::Color, alpha: f32) -> iced::Color {
  iced::Color {
    a: alpha,
    ..base
  }
}
