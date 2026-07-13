use super::{Message, shell};
use crate::ui::components::icon::Icon;

pub(super) fn surface<'a>() -> iced::Element<'a, Message> {
  shell::empty_state(
    Icon::star(),
    "market.watchlist_empty_title",
    "market.watchlist_empty_body",
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_renders_the_watchlist_empty_state() {
    let _el: iced::Element<'_, Message> = surface();
  }
}
