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

  let total = total.to_string();
  Column::with_children(vec![
    eyebrow(
      &t!("skills.panel_attributes.section_title"),
      Some(color::text::secondary()),
    ),
    text(t!("skills.panel_attributes.section_summary", total => total).into_owned())
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
