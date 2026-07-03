use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::{fmt_time_long, section_label};
use crate::{
  features::skills::{
    browse::AttrKey,
    optimizer::{Attributes, Recommendation},
  },
  ui::{
    components::status,
    style::{color, spacing, typography},
  },
};

fn attr_value(attrs: Attributes, key: AttrKey) -> u32 {
  match key {
    AttrKey::Charisma => attrs.charisma,
    AttrKey::Intelligence => attrs.intelligence,
    AttrKey::Memory => attrs.memory,
    AttrKey::Perception => attrs.perception,
    AttrKey::Willpower => attrs.willpower,
  }
}

pub(crate) fn attr_optimization_section<'a, M: 'a>(
  base_attrs: Attributes,
  current_base_sec: f64,
  recommendation: &Recommendation,
  remap_availability: u32,
  remap_reason: &str,
  is_template: bool,
) -> Element<'a, M> {
  let current_label = if is_template {
    t!("skills.summary_attr.unmapped")
  } else {
    t!("skills.summary_attr.current")
  };

  let mut items: Vec<Element<'a, M>> = vec![
    section_label(&t!("skills.summary_attr.heading")),
    Space::new().height(spacing::SPACE_3).into(),
  ];

  if recommendation.is_current {
    items.push(attr_column(&current_label, base_attrs, false));
    items.push(Space::new().height(spacing::SPACE_3).into());
    items.push(already_optimal_callout());
  } else {
    items.push(dual_columns(&current_label, base_attrs, recommendation.base));
    items.push(Space::new().height(spacing::SPACE_3).into());
    items.push(savings_callout(current_base_sec, recommendation.total_sec));
  }

  if !is_template {
    items.push(Space::new().height(spacing::SPACE_3).into());
    items.push(remap_status_row(remap_availability, remap_reason));
  }

  container(column(items).width(Length::Fill))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .width(Length::Fill)
    .into()
}

fn dual_columns<'a, M: 'a>(current_label: &str, current: Attributes, proposed: Attributes) -> Element<'a, M> {
  row(vec![
    attr_column(current_label, current, false),
    Space::new().width(8.0).into(),
    attr_column(&t!("skills.summary_attr.proposed"), proposed, true),
  ])
  .width(Length::Fill)
  .into()
}

fn attr_column<'a, M: 'a>(title: &str, attrs: Attributes, highlight: bool) -> Element<'a, M> {
  let header = text(title.to_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(if highlight {
        color::accent()
      } else {
        color::text::tertiary()
      }),
    });

  let mut rows: Vec<Element<'a, M>> = vec![header.into(), Space::new().height(6.0).into()];
  for key in AttrKey::ALL {
    rows.push(attr_value_row(key, attr_value(attrs, key), highlight));
  }

  let (bg, border_color) = if highlight {
    (color::with_alpha(color::accent(), 0.08), color::accent_muted())
  } else {
    (color::surface::SUNKEN, color::with_alpha(color::text::PRIMARY, 0.1))
  };

  container(column(rows).spacing(2.0).width(Length::Fill))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 10.0,
      right: 10.0,
    })
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: border_color,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn attr_value_row<'a, M: 'a>(key: AttrKey, value: u32, highlight: bool) -> Element<'a, M> {
  let value_color = if highlight {
    color::accent()
  } else {
    color::text::PRIMARY
  };

  row(vec![
    text(key.short())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .width(Length::Fixed(28.0))
      .into(),
    text(key.label())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .width(Length::Fill)
      .into(),
    text(value.to_string())
      .font(typography::mono::MEDIUM)
      .size(12.0)
      .style(move |_| text::Style {
        color: Some(value_color),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(4.0)
  .into()
}

fn already_optimal_callout<'a, M: 'a>() -> Element<'a, M> {
  callout(
    t!("skills.summary_attr.already_optimal").into_owned(),
    color::with_alpha(color::status::ONLINE, 0.08),
    color::with_alpha(color::status::ONLINE, 0.30),
    color::status::ONLINE,
  )
}

fn savings_callout<'a, M: 'a>(current_sec: f64, proposed_sec: f64) -> Element<'a, M> {
  let saved = (current_sec - proposed_sec).max(0.0);
  callout(
    format!("\u{2212}{}", fmt_time_long(saved)),
    color::with_alpha(color::accent(), 0.08),
    color::accent_muted(),
    color::accent(),
  )
}

fn callout<'a, M: 'a>(label: String, bg: Color, border_color: Color, label_color: Color) -> Element<'a, M> {
  container(
    text(label)
      .font(typography::mono::MEDIUM)
      .size(13.0)
      .style(move |_| text::Style {
        color: Some(label_color),
      }),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 14.0,
    right: 14.0,
  })
  .width(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: border_color,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn remap_status_row<'a, M: 'a>(remap_availability: u32, remap_reason: &str) -> Element<'a, M> {
  let (dot_color, status_text) = if remap_availability > 0 {
    (
      color::status::ONLINE,
      t!("skills.summary_attr.remap_available").into_owned(),
    )
  } else if remap_reason.is_empty() {
    (
      color::text::tertiary(),
      t!("skills.summary_attr.no_remap_available").into_owned(),
    )
  } else {
    (color::text::tertiary(), remap_reason.to_owned())
  };

  row(vec![
    status::dot_sized(dot_color, 6.0),
    Space::new().width(6.0).into(),
    text(status_text)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(dot_color),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}
