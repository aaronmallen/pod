//! Chart section of the net worth hero: line chart, tooltip, and character bar.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, stack, text},
};

use crate::{
  components::{HoverData, LineChart},
  format,
  style::{color, spacing, typography::mono},
  views::wallet::{Message, Timeframe, WalletCharacter},
};

/// Builder for the chart section of the net worth hero.
pub struct ChartSection<'a> {
  /// Whether to show per-character breakdown bar.
  all_wallets: bool,
  /// Optional hover data for the tooltip overlay.
  chart_hover: Option<&'a HoverData>,
  /// The list of wallet characters for the by-character bar.
  characters: &'a [WalletCharacter],
  /// Whether the net worth change is positive.
  is_up: bool,
  /// The chart series data points.
  series: Vec<f64>,
  /// The selected timeframe.
  timeframe: &'a Timeframe,
  /// The top-row element (left col + spacer + chips + timeframe picker).
  top_row: Element<'a, Message>,
}

impl<'a> ChartSection<'a> {
  /// Creates a new `ChartSection` builder.
  pub fn new(
    top_row: Element<'a, Message>,
    series: Vec<f64>,
    is_up: bool,
    timeframe: &'a Timeframe,
    chart_hover: Option<&'a HoverData>,
    characters: &'a [WalletCharacter],
    all_wallets: bool,
  ) -> Self {
    Self {
      all_wallets,
      chart_hover,
      characters,
      is_up,
      series,
      timeframe,
      top_row,
    }
  }

  /// Renders the chart section into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let chart_color = net_worth_color(self.is_up);
    let bottom_rule = bottom_border_rule();
    let chart_el = build_chart_element(self.series.clone(), chart_color, self.timeframe, self.chart_hover);
    let mut column_children: Vec<Element<'_, Message>> = vec![self.top_row, Space::new().height(16.0).into(), chart_el];
    append_character_bar(&mut column_children, self.all_wallets, self.characters);
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
}

fn net_worth_color(is_up: bool) -> Color {
  if is_up {
    color::status::ONLINE
  } else {
    color::status::DANGER
  }
}

fn bottom_border_rule<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn build_chart_element<'a>(
  series: Vec<f64>,
  chart_color: Color,
  timeframe: &'a Timeframe,
  chart_hover: Option<&'a HoverData>,
) -> Element<'a, Message> {
  let x_labels = x_labels_for_timeframe(timeframe, series.len());
  let chart = LineChart::new(series.clone(), chart_color)
    .with_labels(x_labels, format::fmt_isk)
    .with_hover(Message::HoverChanged)
    .render(Length::Fill, 180.0);
  if let Some(hover) = chart_hover {
    let prev_value = series
      .get(hover.index.saturating_sub(1))
      .copied()
      .filter(|_| hover.index > 0);
    let tooltip = hover_tooltip(hover, series.len(), prev_value);
    stack([chart, tooltip]).into()
  } else {
    chart
  }
}

fn append_character_bar<'a>(
  children: &mut Vec<Element<'a, Message>>,
  all_wallets: bool,
  characters: &'a [WalletCharacter],
) {
  if !all_wallets || characters.is_empty() {
    return;
  }
  let total_nw: f64 = characters.iter().map(|c| character_net_worth(c)).sum();
  if total_nw > 0.0 {
    children.push(Space::new().height(16.0).into());
    children.push(by_character_bar(characters, total_nw));
  }
}

fn x_labels_for_timeframe(tf: &Timeframe, n: usize) -> Vec<String> {
  let days = tf.days();
  (0..5).map(|i| x_label_at(i, days, n)).collect()
}

fn x_label_at(i: usize, days: usize, n: usize) -> String {
  let series_idx = (i as f32 / 4.0 * (n.saturating_sub(1)) as f32).round() as usize;
  let days_ago = if n > 1 {
    (days as f32 * (1.0 - series_idx as f32 / (n.saturating_sub(1)) as f32)).round() as usize
  } else {
    days
  };
  if i == 4 || days_ago == 0 {
    return "today".to_string();
  }
  days_ago_label(days, days_ago)
}

fn days_ago_label(days: usize, days_ago: usize) -> String {
  if days <= 30 {
    format!("{}d ago", days_ago)
  } else if days <= 90 {
    let weeks = (days_ago + 3) / 7;
    format!("{}w ago", weeks.max(1))
  } else {
    let months = (days_ago + 14) / 30;
    format!("{}mo ago", months.max(1))
  }
}

fn tooltip_date_label(series_len: usize, index: usize) -> String {
  let days_ago = series_len.saturating_sub(1).saturating_sub(index);
  match days_ago {
    0 => "TODAY".to_string(),
    1 => "YESTERDAY".to_string(),
    n => format!("{n} DAYS AGO"),
  }
}

fn tooltip_delta_row(value: f64, prev: f64) -> Vec<Element<'static, Message>> {
  let delta = value - prev;
  let pct = if prev.abs() > 0.01 { delta / prev * 100.0 } else { 0.0 };
  let sign = if delta >= 0.0 { "+" } else { "" };
  let delta_str = format!("{}{}  ({}{:.1}%)  day", sign, format::fmt_isk(delta), sign, pct.abs());
  let delta_color = if delta >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let delta_el: Element<'static, Message> = text(delta_str)
    .font(mono::REGULAR)
    .size(10.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(delta_color),
    })
    .into();
  vec![Space::new().height(4.0).into(), delta_el]
}

fn tooltip_x_portions(x_frac: f32) -> (u16, u16) {
  let x_left = ((x_frac.clamp(0.0, 1.0) * 100.0).round() as u16).min(80);
  let x_right = 100u16.saturating_sub(x_left);
  (x_left, x_right)
}

fn hover_tooltip(hover: &HoverData, series_len: usize, prev_value: Option<f64>) -> Element<'static, Message> {
  let (x_left, x_right) = tooltip_x_portions(hover.x_frac);

  let date_el: Element<'_, Message> = text(tooltip_date_label(series_len, hover.index))
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into();
  let value_el: Element<'_, Message> = text(format!("{} ISK", format::fmt_isk_full(hover.value)))
    .font(mono::MEDIUM)
    .size(14.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .into();

  let mut card_children: Vec<Element<'_, Message>> = vec![date_el, Space::new().height(4.0).into(), value_el];
  if let Some(prev) = prev_value {
    card_children.extend(tooltip_delta_row(hover.value, prev));
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

  row([
    Space::new().width(Length::FillPortion(x_left)).into(),
    card.into(),
    Space::new().width(Length::FillPortion(x_right)).into(),
  ])
  .into()
}

fn character_net_worth(c: &WalletCharacter) -> f64 {
  c.liquid + c.assets + c.escrow
}

fn sort_characters_by_net_worth<'a>(characters: &'a [WalletCharacter]) -> Vec<&'a WalletCharacter> {
  let mut sorted: Vec<&WalletCharacter> = characters.iter().collect();
  sorted.sort_by(|a, b| {
    character_net_worth(b)
      .partial_cmp(&character_net_worth(a))
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  sorted
}

fn by_character_section_label<'a>() -> Element<'a, Message> {
  text("BY CHARACTER")
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into()
}

fn by_character_bar<'a>(characters: &'a [WalletCharacter], total_nw: f64) -> Element<'a, Message> {
  let sorted = sort_characters_by_net_worth(characters);
  let section_label = by_character_section_label();
  let bar_slices: Vec<Element<'_, Message>> = sorted.iter().map(|c| character_bar_slice(c, total_nw)).collect();

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

  let legend_items: Vec<Element<'_, Message>> = sorted.iter().map(|c| character_legend_item(c, total_nw)).collect();
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

fn character_bar_slice<'a>(c: &WalletCharacter, total_nw: f64) -> Element<'a, Message> {
  let nw = character_net_worth(c);
  let share = ((nw / total_nw * 100.0).round() as u16).max(1);
  let slice_color = hsl_to_color(c.portrait_tone as f32, 0.50, 0.48);
  container(Space::new().width(Length::FillPortion(share)).height(6.0))
    .width(Length::FillPortion(share))
    .height(6.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(slice_color)),
      ..container::Style::default()
    })
    .into()
}

fn character_legend_item<'a>(c: &'a WalletCharacter, total_nw: f64) -> Element<'a, Message> {
  let nw = character_net_worth(c);
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
  let pct_el: Element<'_, Message> = text(format!("· {:.0}%", pct))
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
}

fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
  let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
  let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
  let m = l - c / 2.0;
  let (r, g, b) = hsl_rgb_components(h, c, x);
  Color::from_rgb(r + m, g + m, b + m)
}

fn hsl_rgb_components(h: f32, c: f32, x: f32) -> (f32, f32, f32) {
  if h < 120.0 {
    hsl_rgb_low(h, c, x)
  } else if h < 240.0 {
    hsl_rgb_mid(h, c, x)
  } else if h < 300.0 {
    (x, 0.0, c)
  } else {
    (c, 0.0, x)
  }
}

fn hsl_rgb_low(h: f32, c: f32, x: f32) -> (f32, f32, f32) {
  if h < 60.0 { (c, x, 0.0) } else { (x, c, 0.0) }
}

fn hsl_rgb_mid(h: f32, c: f32, x: f32) -> (f32, f32, f32) {
  if h < 180.0 { (0.0, c, x) } else { (0.0, x, c) }
}
