//! Plan totals section — time, SP, step count, and completion date.

use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, text},
};

use super::{super::Message, fmt_sp, fmt_time_short};
use crate::style::{color, spacing, typography::mono};

fn completion_date_string(total_sec: f64) -> String {
  if total_sec <= 0.0 {
    return "\u{2014}".to_string();
  }
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let ts = now + total_sec as u64;
  let hh = (ts % 86400) / 3600;
  let mm = (ts % 3600) / 60;
  let days = ts / 86400;
  let (_, month, day) = days_to_utc_date(days);
  const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  ];
  format!("{} {} \u{00b7} {:02}:{:02}", day, MONTHS[month as usize - 1], hh, mm)
}

fn days_to_utc_date(days: u64) -> (u32, u8, u8) {
  let z = days as i64 + 719468;
  let era = z / 146097;
  let doe = (z - era * 146097) as u64;
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
  let y = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = doy - (153 * mp + 2) / 5 + 1;
  let m = if mp < 10 { mp + 3 } else { mp - 9 };
  let y = if m <= 2 { y + 1 } else { y };
  (y as u32, m as u8, d as u8)
}

fn totals_column(
  time_str: String,
  sp_str: String,
  steps_str: String,
  completion_str: String,
) -> iced::widget::Column<'static, Message> {
  column([
    text(time_str)
      .font(mono::MEDIUM)
      .size(28.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(sp_str)
      .font(mono::MEDIUM)
      .size(16.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text(steps_str)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(completion_str)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .width(Length::Fill)
}

/// Builder for the plan totals section.
pub struct PlanTotalsSection {
  /// Total plan time in seconds.
  pub total_sec: f64,
  /// Total skill points in the plan.
  pub total_sp: u64,
  /// Number of non-skipped steps.
  pub steps: usize,
}

impl PlanTotalsSection {
  /// Create a new `PlanTotalsSection`.
  pub fn new(total_sec: f64, total_sp: u64, steps: usize) -> Self {
    Self {
      steps,
      total_sec,
      total_sp,
    }
  }

  /// Render the section into an [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    let time_str = fmt_time_short(self.total_sec);
    let sp_str = fmt_sp(self.total_sp);
    let steps_str = format!("{} steps", self.steps);
    let completion_str = format!("Completes {}", completion_date_string(self.total_sec));

    container(totals_column(time_str, sp_str, steps_str, completion_str))
      .padding(Padding {
        top: spacing::SPACE_4,
        bottom: spacing::SPACE_4,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}
