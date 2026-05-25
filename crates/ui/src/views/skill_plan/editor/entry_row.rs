//! A single skill entry row in the plan editor.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, mouse_area, row, text, text_input},
};

use super::super::Message;
use crate::{
  plan_math::{ComputedEntry, Priority},
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::skills::{skill_data::AttrKey, training_hero::pip_row::roman},
};

/// A single skill entry row in the plan editor.
pub struct EntryRow {
  entry: ComputedEntry,
  index: usize,
  is_dragging: bool,
  is_hover_target: bool,
  note_expanded: bool,
}

impl EntryRow {
  /// Creates a new `EntryRow`.
  pub fn new(
    entry: ComputedEntry,
    index: usize,
    note_expanded: bool,
    is_dragging: bool,
    is_hover_target: bool,
  ) -> Self {
    Self {
      entry,
      index,
      is_dragging,
      is_hover_target,
      note_expanded,
    }
  }

  /// Renders the entry row into an [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    let entry = self.entry;
    let id = entry.id.clone();
    let id_for_hover = id.clone();

    let row_content = build_entry_row_content(&entry, &id, self.index, self.is_dragging);
    let drop_bar = entry_drop_bar(self.is_hover_target);
    let row_with_indicator = column([drop_bar, row_content.into()]).width(Length::Fill);
    let hoverable = mouse_area(row_with_indicator).on_enter(Message::EntryDragHover(id_for_hover));

    let mut row_items: Vec<Element<'_, Message>> = vec![hoverable.into()];

    if self.note_expanded {
      let note_text = entry.note.clone().unwrap_or_default();
      let note_id = entry.id.clone();
      row_items.push(note_expand_row(note_text, note_id));
    }

    column(row_items).width(Length::Fill).into()
  }
}

fn attr_chip_small(key: AttrKey, primary: bool) -> Element<'static, Message> {
  let bg = if primary {
    color::accent::PLASMA_HIGHLIGHT
  } else {
    color::state::HOVER_OVERLAY
  };
  let fg = if primary {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  let border_col = if primary {
    color::accent::PLASMA_BORDER
  } else {
    color::border::SUBTLE
  };

  container(
    text(key.short())
      .font(mono::REGULAR)
      .size(9.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 1.0,
    bottom: 1.0,
    left: 5.0,
    right: 5.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: border_col,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn badge_chip<'a>(label: &'static str, bg: Color, border: Color, fg: Color) -> Element<'a, Message> {
  container(
    text(label)
      .font(mono::REGULAR)
      .size(9.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 1.0,
    bottom: 1.0,
    left: 6.0,
    right: 6.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: border,
      radius: 3.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn build_entry_row_content(
  entry: &ComputedEntry,
  id: &str,
  index: usize,
  is_dragging: bool,
) -> iced::widget::Container<'static, Message> {
  let inner_row = entry_widgets_row(entry, id, index, is_dragging);
  let row_bg = entry_row_bg(is_dragging);
  container(inner_row)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: row_bg,
      ..container::Style::default()
    })
}

fn entry_drag_handle(id: String, is_dragging: bool) -> iced::widget::MouseArea<'static, Message> {
  mouse_area(
    container(
      text("\u{22ee}\u{22ee}")
        .font(mono::REGULAR)
        .size(11.0)
        .style(move |_| iced::widget::text::Style {
          color: Some(if is_dragging {
            color::accent::PLASMA
          } else {
            color::text::TERTIARY
          }),
        }),
    )
    .padding(Padding {
      top: 4.0,
      bottom: 4.0,
      left: 5.0,
      right: 5.0,
    }),
  )
  .on_press(Message::EntryDragStart(id))
}

fn entry_drop_bar(is_hover_target: bool) -> Element<'static, Message> {
  if is_hover_target {
    container(Space::new().width(Length::Fill).height(2.0))
      .width(Length::Fill)
      .height(2.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        ..container::Style::default()
      })
      .into()
  } else {
    Space::new().height(0.0).into()
  }
}

fn entry_index_col(index: usize) -> iced::widget::Container<'static, Message> {
  let label = text(format!("#{}", index + 1))
    .font(mono::REGULAR)
    .size(10.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });
  container(label)
    .width(Length::Fixed(28.0))
    .align_x(Horizontal::Right)
    .align_y(Vertical::Center)
}

fn entry_name_row(entry: &ComputedEntry) -> iced::widget::Row<'static, Message> {
  let mut name_items: Vec<Element<'_, Message>> = vec![
    text(entry.skill_name.clone())
      .font(body::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    text(roman(entry.to_level))
      .font(mono::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];

  if entry.auto {
    name_items.push(Space::new().width(spacing::SPACE_2).into());
    name_items.push(badge_chip(
      "prereq",
      color::status::ONLINE_SUBTLE,
      color::status::ONLINE_MUTED,
      color::text::SUCCESS,
    ));
  }

  if entry.skipped {
    name_items.push(Space::new().width(spacing::SPACE_2).into());
    name_items.push(badge_chip(
      "already trained",
      color::state::HOVER_OVERLAY,
      color::border::SUBTLE,
      color::text::TERTIARY,
    ));
  }

  row(name_items).align_y(Vertical::Center)
}

fn entry_remove_btn(id: String) -> button::Button<'static, Message> {
  button(
    text("×")
      .font(mono::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 7.0,
    right: 7.0,
  })
  .on_press(Message::EntryRemoved(id))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::status::DANGER_SUBTLE)),
      _ => None,
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
}

fn entry_row_bg(is_dragging: bool) -> Option<Background> {
  if is_dragging {
    Some(Background::Color(color::state::HOVER_OVERLAY))
  } else {
    None
  }
}

fn entry_skill_col(entry: &ComputedEntry) -> iced::widget::Column<'static, Message> {
  let name_row = entry_name_row(entry);
  let attr_sub = row([
    text(format!("×{}", entry.rank))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    attr_chip_small(entry.primary, true),
    Space::new().width(4.0).into(),
    attr_chip_small(entry.secondary, false),
  ])
  .align_y(Vertical::Center);

  column([name_row.into(), Space::new().height(4.0).into(), attr_sub.into()]).width(Length::Fill)
}

fn entry_sp_col(entry: &ComputedEntry) -> Element<'static, Message> {
  if entry.skipped {
    container(
      text("—")
        .font(mono::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        }),
    )
    .width(Length::Fixed(80.0))
    .align_x(Horizontal::Right)
    .align_y(Vertical::Center)
    .into()
  } else {
    container(
      column([
        text(fmt_sp(entry.sp))
          .font(mono::MEDIUM)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text("SP")
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          })
          .into(),
      ])
      .align_x(Horizontal::Right)
      .spacing(2.0),
    )
    .width(Length::Fixed(80.0))
    .align_x(Horizontal::Right)
    .align_y(Vertical::Center)
    .into()
  }
}

fn entry_time_col(entry: &ComputedEntry) -> Element<'static, Message> {
  if entry.skipped {
    Space::new().width(110.0).into()
  } else {
    container(
      column([
        text(fmt_dur_short_f64(entry.sec))
          .font(mono::MEDIUM)
          .size(13.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text(format!("cum {}", fmt_dur_short_f64(entry.cum_sec)))
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          })
          .into(),
      ])
      .align_x(Horizontal::Right)
      .spacing(2.0),
    )
    .width(Length::Fixed(110.0))
    .align_x(Horizontal::Right)
    .align_y(Vertical::Center)
    .into()
  }
}

fn entry_widgets_row(
  entry: &ComputedEntry,
  id: &str,
  index: usize,
  is_dragging: bool,
) -> iced::widget::Row<'static, Message> {
  let priority_dot = priority_dot_btn(entry.priority, id.to_string());
  let skill_col = entry_skill_col(entry);
  let sp_col = entry_sp_col(entry);
  let time_col = entry_time_col(entry);
  let id_for_note = id.to_string();
  let note_btn = small_icon_btn("✎", id_for_note.clone(), move || {
    Message::EntryNoteChanged(id_for_note.clone(), String::new())
  });
  let remove_btn = entry_remove_btn(id.to_string());
  let drag_handle = entry_drag_handle(id.to_string(), is_dragging);
  let index_col = entry_index_col(index);

  row([
    index_col.into(),
    Space::new().width(spacing::SPACE_2).into(),
    priority_dot,
    Space::new().width(spacing::SPACE_2).into(),
    skill_col.into(),
    sp_col,
    Space::new().width(spacing::SPACE_2).into(),
    time_col,
    Space::new().width(spacing::SPACE_2).into(),
    note_btn,
    Space::new().width(4.0).into(),
    drag_handle.into(),
    Space::new().width(4.0).into(),
    remove_btn.into(),
    Space::new().width(spacing::SPACE_3).into(),
  ])
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: spacing::SPACE_3,
    right: 0.0,
  })
}

fn fmt_dur_short_f64(secs: f64) -> String {
  crate::format::fmt_dur_short(secs as u64)
}

fn fmt_sp(sp: u64) -> String {
  if sp >= 1_000_000 {
    format!("{:.1}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.1}k", sp as f64 / 1_000.0)
  } else {
    format!("{}", sp)
  }
}

fn note_expand_row(note_text: String, entry_id: String) -> Element<'static, Message> {
  let id = entry_id.clone();
  container(
    text_input("Add a note…", &note_text)
      .on_input(move |v| Message::EntryNoteChanged(id.clone(), v))
      .padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 10.0,
        right: 10.0,
      })
      .size(12.0)
      .font(body::REGULAR)
      .style(|_, _| iced::widget::text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
          color: color::border::SUBTLE,
          radius: 4.0.into(),
          width: 1.0,
        },
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: color::accent::PLASMA_SUBTLE,
      }),
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

fn priority_dot_btn(priority: Priority, entry_id: String) -> Element<'static, Message> {
  let dot_color = priority_dot_color(priority);
  let next_priority = priority_next(priority);

  let dot = container(Space::new().width(8.0).height(8.0))
    .width(8.0)
    .height(8.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(dot_color)),
      border: Border {
        radius: 4.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  button(dot)
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 6.0,
      right: 6.0,
    })
    .on_press(Message::EntryPriorityChanged(entry_id, next_priority))
    .style(|_, status| button::Style {
      background: match status {
        button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
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

fn priority_dot_color(priority: Priority) -> iced::Color {
  match priority {
    Priority::Low => color::chart::P4,
    Priority::Normal => color::text::TERTIARY,
    Priority::High => color::chart::P3,
  }
}

fn priority_next(priority: Priority) -> Priority {
  match priority {
    Priority::Low => Priority::Normal,
    Priority::Normal => Priority::High,
    Priority::High => Priority::Low,
  }
}

fn small_icon_btn<'a, F>(icon: &'static str, _id: String, on_press: F) -> Element<'a, Message>
where
  F: Fn() -> Message + 'static,
{
  button(
    text(icon)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 6.0,
    right: 6.0,
  })
  .on_press(on_press())
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}
