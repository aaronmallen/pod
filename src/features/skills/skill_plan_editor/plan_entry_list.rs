use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Space, button, column, container, row, scrollable, text},
};

use super::{
  ACTIONS_COL_WIDTH, ATTR_COL_WIDTH, ComputedRow, EditMilestone, GAP_START, Message, MilestoneStats, SP_COL_WIDTH,
  Sort, SortColumn, SortDirection, TIME_COL_WIDTH, entry_row::entry_row, milestone_divider::milestone_divider,
  milestone_insertion::milestone_insertion, stats_strip::stats_strip,
};
use crate::ui::{
  components::{icon::Icon, rule},
  style::{color, radius, spacing, typography},
};

const LIST_SIDE_PADDING: f32 = 28.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn plan_entry_list<'a>(
  rows: &'a [ComputedRow],
  milestones: &'a [EditMilestone],
  stats: HashMap<i64, MilestoneStats>,
  total_sp: u64,
  total_sec: f64,
  is_template: bool,
  now: DateTime<Utc>,
  sort: Sort,
  note_open: Option<i64>,
  dragging: Option<i64>,
  drop_index: Option<usize>,
  hovered_gap: Option<i64>,
  import_menu: Option<i64>,
  export_menu: Option<i64>,
  collapsed: &HashSet<i64>,
) -> Element<'a, Message> {
  let numbers = display_numbers(rows);
  let visible_steps = numbers.iter().flatten().count();
  let card = container(
    column(vec![
      stats_strip(visible_steps, total_sp, total_sec, is_template, now),
      rule::horizontal(),
      col_header(sort),
      rule::horizontal(),
      entry_rows(
        rows,
        &numbers,
        milestones,
        &stats,
        note_open,
        dragging,
        drop_index,
        hovered_gap,
        import_menu,
        export_menu,
        collapsed,
        sort.column == SortColumn::Manual,
      ),
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
  numbers: &[Option<usize>],
  milestones: &'a [EditMilestone],
  stats: &HashMap<i64, MilestoneStats>,
  note_open: Option<i64>,
  dragging: Option<i64>,
  drop_index: Option<usize>,
  hovered_gap: Option<i64>,
  import_menu: Option<i64>,
  export_menu: Option<i64>,
  collapsed: &HashSet<i64>,
  is_manual: bool,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(rows.len() * 3 + 2);
  let visibility = segment_visibility(rows, milestones, collapsed);

  push_milestones(
    &mut children,
    milestones,
    stats,
    None,
    import_menu,
    export_menu,
    collapsed,
  );
  if is_manual && !visibility.start_pill_hidden {
    children.push(milestone_insertion(None, GAP_START, hovered_gap == Some(GAP_START)));
  }

  for (index, entry) in rows.iter().enumerate() {
    if !visibility.hidden_rows.contains(&entry.id)
      && let Some(display_number) = numbers[index]
    {
      push_entry_row(
        &mut children,
        entry,
        index,
        display_number,
        note_open,
        dragging,
        drop_index,
      );
    }

    push_milestones(
      &mut children,
      milestones,
      stats,
      Some(entry.id),
      import_menu,
      export_menu,
      collapsed,
    );

    if numbers[index].is_some() && is_manual && !visibility.hidden_pills.contains(&entry.id) {
      children.push(milestone_insertion(
        Some(entry.id),
        entry.id,
        hovered_gap == Some(entry.id),
      ));
    }
  }

  scrollable(column(children).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

// Renders the milestones anchored at `anchor` (`None` = the start of the list, before any entry), ordered by `order`
// since more than one milestone can share the same anchor. Each divider carries a chevron reflecting whether its
// milestone is in `collapsed`; the segment-row/pill gating that flows from that state is precomputed by
// `segment_visibility`.
fn push_milestones<'a>(
  children: &mut Vec<Element<'a, Message>>,
  milestones: &'a [EditMilestone],
  stats: &HashMap<i64, MilestoneStats>,
  anchor: Option<i64>,
  import_menu: Option<i64>,
  export_menu: Option<i64>,
  collapsed: &HashSet<i64>,
) {
  let mut matched: Vec<&'a EditMilestone> = milestones
    .iter()
    .filter(|milestone| milestone.after_entry_id == anchor)
    .collect();
  matched.sort_by_key(|milestone| milestone.order);

  for milestone in matched {
    let stat = stats.get(&milestone.local_id).copied().unwrap_or_default();
    children.push(milestone_divider(
      milestone,
      stat,
      import_menu == Some(milestone.local_id),
      export_menu == Some(milestone.local_id),
      collapsed.contains(&milestone.local_id),
    ));
  }
}

// Which entry rows and insertion pills are hidden because they fall inside a collapsed milestone segment. A segment
// runs from a milestone divider up to the next divider; the pre-first-milestone `__start` bucket is never collapsible
// so tracking begins expanded, and anchors with no divider carry the current segment's state through. Pills are keyed
// by the entry they sit after (`GAP_START` for the top-of-plan pill), matching the render walk in `entry_rows`.
#[derive(Default)]
struct SegmentVisibility {
  hidden_rows: HashSet<i64>,
  hidden_pills: HashSet<i64>,
  start_pill_hidden: bool,
}

fn segment_visibility(
  rows: &[ComputedRow],
  milestones: &[EditMilestone],
  collapsed: &HashSet<i64>,
) -> SegmentVisibility {
  // Collapse state of the highest-`order` milestone anchored after each entry (the one that governs the segment that
  // opens there), mirroring the last divider `push_milestones` renders at that anchor.
  let mut anchor_state: HashMap<Option<i64>, (i64, bool)> = HashMap::new();
  for milestone in milestones {
    let is_collapsed = collapsed.contains(&milestone.local_id);
    anchor_state
      .entry(milestone.after_entry_id)
      .and_modify(|slot| {
        if milestone.order >= slot.0 {
          *slot = (milestone.order, is_collapsed);
        }
      })
      .or_insert((milestone.order, is_collapsed));
  }
  let last_at = |anchor: Option<i64>| anchor_state.get(&anchor).map(|slot| slot.1);

  let mut visibility = SegmentVisibility::default();
  let mut in_collapsed = last_at(None).unwrap_or(false);
  visibility.start_pill_hidden = in_collapsed;

  for entry in rows {
    if in_collapsed {
      visibility.hidden_rows.insert(entry.id);
    }
    if let Some(state) = last_at(Some(entry.id)) {
      in_collapsed = state;
    }
    if in_collapsed {
      visibility.hidden_pills.insert(entry.id);
    }
  }
  visibility
}

fn push_entry_row<'a>(
  children: &mut Vec<Element<'a, Message>>,
  entry: &'a ComputedRow,
  index: usize,
  display_number: usize,
  note_open: Option<i64>,
  dragging: Option<i64>,
  drop_index: Option<usize>,
) {
  let note_is_open = note_open == Some(entry.id);
  let is_dragging = dragging == Some(entry.id);
  let is_drop_target = drop_index == Some(index) && dragging.is_some() && !is_dragging;
  children.push(entry_row(
    entry,
    index,
    display_number,
    note_is_open,
    is_dragging,
    is_drop_target,
  ));
  children.push(rule::horizontal());
}

fn display_numbers(rows: &[ComputedRow]) -> Vec<Option<usize>> {
  let mut next = 0;
  rows
    .iter()
    .map(|row| {
      if row.skipped {
        None
      } else {
        next += 1;
        Some(next)
      }
    })
    .collect()
}

fn col_header<'a>(sort: Sort) -> Element<'a, Message> {
  container(
    row(vec![
      Space::new().width(28.0).into(),
      Space::new().width(spacing::SPACE_2).into(),
      Space::new().width(spacing::SPACE_3).into(),
      Space::new().width(spacing::SPACE_2).into(),
      header_label(t!("skills.editor.col_skill").into_owned())
        .width(Length::Fill)
        .into(),
      sort_header(
        t!("skills.editor.col_primary").into_owned(),
        SortColumn::Primary,
        ATTR_COL_WIDTH,
        sort,
      ),
      sort_header(
        t!("skills.editor.col_secondary").into_owned(),
        SortColumn::Secondary,
        ATTR_COL_WIDTH,
        sort,
      ),
      container(header_label(t!("skills.editor.col_sp").into_owned()))
        .width(Length::Fixed(SP_COL_WIDTH))
        .align_x(Horizontal::Right)
        .into(),
      Space::new().width(spacing::SPACE_2).into(),
      sort_header(
        t!("skills.editor.col_time").into_owned(),
        SortColumn::Time,
        TIME_COL_WIDTH,
        sort,
      ),
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

fn sort_header<'a>(label: String, column: SortColumn, width: f32, sort: Sort) -> Element<'a, Message> {
  let active = sort.is_active(column);
  let label_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  let mut children: Vec<Element<'a, Message>> = vec![
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(label_color),
      })
      .into(),
  ];
  if let Some(direction) = sort.caret(column) {
    let caret = match direction {
      SortDirection::Ascending => Icon::chevron_up(),
      SortDirection::Descending => Icon::chevron_down(),
    };
    children.push(caret.size(typography::size::XS).color(color::accent()).render());
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

fn header_label<'a>(label: String) -> text::Text<'a> {
  text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::skills::browse::AttrKey;

  fn row(id: i64, skipped: bool) -> ComputedRow {
    ComputedRow {
      cumulative_sec: 0.0,
      group_name: String::new(),
      id,
      is_auto: false,
      note: String::new(),
      primary: AttrKey::Perception,
      priority: super::super::Priority::Normal,
      rank: 1,
      sec: 0.0,
      secondary: AttrKey::Willpower,
      skill_name: String::new(),
      skipped,
      sp: 0,
      to_level: 5,
    }
  }

  mod display_numbers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_numbers_every_row_when_none_are_skipped() {
      let rows = vec![row(1, false), row(2, false), row(3, false)];

      assert_eq!(display_numbers(&rows), vec![Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn it_hides_skipped_rows_and_renumbers_the_remainder() {
      let rows = vec![row(1, true), row(2, false), row(3, true), row(4, false)];

      assert_eq!(display_numbers(&rows), vec![None, Some(1), None, Some(2)]);
    }

    #[test]
    fn it_hides_every_row_when_all_are_skipped() {
      let rows = vec![row(1, true), row(2, true)];

      assert_eq!(display_numbers(&rows), vec![None, None]);
    }
  }

  mod segment_visibility {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;

    use super::*;

    fn milestone(local_id: i64, after_entry_id: Option<i64>, order: i64) -> EditMilestone {
      EditMilestone {
        after_entry_id,
        auto_remap: false,
        base: None,
        local_id,
        name: String::new(),
        order,
      }
    }

    fn collapsed(ids: &[i64]) -> HashSet<i64> {
      ids.iter().copied().collect()
    }

    #[test]
    fn it_hides_no_rows_when_nothing_is_collapsed() {
      let rows = vec![row(1, false), row(2, false), row(3, false)];
      let milestones = vec![milestone(100, Some(1), 0)];

      let visibility = super::super::segment_visibility(&rows, &milestones, &HashSet::new());

      assert!(visibility.hidden_rows.is_empty());
      assert!(visibility.hidden_pills.is_empty());
    }

    #[test]
    fn it_hides_exactly_the_collapsed_segments_rows() {
      // Milestone 100 heads the segment after entry 1, so its rows are entries 2 and 3.
      let rows = vec![row(1, false), row(2, false), row(3, false)];
      let milestones = vec![milestone(100, Some(1), 0)];

      let visibility = super::super::segment_visibility(&rows, &milestones, &collapsed(&[100]));

      assert_eq!(visibility.hidden_rows, collapsed(&[2, 3]));
    }

    #[test]
    fn it_never_collapses_the_pre_first_milestone_start_bucket() {
      // The first milestone sits after entry 2, so entries 1 and 2 are the uncollapsible `__start` bucket.
      let rows = vec![row(1, false), row(2, false), row(3, false)];
      let milestones = vec![milestone(100, Some(2), 0)];

      let visibility = super::super::segment_visibility(&rows, &milestones, &collapsed(&[100]));

      assert_eq!(visibility.hidden_rows, collapsed(&[3]));
      assert!(!visibility.start_pill_hidden);
    }

    #[test]
    fn it_isolates_collapse_to_one_milestone_when_several_exist() {
      // Milestone 100 owns entries 2 and 3; milestone 200 owns entry 4. Collapsing only 100 leaves 4 visible.
      let rows = vec![row(1, false), row(2, false), row(3, false), row(4, false)];
      let milestones = vec![milestone(100, Some(1), 0), milestone(200, Some(3), 1)];

      let visibility = super::super::segment_visibility(&rows, &milestones, &collapsed(&[100]));

      assert_eq!(visibility.hidden_rows, collapsed(&[2, 3]));
    }

    #[test]
    fn it_hides_insertion_pills_inside_a_collapsed_segment() {
      let rows = vec![row(1, false), row(2, false), row(3, false)];
      let milestones = vec![milestone(100, Some(1), 0)];

      let visibility = super::super::segment_visibility(&rows, &milestones, &collapsed(&[100]));

      // The pill after entry 1 opens milestone 100's collapsed segment, so it and every following pill are hidden.
      assert_eq!(visibility.hidden_pills, collapsed(&[1, 2, 3]));
    }
  }
}
