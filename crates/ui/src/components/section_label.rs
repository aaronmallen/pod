use iced::{Element, Theme, widget::text};

use crate::style::{color, typography::mono};

/// Renders an uppercase section-header label: mono regular, size 9, secondary color.
pub fn section_label<'a, MSG: 'a>(title: &'a str) -> Element<'a, MSG> {
  text(title.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()
}
