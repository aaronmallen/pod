use iced::{
  Background, Border, Element, Length, Padding,
  widget::{column, container},
};

use crate::{
  components::Separator,
  style::{color, radius, shadow},
};

pub struct Component<'a, Message> {
  header: Option<Element<'a, Message>>,
  body: Element<'a, Message>,
  footer: Option<Element<'a, Message>>,
  width: Option<Length>,
  max_height: Option<Length>,
  x: f32,
  y: f32,
}

fn build_parts<'a, Message: 'static>(
  header: Option<Element<'a, Message>>,
  body: Element<'a, Message>,
  footer: Option<Element<'a, Message>>,
) -> Vec<Element<'a, Message>> {
  let mut parts: Vec<Element<'a, Message>> = Vec::new();
  if let Some(h) = header {
    parts.push(h);
    parts.push(Separator::horizontal().render());
  }
  parts.push(body);
  if let Some(f) = footer {
    parts.push(Separator::horizontal().render());
    parts.push(f);
  }
  parts
}

fn size_panel<'a, Message: 'static>(
  mut panel: container::Container<'a, Message>,
  width: Option<Length>,
  max_height: Option<Length>,
) -> container::Container<'a, Message> {
  if let Some(w) = width {
    panel = panel.width(w);
  }
  if let Some(Length::Fixed(h)) = max_height {
    panel = panel.max_height(h);
  }
  panel
}

fn wrap_position<'a, Message: 'static>(
  panel: container::Container<'a, Message>,
  x: f32,
  y: f32,
) -> Element<'a, Message> {
  if x == 0.0 && y == 0.0 {
    return panel.into();
  }
  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: y,
      left: x,
      ..Padding::ZERO
    })
    .into()
}

impl<'a, Message: 'static> Component<'a, Message> {
  pub fn new(body: impl Into<Element<'a, Message>>) -> Self {
    Self {
      header: None,
      body: body.into(),
      footer: None,
      width: None,
      max_height: None,
      x: 0.0,
      y: 0.0,
    }
  }

  pub fn header(mut self, header: impl Into<Element<'a, Message>>) -> Self {
    self.header = Some(header.into());
    self
  }

  pub fn footer(mut self, footer: impl Into<Element<'a, Message>>) -> Self {
    self.footer = Some(footer.into());
    self
  }

  pub fn width(mut self, width: impl Into<Length>) -> Self {
    self.width = Some(width.into());
    self
  }

  pub fn max_height(mut self, max_height: impl Into<Length>) -> Self {
    self.max_height = Some(max_height.into());
    self
  }

  pub fn position(mut self, x: f32, y: f32) -> Self {
    self.x = x;
    self.y = y;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let parts = build_parts(self.header, self.body, self.footer);
    let panel = size_panel(
      container(column(parts)).style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::DEFAULT,
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        shadow: shadow::POPOVER,
        ..container::Style::default()
      }),
      self.width,
      self.max_height,
    );
    wrap_position(panel, self.x, self.y)
  }
}
