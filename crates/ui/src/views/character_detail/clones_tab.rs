//! Clones tab: active clone with implant grid and jump clone list.

use std::collections::HashMap;

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, scrollable, text},
};
use pod_model::{CharacterClone, CharacterImplant};

use crate::{
  components::LoadState,
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::{LoadState as DataState, Message},
};

/// Builder for the clones tab content.
pub struct Component<'a> {
  clones: &'a DataState<Vec<CharacterClone>>,
  implant_icons: &'a HashMap<i32, image::Handle>,
}

impl<'a> Component<'a> {
  /// Creates a new clones tab component.
  pub fn new(clones: &'a DataState<Vec<CharacterClone>>, implant_icons: &'a HashMap<i32, image::Handle>) -> Self {
    Self {
      clones,
      implant_icons,
    }
  }

  /// Renders the clones tab.
  pub fn render(self) -> Element<'a, Message> {
    match self.clones {
      DataState::Loading => LoadState::loading("Loading clones…").render(),
      DataState::Error(e) => LoadState::error(e).render(),
      DataState::Loaded(clones) => clones_content(clones, self.implant_icons),
    }
  }
}

fn clones_content<'a>(clones: &'a [CharacterClone], icons: &'a HashMap<i32, image::Handle>) -> Element<'a, Message> {
  let active = clones.iter().find(|c| c.is_active);
  let jump_clones: Vec<&CharacterClone> = clones.iter().filter(|c| !c.is_active).collect();

  let jump_ready_label = active
    .and_then(|c| c.jump_ready_at.as_deref())
    .map(jump_readiness_label)
    .unwrap_or_else(|| "ready".to_string());

  let active_right = format!("Jump readiness · {jump_ready_label}");

  let mut sections: Vec<Element<'_, Message>> = Vec::new();

  let active_section = column([
    section_eyebrow("Active clone", active_right),
    if let Some(c) = active {
      active_clone_card(c, icons)
    } else {
      empty_active_placeholder()
    },
  ])
  .spacing(10.0)
  .into();
  sections.push(active_section);

  let jump_count_label = format!("{} installed", jump_clones.len());

  let jump_grid = if jump_clones.is_empty() {
    container(
      text("No jump clones installed.")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .padding(Padding {
      top: 16.0,
      bottom: 16.0,
      left: 0.0,
      right: 0.0,
    })
    .into()
  } else {
    let cards: Vec<Element<'_, Message>> = jump_clones.iter().map(|c| jump_clone_card(c, icons)).collect();
    let mut grid_rows: Vec<Element<'_, Message>> = Vec::new();
    let mut iter = cards.into_iter();
    loop {
      let a = iter.next();
      if a.is_none() {
        break;
      }
      let b = iter.next();
      let c_el = iter.next();
      let mut row_els: Vec<Element<'_, Message>> = vec![container(a.unwrap()).width(Length::Fill).into()];
      row_els.push(match b {
        Some(el) => container(el).width(Length::Fill).into(),
        None => Space::new().width(Length::Fill).into(),
      });
      row_els.push(match c_el {
        Some(el) => container(el).width(Length::Fill).into(),
        None => Space::new().width(Length::Fill).into(),
      });
      grid_rows.push(row(row_els).spacing(12.0).into());
    }
    column(grid_rows).spacing(12.0).into()
  };

  let jump_section = column([section_eyebrow("Jump clones", jump_count_label), jump_grid])
    .spacing(10.0)
    .into();
  sections.push(jump_section);

  scrollable(
    column(sections)
      .spacing(24.0)
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

fn jump_readiness_label(last_jump_iso: &str) -> String {
  let Ok(jump_time) = parse_iso8601(last_jump_iso) else {
    return "ready".to_string();
  };
  let now_secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let cooldown_secs: i64 = 24 * 3600;
  let ready_at = jump_time + cooldown_secs;
  let remaining = ready_at - now_secs;
  if remaining <= 0 {
    "ready".to_string()
  } else {
    let h = remaining / 3600;
    let m = (remaining % 3600) / 60;
    format!("{h}h {m:02}m")
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

fn section_eyebrow(label: impl Into<String>, right: impl Into<String>) -> Element<'static, Message> {
  let left_el = text(label.into().to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });
  let right_el = text(right.into())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });
  row([left_el.into(), Space::new().width(Length::Fill).into(), right_el.into()])
    .padding(Padding {
      top: 0.0,
      bottom: 10.0,
      left: 0.0,
      right: 0.0,
    })
    .into()
}

fn empty_active_placeholder<'a>() -> Element<'a, Message> {
  container(
    text("No active clone data.")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(20.0)
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

fn active_clone_card<'a>(clone: &'a CharacterClone, icons: &'a HashMap<i32, image::Handle>) -> Element<'a, Message> {
  let header = clone_card_header(clone, true);
  let grid = implant_slot_grid(&clone.implants, 2, icons);

  container(column([header, grid]))
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::accent::PLASMA_MUTED,
        radius: 10.0.into(),
        width: 1.0,
      },
      shadow: iced::Shadow {
        blur_radius: 0.0,
        color: Color::TRANSPARENT,
        offset: iced::Vector::new(0.0, 0.0),
      },
      ..container::Style::default()
    })
    .into()
}

fn jump_clone_card<'a>(clone: &'a CharacterClone, icons: &'a HashMap<i32, image::Handle>) -> Element<'a, Message> {
  let header = clone_card_header(clone, false);
  let grid = implant_slot_grid(&clone.implants, 1, icons);

  container(column([header, grid]))
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
    })
    .into()
}

fn clone_card_header<'a>(clone: &'a CharacterClone, is_active: bool) -> Element<'a, Message> {
  let display_name = if is_active {
    clone.station_name.clone()
  } else {
    clone.name.clone().unwrap_or_else(|| clone.station_name.clone())
  };

  let right_label = if is_active {
    "ACTIVE".to_string()
  } else if clone.implants.is_empty() {
    "EMPTY".to_string()
  } else {
    format!("{} IMPLANTS", clone.implants.len())
  };

  let right_color = if is_active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };

  let name_el = text(display_name)
    .font(body::MEDIUM)
    .size(14.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let location_el = text(clone.station_name.to_uppercase())
    .font(mono::REGULAR)
    .size(10.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let left_col = column([name_el.into(), Space::new().height(2.0).into(), location_el.into()]).into();

  let right_el = text(right_label)
    .font(mono::REGULAR)
    .size(9.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(right_color),
    });

  let header_row = row([left_col, Space::new().width(Length::Fill).into(), right_el.into()])
    .align_y(iced::alignment::Vertical::Center)
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: 16.0,
      right: 16.0,
    });

  container(header_row)
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn implant_slot_grid<'a>(
  implants: &'a [CharacterImplant],
  cols: usize,
  icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  const SLOT_COUNT: usize = 10;
  let by_slot: std::collections::HashMap<usize, &CharacterImplant> = implants.iter().map(|i| (i.slot, i)).collect();

  let mut slot_rows: Vec<Element<'a, Message>> = Vec::new();
  for slot_idx in 1..=SLOT_COUNT {
    let implant = by_slot.get(&slot_idx).copied();
    slot_rows.push(implant_slot_row(slot_idx, implant, icons));
  }

  if cols == 2 {
    let mut grid_rows: Vec<Element<'a, Message>> = Vec::new();
    let mut iter = slot_rows.into_iter();
    while let Some(left) = iter.next() {
      let right = iter.next();
      let mut row_els: Vec<Element<'a, Message>> = vec![left];
      if let Some(r) = right {
        row_els.push(r);
      } else {
        row_els.push(Space::new().width(Length::Fill).into());
      }
      grid_rows.push(row(row_els).spacing(0.0).width(Length::Fill).into());
    }
    container(column(grid_rows).spacing(0.0).padding(Padding {
      top: 4.0,
      bottom: 4.0,
      left: 8.0,
      right: 8.0,
    }))
    .width(Length::Fill)
    .into()
  } else {
    container(column(slot_rows).spacing(0.0).padding(Padding {
      top: 4.0,
      bottom: 4.0,
      left: 8.0,
      right: 8.0,
    }))
    .width(Length::Fill)
    .into()
  }
}

fn implant_slot_row<'a>(
  slot_num: usize,
  implant: Option<&'a CharacterImplant>,
  icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let slot_label_color = if implant.is_some() {
    color::accent::PLASMA
  } else {
    color::text::TERTIARY
  };

  let slot_num_el = text(format!("{slot_num:02}"))
    .font(mono::REGULAR)
    .size(10.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(slot_label_color),
    });

  let icon_box = implant_icon_box(implant, icons);

  let name_el: Element<'_, Message> = if let Some(imp) = implant {
    text(imp.name.clone())
      .font(body::REGULAR)
      .size(12.5)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into()
  } else {
    text("— empty slot —")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(Color::from_rgba(0.957, 0.949, 0.925, 0.18)),
      })
      .width(Length::Fill)
      .into()
  };

  let inner = row([
    container(slot_num_el).width(22.0).into(),
    icon_box,
    Space::new().width(12.0).into(),
    name_el,
  ])
  .align_y(iced::alignment::Vertical::Center)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 8.0,
    right: 8.0,
  });

  container(inner)
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn implant_icon_box<'a>(
  implant: Option<&'a CharacterImplant>,
  icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  if let Some(imp) = implant {
    if let Some(handle) = icons.get(&imp.type_id) {
      return container(
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
          color: Color::from_rgba(0.247, 0.722, 0.859, 0.35),
          radius: 4.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into();
    }
    container(Space::new().width(32.0).height(32.0))
      .width(32.0)
      .height(32.0)
      .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.08))),
        border: Border {
          color: Color::from_rgba(0.247, 0.722, 0.859, 0.35),
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
        background: None,
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
