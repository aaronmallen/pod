pub mod accent {
  use iced::Color;

  pub const PLASMA: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  };
  pub const PLASMA_INK: Color = Color {
    r: 0.0392,
    g: 0.0902,
    b: 0.1098,
    a: 1.0,
  };
  pub const PLASMA_MUTED: Color = Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 0.25,
  };
  pub const PLASMA_PRESSED: Color = Color {
    r: 0.1843,
    g: 0.6392,
    b: 0.7686,
    a: 1.0,
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
  pub const WORMHOLE: Color = Color {
    r: 0.725,
    g: 0.545,
    b: 0.851,
    a: 1.0,
  };

  pub fn series(index: usize) -> Color {
    const PALETTE: [Color; 5] = [
      super::accent::PLASMA,
      super::status::ONLINE,
      super::status::DANGER,
      GOLD,
      VIOLET,
    ];
    PALETTE[index % PALETTE.len()]
  }
}

pub mod state {
  use iced::Color;

  pub const OVERLAY_DARK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.4,
  };
  pub const SCRIM: Color = Color {
    r: 8.0 / 255.0,
    g: 9.0 / 255.0,
    b: 11.0 / 255.0,
    a: 0.6,
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
  pub const DANGER_INK: Color = Color {
    r: 0.1098,
    g: 0.0549,
    b: 0.0392,
    a: 1.0,
  };
  pub const DANGER_PRESSED: Color = Color {
    r: 0.7882,
    g: 0.3843,
    b: 0.2902,
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

  use super::high_contrast;

  pub const PRIMARY: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 1.0,
  };

  const DIM_HC: Color = Color {
    r: 0.573,
    g: 0.565,
    b: 0.545,
    a: 1.0,
  };
  const DIM_OFF: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.45,
  };
  const SECONDARY_HC: Color = Color {
    r: 0.812,
    g: 0.804,
    b: 0.780,
    a: 1.0,
  };
  const SECONDARY_OFF: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.55,
  };
  const TERTIARY_HC: Color = Color {
    r: 0.682,
    g: 0.675,
    b: 0.651,
    a: 1.0,
  };
  const TERTIARY_OFF: Color = Color {
    r: 0.957,
    g: 0.949,
    b: 0.925,
    a: 0.35,
  };

  pub fn dim() -> Color {
    if high_contrast() { DIM_HC } else { DIM_OFF }
  }

  /// Raw high-contrast value for the settings preview's static before/after columns.
  ///
  /// Unlike [`dim`], this ignores the live high-contrast flag so the preview can show both states
  /// side by side regardless of which one is active.
  pub fn dim_hc() -> Color {
    DIM_HC
  }

  /// Raw overlay value for the settings preview's static before/after columns; see [`dim_hc`].
  pub fn dim_off() -> Color {
    DIM_OFF
  }

  pub fn secondary() -> Color {
    if high_contrast() { SECONDARY_HC } else { SECONDARY_OFF }
  }

  pub fn secondary_hc() -> Color {
    SECONDARY_HC
  }

  pub fn secondary_off() -> Color {
    SECONDARY_OFF
  }

  pub fn tertiary() -> Color {
    if high_contrast() { TERTIARY_HC } else { TERTIARY_OFF }
  }

  pub fn tertiary_hc() -> Color {
    TERTIARY_HC
  }

  pub fn tertiary_off() -> Color {
    TERTIARY_OFF
  }
}

pub const ACCENT_PRESETS: [AccentPreset; 8] = [
  AccentPreset {
    hex: "#3FB8DB",
    name: "Plasma",
    shades: PLASMA_SHADES,
  },
  AccentPreset {
    hex: "#36C6A8",
    name: "Aurora",
    shades: AccentShades {
      base: iced::Color::from_rgb8(54, 198, 168),
      ink: iced::Color::from_rgb8(7, 26, 22),
      muted: iced::Color::from_rgba8(54, 198, 168, 0.25),
      pressed: iced::Color::from_rgb8(47, 172, 146),
    },
  },
  AccentPreset {
    hex: "#5FBE6E",
    name: "Verdant",
    shades: AccentShades {
      base: iced::Color::from_rgb8(95, 190, 110),
      ink: iced::Color::from_rgb8(12, 25, 14),
      muted: iced::Color::from_rgba8(95, 190, 110, 0.25),
      pressed: iced::Color::from_rgb8(83, 165, 96),
    },
  },
  AccentPreset {
    hex: "#D9B252",
    name: "Solar",
    shades: AccentShades {
      base: iced::Color::from_rgb8(217, 178, 82),
      ink: iced::Color::from_rgb8(28, 23, 11),
      muted: iced::Color::from_rgba8(217, 178, 82, 0.25),
      pressed: iced::Color::from_rgb8(189, 155, 71),
    },
  },
  AccentPreset {
    hex: "#E08259",
    name: "Ember",
    shades: AccentShades {
      base: iced::Color::from_rgb8(224, 130, 89),
      ink: iced::Color::from_rgb8(29, 17, 12),
      muted: iced::Color::from_rgba8(224, 130, 89, 0.25),
      pressed: iced::Color::from_rgb8(195, 113, 77),
    },
  },
  AccentPreset {
    hex: "#DE6F94",
    name: "Rose",
    shades: AccentShades {
      base: iced::Color::from_rgb8(222, 111, 148),
      ink: iced::Color::from_rgb8(29, 14, 19),
      muted: iced::Color::from_rgba8(222, 111, 148, 0.25),
      pressed: iced::Color::from_rgb8(193, 97, 129),
    },
  },
  AccentPreset {
    hex: "#B89BEA",
    name: "Orchid",
    shades: AccentShades {
      base: iced::Color::from_rgb8(184, 155, 234),
      ink: iced::Color::from_rgb8(24, 20, 30),
      muted: iced::Color::from_rgba8(184, 155, 234, 0.25),
      pressed: iced::Color::from_rgb8(160, 135, 204),
    },
  },
  AccentPreset {
    hex: "#7FA8EC",
    name: "Azure",
    shades: AccentShades {
      base: iced::Color::from_rgb8(127, 168, 236),
      ink: iced::Color::from_rgb8(17, 22, 31),
      muted: iced::Color::from_rgba8(127, 168, 236, 0.25),
      pressed: iced::Color::from_rgb8(110, 146, 205),
    },
  },
];

const ACCENT_MUTED_ALPHA: f32 = 0.25;
const ACCENT_PRESSED_DARKEN: f32 = 0.15;
/// Perceptual luminance cutoff (≈ 150/255) above which a fill is considered light.
const ON_FILL_LUMINANCE_THRESHOLD: f32 = 0.588;
const PLASMA_SHADES: AccentShades = AccentShades {
  base: accent::PLASMA,
  ink: accent::PLASMA_INK,
  muted: accent::PLASMA_MUTED,
  pressed: accent::PLASMA_PRESSED,
};
const RULE_HC_ALPHA: f32 = 0.22;
const RULE_OFF_ALPHA: f32 = 0.10;
const RULE_STRONG_HC_ALPHA: f32 = 0.34;
const RULE_STRONG_OFF_ALPHA: f32 = 0.18;

pub struct AccentPreset {
  pub hex: &'static str,
  #[cfg_attr(not(test), expect(dead_code))]
  pub name: &'static str,
  pub shades: AccentShades,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccentShades {
  pub base: iced::Color,
  pub ink: iced::Color,
  pub muted: iced::Color,
  pub pressed: iced::Color,
}

static ACCENT: std::sync::RwLock<AccentShades> = std::sync::RwLock::new(PLASMA_SHADES);
static HIGH_CONTRAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg_attr(not(test), expect(dead_code))]
pub fn accent() -> iced::Color {
  accent_shades().base
}

#[cfg_attr(not(test), expect(dead_code))]
pub fn accent_ink() -> iced::Color {
  accent_shades().ink
}

#[cfg_attr(not(test), expect(dead_code))]
pub fn accent_muted() -> iced::Color {
  accent_shades().muted
}

#[cfg_attr(not(test), expect(dead_code))]
pub fn accent_pressed() -> iced::Color {
  accent_shades().pressed
}

pub fn high_contrast() -> bool {
  HIGH_CONTRAST.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn rule() -> iced::Color {
  with_alpha(
    text::PRIMARY,
    if high_contrast() { RULE_HC_ALPHA } else { RULE_OFF_ALPHA },
  )
}

pub fn rule_hc_alpha() -> f32 {
  RULE_HC_ALPHA
}

pub fn rule_off_alpha() -> f32 {
  RULE_OFF_ALPHA
}

pub fn rule_strong() -> iced::Color {
  with_alpha(
    text::PRIMARY,
    if high_contrast() {
      RULE_STRONG_HC_ALPHA
    } else {
      RULE_STRONG_OFF_ALPHA
    },
  )
}

pub fn rule_strong_hc_alpha() -> f32 {
  RULE_STRONG_HC_ALPHA
}

pub fn rule_strong_off_alpha() -> f32 {
  RULE_STRONG_OFF_ALPHA
}

#[cfg_attr(not(test), expect(dead_code))]
pub fn set_accent(hex: &str) {
  if let Some(shades) = resolve_accent(hex) {
    *ACCENT.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = shades;
  }
}

pub fn set_high_contrast(enabled: bool) {
  HIGH_CONTRAST.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

pub fn darken(color: iced::Color, amount: f32) -> iced::Color {
  let factor = (1.0 - amount).clamp(0.0, 1.0);
  iced::Color {
    r: color.r * factor,
    g: color.g * factor,
    b: color.b * factor,
    a: color.a,
  }
}

/// Parses `#RGB`, `#RRGGBB`, or either form without the leading `#`.
pub fn from_hex(hex: &str) -> Option<iced::Color> {
  let expanded = expand_hex(hex)?;
  let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
  let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
  let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
  Some(iced::Color::from_rgb8(r, g, b))
}

/// Parses an EVE `#AARRGGBB` color (or the same eight hex digits without the leading `#`).
///
/// EVE stores mail font colors with a leading alpha byte. The alpha is dropped and the resulting
/// color is fully opaque, so authored colors render verbatim regardless of the high-contrast flag.
pub fn from_argb(hex: &str) -> Option<iced::Color> {
  let trimmed = hex.trim().trim_start_matches('#');
  if trimmed.len() != 8 {
    return None;
  }
  let r = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
  let g = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
  let b = u8::from_str_radix(&trimmed[6..8], 16).ok()?;
  Some(iced::Color::from_rgb8(r, g, b))
}

/// Returns a legible foreground (dark or light) for a given fill based on perceptual luminance.
pub fn on_fill(fill: iced::Color) -> iced::Color {
  let luminance = 0.299 * fill.r + 0.587 * fill.g + 0.114 * fill.b;
  if luminance > ON_FILL_LUMINANCE_THRESHOLD {
    surface::BASE
  } else {
    text::PRIMARY
  }
}

pub fn resolve_accent(hex: &str) -> Option<AccentShades> {
  let expanded = expand_hex(hex)?;
  if let Some(preset) = ACCENT_PRESETS
    .iter()
    .find(|preset| preset.hex[1..].eq_ignore_ascii_case(&expanded))
  {
    return Some(preset.shades);
  }
  let base = from_hex(&expanded)?;
  Some(AccentShades {
    base,
    ink: on_fill(base),
    muted: with_alpha(base, ACCENT_MUTED_ALPHA),
    pressed: darken(base, ACCENT_PRESSED_DARKEN),
  })
}

pub fn with_alpha(base: iced::Color, alpha: f32) -> iced::Color {
  iced::Color {
    a: alpha,
    ..base
  }
}

fn accent_shades() -> AccentShades {
  *ACCENT.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn expand_hex(hex: &str) -> Option<String> {
  let trimmed = hex.trim().trim_start_matches('#');
  match trimmed.len() {
    3 => Some(trimmed.chars().flat_map(|c| [c, c]).collect()),
    6 => Some(trimmed.to_owned()),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  mod accent_presets {
    use pretty_assertions::assert_eq;

    use super::super::{ACCENT_PRESETS, accent};

    #[test]
    fn it_defines_the_eight_curated_presets() {
      let names: Vec<&str> = ACCENT_PRESETS.iter().map(|preset| preset.name).collect();
      let hexes: Vec<&str> = ACCENT_PRESETS.iter().map(|preset| preset.hex).collect();

      assert_eq!(
        names,
        vec![
          "Plasma", "Aurora", "Verdant", "Solar", "Ember", "Rose", "Orchid", "Azure"
        ]
      );
      assert_eq!(
        hexes,
        vec![
          "#3FB8DB", "#36C6A8", "#5FBE6E", "#D9B252", "#E08259", "#DE6F94", "#B89BEA", "#7FA8EC"
        ]
      );
    }

    #[test]
    fn it_pins_the_plasma_quartet_to_the_existing_constants() {
      let plasma = &ACCENT_PRESETS[0].shades;

      assert_eq!(plasma.base, accent::PLASMA);
      assert_eq!(plasma.ink, accent::PLASMA_INK);
      assert_eq!(plasma.muted, accent::PLASMA_MUTED);
      assert_eq!(plasma.pressed, accent::PLASMA_PRESSED);
    }
  }

  mod chart {
    mod series {
      use pretty_assertions::{assert_eq, assert_ne};

      use super::super::super::chart::series;

      #[test]
      fn it_cycles_the_palette_beyond_its_length() {
        assert_eq!(series(0), series(5));
        assert_eq!(series(2), series(7));
      }

      #[test]
      fn it_returns_distinct_colors_across_the_palette() {
        let palette = [series(0), series(1), series(2), series(3), series(4)];

        for (i, a) in palette.iter().enumerate() {
          for b in &palette[i + 1..] {
            assert_ne!(a, b);
          }
        }
      }
    }
  }

  mod darken {
    use pretty_assertions::assert_eq;

    use super::super::{darken, with_alpha};

    #[test]
    fn it_floors_the_factor_at_zero_for_large_amounts() {
      let base = iced::Color {
        r: 1.0,
        g: 0.5,
        b: 0.25,
        a: 1.0,
      };

      let darkened = darken(base, 2.0);

      assert_eq!(darkened.r, 0.0);
      assert_eq!(darkened.g, 0.0);
      assert_eq!(darkened.b, 0.0);
    }

    #[test]
    fn it_preserves_alpha() {
      let base = with_alpha(iced::Color::WHITE, 0.3);

      assert_eq!(darken(base, 0.5).a, 0.3);
    }

    #[test]
    fn it_scales_rgb_toward_black() {
      let base = iced::Color {
        r: 1.0,
        g: 0.5,
        b: 0.25,
        a: 1.0,
      };

      let darkened = darken(base, 0.5);

      assert_eq!(darkened.r, 0.5);
      assert_eq!(darkened.g, 0.25);
      assert_eq!(darkened.b, 0.125);
      assert_eq!(darkened.a, 1.0);
    }
  }

  mod from_hex {
    use pretty_assertions::assert_eq;

    use super::super::from_hex;

    #[test]
    fn it_expands_three_digit_shorthand() {
      assert_eq!(from_hex("#abc"), Some(iced::Color::from_rgb8(170, 187, 204)));
    }

    #[test]
    fn it_parses_six_digit_hex_with_and_without_a_hash() {
      assert_eq!(from_hex("#ff6600"), Some(iced::Color::from_rgb8(255, 102, 0)));
      assert_eq!(from_hex("ff6600"), Some(iced::Color::from_rgb8(255, 102, 0)));
    }

    #[test]
    fn it_rejects_malformed_input() {
      assert_eq!(from_hex(""), None);
      assert_eq!(from_hex("#12345"), None);
      assert_eq!(from_hex("zzzzzz"), None);
    }
  }

  mod from_argb {
    use pretty_assertions::assert_eq;

    use super::super::from_argb;

    #[test]
    fn it_drops_the_alpha_byte_and_returns_an_opaque_color() {
      // EVE encodes mail colors as #AARRGGBB; the leading alpha is discarded.
      assert_eq!(from_argb("#ffff0000"), Some(iced::Color::from_rgb8(255, 0, 0)));
      assert_eq!(from_argb("#bfffffff"), Some(iced::Color::from_rgb8(255, 255, 255)));
    }

    #[test]
    fn it_parses_eight_digits_with_or_without_a_hash() {
      assert_eq!(from_argb("ffffe400"), Some(iced::Color::from_rgb8(255, 228, 0)));
      assert_eq!(from_argb("#ffd98d00"), Some(iced::Color::from_rgb8(217, 141, 0)));
    }

    #[test]
    fn it_keeps_full_opacity_regardless_of_the_alpha_byte() {
      assert_eq!(from_argb("#00112233").map(|c| c.a), Some(1.0));
    }

    #[test]
    fn it_rejects_input_that_is_not_eight_hex_digits() {
      assert_eq!(from_argb(""), None);
      assert_eq!(from_argb("#ff0000"), None);
      assert_eq!(from_argb("#zzzzzzzz"), None);
    }
  }

  mod high_contrast {
    use std::sync::Mutex;

    use pretty_assertions::{assert_eq, assert_ne};

    use super::super::{rule, rule_strong, set_high_contrast, text};

    static GUARD: Mutex<()> = Mutex::new(());

    const DIM_HC: iced::Color = iced::Color {
      r: 0.573,
      g: 0.565,
      b: 0.545,
      a: 1.0,
    };

    const DIM_OFF: iced::Color = iced::Color {
      r: 0.957,
      g: 0.949,
      b: 0.925,
      a: 0.45,
    };

    const SECONDARY_HC: iced::Color = iced::Color {
      r: 0.812,
      g: 0.804,
      b: 0.780,
      a: 1.0,
    };

    const SECONDARY_OFF: iced::Color = iced::Color {
      r: 0.957,
      g: 0.949,
      b: 0.925,
      a: 0.55,
    };

    const TERTIARY_HC: iced::Color = iced::Color {
      r: 0.682,
      g: 0.675,
      b: 0.651,
      a: 1.0,
    };

    const TERTIARY_OFF: iced::Color = iced::Color {
      r: 0.957,
      g: 0.949,
      b: 0.925,
      a: 0.35,
    };

    #[test]
    fn it_firms_the_solids_above_the_overlays() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      set_high_contrast(false);
      let off = text::secondary();
      set_high_contrast(true);
      let on = text::secondary();
      set_high_contrast(false);

      assert_ne!(on, off);
    }

    #[test]
    fn it_keeps_primary_text_unchanged_in_both_states() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

      set_high_contrast(true);
      let on = text::PRIMARY;
      set_high_contrast(false);
      let off = text::PRIMARY;

      assert_eq!(on, off);
    }

    #[test]
    fn it_returns_the_translucent_overlays_when_disabled() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      set_high_contrast(false);

      assert_eq!(text::secondary(), SECONDARY_OFF);
      assert_eq!(text::tertiary(), TERTIARY_OFF);
      assert_eq!(text::dim(), DIM_OFF);
      assert_eq!(rule().a, 0.10);
      assert_eq!(rule_strong().a, 0.18);
    }

    #[test]
    fn it_returns_the_tuned_solids_when_enabled() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      set_high_contrast(true);

      assert_eq!(text::secondary(), SECONDARY_HC);
      assert_eq!(text::tertiary(), TERTIARY_HC);
      assert_eq!(text::dim(), DIM_HC);
      assert_eq!(rule().a, 0.22);
      assert_eq!(rule_strong().a, 0.34);

      set_high_contrast(false);
    }

    #[test]
    fn the_preview_accessors_return_fixed_values_regardless_of_the_live_flag() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

      set_high_contrast(true);
      assert_eq!(text::secondary_off(), SECONDARY_OFF);
      assert_eq!(text::secondary_hc(), SECONDARY_HC);
      assert_eq!(text::tertiary_off(), TERTIARY_OFF);
      assert_eq!(text::tertiary_hc(), TERTIARY_HC);
      assert_eq!(text::dim_off(), DIM_OFF);
      assert_eq!(text::dim_hc(), DIM_HC);

      set_high_contrast(false);
      assert_eq!(text::secondary_off(), SECONDARY_OFF);
      assert_eq!(text::secondary_hc(), SECONDARY_HC);
    }
  }

  mod on_fill {
    use pretty_assertions::assert_eq;

    use super::super::{on_fill, surface, text};

    #[test]
    fn it_picks_dark_foreground_over_a_light_fill() {
      assert_eq!(on_fill(iced::Color::from_rgb8(255, 255, 255)), surface::BASE);
      assert_eq!(on_fill(iced::Color::from_rgb8(255, 255, 205)), surface::BASE);
    }

    #[test]
    fn it_picks_light_foreground_over_a_dark_fill() {
      assert_eq!(on_fill(iced::Color::from_rgb8(0, 0, 254)), text::PRIMARY);
      assert_eq!(on_fill(iced::Color::from_rgb8(102, 0, 102)), text::PRIMARY);
    }
  }

  mod resolve_accent {
    use pretty_assertions::assert_eq;

    use super::super::{ACCENT_PRESETS, accent, darken, from_hex, on_fill, resolve_accent, with_alpha};

    #[test]
    fn it_derives_a_quartet_for_a_custom_hex() {
      let base = from_hex("#ff6600").unwrap();

      let shades = resolve_accent("#ff6600").unwrap();

      assert_eq!(shades.base, base);
      assert_eq!(shades.ink, on_fill(base));
      assert_eq!(shades.muted, with_alpha(base, 0.25));
      assert_eq!(shades.pressed, darken(base, 0.15));
    }

    #[test]
    fn it_matches_presets_case_insensitively_with_or_without_a_hash() {
      let plasma = resolve_accent("#3fb8db").unwrap();
      let aurora = resolve_accent("36c6a8").unwrap();

      assert_eq!(plasma.base, accent::PLASMA);
      assert_eq!(plasma.ink, accent::PLASMA_INK);
      assert_eq!(plasma.muted, accent::PLASMA_MUTED);
      assert_eq!(plasma.pressed, accent::PLASMA_PRESSED);
      assert_eq!(aurora, ACCENT_PRESETS[1].shades);
    }

    #[test]
    fn it_rejects_malformed_input() {
      assert_eq!(resolve_accent(""), None);
      assert_eq!(resolve_accent("#12345"), None);
      assert_eq!(resolve_accent("zzzzzz"), None);
    }
  }

  mod set_accent {
    use std::sync::Mutex;

    use pretty_assertions::assert_eq;

    use super::super::{ACCENT_PRESETS, accent, accent_ink, accent_muted, accent_pressed, set_accent};

    static GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn it_defaults_to_the_plasma_quartet() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

      assert_eq!(accent(), accent::PLASMA);
      assert_eq!(accent_ink(), accent::PLASMA_INK);
      assert_eq!(accent_muted(), accent::PLASMA_MUTED);
      assert_eq!(accent_pressed(), accent::PLASMA_PRESSED);
    }

    #[test]
    fn it_keeps_the_store_unchanged_for_invalid_hex() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

      set_accent("not-a-color");

      assert_eq!(accent(), accent::PLASMA);
    }

    #[test]
    fn it_updates_the_accessor_quartet() {
      let _lock = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      let aurora = ACCENT_PRESETS[1].shades;

      set_accent("#36C6A8");
      let observed = (accent(), accent_ink(), accent_muted(), accent_pressed());
      set_accent("#3FB8DB");

      assert_eq!(observed, (aurora.base, aurora.ink, aurora.muted, aurora.pressed));
      assert_eq!(accent(), accent::PLASMA);
    }
  }
}
