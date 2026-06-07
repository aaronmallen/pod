use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Space, button, column, container, row, scrollable, text},
};

use super::{
  ACTIONS_COL_WIDTH, ATTR_COL_WIDTH, ComputedRow, EditRemap, GAP_START, Message, RemapControls, SP_COL_WIDTH, Sort,
  SortColumn, TIME_COL_WIDTH,
  entry_row::entry_row,
  remap_divider::remap_divider,
  remap_insertion::{insertion_gap, remap_exhausted},
  stats_strip::stats_strip,
};
use crate::ui::{
  components::rule,
  style::{color, radius, spacing, typography},
};

const LIST_SIDE_PADDING: f32 = 28.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn plan_entry_list<'a>(
  rows: &'a [ComputedRow],
  remaps: &'a [EditRemap],
  total_sp: u64,
  total_sec: f64,
  now: DateTime<Utc>,
  sort: Sort,
  note_open: Option<i64>,
  dragging: Option<i64>,
  drop_index: Option<usize>,
  hovered_gap: Option<i64>,
  controls: RemapControls<'a>,
) -> Element<'a, Message> {
  let card = container(
    column(vec![
      stats_strip(rows.len(), total_sp, total_sec, now),
      rule::horizontal(),
      col_header(sort),
      rule::horizontal(),
      entry_rows(rows, remaps, note_open, dragging, drop_index, hovered_gap, controls),
    ])
    .width(Length::Fill)
    .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: LIST_SIDE_PADDING,
      right: LIST_SIDE_PADDING,
    })
    .into()
}

#[allow(clippy::too_many_arguments)]
fn entry_rows<'a>(
  rows: &'a [ComputedRow],
  remaps: &'a [EditRemap],
  note_open: Option<i64>,
  dragging: Option<i64>,
  drop_index: Option<usize>,
  hovered_gap: Option<i64>,
  controls: RemapControls<'a>,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(rows.len() * 3 + 2);

  let start_remaps = remaps_anchored(remaps, None);
  let mut start_has_remap = false;
  for remap in start_remaps {
    children.push(remap_divider(remap, "Applied at start of plan"));
    children.push(rule::horizontal());
    start_has_remap = true;
  }
  if !start_has_remap {
    children.push(insertion_slot(None, GAP_START, true, hovered_gap, controls.reason));
  }

  for (index, entry) in rows.iter().enumerate() {
    let note_is_open = note_open == Some(entry.id);
    let is_dragging = dragging == Some(entry.id);
    let is_drop_target = drop_index == Some(index) && dragging.is_some() && !is_dragging;
    children.push(entry_row(entry, index, note_is_open, is_dragging, is_drop_target));
    children.push(rule::horizontal());

    let after = remaps_anchored(remaps, Some(entry.id));
    let mut has_remap = false;
    for remap in after {
      children.push(remap_divider(remap, &format!("After step {}", index + 1)));
      children.push(rule::horizontal());
      has_remap = true;
    }

    if !has_remap && index < rows.len() - 1 {
      children.push(insertion_slot(
        Some(entry.id),
        entry.id,
        controls.can_place,
        hovered_gap,
        controls.reason,
      ));
    }
  }

  scrollable(column(children).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn insertion_slot<'a>(
  after_entry_id: Option<i64>,
  gap_key: i64,
  enabled: bool,
  hovered_gap: Option<i64>,
  reason: &'a str,
) -> Element<'a, Message> {
  if enabled {
    insertion_gap(after_entry_id, gap_key, hovered_gap == Some(gap_key))
  } else {
    remap_exhausted(reason)
  }
}

fn remaps_anchored(remaps: &[EditRemap], anchor: Option<i64>) -> Vec<&EditRemap> {
  remaps.iter().filter(|remap| remap.after_entry_id == anchor).collect()
}

fn col_header<'a>(sort: Sort) -> Element<'a, Message> {
  container(
    row(vec![
      Space::new().width(28.0).into(),
      Space::new().width(spacing::SPACE_2).into(),
      Space::new().width(spacing::SPACE_3).into(),
      Space::new().width(spacing::SPACE_2).into(),
      header_label("Skill").width(Length::Fill).into(),
      sort_header("Pri", SortColumn::Primary, ATTR_COL_WIDTH, sort),
      sort_header("Sec", SortColumn::Secondary, ATTR_COL_WIDTH, sort),
      container(header_label("SP"))
        .width(Length::Fixed(SP_COL_WIDTH))
        .align_x(Horizontal::Right)
        .into(),
      Space::new().width(spacing::SPACE_2).into(),
      sort_header("Time / Cumul.", SortColumn::Time, TIME_COL_WIDTH, sort),
      Space::new().width(spacing::SPACE_2).into(),
      Space::new().width(ACTIONS_COL_WIDTH).into(),
      Space::new().width(spacing::SPACE_3).into(),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: spacing::SPACE_3,
      right: 0.0,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      radius: Radius {
        top_left: radius::CONTROL,
        top_right: radius::CONTROL,
        bottom_right: 0.0,
        bottom_left: 0.0,
      },
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn sort_header<'a>(label: &'a str, column: SortColumn, width: f32, sort: Sort) -> Element<'a, Message> {
  let active = sort.is_active(column);
  let label_color = if active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };

  let mut children: Vec<Element<'a, Message>> = vec![
    text(label.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
  ];
  if let Some(caret) = sort.caret(column) {
    children.push(
      text(caret)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::accent::PLASMA),
        })
        .into(),
    );
  }

  button(
    container(row(children).spacing(spacing::UNIT).align_y(Vertical::Center))
      .width(Length::Fill)
      .align_x(Horizontal::Right),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 4.0,
    right: 4.0,
  })
  .width(Length::Fixed(width))
  .on_press(Message::SortChanged(column))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
      }
      _ => None,
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    ..button::Style::default()
  })
  .into()
}

fn header_label(label: &str) -> text::Text<'_> {
  text(label.to_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::SECONDARY),
    })
}
