use iced::{Color, Shadow, Vector};

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
