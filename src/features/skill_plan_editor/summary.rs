use std::collections::HashMap;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{Space, column, container, scrollable},
};

use super::Message;
pub(super) use crate::ui::format::fmt_sp_labeled as fmt_sp;
use crate::{
  features::skills::{
    optimizer::{Attributes, Recommendation},
    plan_math::injectors_for_plan,
  },
  ui::{
    components::{eyebrow::eyebrow, rule},
    style::{color, spacing},
  },
};

pub(super) mod attr_optimization_section;
pub(super) mod bar_chart;
pub(super) mod implant_effect_section;
pub(super) mod injector_section;
pub(super) mod plan_totals_section;
pub(super) mod time_by_group_section;
pub(super) mod time_by_pair_section;

const GROUP_PALETTE_ALPHA: [f32; 5] = [1.0, 0.75, 0.55, 0.40, 0.28];

#[derive(Clone, Copy, Debug)]
pub(super) struct ImplantEffect {
  pub bonus: Attributes,
  pub with_sec: f64,
  pub without_sec: f64,
}

#[derive(Clone, Debug)]
pub(super) struct SummaryData {
  pub base_attrs: Attributes,
  pub character_total_sp: u64,
  pub current_base_sec: f64,
  #[allow(dead_code)]
  pub current_sec: f64,
  pub group_sec: HashMap<String, f64>,
  pub implant_effect: ImplantEffect,
  pub pair_sec: HashMap<String, f64>,
  pub recommendation: Recommendation,
  pub remap_availability: u32,
  pub remap_reason: String,
  pub steps: usize,
  pub total_sec: f64,
  pub total_sp: u64,
}

impl Default for SummaryData {
  fn default() -> Self {
    SummaryData {
      base_attrs: Attributes::default(),
      character_total_sp: 0,
      current_base_sec: 0.0,
      current_sec: 0.0,
      group_sec: HashMap::new(),
      implant_effect: ImplantEffect {
        bonus: Attributes::default(),
        with_sec: 0.0,
        without_sec: 0.0,
      },
      pair_sec: HashMap::new(),
      recommendation: Recommendation {
        base: Attributes::default(),
        current_out_of_spec: false,
        is_current: true,
        total_sec: 0.0,
      },
      remap_availability: 0,
      remap_reason: String::new(),
      steps: 0,
      total_sec: 0.0,
      total_sp: 0,
    }
  }
}

pub(super) fn group_palette() -> [Color; 5] {
  GROUP_PALETTE_ALPHA.map(|alpha| color::with_alpha(color::accent::PLASMA, alpha))
}

fn clamp_secs(sec: f64) -> u64 {
  if !sec.is_finite() || sec <= 0.0 {
    0
  } else {
    sec.min(u64::MAX as f64) as u64
  }
}

pub(super) fn fmt_time_long(sec: f64) -> String {
  let s = clamp_secs(sec);
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

pub(super) fn fmt_time_short(sec: f64) -> String {
  let s = clamp_secs(sec);
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

pub(super) fn section_label<'a>(title: &'a str) -> Element<'a, Message> {
  eyebrow(title, None)
}

pub(super) fn summary(data: SummaryData, now: DateTime<Utc>) -> Element<'static, Message> {
  let mut sections: Vec<Element<'static, Message>> = vec![
    plan_totals_section::plan_totals_section(data.total_sec, data.total_sp, data.steps, now),
    rule::horizontal(),
    attr_optimization_section::attr_optimization_section(
      data.base_attrs,
      data.current_base_sec,
      &data.recommendation,
      data.remap_availability,
      &data.remap_reason,
    ),
  ];

  if data.total_sp > 0 {
    let estimate = injectors_for_plan(data.total_sp, data.character_total_sp);
    sections.push(rule::horizontal());
    sections.push(injector_section::injector_section(estimate, data.total_sp));
  }

  if implant_effect_section::has_implants(&data.implant_effect) {
    sections.push(rule::horizontal());
    sections.push(implant_effect_section::implant_effect_section(&data.implant_effect));
  }

  if !data.group_sec.is_empty() {
    sections.push(rule::horizontal());
    sections.push(time_by_group_section::time_by_group_section(&data.group_sec));
  }
  if !data.pair_sec.is_empty() {
    sections.push(rule::horizontal());
    sections.push(time_by_pair_section::time_by_pair_section(&data.pair_sec));
  }
  sections.push(Space::new().height(spacing::SPACE_6).into());

  let body = scrollable(column(sections).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fill);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    })
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: 0.0,
      right: spacing::SPACE_2,
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod clamp_secs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collapses_non_finite_and_negative_input_to_zero() {
      assert_eq!(clamp_secs(f64::NAN), 0);
      assert_eq!(clamp_secs(f64::INFINITY), 0);
      assert_eq!(clamp_secs(f64::NEG_INFINITY), 0);
      assert_eq!(clamp_secs(-5.0), 0);
    }

    #[test]
    fn it_passes_through_an_in_range_value() {
      assert_eq!(clamp_secs(90.0), 90);
    }
  }

  mod fmt_time_long {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_guards_out_of_range_and_negative_input() {
      assert_eq!(fmt_time_long(f64::INFINITY), "0m");
      assert_eq!(fmt_time_long(f64::NAN), "0m");
      assert_eq!(fmt_time_long(-5.0), "0m");
    }

    #[test]
    fn it_renders_a_normal_duration() {
      assert_eq!(
        fmt_time_long(14.0 * 86_400.0 + 3.0 * 3_600.0 + 22.0 * 60.0),
        "14d 3h 22m"
      );
    }
  }

  mod fmt_time_short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_guards_out_of_range_and_negative_input() {
      assert_eq!(fmt_time_short(f64::INFINITY), "0m");
      assert_eq!(fmt_time_short(f64::NAN), "0m");
      assert_eq!(fmt_time_short(-5.0), "0m");
    }

    #[test]
    fn it_renders_a_normal_duration() {
      assert_eq!(fmt_time_short(14.0 * 86_400.0 + 3.0 * 3_600.0), "14d 3h");
    }
  }
}
