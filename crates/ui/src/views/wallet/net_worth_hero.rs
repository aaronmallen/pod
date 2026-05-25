//! Net worth hero section — value display, change badge, chart, and timeframe.

pub mod composition_chip;
pub mod timeframe_picker;

pub use composition_chip::Component as CompositionChip;
use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, stack, text},
};
pub use timeframe_picker::Component as TimeframePicker;

use crate::{
  components::{HoverData, LineChart},
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::{Message, State, Timeframe, WalletCharacter},
};

fn hero_lhs(current: f64, change: f64, change_pct: f64, is_up: bool) -> Element<'static, Message> {
  let change_color = if is_up {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let hero_label: Element<'_, Message> = text("NET WORTH")
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into();
  let hero_value: Element<'_, Message> = text(format::fmt_isk_full(current))
    .font(body::MEDIUM)
    .size(32.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into();
  let change_sign = if is_up { "▲" } else { "▼" };
  let change_str = format!(
    "{} {} · {}{:.2}%",
    change_sign,
    format::fmt_isk(change.abs()),
    if change_pct >= 0.0 { "+" } else { "-" },
    change_pct.abs(),
  );
  let change_badge: Element<'_, Message> = container(text(change_str).font(mono::MEDIUM).size(11.0).style(
    move |_: &Theme| iced::widget::text::Style {
      color: Some(change_color),
    },
  ))
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 10.0,
    right: 10.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(change_color, 0.10))),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into();
  column([
    hero_label,
    Space::new().height(6.0).into(),
    hero_value,
    Space::new().height(8.0).into(),
    change_badge,
  ])
  .into()
}

fn x_labels_for_timeframe(tf: &Timeframe, n: usize) -> Vec<String> {
  let days = tf.days();
  let label_for = |i: usize| -> String {
    if i == 4 {
      return "today".to_string();
    }
    let series_idx = (i as f32 / 4.0 * (n.saturating_sub(1)) as f32).round() as usize;
    let days_ago = if n > 1 {
      (days as f32 * (1.0 - series_idx as f32 / (n.saturating_sub(1)) as f32)).round() as usize
    } else {
      days
    };
    if days_ago == 0 {
      return "today".to_string();
    }
    if days <= 30 {
      format!("{}d ago", days_ago)
    } else if days <= 90 {
      let weeks = (days_ago + 3) / 7;
      format!("{}w ago", weeks.max(1))
    } else {
      let months = (days_ago + 14) / 30;
      format!("{}mo ago", months.max(1))
    }
  };
  (0..5).map(label_for).collect()
}

fn tooltip_date_label(series_len: usize, index: usize) -> String {
  let days_ago = series_len.saturating_sub(1).saturating_sub(index);
  match days_ago {
    0 => "TODAY".to_string(),
    1 => "YESTERDAY".to_string(),
    n => format!("{n} DAYS AGO"),
  }
}

fn hover_tooltip(hover: &HoverData, series_len: usize, prev_value: Option<f64>) -> Element<'static, Message> {
  let x_frac = hover.x_frac.clamp(0.0, 1.0);
  let x_left_raw = (x_frac * 100.0).round() as u16;
  let flip = x_frac > 0.65;
  let x_left = x_left_raw.min(80);
  let x_right = 100u16.saturating_sub(x_left);

  let date_str = tooltip_date_label(series_len, hover.index);
  let value_str = format!("{} ISK", format::fmt_isk_full(hover.value));

  let date_el: Element<'_, Message> = text(date_str)
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into();
  let value_el: Element<'_, Message> = text(value_str)
    .font(mono::MEDIUM)
    .size(14.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into();

  let mut card_children: Vec<Element<'_, Message>> = vec![date_el, Space::new().height(4.0).into(), value_el];

  if let Some(prev) = prev_value {
    let delta = hover.value - prev;
    let pct = if prev.abs() > 0.01 { delta / prev * 100.0 } else { 0.0 };
    let sign = if delta >= 0.0 { "+" } else { "" };
    let delta_str = format!(
      "{}{}  ({}{}%)  day",
      sign,
      format::fmt_isk(delta),
      sign,
      format!("{:.1}", pct.abs())
    );
    let delta_color = if delta >= 0.0 {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let delta_el: Element<'_, Message> = text(delta_str)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(delta_color),
      })
      .into();
    card_children.push(Space::new().height(4.0).into());
    card_children.push(delta_el);
  }

  let card = container(column([
    column(card_children).into(),
    Space::new().width(180.0).height(0.0).into(),
  ]))
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::DEFAULT,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  if flip {
    row([
      Space::new().width(Length::FillPortion(x_left)).into(),
      card.into(),
      Space::new().width(Length::FillPortion(x_right)).into(),
    ])
    .into()
  } else {
    row([
      Space::new().width(Length::FillPortion(x_left)).into(),
      card.into(),
      Space::new().width(Length::FillPortion(x_right)).into(),
    ])
    .into()
  }
}

fn chart_section<'a>(
  top_row: Element<'a, Message>,
  series: Vec<f64>,
  is_up: bool,
  timeframe: &Timeframe,
  chart_hover: Option<&'a HoverData>,
  characters: &'a [WalletCharacter],
  all_wallets: bool,
) -> Element<'a, Message> {
  use crate::style::spacing;

  let chart_color = if is_up {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let bottom_rule: Element<'_, Message> = container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into();

  let x_labels = x_labels_for_timeframe(timeframe, series.len());
  let chart = LineChart::new(series.clone(), chart_color)
    .with_labels(x_labels, format::fmt_isk)
    .with_hover(Message::HoverChanged)
    .render(Length::Fill, 180.0);

  let chart_el: Element<'_, Message> = if let Some(hover) = chart_hover {
    let prev_value = if hover.index > 0 {
      series.get(hover.index - 1).copied()
    } else {
      None
    };
    let tooltip = hover_tooltip(hover, series.len(), prev_value);
    stack([chart, tooltip]).into()
  } else {
    chart
  };

  let mut column_children: Vec<Element<'_, Message>> = vec![top_row, Space::new().height(16.0).into(), chart_el];

  if all_wallets && !characters.is_empty() {
    let total_nw: f64 = characters.iter().map(|c| c.liquid + c.assets + c.escrow).sum();
    if total_nw > 0.0 {
      column_children.push(Space::new().height(16.0).into());
      column_children.push(by_character_bar(characters, total_nw));
    }
  }

  column([
    container(column(column_children))
      .width(Length::Fill)
      .padding(Padding {
        top: 24.0,
        bottom: 12.0,
        left: spacing::SPACE_8,
        right: spacing::SPACE_8,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into(),
    bottom_rule,
  ])
  .width(Length::Fill)
  .into()
}

fn by_character_bar<'a>(characters: &'a [WalletCharacter], total_nw: f64) -> Element<'a, Message> {
  let mut sorted: Vec<&WalletCharacter> = characters.iter().collect();
  sorted.sort_by(|a, b| {
    let nw_a = a.liquid + a.assets + a.escrow;
    let nw_b = b.liquid + b.assets + b.escrow;
    nw_b.partial_cmp(&nw_a).unwrap_or(std::cmp::Ordering::Equal)
  });

  let section_label: Element<'_, Message> = text("BY CHARACTER")
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into();

  let bar_slices: Vec<Element<'_, Message>> = sorted
    .iter()
    .map(|c| {
      let nw = c.liquid + c.assets + c.escrow;
      let share = (nw / total_nw * 100.0).round() as u16;
      let share = share.max(1);
      let slice_color = hsl_to_color(c.portrait_tone as f32, 0.50, 0.48);
      container(Space::new().width(Length::FillPortion(share)).height(6.0))
        .width(Length::FillPortion(share))
        .height(6.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(slice_color)),
          ..container::Style::default()
        })
        .into()
    })
    .collect();

  let bar_row: Element<'_, Message> = container(row(bar_slices).width(Length::Fill).height(6.0))
    .width(Length::Fill)
    .height(6.0)
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into();

  let legend_items: Vec<Element<'_, Message>> = sorted
    .iter()
    .map(|c| {
      let nw = c.liquid + c.assets + c.escrow;
      let pct = nw / total_nw * 100.0;
      let swatch_color = hsl_to_color(c.portrait_tone as f32, 0.50, 0.48);
      let swatch: Element<'_, Message> = container(Space::new().width(8.0).height(8.0))
        .width(8.0)
        .height(8.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(swatch_color)),
          border: Border {
            radius: 2.0.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into();
      let name_el: Element<'_, Message> = text(c.name.clone())
        .font(crate::style::typography::body::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into();
      let value_el: Element<'_, Message> = text(format::fmt_isk(nw))
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::MUTED),
        })
        .into();
      let pct_str = format!("· {:.0}%", pct);
      let pct_el: Element<'_, Message> = text(pct_str)
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into();
      row([
        swatch,
        Space::new().width(6.0).into(),
        name_el,
        Space::new().width(6.0).into(),
        value_el,
        Space::new().width(4.0).into(),
        pct_el,
      ])
      .align_y(iced::alignment::Vertical::Center)
      .into()
    })
    .collect();

  let legend_row: Element<'_, Message> = row(legend_items).spacing(18.0).wrap().into();

  column([
    section_label,
    Space::new().height(8.0).into(),
    bar_row,
    Space::new().height(10.0).into(),
    legend_row,
  ])
  .into()
}

fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
  let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
  let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
  let m = l - c / 2.0;
  let (r, g, b) = if h < 60.0 {
    (c, x, 0.0)
  } else if h < 120.0 {
    (x, c, 0.0)
  } else if h < 180.0 {
    (0.0, c, x)
  } else if h < 240.0 {
    (0.0, x, c)
  } else if h < 300.0 {
    (x, 0.0, c)
  } else {
    (c, 0.0, x)
  };
  Color::from_rgb(r + m, g + m, b + m)
}

/// Builder for the net worth hero section.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new net worth hero component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the net worth hero into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let series = state.chart_series.clone();
    let current = state.total_liquid() + state.total_assets() + state.total_escrow();
    let change = state.net_worth_change;
    let start_balance = (current - change).max(0.0);
    let change_pct = if start_balance > 0.01 {
      change / start_balance * 100.0
    } else {
      0.0
    };
    let is_up = change >= 0.0;
    let left_col = hero_lhs(current, change, change_pct, is_up);
    let comp_chips: Element<'_, Message> = row([
      CompositionChip::new("Liquid", state.total_liquid(), color::accent::PLASMA).render(),
      Space::new().width(10.0).into(),
      CompositionChip::new("Assets", state.total_assets(), color::text::SECONDARY).render(),
      Space::new().width(10.0).into(),
      CompositionChip::new("Escrow", state.total_escrow(), color::status::CAUTION).render(),
    ])
    .into();
    let top_row: Element<'_, Message> = row([
      left_col,
      Space::new().width(Length::Fill).into(),
      comp_chips,
      Space::new().width(24.0).into(),
      TimeframePicker::new(&state.timeframe).render(),
    ])
    .align_y(iced::alignment::Vertical::Top)
    .into();
    let all_wallets = state.selected_character().is_none() && state.selected_corporation().is_none();
    chart_section(
      top_row,
      series,
      is_up,
      &state.timeframe,
      state.chart_hover.as_ref(),
      &state.characters,
      all_wallets,
    )
  }
}
