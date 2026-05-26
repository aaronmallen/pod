//! Reusable color-picker component built on [`PopOver`].
//!
//! Renders a color swatch button that toggles a palette popover when
//! clicked. The palette mirrors the tag-color palette defined in the
//! settings view.

use iced::{
  Background, Border, Element, Length, Padding,
  border::Radius,
  widget::{Space, button, column, container, row, text, text_input},
};

use crate::{
  components::PopOver,
  style::{color, radius, spacing, typography},
};

/// Tag color palette — (name, hex) pairs.
pub const PALETTE: &[(&str, &str)] = &[
  ("Plasma", "#3FB8DB"),
  ("Jade", "#5BB97E"),
  ("Gold", "#D9B252"),
  ("Ember", "#E07559"),
  ("Coral", "#E08AA5"),
  ("Orchid", "#C07AD9"),
  ("Violet", "#8A8FD9"),
  ("Cyan", "#5BC9BC"),
  ("Lime", "#A8C97A"),
  ("Rust", "#C97A5B"),
  ("Slate", "#8A95A6"),
];

/// Builder for the color-picker component.
///
/// Renders a colored swatch button that, when `is_open` is true,
/// overlays a palette grid popover via [`PopOver`].
pub struct Component<'a, Message> {
  /// The currently selected hex color (e.g. `"#3FB8DB"`), or an empty
  /// string when no color is assigned.
  pub current_color: &'a str,
  /// Current draft text in the hex input field, or an empty string.
  pub hex_draft: &'a str,
  /// Whether the hex input should render with an error border.
  pub hex_error: bool,
  /// Whether the palette popover is currently visible.
  pub is_open: bool,
  /// Called with each keystroke in the hex input field.
  pub on_hex_changed: Option<Box<dyn Fn(String) -> Message + 'a>>,
  /// Sent when the user presses Enter in the hex input field.
  pub on_hex_submit: Option<Message>,
  /// Called with the hex string of the chosen palette swatch.
  pub on_select: Box<dyn Fn(String) -> Message + 'a>,
  /// Sent when the anchor swatch button is pressed (toggles open/close).
  pub on_toggle: Message,
}

impl<'a, Message: Clone + 'static> Component<'a, Message> {
  /// Create a new color-picker builder.
  pub fn new(
    current_color: &'a str,
    is_open: bool,
    on_select: impl Fn(String) -> Message + 'a,
    on_toggle: Message,
  ) -> Self {
    Self {
      current_color,
      hex_draft: "",
      hex_error: false,
      is_open,
      on_hex_changed: None,
      on_hex_submit: None,
      on_select: Box::new(on_select),
      on_toggle,
    }
  }

  /// Set the draft text shown in the hex input field.
  pub fn hex_draft(mut self, draft: &'a str) -> Self {
    self.hex_draft = draft;
    self
  }

  /// Set whether the hex input renders with an error border.
  pub fn hex_error(mut self, error: bool) -> Self {
    self.hex_error = error;
    self
  }

  /// Set the callback fired on each keystroke in the hex input.
  pub fn on_hex_changed(mut self, f: impl Fn(String) -> Message + 'a) -> Self {
    self.on_hex_changed = Some(Box::new(f));
    self
  }

  /// Set the message sent when the user presses Enter in the hex input.
  pub fn on_hex_submit(mut self, msg: Message) -> Self {
    self.on_hex_submit = Some(msg);
    self
  }

  /// Consume the builder and return the finished [`Element`].
  ///
  /// When `is_open` is `false`, only the swatch anchor button is
  /// rendered. When `is_open` is `true`, a [`PopOver`] palette panel
  /// is stacked below the anchor.
  pub fn render(self) -> Element<'a, Message> {
    let Self {
      current_color,
      hex_draft,
      hex_error,
      is_open,
      on_hex_changed,
      on_hex_submit,
      on_select,
      on_toggle,
    } = self;

    let swatch_color = hex_to_iced_color(current_color).unwrap_or(color::state::TAG_FILL);
    let has_color = !current_color.is_empty();

    let anchor = button(Space::new().width(Length::Fill).height(Length::Fill))
      .padding(Padding::ZERO)
      .width(22.0)
      .height(22.0)
      .on_press(on_toggle)
      .style(move |_, status| swatch_anchor_style(is_open, has_color, swatch_color, status));

    if !is_open {
      return anchor.into();
    }

    let popover_body = palette_body(
      on_select,
      current_color,
      hex_draft,
      hex_error,
      on_hex_changed,
      on_hex_submit,
    );
    let panel = PopOver::new(popover_body).width(256.0).render();

    column([
      anchor.into(),
      container(panel)
        .padding(Padding {
          top: 4.0,
          bottom: 0.0,
          left: 0.0,
          right: 0.0,
        })
        .into(),
    ])
    .spacing(0.0)
    .into()
  }
}

/// Local state for the hex input field in the color-picker popover.
///
/// The caller holds this and passes `hex_draft`/`hex_error` into
/// [`Component`] via the builder methods.
#[derive(Clone, Debug, Default)]
pub struct State {
  /// The text currently typed in the hex input field.
  pub hex_draft: String,
  /// Whether the last submitted hex value was invalid.
  pub hex_error: bool,
}

impl State {
  /// Clear the draft and error state (call when the color is cleared).
  pub fn clear(&mut self) {
    self.hex_draft.clear();
    self.hex_error = false;
  }

  /// Pre-fill the draft from an existing color when the picker opens.
  pub fn open(&mut self, current_color: &str) {
    self.hex_draft = current_color.trim_start_matches('#').to_uppercase();
    self.hex_error = false;
  }

  /// Sync the draft after a palette swatch or hex submit is accepted.
  pub fn set_from_selection(&mut self, hex: &str) {
    self.hex_draft = hex.trim_start_matches('#').to_uppercase();
    self.hex_error = false;
  }
}

/// Normalize a raw hex input string into a canonical `"#RRGGBB"` form.
///
/// Accepts 3-character shorthand (`"abc"` → `"#AABBCC"`) or full
/// 6-character values, with or without a leading `#`. Returns `None`
/// if the input is not a valid hex color.
pub fn normalize_hex(input: &str) -> Option<String> {
  let s = input.trim().trim_start_matches('#');
  let expanded = match s.len() {
    3 => {
      let mut chars = s.chars();
      let (a, b, c) = (chars.next()?, chars.next()?, chars.next()?);
      format!("{a}{a}{b}{b}{c}{c}")
    }
    6 => s.to_string(),
    _ => return None,
  };
  if expanded.chars().all(|c| c.is_ascii_hexdigit()) {
    Some(format!("#{}", expanded.to_uppercase()))
  } else {
    None
  }
}

fn clear_button<Message: Clone + 'static>(msg: Message) -> Element<'static, Message> {
  button(
    text("Clear color")
      .font(typography::body::REGULAR)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
      Some(Background::Color(color::state::HOVER_OVERLAY))
    } else {
      None
    },
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::CHIP.into(),
      width: 1.0,
    },
    snap: false,
    text_color: color::text::SECONDARY,
    shadow: iced::Shadow::default(),
  })
  .into()
}

fn hex_input_label() -> iced::widget::Text<'static, iced::Theme> {
  text("HEX CODE")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
}

fn hex_input_row<'a, Message: Clone + 'static>(
  hex_draft: &'a str,
  hex_error: bool,
  on_hex_changed: Box<dyn Fn(String) -> Message + 'a>,
  on_hex_submit: Message,
) -> Element<'a, Message> {
  let input = text_input("#3FB8DB", hex_draft)
    .font(typography::mono::REGULAR)
    .size(12.0)
    .padding(Padding {
      top: 5.0,
      bottom: 5.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_input(on_hex_changed)
    .on_submit(on_hex_submit)
    .style(move |_, _| text_input::Style {
      background: Background::Color(color::surface::SUNKEN),
      border: Border {
        color: if hex_error {
          color::status::DANGER_BORDER
        } else {
          color::border::DEFAULT
        },
        radius: radius::CHIP.into(),
        width: if hex_error { 1.5 } else { 1.0 },
      },
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      selection: color::state::SELECTION,
      value: color::text::PRIMARY,
    });

  column([hex_input_label().into(), Space::new().height(4.0).into(), input.into()])
    .spacing(0.0)
    .into()
}

fn hex_to_iced_color(hex: &str) -> Option<iced::Color> {
  let hex = hex.trim_start_matches('#');
  if hex.len() != 6 {
    return None;
  }
  let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
  let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
  let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
  Some(iced::Color {
    r,
    g,
    b,
    a: 1.0,
  })
}

fn palette_body<'a, Message: Clone + 'static>(
  on_select: Box<dyn Fn(String) -> Message + 'a>,
  current_hex: &'a str,
  hex_draft: &'a str,
  hex_error: bool,
  on_hex_changed: Option<Box<dyn Fn(String) -> Message + 'a>>,
  on_hex_submit: Option<Message>,
) -> Element<'a, Message> {
  let swatches: Vec<Element<'static, Message>> = PALETTE
    .iter()
    .map(|&(_name, hex)| {
      let Some(swatch_color) = hex_to_iced_color(hex) else {
        return Space::new().width(30.0).height(30.0).into();
      };
      let is_selected = current_hex == hex;
      let msg = on_select(hex.to_string());
      swatch_button(swatch_color, is_selected, msg)
    })
    .collect();

  let clear_msg = on_select(String::new());

  let mut children: Vec<Element<'a, Message>> = vec![
    palette_header().into(),
    Space::new().height(10.0).into(),
    row(swatches).spacing(6.0).wrap().into(),
  ];

  if let (Some(on_changed), Some(on_submit)) = (on_hex_changed, on_hex_submit) {
    children.push(Space::new().height(8.0).into());
    children.push(hex_input_row(hex_draft, hex_error, on_changed, on_submit));
  }

  children.push(Space::new().height(12.0).into());
  children.push(palette_divider());
  children.push(Space::new().height(10.0).into());
  children.push(clear_button(clear_msg));

  container(column(children).spacing(0.0)).padding(12.0).into()
}

fn palette_divider<'a, Message: 'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn palette_header() -> iced::widget::Text<'static, iced::Theme> {
  text("PICK A COLOR")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
}

fn swatch_anchor_border_color(
  is_open: bool,
  has_color: bool,
  swatch_color: iced::Color,
  status: button::Status,
) -> iced::Color {
  if is_open {
    color::accent::PLASMA
  } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
    color::accent::PLASMA_MUTED
  } else if has_color {
    iced::Color {
      a: 0.5,
      ..swatch_color
    }
  } else {
    color::border::SUBTLE
  }
}

fn swatch_anchor_style(
  is_open: bool,
  has_color: bool,
  swatch_color: iced::Color,
  status: button::Status,
) -> button::Style {
  button::Style {
    background: Some(Background::Color(swatch_color)),
    border: Border {
      color: swatch_anchor_border_color(is_open, has_color, swatch_color, status),
      radius: radius::CHIP.into(),
      width: if is_open { 2.0 } else { 1.0 },
    },
    snap: false,
    text_color: iced::Color::TRANSPARENT,
    shadow: iced::Shadow::default(),
  }
}

fn swatch_border_color(swatch_color: iced::Color, is_selected: bool, status: button::Status) -> iced::Color {
  if is_selected {
    color::accent::PLASMA
  } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
    iced::Color {
      a: 0.8,
      ..swatch_color
    }
  } else {
    iced::Color {
      a: 0.5,
      ..swatch_color
    }
  }
}

fn swatch_button<Message: Clone + 'static>(
  swatch_color: iced::Color,
  is_selected: bool,
  msg: Message,
) -> Element<'static, Message> {
  button(Space::new().width(Length::Fill).height(Length::Fill))
    .padding(Padding::ZERO)
    .width(30.0)
    .height(30.0)
    .on_press(msg)
    .style(move |_, status| button::Style {
      background: Some(Background::Color(swatch_color)),
      border: Border {
        color: swatch_border_color(swatch_color, is_selected, status),
        radius: Radius::from(5.0),
        width: if is_selected { 2.0 } else { 1.0 },
      },
      shadow: swatch_shadow(is_selected),
      snap: false,
      text_color: iced::Color::TRANSPARENT,
    })
    .into()
}

fn swatch_shadow(is_selected: bool) -> iced::Shadow {
  if is_selected {
    iced::Shadow {
      color: iced::Color {
        a: 0.3,
        ..color::accent::PLASMA
      },
      offset: iced::Vector::ZERO,
      blur_radius: 4.0,
    }
  } else {
    iced::Shadow::default()
  }
}

#[cfg(test)]
mod tests {
  mod normalize_hex {
    use pretty_assertions::assert_eq;

    use super::super::normalize_hex;

    #[test]
    fn it_accepts_six_char_lowercase_without_hash() {
      assert_eq!(normalize_hex("3fbcd8"), Some("#3FBCD8".to_string()));
    }

    #[test]
    fn it_accepts_six_char_with_hash() {
      assert_eq!(normalize_hex("#3fbcd8"), Some("#3FBCD8".to_string()));
    }

    #[test]
    fn it_uppercases_the_result() {
      assert_eq!(normalize_hex("aabbcc"), Some("#AABBCC".to_string()));
    }

    #[test]
    fn it_expands_three_char_shorthand() {
      assert_eq!(normalize_hex("#abc"), Some("#AABBCC".to_string()));
    }

    #[test]
    fn it_expands_three_char_without_hash() {
      assert_eq!(normalize_hex("abc"), Some("#AABBCC".to_string()));
    }

    #[test]
    fn it_returns_none_for_invalid_chars() {
      assert_eq!(normalize_hex("zzzzzz"), None);
    }

    #[test]
    fn it_returns_none_for_wrong_length() {
      assert_eq!(normalize_hex("12345"), None);
      assert_eq!(normalize_hex("1234567"), None);
    }

    #[test]
    fn it_returns_none_for_empty_string() {
      assert_eq!(normalize_hex(""), None);
    }

    #[test]
    fn it_trims_whitespace() {
      assert_eq!(normalize_hex("  3fbcd8  "), Some("#3FBCD8".to_string()));
    }
  }

  mod state {
    use super::super::State;

    mod open {
      use pretty_assertions::assert_eq;

      use super::State;

      #[test]
      fn it_prefills_draft_without_hash_uppercased() {
        let mut s = State::default();
        s.open("#3fb8db");

        assert_eq!(s.hex_draft, "3FB8DB");
        assert!(!s.hex_error);
      }

      #[test]
      fn it_clears_error_flag() {
        let mut s = State {
          hex_draft: "bad".into(),
          hex_error: true,
        };
        s.open("#3fb8db");

        assert!(!s.hex_error);
      }
    }

    mod set_from_selection {
      use pretty_assertions::assert_eq;

      use super::State;

      #[test]
      fn it_syncs_draft_from_hex() {
        let mut s = State::default();
        s.set_from_selection("#5BB97E");

        assert_eq!(s.hex_draft, "5BB97E");
        assert!(!s.hex_error);
      }
    }

    mod clear {
      use super::State;

      #[test]
      fn it_empties_draft_and_clears_error() {
        let mut s = State {
          hex_draft: "AABBCC".into(),
          hex_error: true,
        };
        s.clear();

        assert!(s.hex_draft.is_empty());
        assert!(!s.hex_error);
      }
    }
  }
}
