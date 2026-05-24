//! Individual skill row with pips component.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::Message;
use crate::{
  components::SkillIndicator,
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::skills::skill_data::SkillDef,
};

pub struct Component {
  name: String,
  rank: u8,
  prereqs: Vec<(String, u8)>,
  char_level: u8,
  queue_delta: u8,
}

impl Component {
  pub fn new(skill: &SkillDef, char_level: u8, queue_level: u8) -> Self {
    let queue_delta = queue_level.saturating_sub(char_level);
    Self {
      name: skill.name.clone(),
      rank: skill.rank,
      prereqs: skill.prereqs.clone(),
      char_level,
      queue_delta,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let prereq_missing = self.char_level == 0 && !self.prereqs.is_empty();

    let mut skill_info: Vec<Element<'_, Message>> = vec![name_row(self.name.clone(), self.rank)];
    if prereq_missing {
      skill_info.push(Space::new().height(5.0).into());
      skill_info.push(prereq_chips(&self.prereqs));
    }

    column([
      container(Space::new().width(Length::Fill).height(1.0))
        .width(Length::Fill)
        .height(1.0)
        .style(|_| container::Style {
          background: Some(Background::Color(color::border::SUBTLE)),
          ..container::Style::default()
        })
        .into(),
      container(
        row([
          column(skill_info).width(Length::Fill).into(),
          queue_badge(self.char_level, self.queue_delta),
        ])
        .align_y(Vertical::Center),
      )
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 30.0,
        right: spacing::SPACE_3,
      })
      .width(Length::Fill)
      .into(),
    ])
    .into()
  }
}

fn name_row(name: String, rank: u8) -> Element<'static, Message> {
  row([
    text(name)
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(8.0).into(),
    text(format!("×{}", rank))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn prereq_chips(prereqs: &[(String, u8)]) -> Element<'static, Message> {
  let roman = ["I", "II", "III", "IV", "V"];
  let chips: Vec<Element<'_, Message>> = prereqs
    .iter()
    .map(|(name, level)| {
      let level_str = roman[(level.saturating_sub(1) as usize).min(4)];
      container(
        text(format!("req · {} {}", name, level_str))
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::status::CAUTION),
          }),
      )
      .padding(Padding {
        top: 1.0,
        bottom: 1.0,
        left: 6.0,
        right: 6.0,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::GOLD_SUBTLE)),
        border: Border {
          color: color::accent::GOLD_MUTED,
          radius: 3.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
    })
    .collect();
  row(chips).spacing(4.0).wrap().into()
}

fn queue_badge(char_level: u8, queue_delta: u8) -> Element<'static, Message> {
  let mut items: Vec<Element<'_, Message>> = vec![SkillIndicator::new(char_level).render()];
  if queue_delta > 0 {
    items.push(Space::new().width(6.0).into());
    items.push(
      container(
        text(format!("+{} queued", queue_delta))
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::accent::PLASMA),
          }),
      )
      .padding(Padding {
        top: 1.0,
        bottom: 1.0,
        left: 5.0,
        right: 5.0,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
        border: Border {
          color: color::state::SELECTION,
          radius: 3.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into(),
    );
  }
  row(items).align_y(Vertical::Center).into()
}
