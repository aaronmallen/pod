//! Reusable color-picker component built on [`PopOver`].
//!
//! Renders a color swatch button that toggles a palette popover when
//! clicked. The palette mirrors the tag-color palette defined in the
//! settings view.

use iced::{
  Background, Border, Element, Length, Padding,
  border::Radius,
  widget::{Space, button, column, container, row, text},
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
  /// Whether the palette popover is currently visible.
  pub is_open: bool,
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
      is_open,
      on_select: Box::new(on_select),
      on_toggle,
    }
  }

  /// Consume the builder and return the finished [`Element`].
  ///
  /// When `is_open` is `false`, only the swatch anchor button is
  /// rendered. When `is_open` is `true`, a [`PopOver`] palette panel
  /// is stacked below the anchor.
  pub fn render(self) -> Element<'a, Message> {
    let Self {
      current_color,
      is_open,
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
      .style(move |_, status| button::Style {
        background: Some(Background::Color(swatch_color)),
        border: Border {
          color: if is_open {
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
          },
          radius: radius::CHIP.into(),
          width: if is_open { 2.0 } else { 1.0 },
        },
        snap: false,
        text_color: iced::Color::TRANSPARENT,
        shadow: iced::Shadow::default(),
      });

    if !is_open {
      return anchor.into();
    }

    let popover_body = palette_body(on_select, current_color);
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
  container(
    column([
      palette_header().into(),
      Space::new().height(10.0).into(),
      row(swatches).spacing(6.0).wrap().into(),
      Space::new().height(12.0).into(),
      palette_divider().into(),
      Space::new().height(10.0).into(),
      clear_button(clear_msg).into(),
    ])
    .spacing(0.0),
  )
  .padding(12.0)
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
        color: if is_selected {
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
        },
        radius: Radius::from(5.0),
        width: if is_selected { 2.0 } else { 1.0 },
      },
      shadow: if is_selected {
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
      },
      snap: false,
      text_color: iced::Color::TRANSPARENT,
    })
    .into()
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
