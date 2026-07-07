use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, Space, button, container, text},
};

use super::super::Message;
use crate::ui::{
  components::skill_detail::info_button,
  style::{color, typography},
};

const PIP_WIDTH: f32 = 12.0;
const PIP_HEIGHT: f32 = 10.0;

pub(in crate::features::skills::skill_plan_editor) fn result_row<'a>(
  skill_id: i64,
  name: &'a str,
  rank: u8,
  trained: u8,
  planned: u8,
) -> Element<'a, Message> {
  let label = text(name)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .width(Length::Fill)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let rank_label = text(format!("\u{00d7}{rank}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });

  Row::with_children(vec![
    label.into(),
    info_button(Message::SkillInfoRequested(skill_id)),
    rank_label.into(),
    Space::new().width(8.0).into(),
    pip_strip(skill_id, trained, planned),
  ])
  .spacing(8.0)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 30.0,
    right: 12.0,
  })
  .width(Length::Fill)
  .into()
}

fn pip_strip<'a>(skill_id: i64, trained: u8, planned: u8) -> Element<'a, Message> {
  let mut pips: Vec<Element<'a, Message>> = Vec::with_capacity(5);
  for level in 1u8..=5 {
    pips.push(pip(skill_id, level, trained, planned));
  }
  Row::with_children(pips).spacing(3.0).align_y(Vertical::Center).into()
}

fn pip<'a>(skill_id: i64, level: u8, trained: u8, planned: u8) -> Element<'a, Message> {
  let is_trained = level <= trained;
  let is_planned = !is_trained && level <= planned;

  if is_trained {
    return container(Space::new().width(PIP_WIDTH).height(PIP_HEIGHT))
      .width(PIP_WIDTH)
      .height(PIP_HEIGHT)
      .style(|_| container::Style {
        background: Some(Background::Color(color::text::PRIMARY)),
        border: Border {
          color: color::text::PRIMARY,
          radius: 1.5.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into();
  }

  let (fill, border) = if is_planned {
    (
      Some(Background::Color(color::with_alpha(color::accent(), 0.25))),
      color::with_alpha(color::accent(), 0.6),
    )
  } else {
    (None, color::with_alpha(color::text::PRIMARY, 0.2))
  };

  button(Space::new().width(PIP_WIDTH).height(PIP_HEIGHT))
    .width(PIP_WIDTH)
    .height(PIP_HEIGHT)
    .padding(0.0)
    .on_press(Message::PickerLevelPicked(skill_id, level))
    .style(move |_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: if hover {
          Some(Background::Color(color::with_alpha(color::accent(), 0.15)))
        } else {
          fill
        },
        border: Border {
          color: if hover { color::accent() } else { border },
          radius: 1.5.into(),
          width: 1.0,
        },
        ..button::Style::default()
      }
    })
    .into()
}
