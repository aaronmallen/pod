use iced::{
  Element, Length,
  widget::{Column, text},
};

use super::{WARNING, attribute_short, card, value_of};
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
  let last = days
    .last_remap_days
    .map_or_else(|| "no prior remap".to_owned(), |d| format!("last remap {d}d ago"));
  let cooldown = match days.cooldown_days {
    Some(0) | None => "available now".to_owned(),
    Some(d) => format!("cooldown {d}d"),
  };
  let bonus = bonus_remaps.max(0);
  let noun = if bonus == 1 { "remap" } else { "remaps" };

  let copy = Column::with_children(vec![
    eyebrow("Neural remap", Some(color::accent::PLASMA)),
    text(format!("{bonus} bonus {noun} available"))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("{last} · annual {cooldown}"))
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
    color::with_alpha(color::accent::PLASMA, 0.08),
    color::with_alpha(color::accent::PLASMA, 0.25),
  )
}

pub fn recommendation_card<'a, Message: 'a>(model: &AttrTabModel) -> Element<'a, Message> {
  let rec = &model.recommendation;

  let mut children: Vec<Element<'a, Message>> = vec![eyebrow(
    "Fastest remap for your current queue",
    Some(color::accent::PLASMA),
  )];

  if rec.is_current {
    children.push(
      text("Already optimal — no remap improves your current queue.")
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    );
  } else {
    children.push(distribution_line(rec.base));
    children.push(
      text(format!(
        "Queue completes in {}",
        fmt_duration(rec.total_sec.round() as i64)
      ))
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
        children.push(
          text(format!("Saves {} vs current", fmt_duration(saved)))
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

  if rec.current_out_of_spec {
    children.push(
      text("Your current attributes are out of spec; suggestion shown is the fastest legal allocation.")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(WARNING),
        })
        .into(),
    );
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
