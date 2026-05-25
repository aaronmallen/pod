//! Picker row + SP/queue stat cells + bottom border.

pub mod hdivider;
pub mod queue_stats;

use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row},
};
pub use queue_stats::{QueueCompleteStat, QueueStat, SpStat};

use super::Message;
use crate::style::{color, spacing};

pub struct Component<'a> {
  state: &'a super::State,
  total_secs: u64,
  queue_len: usize,
  low_queue: bool,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a super::State, total_secs: u64, queue_len: usize, low_queue: bool) -> Self {
    Self {
      state,
      total_secs,
      queue_len,
      low_queue,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let picker_btn = self.state.picker.render().map(Message::Picker);

    let total_sp = self
      .state
      .active_character()
      .map(|c| c.skills().iter().map(|s| s.skillpoints).sum::<i64>() as u64)
      .unwrap_or(0);

    let mut items: Vec<Element<'_, Message>> = vec![
      picker_btn,
      hdivider::HDivider::new().render(),
      SpStat::new(total_sp).render(),
      hdivider::HDivider::new().render(),
      QueueStat::new(self.queue_len, self.total_secs, self.low_queue).render(),
      Space::new().width(Length::Fill).into(),
    ];
    if self.total_secs > 0 {
      items.push(QueueCompleteStat::new(self.total_secs).render());
    }

    let hrow = row(items)
      .spacing(spacing::SPACE_6)
      .align_y(Vertical::Center)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .height(Length::Fixed(spacing::layout::HEADER_HEIGHT));
    let border_line = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      });
    column([hrow.into(), border_line.into()]).width(Length::Fill).into()
  }
}
