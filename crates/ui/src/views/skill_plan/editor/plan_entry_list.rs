//! Plan entry list: summary strip, column headers, and scrollable entry rows.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, column, container, row, scrollable, text},
};

use super::{super::Message, EntryRow};
use crate::{
  components,
  plan_math::ComputedPlan,
  style::{color, spacing, typography::mono},
};

/// The scrollable entry list body, including summary strip and column headers.
pub struct PlanEntryList<'a> {
  computed: &'a ComputedPlan,
  drag_hover_entry_id: Option<&'a str>,
  dragging_entry_id: Option<&'a str>,
  note_expanded: Option<&'a str>,
}

impl<'a> PlanEntryList<'a> {
  /// Creates a new `PlanEntryList`.
  pub fn new(
    computed: &'a ComputedPlan,
    note_expanded: Option<&'a str>,
    dragging_entry_id: Option<&'a str>,
    drag_hover_entry_id: Option<&'a str>,
  ) -> Self {
    Self {
      computed,
      drag_hover_entry_id,
      dragging_entry_id,
      note_expanded,
    }
  }

  /// Renders the entry list into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let summary = summary_strip(
      self.computed.items.len(),
      self.computed.total_sp,
      self.computed.total_sec,
    );
    let col_hdr = col_header_row();
    let entry_list = self.build_entry_list();

    container(
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

fn col_header_label(label: &str) -> iced::widget::Text<'static> {
  text(label.to_string())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
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

fn fmt_dur(secs: u64) -> String {
  crate::format::fmt_dur(secs)
}

fn fmt_eta(secs: u64) -> String {
  crate::format::fmt_eta(secs)
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
