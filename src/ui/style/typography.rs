pub mod body {
  use iced::{Font, font};

  pub const MEDIUM: Font = Font {
    family: font::Family::Name("Space Grotesk"),
    weight: font::Weight::Medium,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
  };
  pub const REGULAR: Font = Font::with_name("Space Grotesk");
}

pub mod bytes {
  pub const BODY_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/SpaceGrotesk-Medium.ttf");
  pub const BODY_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/SpaceGrotesk-Regular.ttf");
  pub const BODY_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/SpaceGrotesk-SemiBold.ttf");
  pub const MONO_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf");
  pub const MONO_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono.ttf");
}

pub mod mono {
  use iced::{Font, font};

  pub const MEDIUM: Font = Font {
    family: font::Family::Name("JetBrains Mono"),
    weight: font::Weight::Medium,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
  };
  pub const REGULAR: Font = Font::with_name("JetBrains Mono");
  pub const SEMIBOLD: Font = Font {
    family: font::Family::Name("JetBrains Mono"),
    weight: font::Weight::Semibold,
    stretch: font::Stretch::Normal,
    style: font::Style::Normal,
  };
}

pub mod size {
  pub const LG: f32 = 17.0;
  pub const MD: f32 = 13.0;
  pub const SM: f32 = 11.0;
  pub const XS: f32 = 9.0;
  pub const XS_PLUS: f32 = 10.0;
}

pub fn colored(color: iced::Color) -> impl Fn(&iced::Theme) -> iced::widget::text::Style {
  move |_| iced::widget::text::Style {
    color: Some(color),
  }
}
