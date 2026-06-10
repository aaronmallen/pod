use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  border,
  widget::{Column, Row, Space, button, container, text},
};

use super::{LABEL_COLUMN_WIDTH, Message, State, model::GroupModel};
use crate::{
  features::skills::{
    browse::{SkillCatalogEntry, SkillCatalogGroup},
    queue_timing::roman,
  },
  ui::{
    components::{avatar::Avatar, icon::Icon, progress_bar::progress_bar, rule},
    style::{color, radius, spacing, typography},
  },
};

const CELL_PAD_X: f32 = 14.0;
const CELL_PAD_Y: f32 = 13.0;
const HEADS_PORTRAIT: f32 = 26.0;
const HEADS_UNDERLINE_ALPHA: f32 = 0.18;
const LEADER_GLYPH: &str = "\u{25bc}";
const MARKER_GAP: f32 = 2.0;
const MARKER_HEIGHT: f32 = 9.0;
const MARKER_SIZE: f32 = 8.0;
const MASTERY_BAR_HEIGHT: f32 = 8.0;
const PIP_GAP: f32 = 3.0;
const PIP_SIZE: f32 = 7.0;
const ROW_RULE_ALPHA: f32 = 0.1;
const SKILL_PAD_Y: f32 = 7.0;
const SKILL_RULE_ALPHA: f32 = 0.05;

pub(super) fn matrix(state: &State) -> Element<'_, Message> {
  let groups = &state.skill_catalog().groups;

  let mut rows: Vec<Element<'_, Message>> = vec![heads_row(state)];
  for group in groups {
    rows.push(group_block(state, group));
  }

  container(Column::with_children(rows).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, ROW_RULE_ALPHA),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
}

/// Marks every column tied for the strict top average (value > 0). Ties all lead; an all-zero
/// group has no leader.
fn avg_leaders(averages: &[f64]) -> Vec<bool> {
  let max = averages.iter().copied().fold(0.0_f64, f64::max);
  if max <= 0.0 {
    return vec![false; averages.len()];
  }
  averages.iter().map(|value| (max - value).abs() < 1e-9).collect()
}

fn group_block<'a>(state: &State, group: &'a SkillCatalogGroup) -> Element<'a, Message> {
  let ids = state.selected_ids();
  let leaders = group_leaders(state, group.id);

  let mut children: Vec<Element<'a, Message>> = vec![label_group(state, group)];
  for (index, id) in ids.iter().enumerate() {
    let is_leader = leaders.get(index).copied().unwrap_or(false);
    let summary = state.model(*id).and_then(|model| model.group(group.id).copied());
    children.push(rule::vertical_fill(ROW_RULE_ALPHA));
    children.push(group_cell(state, *id, summary, is_leader));
  }

  let group_row = Row::with_children(children)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let mut block: Vec<Element<'a, Message>> = vec![
    rule::horizontal_alpha(ROW_RULE_ALPHA),
    container(group_row).width(Length::Fill).into(),
  ];

  if state.is_expanded(group.id) {
    for skill in &group.skills {
      block.push(skill_row(state, skill));
    }
  }

  Column::with_children(block).width(Length::Fill).into()
}

fn group_cell<'a>(state: &State, pilot_id: i64, summary: Option<GroupModel>, leader: bool) -> Element<'a, Message> {
  let accent = state.pilot_accent(pilot_id);
  let summary = summary.unwrap_or_default();
  let fraction = (summary.cap_avg / 5.0) as f32;

  let at_v_color = if summary.at_v > 0 {
    color::with_alpha(color::text::PRIMARY, 0.7)
  } else {
    color::text::TERTIARY
  };

  let counts = Row::with_children(vec![
    text(format!("{}\u{00d7}V", summary.at_v))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(Length::Fill)
      .style(move |_| text::Style {
        color: Some(at_v_color),
      })
      .into(),
    text(format!("{}/{}", summary.trained, summary.total))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .width(Length::Fill);

  let content = Column::with_children(vec![mastery_bar(fraction, accent, leader), counts.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  pilot_cell(content.into())
}

fn group_leaders(state: &State, group_id: i64) -> Vec<bool> {
  let averages: Vec<f64> = state
    .selected_ids()
    .iter()
    .map(|id| {
      state
        .model(*id)
        .and_then(|model| model.group(group_id))
        .map(|group| group.cap_avg)
        .unwrap_or(0.0)
    })
    .collect();

  avg_leaders(&averages)
}

fn head_cell(state: &State, pilot_id: i64) -> Element<'_, Message> {
  let name = state.pilot_name(pilot_id).to_owned();
  let accent = state.pilot_accent(pilot_id);
  let portrait = state.portrait(pilot_id).path();

  let head = Row::with_children(vec![
    Avatar::new(
      pilot_id,
      name.clone(),
      Length::Fixed(HEADS_PORTRAIT),
      HEADS_PORTRAIT,
      portrait,
    )
    .radius(radius::SUBTLE)
    .border(color::with_alpha(accent, 0.5), 1.0)
    .view(),
    text(name)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(head)
    .width(Length::FillPortion(1))
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: CELL_PAD_X,
      bottom: spacing::SPACE_2_5,
      left: CELL_PAD_X,
    })
    .align_y(Vertical::Center)
    .into()
}

fn heads_row(state: &State) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![
    container(Space::new())
      .width(Length::Fixed(LABEL_COLUMN_WIDTH))
      .height(Length::Fill)
      .into(),
  ];
  for id in state.selected_ids() {
    children.push(rule::vertical_fill(ROW_RULE_ALPHA));
    children.push(head_cell(state, *id));
  }

  let row = Row::with_children(children)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let head = container(row).width(Length::Fill).style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      radius: border::Radius {
        top_left: radius::PANEL,
        top_right: radius::PANEL,
        bottom_left: 0.0,
        bottom_right: 0.0,
      },
      ..Border::default()
    },
    ..container::Style::default()
  });

  Column::with_children(vec![head.into(), rule::horizontal_alpha(HEADS_UNDERLINE_ALPHA)])
    .width(Length::Fill)
    .into()
}

fn label_group<'a>(state: &State, group: &'a SkillCatalogGroup) -> Element<'a, Message> {
  let expanded = state.is_expanded(group.id);

  let chevron = if expanded {
    Icon::chevron()
  } else {
    Icon::chevron_right()
  }
  .size(12.0)
  .color(color::text::TERTIARY)
  .render();

  button(
    Row::with_children(vec![
      chevron,
      text(group.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(Length::Fill),
  )
  .width(Length::Fixed(LABEL_COLUMN_WIDTH))
  .padding(Padding {
    top: CELL_PAD_Y,
    right: CELL_PAD_X,
    bottom: CELL_PAD_Y,
    left: CELL_PAD_X,
  })
  .on_press(Message::GroupToggled(group.id))
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn label_skill(skill: &SkillCatalogEntry) -> Element<'_, Message> {
  Row::with_children(vec![
    text(skill.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .width(Length::Fill)
      .style(|_| text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    text(format!("\u{00d7}{}", skill.rank))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center)
  .width(Length::Fixed(LABEL_COLUMN_WIDTH))
  .padding(Padding {
    top: SKILL_PAD_Y,
    right: CELL_PAD_X,
    bottom: SKILL_PAD_Y,
    left: 34.0,
  })
  .into()
}

/// Marks every column tied for the top trained level (level > 0). Ties all lead; an untrained
/// skill (every level 0) has no leader.
fn level_leaders(levels: &[u8]) -> Vec<bool> {
  let max = levels.iter().copied().max().unwrap_or(0);
  if max == 0 {
    return vec![false; levels.len()];
  }
  levels.iter().map(|level| *level == max).collect()
}

fn mastery_bar<'a>(fraction: f32, accent: Color, leader: bool) -> Element<'a, Message> {
  Column::with_children(vec![
    mastery_marker(fraction, accent, leader),
    progress_bar(fraction, accent, MASTERY_BAR_HEIGHT),
  ])
  .spacing(MARKER_GAP)
  .width(Length::Fill)
  .into()
}

/// A caret pinned to the tip of the bar fill marking a leading pilot. Non-leaders reserve the same
/// height so cells in a row stay aligned.
fn mastery_marker<'a>(fraction: f32, accent: Color, leader: bool) -> Element<'a, Message> {
  if !leader {
    return container(Space::new())
      .width(Length::Fill)
      .height(Length::Fixed(MARKER_HEIGHT))
      .into();
  }

  let filled = (fraction.clamp(0.0, 1.0) * 1000.0) as u16;
  let remaining = 1000u16.saturating_sub(filled);
  let caret = text(LEADER_GLYPH)
    .font(typography::mono::REGULAR)
    .size(MARKER_SIZE)
    .style(move |_| text::Style {
      color: Some(accent),
    });

  Row::with_children(vec![
    Space::new().width(marker_portion(filled)).into(),
    caret.into(),
    Space::new().width(marker_portion(remaining)).into(),
  ])
  .height(Length::Fixed(MARKER_HEIGHT))
  .align_y(Vertical::Bottom)
  .into()
}

fn marker_portion(factor: u16) -> Length {
  if factor == 0 {
    Length::Fixed(0.0)
  } else {
    Length::FillPortion(factor)
  }
}

fn pilot_cell<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
  container(content)
    .width(Length::FillPortion(1))
    .padding(Padding {
      top: CELL_PAD_Y,
      right: CELL_PAD_X,
      bottom: CELL_PAD_Y,
      left: CELL_PAD_X,
    })
    .align_y(Vertical::Center)
    .into()
}

fn pilot_level(state: &State, pilot_id: i64, skill_id: i64) -> u8 {
  state
    .model(pilot_id)
    .and_then(|model| model.levels.get(&skill_id).copied())
    .unwrap_or(0)
}

fn pips<'a>(level: u8, accent: Color) -> Element<'a, Message> {
  let cells = (1..=5u8).map(|i| {
    let fill = if i <= level {
      accent
    } else {
      color::with_alpha(color::text::PRIMARY, 0.12)
    };
    container(Space::new())
      .width(Length::Fixed(PIP_SIZE))
      .height(Length::Fixed(PIP_SIZE))
      .style(move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
          radius: 1.5.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  });

  Row::with_children(cells).spacing(PIP_GAP).into()
}

fn skill_cell<'a>(state: &State, pilot_id: i64, level: u8, leader: bool) -> Element<'a, Message> {
  let accent = state.pilot_accent(pilot_id);
  let label = if level == 0 {
    "\u{2014}".to_owned()
  } else {
    roman(i64::from(level))
  };
  let label_color = if level == 0 {
    color::text::TERTIARY
  } else if leader {
    color::text::PRIMARY
  } else {
    color::text::SECONDARY
  };

  let content = Row::with_children(vec![
    pips(level, accent),
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(content)
    .width(Length::FillPortion(1))
    .padding(Padding {
      top: SKILL_PAD_Y,
      right: CELL_PAD_X,
      bottom: SKILL_PAD_Y,
      left: CELL_PAD_X,
    })
    .align_y(Vertical::Center)
    .into()
}

fn skill_leaders(state: &State, skill_id: i64) -> Vec<bool> {
  let levels: Vec<u8> = state
    .selected_ids()
    .iter()
    .map(|id| pilot_level(state, *id, skill_id))
    .collect();

  level_leaders(&levels)
}

fn skill_row<'a>(state: &State, skill: &'a SkillCatalogEntry) -> Element<'a, Message> {
  let ids = state.selected_ids();
  let leaders = skill_leaders(state, skill.type_id);

  let mut children: Vec<Element<'a, Message>> = vec![label_skill(skill)];
  for (index, id) in ids.iter().enumerate() {
    let is_leader = leaders.get(index).copied().unwrap_or(false);
    let level = pilot_level(state, *id, skill.type_id);
    children.push(rule::vertical_fill(ROW_RULE_ALPHA));
    children.push(skill_cell(state, *id, level, is_leader));
  }

  let row = Row::with_children(children)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let body = container(row).width(Length::Fill).style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  });

  Column::with_children(vec![rule::horizontal_alpha(SKILL_RULE_ALPHA), body.into()])
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod avg_leaders {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_marks_the_single_highest_average() {
      assert_eq!(avg_leaders(&[1.5, 3.2, 2.0]), vec![false, true, false]);
    }

    #[test]
    fn it_marks_every_column_tied_for_the_top() {
      assert_eq!(avg_leaders(&[2.5, 2.5, 1.0]), vec![true, true, false]);
    }

    #[test]
    fn it_marks_no_column_when_every_average_is_zero() {
      assert_eq!(avg_leaders(&[0.0, 0.0]), vec![false, false]);
    }
  }

  mod level_leaders {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_marks_the_single_highest_level() {
      assert_eq!(level_leaders(&[3, 5, 4]), vec![false, true, false]);
    }

    #[test]
    fn it_marks_every_column_tied_for_the_top() {
      assert_eq!(level_leaders(&[5, 5, 2]), vec![true, true, false]);
    }

    #[test]
    fn it_marks_all_columns_when_levels_are_unanimous() {
      assert_eq!(level_leaders(&[4, 4, 4]), vec![true, true, true]);
    }

    #[test]
    fn it_marks_no_column_when_all_levels_are_zero() {
      assert_eq!(level_leaders(&[0, 0]), vec![false, false]);
    }
  }
}
