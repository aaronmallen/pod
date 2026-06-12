use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use super::Message;
use crate::{
  config::{CalendarDensity, CalendarTweaks, CalendarWeekStart},
  ui::{
    components::{eyebrow::eyebrow_text, segmented::segment_button, toggle::toggle},
    style::{color, radius, spacing, typography},
  },
};

const PANEL_WIDTH: f32 = 300.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tweak {
  ColorByPilot(bool),
  Density(CalendarDensity),
  LocalTime(bool),
  MonthChips(bool),
  PodOverlays(bool),
  ShowWeekends(bool),
  WeekHours(bool),
  WeekStart(CalendarWeekStart),
}

impl Tweak {
  pub fn apply(self, tweaks: &mut CalendarTweaks) {
    match self {
      Tweak::ColorByPilot(value) => {
        tweaks.set_color_by_pilot(value);
      }
      Tweak::Density(value) => {
        tweaks.set_density(value);
      }
      Tweak::LocalTime(value) => {
        tweaks.set_local_time(value);
      }
      Tweak::MonthChips(value) => {
        tweaks.set_month_chips(value);
      }
      Tweak::PodOverlays(value) => {
        tweaks.set_pod_overlays(value);
      }
      Tweak::ShowWeekends(value) => {
        tweaks.set_show_weekends(value);
      }
      Tweak::WeekHours(value) => {
        tweaks.set_week_hours(value);
      }
      Tweak::WeekStart(value) => {
        tweaks.set_week_start(value);
      }
    }
  }
}

pub(super) fn panel<'a>(tweaks: CalendarTweaks) -> Element<'a, Message> {
  let body = Column::with_children(vec![
    section("Display"),
    segmented_row(
      "Color by",
      &[
        ("Owner", !tweaks.color_by_pilot(), Tweak::ColorByPilot(false)),
        ("Pilot", tweaks.color_by_pilot(), Tweak::ColorByPilot(true)),
      ],
    ),
    segmented_row(
      "Density",
      &[
        (
          "Comfortable",
          tweaks.density() == CalendarDensity::Comfortable,
          Tweak::Density(CalendarDensity::Comfortable),
        ),
        (
          "Compact",
          tweaks.density() == CalendarDensity::Compact,
          Tweak::Density(CalendarDensity::Compact),
        ),
      ],
    ),
    toggle_row(
      "Local time",
      tweaks.local_time(),
      Tweak::LocalTime(!tweaks.local_time()),
    ),
    toggle_row(
      "Pod overlays",
      tweaks.pod_overlays(),
      Tweak::PodOverlays(!tweaks.pod_overlays()),
    ),
    section("Grids"),
    toggle_row(
      "Month chips",
      tweaks.month_chips(),
      Tweak::MonthChips(!tweaks.month_chips()),
    ),
    toggle_row(
      "Week hours",
      tweaks.week_hours(),
      Tweak::WeekHours(!tweaks.week_hours()),
    ),
    segmented_row(
      "Week starts",
      &[
        (
          "Mon",
          tweaks.week_start() == CalendarWeekStart::Monday,
          Tweak::WeekStart(CalendarWeekStart::Monday),
        ),
        (
          "Sun",
          tweaks.week_start() == CalendarWeekStart::Sunday,
          Tweak::WeekStart(CalendarWeekStart::Sunday),
        ),
      ],
    ),
    toggle_row(
      "Show weekends",
      tweaks.show_weekends(),
      Tweak::ShowWeekends(!tweaks.show_weekends()),
    ),
  ])
  .spacing(spacing::SPACE_2_5)
  .padding(spacing::SPACE_3);

  container(body)
    .width(Length::Fixed(PANEL_WIDTH))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn section<'a>(label: &str) -> Element<'a, Message> {
  container(eyebrow_text(label, None))
    .padding(Padding {
      top: spacing::SPACE_2,
      ..Padding::ZERO
    })
    .into()
}

fn segmented_row<'a>(label: &str, options: &[(&str, bool, Tweak)]) -> Element<'a, Message> {
  let segments: Vec<Element<'a, Message>> = options
    .iter()
    .map(|(text_label, active, tweak)| {
      segment_button(
        (*text_label).to_owned(),
        *active,
        Padding {
          top: spacing::UNIT,
          bottom: spacing::UNIT,
          left: spacing::SPACE_2,
          right: spacing::SPACE_2,
        },
        Message::TweakChanged(*tweak),
      )
    })
    .collect();

  let control = container(Row::with_children(segments).spacing(2.0))
    .padding(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  Row::with_children(vec![
    row_label(label),
    Space::new().width(Length::Fill).into(),
    control.into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn row_label<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn toggle_row<'a>(label: &str, on: bool, tweak: Tweak) -> Element<'a, Message> {
  Row::with_children(vec![
    row_label(label),
    Space::new().width(Length::Fill).into(),
    toggle(on, Message::TweakChanged(tweak)),
  ])
  .align_y(Vertical::Center)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod apply {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_flips_a_boolean_tweak() {
      let mut tweaks = CalendarTweaks::default();

      Tweak::PodOverlays(true).apply(&mut tweaks);

      assert!(tweaks.pod_overlays());
    }

    #[test]
    fn it_sets_an_enum_tweak() {
      let mut tweaks = CalendarTweaks::default();

      Tweak::WeekStart(CalendarWeekStart::Sunday).apply(&mut tweaks);

      assert_eq!(tweaks.week_start(), CalendarWeekStart::Sunday);
    }
  }
}
