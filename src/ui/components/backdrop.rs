use iced::{
  Color, Element, Length,
  widget::{Space, button},
};

const SCRIM: Color = Color {
  r: 8.0 / 255.0,
  g: 9.0 / 255.0,
  b: 11.0 / 255.0,
  a: 0.6,
};

pub fn backdrop<'a, M>(on_press: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(Space::new())
    .width(Length::Fill)
    .height(Length::Fill)
    .on_press(on_press)
    .style(|_, _| button::Style {
      background: Some(SCRIM.into()),
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
