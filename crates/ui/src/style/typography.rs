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

/// Raw font bytes for registration with the iced runtime.
pub mod bytes {
  pub const BODY_MEDIUM: &[u8] = include_bytes!("../../../../assets/fonts/SpaceGrotesk-Medium.ttf");
  pub const BODY_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/SpaceGrotesk-Regular.ttf");
  pub const BODY_SEMIBOLD: &[u8] = include_bytes!("../../../../assets/fonts/SpaceGrotesk-SemiBold.ttf");
  pub const MONO_ITALIC: &[u8] = include_bytes!("../../../../assets/fonts/JetBrainsMono-Italic.ttf");
  pub const MONO_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/JetBrainsMono.ttf");
}
