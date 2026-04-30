use iced::{
  Background, Border, Element, Padding,
  widget::{column, container, row, text},
};
use pod_model::Character;

use crate::{
  components,
  style::{color, spacing, typography},
};

pub struct Component<'a> {
  character: &'a Character,
}

impl<'a> Component<'a> {
  pub fn new(character: &'a Character) -> Self {
    Self {
      character,
    }
  }

  pub fn render<MSG: 'static>(self) -> Element<'a, MSG> {
    let queue = self.character.training_queue();
    let is_paused = !queue.is_empty() && queue.iter().all(|e| e.start_date.is_none());
    let content = if self.character.active_training().is_some() {
      active_training(self.character)
    } else if is_paused {
      paused_training()
    } else {
      idle_training()
    };

    container(content)
      .padding(Padding {
        top: 12.0,
        bottom: 12.0,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(iced::Length::Fill)
      .height(iced::Length::Fill)
      .into()
  }
}

fn active_training<'a, MSG: 'static>(character: &'a Character) -> Element<'a, MSG> {
  let skill = character.active_training().expect("checked before call");
  let skill_name = skill
    .skill_name
    .clone()
    .unwrap_or_else(|| format!("Skill {}", skill.skill_id));
  let level_roman = ["I", "II", "III", "IV", "V"]
    .get((skill.active_level as usize).saturating_sub(1).min(4))
    .copied()
    .unwrap_or("I");
  let pct = character.training_percent().unwrap_or(0.0);
  let eta = skill.training_end_time.map(format_eta).unwrap_or_default();

  column([
    row([
      training_label(),
      iced::widget::Space::new().width(iced::Length::Fill).into(),
      text(eta)
        .font(typography::mono::REGULAR)
        .size(11.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .into(),
    row([
      text(skill_name)
        .font(typography::body::REGULAR)
        .size(14.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(format!(" {level_roman}"))
        .font(typography::body::REGULAR)
        .size(12.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      iced::widget::Space::new().width(iced::Length::Fill).into(),
      skill_level_pips(skill.active_level),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .spacing(spacing::SPACE_1)
    .into(),
    components::ProgressBar::new(pct as f32).render(),
  ])
  .spacing(spacing::SPACE_1)
  .into()
}

fn paused_training<'a, MSG: 'a>() -> Element<'a, MSG> {
  column([
    training_label(),
    row([
      text("●")
        .size(12.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::status::CAUTION),
        })
        .into(),
      text("Training paused")
        .font(typography::body::REGULAR)
        .size(14.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::status::CAUTION),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_1)
    .into(),
  ])
  .spacing(spacing::SPACE_1)
  .into()
}

fn idle_training<'a, MSG: 'a>() -> Element<'a, MSG> {
  column([
    training_label(),
    row([
      text("●")
        .size(12.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
      text("Skill queue empty")
        .font(typography::body::REGULAR)
        .size(14.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_1)
    .into(),
  ])
  .spacing(spacing::SPACE_1)
  .into()
}

fn training_label<'a, MSG: 'a>() -> Element<'a, MSG> {
  text("TRAINING")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()
}

fn skill_level_pips<'a, MSG: 'static>(level: i32) -> Element<'a, MSG> {
  let pips: Vec<Element<'a, MSG>> = (1..=5)
    .map(|i| {
      container(iced::widget::Space::new())
        .width(6.0)
        .height(6.0)
        .style(move |_| {
          let pip_color = if i <= level {
            color::text::PRIMARY
          } else {
            color::border::SUBTLE
          };
          container::Style {
            background: Some(Background::Color(pip_color)),
            border: Border {
              radius: 1.5.into(),
              ..Border::default()
            },
            ..container::Style::default()
          }
        })
        .into()
    })
    .collect();

  row(pips).spacing(3.0).into()
}

fn format_eta(end_time: i64) -> String {
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let remaining = (end_time - now).max(0);
  let days = remaining / 86400;
  let hours = (remaining % 86400) / 3600;
  let minutes = (remaining % 3600) / 60;
  match (days, hours, minutes) {
    (d, h, _) if d > 0 && h > 0 => format!("{d}d {h}h"),
    (d, _, m) if d > 0 => format!("{d}d {m}m"),
    (_, h, m) if h > 0 => format!("{h}h {m}m"),
    (_, _, m) => format!("{m}m"),
  }
}
