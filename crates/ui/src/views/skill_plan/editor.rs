//! Plan editor: header bar, entry rows, and center-column layout.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, mouse_area, row, scrollable, stack, text, text_input},
};

use super::Message;
use crate::{
  components,
  plan_math::{ComputedEntry, ComputedPlan, Priority},
  style::{
    color, radius, spacing,
    typography::{body, mono},
  },
  views::skills::{skill_data::AttrKey, training_hero::pip_row::roman},
};

pub struct EditorHeader<'a> {
  dirty: bool,
  picker_open: bool,
  plan_name: &'a str,
}

impl<'a> EditorHeader<'a> {
  pub fn new(plan_name: &'a str, dirty: bool, picker_open: bool) -> Self {
    Self {
      dirty,
      picker_open,
      plan_name,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let close_btn = header_close_btn();
    let name_input = header_name_input(self.plan_name);
    let dirty_dot = header_dirty_dot(self.dirty);
    let import_trigger = import_trigger_btn();
    let export_trigger = export_trigger_btn();
    let picker_label = if self.picker_open { "Hide picker" } else { "Add skills" };
    let picker_btn =
      components::Button::ghost(text(picker_label).font(body::REGULAR).size(13.0)).on_press(Message::PickerToggled);
    let save_btn = header_save_btn(self.dirty);

    let header_row = row([
      close_btn.into(),
      Space::new().width(spacing::SPACE_2).into(),
      name_input.into(),
      dirty_dot,
      Space::new().width(Length::Fill).into(),
      import_trigger.into(),
      Space::new().width(spacing::SPACE_2).into(),
      export_trigger.into(),
      Space::new().width(spacing::SPACE_2).into(),
      picker_btn.into(),
      Space::new().width(spacing::SPACE_2).into(),
      save_btn.into(),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    });

    container(column([
      container(header_row)
        .height(52.0)
        .width(Length::Fill)
        .align_y(Vertical::Center)
        .into(),
      components::Separator::horizontal().render(),
    ]))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      ..container::Style::default()
    })
    .into()
  }
}

pub struct EntryRow {
  entry: ComputedEntry,
  index: usize,
  note_expanded: bool,
  is_dragging: bool,
  is_hover_target: bool,
}

impl EntryRow {
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
      note_expanded,
      is_dragging,
      is_hover_target,
    }
  }

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

pub struct PlanEditor<'a> {
  plan_name: &'a str,
  dirty: bool,
  picker_open: bool,
  import_dropdown_open: bool,
  export_dropdown_open: bool,
  computed: &'a ComputedPlan,
  note_expanded: Option<&'a str>,
  dragging_entry_id: Option<&'a str>,
  drag_hover_entry_id: Option<&'a str>,
}

impl<'a> PlanEditor<'a> {
  pub fn new(
    plan_name: &'a str,
    dirty: bool,
    picker_open: bool,
    import_dropdown_open: bool,
    export_dropdown_open: bool,
    computed: &'a ComputedPlan,
    note_expanded: Option<&'a str>,
    dragging_entry_id: Option<&'a str>,
    drag_hover_entry_id: Option<&'a str>,
  ) -> Self {
    Self {
      plan_name,
      dirty,
      picker_open,
      import_dropdown_open,
      export_dropdown_open,
      computed,
      note_expanded,
      dragging_entry_id,
      drag_hover_entry_id,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let header = EditorHeader::new(self.plan_name, self.dirty, self.picker_open).render();
    let import_open = self.import_dropdown_open;
    let export_open = self.export_dropdown_open;
    let body = self.render_body();
    let col = column([header, body]).height(Length::Fill).width(Length::Fill);

    if import_open {
      let overlay = import_dropdown_overlay();
      stack([col.into(), overlay.into()]).into()
    } else if export_open {
      let overlay = export_dropdown_overlay();
      stack([col.into(), overlay.into()]).into()
    } else {
      col.into()
    }
  }

  fn render_body(self) -> Element<'a, Message> {
    if self.computed.items.is_empty() {
      return empty_state();
    }

    let summary = summary_strip(
      self.computed.items.len(),
      self.computed.total_sp,
      self.computed.total_sec,
    );
    let col_hdr = col_header_row();
    let entry_list = self.build_entry_list();

    let inner = container(
      column([summary, col_hdr, entry_list])
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

    container(inner)
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_4,
        bottom: spacing::SPACE_4,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .into()
  }

  fn build_entry_list(self) -> Element<'a, Message> {
    let mut rows: Vec<Element<'_, Message>> = self
      .computed
      .items
      .iter()
      .enumerate()
      .map(|(i, entry)| {
        let note_open = self.note_expanded.map(|id| id == entry.id).unwrap_or(false);
        let is_dragging = self.dragging_entry_id == Some(entry.id.as_str());
        let is_hover_target = self.drag_hover_entry_id == Some(entry.id.as_str())
          && self.dragging_entry_id.is_some()
          && self.dragging_entry_id != Some(entry.id.as_str());
        let row_el = EntryRow::new(entry.clone(), i, note_open, is_dragging, is_hover_target).render();
        let sep = components::Separator::horizontal().render();
        column([sep, row_el]).into()
      })
      .collect();

    rows.push(components::Separator::horizontal().render());

    scrollable(column(rows).width(Length::Fill))
      .height(Length::Fill)
      .width(Length::Fill)
      .into()
  }
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

fn summary_strip<'a>(steps: usize, total_sp: u64, total_sec: f64) -> Element<'a, Message> {
  let strip = container(
    row([
      summary_cell("Steps", &steps.to_string()),
      Space::new().width(spacing::SPACE_6).into(),
      summary_cell("Total SP", &fmt_sp(total_sp)),
      Space::new().width(spacing::SPACE_6).into(),
      summary_cell("Training time", &fmt_dur(total_sec as u64)),
      Space::new().width(spacing::SPACE_6).into(),
      summary_cell("Completes", &fmt_eta(total_sec as u64)),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  });

  column([strip.into(), components::Separator::horizontal().render()]).into()
}

fn summary_cell<'a>(label: &str, value: &str) -> Element<'a, Message> {
  column([
    text(label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(value.to_string())
      .font(mono::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .into()
}

fn col_header_row<'a>() -> Element<'a, Message> {
  let hdr = container(
    row([
      Space::new().width(32.0).into(),
      Space::new().width(spacing::SPACE_2).into(),
      Space::new().width(spacing::SPACE_3).into(),
      Space::new().width(spacing::SPACE_2).into(),
      col_header_label("Skill").width(Length::Fill).into(),
      container(col_header_label("SP"))
        .width(Length::Fixed(80.0))
        .align_x(Horizontal::Right)
        .into(),
      Space::new().width(spacing::SPACE_2).into(),
      container(col_header_label("Time / Cumul."))
        .width(Length::Fixed(110.0))
        .align_x(Horizontal::Right)
        .into(),
      Space::new().width(80.0).into(),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: spacing::SPACE_3,
      right: 0.0,
    }),
  )
  .width(Length::Fill);

  column([hdr.into(), components::Separator::horizontal().render()]).into()
}

fn col_header_label(label: &str) -> iced::widget::Text<'static> {
  text(label.to_string())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
}

fn empty_state<'a>() -> Element<'a, Message> {
  let inner = empty_state_card();
  container(inner)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: spacing::SPACE_4,
      bottom: spacing::SPACE_4,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    })
    .into()
}

fn empty_state_card<'a>() -> iced::widget::Container<'a, Message> {
  container(
    column([
      text("No skills in this plan yet")
        .font(body::MEDIUM)
        .size(16.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(6.0).into(),
      text("Add your first skill using the skill picker on the left.")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().height(spacing::SPACE_4).into(),
      components::Button::ghost(text("Open skill picker").font(body::REGULAR).size(13.0))
        .on_press(Message::PickerToggled)
        .into(),
    ])
    .align_x(Horizontal::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_8,
    bottom: spacing::SPACE_8,
    left: spacing::SPACE_7,
    right: spacing::SPACE_7,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .width(Length::Fill)
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
        button::Status::Hovered | button::Status::Pressed => {
          Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05)))
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

fn priority_dot_color(priority: Priority) -> Color {
  match priority {
    Priority::Low => Color {
      r: 0.498,
      g: 0.710,
      b: 0.353,
      a: 1.0,
    },
    Priority::Normal => Color {
      r: 0.957,
      g: 0.949,
      b: 0.925,
      a: 0.35,
    },
    Priority::High => Color {
      r: 0.843,
      g: 0.459,
      b: 0.349,
      a: 1.0,
    },
  }
}

fn priority_next(priority: Priority) -> Priority {
  match priority {
    Priority::Low => Priority::Normal,
    Priority::Normal => Priority::High,
    Priority::High => Priority::Low,
  }
}

fn attr_chip_small(key: AttrKey, primary: bool) -> Element<'static, Message> {
  let bg = if primary {
    Color::from_rgba(0.247, 0.722, 0.859, 0.12)
  } else {
    Color::from_rgba(0.957, 0.949, 0.925, 0.05)
  };
  let fg = if primary {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  let border_col = if primary {
    Color::from_rgba(0.247, 0.722, 0.859, 0.35)
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
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05)))
      }
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

fn fmt_sp(sp: u64) -> String {
  if sp >= 1_000_000 {
    format!("{:.1}M", sp as f64 / 1_000_000.0)
  } else if sp >= 1_000 {
    format!("{:.1}k", sp as f64 / 1_000.0)
  } else {
    format!("{}", sp)
  }
}

fn fmt_dur_short_f64(secs: f64) -> String {
  crate::format::fmt_dur_short(secs as u64)
}

fn fmt_eta(secs: u64) -> String {
  crate::format::fmt_eta(secs)
}

fn fmt_dur(secs: u64) -> String {
  crate::format::fmt_dur(secs)
}

fn header_close_btn() -> button::Button<'static, Message> {
  button(
    text("←")
      .font(mono::REGULAR)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::CloseRequested)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05)))
      }
      _ => None,
    },
    border: Border {
      radius: radius::CHIP.into(),
      ..Border::default()
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
}

fn header_name_input(plan_name: &str) -> iced::widget::TextInput<'_, Message> {
  text_input("Untitled plan", plan_name)
    .on_input(Message::NameChanged)
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 8.0,
      right: 8.0,
    })
    .size(15.0)
    .font(body::MEDIUM)
    .style(|_, _| iced::widget::text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border {
        color: Color::TRANSPARENT,
        radius: 4.0.into(),
        width: 0.0,
      },
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::accent::PLASMA_SUBTLE,
    })
}

fn header_dirty_dot(dirty: bool) -> Element<'static, Message> {
  if dirty {
    container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(|_| container::Style {
        background: Some(Background::Color(Color {
          r: 0.851,
          g: 0.698,
          b: 0.322,
          a: 1.0,
        })),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  } else {
    Space::new().width(0.0).height(0.0).into()
  }
}

fn header_save_btn(dirty: bool) -> button::Button<'static, Message> {
  if dirty {
    components::Button::primary(text("Save").font(body::MEDIUM).size(13.0)).on_press(Message::SaveRequested)
  } else {
    components::Button::primary(
      text("Save")
        .font(body::MEDIUM)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        }),
    )
  }
}

fn dropdown_trigger_btn(label: &'static str, on_press: Message) -> button::Button<'static, Message> {
  button(
    row([
      text(label).font(body::REGULAR).size(13.0).into(),
      container(Space::new().width(1.0).height(14.0))
        .width(1.0)
        .height(14.0)
        .style(|_| container::Style {
          background: Some(Background::Color(color::border::SUBTLE)),
          ..container::Style::default()
        })
        .into(),
      text("\u{25be}").font(body::REGULAR).size(13.0).into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: None,
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::border::DEFAULT,
        _ => color::border::SUBTLE,
      },
      radius: 8.0.into(),
      width: 1.0,
    },
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
}

fn import_trigger_btn() -> button::Button<'static, Message> {
  dropdown_trigger_btn("Import", Message::ImportDropdownToggled)
}

fn export_trigger_btn() -> button::Button<'static, Message> {
  dropdown_trigger_btn("Export", Message::ExportDropdownToggled)
}

fn dropdown_menu_btn(label: &'static str, on_press: Message) -> button::Button<'static, Message> {
  button(text(label).font(body::REGULAR).size(13.0))
    .width(Length::Fill)
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 14.0,
      right: 14.0,
    })
    .on_press(on_press)
    .style(|_, status| button::Style {
      background: match status {
        button::Status::Hovered | button::Status::Pressed => Some(Background::Color(Color {
          r: 0.957,
          g: 0.949,
          b: 0.925,
          a: 0.06,
        })),
        _ => None,
      },
      border: Border::default(),
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
}

fn dropdown_panel<'a>(items: Vec<Element<'a, Message>>) -> iced::widget::Container<'a, Message> {
  container(column(items).width(Length::Fixed(180.0))).style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
}

fn import_dropdown_overlay<'a>() -> iced::widget::Container<'a, Message> {
  let from_clipboard_btn = dropdown_menu_btn("From clipboard", Message::ImportFromClipboard);
  let from_file_btn = dropdown_menu_btn("From file\u{2026}", Message::ImportFromFile);
  let dropdown = dropdown_panel(vec![from_clipboard_btn.into(), from_file_btn.into()]);
  container(dropdown)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 52.0,
      right: spacing::SPACE_4 + 180.0 + spacing::SPACE_2 * 3.0 + 70.0,
      ..Padding::ZERO
    })
}

fn export_dropdown_overlay<'a>() -> iced::widget::Container<'a, Message> {
  let to_clipboard_btn = dropdown_menu_btn("To clipboard", Message::ExportToClipboard);
  let to_file_btn = dropdown_menu_btn("To file\u{2026}", Message::ExportToFile);
  let dropdown = dropdown_panel(vec![to_clipboard_btn.into(), to_file_btn.into()]);
  container(dropdown)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 52.0,
      right: spacing::SPACE_4 + 70.0,
      ..Padding::ZERO
    })
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
      Color::from_rgba(0.357, 0.725, 0.494, 0.15),
      Color::from_rgba(0.357, 0.725, 0.494, 0.35),
      color::text::SUCCESS,
    ));
  }

  if entry.skipped {
    name_items.push(Space::new().width(spacing::SPACE_2).into());
    name_items.push(badge_chip(
      "already trained",
      Color::from_rgba(0.957, 0.949, 0.925, 0.05),
      color::border::SUBTLE,
      color::text::TERTIARY,
    ));
  }

  row(name_items).align_y(Vertical::Center)
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
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(Color::from_rgba(0.878, 0.459, 0.349, 0.12)))
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

fn entry_row_bg(is_dragging: bool) -> Option<Background> {
  if is_dragging {
    Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.04)))
  } else {
    None
  }
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
