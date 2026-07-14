use iced::{
  Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{container, text},
};

use super::{Message, State};
use crate::ui::style::{color, spacing, typography};

pub(super) fn tab(state: &State) -> Element<'_, Message> {
  let _ = state;
  container(
    text(t!("industry.colonies.empty"))
      .font(typography::body::REGULAR)
      .size(typography::size::LG)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .padding(spacing::SPACE_6)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

#[cfg(test)]
mod tests {
  use super::{
    super::{EMPTY_INDUSTRY_SELECTION, FacilityDefaults, Tab},
    *,
  };

  fn state_with_colonies_tab() -> State {
    let mut state = State::new(
      EMPTY_INDUSTRY_SELECTION,
      Vec::new(),
      crate::config::FeatureFlags::default(),
      FacilityDefaults::default(),
      None,
      false,
    );
    state.seed_tab(Tab::Colonies);
    state
  }

  mod tab {
    use super::*;

    #[test]
    fn it_renders_the_empty_state() {
      let state = state_with_colonies_tab();

      let _el: Element<'_, Message> = tab(&state);
    }
  }
}
