use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{container, row, text_input},
};

use super::super::Message;
use crate::ui::{
  components::{icon::Icon, text_input::inner_style},
  style::{color, radius, typography},
};

const FONT_SIZE: f32 = 13.0;
const HEIGHT: f32 = 36.0;
const HORIZONTAL_PADDING: f32 = 12.0;
const ICON_SIZE: f32 = 14.0;
const ICON_SPACING: f32 = 8.0;

pub(in crate::features::skills::skill_plan_editor) fn search_bar<'a>(
  query: &'a str,
  placeholder: String,
) -> Element<'a, Message> {
  let input = text_input(&placeholder, query)
    .on_input(Message::PickerSearchChanged)
    .font(typography::body::REGULAR)
    .size(FONT_SIZE)
    .padding(Padding::ZERO)
    .width(Length::Fill)
    .style(inner_style());

  let children: Vec<Element<'a, Message>> = vec![
    Icon::search()
      .size(ICON_SIZE)
      .color(color::text::secondary())
      .render::<Message>(),
    input.into(),
  ];

  container(row(children).spacing(ICON_SPACING).align_y(Vertical::Center))
    .width(Length::Fill)
    .height(HEIGHT)
    .align_y(Vertical::Center)
    .padding(Padding {
      bottom: 0.0,
      left: HORIZONTAL_PADDING,
      right: HORIZONTAL_PADDING,
      top: 0.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}
