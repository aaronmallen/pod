//! Notifications tab: categorised list with unread indicators and filter.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, scrollable, text},
};
use pod_model::CharacterNotification;

use crate::{
  components::{Icon, LoadState},
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::{LoadState as DataState, Message, NotificationsFilter},
};

/// Builder for the notifications tab content.
pub struct Component<'a> {
  filter: &'a NotificationsFilter,
  filtered: &'a [CharacterNotification],
  notifications: &'a DataState<Vec<CharacterNotification>>,
  unread_count: usize,
}

impl<'a> Component<'a> {
  /// Creates a new notifications tab component.
  pub fn new(
    notifications: &'a DataState<Vec<CharacterNotification>>,
    filtered: &'a [CharacterNotification],
    unread_count: usize,
    filter: &'a NotificationsFilter,
  ) -> Self {
    Self {
      filter,
      filtered,
      notifications,
      unread_count,
    }
  }

  /// Renders the notifications tab.
  pub fn render(self) -> Element<'a, Message> {
    match self.notifications {
      DataState::Loading => LoadState::loading("Loading notifications…").render(),
      DataState::Error(e) => LoadState::error(e).render(),
      DataState::Loaded(_) => notifications_content(self.filtered, self.unread_count, self.filter),
    }
  }
}

fn notifications_content<'a>(
  notifications: &'a [CharacterNotification],
  unread_count: usize,
  filter: &'a NotificationsFilter,
) -> Element<'a, Message> {
  let visible: Vec<&CharacterNotification> = notifications.iter().collect();

  let eyebrow_left = format!("Notifications · {}", visible.len());
  let eyebrow_right = format!("{unread_count} unread");

  let eyebrow_row = row([
    text(eyebrow_left.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(8.0).into(),
    text(eyebrow_right)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    segmented_control(filter),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();

  let mut notif_rows: Vec<Element<'_, Message>> = visible
    .iter()
    .enumerate()
    .map(|(i, n)| notif_row(n, i == visible.len() - 1))
    .collect();

  if notif_rows.is_empty() {
    notif_rows.push(
      container(
        text("No notifications match your filter.")
          .font(body::REGULAR)
          .size(13.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .padding(20.0)
      .width(Length::Fill)
      .into(),
    );
  }

  let card = container(column(notif_rows))
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  scrollable(
    column([eyebrow_row, card.into()])
      .spacing(16.0)
      .padding(Padding {
        top: 24.0,
        bottom: 24.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
  )
  .height(Length::Fill)
  .into()
}

fn segmented_control(filter: &NotificationsFilter) -> Element<'_, Message> {
  let options = [
    (NotificationsFilter::All, "All"),
    (NotificationsFilter::Unread, "Unread"),
    (NotificationsFilter::War, "War"),
    (NotificationsFilter::Combat, "Combat"),
    (NotificationsFilter::Corp, "Corp"),
    (NotificationsFilter::Structure, "Structure"),
  ];

  let btns: Vec<Element<'_, Message>> = options
    .iter()
    .map(|(opt, label)| {
      let is_active = filter == opt;
      let opt_clone = opt.clone();
      button(
        text(label.to_string())
          .font(body::MEDIUM)
          .size(12.0)
          .style(move |_: &Theme| iced::widget::text::Style {
            color: Some(if is_active {
              color::accent::PLASMA
            } else {
              color::text::SECONDARY
            }),
          }),
      )
      .padding(Padding {
        top: 5.0,
        bottom: 5.0,
        left: 12.0,
        right: 12.0,
      })
      .style(move |_, _| button::Style {
        background: if is_active {
          Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.12)))
        } else {
          None
        },
        border: Border {
          radius: 4.0.into(),
          ..Border::default()
        },
        text_color: if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        },
        ..button::Style::default()
      })
      .on_press(Message::NotificationsFilterChanged(opt_clone))
      .into()
    })
    .collect();

  container(row(btns).spacing(2.0).padding(2.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
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
    .splitn(2, '+')
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

fn notif_row<'a>(notif: &'a CharacterNotification, is_last: bool) -> Element<'a, Message> {
  let cat_color = category_color(&notif.category);
  let is_unread = !notif.is_read;

  let icon_el = category_icon(&notif.category)
    .size(16.0)
    .color(cat_color)
    .render::<Message>();
  let icon_box = container(icon_el)
    .width(28.0)
    .height(28.0)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.04))),
      border: Border {
        color: cat_color,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let cat_label = notif.category.to_uppercase();
  let title_text = format_notif_type(&notif.type_);
  let title_weight = if is_unread { mono::MEDIUM } else { mono::REGULAR };
  let time_label = relative_time(&notif.timestamp);

  let body_snippet = notif.text.as_deref().map(|t| {
    let first_line = t.lines().next().unwrap_or("").trim();
    if first_line.len() > 80 {
      format!("{}…", &first_line[..80])
    } else {
      first_line.to_string()
    }
  });

  let mut content_items: Vec<Element<'_, Message>> = vec![
    text(cat_label)
      .font(mono::REGULAR)
      .size(9.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(cat_color),
      })
      .into(),
    text(title_text)
      .font(title_weight)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(snippet) = body_snippet {
    if !snippet.is_empty() {
      content_items.push(
        text(snippet)
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      );
    }
  }

  let content_col = column(content_items).spacing(3.0).width(Length::Fill).into();

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

  let inner_row = row([icon_box.into(), content_col, timestamp_el])
    .spacing(14.0)
    .align_y(iced::alignment::Vertical::Top)
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: if is_unread { 14.0 } else { 16.0 },
      right: 16.0,
    });

  let bg_color = if is_unread {
    Color::from_rgba(0.247, 0.722, 0.859, 0.04)
  } else {
    Color::TRANSPARENT
  };

  let border_left_color = if is_unread {
    color::accent::PLASMA
  } else {
    Color::TRANSPARENT
  };

  let notif_id = notif.notification_id;
  if is_unread {
    let plasma_bar = container(Space::new().width(2.0).height(Length::Fill))
      .width(2.0)
      .height(Length::Fill)
      .style(move |_| container::Style {
        background: Some(Background::Color(border_left_color)),
        ..container::Style::default()
      });

    button(
      row([plasma_bar.into(), inner_row.into()])
        .width(Length::Fill)
        .height(Length::Shrink),
    )
    .width(Length::Fill)
    .padding(0)
    .style(move |_, _| button::Style {
      background: Some(Background::Color(bg_color)),
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
  } else {
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
}

fn category_icon(category: &str) -> Icon {
  match category {
    "alliance" => Icon::notif_alliance(),
    "clone" => Icon::notif_clone(),
    "combat" => Icon::notif_combat(),
    "contact" => Icon::notif_contact(),
    "contract" => Icon::notif_contract(),
    "corp" => Icon::notif_corp(),
    "fw" => Icon::notif_fw(),
    "incursion" => Icon::notif_incursion(),
    "industry" => Icon::notif_industry(),
    "insurance" => Icon::notif_insurance(),
    "market" => Icon::notif_market(),
    "mission" => Icon::notif_mission(),
    "reward" => Icon::notif_reward(),
    "standing" => Icon::notif_standing(),
    "structure" => Icon::notif_structure(),
    "war" => Icon::notif_war(),
    _ => Icon::notif_system(),
  }
}

fn category_color(category: &str) -> Color {
  match category {
    "war" | "incursion" | "combat" => color::status::DANGER,
    "corp" | "alliance" | "fw" => color::status::CAUTION,
    "structure" | "mission" | "industry" | "standing" => color::accent::PLASMA,
    "market" | "insurance" | "reward" => color::status::ONLINE,
    "contract" | "clone" | "contact" => Color {
      r: 0.376,
      g: 0.647,
      b: 0.902,
      a: 1.0,
    },
    _ => color::text::SECONDARY,
  }
}
