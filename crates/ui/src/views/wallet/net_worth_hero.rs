//! Net worth hero section — value display, change badge, chart, and timeframe.

pub mod chart_section;
pub mod composition_chip;
pub mod hero_lhs;
pub mod timeframe_picker;

pub use chart_section::ChartSection;
pub use composition_chip::Component as CompositionChip;
pub use hero_lhs::HeroLhs;
use iced::{
  Element, Length,
  widget::{Space, row},
};
pub use timeframe_picker::Component as TimeframePicker;

use crate::{
  style::color,
  views::wallet::{Message, State},
};

/// Builder for the net worth hero section.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new net worth hero component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the net worth hero into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let series = state.chart_series.clone();
    let current = state.total_liquid() + state.total_assets() + state.total_escrow();
    let change = state.net_worth_change;
    let start_balance = (current - change).max(0.0);
    let change_pct = if start_balance > 0.01 {
      change / start_balance * 100.0
    } else {
      0.0
    };
    let is_up = change >= 0.0;
    let left_col = HeroLhs::new(current, change, change_pct, is_up).render();
    let comp_chips: Element<'_, Message> = row([
      CompositionChip::new("Liquid", state.total_liquid(), color::accent::PLASMA).render(),
      Space::new().width(10.0).into(),
      CompositionChip::new("Assets", state.total_assets(), color::text::SECONDARY).render(),
      Space::new().width(10.0).into(),
      CompositionChip::new("Escrow", state.total_escrow(), color::status::CAUTION).render(),
    ])
    .into();
    let top_row: Element<'_, Message> = row([
      left_col,
      Space::new().width(Length::Fill).into(),
      comp_chips,
      Space::new().width(24.0).into(),
      TimeframePicker::new(&state.timeframe).render(),
    ])
    .align_y(iced::alignment::Vertical::Top)
    .into();
    let all_wallets = state.selected_character().is_none() && state.selected_corporation().is_none();
    ChartSection::new(
      top_row,
      series,
      is_up,
      &state.timeframe,
      state.chart_hover.as_ref(),
      &state.characters,
      all_wallets,
    )
    .render()
  }
}
