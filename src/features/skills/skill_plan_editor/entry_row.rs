use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, mouse_area, row, text, text_input},
};

use super::{
  ACTIONS_COL_WIDTH, ATTR_COL_WIDTH, ComputedRow, INDEX_COL_WIDTH, Message, Priority, SP_COL_WIDTH, TIME_COL_WIDTH,
  fmt_sp,
};
use crate::{
  features::skills::{browse::AttrKey, queue_timing::roman},
  ui::{
    components::{badge::badge, status},
    style::{color, spacing, typography},
  },
};

pub(super) fn entry_row<'a>(
  entry: &'a ComputedRow,
  index: usize,
  display_number: usize,
  note_open: bool,
  is_dragging: bool,
  is_drop_target: bool,
) -> Element<'a, Message> {
  let inner = row(vec![
    index_col(display_number),
    Space::new().width(spacing::SPACE_2).into(),
    priority_dot(entry.priority, entry.id),
    Space::new().width(spacing::SPACE_2).into(),
    skill_col(entry),
    attr_col(entry.primary, true),
    attr_col(entry.secondary, false),
    sp_col(entry),
    Space::new().width(spacing::SPACE_2).into(),
    time_col(entry),
    Space::new().width(spacing::SPACE_2).into(),
    actions_col(entry.id, is_dragging, entry.is_auto),
    Space::new().width(spacing::SPACE_3).into(),
  ])
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: spacing::SPACE_3,
    right: 0.0,
  });

  let row_bg = is_dragging.then(|| Background::Color(color::with_alpha(color::accent(), 0.06)));
  let row_body = container(inner).width(Length::Fill).style(move |_| container::Style {
    background: row_bg,
    ..container::Style::default()
  });

  let hoverable = mouse_area(column(vec![drop_bar(is_drop_target), row_body.into()]).width(Length::Fill))
    .on_enter(Message::DragHovered(index))
    .on_exit(Message::DragLeft(index))
    .on_right_press(Message::EntryRowRightPressed(entry.id));

  let mut children: Vec<Element<'a, Message>> = vec![hoverable.into()];
  if note_open {
    children.push(note_editor(&entry.note, entry.id));
  }
  column(children).width(Length::Fill).into()
}

fn drop_bar<'a>(is_drop_target: bool) -> Element<'a, Message> {
  if is_drop_target {
    container(Space::new().width(Length::Fill).height(2.0))
      .width(Length::Fill)
      .height(2.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent())),
        ..container::Style::default()
      })
      .into()
  } else {
    Space::new().height(0.0).into()
  }
}

fn index_col<'a>(display_number: usize) -> Element<'a, Message> {
  container(
    text(format!("#{display_number}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .width(Length::Fixed(INDEX_COL_WIDTH))
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .into()
}

fn priority_dot<'a>(priority: Priority, id: i64) -> Element<'a, Message> {
  let dot = status::dot_sized(priority_color(priority), 8.0);

  button(dot)
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 6.0,
      right: 6.0,
    })
    .on_press(Message::EntryPriorityCycled(id))
    .style(|_, status| button::Style {
      background: hover_overlay(status),
      border: Border {
        radius: 4.0.into(),
        ..Border::default()
      },
      ..button::Style::default()
    })
    .into()
}

fn priority_color(priority: Priority) -> Color {
  match priority {
    Priority::Low => color::status::ONLINE,
    Priority::Normal => color::text::tertiary(),
    Priority::High => color::status::DANGER,
  }
}

fn skill_col<'a>(entry: &'a ComputedRow) -> Element<'a, Message> {
  let mut name_items: Vec<Element<'a, Message>> = vec![
    text(entry.skill_name.clone())
      .font(typography::body::MEDIUM)
      .size(14.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    text(roman(i64::from(entry.to_level)))
      .font(typography::mono::MEDIUM)
      .size(13.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];

  if entry.is_auto {
    name_items.push(Space::new().width(spacing::SPACE_2).into());
    name_items.push(badge(
      t!("skills.editor.prereq_badge").into_owned(),
      Some(color::status::ONLINE),
    ));
  }

  let name_row = row(name_items).align_y(Vertical::Center);

  let rank_sub = row(vec![
    text(format!("×{}", entry.rank))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .align_y(Vertical::Center);

  column(vec![name_row.into(), Space::new().height(4.0).into(), rank_sub.into()])
    .width(Length::Fill)
    .into()
}

fn attr_col<'a>(key: AttrKey, primary: bool) -> Element<'a, Message> {
  let chip = if primary {
    badge(key.short(), Some(color::accent()))
  } else {
    badge(key.short(), None)
  };

  container(chip)
    .width(Length::Fixed(ATTR_COL_WIDTH))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn sp_col<'a>(entry: &'a ComputedRow) -> Element<'a, Message> {
  container(
    column(vec![
      text(fmt_sp(entry.sp))
        .font(typography::mono::MEDIUM)
        .size(13.0)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(t!("skills.editor.sp_unit"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    ])
    .align_x(Horizontal::Right)
    .spacing(2.0),
  )
  .width(Length::Fixed(SP_COL_WIDTH))
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .into()
}

fn time_col<'a>(entry: &'a ComputedRow) -> Element<'a, Message> {
  container(
    column(vec![
      text(fmt_dur_short(entry.sec))
        .font(typography::mono::MEDIUM)
        .size(13.0)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(t!("skills.editor.cumulative_time", time => fmt_dur_short(entry.cumulative_sec)))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    ])
    .align_x(Horizontal::Right)
    .spacing(2.0),
  )
  .width(Length::Fixed(TIME_COL_WIDTH))
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .into()
}

fn actions_col<'a>(id: i64, is_dragging: bool, is_auto: bool) -> Element<'a, Message> {
  let remove: Element<'a, Message> = if is_auto { locked_remove() } else { remove_btn(id) };

  container(
    row(vec![
      note_btn(id),
      Space::new().width(4.0).into(),
      drag_handle(id, is_dragging),
      Space::new().width(4.0).into(),
      remove,
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fixed(ACTIONS_COL_WIDTH))
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .into()
}

fn locked_remove<'a>() -> Element<'a, Message> {
  container(
    text("\u{1f512}")
      .font(typography::mono::REGULAR)
      .size(11.0)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .into()
}

fn remove_btn<'a>(id: i64) -> Element<'a, Message> {
  button(
    text("\u{00d7}")
      .font(typography::mono::REGULAR)
      .size(13.0)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(Message::EntryRemoved(id))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::status::DANGER, 0.12)))
      }
      _ => None,
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
  .into()
}

fn note_btn<'a>(id: i64) -> Element<'a, Message> {
  button(
    text("\u{270e}")
      .font(typography::mono::REGULAR)
      .size(11.0)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(Message::EntryNoteToggled(id))
  .style(|_, status| button::Style {
    background: hover_overlay(status),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::text::secondary(),
    ..button::Style::default()
  })
  .into()
}

fn drag_handle<'a>(id: i64, is_dragging: bool) -> Element<'a, Message> {
  let handle_color = if is_dragging {
    color::accent()
  } else {
    color::text::tertiary()
  };
  mouse_area(
    container(
      text("\u{22ee}\u{22ee}")
        .font(typography::mono::REGULAR)
        .size(11.0)
        .style(move |_| text::Style {
          color: Some(handle_color),
        }),
    )
    .padding(Padding {
      top: 4.0,
      bottom: 4.0,
      left: 5.0,
      right: 5.0,
    }),
  )
  .on_press(Message::DragStarted(id))
  .into()
}

fn note_editor<'a>(note: &'a str, id: i64) -> Element<'a, Message> {
  let placeholder = t!("skills.editor.note_placeholder");
  container(
    text_input(&placeholder, note)
      .on_input(move |value| Message::EntryNoteChanged(id, value))
      .font(typography::body::REGULAR)
      .size(12.0)
      .padding(Padding::from(6.0))
      .width(Length::Fill)
      .style(crate::ui::components::text_input::style()),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 8.0,
    left: 56.0,
    right: spacing::SPACE_3,
  })
  .width(Length::Fill)
  .into()
}

fn hover_overlay(status: button::Status) -> Option<Background> {
  match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
    }
    _ => None,
  }
}

fn fmt_dur_short(secs: f64) -> String {
  crate::features::skills::format::fmt_dur_short(secs as i64)
}
