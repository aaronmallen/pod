//! Kill log tab: summary tiles, filter control, and kill/loss entry rows.

pub mod filter_control;
pub mod kill_row;
pub mod summary_tile;

use std::collections::HashMap;

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  border::Radius,
  widget::{Space, column, container, image, row, scrollable, text},
};
use pod_model::CharacterKillEntry;

use crate::{
  components::LoadState,
  style::{color, typography::mono},
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

struct KillStats {
  kill_count: usize,
  kill_isk: f64,
  loss_count: usize,
  loss_isk: f64,
}

fn compute_stats(entries: &[CharacterKillEntry]) -> KillStats {
  let kills: Vec<&CharacterKillEntry> = entries.iter().filter(|e| e.is_kill).collect();
  let losses: Vec<&CharacterKillEntry> = entries.iter().filter(|e| !e.is_kill).collect();
  KillStats {
    kill_count: kills.len(),
    kill_isk: kills.iter().map(|e| e.total_value).sum(),
    loss_count: losses.len(),
    loss_isk: losses.iter().map(|e| e.total_value).sum(),
  }
}

fn activity_eyebrow_row<'a>(visible_count: usize, filter: &'a KilllogFilter) -> Element<'a, Message> {
  row([
    text(format!("Activity · {} entries", visible_count).to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    filter_control::Component::new(filter).render(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn empty_filter_message<'a>() -> Element<'a, Message> {
  container(
    text("No entries match your filter.")
      .font(mono::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(20.0)
  .width(Length::Fill)
  .into()
}

fn kill_entries_card<'a>(
  visible: &[&'a CharacterKillEntry],
  ship_icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let header_row = killlog_header_row();
  let mut kill_rows: Vec<Element<'_, Message>> = visible
    .iter()
    .enumerate()
    .map(|(i, e)| kill_row::Component::new(e, i == visible.len() - 1, ship_icons).render())
    .collect();
  if kill_rows.is_empty() {
    kill_rows.push(empty_filter_message());
  }
  container(column([header_row].into_iter().chain(kill_rows).collect::<Vec<_>>()))
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

fn killlog_content<'a>(
  entries: &'a [CharacterKillEntry],
  filtered: &'a [CharacterKillEntry],
  filter: &'a KilllogFilter,
  ship_icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let stats = compute_stats(entries);
  let visible: Vec<&CharacterKillEntry> = filtered.iter().collect();
  let tiles = summary_tile::Component::new(stats.kill_count, stats.kill_isk, stats.loss_count, stats.loss_isk).render();
  let eyebrow = activity_eyebrow_row(visible.len(), filter);
  let card = kill_entries_card(&visible, ship_icons);
  scrollable(
    column([tiles, eyebrow, card])
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
