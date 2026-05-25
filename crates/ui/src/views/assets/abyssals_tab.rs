//! Abyssals tab — mutated module grid with stat rows, filter sidebar, and search.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, scrollable, text, text_input},
};
use pod_model::{AbyssalStatViewModel, AbyssalViewModel};

use super::State;
use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

/// Unit ID → suffix string mapping for abyssal stat display.
///
/// Covers the ~10 unit IDs that appear in abyssal module stats.
const UNIT_SUFFIX_TABLE: &[(i32, &str)] = &[
  (71, " GJ"),
  (101, " m/s"),
  (105, " HP"),
  (108, " s"),
  (114, " kg"),
  (115, " tf"),
  (116, " MW"),
  (117, " km"),
  (121, " m\u{00b3}"),
  (124, "%"),
];

fn unit_suffix_for_id(unit_id: Option<i32>) -> &'static str {
  unit_id
    .and_then(|id| UNIT_SUFFIX_TABLE.iter().find(|&&(k, _)| k == id).map(|&(_, v)| v))
    .unwrap_or("")
}

/// Extracts the unit suffix for display from the unit_id stored on a stat view model.
///
/// Used when building the view from a `unit_suffix` string already resolved by the controller.
pub fn dogma_unit_suffix(unit_id: Option<i32>) -> String {
  unit_suffix_for_id(unit_id).to_string()
}

/// Messages produced by the abyssals tab.
#[derive(Clone, Debug)]
pub enum Message {
  /// The search query text changed.
  SearchChanged(String),
  /// The "only net-positive rolls" toggle was clicked.
  OnlyPositiveToggled,
  /// A module type was selected in the filter sidebar (None = all).
  TypeSelected(Option<i32>),
  /// The filter sidebar was reset.
  FilterReset,
}

/// Mutaplasmid tier badge color, derived from the tier name.
fn tier_badge_color(tier: &str) -> Color {
  let lower = tier.to_lowercase();
  if lower.contains("glorified") && lower.contains("unstable") {
    Color::from_rgb(0.741, 0.490, 0.133)
  } else if lower.contains("glorified") && lower.contains("gravid") {
    Color::from_rgb(0.588, 0.349, 0.792)
  } else if lower.contains("glorified") && lower.contains("decayed") {
    Color::from_rgb(0.247, 0.557, 0.859)
  } else if lower.contains("unstable") {
    Color::from_rgb(0.878, 0.459, 0.349)
  } else if lower.contains("gravid") {
    Color::from_rgb(0.612, 0.408, 0.839)
  } else {
    Color::from_rgb(0.247, 0.600, 0.780)
  }
}

/// Renders a mutaplasmid tier badge element.
fn tier_badge(tier: &str) -> Element<'static, Message> {
  let col = tier_badge_color(tier);
  let tier_label = tier.to_uppercase();
  container(
    text(tier_label)
      .font(mono::REGULAR)
      .size(9.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(col),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 7.0,
    right: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(col, 0.12))),
    border: Border {
      color: color::with_alpha(col, 0.45),
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

/// Renders the type icon tile (monogram fallback used since icons come from ESI).
fn type_icon_tile(base_type_name: &str, type_id: i32) -> Element<'static, Message> {
  let letters: String = base_type_name
    .split_whitespace()
    .filter(|w| w.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
    .take(2)
    .map(|w| w.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'))
    .collect();
  let letters = if letters.is_empty() {
    format!("{}", type_id % 100)
  } else {
    letters
  };
  let hue = (type_id % 360) as f32;
  let col = Color::from([
    0.5 + 0.3 * (hue.to_radians()).cos(),
    0.5 + 0.3 * (hue.to_radians() + 2.094).cos(),
    0.5 + 0.3 * (hue.to_radians() + 4.189).cos(),
    1.0,
  ]);
  container(
    text(letters)
      .font(mono::REGULAR)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(Color::WHITE),
      }),
  )
  .width(42.0)
  .height(42.0)
  .center(42.0)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(col, 0.8))),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

/// Renders a single stat row within an abyssal card.
fn stat_roll_direction(stat: &AbyssalStatViewModel) -> Option<bool> {
  let delta = stat.rolled_value - stat.base_value;
  if delta.abs() < 1e-9 {
    None
  } else if stat.high_is_good {
    Some(delta > 0.0)
  } else {
    Some(delta < 0.0)
  }
}

fn stat_direction_color(dir: Option<bool>) -> Color {
  match dir {
    Some(true) => color::text::SUCCESS,
    Some(false) => color::text::DANGER,
    None => color::text::TERTIARY,
  }
}

fn stat_delta_intensity(stat: &AbyssalStatViewModel, delta: f64) -> f32 {
  let range_span = (stat.max_mult - 1.0).abs().max(1e-9);
  let delta_pct = if stat.base_value.abs() > 1e-9 {
    (delta / stat.base_value).abs()
  } else {
    0.0
  };
  (delta_pct / range_span).clamp(0.0, 1.0) as f32
}

fn stat_intensity_bar(intensity: f32, fill_col: Color) -> Element<'static, Message> {
  let bg_col = color::border::SUBTLE;
  container(
    container(Space::new().width(Length::Fixed(intensity * 110.0)).height(4.0)).style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(fill_col, 0.9))),
      ..container::Style::default()
    }),
  )
  .width(110.0)
  .height(4.0)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_col)),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .clip(true)
  .into()
}

fn stat_row(stat: &AbyssalStatViewModel, highlighted: bool) -> Element<'static, Message> {
  let delta = stat.rolled_value - stat.base_value;
  let stat_color = stat_direction_color(stat_roll_direction(stat));
  let intensity = stat_delta_intensity(stat, delta);
  let delta_sign = if delta >= 0.0 { "+" } else { "" };
  let delta_str = format!("{}{}", delta_sign, format_stat_value(delta, &stat.unit_suffix));
  let name_color = if highlighted {
    color::text::ACCENT
  } else {
    color::text::SECONDARY
  };

  let name_el: Element<'static, Message> = text(stat.display_name.clone())
    .font(body::REGULAR)
    .size(11.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(name_color),
    })
    .into();
  let value_row: Element<'static, Message> = row([
    text(format_stat_value(stat.rolled_value, &stat.unit_suffix))
      .font(mono::MEDIUM)
      .size(14.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(8.0).into(),
    text(delta_str)
      .font(mono::REGULAR)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(stat_color),
      })
      .into(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();
  let inner: Element<'static, Message> = row([
    stat_icon_tile(&stat.display_name, stat.icon_id),
    Space::new().width(10.0).into(),
    column([name_el, value_row]).width(Length::Fill).into(),
    stat_intensity_bar(intensity, stat_color),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();

  if highlighted {
    container(inner)
      .padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 8.0,
        right: 8.0,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.08))),
        border: Border {
          color: color::with_alpha(color::text::ACCENT, 0.20),
          radius: 6.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  } else {
    container(inner)
      .padding(Padding {
        top: 4.0,
        bottom: 4.0,
        left: 0.0,
        right: 0.0,
      })
      .into()
  }
}

/// Renders the stat icon tile for a dogma attribute.
fn stat_icon_tile(display_name: &str, icon_id: Option<i32>) -> Element<'static, Message> {
  let mono_label: String = display_name
    .split_whitespace()
    .filter(|w| w.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false))
    .take(3)
    .map(|w| w.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?'))
    .collect();
  let mono_label = if mono_label.is_empty() {
    "?".to_string()
  } else {
    mono_label
  };
  let hue = icon_id.unwrap_or(220) % 360;
  let hue_f = hue as f32;
  let col = Color::from([
    0.35 + 0.25 * (hue_f.to_radians()).cos(),
    0.35 + 0.25 * (hue_f.to_radians() + 2.094).cos(),
    0.35 + 0.25 * (hue_f.to_radians() + 4.189).cos(),
    1.0,
  ]);
  container(
    text(mono_label)
      .font(mono::REGULAR)
      .size(8.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(Color::from_rgb(1.0, 1.0, 1.0)),
      }),
  )
  .width(34.0)
  .height(34.0)
  .center(34.0)
  .style(move |_| container::Style {
    background: Some(Background::Color(col)),
    border: Border {
      color: color::with_alpha(col, 0.65),
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

/// Formats a stat value with its unit suffix, choosing appropriate precision.
fn format_stat_value(value: f64, unit_suffix: &str) -> String {
  let abs = value.abs();
  if unit_suffix == "%" {
    format!("{:.2}{unit_suffix}", value)
  } else if abs >= 1_000.0 {
    format!("{:.0}{unit_suffix}", value)
  } else if abs >= 10.0 {
    format!("{:.1}{unit_suffix}", value)
  } else {
    format!("{:.2}{unit_suffix}", value)
  }
}

/// Renders a character initials tile.
fn char_initials_tile(char_name: &str) -> Element<'static, Message> {
  let initials: String = char_name
    .split_whitespace()
    .take(2)
    .map(|w| w.chars().next().unwrap_or(' '))
    .collect();
  container(
    text(initials)
      .font(body::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(18.0)
  .height(18.0)
  .center(18.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

/// Computes the net-positive roll score for an abyssal item.
///
/// Returns the average signed delta weighted by stat direction. Positive = net good.
pub fn roll_score(item: &AbyssalViewModel) -> f64 {
  if item.stats.is_empty() {
    return 0.0;
  }
  let sum: f64 = item
    .stats
    .iter()
    .filter_map(|s| {
      let delta = s.rolled_value - s.base_value;
      if delta.abs() < 1e-9 {
        return None;
      }
      let pct = if s.base_value.abs() > 1e-9 {
        delta / s.base_value * 100.0
      } else {
        0.0
      };
      Some(if s.high_is_good { pct } else { -pct })
    })
    .sum();
  sum / item.stats.len() as f64
}

/// Checks whether a search token matches any stat display name.
fn token_matches_stat(token: &str, item: &AbyssalViewModel) -> bool {
  let lc = token.to_lowercase();
  item.stats.iter().any(|s| s.display_name.to_lowercase().contains(&lc))
}

/// Returns the set of stat display names matched by the search query.
fn highlighted_stat_names(query: &str, item: &AbyssalViewModel) -> Vec<String> {
  if query.trim().is_empty() {
    return Vec::new();
  }
  let tokens: Vec<&str> = query.split_whitespace().collect();
  item
    .stats
    .iter()
    .filter(|s| {
      tokens
        .iter()
        .any(|t| s.display_name.to_lowercase().contains(&t.to_lowercase()))
    })
    .map(|s| s.display_name.clone())
    .collect()
}

/// Returns true if the item matches the search query (all tokens must match at least one field).
fn item_matches_query(query: &str, item: &AbyssalViewModel, char_name: &str) -> bool {
  if query.trim().is_empty() {
    return true;
  }
  let tokens: Vec<&str> = query.split_whitespace().collect();
  tokens.iter().all(|token| {
    let lc = token.to_lowercase();
    item.base_type_name.to_lowercase().contains(&lc)
      || item.mutaplasmid_tier.to_lowercase().contains(&lc)
      || char_name.to_lowercase().contains(&lc)
      || item.location.to_lowercase().contains(&lc)
      || token_matches_stat(token, item)
  })
}

/// Renders the header section of an abyssal card (icon, name, tier badge, price).
fn abyssal_card_header(item: &AbyssalViewModel) -> Element<'_, Message> {
  let price_label = item
    .muta_price_isk
    .map(format::fmt_isk)
    .unwrap_or_else(|| "\u{2014}".to_string());
  container(
    row([
      type_icon_tile(&item.base_type_name, item.type_id),
      Space::new().width(12.0).into(),
      column([
        row([
          text(item.base_type_name.clone())
            .font(body::MEDIUM)
            .size(13.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::PRIMARY),
            })
            .width(Length::Fill)
            .into(),
          tier_badge(&item.mutaplasmid_tier),
        ])
        .align_y(iced::alignment::Vertical::Center)
        .spacing(8.0)
        .into(),
        Space::new().height(2.0).into(),
        text(format!("{} Mutaplasmid", item.mutaplasmid_tier))
          .font(body::REGULAR)
          .size(11.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .width(Length::Fill)
      .into(),
      column([text(price_label)
        .font(mono::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::ACCENT),
        })
        .into()])
      .align_x(iced::alignment::Horizontal::Right)
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 12.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill)
  .into()
}

/// Renders the stats section of an abyssal card.
fn abyssal_card_stats<'a>(item: &'a AbyssalViewModel, hi_stats: &[String]) -> Element<'a, Message> {
  let mut sorted_stats: Vec<&AbyssalStatViewModel> = item.stats.iter().collect();
  sorted_stats.sort_by(|a, b| a.display_name.cmp(&b.display_name));

  let stat_rows: Vec<Element<'_, Message>> = sorted_stats
    .iter()
    .map(|s| {
      let highlighted = hi_stats.iter().any(|hn| hn == &s.display_name);
      stat_row(s, highlighted)
    })
    .collect();

  container(column(stat_rows).spacing(0.0))
    .padding(Padding {
      top: 6.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill)
    .into()
}

/// Renders the footer section of an abyssal card (character name and location).
fn abyssal_card_footer<'a>(item: &'a AbyssalViewModel, char_name: &'a str) -> Element<'a, Message> {
  container(
    row([
      char_initials_tile(char_name),
      Space::new().width(8.0).into(),
      text(char_name.to_string())
        .font(body::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().width(8.0).into(),
      text("\u{00b7}")
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      Space::new().width(8.0).into(),
      text(item.location.clone())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .width(Length::Fill)
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 16.0,
    right: 16.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

/// Renders a single abyssal card.
fn abyssal_card<'a>(item: &'a AbyssalViewModel, char_name: &'a str, search_query: &'a str) -> Element<'a, Message> {
  let hi_stats = highlighted_stat_names(search_query, item);
  let header = abyssal_card_header(item);
  let stats_area = abyssal_card_stats(item, &hi_stats);
  let footer = abyssal_card_footer(item, char_name);

  container(column([header, stats_area, footer]))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

/// Renders the filter bar (search, toggle, summary strip).
fn filter_bar(state: &State) -> Element<'_, Message> {
  let abyssals_state = &state.abyssals;
  let (total_count, total_value, avg_score) = filter_bar_stats(state);
  let search_box = filter_search_box(&abyssals_state.search_query);
  let toggle_el = only_positive_button(abyssals_state.only_positive);
  let summary_strip = filter_summary_strip(total_count, total_value, avg_score);
  let controls_row: Element<'_, Message> = container(
    row([toggle_el, Space::new().width(Length::Fill).into(), summary_strip]).align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 14.0,
    left: 28.0,
    right: 28.0,
  })
  .width(Length::Fill)
  .into();
  container(column([search_box, controls_row]))
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn filter_bar_stats(state: &State) -> (usize, f64, f64) {
  let abyssals_state = &state.abyssals;
  let visible: Vec<&AbyssalViewModel> = state
    .abyssals
    .abyssals
    .iter()
    .filter(|item| {
      let char_name = state
        .characters
        .iter()
        .find(|c| *c.id() == item.character_id)
        .map(|c| c.name().as_str())
        .unwrap_or("");
      let passes_query = item_matches_query(&abyssals_state.search_query, item, char_name);
      let passes_type = abyssals_state.selected_type_id.is_none_or(|tid| item.type_id == tid);
      let passes_score = !abyssals_state.only_positive || roll_score(item) >= 0.0;
      passes_query && passes_type && passes_score
    })
    .collect();
  let total_count = visible.len();
  let total_value: f64 = visible.iter().filter_map(|i| i.muta_price_isk).sum();
  let avg_score = if visible.is_empty() {
    0.0
  } else {
    visible.iter().map(|i| roll_score(i)).sum::<f64>() / visible.len() as f64
  };
  (total_count, total_value, avg_score)
}

fn filter_search_box(query: &str) -> Element<'_, Message> {
  container(
    row([text_input("Search by item or stat\u{2026}", query)
      .on_input(Message::SearchChanged)
      .font(body::REGULAR)
      .size(14.0)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: 14.0,
        right: 14.0,
      })
      .style(|_, _| iced::widget::text_input::Style {
        background: Background::Color(color::surface::SUNKEN),
        border: Border {
          color: color::border::SUBTLE,
          radius: 10.0.into(),
          width: 1.0,
        },
        icon: color::text::TERTIARY,
        placeholder: color::text::TERTIARY,
        selection: color::with_alpha(color::text::ACCENT, 0.3),
        value: color::text::PRIMARY,
      })
      .width(Length::Fill)
      .into()])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 8.0,
    left: 28.0,
    right: 28.0,
  })
  .width(Length::Fill)
  .into()
}

fn only_positive_button(active: bool) -> Element<'static, Message> {
  let check_bg = if active {
    Some(Background::Color(color::text::SUCCESS))
  } else {
    None
  };
  let check_border_color = if active {
    color::text::SUCCESS
  } else {
    color::border::DEFAULT
  };
  let label_color = if active {
    color::text::SUCCESS
  } else {
    color::text::SECONDARY
  };
  let btn_bg = if active {
    Some(Background::Color(color::with_alpha(color::text::SUCCESS, 0.10)))
  } else {
    None
  };
  let btn_border_color = if active {
    color::with_alpha(color::text::SUCCESS, 0.45)
  } else {
    color::border::SUBTLE
  };
  button(
    row([
      container(Space::new().width(12.0).height(12.0))
        .style(move |_| container::Style {
          background: check_bg,
          border: Border {
            color: check_border_color,
            radius: 3.0.into(),
            width: 1.0,
          },
          ..container::Style::default()
        })
        .into(),
      Space::new().width(6.0).into(),
      text("Only net-positive rolls")
        .font(body::REGULAR)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(label_color),
        })
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .on_press(Message::OnlyPositiveToggled)
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 12.0,
    right: 12.0,
  })
  .style(move |_, _| button::Style {
    background: btn_bg,
    border: Border {
      color: btn_border_color,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn filter_avg_roll_stat(avg_score: f64) -> Element<'static, Message> {
  let avg_color = if avg_score >= 0.0 {
    color::text::SUCCESS
  } else {
    color::text::DANGER
  };
  let avg_sign = if avg_score >= 0.0 { "+" } else { "" };
  container(
    row([
      text("Avg roll")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      Space::new().width(6.0).into(),
      text(format!("{}{:.2}%", avg_sign, avg_score))
        .font(mono::MEDIUM)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(avg_color),
        })
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .into()
}

fn filter_summary_strip(total_count: usize, total_value: f64, avg_score: f64) -> Element<'static, Message> {
  container(
    row([
      summary_stat("Modules", &total_count.to_string()),
      Space::new().width(18.0).into(),
      summary_stat("Est. value", &format::fmt_isk(total_value)),
      Space::new().width(18.0).into(),
      filter_avg_roll_stat(avg_score),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 14.0,
    right: 14.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

/// Renders a small label + value stat for the summary strip.
fn summary_stat(label: &str, value: &str) -> Element<'static, Message> {
  row([
    text(label.to_string())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    Space::new().width(6.0).into(),
    text(value.to_string())
      .font(mono::MEDIUM)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

/// Renders the card grid body.
fn card_grid<'a>(state: &'a State, search_query: &'a str, only_positive: bool) -> Element<'a, Message> {
  let char_name_map: std::collections::HashMap<i64, &str> =
    state.characters.iter().map(|c| (*c.id(), c.name().as_str())).collect();

  let items: Vec<&AbyssalViewModel> = state
    .abyssals
    .abyssals
    .iter()
    .filter(|item| {
      let char_name = char_name_map.get(&item.character_id).copied().unwrap_or("");
      let passes_query = item_matches_query(search_query, item, char_name);
      let passes_type = state.abyssals.selected_type_id.is_none_or(|tid| item.type_id == tid);
      let passes_score = !only_positive || roll_score(item) >= 0.0;
      passes_query && passes_type && passes_score
    })
    .collect();

  if items.is_empty() {
    return container(
      text("No abyssal modules found.")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center(Length::Fill)
    .into();
  }

  let cards: Vec<Element<'_, Message>> = items
    .iter()
    .map(|item| {
      let char_name = char_name_map.get(&item.character_id).copied().unwrap_or("");
      container(abyssal_card(item, char_name, search_query))
        .padding(Padding {
          top: 0.0,
          bottom: 16.0,
          left: 0.0,
          right: 0.0,
        })
        .width(Length::Fill)
        .into()
    })
    .collect();

  scrollable(
    container(column(cards).spacing(0.0))
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
  )
  .height(Length::Fill)
  .into()
}

/// Renders the filter sidebar with module type picker.
fn filter_sidebar<'a>(state: &'a State) -> Element<'a, Message> {
  let abyssals_state = &state.abyssals;
  let all_btn = sidebar_all_button(abyssals_state.selected_type_id.is_none());

  let mut type_ids: Vec<i32> = abyssals_state
    .abyssals
    .iter()
    .map(|i| i.type_id)
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();
  type_ids.sort();

  let type_btns: Vec<Element<'_, Message>> = type_ids
    .iter()
    .map(|&tid| {
      let is_active = abyssals_state.selected_type_id == Some(tid);
      let type_name = abyssals_state
        .abyssals
        .iter()
        .find(|i| i.type_id == tid)
        .map(|i| i.base_type_name.as_str())
        .unwrap_or("Unknown");
      sidebar_type_button(type_name, tid, is_active)
    })
    .collect();

  let reset_btn: Element<'_, Message> = button(text("Reset filters").font(mono::REGULAR).size(9.0).style(
    |_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    },
  ))
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::FilterReset)
  .style(|_, _| button::Style {
    background: None,
    border: Border {
      color: color::border::SUBTLE,
      radius: 5.0.into(),
      width: 1.0,
    },
    text_color: color::text::TERTIARY,
    ..button::Style::default()
  })
  .into();

  let mut sidebar_items: Vec<Element<'_, Message>> = vec![all_btn, Space::new().height(6.0).into()];
  sidebar_items.extend(type_btns);
  sidebar_items.push(Space::new().height(12.0).into());
  sidebar_items.push(reset_btn);

  container(
    scrollable(
      container(column(sidebar_items).spacing(2.0))
        .padding(Padding {
          top: 16.0,
          bottom: 16.0,
          left: 12.0,
          right: 12.0,
        })
        .width(Length::Fill),
    )
    .height(Length::Fill),
  )
  .width(220.0)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn sidebar_all_button(active: bool) -> Element<'static, Message> {
  let text_color = if active {
    color::text::ACCENT
  } else {
    color::text::PRIMARY
  };
  let bg = if active {
    Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.10)))
  } else {
    None
  };
  let border_color = if active {
    color::text::ACCENT
  } else {
    color::border::SUBTLE
  };
  button(
    text("All module types")
      .font(body::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::TypeSelected(None))
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      color: border_color,
      radius: 5.0.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn sidebar_type_button(type_name: &str, tid: i32, active: bool) -> Element<'static, Message> {
  let label_color = if active {
    color::text::ACCENT
  } else {
    color::text::SECONDARY
  };
  let bg = if active {
    Some(Background::Color(color::with_alpha(color::text::ACCENT, 0.08)))
  } else {
    None
  };
  let border_color = if active {
    color::with_alpha(color::text::ACCENT, 0.3)
  } else {
    Color::TRANSPARENT
  };
  button(
    text(type_name.to_string())
      .font(body::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(label_color),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::TypeSelected(Some(tid)))
  .style(move |_, _| button::Style {
    background: bg,
    border: Border {
      color: border_color,
      radius: 5.0.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

/// State for the abyssals tab sub-view.
#[derive(Clone, Debug, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct AbyssalsState {
  /// All loaded abyssal view models.
  pub abyssals: Vec<AbyssalViewModel>,
  /// Current text in the search input.
  pub search_query: String,
  /// When true, only items with net-positive roll scores are shown.
  pub only_positive: bool,
  /// Currently selected module type ID for sidebar filtering.
  pub selected_type_id: Option<i32>,
}

/// Builder for the abyssals tab.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new abyssals tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the abyssals tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;

    if state.abyssals.abyssals.is_empty() {
      return container(
        text("No abyssal modules synced yet.\nSync your characters to load abyssal data.")
          .font(body::REGULAR)
          .size(13.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .center(Length::Fill)
      .into();
    }

    let filter_bar_el = filter_bar(state);
    let sidebar_el = filter_sidebar(state);
    let grid_el = card_grid(state, &state.abyssals.search_query, state.abyssals.only_positive);

    let main_area: Element<'a, Message> = row([sidebar_el, grid_el])
      .width(Length::Fill)
      .height(Length::Fill)
      .into();

    column([filter_bar_el, main_area])
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod roll_score {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_item(stats: Vec<AbyssalStatViewModel>) -> AbyssalViewModel {
      AbyssalViewModel {
        base_type_name: "Test Module".to_string(),
        character_id: 1,
        item_id: 100,
        location: "Jita".to_string(),
        muta_price_isk: None,
        mutaplasmid_color_hue: 220,
        mutaplasmid_tier: "Decayed".to_string(),
        stats,
        type_id: 47804,
      }
    }

    fn make_stat(display_name: &str, base: f64, rolled: f64, high_is_good: bool) -> AbyssalStatViewModel {
      AbyssalStatViewModel {
        attribute_id: 1,
        base_value: base,
        display_name: display_name.to_string(),
        high_is_good,
        icon_id: None,
        max_mult: 1.5,
        min_mult: 0.7,
        rolled_value: rolled,
        unit_suffix: "".to_string(),
      }
    }

    #[test]
    fn it_returns_zero_for_empty_stats() {
      let item = make_item(vec![]);

      assert_eq!(roll_score(&item), 0.0);
    }

    #[test]
    fn it_returns_positive_for_good_rolls() {
      let stats = vec![
        make_stat("Damage", 100.0, 110.0, true),
        make_stat("CPU Use", 50.0, 45.0, false),
      ];
      let item = make_item(stats);

      let score = roll_score(&item);

      assert!(score > 0.0);
    }

    #[test]
    fn it_returns_negative_for_bad_rolls() {
      let stats = vec![
        make_stat("Damage", 100.0, 90.0, true),
        make_stat("CPU Use", 50.0, 55.0, false),
      ];
      let item = make_item(stats);

      let score = roll_score(&item);

      assert!(score < 0.0);
    }
  }

  mod item_matches_query {
    use super::*;

    fn make_item(base_type_name: &str, tier: &str, location: &str) -> AbyssalViewModel {
      AbyssalViewModel {
        base_type_name: base_type_name.to_string(),
        character_id: 1,
        item_id: 100,
        location: location.to_string(),
        muta_price_isk: None,
        mutaplasmid_color_hue: 220,
        mutaplasmid_tier: tier.to_string(),
        stats: vec![],
        type_id: 47804,
      }
    }

    #[test]
    fn it_matches_empty_query() {
      let item = make_item("Shield Extender", "Decayed", "Jita");

      assert!(item_matches_query("", &item, "Alice"));
    }

    #[test]
    fn it_matches_base_type_name() {
      let item = make_item("Shield Extender", "Decayed", "Jita");

      assert!(item_matches_query("shield", &item, "Alice"));
    }

    #[test]
    fn it_matches_tier() {
      let item = make_item("Shield Extender", "Decayed", "Jita");

      assert!(item_matches_query("decayed", &item, "Alice"));
    }

    #[test]
    fn it_matches_character_name() {
      let item = make_item("Shield Extender", "Decayed", "Jita");

      assert!(item_matches_query("alice", &item, "Alice"));
    }

    #[test]
    fn it_matches_location() {
      let item = make_item("Shield Extender", "Decayed", "Jita");

      assert!(item_matches_query("jita", &item, "Alice"));
    }

    #[test]
    fn it_requires_all_tokens_to_match() {
      let item = make_item("Shield Extender", "Decayed", "Jita");

      assert!(!item_matches_query("shield unstable", &item, "Alice"));
    }

    #[test]
    fn it_returns_false_when_no_token_matches() {
      let item = make_item("Shield Extender", "Decayed", "Jita");

      assert!(!item_matches_query("webifier", &item, "Alice"));
    }
  }

  mod format_stat_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_percentage_with_two_decimals() {
      assert_eq!(format_stat_value(35.5, "%"), "35.50%");
    }

    #[test]
    fn it_formats_large_values_without_decimals() {
      assert_eq!(format_stat_value(50_000.0, " kg"), "50000 kg");
    }

    #[test]
    fn it_formats_medium_values_without_decimals() {
      assert_eq!(format_stat_value(1_500.0, " HP"), "1500 HP");
    }

    #[test]
    fn it_formats_small_values_with_one_decimal() {
      assert_eq!(format_stat_value(25.5, " tf"), "25.5 tf");
    }

    #[test]
    fn it_formats_tiny_values_with_two_decimals() {
      assert_eq!(format_stat_value(4.75, " GJ"), "4.75 GJ");
    }
  }

  mod tier_badge_color {
    use super::*;

    #[test]
    fn it_returns_a_color_for_decayed() {
      let col = tier_badge_color("Decayed");

      assert!(col.r + col.g + col.b > 0.0);
    }

    #[test]
    fn it_returns_a_color_for_unstable() {
      let col = tier_badge_color("Unstable");

      assert!(col.r > 0.5);
    }

    #[test]
    fn it_returns_a_color_for_glorified_unstable() {
      let col = tier_badge_color("Glorified Unstable");

      assert!(col.r + col.g + col.b > 0.0);
    }
  }
}
