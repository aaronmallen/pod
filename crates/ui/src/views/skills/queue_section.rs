//! Skill queue list container component.

pub mod col_header;
pub mod empty_state;
pub mod footer;
pub mod row;

pub use col_header::Component as ColHeader;
pub use empty_state::Component as EmptyState;
pub use footer::Component as Footer;
use iced::{
  Background, Border, Element, Length, Padding,
  widget::{column, container},
};
pub use row::Component as Row;

use super::{ComputedQueueItem, Message, State};
use crate::style::{color, spacing};

fn build_queue_container(items: &[ComputedQueueItem], skip_n: usize) -> Element<'static, Message> {
  if items.len() <= skip_n {
    return EmptyState::new().render();
  }
  let total_secs: f32 = items.iter().map(|i| i.duration_secs).sum();
  let total_n = items.len() - skip_n;

  let mut row_els: Vec<Element<'_, Message>> = vec![ColHeader::new().render()];
  for (i, item) in items.iter().enumerate().skip(skip_n) {
    row_els.push(Row::new(item.clone(), i - skip_n).render());
  }
  row_els.push(Footer::new(total_n, total_secs).render());

  container(column(row_els).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

pub struct Component<'a, 'b> {
  state: &'a State,
  items: &'b [ComputedQueueItem],
}

impl<'a, 'b> Component<'a, 'b>
where
  'a: 'b,
{
  pub fn new(state: &'a State, items: &'b [ComputedQueueItem]) -> Self {
    Self {
      state,
      items,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let has_active = self
      .state
      .active_character()
      .and_then(|c| c.active_training())
      .is_some();
    let skip_n = if has_active { 1 } else { 0 };
    let queue_container = build_queue_container(self.items, skip_n);

    container(queue_container)
      .width(Length::Fill)
      .padding(Padding {
        top: 20.0,
        bottom: 0.0,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .into()
  }
}
