use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, row, text},
};

use super::{EditRemap, Message, REMAP_ATTR_ORDER, attr_value, attribute_to_attr_key};
use crate::ui::{
  components::{eyebrow::eyebrow, icon::Icon},
  style::{color, typography},
};

const INDEX_COL_WIDTH: f32 = 28.0;

pub(super) fn remap_divider<'a>(remap: &EditRemap, label: &str) -> Element<'a, Message> {
  let local_id = remap.local_id;

  let steppers = REMAP_ATTR_ORDER.iter().fold(
    row(Vec::new()).align_y(Vertical::Center).spacing(4.0),
    |acc, &attribute| acc.push(stepper(local_id, attribute, attr_value(remap.base, attribute))),
  );

  let inner = row(vec![
    index_mark(),
    Space::new().width(8.0).into(),
    title_block(label),
    container(steppers)
      .width(Length::Fill)
      .align_x(Horizontal::Right)
      .into(),
    Space::new().width(8.0).into(),
    remove_btn(local_id),
    Space::new().width(12.0).into(),
  ])
  .align_y(Vertical::Center)
  .spacing(10.0)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 12.0,
    right: 0.0,
  });

  container(inner)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.05))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, 0.2),
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn index_mark<'a>() -> Element<'a, Message> {
  container(
    text("\u{21bb}")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(Length::Fixed(INDEX_COL_WIDTH))
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .into()
}

fn title_block<'a>(label: &str) -> Element<'a, Message> {
  column(vec![
    eyebrow("NEURAL REMAP", Some(color::accent::PLASMA)),
    Space::new().height(2.0).into(),
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .into()
}

fn stepper<'a>(local_id: i64, attribute: super::Attribute, value: u32) -> Element<'a, Message> {
  let key = attribute_to_attr_key(attribute);

  let body = row(vec![
    text(key.short())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().width(4.0).into(),
    container(
      text(value.to_string())
        .font(typography::mono::MEDIUM)
        .size(12.0)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .width(Length::Fixed(18.0))
    .align_x(Horizontal::Right)
    .into(),
    Space::new().width(4.0).into(),
    column(vec![
      step_btn(Icon::chevron_up(), Message::RemapAttrBumped(local_id, key, 1)),
      Space::new().height(1.0).into(),
      step_btn(Icon::chevron_down(), Message::RemapAttrBumped(local_id, key, -1)),
    ])
    .into(),
  ])
  .align_y(Vertical::Center);

  container(body)
    .padding(Padding {
      top: 3.0,
      bottom: 3.0,
      left: 8.0,
      right: 4.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.08))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, 0.25),
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn step_btn<'a>(icon: Icon, message: Message) -> Element<'a, Message> {
  button(icon.size(7.0).color(color::text::secondary()).render())
    .padding(0.0)
    .width(Length::Fixed(14.0))
    .height(Length::Fixed(9.0))
    .on_press(message)
    .style(|_, status| button::Style {
      background: match status {
        button::Status::Hovered | button::Status::Pressed => {
          Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.2)))
        }
        _ => None,
      },
      border: Border {
        radius: 2.0.into(),
        ..Border::default()
      },
      ..button::Style::default()
    })
    .into()
}

fn remove_btn<'a>(local_id: i64) -> Element<'a, Message> {
  button(
    text("\u{00d7}")
      .font(typography::mono::REGULAR)
      .size(13.0)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(Message::RemapRemoved(local_id))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::status::DANGER, 0.12)))
      }
      _ => None,
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
  .into()
}
