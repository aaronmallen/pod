//! Header SP/queue stat components.

mod stat_cell;

use iced::{
  Element, Length,
  widget::{Space, column, text},
};

use super::super::fmt_sp;
use crate::{
  format::{fmt_dur, fmt_eta},
  style::{color, typography::mono},
};

pub struct SpStat {
  total_sp: u64,
}

impl SpStat {
  pub fn new(total_sp: u64) -> Self {
    Self {
      total_sp,
    }
  }

  pub fn render(self) -> Element<'static, super::super::Message> {
    let sp_value = fmt_sp(self.total_sp) + " SP";
    stat_cell::StatCell::new("Total skill points", sp_value, color::text::PRIMARY).render()
  }
}

pub struct QueueStat {
  queue_len: usize,
  total_secs: u64,
  low_queue: bool,
}

impl QueueStat {
  pub fn new(queue_len: usize, total_secs: u64, low_queue: bool) -> Self {
    Self {
      queue_len,
      total_secs,
      low_queue,
    }
  }

  pub fn render(self) -> Element<'static, super::super::Message> {
    let label = format!(
      "Queue · {} {}",
      self.queue_len,
      if self.queue_len == 1 { "skill" } else { "skills" }
    );
    let value = if self.total_secs > 0 {
      fmt_dur(self.total_secs)
    } else {
      "Empty".to_string()
    };
    let color = if self.low_queue {
      color::status::DANGER
    } else {
      color::text::PRIMARY
    };
    stat_cell::StatCell::new(label, value, color).render()
  }
}

pub struct QueueCompleteStat {
  total_secs: u64,
}

impl QueueCompleteStat {
  pub fn new(total_secs: u64) -> Self {
    Self {
      total_secs,
    }
  }

  pub fn render(self) -> Element<'static, super::super::Message> {
    let label_el = text("Queue completes".to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      });
    let value_el = text(format!("{} EVE", fmt_eta(self.total_secs)))
      .font(mono::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      });
    column([label_el.into(), Space::new().height(4.0).into(), value_el.into()])
      .width(Length::Shrink)
      .into()
  }
}
