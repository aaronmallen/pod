//! Individual plan card with open and delete actions.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};
use pod_model::SkillPlan;

use super::Message;
use crate::style::{
  color, spacing,
  typography::{body, mono},
};

pub struct Component<'a> {
  plan: &'a SkillPlan,
  is_first: bool,
  confirm_delete: bool,
}

impl<'a> Component<'a> {
  pub fn new(plan: &'a SkillPlan, is_first: bool, confirm_delete: bool) -> Self {
    Self {
      plan,
      is_first,
      confirm_delete,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    use crate::components;

    let plan_id = self.plan.id.clone();
    let entry_count = self.plan.entries.iter().filter(|e| !e.auto).count();
    let updated = fmt_plan_date(self.plan.updated_at);

    let badge_el = badge(entry_count);
    let info_el = info_col(self.plan.name.clone(), updated, entry_count, self.plan.entries.len());

    let actions: Element<'static, Message> = if self.confirm_delete {
      confirm_row(plan_id)
    } else {
      action_row(plan_id)
    };

    let content =
      container(row([badge_el, Space::new().width(14.0).into(), info_el, actions]).align_y(Vertical::Center))
        .padding(Padding {
          top: 14.0,
          bottom: 14.0,
          left: spacing::SPACE_4,
          right: spacing::SPACE_4,
        })
        .width(Length::Fill);

    if self.is_first {
      content.into()
    } else {
      column([components::Separator::horizontal().render(), content.into()]).into()
    }
  }
}

fn info_col(name: String, updated: String, user_count: usize, _total_count: usize) -> Element<'static, Message> {
  let subtitle = format!("{} skills \u{00b7} {}", user_count, updated);

  column([
    text(name)
      .font(body::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(subtitle)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn badge(entry_count: usize) -> Element<'static, Message> {
  container(
    text(entry_count.to_string())
      .font(mono::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(36.0)
  .height(36.0)
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(Vertical::Center)
  .style(|_| container::Style {
    background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
    border: Border {
      color: color::accent::PLASMA_MUTED,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn action_row(plan_id: String) -> Element<'static, Message> {
  let open_id = plan_id.clone();
  let delete_id = plan_id;

  row([
    open_btn(open_id),
    Space::new().width(spacing::SPACE_2).into(),
    delete_btn(delete_id),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn confirm_row(plan_id: String) -> Element<'static, Message> {
  let confirm_id = plan_id.clone();
  let cancel_id = plan_id;

  row([
    text("Delete?")
      .font(body::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    confirm_delete_btn(confirm_id),
    Space::new().width(4.0).into(),
    cancel_delete_btn(cancel_id),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn open_btn(plan_id: String) -> Element<'static, Message> {
  button(
    text("Open")
      .font(body::MEDIUM)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::OpenPlan(plan_id))
  .style(|_, status| button::Style {
    background: None,
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA,
        _ => color::border::SUBTLE,
      },
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA,
      _ => color::text::PRIMARY,
    },
    ..button::Style::default()
  })
  .into()
}

fn delete_btn(plan_id: String) -> Element<'static, Message> {
  button(
    text("\u{00d7}")
      .font(body::REGULAR)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::DeleteRequested(plan_id))
  .style(|_, status| button::Style {
    background: None,
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::status::DANGER,
        _ => Color::TRANSPARENT,
      },
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::status::DANGER,
      _ => color::text::TERTIARY,
    },
    ..button::Style::default()
  })
  .into()
}

fn confirm_delete_btn(plan_id: String) -> Element<'static, Message> {
  button(
    text("Confirm")
      .font(body::MEDIUM)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::DeleteConfirmed(plan_id))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::status::DANGER_SUBTLE)),
      _ => None,
    },
    border: Border {
      color: color::status::DANGER,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
  .into()
}

fn cancel_delete_btn(_plan_id: String) -> Element<'static, Message> {
  button(
    text("Cancel")
      .font(body::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::DeleteCancelled)
  .style(|_, status| button::Style {
    background: None,
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::border::DEFAULT,
        _ => color::border::SUBTLE,
      },
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}

fn fmt_plan_date(unix_ms: i64) -> String {
  let secs = (unix_ms / 1000).max(0) as u64;
  let days = secs / 86400;
  let (year, month, day) = days_to_utc_date(days);
  const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  ];
  let month_str = MONTHS[(month as usize).saturating_sub(1).min(11)];
  let year_short = year % 100;
  format!("{} {} '{:02}", day, month_str, year_short)
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
