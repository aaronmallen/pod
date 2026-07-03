use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Space, column, container, scrollable},
};

use super::Message;
pub(super) use crate::features::skills::plan_summary::{ImplantEffect, SummaryData};
use crate::{
  features::skills::{
    plan_math::injectors_for_plan,
    plan_summary::{
      attr_optimization_section::attr_optimization_section,
      implant_effect_section::{has_implants, implant_effect_section},
      injector_section::injector_section,
      plan_totals_section::plan_totals_section,
      time_by_group_section::time_by_group_section,
      time_by_pair_section::time_by_pair_section,
    },
  },
  ui::{
    components::rule,
    style::{color, spacing},
  },
};

pub(super) fn summary(data: SummaryData, now: DateTime<Utc>) -> Element<'static, Message> {
  let mut sections: Vec<Element<'static, Message>> = vec![plan_totals_section(
    data.total_sec,
    data.total_sp,
    data.steps,
    data.is_template,
    now,
  )];

  if !data.is_template {
    sections.push(rule::horizontal());
    sections.push(attr_optimization_section(
      data.base_attrs,
      data.current_base_sec,
      &data.recommendation,
      data.remap_availability,
      &data.remap_reason,
    ));
  }

  if data.total_sp > 0 {
    let estimate = injectors_for_plan(data.total_sp, data.character_total_sp);
    sections.push(rule::horizontal());
    sections.push(injector_section(estimate, data.total_sp));
  }

  if has_implants(&data.implant_effect) {
    sections.push(rule::horizontal());
    sections.push(implant_effect_section(&data.implant_effect));
  }

  if !data.group_sec.is_empty() {
    sections.push(rule::horizontal());
    sections.push(time_by_group_section(&data.group_sec));
  }
  if !data.pair_sec.is_empty() {
    sections.push(rule::horizontal());
    sections.push(time_by_pair_section(&data.pair_sec));
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

  mod summary_injectors {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_charges_zero_injectors_when_no_needed_sp_remains() {
      let estimate = injectors_for_plan(0, 50_000_000);

      assert_eq!(estimate.large, 0);
      assert_eq!(estimate.small, 0);
    }

    #[test]
    fn it_estimates_injectors_from_the_needed_only_plan_sp() {
      let trained_only = injectors_for_plan(0, 50_000_000);
      let needed_only = injectors_for_plan(600_000, 50_000_000);

      assert_eq!(trained_only.large + trained_only.small, 0);
      assert!(needed_only.large + needed_only.small > 0);
    }
  }
}
