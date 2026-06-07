use iced::{
  Background, Border, Color, Element, Length, Padding, Point, Shadow, Vector,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text, text_input},
};

use crate::ui::{
  components::text_input as text_input_component,
  style::{color, spacing, typography},
};

const GRID_COLUMNS: usize = 6;
const GRID_GAP: f32 = 6.0;
const HEX_PREVIEW_RADIUS: f32 = 4.0;
const HEX_PREVIEW_SIZE: f32 = 18.0;
const HEX_WELL_RADIUS: f32 = 6.0;
const POPOVER_RADIUS: f32 = 10.0;
const POPOVER_WIDTH: f32 = 256.0;
const PRESET_RADIUS: f32 = 5.0;
const RING_ALPHA: f32 = 0.30;
const SWATCH_RADIUS: f32 = 7.0;
const SWATCH_SIZE: f32 = 28.0;

pub struct Preset {
  pub hex: &'static str,
  #[allow(dead_code)]
  pub name: &'static str,
}

pub const PALETTE: &[Preset] = &[
  Preset {
    hex: "#3FB8DB",
    name: "Plasma",
  },
  Preset {
    hex: "#5BB97E",
    name: "Jade",
  },
  Preset {
    hex: "#D9B252",
    name: "Gold",
  },
  Preset {
    hex: "#E07559",
    name: "Ember",
  },
  Preset {
    hex: "#E08AA5",
    name: "Coral",
  },
  Preset {
    hex: "#C07AD9",
    name: "Orchid",
  },
  Preset {
    hex: "#8A8FD9",
    name: "Violet",
  },
  Preset {
    hex: "#5BC9BC",
    name: "Cyan",
  },
  Preset {
    hex: "#A8C97A",
    name: "Lime",
  },
  Preset {
    hex: "#C97A5B",
    name: "Rust",
  },
  Preset {
    hex: "#8A95A6",
    name: "Slate",
  },
];

pub fn color_popover<'a, M>(
  current: Option<&str>,
  hex_draft: &'a str,
  hex_error: bool,
  on_select: impl Fn(String) -> M + 'a,
  on_hex_changed: impl Fn(String) -> M + 'a,
  on_hex_submit: M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let body = Column::with_children(vec![
    popover_header(),
    preset_grid(current, &on_select),
    hex_entry(hex_draft, hex_error, on_hex_changed, on_hex_submit),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fixed(POPOVER_WIDTH));

  popover_shell(body)
}

pub fn color_popover_with_clear<'a, M>(
  current: Option<&str>,
  hex_draft: &'a str,
  hex_error: bool,
  on_select: impl Fn(String) -> M + 'a,
  on_hex_changed: impl Fn(String) -> M + 'a,
  on_hex_submit: M,
  on_clear: M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let body = Column::with_children(vec![
    popover_header(),
    preset_grid(current, &on_select),
    hex_entry(hex_draft, hex_error, on_hex_changed, on_hex_submit),
    clear_rule(),
    clear_button(on_clear),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fixed(POPOVER_WIDTH));

  popover_shell(body)
}

pub fn color_swatch<'a, M>(current: Option<&str>, on_toggle: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let fill = current.and_then(hex_to_color).unwrap_or(Color::TRANSPARENT);
  let border_color = Color {
    a: 0.5,
    ..fill
  };

  let swatch = button(Space::new())
    .width(Length::Fixed(SWATCH_SIZE))
    .height(Length::Fixed(SWATCH_SIZE))
    .padding(Padding::ZERO)
    .on_press(on_toggle)
    .style(move |_, _| button::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: SWATCH_RADIUS.into(),
      },
      ..button::Style::default()
    });

  let caption = text(current.unwrap_or("").to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    });

  Row::with_children(vec![swatch.into(), caption.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .into()
}

pub fn floating<'a, M>(popover: Element<'a, M>, anchor: Point) -> Element<'a, M>
where
  M: 'a,
{
  container(popover)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: anchor.y.max(0.0),
      left: anchor.x.max(0.0),
      ..Padding::ZERO
    })
    .into()
}

pub fn normalize_hex(raw: &str) -> Option<String> {
  let trimmed = raw.trim().trim_start_matches('#');
  let expanded = match trimmed.len() {
    3 => trimmed.chars().flat_map(|c| [c, c]).collect::<String>(),
    6 => trimmed.to_string(),
    _ => return None,
  };
  if expanded.chars().all(|c| c.is_ascii_hexdigit()) {
    Some(format!("#{}", expanded.to_uppercase()))
  } else {
    None
  }
}

fn clear_button<'a, M>(on_clear: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(
    text("Clear color")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    right: spacing::SPACE_2_5,
    bottom: 7.0,
    left: spacing::SPACE_2_5,
  })
  .on_press(on_clear)
  .style(|_, status| {
    let (border_alpha, text_color) = match status {
      button::Status::Hovered | button::Status::Pressed => (0.18, color::text::PRIMARY),
      _ => (0.1, color::text::SECONDARY),
    };
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      text_color,
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, border_alpha),
        width: 1.0,
        radius: HEX_WELL_RADIUS.into(),
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn clear_rule<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    })
    .into()
}

fn hex_entry<'a, M>(
  hex_draft: &'a str,
  hex_error: bool,
  on_hex_changed: impl Fn(String) -> M + 'a,
  on_hex_submit: M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let label = text("HEX")
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::TERTIARY),
    });

  let input = text_input("#RRGGBB", hex_draft)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding::ZERO)
    .on_input(on_hex_changed)
    .on_submit(on_hex_submit)
    .style(text_input_component::inner_style());

  let preview_fill = hex_to_color(hex_draft).unwrap_or(Color::TRANSPARENT);
  let preview = container(Space::new())
    .width(Length::Fixed(HEX_PREVIEW_SIZE))
    .height(Length::Fixed(HEX_PREVIEW_SIZE))
    .style(move |_| container::Style {
      background: Some(Background::Color(preview_fill)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.18),
        width: 1.0,
        radius: HEX_PREVIEW_RADIUS.into(),
      },
      ..container::Style::default()
    });

  let well_border = if hex_error {
    color::status::DANGER
  } else {
    color::with_alpha(color::text::PRIMARY, 0.1)
  };

  container(
    Row::with_children(vec![label.into(), input.into(), preview.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 6.0,
    right: spacing::SPACE_2,
    bottom: 6.0,
    left: 10.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: well_border,
      width: 1.0,
      radius: HEX_WELL_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn hex_to_color(hex: &str) -> Option<Color> {
  let normalized = normalize_hex(hex)?;
  let digits = normalized.trim_start_matches('#');
  let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
  let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
  let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
  Some(Color::from_rgb8(r, g, b))
}

fn popover_header<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  text("PICK A COLOR")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()
}

fn popover_shell<'a, M>(body: Column<'a, M>) -> Element<'a, M>
where
  M: Clone + 'a,
{
  container(body)
    .width(Length::Fixed(POPOVER_WIDTH))
    .padding(spacing::SPACE_3)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.18),
        width: 1.0,
        radius: POPOVER_RADIUS.into(),
      },
      shadow: Shadow {
        color: color::with_alpha(Color::BLACK, 0.55),
        offset: Vector {
          x: 0.0,
          y: 16.0,
        },
        blur_radius: 40.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn preset_grid<'a, M>(current: Option<&str>, on_select: &impl Fn(String) -> M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let rows: Vec<Element<'a, M>> = PALETTE
    .chunks(GRID_COLUMNS)
    .map(|chunk| {
      let mut cells: Vec<Element<'a, M>> = chunk
        .iter()
        .map(|preset| preset_swatch(preset, current, on_select))
        .collect();
      while cells.len() < GRID_COLUMNS {
        cells.push(Space::new().width(Length::Fill).into());
      }
      Row::with_children(cells).spacing(GRID_GAP).width(Length::Fill).into()
    })
    .collect();

  Column::with_children(rows).spacing(GRID_GAP).width(Length::Fill).into()
}

fn preset_swatch<'a, M>(preset: &Preset, current: Option<&str>, on_select: &impl Fn(String) -> M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let fill = hex_to_color(preset.hex).unwrap_or(Color::TRANSPARENT);
  let selected = current.is_some_and(|c| c.eq_ignore_ascii_case(preset.hex));
  let border_color = if selected {
    color::accent::PLASMA
  } else {
    Color {
      a: 0.5,
      ..fill
    }
  };
  let ring = if selected {
    Shadow {
      color: color::with_alpha(color::accent::PLASMA, RING_ALPHA),
      offset: Vector::ZERO,
      blur_radius: 2.0,
    }
  } else {
    Shadow::default()
  };

  button(Space::new().width(Length::Fill).height(Length::Fixed(SWATCH_SIZE)))
    .width(Length::Fill)
    .height(Length::Fixed(SWATCH_SIZE))
    .padding(Padding::ZERO)
    .on_press(on_select(preset.hex.to_string()))
    .style(move |_, _| button::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: PRESET_RADIUS.into(),
      },
      shadow: ring,
      ..button::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod normalize_hex {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_normalizes_a_six_digit_hex_with_hash() {
      assert_eq!(normalize_hex("#3fb8db"), Some("#3FB8DB".to_string()));
    }

    #[test]
    fn it_normalizes_a_six_digit_hex_without_hash() {
      assert_eq!(normalize_hex("3fb8db"), Some("#3FB8DB".to_string()));
    }

    #[test]
    fn it_expands_three_digit_shorthand() {
      assert_eq!(normalize_hex("#abc"), Some("#AABBCC".to_string()));
      assert_eq!(normalize_hex("abc"), Some("#AABBCC".to_string()));
    }

    #[test]
    fn it_trims_surrounding_whitespace() {
      assert_eq!(normalize_hex("  #3fb8db  "), Some("#3FB8DB".to_string()));
    }

    #[test]
    fn it_rejects_empty_input() {
      assert_eq!(normalize_hex(""), None);
      assert_eq!(normalize_hex("   "), None);
      assert_eq!(normalize_hex("#"), None);
    }

    #[test]
    fn it_rejects_wrong_length() {
      assert_eq!(normalize_hex("12345"), None);
      assert_eq!(normalize_hex("1234567"), None);
    }

    #[test]
    fn it_rejects_non_hex_characters() {
      assert_eq!(normalize_hex("zzzzzz"), None);
      assert_eq!(normalize_hex("#12345g"), None);
    }
  }

  mod palette {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_holds_the_eleven_design_presets_plasma_first_slate_last() {
      assert_eq!(PALETTE.len(), 11);
      assert_eq!(PALETTE[0].name, "Plasma");
      assert_eq!(PALETTE[0].hex, "#3FB8DB");
      assert_eq!(PALETTE[10].name, "Slate");
      assert_eq!(PALETTE[10].hex, "#8A95A6");
    }

    #[test]
    fn every_preset_hex_parses_to_a_color() {
      for preset in PALETTE {
        assert!(
          hex_to_color(preset.hex).is_some(),
          "preset {} ({}) should parse",
          preset.name,
          preset.hex
        );
      }
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_swatch_with_a_selected_color_and_caption() {
      let _swatch: Element<'_, ()> = color_swatch(Some("#3FB8DB"), ());
    }

    #[test]
    fn it_renders_the_swatch_with_no_color() {
      let _swatch: Element<'_, ()> = color_swatch(None, ());
    }

    #[test]
    fn it_renders_the_popover_with_the_selected_ring() {
      let el: Element<'_, ()> = color_popover(Some("#3FB8DB"), "#3FB8DB", false, |_| (), |_| (), ());
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);
      assert!(
        !tree.children.is_empty(),
        "the popover should build a non-empty widget tree"
      );
    }

    #[test]
    fn it_renders_the_popover_with_a_hex_error_and_no_selection() {
      let _el: Element<'_, ()> = color_popover(None, "nothex", true, |_| (), |_| (), ());
    }

    #[test]
    fn it_renders_the_clear_variant_with_the_clear_button() {
      let el: Element<'_, ()> = color_popover_with_clear(Some("#5BB97E"), "#5BB97E", false, |_| (), |_| (), (), ());
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);
      assert!(
        !tree.children.is_empty(),
        "the clear-variant popover should build a non-empty widget tree"
      );
    }

    #[test]
    fn it_floats_a_popover_at_an_anchor_without_panicking() {
      let popover: Element<'_, ()> = color_popover(Some("#3FB8DB"), "#3FB8DB", false, |_| (), |_| (), ());
      let el: Element<'_, ()> = floating(popover, Point::new(42.0, 96.0));
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);
      assert!(
        !tree.children.is_empty(),
        "the floating layer should build a widget tree"
      );
    }
  }
}
