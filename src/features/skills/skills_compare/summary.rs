use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use super::{LABEL_COLUMN_WIDTH, Message, State};
use crate::{
  features::skills::fmt_sp,
  ui::{
    components::{avatar::Avatar, eyebrow::eyebrow_text, icon::Icon, rule},
    style::{color, radius, spacing, typography},
  },
};

const HEAD_PORTRAIT: f32 = 38.0;
const HEADS_ROW_HEIGHT: f32 = 60.0;
const MARKER_SIZE: f32 = 9.0;
const ROW_HEIGHT: f32 = 38.0;

pub(super) fn summary(state: &State) -> Element<'_, Message> {
  let ids = state.selected_ids();

  let total_sp: Vec<f64> = ids
    .iter()
    .map(|id| state.model(*id).map(|model| model.total_sp as f64).unwrap_or(0.0))
    .collect();
  let at_v: Vec<f64> = ids
    .iter()
    .map(|id| state.model(*id).map(|model| model.at_v_count as f64).unwrap_or(0.0))
    .collect();
  let at_iv: Vec<f64> = ids
    .iter()
    .map(|id| state.model(*id).map(|model| model.at_iv_count as f64).unwrap_or(0.0))
    .collect();
  let trained: Vec<f64> = ids
    .iter()
    .map(|id| state.model(*id).map(|model| model.trained_count as f64).unwrap_or(0.0))
    .collect();

  let body = Column::with_children(vec![
    heads_row(state),
    stat_row(state, &t!("skills.compare_summary.total_sp"), &total_sp, |value| {
      t!("skills.compare_summary.sp_value", sp => fmt_sp(value as i64)).into_owned()
    }),
    stat_row(state, &t!("skills.compare_summary.skills_at_v"), &at_v, |value| {
      (value as i64).to_string()
    }),
    stat_row(state, &t!("skills.compare_summary.skills_at_iv"), &at_iv, |value| {
      (value as i64).to_string()
    }),
    stat_row(state, &t!("skills.compare_summary.skills_trained"), &trained, |value| {
      (value as i64).to_string()
    }),
  ])
  .width(Length::Fill);

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT,
      right: spacing::SPACE_6,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_6,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn head_cell(state: &State, pilot_id: i64) -> Element<'_, Message> {
  let name = state.pilot_name(pilot_id).to_owned();
  let accent = state.pilot_accent(pilot_id);
  let portrait = state.portrait(pilot_id).path();

  let head = Row::with_children(vec![
    Avatar::new(
      pilot_id,
      name.clone(),
      Length::Fixed(HEAD_PORTRAIT),
      HEAD_PORTRAIT,
      portrait,
    )
    .radius(radius::CONTROL)
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
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::FillPortion(1));

  container(head)
    .width(Length::FillPortion(1))
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3_5,
      bottom: 0.0,
      left: spacing::SPACE_3_5,
    })
    .align_y(Vertical::Center)
    .into()
}

fn heads_row(state: &State) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![
    container(Space::new())
      .width(Length::Fixed(LABEL_COLUMN_WIDTH))
      .height(Length::Fixed(HEADS_ROW_HEIGHT))
      .into(),
  ];
  for id in state.selected_ids() {
    children.push(head_cell(state, *id));
  }

  let row = Row::with_children(children)
    .height(Length::Fixed(HEADS_ROW_HEIGHT))
    .align_y(Vertical::Center)
    .width(Length::Fill);

  Column::with_children(vec![row.into(), rule::horizontal_alpha(0.1)])
    .width(Length::Fill)
    .into()
}

/// Marks every column tied for the strict top value (value > 0). Ties all lead.
fn leaders(values: &[f64]) -> Vec<bool> {
  let max = values.iter().copied().fold(0.0_f64, f64::max);
  if max <= 0.0 {
    return vec![false; values.len()];
  }
  values.iter().map(|value| (max - value).abs() < 1e-9).collect()
}

fn stat_cell<'a>(state: &State, pilot_id: i64, label: String, leading: bool) -> Element<'a, Message> {
  let value_color = if leading {
    color::text::PRIMARY
  } else {
    color::with_alpha(color::text::PRIMARY, 0.75)
  };
  let font = if leading {
    typography::body::MEDIUM
  } else {
    typography::mono::REGULAR
  };

  let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(2);
  if leading {
    let accent = state.pilot_accent(pilot_id);
    cells.push(Icon::chevron_up().size(MARKER_SIZE).color(accent).render());
  }
  cells.push(
    text(label)
      .font(font)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(value_color),
      })
      .into(),
  );

  container(
    Row::with_children(cells)
      .spacing(spacing::SPACE_2 - 1.0)
      .align_y(Vertical::Center),
  )
  .width(Length::FillPortion(1))
  .height(Length::Fixed(ROW_HEIGHT))
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_3_5,
    bottom: 0.0,
    left: spacing::SPACE_3_5,
  })
  .align_y(Vertical::Center)
  .into()
}

fn stat_row<'a>(state: &State, label: &str, values: &[f64], fmt: impl Fn(f64) -> String) -> Element<'a, Message> {
  let ids = state.selected_ids();
  let lead = if values.len() > 1 {
    leaders(values)
  } else {
    vec![false; values.len()]
  };

  let label_cell = container(eyebrow_text(label, Some(color::text::secondary())))
    .width(Length::Fixed(LABEL_COLUMN_WIDTH))
    .height(Length::Fixed(ROW_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3_5,
      bottom: 0.0,
      left: spacing::SPACE_3_5,
    })
    .align_y(Vertical::Center);

  let mut children: Vec<Element<'a, Message>> = vec![label_cell.into()];
  for (index, value) in values.iter().copied().enumerate() {
    let pilot_id = ids.get(index).copied().unwrap_or_default();
    let leading = lead.get(index).copied().unwrap_or(false);
    children.push(stat_cell(state, pilot_id, fmt(value), leading));
  }

  Row::with_children(children).width(Length::Fill).into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod leaders {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_marks_every_column_tied_for_the_top() {
      assert_eq!(leaders(&[5.0, 5.0, 1.0]), vec![true, true, false]);
    }

    #[test]
    fn it_marks_no_column_when_every_value_is_zero() {
      assert_eq!(leaders(&[0.0, 0.0]), vec![false, false]);
    }

    #[test]
    fn it_marks_the_single_highest_value() {
      assert_eq!(leaders(&[10.0, 42.0, 7.0]), vec![false, true, false]);
    }
  }
}
