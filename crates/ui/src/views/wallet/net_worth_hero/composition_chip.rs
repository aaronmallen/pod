//! Colored dot + label + ISK value chip for the net worth hero section.

use iced::{
  Background, Border, Color, Element, Padding, Theme,
  widget::{Space, column, container, row, text},
};

use crate::{
  format,
  style::{color, typography::mono},
  views::wallet::Message,
};

fn composition_dot(dot_color: Color) -> Element<'static, Message> {
  container(Space::new().width(6.0).height(6.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(dot_color)),
      border: Border {
        radius: 50.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

/// Builder for a composition chip (dot + label + ISK value).
pub struct Component {
  label: &'static str,
  value: Option<f64>,
  dot_color: Color,
}

impl Component {
  /// Creates a new composition chip with a known ISK value.
  pub fn new(label: &'static str, value: f64, dot_color: Color) -> Self {
    Self {
      label,
      value: Some(value),
      dot_color,
    }
  }

  /// Creates a chip whose value is not yet available (renders as "—").
  pub fn unavailable(label: &'static str, dot_color: Color) -> Self {
    Self {
      label,
      value: None,
      dot_color,
    }
  }

  /// Renders the chip into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let dot = composition_dot(self.dot_color);
    let label_el: Element<'_, Message> = text(self.label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into();
    let value_str = self.value.map_or("N/A".to_string(), format::fmt_isk);
    let value_color = if self.value.is_some() {
      color::text::PRIMARY
    } else {
      color::text::TERTIARY
    };
    let value_el: Element<'_, Message> = text(value_str)
      .font(mono::MEDIUM)
      .size(13.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(value_color),
      })
      .into();
    container(column([
      row([dot, Space::new().width(6.0).into(), label_el])
        .align_y(iced::alignment::Vertical::Center)
        .into(),
      Space::new().height(4.0).into(),
      value_el,
    ]))
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 14.0,
      right: 14.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}
