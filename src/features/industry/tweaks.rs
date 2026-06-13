use getset::{CopyGetters, Setters};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};
use serde::{Deserialize, Serialize};

use super::Message;
use crate::ui::{
  components::{eyebrow::eyebrow_text, segmented::segment_button, toggle::toggle},
  style::{color, radius, spacing, typography},
};

const PANEL_WIDTH: f32 = 300.0;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BarColor {
  #[default]
  Activity,
  Status,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
  #[default]
  Comfortable,
  Compact,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
  Activity,
  Facility,
  #[default]
  None,
  Owner,
}

#[derive(Clone, Copy, CopyGetters, Debug, Deserialize, Eq, PartialEq, Serialize, Setters)]
#[getset(get_copy = "pub", set = "pub")]
pub struct IndustryTweaks {
  #[serde(default)]
  bar_color: BarColor,
  #[serde(default)]
  density: Density,
  #[serde(default)]
  group_by: GroupBy,
  #[serde(default = "default_true")]
  show_rail: bool,
}

impl IndustryTweaks {
  pub fn is_default(&self) -> bool {
    *self == IndustryTweaks::default()
  }
}

impl Default for IndustryTweaks {
  fn default() -> Self {
    IndustryTweaks {
      bar_color: BarColor::default(),
      density: Density::default(),
      group_by: GroupBy::default(),
      show_rail: true,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tweak {
  BarColor(BarColor),
  Density(Density),
  GroupBy(GroupBy),
  ShowRail(bool),
}

impl Tweak {
  pub fn apply(self, tweaks: &mut IndustryTweaks) {
    match self {
      Tweak::BarColor(value) => {
        tweaks.set_bar_color(value);
      }
      Tweak::Density(value) => {
        tweaks.set_density(value);
      }
      Tweak::GroupBy(value) => {
        tweaks.set_group_by(value);
      }
      Tweak::ShowRail(value) => {
        tweaks.set_show_rail(value);
      }
    }
  }
}

pub(super) fn panel<'a>(tweaks: IndustryTweaks) -> Element<'a, Message> {
  let body = Column::with_children(vec![
    section("Layout"),
    segmented_row(
      "Density",
      &[
        (
          "Comfortable",
          tweaks.density() == Density::Comfortable,
          Tweak::Density(Density::Comfortable),
        ),
        (
          "Compact",
          tweaks.density() == Density::Compact,
          Tweak::Density(Density::Compact),
        ),
      ],
    ),
    toggle_row(
      "Jobs side rail",
      tweaks.show_rail(),
      Tweak::ShowRail(!tweaks.show_rail()),
    ),
    section("Jobs"),
    segmented_row(
      "Group by",
      &[
        (
          "None",
          tweaks.group_by() == GroupBy::None,
          Tweak::GroupBy(GroupBy::None),
        ),
        (
          "Owner",
          tweaks.group_by() == GroupBy::Owner,
          Tweak::GroupBy(GroupBy::Owner),
        ),
        (
          "Activity",
          tweaks.group_by() == GroupBy::Activity,
          Tweak::GroupBy(GroupBy::Activity),
        ),
        (
          "Facility",
          tweaks.group_by() == GroupBy::Facility,
          Tweak::GroupBy(GroupBy::Facility),
        ),
      ],
    ),
    segmented_row(
      "Bar color",
      &[
        (
          "Activity",
          tweaks.bar_color() == BarColor::Activity,
          Tweak::BarColor(BarColor::Activity),
        ),
        (
          "Status",
          tweaks.bar_color() == BarColor::Status,
          Tweak::BarColor(BarColor::Status),
        ),
      ],
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

fn default_true() -> bool {
  true
}

fn row_label<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()))
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
    fn it_flips_the_rail_toggle() {
      let mut tweaks = IndustryTweaks::default();

      Tweak::ShowRail(false).apply(&mut tweaks);

      assert!(!tweaks.show_rail());
    }

    #[test]
    fn it_sets_an_enum_tweak() {
      let mut tweaks = IndustryTweaks::default();

      Tweak::GroupBy(GroupBy::Activity).apply(&mut tweaks);

      assert_eq!(tweaks.group_by(), GroupBy::Activity);
    }
  }
}
