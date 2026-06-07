use iced::{Element, Length, widget::Column};

use super::{AbyssalCard, card, group_by_type};
use crate::{
  features::assets::Message,
  ui::{components::section_header::section_header, style::spacing},
};

pub(super) fn grid<'a>(cards: &[&'a AbyssalCard]) -> Element<'a, Message> {
  let mut groups: Vec<Element<'a, Message>> = Vec::new();
  for (label, members) in group_by_type(cards) {
    groups.push(group_block(label, &members));
  }
  Column::with_children(groups)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill)
    .into()
}

fn group_block<'a>(label: String, members: &[&'a AbyssalCard]) -> Element<'a, Message> {
  let count = format!("{} module{}", members.len(), if members.len() == 1 { "" } else { "s" });
  let heading = section_header(&label, Some(&count));

  let cells: Vec<Element<'a, Message>> = members.iter().map(|card| card::view(card)).collect();
  let cards_row = iced::widget::Row::with_children(cells)
    .spacing(spacing::SPACE_3_5)
    .wrap();

  Column::with_children(vec![heading, cards_row.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::assets::abyssals::AbyssalStat;

  fn card(item_id: i64, module: &str, group_type_id: i64) -> AbyssalCard {
    AbyssalCard {
      character_id: 7,
      estimate: Some(1_000_000.0),
      group_type_id,
      item_id,
      location: "Jita IV - Moon 4".to_owned(),
      module_name: module.to_owned(),
      owner_name: "Vex".to_owned(),
      price_unavailable: false,
      stats: vec![AbyssalStat {
        attribute_id: 50,
        base_value: 47.0,
        bound_hi: 56.0,
        bound_lo: 28.0,
        display_name: "Stasis".to_owned(),
        high_is_good: true,
        rolled: 41.0,
        unit_suffix: " tf".to_owned(),
      }],
      tier_label: "Gravid".to_owned(),
    }
  }

  mod grid {
    use super::*;

    #[test]
    fn it_renders_one_block_per_module_type() {
      let cards = [
        card(1, "Heavy Assault Missile Launcher II", 2410),
        card(2, "Adaptive Invulnerability Field II", 2281),
      ];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();

      let _el: Element<'_, Message> = grid(&refs);
    }
  }
}
