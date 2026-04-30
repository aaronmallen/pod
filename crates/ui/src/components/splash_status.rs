use iced::{Element, Pixels};

use crate::style::{color, typography as font};

/// Renders a muted status label in JetBrains Mono.
pub struct Component {
  label: String,
}

impl Component {
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
    }
  }

  pub fn render<M: 'static>(self) -> Element<'static, M> {
    iced::widget::text(self.label)
      .font(font::mono::REGULAR)
      .size(Pixels(11.0))
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into()
  }
}
