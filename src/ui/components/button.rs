use iced::{
  Background, Border, Color, Element, Length, Padding, Radians, Rectangle, Shadow, Size as IcedSize, Vector,
  advanced::{
    Layout, Widget,
    layout::{self, Limits, Node},
    mouse, renderer, svg,
    widget::Tree,
  },
  alignment::Vertical,
  widget::{Row, button, container, text},
};

use crate::ui::{
  components::icon::Icon,
  style::{color, typography},
};

const BORDER_WIDTH: f32 = 1.0;
const DANGER_GLOW_ALPHA: f32 = 0.22;
const DISABLED_ALPHA_SCALE: f32 = 0.4;
const GAP: f32 = 8.0;
const GHOST_HOVER_FILL_ALPHA: f32 = 0.08;
const GHOST_PRESS_FILL_ALPHA: f32 = 0.04;
const GLOW_BLUR: f32 = 18.0;
const IDLE_BORDER_ALPHA: f32 = 0.5;
const IDLE_FILL_ALPHA: f32 = 0.14;
const MONO_FONT_SIZE: f32 = 10.5;
const MONO_FONT_SIZE_SM: f32 = 9.5;
const SECONDARY_BORDER_ALPHA: f32 = 0.18;

pub struct Button<Message> {
  block: bool,
  height: Option<f32>,
  icon_only: bool,
  label: String,
  leading: Option<Icon>,
  mono: bool,
  on_press: Option<Message>,
  size: Size,
  trailing: Option<Icon>,
  variant: Variant,
}

impl<Message> Button<Message> {
  pub fn danger(label: impl Into<String>) -> Self {
    Self::labeled(Variant::Danger, label)
  }

  pub fn danger_icon(icon: Icon) -> Self {
    Self::icon_button(Variant::Danger, icon)
  }

  pub fn ghost(label: impl Into<String>) -> Self {
    Self::labeled(Variant::Ghost, label)
  }

  pub fn ghost_icon(icon: Icon) -> Self {
    Self::icon_button(Variant::Ghost, icon)
  }

  pub fn primary(label: impl Into<String>) -> Self {
    Self::labeled(Variant::Primary, label)
  }

  pub fn primary_icon(icon: Icon) -> Self {
    Self::icon_button(Variant::Primary, icon)
  }

  pub fn secondary(label: impl Into<String>) -> Self {
    Self::labeled(Variant::Secondary, label)
  }

  pub fn secondary_icon(icon: Icon) -> Self {
    Self::icon_button(Variant::Secondary, icon)
  }

  fn icon_button(variant: Variant, icon: Icon) -> Self {
    Self {
      block: false,
      height: None,
      icon_only: true,
      label: String::new(),
      leading: Some(icon),
      mono: false,
      on_press: None,
      size: Size::default(),
      trailing: None,
      variant,
    }
  }

  fn labeled(variant: Variant, label: impl Into<String>) -> Self {
    Self {
      block: false,
      height: None,
      icon_only: false,
      label: label.into(),
      leading: None,
      mono: false,
      on_press: None,
      size: Size::default(),
      trailing: None,
      variant,
    }
  }

  pub fn block(mut self) -> Self {
    self.block = true;
    self
  }

  pub fn height(mut self, height: f32) -> Self {
    self.height = Some(height);
    self
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.leading = Some(icon);
    self
  }

  pub fn icon_right(mut self, icon: Icon) -> Self {
    self.trailing = Some(icon);
    self
  }

  pub fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  pub fn on_press(mut self, message: Message) -> Self {
    self.on_press = Some(message);
    self
  }

  pub fn on_press_maybe(mut self, message: Option<Message>) -> Self {
    self.on_press = message;
    self
  }

  pub fn size(mut self, size: Size) -> Self {
    self.size = size;
    self
  }
}

impl<'a, Message> From<Button<Message>> for Element<'a, Message>
where
  Message: Clone + 'a,
{
  fn from(value: Button<Message>) -> Self {
    let block = value.block;
    let icon_only = value.icon_only;
    let metrics = value.size.metrics();
    let height = value.height.unwrap_or(metrics.height);
    let mono = value.mono;
    let radius = metrics.radius;
    let variant = value.variant;

    let mut children: Vec<Element<'a, Message>> = Vec::new();
    if let Some(icon) = value.leading {
      children.push(StatusTintedIcon::new(icon.handle(), metrics.icon).into());
    }
    if !icon_only && !value.label.is_empty() {
      children.push(label_element(value.label, mono, value.size, &metrics));
    }
    if let Some(icon) = value.trailing {
      children.push(StatusTintedIcon::new(icon.handle(), metrics.icon).into());
    }

    let inner = Row::with_children(children).spacing(GAP).align_y(Vertical::Center);
    let mut body = container(inner).center_y(Length::Fill);
    if block || icon_only {
      body = body.center_x(Length::Fill);
    }

    let padding = if icon_only {
      Padding::ZERO
    } else {
      Padding {
        top: 0.0,
        right: metrics.h_padding,
        bottom: 0.0,
        left: metrics.h_padding,
      }
    };

    let mut control = button(body)
      .padding(padding)
      .height(Length::Fixed(height))
      .on_press_maybe(value.on_press)
      .style(move |_theme, status| appearance(variant, radius, status));
    if icon_only {
      control = control.width(Length::Fixed(height));
    } else if block {
      control = control.width(Length::Fill);
    }

    control.into()
  }
}

fn label_element<'a, Message: 'a>(label: String, mono: bool, size: Size, metrics: &Metrics) -> Element<'a, Message> {
  let label = if mono { label.to_uppercase() } else { label };
  let font = if mono {
    typography::mono::MEDIUM
  } else {
    typography::body::MEDIUM
  };
  let font_size = if mono { size.mono_font_size() } else { metrics.font_size };
  text(label).font(font).size(font_size).into()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Size {
  Lg,
  #[default]
  Md,
  Sm,
}

impl Size {
  fn metrics(self) -> Metrics {
    match self {
      Size::Lg => Metrics {
        font_size: 15.0,
        h_padding: 24.0,
        height: 46.0,
        icon: 17.0,
        radius: 12.0,
      },
      Size::Md => Metrics {
        font_size: 13.5,
        h_padding: 18.0,
        height: 38.0,
        icon: 15.0,
        radius: 10.0,
      },
      Size::Sm => Metrics {
        font_size: 12.0,
        h_padding: 13.0,
        height: 30.0,
        icon: 13.0,
        radius: 8.0,
      },
    }
  }

  fn mono_font_size(self) -> f32 {
    match self {
      Size::Sm => MONO_FONT_SIZE_SM,
      _ => MONO_FONT_SIZE,
    }
  }
}

struct Metrics {
  font_size: f32,
  h_padding: f32,
  height: f32,
  icon: f32,
  radius: f32,
}

struct Palette {
  background: Color,
  border: Color,
  foreground: Color,
  glow: Option<Color>,
}

impl Palette {
  fn dimmed(self) -> Self {
    Palette {
      background: scale_alpha(self.background, DISABLED_ALPHA_SCALE),
      border: scale_alpha(self.border, DISABLED_ALPHA_SCALE),
      foreground: scale_alpha(self.foreground, DISABLED_ALPHA_SCALE),
      glow: None,
    }
  }
}

struct StatusTintedIcon {
  handle: svg::Handle,
  size: f32,
}

impl StatusTintedIcon {
  fn new(handle: svg::Handle, size: f32) -> Self {
    Self {
      handle,
      size,
    }
  }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for StatusTintedIcon
where
  Renderer: svg::Renderer,
{
  fn size(&self) -> IcedSize<Length> {
    IcedSize::new(Length::Fixed(self.size), Length::Fixed(self.size))
  }

  fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
    layout::atomic(limits, Length::Fixed(self.size), Length::Fixed(self.size))
  }

  fn draw(
    &self,
    _tree: &Tree,
    renderer: &mut Renderer,
    _theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    _cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    renderer.draw_svg(
      svg::Svg {
        color: Some(style.text_color),
        handle: self.handle.clone(),
        opacity: 1.0,
        rotation: Radians(0.0),
      },
      layout.bounds(),
      *viewport,
    );
  }
}

impl<'a, Message, Theme, Renderer> From<StatusTintedIcon> for Element<'a, Message, Theme, Renderer>
where
  Message: 'a,
  Theme: 'a,
  Renderer: svg::Renderer + 'a,
{
  fn from(icon: StatusTintedIcon) -> Self {
    Element::new(icon)
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Variant {
  Danger,
  Ghost,
  Primary,
  Secondary,
}

impl Variant {
  fn hovered(self) -> Palette {
    match self {
      Variant::Danger => Palette {
        background: color::status::DANGER,
        border: color::status::DANGER,
        foreground: color::status::DANGER_INK,
        glow: Some(color::with_alpha(color::status::DANGER, DANGER_GLOW_ALPHA)),
      },
      Variant::Ghost => Palette {
        background: color::with_alpha(color::accent(), GHOST_HOVER_FILL_ALPHA),
        border: Color::TRANSPARENT,
        foreground: color::accent(),
        glow: None,
      },
      Variant::Primary => Palette {
        background: color::accent(),
        border: color::accent(),
        foreground: color::accent_ink(),
        glow: Some(color::accent_muted()),
      },
      Variant::Secondary => Palette {
        background: Color::TRANSPARENT,
        border: color::accent(),
        foreground: color::accent(),
        glow: None,
      },
    }
  }

  fn idle(self) -> Palette {
    match self {
      Variant::Danger => Palette {
        background: color::with_alpha(color::status::DANGER, IDLE_FILL_ALPHA),
        border: color::with_alpha(color::status::DANGER, IDLE_BORDER_ALPHA),
        foreground: color::status::DANGER,
        glow: None,
      },
      Variant::Ghost => Palette {
        background: Color::TRANSPARENT,
        border: Color::TRANSPARENT,
        foreground: color::text::secondary_off(),
        glow: None,
      },
      Variant::Primary => Palette {
        background: color::with_alpha(color::accent(), IDLE_FILL_ALPHA),
        border: color::with_alpha(color::accent(), IDLE_BORDER_ALPHA),
        foreground: color::accent(),
        glow: None,
      },
      Variant::Secondary => Palette {
        background: Color::TRANSPARENT,
        border: color::with_alpha(color::text::PRIMARY, SECONDARY_BORDER_ALPHA),
        foreground: color::text::PRIMARY,
        glow: None,
      },
    }
  }

  fn palette(self, status: button::Status) -> Palette {
    match status {
      button::Status::Active => self.idle(),
      button::Status::Disabled => self.idle().dimmed(),
      button::Status::Hovered => self.hovered(),
      button::Status::Pressed => self.pressed(),
    }
  }

  fn pressed(self) -> Palette {
    match self {
      Variant::Danger => Palette {
        background: color::status::DANGER_PRESSED,
        border: color::status::DANGER_PRESSED,
        foreground: color::status::DANGER_INK,
        glow: None,
      },
      Variant::Ghost => Palette {
        background: color::with_alpha(color::accent(), GHOST_PRESS_FILL_ALPHA),
        border: Color::TRANSPARENT,
        foreground: color::accent(),
        glow: None,
      },
      Variant::Primary => Palette {
        background: color::accent_pressed(),
        border: color::accent_pressed(),
        foreground: color::accent_ink(),
        glow: None,
      },
      Variant::Secondary => self.hovered(),
    }
  }
}

fn appearance(variant: Variant, radius: f32, status: button::Status) -> button::Style {
  let palette = variant.palette(status);
  let shadow = match palette.glow {
    Some(color) => Shadow {
      blur_radius: GLOW_BLUR,
      color,
      offset: Vector::ZERO,
    },
    None => Shadow::default(),
  };

  button::Style {
    background: Some(Background::Color(palette.background)),
    border: Border {
      color: palette.border,
      radius: radius.into(),
      width: BORDER_WIDTH,
    },
    shadow,
    text_color: palette.foreground,
    ..button::Style::default()
  }
}

fn scale_alpha(base: Color, scale: f32) -> Color {
  color::with_alpha(base, base.a * scale)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod appearance {
    use iced::{Background, widget::button};
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ui::style::color;

    #[test]
    fn it_renders_primary_idle_as_tinted_glass() {
      let style = super::appearance(Variant::Primary, 10.0, button::Status::Active);

      assert_eq!(style.text_color, color::accent());
      assert_eq!(
        style.background,
        Some(Background::Color(color::with_alpha(color::accent(), 0.14)))
      );
      assert_eq!(style.border.color, color::with_alpha(color::accent(), 0.5));
      assert_eq!(style.shadow.blur_radius, 0.0);
    }

    #[test]
    fn it_fills_primary_to_plasma_with_dark_ink_and_glow_on_hover() {
      let style = super::appearance(Variant::Primary, 10.0, button::Status::Hovered);

      assert_eq!(style.background, Some(Background::Color(color::accent())));
      assert_eq!(style.text_color, color::accent_ink());
      assert_eq!(style.shadow.color, color::accent_muted());
      assert_eq!(style.shadow.blur_radius, GLOW_BLUR);
    }

    #[test]
    fn it_darkens_primary_on_press_without_glow() {
      let style = super::appearance(Variant::Primary, 10.0, button::Status::Pressed);

      assert_eq!(style.background, Some(Background::Color(color::accent_pressed())));
      assert_eq!(style.text_color, color::accent_ink());
      assert_eq!(style.shadow.blur_radius, 0.0);
    }

    #[test]
    fn it_premultiplies_disabled_alpha_by_the_disabled_scale() {
      let style = super::appearance(Variant::Primary, 10.0, button::Status::Disabled);

      assert_eq!(style.text_color, color::with_alpha(color::accent(), 0.4));
      assert_eq!(
        style.background,
        Some(Background::Color(color::with_alpha(color::accent(), 0.14 * 0.4)))
      );
      assert_eq!(style.shadow.blur_radius, 0.0);
    }

    #[test]
    fn it_fills_danger_to_warning_hue_with_dark_ink_on_hover() {
      let style = super::appearance(Variant::Danger, 10.0, button::Status::Hovered);

      assert_eq!(style.background, Some(Background::Color(color::status::DANGER)));
      assert_eq!(style.text_color, color::status::DANGER_INK);
      assert_eq!(
        style.shadow.color,
        color::with_alpha(color::status::DANGER, DANGER_GLOW_ALPHA)
      );
    }

    #[test]
    fn it_keeps_secondary_pressed_identical_to_hover() {
      let hovered = super::appearance(Variant::Secondary, 10.0, button::Status::Hovered);
      let pressed = super::appearance(Variant::Secondary, 10.0, button::Status::Pressed);

      assert_eq!(hovered, pressed);
    }

    #[test]
    fn it_renders_ghost_idle_borderless_at_the_dimmed_ink() {
      let style = super::appearance(Variant::Ghost, 8.0, button::Status::Active);

      assert_eq!(style.text_color, color::text::secondary_off());
      assert_eq!(style.background, Some(Background::Color(Color::TRANSPARENT)));
      assert_eq!(style.border.color, Color::TRANSPARENT);
    }

    #[test]
    fn it_carries_the_size_radius_onto_the_border() {
      let style = super::appearance(Variant::Primary, 12.0, button::Status::Active);

      assert_eq!(style.border.radius, 12.0.into());
    }
  }

  mod size {
    use super::*;

    mod metrics {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_maps_the_medium_default_to_the_locked_metrics() {
        let metrics = Size::Md.metrics();

        assert_eq!(metrics.font_size, 13.5);
        assert_eq!(metrics.h_padding, 18.0);
        assert_eq!(metrics.height, 38.0);
        assert_eq!(metrics.icon, 15.0);
        assert_eq!(metrics.radius, 10.0);
      }

      #[test]
      fn it_maps_small_to_the_compact_metrics() {
        let metrics = Size::Sm.metrics();

        assert_eq!(metrics.font_size, 12.0);
        assert_eq!(metrics.h_padding, 13.0);
        assert_eq!(metrics.height, 30.0);
        assert_eq!(metrics.icon, 13.0);
        assert_eq!(metrics.radius, 8.0);
      }

      #[test]
      fn it_maps_large_to_the_roomy_metrics() {
        let metrics = Size::Lg.metrics();

        assert_eq!(metrics.font_size, 15.0);
        assert_eq!(metrics.h_padding, 24.0);
        assert_eq!(metrics.height, 46.0);
        assert_eq!(metrics.icon, 17.0);
        assert_eq!(metrics.radius, 12.0);
      }
    }

    mod mono_font_size {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_shrinks_the_mono_label_only_at_the_small_size() {
        assert_eq!(Size::Sm.mono_font_size(), 9.5);
        assert_eq!(Size::Md.mono_font_size(), 10.5);
        assert_eq!(Size::Lg.mono_font_size(), 10.5);
      }
    }

    mod default {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_defaults_to_medium() {
        assert_eq!(Size::default(), Size::Md);
      }
    }
  }

  mod button {
    use super::*;
    use crate::ui::components::icon::Icon;

    #[test]
    fn it_builds_every_variant_with_a_press_message() {
      let _danger: Element<'_, ()> = Button::danger("Delete").on_press(()).into();
      let _ghost: Element<'_, ()> = Button::ghost("Cancel").on_press(()).into();
      let _primary: Element<'_, ()> = Button::primary("Save").on_press(()).into();
      let _secondary: Element<'_, ()> = Button::secondary("Export").on_press(()).into();
    }

    #[test]
    fn it_builds_icon_bearing_and_icon_only_buttons() {
      let _leading: Element<'_, ()> = Button::primary("Save").icon(Icon::plus()).on_press(()).into();
      let _trailing: Element<'_, ()> = Button::primary("Continue")
        .icon_right(Icon::forward())
        .on_press(())
        .into();
      let _only: Element<'_, ()> = Button::primary_icon(Icon::plus()).on_press(()).into();
    }

    #[test]
    fn it_builds_mono_and_block_modifiers_across_sizes() {
      let _mono: Element<'_, ()> = Button::ghost("Buy").mono().size(Size::Sm).on_press(()).into();
      let _block: Element<'_, ()> = Button::primary("Apply").block().size(Size::Lg).on_press(()).into();
    }

    #[test]
    fn it_builds_a_disabled_button_from_a_none_press() {
      let _disabled: Element<'_, ()> = Button::primary("Apply").on_press_maybe(None).into();
    }

    #[test]
    fn it_builds_with_an_overridden_height() {
      let _sized: Element<'_, ()> = Button::secondary("Compare")
        .size(Size::Sm)
        .height(36.0)
        .on_press(())
        .into();
      let _icon: Element<'_, ()> = Button::secondary_icon(Icon::plus()).height(36.0).on_press(()).into();
    }
  }
}
