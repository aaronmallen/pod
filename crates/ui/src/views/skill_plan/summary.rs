//! Plan summary panel — right column of the skill plan editor window.
//!
//! Renders plan totals, attribute optimisation controls, implant
//! suggestions, time-by-group and time-by-pair bar charts.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, scrollable, text},
};

use super::Message;
use crate::{
  components::{self, section_label},
  plan_math::{BaseAttrs, ComputedPlan, EffectiveAttrs, ImplantBonus, ImplantSaving, ImplantSet, RemapResult},
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::skills::skill_data::AttrKey,
};

/// Group palette — cycled in order for the time-by-group bar chart.
const GROUP_PALETTE: [Color; 7] = [
  Color {
    r: 0.247,
    g: 0.722,
    b: 0.859,
    a: 1.0,
  },
  Color {
    r: 0.851,
    g: 0.698,
    b: 0.322,
    a: 1.0,
  },
  Color {
    r: 0.843,
    g: 0.459,
    b: 0.349,
    a: 1.0,
  },
  Color {
    r: 0.498,
    g: 0.710,
    b: 0.353,
    a: 1.0,
  },
  Color {
    r: 0.655,
    g: 0.498,
    b: 0.847,
    a: 1.0,
  },
  Color {
    r: 0.353,
    g: 0.722,
    b: 0.627,
    a: 1.0,
  },
  Color {
    r: 0.847,
    g: 0.400,
    b: 0.431,
    a: 1.0,
  },
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

fn completion_date_string(total_sec: f64) -> String {
  if total_sec <= 0.0 {
    return "\u{2014}".to_string();
  }
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let ts = now + total_sec as u64;
  let hh = (ts % 86400) / 3600;
  let mm = (ts % 3600) / 60;
  let days = ts / 86400;
  let (_, month, day) = days_to_utc_date(days);
  const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  ];
  format!("{} {} \u{00b7} {:02}:{:02}", day, MONTHS[month as usize - 1], hh, mm)
}

fn days_to_utc_date(days: u64) -> (u32, u8, u8) {
  let z = days as i64 + 719468;
  let era = z / 146097;
  let doe = (z - era * 146097) as u64;
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
  let y = yoe as i64 + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = doy - (153 * mp + 2) / 5 + 1;
  let m = if mp < 10 { mp + 3 } else { mp - 9 };
  let y = if m <= 2 { y + 1 } else { y };
  (y as u32, m as u8, d as u8)
}

fn attr_base(attrs: &BaseAttrs, key: AttrKey) -> i32 {
  match key {
    AttrKey::Perception => attrs.perception,
    AttrKey::Memory => attrs.memory,
    AttrKey::Willpower => attrs.willpower,
    AttrKey::Intelligence => attrs.intelligence,
    AttrKey::Charisma => attrs.charisma,
  }
}

fn attr_implant(implant: &ImplantBonus, key: AttrKey) -> i32 {
  match key {
    AttrKey::Perception => implant.perception,
    AttrKey::Memory => implant.memory,
    AttrKey::Willpower => implant.willpower,
    AttrKey::Intelligence => implant.intelligence,
    AttrKey::Charisma => implant.charisma,
  }
}

/// Builder for the plan summary panel.
pub struct Component<'a> {
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
  clone_data_missing: bool,
}

impl<'a> Component<'a> {
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
      computed,
      base_attrs,
      effective_attrs,
      implant,
      implant_set,
      optimizer_result,
      optimizer_running,
      show_remap,
      show_implant_suggestions,
      implant_savings,
      remap_cooldown_days,
      remap_available,
      bonus_remaps,
      clone_data_missing: false,
    }
  }

  /// Set whether the character's active-clone data is missing from the DB.
  /// When true and `ImplantSet::Current` is selected, a warning label is
  /// shown next to the implant-set picker.
  pub fn clone_data_missing(mut self, missing: bool) -> Self {
    self.clone_data_missing = missing;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let sep = || components::Separator::horizontal().render::<Message>();

    let steps = self.computed.items.iter().filter(|i| !i.skipped).count();

    let mut sections: Vec<Element<'_, Message>> = Vec::new();

    sections.push(plan_totals_section(
      self.computed.total_sec,
      self.computed.total_sp,
      steps,
    ));

    sections.push(sep());
    sections.push(attr_optimization_section(
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
    ));

    sections.push(sep());
    sections.push(implant_suggestions_section(
      self.show_implant_suggestions,
      self.implant_savings,
    ));

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

fn plan_totals_section(total_sec: f64, total_sp: u64, steps: usize) -> Element<'static, Message> {
  let time_str = fmt_time_short(total_sec);
  let sp_str = fmt_sp(total_sp);
  let steps_str = format!("{steps} steps");
  let completion_str = format!("Completes {}", completion_date_string(total_sec));

  container(totals_column(time_str, sp_str, steps_str, completion_str))
    .padding(Padding {
      top: spacing::SPACE_4,
      bottom: spacing::SPACE_4,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn totals_column(
  time_str: String,
  sp_str: String,
  steps_str: String,
  completion_str: String,
) -> iced::widget::Column<'static, Message> {
  column([
    text(time_str)
      .font(mono::MEDIUM)
      .size(28.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(sp_str)
      .font(mono::MEDIUM)
      .size(16.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text(steps_str)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(completion_str)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .width(Length::Fill)
}

#[allow(clippy::too_many_arguments)]
fn attr_optimization_section<'a>(
  base_attrs: &'a BaseAttrs,
  effective_attrs: &'a EffectiveAttrs,
  implant: &'a ImplantBonus,
  implant_set: ImplantSet,
  optimizer_result: Option<&'a RemapResult>,
  optimizer_running: bool,
  show_remap: bool,
  remap_cooldown_days: i32,
  remap_available: bool,
  _bonus_remaps: u32,
  clone_data_missing: bool,
) -> Element<'a, Message> {
  let mut items: Vec<Element<'_, Message>> = Vec::new();

  items.push(optimization_header_row());
  items.push(Space::new().height(spacing::SPACE_3).into());
  items.push(implant_set_picker(implant_set, clone_data_missing));
  items.push(Space::new().height(spacing::SPACE_3).into());

  if !show_remap {
    items.push(single_attr_column(base_attrs, effective_attrs, implant));
  } else if optimizer_running {
    items.push(computing_text());
  } else if let Some(result) = optimizer_result {
    items.push(dual_attr_columns(base_attrs, effective_attrs, implant, result));
    items.push(Space::new().height(spacing::SPACE_3).into());
    items.push(savings_callout(result, effective_attrs));
    items.push(Space::new().height(spacing::SPACE_3).into());
    items.push(remap_status_row(remap_cooldown_days, remap_available));
  } else {
    items.push(single_attr_column(base_attrs, effective_attrs, implant));
  }

  container(column(items).width(Length::Fill))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_4,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn optimization_header_row() -> Element<'static, Message> {
  row([
    text("ATTRIBUTE OPTIMIZATION")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .width(Length::Fill)
      .into(),
    ghost_button("Optimize", Message::OptimizerRequested),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn ghost_button(label: &'static str, msg: Message) -> Element<'static, Message> {
  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(msg)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_SUBTLE,
      _ => Color::TRANSPARENT,
    })),
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_MUTED,
        _ => color::border::SUBTLE,
      },
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .into()
}

fn implant_set_picker(current: ImplantSet, clone_data_missing: bool) -> Element<'static, Message> {
  let sets = [
    (ImplantSet::None, "None"),
    (ImplantSet::Plus3, "+3"),
    (ImplantSet::Plus4, "+4"),
    (ImplantSet::Plus5, "+5"),
    (ImplantSet::Current, "Current"),
  ];

  let buttons: Vec<Element<'_, Message>> = sets
    .iter()
    .map(|(set, label)| implant_set_button(*set, label, current == *set))
    .collect();

  let picker_row = row(buttons).spacing(4.0);

  if current == ImplantSet::Current && clone_data_missing {
    column([
      picker_row.into(),
      Space::new().height(4.0).into(),
      text("(clone not synced)")
        .font(crate::style::typography::mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    ])
    .width(Length::Fill)
    .into()
  } else {
    picker_row.into()
  }
}

fn implant_set_button(set: ImplantSet, label: &'static str, active: bool) -> Element<'static, Message> {
  let bg = implant_btn_bg(active);
  let border_color = if active {
    color::accent::PLASMA_MUTED
  } else {
    color::border::SUBTLE
  };
  let text_color = if active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };

  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        }),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::ImplantSetChanged(set))
  .style(move |_, status| button::Style {
    background: Some(Background::Color(if active {
      bg
    } else {
      implant_btn_hover_bg(status)
    })),
    border: Border {
      color: border_color,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn implant_btn_bg(active: bool) -> Color {
  if active {
    color::accent::PLASMA_SUBTLE
  } else {
    Color::TRANSPARENT
  }
}

fn implant_btn_hover_bg(status: button::Status) -> Color {
  match status {
    button::Status::Hovered => Color::from_rgba(
      color::accent::PLASMA.r,
      color::accent::PLASMA.g,
      color::accent::PLASMA.b,
      0.05,
    ),
    _ => Color::TRANSPARENT,
  }
}

fn single_attr_column<'a>(
  base: &'a BaseAttrs,
  effective: &'a EffectiveAttrs,
  implant: &'a ImplantBonus,
) -> Element<'a, Message> {
  let rows: Vec<Element<'_, Message>> = AttrKey::ALL
    .iter()
    .map(|&key| {
      let base_val = attr_base(base, key);
      let imp_val = attr_implant(implant, key);
      attr_value_row(key, base_val, imp_val, false)
    })
    .collect();

  let _ = effective;
  container(column(rows).spacing(2.0).width(Length::Fill))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 12.0,
      right: 12.0,
    })
    .width(Length::Fill)
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

fn attr_value_row(key: AttrKey, base_val: i32, implant_val: i32, highlight: bool) -> Element<'static, Message> {
  row([
    text(key.short())
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .width(Length::Fixed(30.0))
      .into(),
    text(key.label())
      .font(body::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .width(Length::Fill)
      .into(),
    text(base_val.to_string())
      .font(mono::MEDIUM)
      .size(12.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if highlight {
          color::accent::PLASMA
        } else {
          color::text::PRIMARY
        }),
      })
      .into(),
    if implant_val > 0 {
      text(format!("+{implant_val}"))
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SUCCESS),
        })
        .width(Length::Fixed(28.0))
        .into()
    } else {
      Space::new().width(Length::Fixed(28.0)).into()
    },
  ])
  .align_y(Vertical::Center)
  .spacing(4.0)
  .into()
}

fn computing_text() -> Element<'static, Message> {
  container(
    text("Computing optimal remap\u{2026}")
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding::new(12.0))
  .width(Length::Fill)
  .into()
}

fn dual_attr_columns<'a>(
  base: &'a BaseAttrs,
  effective: &'a EffectiveAttrs,
  implant: &'a ImplantBonus,
  result: &'a RemapResult,
) -> Element<'a, Message> {
  let _ = effective;
  let current_col = attr_column_panel("CURRENT", base, implant, false);
  let proposed_col = attr_column_panel("PROPOSED", &result.base, implant, true);

  row([current_col, Space::new().width(8.0).into(), proposed_col])
    .width(Length::Fill)
    .into()
}

fn attr_column_panel<'a>(
  title: &'static str,
  base: &'a BaseAttrs,
  implant: &'a ImplantBonus,
  highlight: bool,
) -> Element<'a, Message> {
  let header = attr_panel_header(title, highlight);
  let rows: Vec<Element<'_, Message>> = AttrKey::ALL
    .iter()
    .map(|&key| attr_value_row(key, attr_base(base, key), attr_implant(implant, key), highlight))
    .collect();

  let items: Vec<Element<'_, Message>> = std::iter::once(header)
    .chain(std::iter::once(Space::new().height(6.0).into()))
    .chain(rows)
    .collect();

  attr_panel_container(highlight, items)
}

fn attr_panel_container<'a>(highlight: bool, items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  let bg = if highlight {
    color::accent::PLASMA_SUBTLE
  } else {
    color::surface::SUNKEN
  };
  let border_color = if highlight {
    color::accent::PLASMA_MUTED
  } else {
    color::border::SUBTLE
  };

  container(column(items).spacing(2.0).width(Length::Fill))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 10.0,
      right: 10.0,
    })
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: border_color,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn attr_panel_header(title: &'static str, highlight: bool) -> Element<'static, Message> {
  text(title)
    .font(mono::REGULAR)
    .size(9.0)
    .style(move |_| iced::widget::text::Style {
      color: Some(if highlight {
        color::accent::PLASMA
      } else {
        color::text::TERTIARY
      }),
    })
    .into()
}

fn savings_callout(result: &RemapResult, effective: &EffectiveAttrs) -> Element<'static, Message> {
  let _ = effective;
  let (bg_color, border_color, label_color, msg_str) = savings_callout_style(result);

  container(
    text(msg_str)
      .font(mono::MEDIUM)
      .size(13.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(label_color),
      }),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 14.0,
    right: 14.0,
  })
  .width(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg_color)),
    border: Border {
      color: border_color,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn savings_callout_style(result: &RemapResult) -> (Color, Color, Color, String) {
  if result.is_current {
    (
      Color::from_rgba(
        color::text::SUCCESS.r,
        color::text::SUCCESS.g,
        color::text::SUCCESS.b,
        0.08,
      ),
      Color::from_rgba(
        color::text::SUCCESS.r,
        color::text::SUCCESS.g,
        color::text::SUCCESS.b,
        0.30,
      ),
      color::text::SUCCESS,
      "Already optimal".to_string(),
    )
  } else {
    let saved = (result.current_sec - result.total_sec).max(0.0);
    (
      color::accent::PLASMA_SUBTLE,
      color::accent::PLASMA_MUTED,
      color::accent::PLASMA,
      format!("\u{2212}{}", fmt_time_long(saved)),
    )
  }
}

fn remap_status_row(cooldown_days: i32, remap_available: bool) -> Element<'static, Message> {
  let (dot_color, status_text) = remap_status_info(cooldown_days, remap_available);

  row([
    container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(6.0).into(),
    text(status_text)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(dot_color),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn remap_status_info(cooldown_days: i32, remap_available: bool) -> (Color, String) {
  if cooldown_days > 0 {
    (
      color::status::CAUTION,
      format!("Remap on cooldown for {cooldown_days} days"),
    )
  } else if remap_available {
    (color::status::ONLINE, "Remap available now".to_string())
  } else {
    (color::text::TERTIARY, "No remap available".to_string())
  }
}

fn implant_suggestions_section<'a>(show: bool, savings: &'a [ImplantSaving]) -> Element<'a, Message> {
  let mut items: Vec<Element<'_, Message>> = Vec::new();

  items.push(implant_suggestions_header_row());
  items.push(Space::new().height(spacing::SPACE_3).into());

  if !show {
    items.push(
      text("See which implant upgrades save the most plan time.")
        .font(body::REGULAR)
        .size(11.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    );
  } else if savings.is_empty() {
    items.push(
      text("No savings \u{2014} implants already maxed for this plan\u{2019}s mix.")
        .font(body::REGULAR)
        .size(11.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    );
  } else {
    for (i, saving) in savings.iter().enumerate() {
      if i > 0 {
        items.push(Space::new().height(4.0).into());
      }
      items.push(implant_saving_row(saving, i == 0));
    }
  }

  container(column(items).width(Length::Fill))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_4,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn implant_suggestions_header_row() -> Element<'static, Message> {
  row([
    text("IMPLANT SUGGESTIONS")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .width(Length::Fill)
      .into(),
    ghost_button("Suggest", Message::ImplantSuggestionsToggled),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn implant_saving_row(saving: &ImplantSaving, is_first: bool) -> Element<'static, Message> {
  let (badge_bg, badge_border, badge_text_color) = implant_saving_badge_style(is_first);
  let attr_name = saving.attr.label();
  let saved_str = fmt_time_short(saving.saved_sec);

  row([
    implant_saving_badge(badge_bg, badge_border, badge_text_color),
    Space::new().width(8.0).into(),
    implant_saving_label(attr_name, saved_str),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn implant_saving_badge_style(is_first: bool) -> (Color, Color, Color) {
  let badge_bg = if is_first {
    color::accent::PLASMA_SUBTLE
  } else {
    Color::from_rgba(
      color::text::SECONDARY.r,
      color::text::SECONDARY.g,
      color::text::SECONDARY.b,
      0.08,
    )
  };
  let badge_border = if is_first {
    color::accent::PLASMA_MUTED
  } else {
    color::border::SUBTLE
  };
  let badge_text_color = if is_first {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  (badge_bg, badge_border, badge_text_color)
}

fn implant_saving_badge(badge_bg: Color, badge_border: Color, badge_text_color: Color) -> Element<'static, Message> {
  container(
    text("+1")
      .font(mono::MEDIUM)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(badge_text_color),
      }),
  )
  .width(28.0)
  .height(20.0)
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(badge_bg)),
    border: Border {
      color: badge_border,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn implant_saving_label(attr_name: &'static str, saved_str: String) -> Element<'static, Message> {
  column([
    text(attr_name)
      .font(body::MEDIUM)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("saves {saved_str}"))
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .into()
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
