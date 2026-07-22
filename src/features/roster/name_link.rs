//! `Arrow` and `Underlined` have no hover state of their own; they read the hover/press tint from the
//! `renderer::Style::text_color` inherited from the enclosing `button`'s style function (set via `link_style`) and
//! compare it against the accent color to decide which tint to draw.

use iced::{
  Background, Border, Color, Element, Length, Radians, Rectangle, Shadow, Size,
  advanced::{
    Layout, Widget,
    layout::{self, Limits, Node},
    mouse, renderer, svg,
    widget::Tree,
  },
  alignment::Vertical,
  widget::{Row, button, text},
};

use crate::ui::{
  components::icon::Icon,
  style::{color, typography},
};

const ARROW_GAP: f32 = 4.0;
const ARROW_SCALE: f32 = 0.82;
const UNDERLINE_ACTIVE_ALPHA: f32 = 0.6;
const UNDERLINE_GAP: f32 = 2.0;
const UNDERLINE_HEIGHT: f32 = 1.0;
const UNDERLINE_REST_ALPHA: f32 = 0.22;

struct Arrow {
  active: Color,
  handle: svg::Handle,
  rest: Color,
  size: f32,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Arrow
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
    let color = tint(style.text_color, self.rest, self.active);

    renderer.draw_svg(
      svg::Svg {
        color: Some(color),
        handle: self.handle.clone(),
        opacity: 1.0,
        rotation: Radians(0.0),
      },
      layout.bounds(),
      *viewport,
    );
  }
}

impl<'a, Message, Theme, Renderer> From<Arrow> for Element<'a, Message, Theme, Renderer>
where
  Message: 'a,
  Theme: 'a,
  Renderer: svg::Renderer + 'a,
{
  fn from(arrow: Arrow) -> Self {
    Element::new(arrow)
  }
}

struct Underlined<'a, Message, Theme, Renderer> {
  active: Color,
  content: Element<'a, Message, Theme, Renderer>,
  gap: f32,
  rest: Color,
  thickness: f32,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Underlined<'_, Message, Theme, Renderer>
where
  Renderer: iced::advanced::Renderer,
{
  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.content)]
  }

  fn diff(&self, tree: &mut Tree) {
    tree.diff_children(&[&self.content]);
  }

  fn size(&self) -> Size<Length> {
    self.content.as_widget().size()
  }

  fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
    let content = self
      .content
      .as_widget_mut()
      .layout(&mut tree.children[0], renderer, limits);
    let size = content.size();
    let node_size = Size::new(size.width, size.height + self.gap + self.thickness);

    Node::with_children(node_size, vec![content])
  }

  fn draw(
    &self,
    tree: &Tree,
    renderer: &mut Renderer,
    theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    let content_layout = layout
      .children()
      .next()
      .expect("underlined label always has a child layout");
    self.content.as_widget().draw(
      &tree.children[0],
      renderer,
      theme,
      style,
      content_layout,
      cursor,
      viewport,
    );

    let bounds = content_layout.bounds();
    let color = tint(style.text_color, self.rest, self.active);

    renderer.fill_quad(
      renderer::Quad {
        bounds: Rectangle {
          x: bounds.x,
          y: bounds.y + bounds.height + self.gap,
          width: bounds.width,
          height: self.thickness,
        },
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
      },
      color,
    );
  }
}

impl<'a, Message, Theme, Renderer> From<Underlined<'a, Message, Theme, Renderer>>
  for Element<'a, Message, Theme, Renderer>
where
  Message: 'a,
  Theme: 'a,
  Renderer: iced::advanced::Renderer + 'a,
{
  fn from(underlined: Underlined<'a, Message, Theme, Renderer>) -> Self {
    Element::new(underlined)
  }
}

pub(super) fn name_link<'a, Message>(name: String, font_size: f32, on_press: Message) -> Element<'a, Message>
where
  Message: Clone + 'a,
{
  let arrow = Arrow {
    active: color::accent(),
    handle: Icon::arrow_out().handle(),
    rest: color::text::secondary(),
    size: (font_size * ARROW_SCALE).round(),
  };

  let label = text(name)
    .font(typography::body::MEDIUM)
    .size(font_size)
    .wrapping(text::Wrapping::None);

  let underlined = Underlined {
    active: color::with_alpha(color::accent(), UNDERLINE_ACTIVE_ALPHA),
    content: label.into(),
    gap: UNDERLINE_GAP,
    rest: color::with_alpha(color::text::PRIMARY, UNDERLINE_REST_ALPHA),
    thickness: UNDERLINE_HEIGHT,
  };

  let row = Row::with_children(vec![underlined.into(), arrow.into()])
    .spacing(ARROW_GAP)
    .align_y(Vertical::Center);

  button(row).padding(0).on_press(on_press).style(link_style).into()
}

fn link_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let text_color = match status {
    button::Status::Hovered | button::Status::Pressed => color::accent(),
    button::Status::Active | button::Status::Disabled => color::text::PRIMARY,
  };

  button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color,
    ..button::Style::default()
  }
}

fn tint(inherited: Color, rest: Color, active: Color) -> Color {
  if inherited == color::accent() { active } else { rest }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod name_link {
    use super::*;

    #[test]
    fn it_builds_a_navigable_name_link() {
      let _el: Element<'_, ()> = name_link("Test Pilot".to_owned(), typography::size::LG, ());
    }
  }

  mod tint {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_active_tint_when_the_inherited_color_is_the_accent() {
      let rest = color::text::secondary();
      let active = color::accent();

      assert_eq!(tint(color::accent(), rest, active), active);
    }

    #[test]
    fn it_falls_back_to_the_rest_tint_for_any_other_inherited_color() {
      let rest = color::text::secondary();
      let active = color::accent();

      assert_eq!(tint(color::text::PRIMARY, rest, active), rest);
    }
  }
}
