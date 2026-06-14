use iced::{
  Background, Border, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, Space, container, image, text},
};

use super::{
  super::{LoadState, Message, fmt_isk},
  shared,
};
use crate::{
  clients::eve_image::Size,
  store::images::{self, IconResolution},
  ui::{
    components::{
      badge::badge,
      card,
      clip::clip_layer,
      empty_state::{LoadStateView, empty_state, load_state_view},
      eyebrow::eyebrow_text,
      icon_tile::icon_tile,
      section_header::section_header,
      segmented::segment_button_style,
      virtual_list::{VirtualList, VirtualListConfig},
    },
    style::{color, radius, spacing, typography},
  },
};

/// Nominal height of one kill-log row, in pixels. Rows have a two-line victim/ship cell, so this only feeds the
/// [`VirtualList`] offset math; overscan absorbs the variance.
const ESTIMATED_ROW_HEIGHT: f32 = 52.0;
const SHIP_ICON_SIZE: Size = Size::S64;
const SHIP_ICON_BOX: f32 = 32.0;
const SYSTEM_WIDTH: f32 = 100.0;
const VALUE_WIDTH: f32 = 110.0;
const ATTACKERS_WIDTH: f32 = 80.0;
const TIME_WIDTH: f32 = 90.0;

#[derive(Clone, Debug, PartialEq)]
pub struct KillLogEntry {
  pub attacker_count: i64,
  pub final_blow: bool,
  pub is_kill: bool,
  pub kill_time: String,
  pub killmail_id: i64,
  pub ship_name: String,
  pub ship_type_id: i64,
  pub system_name: Option<String>,
  pub system_security: f64,
  pub value_destroyed_isk: f64,
  pub value_isk: f64,
  pub victim_corp: String,
  pub victim_name: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KilllogFilter {
  #[default]
  All,
  Kills,
  Losses,
}

impl KilllogFilter {
  const SEGMENTS: [(KilllogFilter, &'static str); 3] = [
    (KilllogFilter::All, "All"),
    (KilllogFilter::Kills, "Kills"),
    (KilllogFilter::Losses, "Losses"),
  ];

  pub(in crate::features::character_detail) fn matches(self, entry: &KillLogEntry) -> bool {
    match self {
      KilllogFilter::All => true,
      KilllogFilter::Kills => entry.is_kill,
      KilllogFilter::Losses => !entry.is_kill,
    }
  }
}

struct KillStats {
  kill_count: usize,
  kill_isk: f64,
  loss_count: usize,
  loss_isk: f64,
}

fn compute_stats(entries: &[KillLogEntry]) -> KillStats {
  let mut stats = KillStats {
    kill_count: 0,
    kill_isk: 0.0,
    loss_count: 0,
    loss_isk: 0.0,
  };
  for entry in entries {
    if entry.is_kill {
      stats.kill_count += 1;
      stats.kill_isk += entry.value_destroyed_isk;
    } else {
      stats.loss_count += 1;
      stats.loss_isk += entry.value_destroyed_isk;
    }
  }
  stats
}

/// The non-scrolling header for the Kill Log tab: the kill/loss summary tiles and the activity eyebrow with the
/// kill/loss facet. Hoisted above the windowed list. Returns `None` for the loading/error/empty states (which the
/// body renders as a single full-height placeholder instead).
pub(in crate::features::character_detail) fn header(
  killlog: &LoadState<Vec<KillLogEntry>>,
  filter: KilllogFilter,
) -> Option<Element<'_, Message>> {
  let LoadState::Loaded(entries) = killlog else {
    return None;
  };
  if entries.is_empty() {
    return None;
  }

  let stats = compute_stats(entries);
  let visible = entries.iter().filter(|entry| filter.matches(entry)).count();

  let tiles = summary_tiles(&stats);
  let eyebrow = Row::with_children(vec![
    section_header(&format!("Activity \u{00b7} {visible} entries"), None),
    segmented(filter),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  Some(
    Column::with_children(vec![tiles, eyebrow.into()])
      .spacing(spacing::SPACE_3_5 + spacing::SPACE_2)
      .width(Length::Fill)
      .into(),
  )
}

/// The windowed body for the Kill Log tab: the (filtered) entries table, windowed so a multi-page kill log renders
/// only the viewport's rows. Designed to be the sole content of the tab's scrollable.
pub(in crate::features::character_detail) fn body(
  killlog: &LoadState<Vec<KillLogEntry>>,
  filter: KilllogFilter,
  viewport_height: f32,
  scroll_offset: f32,
) -> Element<'_, Message> {
  let entries = match killlog {
    LoadState::Loaded(entries) => entries,
    LoadState::Loading => {
      return load_state_view(LoadStateView::Loading("Loading kill log\u{2026}"));
    }
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };
  if entries.is_empty() {
    return load_state_view(LoadStateView::Empty(empty_state("No killmails recorded")));
  }

  let visible: Vec<&KillLogEntry> = entries.iter().filter(|entry| filter.matches(entry)).collect();
  entries_card(visible, viewport_height, scroll_offset)
}

fn summary_tiles<'a>(stats: &KillStats) -> Element<'a, Message> {
  let total_isk = stats.kill_isk + stats.loss_isk;
  let (eff_label, eff_color) = if total_isk <= 0.0 {
    ("\u{2014}".to_owned(), color::text::secondary())
  } else {
    let pct = stats.kill_isk / total_isk * 100.0;
    let color = if pct >= 50.0 {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    (format!("{pct:.1}%"), color)
  };

  Row::with_children(vec![
    summary_tile("Kills", stats.kill_count.to_string(), color::status::ONLINE),
    summary_tile("Losses", stats.loss_count.to_string(), color::status::DANGER),
    summary_tile(
      "ISK Destroyed",
      format!("{} ISK", fmt_isk(Some(stats.kill_isk))),
      color::status::ONLINE,
    ),
    summary_tile("Efficiency", eff_label, eff_color),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .into()
}

fn summary_tile<'a>(label: &str, value: String, accent: iced::Color) -> Element<'a, Message> {
  let label_el = eyebrow_text(label, Some(color::text::secondary()));
  let value_el = text(value)
    .font(typography::mono::MEDIUM)
    .size(22.0)
    .style(move |_| text::Style {
      color: Some(accent),
    });

  container(
    Column::with_children(vec![
      label_el.into(),
      Space::new().height(Length::Fixed(6.0)).into(),
      value_el.into(),
    ])
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5 + spacing::SPACE_2,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3_5 + spacing::SPACE_2,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.08),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn segmented<'a>(active: KilllogFilter) -> Element<'a, Message> {
  let mut buttons: Vec<Element<'a, Message>> = Vec::with_capacity(KilllogFilter::SEGMENTS.len());
  for (filter, label) in KilllogFilter::SEGMENTS {
    let selected = filter == active;
    let label_color = if selected {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    };
    buttons.push(
      iced::widget::button(
        text(label)
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(move |_| text::Style {
            color: Some(label_color),
          }),
      )
      .padding(Padding {
        top: spacing::UNIT + 1.0,
        right: spacing::SPACE_3,
        bottom: spacing::UNIT + 1.0,
        left: spacing::SPACE_3,
      })
      .on_press(Message::KilllogFilterChanged(filter))
      .style(move |_, status| segment_button_style(selected, status))
      .into(),
    );
  }

  container(Row::with_children(buttons).spacing(2.0))
    .padding(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.08),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn entries_card<'a>(visible: Vec<&'a KillLogEntry>, viewport_height: f32, scroll_offset: f32) -> Element<'a, Message> {
  if visible.is_empty() {
    let empty = container(
      text("No entries match this filter")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_3_5 + spacing::SPACE_2);
    return card::panel(
      Column::with_children(vec![header_row(), empty.into()]).width(Length::Fill),
      false,
    );
  }

  // Window the (filtered) entries so a multi-page kill log renders only the viewport's rows; the column header
  // stays mounted above the windowed list.
  let config = VirtualListConfig::new(visible.len(), ESTIMATED_ROW_HEIGHT)
    .viewport_height(viewport_height)
    .scroll_offset(scroll_offset);
  let list = VirtualList::new(config, |index| kill_row(visible[index], index == visible.len() - 1)).view();
  let body = Column::with_children(vec![header_row(), list]).width(Length::Fill);

  card::panel(body, false)
}

fn col_label<'a>(label: &str, right: bool) -> Element<'a, Message> {
  let cell = eyebrow_text(label, Some(color::text::tertiary())).width(Length::Fill);

  container(cell)
    .width(Length::Fill)
    .align_x(if right { Horizontal::Right } else { Horizontal::Left })
    .into()
}

fn header_row<'a>() -> Element<'a, Message> {
  let row = Row::with_children(vec![
    Space::new().width(Length::Fixed(4.0)).into(),
    Space::new().width(Length::Fixed(SHIP_ICON_BOX)).into(),
    col_label("Ship", false),
    col_label("Victim \u{00b7} Corp", false),
    cell(col_label("System", false), SYSTEM_WIDTH),
    cell(col_label("Value", true), VALUE_WIDTH),
    cell(col_label("Attackers", true), ATTACKERS_WIDTH),
    cell(col_label("Time", true), TIME_WIDTH),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: 0.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: 1.0,
        radius: Radius {
          top_left: radius::CARD,
          top_right: radius::CARD,
          bottom_right: 0.0,
          bottom_left: 0.0,
        },
      },
      ..container::Style::default()
    })
    .into()
}

fn kill_row<'a>(entry: &'a KillLogEntry, last: bool) -> Element<'a, Message> {
  let accent = if entry.is_kill {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  let inner = Row::with_children(vec![
    color_bar(accent),
    ship_icon(entry.ship_type_id),
    ship_col(entry),
    victim_col(entry),
    cell(system_col(entry), SYSTEM_WIDTH),
    cell(value_col(entry, accent), VALUE_WIDTH),
    cell(
      right_align(
        text(entry.attacker_count.to_string())
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          })
          .into(),
      ),
      ATTACKERS_WIDTH,
    ),
    cell(
      right_align(
        text(relative_time(&entry.kill_time))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          })
          .into(),
      ),
      TIME_WIDTH,
    ),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };
  container(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: 0.0,
    })
    .style(move |_| shared::row_rule_style(border_bottom))
    .into()
}

fn color_bar<'a>(accent: iced::Color) -> Element<'a, Message> {
  container(Space::new().width(Length::Fixed(4.0)).height(Length::Fixed(28.0)))
    .width(Length::Fixed(4.0))
    .height(Length::Fixed(28.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(accent)),
      border: Border {
        radius: 1.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn ship_icon<'a>(ship_type_id: i64) -> Element<'a, Message> {
  match images::default_store().resolve_type_icon(ship_type_id, None, SHIP_ICON_SIZE) {
    IconResolution::Found(path) => icon_tile(
      clip_layer(
        image(image::Handle::from_path(path))
          .width(Length::Fill)
          .height(Length::Fill)
          .content_fit(ContentFit::Cover),
        Length::Fill,
        Length::Fill,
      ),
      SHIP_ICON_BOX,
    ),
    IconResolution::Missing => icon_tile(Space::new(), SHIP_ICON_BOX),
  }
}

fn ship_col<'a>(entry: &'a KillLogEntry) -> Element<'a, Message> {
  let mut items: Vec<Element<'a, Message>> = vec![
    text(entry.ship_name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
  ];
  if entry.final_blow {
    items.push(badge("FINAL BLOW", Some(color::status::ONLINE)));
  }
  Column::with_children(items)
    .spacing(spacing::UNIT)
    .width(Length::Fill)
    .into()
}

fn victim_col<'a>(entry: &'a KillLogEntry) -> Element<'a, Message> {
  Column::with_children(vec![
    text(entry.victim_name.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(entry.victim_corp.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .width(Length::Fill)
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill)
  .into()
}

fn system_col<'a>(entry: &'a KillLogEntry) -> Element<'a, Message> {
  let Some(name) = entry.system_name.as_ref() else {
    return text("\u{2014}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into();
  };

  let sec = entry.system_security;
  let sec_color = if sec >= 0.5 {
    color::status::ONLINE
  } else if sec > 0.0 {
    color::with_alpha(color::status::DANGER, 0.7)
  } else {
    color::status::DANGER
  };

  Column::with_children(vec![
    text(name.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("{:.1}", sec.clamp(-1.0, 1.0)))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(sec_color),
      })
      .into(),
  ])
  .spacing(2.0)
  .into()
}

fn value_col<'a>(entry: &'a KillLogEntry, accent: iced::Color) -> Element<'a, Message> {
  let label = if entry.value_isk > 0.0 {
    format!("{} ISK", fmt_isk(Some(entry.value_isk)))
  } else {
    "\u{2014}".to_owned()
  };
  right_align(
    text(label)
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(accent),
      })
      .into(),
  )
}

fn cell(child: Element<'_, Message>, width: f32) -> Element<'_, Message> {
  container(child).width(Length::Fixed(width)).into()
}

fn right_align(child: Element<'_, Message>) -> Element<'_, Message> {
  container(child).width(Length::Fill).align_x(Horizontal::Right).into()
}

pub(in crate::features::character_detail) fn relative_time(iso: &str) -> String {
  let Some(ts) = parse_iso8601(iso) else {
    return iso.to_owned();
  };
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let diff = now - ts;
  if diff < 60 {
    "just now".to_owned()
  } else if diff < 3600 {
    format!("{}m ago", diff / 60)
  } else if diff < 86_400 {
    format!("{}h ago", diff / 3600)
  } else {
    format!("{}d ago", diff / 86_400)
  }
}

fn parse_iso8601(s: &str) -> Option<i64> {
  let s = s.trim().trim_end_matches('Z');
  let (date, time) = s.split_once('T')?;
  let date_parts: Vec<i64> = date.split('-').filter_map(|p| p.parse().ok()).collect();
  let time_parts: Vec<i64> = time
    .split('+')
    .next()
    .unwrap_or("")
    .split(':')
    .filter_map(|p| p.parse::<f64>().ok().map(|v| v as i64))
    .collect();
  if date_parts.len() < 3 || time_parts.len() < 3 {
    return None;
  }
  let days = days_since_epoch(date_parts[0], date_parts[1], date_parts[2]);
  Some(days * 86_400 + time_parts[0] * 3600 + time_parts[1] * 60 + time_parts[2])
}

fn days_since_epoch(y: i64, m: i64, d: i64) -> i64 {
  let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let doy = (153 * m + 2) / 5 + d - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entry(killmail_id: i64, is_kill: bool, value_isk: f64) -> KillLogEntry {
    entry_with(killmail_id, is_kill, value_isk, value_isk)
  }

  fn entry_with(killmail_id: i64, is_kill: bool, value_isk: f64, value_destroyed_isk: f64) -> KillLogEntry {
    KillLogEntry {
      attacker_count: 3,
      final_blow: is_kill,
      is_kill,
      kill_time: "2024-01-01T00:00:00Z".to_owned(),
      killmail_id,
      ship_name: "Rifter".to_owned(),
      ship_type_id: 587,
      system_name: Some("Jita".to_owned()),
      system_security: 0.9,
      value_destroyed_isk,
      value_isk,
      victim_corp: "Hostile Corp".to_owned(),
      victim_name: "Target Pilot".to_owned(),
    }
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_each_filter() {
      let loaded = LoadState::Loaded(vec![entry(1, true, 1_000_000.0), entry(2, false, 2_000_000.0)]);

      for filter in [KilllogFilter::All, KilllogFilter::Kills, KilllogFilter::Losses] {
        let _el: Element<'_, Message> = body(&loaded, filter, 600.0, 0.0);
      }
    }

    #[test]
    fn it_renders_the_empty_loading_and_error_states() {
      let empty = LoadState::Loaded(Vec::new());
      let loading: LoadState<Vec<KillLogEntry>> = LoadState::Loading;
      let error: LoadState<Vec<KillLogEntry>> = LoadState::Error("boom".to_owned());

      let _empty: Element<'_, Message> = body(&empty, KilllogFilter::All, 600.0, 0.0);
      let _loading: Element<'_, Message> = body(&loading, KilllogFilter::All, 600.0, 0.0);
      let _error: Element<'_, Message> = body(&error, KilllogFilter::All, 600.0, 0.0);
    }
  }

  mod filter {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_passes_everything_for_all() {
      let entries = [entry(1, true, 1.0), entry(2, false, 1.0)];
      let matched = entries.iter().filter(|e| KilllogFilter::All.matches(e)).count();
      assert_eq!(matched, 2);
    }

    #[test]
    fn it_filters_kills_and_losses() {
      let entries = [entry(1, true, 1.0), entry(2, false, 1.0), entry(3, true, 1.0)];
      let kills = entries.iter().filter(|e| KilllogFilter::Kills.matches(e)).count();
      let losses = entries.iter().filter(|e| KilllogFilter::Losses.matches(e)).count();
      assert_eq!(kills, 2);
      assert_eq!(losses, 1);
    }
  }

  mod segmented {
    use super::*;

    #[test]
    fn it_renders_each_active_filter() {
      for filter in [KilllogFilter::All, KilllogFilter::Kills, KilllogFilter::Losses] {
        let _el: Element<'_, Message> = super::super::segmented(filter);
      }
    }
  }

  mod compute_stats {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_counts_and_isk_per_side() {
      let entries = vec![
        entry(1, true, 1_000_000.0),
        entry(2, true, 500_000.0),
        entry(3, false, 2_000_000.0),
      ];

      let stats = compute_stats(&entries);

      assert_eq!(stats.kill_count, 2);
      assert_eq!(stats.kill_isk, 1_500_000.0);
      assert_eq!(stats.loss_count, 1);
      assert_eq!(stats.loss_isk, 2_000_000.0);
    }

    #[test]
    fn it_uses_the_destroyed_only_basis_not_the_displayed_total() {
      let entries = vec![
        entry_with(1, true, 1_000_000.0, 800_000.0),
        entry_with(2, false, 2_000_000.0, 1_200_000.0),
      ];

      let stats = compute_stats(&entries);

      assert_eq!(stats.kill_isk, 800_000.0);
      assert_eq!(stats.loss_isk, 1_200_000.0);
    }
  }

  mod system_col {
    use super::*;

    #[test]
    fn it_renders_a_resolved_and_an_unresolved_system_cell() {
      let mut resolved = entry(1, true, 0.0);
      resolved.system_name = Some("Jita".to_owned());
      let _el: Element<'_, Message> = system_col(&resolved);

      let mut unresolved = entry(2, false, 0.0);
      unresolved.system_name = None;
      let _el: Element<'_, Message> = system_col(&unresolved);
    }
  }

  mod relative_time {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_raw_string_for_an_unparseable_value() {
      assert_eq!(relative_time("not-a-date"), "not-a-date");
    }

    #[test]
    fn it_buckets_a_parseable_timestamp_into_a_relative_label() {
      let label = relative_time("2000-01-01T00:00:00Z");
      assert!(label.ends_with("d ago"), "expected a days-ago bucket, got {label}");
    }
  }
}
