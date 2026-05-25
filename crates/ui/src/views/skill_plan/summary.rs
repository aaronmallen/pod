//! Plan summary panel — right column of the skill plan editor window.
//!
//! Renders plan totals, attribute optimisation controls, implant
//! suggestions, time-by-group and time-by-pair bar charts.

pub mod attr_optimization_section;
pub mod implant_suggestions_section;
pub mod plan_totals_section;

use attr_optimization_section::AttrOptimizationSection;
use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, scrollable, text},
};
use implant_suggestions_section::ImplantSuggestionsSection;
use plan_totals_section::PlanTotalsSection;

use super::Message;
use crate::{
  components::{self, section_label},
  plan_math::{BaseAttrs, ComputedPlan, EffectiveAttrs, ImplantBonus, ImplantSaving, ImplantSet, RemapResult},
  style::{
    color, spacing,
    typography::{body, mono},
  },
};

/// Group palette — cycled in order for the time-by-group bar chart.
const GROUP_PALETTE: [Color; 7] = [
  color::accent::PLASMA,
  color::accent::GOLD,
  color::chart::P3,
  color::chart::P4,
  color::chart::P5,
  color::chart::P6,
  color::chart::P7,
];

/// Format seconds as "14d 3h 22m" (always shows at least minutes).
pub fn fmt_time_long(sec: f64) -> String {
  let s = sec.max(0.0) as u64;
  let d = s / 86_400;
  let h = (s % 86_400) / 3_600;
  let m = (s % 3_600) / 60;
  if d > 0 {
    format!("{d}d {h}h {m}m")
  } else if h > 0 {
    format!("{h}h {m}m")
  } else {
    format!("{m}m")
  }
}

/// Format seconds as "14d 3h" (drops minutes).
pub fn fmt_time_short(sec: f64) -> String {
  let s = sec.max(0.0) as u64;
  let d = s / 86_400;
  let h = (s % 86_400) / 3_600;
  if d > 0 {
    format!("{d}d {h}h")
  } else if h > 0 {
    format!("{h}h")
  } else {
    let m = (s % 3_600) / 60;
    format!("{m}m")
  }
}

/// Format SP as "2.4M SP", "450k SP", or "250 SP".
pub fn fmt_sp(sp: u64) -> String {
  if sp >= 1_000_000 {
    format!("{:.1}M SP", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.0}k SP", sp as f64 / 1_000.0)
  } else {
    format!("{sp} SP")
  }
}

/// Builder for the plan summary panel.
pub struct Component<'a> {
  base_attrs: &'a BaseAttrs,
  bonus_remaps: u32,
  clone_data_missing: bool,
  computed: &'a ComputedPlan,
  effective_attrs: &'a EffectiveAttrs,
  implant: &'a ImplantBonus,
  implant_savings: &'a [ImplantSaving],
  implant_set: ImplantSet,
  optimizer_result: Option<&'a RemapResult>,
  optimizer_running: bool,
  remap_available: bool,
  remap_cooldown_days: i32,
  show_implant_suggestions: bool,
  show_remap: bool,
}

impl<'a> Component<'a> {
  /// Create a new plan summary `Component`.
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    computed: &'a ComputedPlan,
    base_attrs: &'a BaseAttrs,
    effective_attrs: &'a EffectiveAttrs,
    implant: &'a ImplantBonus,
    implant_set: ImplantSet,
    optimizer_result: Option<&'a RemapResult>,
    optimizer_running: bool,
    show_remap: bool,
    show_implant_suggestions: bool,
    implant_savings: &'a [ImplantSaving],
    remap_cooldown_days: i32,
    remap_available: bool,
    bonus_remaps: u32,
  ) -> Self {
    Self {
      base_attrs,
      bonus_remaps,
      clone_data_missing: false,
      computed,
      effective_attrs,
      implant,
      implant_savings,
      implant_set,
      optimizer_result,
      optimizer_running,
      remap_available,
      remap_cooldown_days,
      show_implant_suggestions,
      show_remap,
    }
  }

  /// Set whether the character's active-clone data is missing from the DB.
  /// When true and `ImplantSet::Current` is selected, a warning label is
  /// shown next to the implant-set picker.
  pub fn clone_data_missing(mut self, missing: bool) -> Self {
    self.clone_data_missing = missing;
    self
  }

  /// Render the summary panel into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let sep = || components::Separator::horizontal().render::<Message>();

    let steps = self.computed.items.iter().filter(|i| !i.skipped).count();

    let mut sections: Vec<Element<'_, Message>> = vec![
      PlanTotalsSection::new(self.computed.total_sec, self.computed.total_sp, steps).render(),
      sep(),
      AttrOptimizationSection::new(
        self.base_attrs,
        self.effective_attrs,
        self.implant,
        self.implant_set,
        self.optimizer_result,
        self.optimizer_running,
        self.show_remap,
        self.remap_cooldown_days,
        self.remap_available,
        self.bonus_remaps,
        self.clone_data_missing,
      )
      .render(),
      sep(),
      ImplantSuggestionsSection::new(self.show_implant_suggestions, self.implant_savings).render(),
    ];

    if !self.computed.group_sec.is_empty() {
      sections.push(sep());
      sections.push(time_by_group_section(&self.computed.group_sec));
    }

    if !self.computed.pair_sec.is_empty() {
      sections.push(sep());
      sections.push(time_by_pair_section(&self.computed.pair_sec));
    }

    sections.push(Space::new().height(spacing::SPACE_4).into());

    scrollable(column(sections).width(Length::Fill))
      .height(Length::Fill)
      .into()
  }
}

fn time_by_group_section(group_sec: &std::collections::HashMap<String, f64>) -> Element<'static, Message> {
  let mut entries: Vec<(&String, &f64)> = group_sec.iter().collect();
  entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

  let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);

  let rows: Vec<Element<'_, Message>> = entries
    .iter()
    .enumerate()
    .flat_map(|(i, &(name, sec))| {
      let sec = *sec;
      let color = GROUP_PALETTE[i % GROUP_PALETTE.len()];
      let time_str = fmt_time_short(sec);
      let fraction = if max_sec > 0.0 { (sec / max_sec) as f32 } else { 0.0 };
      let name_str = name.as_str().to_string();
      [
        bar_chart_row(name_str, time_str, fraction, color),
        Space::new().height(6.0).into(),
      ]
    })
    .collect();

  time_chart_section("TIME BY SKILL GROUP", rows)
}

fn time_by_pair_section(pair_sec: &std::collections::HashMap<String, f64>) -> Element<'static, Message> {
  let mut entries: Vec<(&String, &f64)> = pair_sec.iter().collect();
  entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

  let max_sec = entries.first().map(|&(_, s)| *s).unwrap_or(1.0);
  let bar_color = color::accent::PLASMA;

  let rows: Vec<Element<'_, Message>> = entries
    .iter()
    .flat_map(|&(name, sec)| {
      let sec = *sec;
      let time_str = fmt_time_short(sec);
      let fraction = if max_sec > 0.0 { (sec / max_sec) as f32 } else { 0.0 };
      let name_str = name.as_str().to_string();
      [
        bar_chart_row(name_str, time_str, fraction, bar_color),
        Space::new().height(6.0).into(),
      ]
    })
    .collect();

  time_chart_section("TIME BY ATTRIBUTE PAIR", rows)
}

fn time_chart_section(title: &'static str, rows: Vec<Element<'static, Message>>) -> Element<'static, Message> {
  container(
    column([
      container(section_label(title))
        .padding(Padding {
          top: 0.0,
          bottom: spacing::SPACE_3,
          left: 0.0,
          right: 0.0,
        })
        .width(Length::Fill)
        .into(),
      column(rows).width(Length::Fill).into(),
    ])
    .width(Length::Fill),
  )
  .padding(Padding {
    top: spacing::SPACE_3,
    bottom: spacing::SPACE_4,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .width(Length::Fill)
  .into()
}

fn bar_chart_row(label: String, time_str: String, fraction: f32, bar_color: Color) -> Element<'static, Message> {
  let filled = (fraction * 1000.0) as u16;
  let rest = 1000u16.saturating_sub(filled);

  column([
    bar_label_row(label, time_str),
    Space::new().height(4.0).into(),
    bar_track(filled, rest, bar_color),
  ])
  .width(Length::Fill)
  .into()
}

fn bar_label_row(label: String, time_str: String) -> Element<'static, Message> {
  row([
    text(label)
      .font(body::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(time_str)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn bar_track(filled: u16, rest: u16, bar_color: Color) -> Element<'static, Message> {
  container(
    row([
      container(Space::new().width(Length::Fill).height(4.0))
        .width(Length::FillPortion(filled))
        .height(4.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(bar_color)),
          border: Border {
            radius: 2.0.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
      if rest > 0 {
        Space::new().width(Length::FillPortion(rest)).height(4.0).into()
      } else {
        Space::new().width(0.0).into()
      },
    ])
    .height(4.0)
    .spacing(0.0),
  )
  .height(4.0)
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::border::SUBTLE)),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}
