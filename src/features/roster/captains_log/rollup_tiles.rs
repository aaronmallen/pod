use iced::{Element, widget::Space};

use super::{Message as Parent, State};

pub(super) fn view(_state: &State) -> Element<'_, Parent> {
  Space::new().into()
}
