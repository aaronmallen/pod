use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{super::super::browse::SkillLeaf, Message};
use crate::ui::{
  components::badge::badge,
  style::{color, radius, spacing, typography},
};

const ETA_COLUMN_WIDTH: f32 = 70.0;
const LEAF_INDENT: f32 = 30.0;
const PIP_SIZE: f32 = 6.0;
const WARNING: iced::Color = color::status::WARNING;

pub fn skill_row(leaf: &SkillLeaf) -> Element<'_, Message> {
  let mut name_block: Vec<Element<'_, Message>> = vec![
    Row::with_children(vec![
      text(leaf.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(format!("\u{00d7}{}", leaf.rank))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into(),
  ];

  if !leaf.prereqs.is_empty() {
    let chips: Vec<Element<'_, Message>> = leaf
      .prereqs
      .iter()
      .map(|(name, level)| prereq_chip(name, *level))
      .collect();
    name_block.push(Row::with_children(chips).spacing(spacing::UNIT).wrap().into());
  }

  let name_column = Column::with_children(name_block).spacing(5.0).width(Length::Fill);

  let mut indicators: Vec<Element<'_, Message>> = vec![mini_pips(leaf.level)];
  if leaf.queue_delta > 0 {
    indicators.push(queued_marker(leaf.queue_delta));
  }
  let indicator_row = Row::with_children(indicators)
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center);

  let eta = container(
    text(leaf.next_eta.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .width(Length::Fixed(ETA_COLUMN_WIDTH))
  .align_x(Horizontal::Right);

  let row = Row::with_children(vec![name_column.into(), indicator_row.into(), eta.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2,
      left: LEAF_INDENT,
    })
    .on_press(Message::SkillSelected(leaf.skill_id))
    .style(|_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: hover.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.04))),
        text_color: color::text::PRIMARY,
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.1),
          width: 1.0,
          ..Border::default()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn mini_pips<'a>(level: u8) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = (1..=5)
    .map(|i| {
      let filled = i <= level;
      container(
        Space::new()
          .width(Length::Fixed(PIP_SIZE))
          .height(Length::Fixed(PIP_SIZE)),
      )
      .style(move |_| container::Style {
        background: Some(Background::Color(if filled {
          color::text::PRIMARY
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        })),
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
    })
    .collect();

  Row::with_children(cells).spacing(3.0).align_y(Vertical::Center).into()
}

fn prereq_chip<'a>(name: &str, level: u8) -> Element<'a, Message> {
  badge(format!("req \u{00b7} {} {}", name, roman(level)), Some(WARNING))
}

fn queued_marker<'a>(delta: u8) -> Element<'a, Message> {
  badge(format!("+{delta} queued"), Some(color::accent::PLASMA))
}

fn roman(level: u8) -> &'static str {
  match level {
    1 => "I",
    2 => "II",
    3 => "III",
    4 => "IV",
    5 => "V",
    _ => "",
  }
}
