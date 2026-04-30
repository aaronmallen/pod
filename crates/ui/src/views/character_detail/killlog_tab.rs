//! Kill log tab: summary tiles, filter control, and kill/loss entry rows.

use std::collections::HashMap;

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Theme,
  border::Radius,
  widget::{Space, button, column, container, image, row, scrollable, text},
};
use pod_model::CharacterKillEntry;

use crate::{
  components::LoadState,
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::{KilllogFilter, LoadState as DataState, Message},
};

/// Builder for the kill log tab content.
pub struct Component<'a> {
  filter: &'a KilllogFilter,
  filtered: &'a [CharacterKillEntry],
  killlog: &'a DataState<Vec<CharacterKillEntry>>,
  ship_icons: &'a HashMap<i32, image::Handle>,
}

impl<'a> Component<'a> {
  /// Creates a new kill log tab component.
  pub fn new(
    killlog: &'a DataState<Vec<CharacterKillEntry>>,
    filtered: &'a [CharacterKillEntry],
    filter: &'a KilllogFilter,
    ship_icons: &'a HashMap<i32, image::Handle>,
  ) -> Self {
    Self {
      filter,
      filtered,
      killlog,
      ship_icons,
    }
  }

  /// Renders the kill log tab.
  pub fn render(self) -> Element<'a, Message> {
    match self.killlog {
      DataState::Loading => LoadState::loading("Loading kill log…").render(),
      DataState::Error(e) => LoadState::error(e).render(),
      DataState::Loaded(entries) => killlog_content(entries, self.filtered, self.filter, self.ship_icons),
    }
  }
}

fn killlog_content<'a>(
  entries: &'a [CharacterKillEntry],
  filtered: &'a [CharacterKillEntry],
  filter: &'a KilllogFilter,
  ship_icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let kills: Vec<&CharacterKillEntry> = entries.iter().filter(|e| e.is_kill).collect();
  let losses: Vec<&CharacterKillEntry> = entries.iter().filter(|e| !e.is_kill).collect();
  let kill_count = kills.len();
  let loss_count = losses.len();

  let kill_isk: f64 = kills.iter().map(|e| e.total_value).sum();
  let loss_isk: f64 = losses.iter().map(|e| e.total_value).sum();
  let total_isk = kill_isk + loss_isk;
  let efficiency = if total_isk > 0.0 {
    kill_isk / total_isk * 100.0
  } else {
    0.0
  };

  let eff_color = if total_isk <= 0.0 {
    color::text::SECONDARY
  } else if efficiency >= 50.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let eff_label = if total_isk <= 0.0 {
    "\u{2014}".to_string()
  } else {
    format!("{efficiency:.1}%")
  };

  let summary_tiles = row([
    summary_tile("Kills", kill_count.to_string(), color::status::ONLINE),
    summary_tile("Losses", loss_count.to_string(), color::status::DANGER),
    summary_tile(
      "ISK Destroyed",
      format!("{} ISK", format::fmt_isk(kill_isk)),
      color::status::ONLINE,
    ),
    summary_tile("Efficiency", eff_label, eff_color),
  ])
  .spacing(12.0)
  .width(Length::Fill)
  .into();

  let visible: Vec<&CharacterKillEntry> = filtered.iter().collect();

  let eyebrow_row = row([
    text(format!("Activity · {} entries", visible.len()).to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    segmented_control(filter),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();

  let header_row = killlog_header_row();

  let mut kill_rows: Vec<Element<'_, Message>> = visible
    .iter()
    .enumerate()
    .map(|(i, e)| kill_row(e, i == visible.len() - 1, ship_icons))
    .collect();

  if kill_rows.is_empty() {
    kill_rows.push(
      container(
        text("No entries match your filter.")
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

  let card = container(column([header_row].into_iter().chain(kill_rows).collect::<Vec<_>>()))
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
    column([summary_tiles, eyebrow_row, card.into()])
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

fn summary_tile(label: &str, value: String, accent: Color) -> Element<'static, Message> {
  let label_el = text(label.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let value_el = text(value)
    .font(mono::MEDIUM)
    .size(22.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(accent),
    });

  container(
    column([label_el.into(), Space::new().height(6.0).into(), value_el.into()]).padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::DEFAULT,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn segmented_control(filter: &KilllogFilter) -> Element<'_, Message> {
  let options = [
    (KilllogFilter::All, "All"),
    (KilllogFilter::Kill, "Kills"),
    (KilllogFilter::Loss, "Losses"),
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
      .on_press(Message::KilllogFilterChanged(opt_clone))
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

fn killlog_header_row<'a>() -> Element<'a, Message> {
  container(
    row([
      Space::new().width(4.0).into(),
      Space::new().width(32.0).into(),
      col_label("Ship", false, Length::Fill),
      col_label("Victim · Corp", false, Length::Fill),
      col_label("System", false, Length::Fixed(100.0)),
      col_label("Value", true, Length::Fixed(110.0)),
      col_label("Attackers", true, Length::Fixed(80.0)),
      col_label("Time", true, Length::Fixed(90.0)),
    ])
    .spacing(12.0)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 0.0,
      right: 12.0,
    })
    .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      radius: Radius {
        top_left: 10.0,
        top_right: 10.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
      },
    },
    ..container::Style::default()
  })
  .into()
}

fn col_label<'a>(label: &'a str, right: bool, width: Length) -> Element<'a, Message> {
  let align = if right {
    iced::alignment::Horizontal::Right
  } else {
    iced::alignment::Horizontal::Left
  };
  container(
    text(label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .width(width)
  .align_x(align)
  .into()
}

fn kill_row<'a>(
  entry: &'a CharacterKillEntry,
  is_last: bool,
  ship_icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let is_kill = entry.is_kill;
  let entry_color = if is_kill {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  let color_bar = container(Space::new().width(4.0).height(Length::Fill))
    .width(4.0)
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(entry_color)),
      border: Border {
        radius: 1.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let ship_icon_el: Element<'_, Message> = if let Some(handle) = ship_icons.get(&entry.ship_type_id) {
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
        background: Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05))),
        border: Border {
          color: color::border::SUBTLE,
          radius: 4.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  };

  let mut ship_items: Vec<Element<'_, Message>> = vec![
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
    ship_items.push(
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
        background: Some(Background::Color(Color::from_rgba(0.275, 0.788, 0.431, 0.12))),
        border: Border {
          color: color::status::ONLINE,
          radius: 3.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into(),
    );
  }
  let ship_el = column(ship_items).spacing(4.0).width(Length::Fill);

  let victim_el = column([
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
  .width(Length::Fill);

  let sec = entry.solar_system_security;
  let sec_color = if sec >= 0.5 {
    color::status::ONLINE
  } else if sec > 0.0 {
    color::status::CAUTION
  } else {
    color::status::DANGER
  };
  let sec_label = format!("{:.1}", sec.max(-1.0).min(1.0));

  let system_el = container(
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
  .width(100.0);

  let value_text = if entry.total_value > 0.0 {
    format!("{} ISK", format::fmt_isk(entry.total_value))
  } else {
    "\u{2014}".to_string()
  };
  let value_el =
    container(
      text(value_text)
        .font(mono::MEDIUM)
        .size(12.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(entry_color),
        }),
    )
    .width(110.0)
    .align_x(iced::alignment::Horizontal::Right);

  let attackers_el = container(
    text(entry.attacker_count.to_string())
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(80.0)
  .align_x(iced::alignment::Horizontal::Right);

  let time_el = container(
    text(relative_time(&entry.timestamp))
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .width(90.0)
  .align_x(iced::alignment::Horizontal::Right);

  let inner = row([
    color_bar.into(),
    ship_icon_el,
    ship_el.into(),
    victim_el.into(),
    system_el.into(),
    value_el.into(),
    attackers_el.into(),
    time_el.into(),
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
