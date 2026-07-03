pub mod attr_row;
pub mod rate_grid;
pub mod remap_card;
pub mod section_header;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, scrollable, text},
};

use super::super::attributes::{AttrTabModel, project_rows, remap_days, sp_per_hr_matrix};
use crate::{
  features::skills::optimizer::{Attribute, Attributes},
  ui::style::{color, radius, spacing, typography},
};

pub(super) const ATTR_ORDER: [Attribute; 5] = [
  Attribute::Perception,
  Attribute::Willpower,
  Attribute::Intelligence,
  Attribute::Memory,
  Attribute::Charisma,
];
pub(super) const WARNING: iced::Color = color::status::WARNING;

pub(super) fn pair_label(index: usize) -> String {
  match index {
    0 => t!("skills.panel_attributes.pair_combat"),
    1 => t!("skills.panel_attributes.pair_engineering"),
    2 => t!("skills.panel_attributes.pair_drones"),
    3 => t!("skills.panel_attributes.pair_navigation"),
    4 => t!("skills.panel_attributes.pair_trade"),
    _ => t!("skills.panel_attributes.pair_social"),
  }
  .into_owned()
}

pub fn view<'a, Message: 'a>(model: Option<&'a AttrTabModel>, now: DateTime<Utc>) -> Element<'a, Message> {
  let Some(model) = model else {
    return awaiting_state();
  };

  let effective = effective_of(model);
  let rows = project_rows(model.base, model.implants, model.booster_n, model.active);
  let matrix = sp_per_hr_matrix(effective, model.active);
  let days = remap_days(
    now,
    model.last_remap_date.as_deref(),
    model.accrued_remap_cooldown_date.as_deref(),
  );

  let mut children: Vec<Element<'a, Message>> = vec![section_header::section_header(model.base)];
  if !model.consistent {
    children.push(stale_notice());
  }
  for (index, row) in rows.iter().enumerate() {
    children.push(attr_row::attr_row(*row, index == 0));
  }
  children.push(rate_grid::rate_grid(&matrix));
  children.push(remap_card::remap_cta(model.bonus_remaps, days));
  if model.consistent {
    children.push(remap_card::recommendation_card(model));
  }
  children.push(Space::new().height(Length::Fixed(spacing::SPACE_3)).into());

  let body = Column::with_children(children)
    .spacing(spacing::SPACE_2)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3,
      bottom: spacing::UNIT,
      left: spacing::SPACE_3,
    })
    .width(Length::Fill);

  scrollable(body)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn effective_of(model: &AttrTabModel) -> Attributes {
  Attributes {
    charisma: model.base.charisma + model.implants.charisma + model.booster_n,
    intelligence: model.base.intelligence + model.implants.intelligence + model.booster_n,
    memory: model.base.memory + model.implants.memory + model.booster_n,
    perception: model.base.perception + model.implants.perception + model.booster_n,
    willpower: model.base.willpower + model.implants.willpower + model.booster_n,
  }
}

fn stale_notice<'a, Message: 'a>() -> Element<'a, Message> {
  let body = Row::with_children(vec![
    text("\u{26a0}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(WARNING),
      })
      .into(),
    text(t!("skills.panel_attributes.stale_data_notice"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(WARNING, 0.10))),
      border: Border {
        color: color::with_alpha(WARNING, 0.35),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn awaiting_state<'a, Message: 'a>() -> Element<'a, Message> {
  container(
    text(t!("skills.panel_attributes.awaiting"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .padding(spacing::SPACE_6)
  .into()
}

pub(super) fn card<'a, Message: 'a>(
  body: Element<'a, Message>,
  fill: iced::Color,
  border: iced::Color,
) -> Element<'a, Message> {
  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| container::Style {
      background: Some(iced::Background::Color(fill)),
      border: iced::Border {
        color: border,
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

pub(super) fn attribute_label(attribute: Attribute) -> String {
  match attribute {
    Attribute::Charisma => t!("skills.panel_attributes.attr_charisma"),
    Attribute::Intelligence => t!("skills.panel_attributes.attr_intelligence"),
    Attribute::Memory => t!("skills.panel_attributes.attr_memory"),
    Attribute::Perception => t!("skills.panel_attributes.attr_perception"),
    Attribute::Willpower => t!("skills.panel_attributes.attr_willpower"),
  }
  .into_owned()
}

pub(super) fn attribute_short(attribute: Attribute) -> String {
  match attribute {
    Attribute::Charisma => t!("skills.panel_attributes.attr_short_charisma"),
    Attribute::Intelligence => t!("skills.panel_attributes.attr_short_intelligence"),
    Attribute::Memory => t!("skills.panel_attributes.attr_short_memory"),
    Attribute::Perception => t!("skills.panel_attributes.attr_short_perception"),
    Attribute::Willpower => t!("skills.panel_attributes.attr_short_willpower"),
  }
  .into_owned()
}

pub(super) fn value_of(attributes: Attributes, attribute: Attribute) -> u32 {
  match attribute {
    Attribute::Charisma => attributes.charisma,
    Attribute::Intelligence => attributes.intelligence,
    Attribute::Memory => attributes.memory,
    Attribute::Perception => attributes.perception,
    Attribute::Willpower => attributes.willpower,
  }
}

pub(super) fn group_thousands_u64(value: u64) -> String {
  let digits = value.to_string();
  let mut out = String::with_capacity(digits.len() + digits.len() / 3);
  let bytes = digits.as_bytes();
  for (index, byte) in bytes.iter().enumerate() {
    if index > 0 && (bytes.len() - index).is_multiple_of(3) {
      out.push(',');
    }
    out.push(*byte as char);
  }
  out
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;
  use crate::{features::skills::optimizer::PairWeight, store::model::CharacterAttributes};

  fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
  }

  fn attributes_row() -> CharacterAttributes {
    CharacterAttributes {
      accrued_remap_cooldown_date: Some("2026-09-01T12:00:00Z".to_owned()),
      bonus_remaps: 2,
      character_id: 42,
      charisma: 17,
      intelligence: 21,
      last_remap_date: Some("2026-04-01T12:00:00Z".to_owned()),
      memory: 20,
      perception: 22,
      unallocated_sp: 0,
      willpower: 19,
    }
  }

  fn stored_row(charisma: i64, intelligence: i64, memory: i64, perception: i64, willpower: i64) -> CharacterAttributes {
    CharacterAttributes {
      charisma,
      intelligence,
      memory,
      perception,
      willpower,
      ..attributes_row()
    }
  }

  fn weights() -> Vec<PairWeight> {
    vec![PairWeight {
      primary: Attribute::Perception,
      secondary: Attribute::Willpower,
      sp: 1_000_000,
    }]
  }

  #[test]
  fn it_recommends_a_remap_for_a_no_implant_pilot_without_a_stale_notice() {
    let model = AttrTabModel::new(
      &attributes_row(),
      Attributes::default(),
      Some((Attribute::Perception, Attribute::Willpower)),
      &weights(),
    );

    assert!(model.consistent);
    assert_eq!(model.booster_n, 0);
    assert!(
      !model.recommendation.is_current,
      "a per/wil queue improves on this base"
    );

    let _el: Element<'_, ()> = view(Some(&model), now());
  }

  #[test]
  fn it_recovers_the_base_for_an_implant_only_pilot() {
    let row = stored_row(22, 26, 25, 27, 24);

    let model = AttrTabModel::new(
      &row,
      implants_uniform(),
      Some((Attribute::Perception, Attribute::Willpower)),
      &weights(),
    );

    assert!(model.consistent);
    assert_eq!(model.booster_n, 0);
    assert_eq!(model.base, base_row());
    assert!(!model.recommendation.is_current);

    let _el: Element<'_, ()> = view(Some(&model), now());
  }

  #[test]
  fn it_peels_a_booster_for_a_boosted_pilot() {
    let row = stored_row(36, 37, 37, 37, 37);

    let model = AttrTabModel::new(&row, implants_uniform(), None, &weights());

    assert!(model.consistent);
    assert_eq!(model.booster_n, 12);
    assert_eq!(
      model.base,
      Attributes {
        charisma: 19,
        intelligence: 20,
        memory: 20,
        perception: 20,
        willpower: 20,
      }
    );

    let _el: Element<'_, ()> = view(Some(&model), now());
  }

  #[test]
  fn it_flags_inconsistent_data_and_keeps_raw_stored_values() {
    let model = AttrTabModel::new(&attributes_row(), implants_uniform(), None, &weights());

    assert!(!model.consistent);
    assert_eq!(model.booster_n, 0);
    assert_eq!(model.implants, Attributes::default());
    assert_eq!(model.base, base_row());

    let _el: Element<'_, ()> = view(Some(&model), now());
  }

  #[test]
  fn it_renders_the_awaiting_state_with_no_model() {
    let _el: Element<'_, ()> = view(None, now());
  }

  fn base_row() -> Attributes {
    Attributes {
      charisma: 17,
      intelligence: 21,
      memory: 20,
      perception: 22,
      willpower: 19,
    }
  }

  fn implants_uniform() -> Attributes {
    Attributes {
      charisma: 5,
      intelligence: 5,
      memory: 5,
      perception: 5,
      willpower: 5,
    }
  }
}
