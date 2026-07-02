use iced::Element;

use super::{
  Message,
  card::{self, CardModel, Sections},
};
use crate::sync::Phase;

pub(super) fn list_row<'a>(
  model: &'a CardModel,
  failure: Option<Phase>,
  dragging: bool,
  sections: Sections,
) -> Element<'a, Message> {
  card::card(model, failure, dragging, sections)
}
