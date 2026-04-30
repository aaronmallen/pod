use iced::{
  Element, Length, Padding,
  widget::{container, row, space, text},
};

use crate::style::{color, typography};

pub struct Component<'a, MSG> {
  title: &'a str,
  actions: Vec<Element<'a, MSG>>,
}

impl<'a, MSG: Clone + 'a> Component<'a, MSG> {
  pub fn new(title: &'a str) -> Self {
    Self {
      title,
      actions: vec![],
    }
  }

  pub fn action(mut self, el: impl Into<Element<'a, MSG>>) -> Self {
    self.actions.push(el.into());
    self
  }

  pub fn render(self) -> Element<'a, MSG> {
    let title: Element<'a, MSG> = text(self.title)
      .font(typography::mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into();

    let spacer: Element<'a, MSG> = space().width(Length::Fill).into();

    let mut children: Vec<Element<'a, MSG>> = vec![title, spacer];
    children.extend(self.actions);

    container(row(children).align_y(iced::alignment::Vertical::Center))
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: 14.0,
        right: 14.0,
      })
      .width(Length::Fill)
      .into()
  }
}
