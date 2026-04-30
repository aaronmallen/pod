//! EVE-active training hero component.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};
use pod_model::AttrKey;

use super::{
  super::{Message, State, skill_data::find_skill},
  pip_row::{Component as PipRow, roman},
  progress_bar::Component as ProgressBar,
  right_col::Component as RightCol,
};
use crate::{
  format::sp_cost,
  style::{
    color, spacing,
    typography::{body, mono},
  },
};

pub struct Component<'a> {
  state: &'a State,
  sp_rate: f32,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State, sp_rate: f32) -> Self {
    Self {
      state,
      sp_rate,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let character = self.state.active_character().expect("checked before call");
    let training = character.active_training().expect("checked before call");

    let skill_name = training.skill_name.as_deref().unwrap_or("Unknown Skill");
    let to_level = training.active_level as u8;
    let from_level = training.trained_level as u8;
    let progress = character.training_percent().unwrap_or(0.0) as f32;
    let pct = (progress * 100.0).round() as u32;
    let remaining_secs = training_remaining_secs(training.training_end_time);
    let sp_min = (self.sp_rate * 60.0).round() as u64;
    let sp_day_k = ((self.sp_rate * 86400.0) as f64 / 1000.0) as u64;

    let (rank, primary, secondary, group_name) = find_skill(skill_name, &self.state.skill_groups)
      .map(|(s, g)| (s.rank, s.primary, s.secondary, g))
      .unwrap_or((1, AttrKey::Perception, AttrKey::Willpower, "EVE Training"));
    let (sp_now_val, sp_end) = eve_sp_now_and_end(
      training.training_level_start_sp,
      training.training_level_end_sp,
      rank,
      to_level,
      progress,
    );

    let left_col = left_col(group_name, skill_name, to_level, rank, from_level, remaining_secs);
    let right_col = RightCol::new(
      pct,
      sp_now_val,
      sp_end,
      primary,
      secondary,
      sp_min,
      sp_day_k,
      remaining_secs,
    )
    .render();
    layout(progress, left_col, right_col)
  }
}

fn left_col(
  group_name: &str,
  skill_name: &str,
  to_level: u8,
  rank: u8,
  from_level: u8,
  remaining_secs: u64,
) -> Element<'static, Message> {
  column([
    hero_header_row(group_name.to_uppercase()),
    Space::new().height(8.0).into(),
    hero_skill_name_row(skill_name.to_string(), to_level, rank),
    Space::new().height(16.0).into(),
    PipRow::new(from_level, to_level).render(),
    Space::new().height(18.0).into(),
    hero_remain_row(remaining_secs),
  ])
  .into()
}

fn layout<'a>(progress: f32, left_col: Element<'a, Message>, right_col: Element<'a, Message>) -> Element<'a, Message> {
  let content_row = row([
    container(left_col).width(Length::Fill).into(),
    Space::new().width(spacing::SPACE_7).into(),
    right_col,
  ]);
  column([
    ProgressBar::new(progress).render(),
    container(content_row)
      .padding(Padding {
        top: 22.0,
        bottom: 24.0,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .into(),
  ])
  .into()
}

fn training_remaining_secs(end_time: Option<i64>) -> u64 {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs() as i64;
  end_time.map(|e| (e - now).max(0) as u64).unwrap_or(0)
}

fn eve_sp_now_and_end(start_sp: Option<i64>, end_sp: Option<i64>, rank: u8, to_level: u8, progress: f32) -> (u64, u64) {
  let sp_start = start_sp.unwrap_or(0) as u64;
  let sp_end = end_sp
    .map(|v| v as u64)
    .unwrap_or_else(|| sp_start + sp_cost(rank as f64, to_level));
  let sp_earned = (sp_end.saturating_sub(sp_start) as f32 * progress) as u64;
  (sp_start + sp_earned, sp_end)
}

fn hero_header_row(group_name: String) -> Element<'static, Message> {
  row([
    text("Currently training")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Space::new().width(6.0).into(),
    container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(6.0).into(),
    text(group_name)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn hero_skill_name_row(name: String, to_level: u8, rank: u8) -> Element<'static, Message> {
  row([
    text(name)
      .font(body::MEDIUM)
      .size(32.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(14.0).into(),
    text(roman(to_level))
      .font(mono::MEDIUM)
      .size(22.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    container(
      text(format!("×{}", rank))
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 3.0,
      bottom: 3.0,
      left: 8.0,
      right: 8.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into(),
  ])
  .align_y(Vertical::Bottom)
  .into()
}

fn hero_remain_row(remaining_secs: u64) -> Element<'static, Message> {
  use crate::format::fmt_dur;
  row([
    text(fmt_dur(remaining_secs))
      .font(mono::MEDIUM)
      .size(28.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    text("remaining")
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .align_y(Vertical::Bottom)
  .into()
}
