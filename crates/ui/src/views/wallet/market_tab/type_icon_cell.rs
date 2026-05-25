//! Type icon cell for market entry rows — renders an image or a placeholder.

use iced::{
  Border, Element,
  widget::{Space, container, image},
};

use crate::{style::color, views::wallet::market_tab::Message};

/// Builder for a type icon cell.
pub struct TypeIconCell {
  handle: Option<image::Handle>,
}

impl TypeIconCell {
  /// Creates a new type icon cell with an optional image handle.
  ///
  /// When `handle` is `None` a placeholder box is rendered instead.
  pub fn new(handle: Option<image::Handle>) -> Self {
    Self {
      handle,
    }
  }

  /// Renders the cell into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let size = 32.0f32;
    if let Some(h) = self.handle {
      container(image::Image::new(h).width(size).height(size))
        .width(size)
        .height(size)
        .into()
    } else {
      container(Space::new().width(size).height(size))
        .width(size)
        .height(size)
        .style(|_| container::Style {
          background: Some(iced::Background::Color(color::state::HOVER_OVERLAY)),
          border: Border {
            radius: 4.0.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into()
    }
  }
}
