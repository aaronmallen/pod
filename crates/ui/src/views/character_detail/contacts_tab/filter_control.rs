//! Contacts filter control: segmented All/Characters/Corps/Alliances selector.

use iced::{
  Background, Border, Element, Padding, Theme,
  widget::{button, container, row, text},
};

use crate::{
  style::{color, typography::body},
  views::character_detail::{ContactFilter, Message},
};

/// Builder for the contacts filter segmented control.
pub struct Component<'a> {
  filter: &'a ContactFilter,
}

impl<'a> Component<'a> {
  /// Creates a new filter control bound to the current filter state.
  pub fn new(filter: &'a ContactFilter) -> Self {
    Self {
      filter,
    }
  }

  /// Renders the filter control.
  pub fn render(self) -> Element<'a, Message> {
    segmented_control(self.filter)
  }
}

fn filter_btn_style(is_active: bool) -> button::Style {
  button::Style {
    background: if is_active {
      Some(Background::Color(color::accent::PLASMA_HIGHLIGHT))
    } else {
      None
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: if is_active {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    },
    ..button::Style::default()
  }
}

fn filter_button<'a>(opt: &ContactFilter, label: &'static str, filter: &'a ContactFilter) -> Element<'a, Message> {
  let is_active = filter == opt;
  let opt_clone = opt.clone();
  button(
    text(label.to_string())
      .font(body::MEDIUM)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        }),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: 12.0,
    right: 12.0,
  })
  .style(move |_, _| filter_btn_style(is_active))
  .on_press(Message::ContactsFilterChanged(opt_clone))
  .into()
}

fn segmented_control(filter: &ContactFilter) -> Element<'_, Message> {
  let options: &[(ContactFilter, &'static str)] = &[
    (ContactFilter::All, "All"),
    (ContactFilter::Character, "Characters"),
    (ContactFilter::Corp, "Corps"),
    (ContactFilter::Alliance, "Alliances"),
  ];
  let btns: Vec<Element<'_, Message>> = options
    .iter()
    .map(|(opt, label)| filter_button(opt, label, filter))
    .collect();
  container(row(btns).spacing(2.0).padding(2.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}
