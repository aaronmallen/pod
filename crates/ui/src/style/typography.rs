/// Space Grotesk — primary sans-serif typeface used for body text and UI labels.
pub mod body {
  use iced::{Font, font};

  /// Regular weight (400).
  pub const REGULAR: Font = Font::with_name("Space Grotesk");

  /// Medium weight (500) — section labels, card names, emphasis.
  pub const MEDIUM: Font = Font {
    family: font::Family::Name("Space Grotesk"),
    weight: font::Weight::Medium,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
  };

  /// Semibold weight (600) — headers, strong emphasis.
  pub const SEMIBOLD: Font = Font {
    family: font::Family::Name("Space Grotesk"),
    weight: font::Weight::Semibold,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
  };
}

/// Raw font bytes for registration with the iced runtime.
pub mod bytes {
  pub const BODY_MEDIUM: &[u8] = include_bytes!("../../../../assets/fonts/SpaceGrotesk-Medium.ttf");
  pub const BODY_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/SpaceGrotesk-Regular.ttf");
  pub const BODY_SEMIBOLD: &[u8] = include_bytes!("../../../../assets/fonts/SpaceGrotesk-SemiBold.ttf");
  pub const MONO_ITALIC: &[u8] = include_bytes!("../../../../assets/fonts/JetBrainsMono-Italic.ttf");
  pub const MONO_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/JetBrainsMono.ttf");
}

/// JetBrains Mono — monospace typeface used for code, query syntax, and technical labels.
pub mod mono {
  use iced::{Font, font};

  /// Regular weight (400).
  pub const REGULAR: Font = Font::with_name("JetBrains Mono");

  /// Medium weight (500) — query chips, section headings.
  pub const MEDIUM: Font = Font {
    family: font::Family::Name("JetBrains Mono"),
    weight: font::Weight::Medium,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
  };

  /// Italic regular — code annotations, inline examples.
  pub const ITALIC: Font = Font {
    family: font::Family::Name("JetBrains Mono"),
    weight: font::Weight::Normal,
    stretch: font::Stretch::Normal,
    style: font::Style::Italic,
  };
}

/// Font size scale tokens (in points).
pub mod size {
  /// Extra-small — 9 pt. Micro labels, badge counts.
  pub const XS: f32 = 9.0;
  /// Small — 11 pt. Secondary labels and tag text.
  pub const SM: f32 = 11.0;
  /// Medium — 13 pt. Default body text (iced default).
  pub const MD: f32 = 13.0;
  /// Large — 15 pt. Emphasis and stat values.
  pub const LG: f32 = 15.0;
  /// Extra-large — 17 pt. Headings and character names.
  pub const XL: f32 = 17.0;
}
