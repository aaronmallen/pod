//! Summary stats panel on the right side of the wallet view.

pub mod divider;
pub mod recent_activity_row;
pub mod section_label;
pub mod summary_stat_row;

pub use divider::Component as Divider;
use iced::{
  Background, Border, Element, Length,
  widget::{Space, column, container, scrollable},
};
pub use recent_activity_row::Component as RecentActivityRow;
pub use section_label::Component as SectionLabel;
pub use summary_stat_row::Component as SummaryStatRow;

use crate::{
  format,
  style::color,
  views::wallet::{Message, State},
};

fn recent_activity_rows<'a>(state: &'a State) -> Vec<Element<'a, Message>> {
  state
    .filtered_journal
    .iter()
    .take(8)
    .map(|j| RecentActivityRow::new(j).render())
    .collect()
}

/// Builder for the wallet right rail.
pub struct Component<'a> {
  state: &'a State,
  width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new right rail component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      width: 220.0,
    }
  }

  /// Sets the panel width.
  pub fn width(mut self, w: f32) -> Self {
    self.width = w;
    self
  }

  /// Renders the right rail into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let width = self.width;
    let state = self.state;
    let income = state.journal_income;
    let spend = state.journal_spend;
    let net = income - spend;
    let net_str = format!("{}{}", if net >= 0.0 { "+" } else { "−" }, format::fmt_isk(net.abs()));
    let net_color = if net >= 0.0 {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let content = scrollable(
      column([
        SectionLabel::new("30-Day Summary").render(),
        SummaryStatRow::new("Income", format::fmt_isk(income), color::status::ONLINE).render(),
        SummaryStatRow::new("Spend", format!("−{}", format::fmt_isk(spend)), color::status::DANGER).render(),
        SummaryStatRow::new("Net", net_str, net_color).render(),
        Divider::new().render(),
        SectionLabel::new("Recent Activity").render(),
        column(recent_activity_rows(state)).width(Length::Fill).into(),
        Space::new().height(Length::Fill).into(),
      ])
      .width(Length::Fill),
    )
    .height(Length::Fill);
    container(content)
      .width(Length::Fixed(width))
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::border::SUBTLE,
          width: 1.0,
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}
