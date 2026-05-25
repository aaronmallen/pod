//! Neural attribute display component.

pub mod attr_row;
pub mod rate_grid;
pub mod remap_card;

pub use attr_row::Component as AttrRow;
use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, text},
};
pub use rate_grid::RateGrid;
pub use remap_card::RemapCard;

use super::super::{State, skill_data::AttrKey};
use crate::{
  components,
  style::{color, spacing, typography::mono},
};

/// Messages produced by the attributes tab.
#[derive(Clone, Debug)]
pub enum Message {}

/// Attributes tab content for the skills right panel.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Constructs the attributes tab bound to the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the attributes tab into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let attr_pair = self
      .state
      .queue
      .first()
      .and_then(|q| super::super::skill_data::find_skill(&q.skill_name, &self.state.skill_groups))
      .map(|(s, _)| (s.primary, s.secondary))
      .or_else(|| {
        self
          .state
          .active_character()
          .and_then(|c| c.active_training())
          .and_then(|t| t.skill_name.as_ref())
          .and_then(|n| super::super::skill_data::find_skill(n, &self.state.skill_groups))
          .map(|(s, _)| (s.primary, s.secondary))
      });
    let active_primary = attr_pair.map(|(p, _)| p);
    let active_secondary = attr_pair.map(|(_, s)| s);
    let total_pts: u32 = AttrKey::ALL.iter().map(|k| self.state.attr_value(*k)).sum();
    let attr_bars = bar_items(self.state, total_pts, active_primary, active_secondary);

    column([
      column(attr_bars).width(Length::Fill).into(),
      RateGrid::new(self.state, active_primary, active_secondary).render(),
      RemapCard::new(self.state).render(),
      Space::new().height(spacing::SPACE_4).into(),
    ])
    .width(Length::Fill)
    .into()
  }
}

fn bar_items<'a>(
  state: &'a State,
  total_pts: u32,
  active_primary: Option<AttrKey>,
  active_secondary: Option<AttrKey>,
) -> Vec<Element<'a, Message>> {
  let mut bars: Vec<Element<'_, Message>> = vec![section_header(total_pts)];
  for (i, key) in AttrKey::ALL.iter().enumerate() {
    let is_primary = active_primary == Some(*key);
    let is_secondary = active_secondary == Some(*key);
    let accent = if is_primary {
      color::accent::PLASMA
    } else if is_secondary {
      color::accent::PLASMA_HOVER
    } else {
      color::text::PRIMARY
    };
    if i > 0 {
      bars.push(components::Separator::horizontal().render());
    }
    bars.push(AttrRow::new(*key, state.attr_value(*key), accent, is_primary, is_secondary).render());
  }
  bars
}

fn section_header<'a>(total_pts: u32) -> Element<'a, Message> {
  container(
    column([
      text("Neural attributes")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().height(4.0).into(),
      text(format!("{} pts allocated", total_pts))
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .width(Length::Fill),
  )
  .padding(Padding {
    top: 14.0,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .width(Length::Fill)
  .into()
}
