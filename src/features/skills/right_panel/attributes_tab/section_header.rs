use iced::{
  Element, Padding,
  widget::{Column, text},
};

use crate::{
  features::skills::optimizer::Attributes,
  ui::{
    components::eyebrow::eyebrow,
    style::{color, spacing, typography},
  },
};

pub fn section_header<'a, Message: 'a>(base: Attributes) -> Element<'a, Message> {
  let total: u32 = [
    base.charisma,
    base.intelligence,
    base.memory,
    base.perception,
    base.willpower,
  ]
  .into_iter()
  .sum();

  Column::with_children(vec![
    eyebrow("Neural attributes", Some(color::text::SECONDARY)),
    text(format!("{total} pts allocated · base"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .padding(Padding {
    top: 0.0,
    right: 0.0,
    bottom: spacing::SPACE_2,
    left: 0.0,
  })
  .into()
}
