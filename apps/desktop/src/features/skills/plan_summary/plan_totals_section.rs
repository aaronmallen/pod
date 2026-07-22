use chrono::{DateTime, Utc};
use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, text},
};

use super::{fmt_sp, fmt_time_short};
use crate::{
  features::skills::fmt_eta,
  ui::style::{color, spacing, typography},
};

pub(crate) fn plan_totals_section<'a, M: 'a>(
  total_sec: f64,
  total_sp: u64,
  steps: usize,
  is_template: bool,
  now: DateTime<Utc>,
) -> Element<'a, M> {
  let mut rows: Vec<Element<'a, M>> = Vec::new();
  rows.push(
    text(fmt_time_short(total_sec))
      .font(typography::mono::MEDIUM)
      .size(28.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  );
  rows.push(Space::new().height(2.0).into());
  rows.push(
    text(fmt_sp(total_sp))
      .font(typography::mono::MEDIUM)
      .size(16.0)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  );
  rows.push(Space::new().height(4.0).into());
  rows.push(
    text(t!("skills.summary_totals.steps", count => steps))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  );
  if !is_template {
    let completion = t!(
      "skills.summary_totals.completes",
      eta => fmt_eta(now, total_sec as i64)
    )
    .into_owned();
    rows.push(Space::new().height(2.0).into());
    rows.push(
      text(completion)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let col = column(rows).width(Length::Fill);

  container(col)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .width(Length::Fill)
    .into()
}
