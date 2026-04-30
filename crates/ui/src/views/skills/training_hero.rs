//! Hero panel that dispatches to active/queue_item/idle.

pub mod active;
pub mod idle;
pub mod pip_row;
pub mod progress_bar;
pub mod queue_item;
pub mod right_col;

pub use active::Component as Active;
use iced::{Background, Border, Element, Length, Padding, widget::container};
pub use idle::Component as Idle;
pub use pip_row::Component as PipRow;
pub use progress_bar::Component as ProgressBar;
pub use queue_item::Component as QueueItem;
pub use right_col::Component as RightCol;

use super::{ComputedQueueItem, Message, State};
use crate::style::{color, spacing};

pub struct Component<'a, 'b> {
  state: &'a State,
  items: &'b [ComputedQueueItem],
  sp_rate: f32,
}

impl<'a, 'b> Component<'a, 'b>
where
  'a: 'b,
{
  pub fn new(state: &'a State, items: &'b [ComputedQueueItem], sp_rate: f32) -> Self {
    Self {
      state,
      items,
      sp_rate,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let hero_inner: Element<'_, Message> = if let Some(entry) = self.items.first()
      && self
        .state
        .active_character()
        .and_then(|c| c.active_training())
        .is_none()
    {
      QueueItem::new(entry.clone(), self.sp_rate).render()
    } else if self
      .state
      .active_character()
      .and_then(|c| c.active_training())
      .is_some()
    {
      Active::new(self.state, self.sp_rate).render()
    } else {
      Idle::new().render()
    };

    container(container(hero_inner).width(Length::Fill).style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    }))
    .padding(Padding {
      top: 20.0,
      bottom: 0.0,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    })
    .width(Length::Fill)
    .into()
  }
}
