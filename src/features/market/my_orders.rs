use super::{Message, shell};
use crate::ui::components::icon::Icon;

pub(super) fn surface<'a>() -> iced::Element<'a, Message> {
  shell::empty_state(
    Icon::contracts(),
    "market.orders_empty_title",
    "market.orders_empty_body",
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_renders_the_orders_empty_state() {
    let _el: iced::Element<'_, Message> = surface();
  }
}
