use iced::{
  Element, Length,
  widget::{Space, button},
};

pub struct Component<MSG> {
  on_press: MSG,
}

impl<MSG: Clone + 'static> Component<MSG> {
  pub fn new(on_press: MSG) -> Self {
    Self {
      on_press,
    }
  }

  pub fn render<'a>(self) -> Element<'a, MSG> {
    button(Space::new())
      .width(Length::Fill)
      .height(Length::Fill)
      .on_press(self.on_press)
      .style(|_, _| button::Style::default())
      .into()
  }
}
