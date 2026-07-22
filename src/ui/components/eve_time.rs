use chrono::{DateTime, Utc};
use iced::{
  Element,
  alignment::Vertical,
  widget::{Row, text},
};

use crate::ui::style::{color, spacing, typography};

pub fn eve_time<'a, M>(now: DateTime<Utc>) -> Element<'a, M>
where
  M: 'a,
{
  let clock = now.format("%H:%M:%S").to_string();

  Row::with_children(vec![
    text(t!("common.eve_time.label"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
    text(clock)
      .font(typography::mono::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}
