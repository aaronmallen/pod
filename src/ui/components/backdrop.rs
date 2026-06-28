use iced::{
  Color, Element, Length,
  widget::{Space, button},
};

use crate::ui::style::color;

pub fn backdrop<'a, M>(on_press: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(Space::new())
    .width(Length::Fill)
    .height(Length::Fill)
    .on_press(on_press)
    .style(|_, _| button::Style {
      background: Some(color::state::SCRIM.into()),
      ..button::Style::default()
    })
    .into()
}

pub fn click_catcher<'a, M>(on_press: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(Space::new())
    .width(Length::Fill)
    .height(Length::Fill)
    .on_press(on_press)
    .style(|_, _| button::Style {
      background: Some(Color::TRANSPARENT.into()),
      ..button::Style::default()
    })
    .into()
}
