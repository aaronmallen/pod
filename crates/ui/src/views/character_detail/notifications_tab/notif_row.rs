//! Notification row component: read/unread entry with icon, title, snippet, and timestamp.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, text},
};
use pod_model::CharacterNotification;

use super::notif_icon_box;
use crate::{
  style::{color, typography::mono},
  views::character_detail::Message,
};

/// Builder for a single notification row.
pub struct Component<'a> {
  is_last: bool,
  notif: &'a CharacterNotification,
}

impl<'a> Component<'a> {
  /// Creates a new notification row for the given notification.
  pub fn new(notif: &'a CharacterNotification, is_last: bool) -> Self {
    Self {
      is_last,
      notif,
    }
  }

  /// Renders the notification row.
  pub fn render(self) -> Element<'a, Message> {
    if !self.notif.is_read {
      notif_row_unread(self.notif, self.is_last)
    } else {
      notif_row_read(self.notif, self.is_last)
    }
  }
}

fn format_notif_type(notif_type: &str) -> String {
  notif_type.chars().fold(String::new(), |mut acc, c| {
    if c.is_uppercase() && !acc.is_empty() {
      acc.push(' ');
    }
    acc.push(c);
    acc
  })
}

fn notif_body_snippet(notif: &CharacterNotification) -> Option<String> {
  notif.text.as_deref().map(|t| {
    let first_line = t.lines().next().unwrap_or("").trim();
    if first_line.len() > 80 {
      format!("{}…", &first_line[..80])
    } else {
      first_line.to_string()
    }
  })
}

fn notif_content_col<'a>(notif: &'a CharacterNotification) -> Element<'a, Message> {
  let cat_color = notif_icon_box::category_color(&notif.category);
  let is_unread = !notif.is_read;
  let title_weight = if is_unread { mono::MEDIUM } else { mono::REGULAR };
  let mut items: Vec<Element<'_, Message>> = vec![
    text(notif.category.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(cat_color),
      })
      .into(),
    text(format_notif_type(&notif.type_))
      .font(title_weight)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(snippet) = notif_body_snippet(notif)
    && !snippet.is_empty()
  {
    items.push(
      text(snippet)
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    );
  }
  column(items).spacing(3.0).width(Length::Fill).into()
}

fn notif_inner_row<'a>(notif: &'a CharacterNotification, is_unread: bool) -> Element<'a, Message> {
  let icon_box = notif_icon_box::Component::new(&notif.category).render();
  let content = notif_content_col(notif);
  let time_label = relative_time(&notif.timestamp);
  let timestamp_el =
    container(
      text(time_label)
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        }),
    )
    .align_x(iced::alignment::Horizontal::Right)
    .into();
  row([icon_box, content, timestamp_el])
    .spacing(14.0)
    .align_y(iced::alignment::Vertical::Top)
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: if is_unread { 14.0 } else { 16.0 },
      right: 16.0,
    })
    .into()
}

fn notif_row_read<'a>(notif: &'a CharacterNotification, is_last: bool) -> Element<'a, Message> {
  let inner_row = notif_inner_row(notif, false);
  container(inner_row)
    .width(Length::Fill)
    .style(move |_| container::Style {
      border: Border {
        color: if is_last {
          Color::TRANSPARENT
        } else {
          color::border::SUBTLE
        },
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn notif_row_unread<'a>(notif: &'a CharacterNotification, is_last: bool) -> Element<'a, Message> {
  let notif_id = notif.notification_id;
  let plasma_bar = container(Space::new().width(2.0).height(Length::Fill))
    .width(2.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      ..container::Style::default()
    });
  let inner_row = notif_inner_row(notif, true);
  button(
    row([plasma_bar.into(), inner_row])
      .width(Length::Fill)
      .height(Length::Shrink),
  )
  .width(Length::Fill)
  .padding(0)
  .style(move |_, _| button::Style {
    background: Some(Background::Color(color::accent::PLASMA_SELECTED)),
    border: Border {
      color: if is_last {
        Color::TRANSPARENT
      } else {
        color::border::SUBTLE
      },
      width: 1.0,
      radius: 0.0.into(),
    },
    ..button::Style::default()
  })
  .on_press(Message::NotificationRead(notif_id))
  .into()
}

fn days_since_epoch(y: i64, m: i64, d: i64) -> i64 {
  let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let doy = (153 * m + 2) / 5 + d - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146097 + doe - 719468
}

fn parse_iso8601(s: &str) -> Result<i64, ()> {
  let s = s.trim_end_matches('Z').trim_end_matches('+').trim();
  let parts: Vec<&str> = s.splitn(2, 'T').collect();
  if parts.len() != 2 {
    return Err(());
  }
  let date_parts: Vec<u32> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
  let time_parts: Vec<u32> = parts[1]
    .split('+')
    .next()
    .unwrap_or("")
    .split(':')
    .filter_map(|p| p.parse().ok())
    .collect();
  if date_parts.len() < 3 || time_parts.len() < 3 {
    return Err(());
  }
  let (y, mo, d) = (date_parts[0] as i64, date_parts[1] as i64, date_parts[2] as i64);
  let (h, mi, sec) = (time_parts[0] as i64, time_parts[1] as i64, time_parts[2] as i64);
  let days = days_since_epoch(y, mo, d);
  Ok(days * 86400 + h * 3600 + mi * 60 + sec)
}

fn relative_time(iso: &str) -> String {
  let Ok(ts) = parse_iso8601(iso) else {
    return iso.to_string();
  };
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let diff = now - ts;
  if diff < 60 {
    "just now".to_string()
  } else if diff < 3600 {
    format!("{}m ago", diff / 60)
  } else if diff < 86400 {
    format!("{}h ago", diff / 3600)
  } else {
    format!("{}d ago", diff / 86400)
  }
}
