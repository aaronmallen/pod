use chrono::{DateTime, Utc};
use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, text},
};

use super::{super::Message, fmt_sp, fmt_time_short};
use crate::ui::style::{color, spacing, typography};

pub(super) fn plan_totals_section(
  total_sec: f64,
  total_sp: u64,
  steps: usize,
  now: DateTime<Utc>,
) -> Element<'static, Message> {
  let completion = t!(
    "skills.summary_totals.completes",
    eta => super::super::fmt_eta(now, total_sec as i64)
  )
  .into_owned();

  let col = column(vec![
    text(fmt_time_short(total_sec))
      .font(typography::mono::MEDIUM)
      .size(28.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(fmt_sp(total_sp))
      .font(typography::mono::MEDIUM)
      .size(16.0)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().height(4.0).into(),
    text(t!("skills.summary_totals.steps", count => steps))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(completion)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .width(Length::Fill);

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
