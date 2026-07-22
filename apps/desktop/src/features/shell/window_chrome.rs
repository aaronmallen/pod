use iced::{Background, Border, widget::container};

use crate::ui::style::{color, radius};

pub fn panel_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      radius: radius::PANEL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  }
}
