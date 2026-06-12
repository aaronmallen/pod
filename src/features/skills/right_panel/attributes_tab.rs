pub mod attr_row;
pub mod rate_grid;
pub mod remap_card;
pub mod section_header;

use chrono::{DateTime, Utc};
use iced::{
  Element, Length, Padding,
  widget::{Column, Space, container, scrollable, text},
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
pub(super) const PAIR_LABELS: [&str; 6] = ["Combat", "Engineering", "Drones", "Navigation", "Trade", "Social"];
pub(super) const WARNING: iced::Color = color::status::WARNING;

pub fn view<'a, Message: 'a>(model: Option<&'a AttrTabModel>, now: DateTime<Utc>) -> Element<'a, Message> {
  let Some(model) = model else {
    return awaiting_state();
  };

  let effective = effective_of(model);
  let rows = project_rows(model.base, model.implants, model.active);
  let matrix = sp_per_hr_matrix(effective, model.active);
  let days = remap_days(
    now,
    model.last_remap_date.as_deref(),
    model.accrued_remap_cooldown_date.as_deref(),
  );

  let mut children: Vec<Element<'a, Message>> = vec![section_header::section_header(model.base)];
  for (index, row) in rows.iter().enumerate() {
    children.push(attr_row::attr_row(*row, index == 0));
  }
  children.push(rate_grid::rate_grid(&matrix));
  children.push(remap_card::remap_cta(model.bonus_remaps, days));
  children.push(remap_card::recommendation_card(model));
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
    charisma: model.base.charisma + model.implants.charisma,
    intelligence: model.base.intelligence + model.implants.intelligence,
    memory: model.base.memory + model.implants.memory,
    perception: model.base.perception + model.implants.perception,
    willpower: model.base.willpower + model.implants.willpower,
  }
}

fn awaiting_state<'a, Message: 'a>() -> Element<'a, Message> {
  container(
    text("Neural attributes will appear once synced.")
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

pub(super) fn attribute_label(attribute: Attribute) -> &'static str {
  match attribute {
    Attribute::Charisma => "Charisma",
    Attribute::Intelligence => "Intelligence",
    Attribute::Memory => "Memory",
    Attribute::Perception => "Perception",
    Attribute::Willpower => "Willpower",
  }
}

pub(super) fn attribute_short(attribute: Attribute) -> &'static str {
  match attribute {
    Attribute::Charisma => "Cha",
    Attribute::Intelligence => "Int",
    Attribute::Memory => "Mem",
    Attribute::Perception => "Per",
    Attribute::Willpower => "Wil",
  }
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

  fn implants() -> Attributes {
    Attributes {
      charisma: 3,
      intelligence: 4,
      memory: 4,
      perception: 5,
      willpower: 5,
    }
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

  fn weights() -> Vec<PairWeight> {
    vec![PairWeight {
      primary: Attribute::Perception,
      secondary: Attribute::Willpower,
      sp: 1_000_000,
    }]
  }

  #[test]
  fn it_renders_the_awaiting_state_with_no_model() {
    let _el: Element<'_, ()> = view(None, now());
  }

  #[test]
  fn it_renders_a_recommendation_when_a_remap_helps() {
    let model = AttrTabModel::new(
      &attributes_row(),
      implants(),
      Some((Attribute::Perception, Attribute::Willpower)),
      &weights(),
    );

    assert!(
      !model.recommendation.is_current,
      "a per/wil queue improves on this base"
    );
    let _el: Element<'_, ()> = view(Some(&model), now());
  }

  #[test]
  fn it_renders_the_already_optimal_state() {
    let mut row = attributes_row();
    row.perception = 27;
    row.willpower = 21;
    row.charisma = 17;
    row.intelligence = 17;
    row.memory = 17;
    let model = AttrTabModel::new(
      &row,
      Attributes::default(),
      Some((Attribute::Perception, Attribute::Willpower)),
      &weights(),
    );

    assert!(model.recommendation.is_current);
    let _el: Element<'_, ()> = view(Some(&model), now());
  }

  #[test]
  fn it_flags_an_out_of_spec_current_base() {
    let mut row = attributes_row();
    row.charisma = 31;
    row.intelligence = 17;
    row.memory = 17;
    row.perception = 17;
    row.willpower = 17;
    let model = AttrTabModel::new(&row, Attributes::default(), None, &weights());

    assert!(model.recommendation.current_out_of_spec);
    let _el: Element<'_, ()> = view(Some(&model), now());
  }
}
