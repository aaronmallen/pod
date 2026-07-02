use iced::{
  Background, Border, Element, Length, Radians, Rectangle, Size,
  advanced::{
    Layout, Widget,
    layout::{self, Limits, Node},
    mouse, renderer, svg,
    widget::Tree,
  },
  alignment::{Horizontal, Vertical},
  widget::{Row, button, container, tooltip},
};

use super::{Message, Pane, ViewMode};
use crate::ui::style::{color, radius, spacing, typography};

const ACTIVE_BG_ALPHA: f32 = 0.14;

const BUTTON_HEIGHT: f32 = 28.0;

const BUTTON_RADIUS: f32 = 6.0;

const BUTTON_WIDTH: f32 = 34.0;

const GLYPH_SIZE: f32 = 15.0;

const TOGGLE_GAP: f32 = 2.0;

const TOGGLE_HEIGHT: f32 = 36.0;

const TOGGLE_PADDING: f32 = 3.0;

const TOGGLE_RADIUS: f32 = 8.0;

static CARDS_ICON: &[u8] = include_bytes!("../../../assets/images/icons/view-cards.svg");

static COMPACT_ICON: &[u8] = include_bytes!("../../../assets/images/icons/view-compact.svg");

static LIST_ICON: &[u8] = include_bytes!("../../../assets/images/icons/view-list.svg");

/// Draws the SVG using the *inherited* renderer style's text color rather than a fixed color, so the icon
/// automatically follows a parent button's active/hover tint without any explicit status wiring.
struct TintedIcon {
  handle: svg::Handle,
  size: f32,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TintedIcon
where
  Renderer: svg::Renderer,
{
  fn size(&self) -> Size<Length> {
    Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
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

impl<'a, Message, Theme, Renderer> From<TintedIcon> for Element<'a, Message, Theme, Renderer>
where
  Message: 'a,
  Theme: 'a,
  Renderer: svg::Renderer + 'a,
{
  fn from(icon: TintedIcon) -> Self {
    Element::new(icon)
  }
}

pub(super) fn toggle<'a>(active: ViewMode, pane: Pane) -> Element<'a, Message> {
  let cards = mode_button(
    CARDS_ICON,
    t!("roster.view_mode.cards").into_owned(),
    ViewMode::Cards,
    active,
    pane,
  );
  let compact = mode_button(
    COMPACT_ICON,
    t!("roster.view_mode.compact").into_owned(),
    ViewMode::Compact,
    active,
    pane,
  );
  let list = mode_button(
    LIST_ICON,
    t!("roster.view_mode.list").into_owned(),
    ViewMode::List,
    active,
    pane,
  );

  container(
    Row::with_children(vec![cards, compact, list])
      .spacing(TOGGLE_GAP)
      .align_y(Vertical::Center),
  )
  .height(Length::Fixed(TOGGLE_HEIGHT))
  .padding(TOGGLE_PADDING)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: TOGGLE_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn mode_button<'a>(
  glyph: &'static [u8],
  label: String,
  mode: ViewMode,
  active: ViewMode,
  pane: Pane,
) -> Element<'a, Message> {
  let selected = mode == active;
  let icon = container(TintedIcon {
    handle: svg::Handle::from_memory(glyph),
    size: GLYPH_SIZE,
  })
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center);

  let control = button(icon)
    .width(Length::Fixed(BUTTON_WIDTH))
    .height(Length::Fixed(BUTTON_HEIGHT))
    .padding(0)
    .on_press(Message::ViewModeChanged {
      mode,
      pane,
    })
    .style(move |_, status| mode_button_style(selected, status));

  tooltip(control, tooltip_body(label), tooltip::Position::Bottom)
    .gap(spacing::SPACE_2)
    .into()
}

fn mode_button_style(active: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let text_color = if active {
    color::accent()
  } else if hovered {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };

  button::Style {
    background: active.then(|| Background::Color(color::with_alpha(color::accent(), ACTIVE_BG_ALPHA))),
    text_color,
    border: Border {
      radius: BUTTON_RADIUS.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn tooltip_body<'a>(label: String) -> Element<'a, Message> {
  container(
    iced::widget::text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .padding(spacing::SPACE_2)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod toggle {
    use super::*;

    #[test]
    fn it_renders_the_three_mode_buttons_for_each_pane() {
      let _cards: Element<'_, Message> = toggle(ViewMode::Cards, Pane::Characters);
      let _compact: Element<'_, Message> = toggle(ViewMode::Compact, Pane::Corporations);
      let _list: Element<'_, Message> = toggle(ViewMode::List, Pane::Characters);
    }
  }

  mod mode_button_style {
    use super::*;

    #[test]
    fn it_fills_and_accents_the_active_button() {
      let style = mode_button_style(true, button::Status::Active);

      assert!(style.background.is_some());
      assert_eq!(style.text_color, color::accent());
    }

    #[test]
    fn it_leaves_an_inactive_idle_button_muted_and_transparent() {
      let style = mode_button_style(false, button::Status::Active);

      assert!(style.background.is_none());
      assert_eq!(style.text_color, color::text::secondary());
    }

    #[test]
    fn it_brightens_an_inactive_hovered_button_to_ink() {
      let style = mode_button_style(false, button::Status::Hovered);

      assert_eq!(style.text_color, color::text::PRIMARY);
    }
  }
}
