use iced::{Color, Shadow, Vector};

/// Deep shadow for floating panels — popovers, context menus, dropdowns.
/// High blur and vertical offset creates strong depth separation from content below.
pub const POPOVER: Shadow = Shadow {
  color: Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.6,
  },
  offset: Vector {
    x: 0.0,
    y: 24.0,
  },
  blur_radius: 64.0,
};

/// Medium shadow for elevated cards.
/// Softer than POPOVER — cards sit close to the surface, not fully detached.
pub const CARD: Shadow = Shadow {
  color: Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.4,
  },
  offset: Vector {
    x: 0.0,
    y: 8.0,
  },
  blur_radius: 24.0,
};

/// No shadow — flat surface, no elevation.
pub const NONE: Shadow = Shadow {
  color: Color::TRANSPARENT,
  offset: Vector {
    x: 0.0,
    y: 0.0,
  },
  blur_radius: 0.0,
};
