use iced::{Background, Border, Element, Length, Padding, Shadow, Vector, widget::container};

use crate::style::{color, radius};

/// Floating panel surface container.
pub struct Component<'a, MSG> {
  content: Element<'a, MSG>,
  padding: Padding,
  width: Option<Length>,
  height: Option<Length>,
  max_height: Option<f32>,
  max_width: Option<f32>,
}

impl<'a, MSG: 'a> Component<'a, MSG> {
  pub fn new(content: impl Into<Element<'a, MSG>>) -> Self {
    Self {
      content: content.into(),
      padding: Padding::ZERO,
      width: None,
      height: None,
      max_height: None,
      max_width: None,
    }
  }

  pub fn padding(mut self, p: impl Into<Padding>) -> Self {
    self.padding = p.into();
    self
  }

  pub fn width(mut self, w: impl Into<Length>) -> Self {
    self.width = Some(w.into());
    self
  }

  pub fn height(mut self, h: impl Into<Length>) -> Self {
    self.height = Some(h.into());
    self
  }

  pub fn max_height(mut self, h: f32) -> Self {
    self.max_height = Some(h);
    self
  }

  pub fn max_width(mut self, w: f32) -> Self {
    self.max_width = Some(w);
    self
  }

  pub fn render(self) -> Element<'a, MSG> {
    let mut c = container(self.content)
      .padding(self.padding)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::DEFAULT,
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        shadow: Shadow {
          blur_radius: 64.0,
          color: color::state::OVERLAY_DARKER,
          offset: Vector::new(0.0, 24.0),
        },
        ..container::Style::default()
      });
    if let Some(w) = self.width {
      c = c.width(w);
    }
    if let Some(h) = self.height {
      c = c.height(h);
    }
    if let Some(mh) = self.max_height {
      c = c.max_height(mh);
    }
    if let Some(mw) = self.max_width {
      c = c.max_width(mw);
    }
    c.into()
  }
}
