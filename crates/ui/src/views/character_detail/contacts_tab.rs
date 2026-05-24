//! Contacts tab: address book with type filter and standing-coloured rows.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  border::Radius,
  widget::{Space, button, column, container, row, text},
};
use pod_model::CharacterContact;

use crate::{
  components::{DataTable, LoadState},
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::{ContactFilter, LoadState as DataState, Message},
};

/// Builder for the contacts tab content.
pub struct Component<'a> {
  contacts: &'a DataState<Vec<CharacterContact>>,
  filter: &'a ContactFilter,
  filtered: &'a [CharacterContact],
}

impl<'a> Component<'a> {
  /// Creates a new contacts tab component.
  pub fn new(
    contacts: &'a DataState<Vec<CharacterContact>>,
    filtered: &'a [CharacterContact],
    filter: &'a ContactFilter,
  ) -> Self {
    Self {
      contacts,
      filter,
      filtered,
    }
  }

  /// Renders the contacts tab.
  pub fn render(self) -> Element<'a, Message> {
    match self.contacts {
      DataState::Loading => LoadState::loading("Loading contacts…").render(),
      DataState::Error(e) => LoadState::error(e).render(),
      DataState::Loaded(_) => contacts_content(self.filtered, self.filter),
    }
  }
}

fn contacts_content<'a>(contacts: &'a [CharacterContact], filter: &'a ContactFilter) -> Element<'a, Message> {
  let visible: Vec<&CharacterContact> = contacts.iter().collect();
  let eyebrow_row = row([
    section_eyebrow("Address book", format!("{} contacts", visible.len())),
    Space::new().width(Length::Fill).into(),
    segmented_control(filter),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();
  let table = DataTable::new(visible.iter().copied(), |c, i, n| contact_row(c, i == n - 1))
    .header(contacts_header_row())
    .empty_message("No contacts match your filter.")
    .render();
  let card = container(table)
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });
  iced::widget::scrollable(
    column([eyebrow_row, card.into()])
      .spacing(16.0)
      .padding(Padding {
        top: 24.0,
        bottom: 24.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
  )
  .height(Length::Fill)
  .into()
}

fn section_eyebrow(label: impl Into<String>, right: impl Into<String>) -> Element<'static, Message> {
  let left_el = text(label.into().to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });
  let right_el = text(right.into())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });
  row([left_el.into(), Space::new().width(8.0).into(), right_el.into()])
    .align_y(iced::alignment::Vertical::Center)
    .into()
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
  .style(move |_, _| button::Style {
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
  })
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

fn contacts_header_row<'a>() -> Element<'a, Message> {
  let cols: Vec<Element<'_, Message>> = vec![
    col_label("Entity", false, Length::Fill),
    col_label("Type", false, Length::Fixed(90.0)),
    col_label("Standing", true, Length::Fixed(70.0)),
    col_label("Note", false, Length::Fill),
    col_label("Watchlist", true, Length::Fixed(80.0)),
  ];
  container(row(cols).spacing(16.0).padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 16.0,
    right: 16.0,
  }))
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      radius: Radius {
        top_left: 10.0,
        top_right: 10.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
      },
    },
    ..container::Style::default()
  })
  .into()
}

fn col_label<'a>(label: &'a str, right: bool, width: Length) -> Element<'a, Message> {
  let align = if right {
    iced::alignment::Horizontal::Right
  } else {
    iced::alignment::Horizontal::Left
  };
  container(
    text(label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .width(width)
  .align_x(align)
  .into()
}

fn watchlist_badge<'a>() -> Element<'a, Message> {
  container(
    text("WATCH")
      .font(mono::REGULAR)
      .size(8.5)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 7.0,
    right: 7.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
    border: Border {
      color: color::accent::PLASMA,
      radius: 999.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn contact_standing_el<'a>(v: f64, standing_color: Color) -> Element<'a, Message> {
  let standing_label = format!("{}{:.1}", if v >= 0.0 { "+" } else { "" }, v);
  container(
    text(standing_label)
      .font(mono::MEDIUM)
      .size(14.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(standing_color),
      }),
  )
  .width(70.0)
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn contact_watch_el<'a>(is_watched: bool) -> Element<'a, Message> {
  let inner: Element<'_, Message> = if is_watched {
    watchlist_badge()
  } else {
    Space::new().width(80.0).into()
  };
  container(inner)
    .width(80.0)
    .align_x(iced::alignment::Horizontal::Right)
    .into()
}

fn contact_row_inner<'a>(contact: &'a CharacterContact) -> Element<'a, Message> {
  let v = contact.standing;
  let name_el = text(contact.name.clone())
    .font(body::REGULAR)
    .size(13.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .width(Length::Fill);
  let type_el = container(
    text(contact.contact_type.to_uppercase())
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(90.0);
  let note_el = text(contact.label_names.join(", "))
    .font(body::REGULAR)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .width(Length::Fill);
  row([
    name_el.into(),
    type_el.into(),
    contact_standing_el(v, contact_standing_color(v)),
    note_el.into(),
    contact_watch_el(contact.is_watched),
  ])
  .spacing(16.0)
  .align_y(iced::alignment::Vertical::Center)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 16.0,
    right: 16.0,
  })
  .into()
}

fn contact_row<'a>(contact: &'a CharacterContact, is_last: bool) -> Element<'a, Message> {
  container(contact_row_inner(contact))
    .width(Length::Fill)
    .style(move |_| container::Style {
      border: Border {
        color: if is_last {
          Color::TRANSPARENT
        } else {
          color::border::SUBTLE
        },
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn contact_standing_color(v: f64) -> Color {
  if v >= 5.0 {
    color::status::ONLINE
  } else if v > 0.0 {
    color::status::ONLINE_STRONG
  } else if v == 0.0 {
    color::text::SECONDARY
  } else if v > -5.0 {
    color::status::DANGER_STRONG
  } else {
    color::status::DANGER
  }
}
