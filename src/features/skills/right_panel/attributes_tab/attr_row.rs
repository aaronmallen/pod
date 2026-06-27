use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use super::{attribute_label, attribute_short};
use crate::{
  features::skills::attributes::{AttrRow, Role},
  ui::{
    components::{eyebrow::eyebrow, rule},
    style::{color, radius, spacing, typography},
  },
};

const MAX_ATTR: u32 = 35;
const SUCCESS: iced::Color = color::status::ONLINE;

pub fn attr_row<'a, Message: 'a>(row: AttrRow, first: bool) -> Element<'a, Message> {
  let accent = match row.role {
    Role::Primary => color::accent::PLASMA,
    Role::Secondary => color::with_alpha(color::accent::PLASMA, 0.7),
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

fn attr_bar<'a, Message: 'a>(row: AttrRow, accent: iced::Color) -> Element<'a, Message> {
  let base_fill = (f64::from(row.base) / f64::from(MAX_ATTR)).clamp(0.0, 1.0);
  let implant_fill = (f64::from(row.implant) / f64::from(MAX_ATTR)).clamp(0.0, 1.0 - base_fill);
  let remainder = (1.0 - base_fill - implant_fill).max(0.0);

  let base_opacity = match row.role {
    Role::Primary => 1.0,
    Role::Secondary => 0.85,
    Role::None => 0.6,
  };

  let base_seg = container(Space::new())
    .width(Length::FillPortion((base_fill * 1_000.0) as u16))
    .height(Length::Fixed(8.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, base_opacity))),
      ..container::Style::default()
    });
  let implant_seg = container(Space::new())
    .width(Length::FillPortion((implant_fill * 1_000.0) as u16))
    .height(Length::Fixed(8.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.30))),
      ..container::Style::default()
    });
  let rest = container(Space::new()).width(Length::FillPortion((remainder * 1_000.0) as u16));

  container(Row::with_children(vec![
    base_seg.into(),
    implant_seg.into(),
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
  Some(eyebrow(&label, Some(color::accent::PLASMA)))
}
