use iced::{
  Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Theme,
  alignment::{Horizontal, Vertical},
  border::Radius,
  mouse,
  widget::{Column, Row, Space, canvas, container, scrollable, text},
};

use super::{
  Message, State,
  budget::{self, BudgetRange, ReflectView, SpendRow, TargetTally},
};
use crate::{
  features::wallet::budget_engine::MonthFlow,
  ui::{
    components::{eyebrow::eyebrow_text, icon::Icon},
    style::{color, spacing, typography},
  },
};

const AGE_PAD: f32 = 3.0;
const CARD_RADIUS: f32 = 12.0;
const FLOW_BAR_WIDTH: f32 = 14.0;
const FLOW_HEIGHT: f32 = 168.0;
const GRID_GAP: f32 = 20.0;
const GRID_PADDING: f32 = 24.0;
const SPARK_HEIGHT: f32 = 80.0;

pub(super) fn reflect_surface(state: &State) -> Element<'_, Message> {
  let Some(view) = state.budget() else {
    return loading();
  };
  let reflect = budget::reflect(view, state.budget_history().to_vec());

  container(
    scrollable(Column::with_children(vec![stat_band(&reflect), report_grid(state, &reflect)]).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn loading<'a>() -> Element<'a, Message> {
  container(
    text(t!("wallet.budget.loading_budget"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6)
  .into()
}

fn stat_band<'a>(reflect: &ReflectView) -> Element<'a, Message> {
  let net = reflect.net();
  let net_color = if net >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  let cells = vec![
    stat_cell(
      super::i18n::tr_static("wallet.budget.stat_net_this_month"),
      signed_isk(net),
      super::i18n::tr_static("wallet.budget.unit_isk"),
      net_color,
      None,
      false,
    ),
    stat_cell(
      super::i18n::tr_static("wallet.budget.stat_assigned"),
      crate::ui::format::fmt_isk(reflect.assigned),
      super::i18n::tr_static("wallet.budget.unit_isk"),
      color::text::PRIMARY,
      None,
      true,
    ),
    stat_cell(
      super::i18n::tr_static("wallet.budget.stat_income"),
      crate::ui::format::fmt_isk(reflect.income),
      super::i18n::tr_static("wallet.budget.unit_isk"),
      color::status::ONLINE,
      None,
      true,
    ),
    stat_cell(
      super::i18n::tr_static("wallet.budget.stat_spent"),
      crate::ui::format::fmt_isk(reflect.spend),
      super::i18n::tr_static("wallet.budget.unit_isk"),
      color::status::DANGER,
      None,
      true,
    ),
    stat_cell(
      super::i18n::tr_static("wallet.budget.stat_age_of_isk"),
      format!("{}", reflect.age.round() as i64),
      super::i18n::tr_static("wallet.budget.unit_days"),
      color::text::PRIMARY,
      age_delta(reflect),
      true,
    ),
  ];

  container(Row::with_children(cells).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn signed_isk(value: f64) -> String {
  format!(
    "{}{}",
    if value >= 0.0 { "+" } else { "-" },
    crate::ui::format::fmt_isk(value.abs())
  )
}

fn age_delta(reflect: &ReflectView) -> Option<(bool, String)> {
  if reflect.prev_label.is_empty() || reflect.age_delta == 0.0 {
    return None;
  }
  let up = reflect.age_delta >= 0.0;
  let days = reflect.age_delta.abs().round() as i64;
  Some((
    up,
    t!("wallet.budget.age_delta", days => days, label => reflect.prev_label).into_owned(),
  ))
}

fn stat_cell<'a>(
  label: &'a str,
  value: String,
  unit: &'a str,
  value_color: Color,
  delta: Option<(bool, String)>,
  border: bool,
) -> Element<'a, Message> {
  let value_row = Row::with_children(vec![
    text(value)
      .font(typography::mono::MEDIUM)
      .size(22.0)
      .style(typography::colored(value_color))
      .into(),
    text(unit.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_y(Vertical::Bottom);

  let mut children: Vec<Element<'a, Message>> = vec![eyebrow_text(label, None).into(), value_row.into()];
  if let Some((up, body)) = delta {
    let arrow = if up { Icon::chevron_up() } else { Icon::chevron_down() };
    let delta_color = if up {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    children.push(
      Row::with_children(vec![
        arrow.size(typography::size::XS_PLUS).color(delta_color).render(),
        text(body)
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(delta_color))
          .into(),
      ])
      .spacing(spacing::UNIT)
      .align_y(Vertical::Center)
      .into(),
    );
  }

  let column = Column::with_children(children)
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  container(column)
    .width(Length::Fill)
    .padding(Padding {
      top: 16.0,
      right: 20.0,
      bottom: 16.0,
      left: 20.0,
    })
    .style(move |_| container::Style {
      border: Border {
        color: if border { color::rule() } else { Color::TRANSPARENT },
        width: if border { 1.0 } else { 0.0 },
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn report_grid<'a>(state: &State, reflect: &ReflectView) -> Element<'a, Message> {
  let range = state.budget_range();
  let flow_months = trailing(&reflect.history, range.months());

  let top = Row::with_children(vec![
    card(
      super::i18n::tr_static("wallet.budget.card_income_vs_spending"),
      Some(range_toggle(range)),
      flow_chart(&flow_months),
      14,
    ),
    card(
      super::i18n::tr_static("wallet.budget.card_age_of_isk"),
      None,
      age_block(reflect),
      10,
    ),
  ])
  .spacing(GRID_GAP);

  let bottom = Row::with_children(vec![
    card(
      super::i18n::tr_static("wallet.budget.card_spending_by_category"),
      Some(spend_total(reflect.spend)),
      spend_bars(&reflect.spend_rows, reflect.spend),
      14,
    ),
    card(
      super::i18n::tr_static("wallet.budget.card_target_health"),
      None,
      target_health(&reflect.tally),
      10,
    ),
  ])
  .spacing(GRID_GAP);

  Column::with_children(vec![
    top.into(),
    bottom.into(),
    Space::new().height(Length::Fixed(GRID_PADDING)).into(),
  ])
  .spacing(GRID_GAP)
  .padding(GRID_PADDING)
  .width(Length::Fill)
  .into()
}

fn trailing(history: &[MonthFlow], months: usize) -> Vec<MonthFlow> {
  let start = history.len().saturating_sub(months);
  history[start..].to_vec()
}

fn card<'a>(
  title: &'a str,
  right: Option<Element<'a, Message>>,
  body: Element<'a, Message>,
  portion: u16,
) -> Element<'a, Message> {
  let mut head: Vec<Element<'a, Message>> = vec![
    text(title.to_owned())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .width(Length::Fill)
      .into(),
  ];
  if let Some(right) = right {
    head.push(right);
  }

  let header = container(Row::with_children(head).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 18.0,
      bottom: 14.0,
      left: 18.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  container(
    Column::with_children(vec![
      header.into(),
      container(body).padding(18.0).width(Length::Fill).into(),
    ])
    .width(Length::Fill),
  )
  .width(Length::FillPortion(portion))
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: CARD_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .clip(true)
  .into()
}

fn range_toggle<'a>(active: BudgetRange) -> Element<'a, Message> {
  let divider = container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(22.0)))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into();
  let buttons = Row::with_children(vec![
    range_button(
      super::i18n::tr_static("wallet.budget.range_3m"),
      BudgetRange::ThreeMonths,
      active,
    ),
    divider,
    range_button(
      super::i18n::tr_static("wallet.budget.range_6m"),
      BudgetRange::SixMonths,
      active,
    ),
  ]);

  container(buttons)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 6.0.into(),
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
}

fn range_button<'a>(label: &'a str, range: BudgetRange, active: BudgetRange) -> Element<'a, Message> {
  let is_active = range == active;
  let text_color = if is_active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let background = if is_active {
    Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))
  } else {
    Background::Color(Color::TRANSPARENT)
  };

  iced::widget::button(
    text(label.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: 4.0,
    right: 10.0,
    bottom: 4.0,
    left: 10.0,
  })
  .on_press_maybe((!is_active).then_some(Message::BudgetRangeSelected(range)))
  .style(move |_, _| iced::widget::button::Style {
    background: Some(background),
    border: Border {
      color: color::rule(),
      width: 0.0,
      radius: Radius {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
      },
    },
    text_color,
    ..iced::widget::button::Style::default()
  })
  .into()
}

fn flow_chart<'a>(months: &[MonthFlow]) -> Element<'a, Message> {
  let max = months
    .iter()
    .map(|m| m.income.max(m.spend))
    .fold(0.0_f64, f64::max)
    .max(1.0)
    * 1.1;

  let bars: Vec<Element<'a, Message>> = months.iter().map(|month| flow_column(month, max)).collect();

  let chart = Row::with_children(bars)
    .spacing(14.0)
    .height(Length::Fixed(FLOW_HEIGHT))
    .align_y(Vertical::Bottom);

  let footer = Row::with_children(vec![
    legend(
      color::status::ONLINE,
      super::i18n::tr_static("wallet.budget.legend_income"),
    ),
    legend(
      color::status::DANGER,
      super::i18n::tr_static("wallet.budget.legend_spend"),
    ),
    Space::new().width(Length::Fill).into(),
    text(t!("wallet.budget.net_under_each_month"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(18.0)
  .align_y(Vertical::Center);

  Column::with_children(vec![
    chart.into(),
    container(crate::ui::components::rule::horizontal())
      .padding(Padding {
        top: 14.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
      })
      .into(),
    container(footer)
      .padding(Padding {
        top: 12.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn flow_column<'a>(month: &MonthFlow, max: f64) -> Element<'a, Message> {
  let net = month.income - month.spend;
  let net_color = if net >= 0.0 {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };

  let bars = Row::with_children(vec![
    flow_bar(month.income, max, color::with_alpha(color::status::ONLINE, 0.85)),
    flow_bar(month.spend, max, color::with_alpha(color::status::DANGER, 0.8)),
  ])
  .spacing(4.0)
  .height(Length::Fill)
  .align_y(Vertical::Bottom);

  Column::with_children(vec![
    container(bars).height(Length::Fill).align_y(Vertical::Bottom).into(),
    text(budget::month_short_label(&month.month))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text(format!(
      "{}{}",
      if net >= 0.0 { "+" } else { "-" },
      crate::ui::format::fmt_isk(net.abs())
    ))
    .font(typography::mono::REGULAR)
    .size(9.5)
    .style(typography::colored(net_color))
    .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_x(Horizontal::Center)
  .width(Length::FillPortion(1))
  .into()
}

fn flow_bar<'a>(value: f64, max: f64, fill: Color) -> Element<'a, Message> {
  let fraction = (value / max).clamp(0.0, 1.0) as f32;
  Column::with_children(vec![
    Space::new()
      .height(Length::FillPortion(((1.0 - fraction) * 1000.0) as u16))
      .into(),
    container(Space::new())
      .width(Length::Fixed(FLOW_BAR_WIDTH))
      .height(Length::FillPortion((fraction * 1000.0) as u16))
      .style(move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
          radius: Radius {
            top_left: 3.0,
            top_right: 3.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
          },
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
  ])
  .width(Length::Fixed(FLOW_BAR_WIDTH))
  .height(Length::Fill)
  .into()
}

fn legend<'a>(dot: Color, label: &'a str) -> Element<'a, Message> {
  Row::with_children(vec![
    container(Space::new().width(Length::Fixed(9.0)).height(Length::Fixed(9.0)))
      .style(move |_| container::Style {
        background: Some(Background::Color(dot)),
        border: Border {
          radius: 2.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(7.0)
  .align_y(Vertical::Center)
  .into()
}

fn age_block<'a>(reflect: &ReflectView) -> Element<'a, Message> {
  let cur = reflect.age.round() as i64;

  let mut head: Vec<Element<'a, Message>> = vec![
    text(format!("{cur}"))
      .font(typography::mono::MEDIUM)
      .size(30.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("wallet.budget.unit_days"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if let Some((up, body)) = age_delta(reflect) {
    let arrow = if up { Icon::chevron_up() } else { Icon::chevron_down() };
    let delta_color = if up {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    head.push(
      Row::with_children(vec![
        arrow.size(typography::size::XS_PLUS).color(delta_color).render(),
        text(body)
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(delta_color))
          .into(),
      ])
      .spacing(spacing::UNIT)
      .align_y(Vertical::Center)
      .into(),
    );
  }

  let spark: Element<'a, Message> = canvas(Sparkline {
    values: reflect.history.iter().map(|m| m.age).collect(),
  })
  .width(Length::Fill)
  .height(Length::Fixed(SPARK_HEIGHT))
  .into();

  Column::with_children(vec![
    Row::with_children(head)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Bottom)
      .into(),
    spark,
    text(t!("wallet.budget.age_of_isk_explainer"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(14.0)
  .width(Length::Fill)
  .into()
}

struct Sparkline {
  values: Vec<f64>,
}

fn sparkline_points(values: &[f64], width: f32, height: f32) -> Vec<Point> {
  let min = values.iter().copied().fold(f64::INFINITY, f64::min) - f64::from(AGE_PAD);
  let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max) + f64::from(AGE_PAD);
  let span = (max - min).max(1.0);
  let last_index = (values.len() as f32 - 1.0).max(1.0);

  values
    .iter()
    .enumerate()
    .map(|(index, &value)| {
      let x = (index as f32 / last_index) * width;
      let y = height - (((value - min) / span) as f32) * height;
      Point::new(x, y)
    })
    .collect()
}

fn polyline_path(points: &[Point]) -> canvas::Path {
  canvas::Path::new(|builder| {
    let mut points = points.iter();
    if let Some(&first) = points.next() {
      builder.move_to(first);
      for &point in points {
        builder.line_to(point);
      }
    }
  })
}

impl canvas::Program<Message> for Sparkline {
  type State = ();

  fn draw(
    &self,
    _state: &Self::State,
    renderer: &Renderer,
    _theme: &Theme,
    bounds: Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<canvas::Geometry> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    if self.values.len() < 2 {
      return vec![frame.into_geometry()];
    }

    let (width, height) = (bounds.width, bounds.height);
    let points = sparkline_points(&self.values, width, height);

    let area = canvas::Path::new(|builder| {
      builder.move_to(points[0]);
      for &point in &points[1..] {
        builder.line_to(point);
      }
      builder.line_to(Point::new(width, height));
      builder.line_to(Point::new(0.0, height));
      builder.close();
    });
    frame.fill(&area, color::with_alpha(color::accent::PLASMA, 0.18));

    frame.stroke(
      &polyline_path(&points),
      canvas::Stroke::default()
        .with_width(2.0)
        .with_color(color::accent::PLASMA),
    );

    let last = points[points.len() - 1];
    frame.fill(&canvas::Path::circle(last, 3.5), color::accent::PLASMA);

    vec![frame.into_geometry()]
  }
}

fn spend_total<'a>(spend: f64) -> Element<'a, Message> {
  text(t!("wallet.budget.spend_total", amount => crate::ui::format::fmt_isk(spend)))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn spend_bars<'a>(rows: &[SpendRow], total: f64) -> Element<'a, Message> {
  if rows.is_empty() {
    return text(t!("wallet.budget.no_spending_recorded"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into();
  }

  let max = rows.first().map_or(1.0, |row| row.spend).max(1.0);
  let bars: Vec<Element<'a, Message>> = rows.iter().map(|row| spend_row(row, max, total)).collect();

  Column::with_children(bars).spacing(13.0).width(Length::Fill).into()
}

fn spend_row<'a>(row: &SpendRow, max: f64, total: f64) -> Element<'a, Message> {
  let tone = budget::tone_color(row.tone.as_deref());
  let pct = if total > 0.0 {
    (row.spend / total * 100.0).round() as i64
  } else {
    0
  };

  let head = Row::with_children(vec![
    container(Space::new().width(Length::Fixed(10.0)).height(Length::Fixed(10.0)))
      .style(move |_| container::Style {
        background: Some(Background::Color(tone)),
        border: Border {
          radius: 5.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    text(row.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .width(Length::Fill)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(crate::ui::format::fmt_isk(row.spend))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    container(
      text(format!("{pct}%"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .width(Length::Fill)
        .align_x(Horizontal::Right)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fixed(38.0))
    .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let fraction = (row.spend / max).clamp(0.0, 1.0);
  let filled = (fraction * 1000.0) as u16;
  let empty = 1000_u16.saturating_sub(filled);
  let bar = container(
    Row::with_children(vec![
      bar_segment(filled, color::with_alpha(tone, 0.7)),
      bar_segment(empty, Color::TRANSPARENT),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fixed(5.0))
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
    border: Border {
      radius: 3.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  Column::with_children(vec![
    head.into(),
    container(bar)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 28.0,
      })
      .width(Length::Fill)
      .into(),
  ])
  .spacing(5.0)
  .width(Length::Fill)
  .into()
}

fn bar_segment<'a>(portion: u16, fill: Color) -> Element<'a, Message> {
  if portion == 0 {
    return Space::new().width(Length::FillPortion(0)).into();
  }
  container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(Length::FillPortion(portion))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    })
    .into()
}

fn target_health<'a>(tally: &TargetTally) -> Element<'a, Message> {
  let segments = container(
    Row::with_children(vec![
      health_segment(tally.met as u16, color::status::ONLINE),
      health_segment(tally.under as u16, color::status::WARNING),
      health_segment(tally.over as u16, color::status::DANGER),
    ])
    .spacing(2.0)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fixed(8.0))
  .clip(true)
  .style(|_| container::Style {
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let tallies = Row::with_children(vec![
    tally_cell(
      tally.met,
      super::i18n::tr_static("wallet.budget.tally_funded"),
      color::status::ONLINE,
    ),
    tally_cell(
      tally.under,
      super::i18n::tr_static("wallet.budget.tally_underfunded"),
      color::status::WARNING,
    ),
    tally_cell(
      tally.over,
      super::i18n::tr_static("wallet.budget.tally_overspent"),
      color::status::DANGER,
    ),
  ])
  .spacing(18.0);

  let mut children: Vec<Element<'a, Message>> = vec![
    container(segments)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: 14.0,
        left: 0.0,
      })
      .into(),
    container(tallies)
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: 18.0,
        left: 0.0,
      })
      .into(),
  ];

  if !tally.attention.is_empty() {
    children.push(attention_list(tally));
  }

  Column::with_children(children).width(Length::Fill).into()
}

fn health_segment<'a>(count: u16, fill: Color) -> Element<'a, Message> {
  if count == 0 {
    return Space::new().width(Length::FillPortion(0)).into();
  }
  container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(Length::FillPortion(count))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    })
    .into()
}

fn tally_cell<'a>(count: usize, label: &'a str, value_color: Color) -> Element<'a, Message> {
  Column::with_children(vec![
    text(format!("{count}"))
      .font(typography::mono::MEDIUM)
      .size(22.0)
      .style(typography::colored(value_color))
      .into(),
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .into()
}

fn attention_list<'a>(tally: &TargetTally) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> =
    vec![eyebrow_text(super::i18n::tr_static("wallet.budget.needs_attention"), None).into()];
  for alert in tally.attention.iter().take(5) {
    let tone = if alert.over {
      color::status::DANGER
    } else {
      color::status::WARNING
    };
    let figure = if alert.over {
      crate::ui::format::fmt_isk(alert.amount)
    } else {
      t!("wallet.budget.attention_short", amount => crate::ui::format::fmt_isk(alert.amount)).into_owned()
    };
    rows.push(
      Row::with_children(vec![
        container(Space::new().width(Length::Fixed(6.0)).height(Length::Fixed(6.0)))
          .style(move |_| container::Style {
            background: Some(Background::Color(tone)),
            border: Border {
              radius: 3.0.into(),
              ..Border::default()
            },
            ..container::Style::default()
          })
          .into(),
        text(alert.name.clone())
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .width(Length::Fill)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
        text(figure)
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(tone))
          .into(),
      ])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center)
      .into(),
    );
  }

  container(Column::with_children(rows).spacing(10.0).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 0.0,
      bottom: 0.0,
      left: 0.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::{super::Tab, *};

  fn flow(label_month: &str, income: f64, spend: f64, age: f64) -> MonthFlow {
    MonthFlow {
      age,
      assigned: 0.0,
      income,
      month: label_month.to_owned(),
      spend,
    }
  }

  fn state_with_history(history: Vec<MonthFlow>) -> State {
    let mut state = State::new(crate::config::FeatureFlags::default());
    state.tab = Tab::Budget;
    state.budget_mode = budget::Mode::Reflect;
    state.budget = Some(budget::BudgetView::default());
    state.budget_history = history;
    state
  }

  mod reflect_surface {
    use super::*;

    #[test]
    fn it_renders_with_a_full_history() {
      let history = vec![
        flow("2026-01", 18.0, 14.0, 41.0),
        flow("2026-02", 16.0, 15.0, 43.0),
        flow("2026-03", 22.0, 13.0, 44.0),
        flow("2026-04", 15.0, 17.0, 46.0),
        flow("2026-05", 24.0, 16.0, 45.0),
        flow("2026-06", 21.0, 12.0, 47.0),
      ];
      let state = state_with_history(history);

      let _el: Element<'_, Message> = reflect_surface(&state);
    }

    #[test]
    fn it_renders_with_no_history() {
      let state = state_with_history(Vec::new());

      let _el: Element<'_, Message> = reflect_surface(&state);
    }

    #[test]
    fn it_renders_loading_without_a_view() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Budget;
      state.budget_mode = budget::Mode::Reflect;

      let _el: Element<'_, Message> = reflect_surface(&state);
    }
  }

  mod sparkline_geometry {
    use super::*;

    #[test]
    fn it_spaces_x_monotonically_from_zero_to_width() {
      let points = sparkline_points(&[10.0, 20.0, 30.0], 100.0, 50.0);

      assert_eq!(points.len(), 3);
      assert!((points[0].x - 0.0).abs() < 1e-4);
      assert!((points[1].x - 50.0).abs() < 1e-4);
      assert!((points[2].x - 100.0).abs() < 1e-4);
      assert!(points[0].x < points[1].x && points[1].x < points[2].x);
    }

    #[test]
    fn it_inverts_y_so_larger_values_sit_higher() {
      let points = sparkline_points(&[10.0, 30.0], 100.0, 50.0);

      assert!(points[1].y < points[0].y);
    }

    #[test]
    fn it_pads_the_range_so_extremes_never_touch_the_edges() {
      let height = 50.0;
      let points = sparkline_points(&[10.0, 30.0], 100.0, height);
      let expected_low = height - (AGE_PAD / 26.0) * height;
      let expected_high = height - ((20.0 + AGE_PAD) / 26.0) * height;

      assert!((points[0].y - expected_low).abs() < 1e-3);
      assert!((points[1].y - expected_high).abs() < 1e-3);
      assert!(points[0].y < height && points[1].y > 0.0);
    }

    #[test]
    fn it_avoids_division_by_zero_for_a_single_value() {
      let points = sparkline_points(&[42.0], 100.0, 50.0);

      assert_eq!(points.len(), 1);
      assert!(points[0].x.is_finite() && points[0].y.is_finite());
    }

    #[test]
    fn polyline_path_handles_empty_and_populated_inputs() {
      let _empty = polyline_path(&[]);
      let _line = polyline_path(&[Point::new(0.0, 0.0), Point::new(1.0, 1.0)]);
    }
  }
}
