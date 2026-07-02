use iced::Element;

use super::{
  Message,
  corp_card::{self, CorpCardModel},
};
use crate::sync::Phase;

pub(super) fn corp_list_row<'a>(model: &'a CorpCardModel, failure: Option<Phase>) -> Element<'a, Message> {
  corp_card::corp_card(model, failure)
}
