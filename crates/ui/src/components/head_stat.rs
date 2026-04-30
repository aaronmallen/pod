use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, row, text},
};

use crate::style::{color, typography};

pub struct Component<'a, M: 'a> {
  label: String,
  value: String,
  accent: Option<Element<'a, M>>,
}

impl<'a, M: 'a> Component<'a, M> {
  pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      value: value.into(),
      accent: None,
    }
  }

  pub fn accent(mut self, el: Element<'a, M>) -> Self {
    self.accent = Some(el);
    self
  }

  pub fn render(self) -> Element<'a, M> {
    let label_el = text(self.label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      });

    let value_el = text(self.value)
      .font(typography::mono::MEDIUM)
      .size(15.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      });

    let value_row: Element<'a, M> = if let Some(accent_el) = self.accent {
      row([value_el.into(), Space::new().width(8.0).into(), accent_el])
        .align_y(iced::alignment::Vertical::Center)
        .into()
    } else {
      value_el.into()
    };

    container(column([label_el.into(), Space::new().height(3.0).into(), value_row]).width(Length::Shrink))
      .padding(Padding::ZERO)
      .into()
  }
}
