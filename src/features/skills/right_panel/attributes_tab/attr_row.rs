use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use super::{attribute_label, attribute_short};
use crate::{
  features::skills::attributes::{AttrRow, Role},
  ui::{
    components::{eyebrow::eyebrow, progress_bar::portion, rule},
    style::{color, radius, spacing, typography},
  },
};

const BOOSTER: iced::Color = color::status::WARNING;
const MAX_ATTR: u32 = 44;
const SUCCESS: iced::Color = color::status::ONLINE;

pub fn attr_row<'a, Message: 'a>(row: AttrRow, first: bool) -> Element<'a, Message> {
  let accent = match row.role {
    Role::Primary => color::accent(),
    Role::Secondary => color::with_alpha(color::accent(), 0.7),
    Role::None => color::text::PRIMARY,
  };

  let mut label_children: Vec<Element<'a, Message>> = vec![
    text(attribute_label(row.attribute))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(badge) = role_badge(row.role) {
    label_children.push(badge);
  }
  let label = Row::with_children(label_children)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

  let mut values: Vec<Element<'a, Message>> = vec![
    text(row.base.to_string())
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if row.implant > 0 {
    values.push(
      text(format!("+{}", row.implant))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(SUCCESS),
        })
        .into(),
    );
  }
  if row.booster > 0 {
    values.push(
      text(format!("+{}", row.booster))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(BOOSTER),
        })
        .into(),
    );
  }
  let value_row = Row::with_children(values)
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Bottom);

  let top =
    Row::with_children(vec![container(label).width(Length::Fill).into(), value_row.into()]).align_y(Vertical::Center);

  let bar = Row::with_children(vec![
    text(attribute_short(row.attribute))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .width(Length::Fixed(34.0))
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    container(attr_bar(row, accent)).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let inner = Column::with_children(vec![top.into(), bar.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  let body = container(inner).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3,
    right: 0.0,
    bottom: spacing::SPACE_3,
    left: 0.0,
  });

  if first {
    return body.into();
  }
  Column::with_children(vec![rule::horizontal(), body.into()])
    .width(Length::Fill)
    .into()
}

fn attr_bar_widths(row: AttrRow) -> [Length; 4] {
  let base_fill = (f64::from(row.base) / f64::from(MAX_ATTR)).clamp(0.0, 1.0);
  let implant_fill = (f64::from(row.implant) / f64::from(MAX_ATTR)).clamp(0.0, 1.0 - base_fill);
  let booster_fill = (f64::from(row.booster) / f64::from(MAX_ATTR)).clamp(0.0, 1.0 - base_fill - implant_fill);
  let remainder = (1.0 - base_fill - implant_fill - booster_fill).max(0.0);

  [
    portion((base_fill * 1_000.0) as u16),
    portion((implant_fill * 1_000.0) as u16),
    portion((booster_fill * 1_000.0) as u16),
    portion((remainder * 1_000.0) as u16),
  ]
}

fn attr_bar<'a, Message: 'a>(row: AttrRow, accent: iced::Color) -> Element<'a, Message> {
  let [base_width, implant_width, booster_width, remainder_width] = attr_bar_widths(row);

  let base_opacity = match row.role {
    Role::Primary => 1.0,
    Role::Secondary => 0.85,
    Role::None => 0.6,
  };

  let base_seg = container(Space::new())
    .width(base_width)
    .height(Length::Fixed(8.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, base_opacity))),
      ..container::Style::default()
    });
  let implant_seg = container(Space::new())
    .width(implant_width)
    .height(Length::Fixed(8.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.30))),
      ..container::Style::default()
    });
  let booster_seg = container(Space::new())
    .width(booster_width)
    .height(Length::Fixed(8.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(BOOSTER, 0.45))),
      ..container::Style::default()
    });
  let rest = container(Space::new()).width(remainder_width);

  container(Row::with_children(vec![
    base_seg.into(),
    implant_seg.into(),
    booster_seg.into(),
    rest.into(),
  ]))
  .width(Length::Fill)
  .height(Length::Fixed(8.0))
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn role_badge<'a, Message: 'a>(role: Role) -> Option<Element<'a, Message>> {
  let label = match role {
    Role::Primary => t!("skills.panel_attributes.role_primary"),
    Role::Secondary => t!("skills.panel_attributes.role_secondary"),
    Role::None => return None,
  };
  Some(eyebrow(&label, Some(color::accent())))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::skills::optimizer::Attribute;

  mod attr_bar_widths {
    use super::*;

    fn row(base: u32, implant: u32, booster: u32) -> AttrRow {
      AttrRow {
        attribute: Attribute::Perception,
        base,
        booster,
        effective: base + implant + booster,
        fill: 0.0,
        implant,
        role: Role::Primary,
      }
    }

    fn assert_no_fill_portion_zero(widths: [Length; 4]) {
      for width in widths {
        assert_ne!(width, Length::FillPortion(0));
      }
    }

    #[test]
    fn it_never_emits_fill_portion_zero_at_max_attr() {
      assert_no_fill_portion_zero(attr_bar_widths(row(27, 5, 12)));
    }

    #[test]
    fn it_never_emits_fill_portion_zero_with_zero_implant() {
      assert_no_fill_portion_zero(attr_bar_widths(row(20, 0, 8)));
    }

    #[test]
    fn it_never_emits_fill_portion_zero_with_zero_booster() {
      assert_no_fill_portion_zero(attr_bar_widths(row(20, 6, 0)));
    }

    #[test]
    fn it_zeroes_the_remainder_at_max_attr() {
      let [_, _, _, remainder] = attr_bar_widths(row(27, 5, 12));
      assert_eq!(remainder, Length::Fixed(0.0));
    }
  }
}
