use iced::{
  Element, Length,
  widget::{Column, text},
};

use super::{attribute_short, card, value_of};
use crate::{
  features::skills::{
    attributes::{AttrTabModel, RemapDays},
    fmt_duration,
    optimizer::Attributes,
  },
  ui::{
    components::eyebrow::eyebrow,
    style::{color, spacing, typography},
  },
};

const SUCCESS: iced::Color = color::status::ONLINE;

pub fn remap_cta<'a, Message: 'a>(bonus_remaps: i64, days: RemapDays) -> Element<'a, Message> {
  let last = days.last_remap_days.map_or_else(
    || t!("skills.panel_attributes.remap_last_none").into_owned(),
    |d| {
      let days = d.to_string();
      t!("skills.panel_attributes.remap_last_ago", days => days).into_owned()
    },
  );
  let cooldown = match days.cooldown_days {
    Some(0) | None => t!("skills.panel_attributes.remap_cooldown_available").into_owned(),
    Some(d) => {
      let days = d.to_string();
      t!("skills.panel_attributes.remap_cooldown_days", days => days).into_owned()
    }
  };
  let bonus = bonus_remaps.max(0);
  let bonus_count = bonus.to_string();
  let bonus_key = if bonus == 1 {
    "skills.panel_attributes.remap_bonus_one"
  } else {
    "skills.panel_attributes.remap_bonus_other"
  };

  let copy = Column::with_children(vec![
    eyebrow(&t!("skills.panel_attributes.remap_title"), Some(color::accent())),
    text(t!(bonus_key, count => bonus_count).into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(t!("skills.panel_attributes.remap_status", last => last, cooldown => cooldown).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  card(
    copy.into(),
    color::with_alpha(color::accent(), 0.08),
    color::with_alpha(color::accent(), 0.25),
  )
}

pub fn recommendation_card<'a, Message: 'a>(model: &AttrTabModel) -> Element<'a, Message> {
  let rec = &model.recommendation;

  let mut children: Vec<Element<'a, Message>> = vec![eyebrow(
    &t!("skills.panel_attributes.recommendation_title"),
    Some(color::accent()),
  )];

  if rec.is_current {
    children.push(
      text(t!("skills.panel_attributes.recommendation_optimal"))
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    );
  } else {
    children.push(distribution_line(rec.base));
    let duration = fmt_duration(rec.total_sec.round() as i64);
    children.push(
      text(t!("skills.panel_attributes.recommendation_completes", duration => duration).into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    );
    if model.current_total_sec.is_finite() && rec.total_sec.is_finite() {
      let saved = (model.current_total_sec - rec.total_sec).round() as i64;
      if saved > 0 {
        let duration = fmt_duration(saved);
        children.push(
          text(t!("skills.panel_attributes.recommendation_saves", duration => duration).into_owned())
            .font(typography::mono::MEDIUM)
            .size(typography::size::SM)
            .style(|_| text::Style {
              color: Some(SUCCESS),
            })
            .into(),
        );
      }
    }
  }

  let body = Column::with_children(children)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  card(
    body.into(),
    color::surface::SUNKEN,
    color::with_alpha(color::text::PRIMARY, 0.1),
  )
}

fn distribution_line<'a, Message: 'a>(base: Attributes) -> Element<'a, Message> {
  let parts: Vec<String> = super::ATTR_ORDER
    .iter()
    .map(|&attr| format!("{} {}", attribute_short(attr), value_of(base, attr)))
    .collect();

  text(parts.join(" · "))
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into()
}
