//! Kill row component: a single kill or loss entry in the kill log table.

use std::collections::HashMap;

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, text},
};
use pod_model::CharacterKillEntry;

use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::Message,
};

/// Builder for a single kill/loss entry row.
pub struct Component<'a> {
  entry: &'a CharacterKillEntry,
  is_last: bool,
  ship_icons: &'a HashMap<i32, image::Handle>,
}

impl<'a> Component<'a> {
  /// Creates a new kill row for the given entry.
  pub fn new(entry: &'a CharacterKillEntry, is_last: bool, ship_icons: &'a HashMap<i32, image::Handle>) -> Self {
    Self {
      entry,
      is_last,
      ship_icons,
    }
  }

  /// Renders the kill row.
  pub fn render(self) -> Element<'a, Message> {
    kill_row(self.entry, self.is_last, self.ship_icons)
  }
}

fn kill_row<'a>(
  entry: &'a CharacterKillEntry,
  is_last: bool,
  ship_icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let entry_color = if entry.is_kill {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let inner = row([
    color_bar(entry_color),
    ship_icon_el(entry, ship_icons),
    ship_col(entry),
    victim_col(entry),
    system_col(entry),
    value_col(entry, entry_color),
    attackers_col(entry),
    time_col(entry),
  ])
  .spacing(12.0)
  .align_y(iced::alignment::Vertical::Center)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 0.0,
    right: 12.0,
  });
  container(inner)
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

fn attackers_col<'a>(entry: &'a CharacterKillEntry) -> Element<'a, Message> {
  container(
    text(entry.attacker_count.to_string())
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(80.0)
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn color_bar(entry_color: Color) -> Element<'static, Message> {
  container(Space::new().width(4.0).height(Length::Fill))
    .width(4.0)
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(entry_color)),
      border: Border {
        radius: 1.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn final_blow_badge<'a>() -> Element<'a, Message> {
  container(
    text("FINAL BLOW")
      .font(mono::REGULAR)
      .size(8.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::status::ONLINE),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 5.0,
    right: 5.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::status::VICTORY_SUBTLE)),
    border: Border {
      color: color::status::ONLINE,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn ship_col<'a>(entry: &'a CharacterKillEntry) -> Element<'a, Message> {
  let mut items: Vec<Element<'_, Message>> = vec![
    text(entry.ship_name.clone())
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
  ];
  if entry.final_blow {
    items.push(final_blow_badge());
  }
  column(items).spacing(4.0).width(Length::Fill).into()
}

fn ship_icon_el<'a>(
  entry: &'a CharacterKillEntry,
  ship_icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  if let Some(handle) = ship_icons.get(&entry.ship_type_id) {
    container(
      image(handle.clone())
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .content_fit(ContentFit::Cover),
    )
    .width(32.0)
    .height(32.0)
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  } else {
    container(Space::new().width(32.0).height(32.0))
      .width(32.0)
      .height(32.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::state::HOVER_OVERLAY)),
        border: Border {
          color: color::border::SUBTLE,
          radius: 4.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn system_col<'a>(entry: &'a CharacterKillEntry) -> Element<'a, Message> {
  let sec = entry.solar_system_security;
  let sec_color = if sec >= 0.5 {
    color::status::ONLINE
  } else if sec > 0.0 {
    color::status::CAUTION
  } else {
    color::status::DANGER
  };
  let sec_label = format!("{:.1}", sec.clamp(-1.0, 1.0));
  container(
    column([
      text(entry.solar_system_name.clone())
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(sec_label)
        .font(mono::REGULAR)
        .size(9.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(sec_color),
        })
        .into(),
    ])
    .spacing(2.0),
  )
  .width(100.0)
  .into()
}

fn time_col<'a>(entry: &'a CharacterKillEntry) -> Element<'a, Message> {
  container(
    text(relative_time(&entry.timestamp))
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .width(90.0)
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn value_col<'a>(entry: &'a CharacterKillEntry, entry_color: Color) -> Element<'a, Message> {
  let value_text = if entry.total_value > 0.0 {
    format!("{} ISK", format::fmt_isk(entry.total_value))
  } else {
    "\u{2014}".to_string()
  };
  container(
    text(value_text)
      .font(mono::MEDIUM)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(entry_color),
      }),
  )
  .width(110.0)
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn victim_col<'a>(entry: &'a CharacterKillEntry) -> Element<'a, Message> {
  column([
    text(entry.victim_name.clone())
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(entry.victim_corp.clone())
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .width(Length::Fill)
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill)
  .into()
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

fn days_since_epoch(y: i64, m: i64, d: i64) -> i64 {
  let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let doy = (153 * m + 2) / 5 + d - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146097 + doe - 719468
}
