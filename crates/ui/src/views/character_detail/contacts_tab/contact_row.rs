//! Contact row component: name, type, standing, note, and watchlist badge.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, container, row, text},
};
use pod_model::CharacterContact;

use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::Message,
};

/// Builder for a single contact row.
pub struct Component<'a> {
  contact: &'a CharacterContact,
  is_last: bool,
}

impl<'a> Component<'a> {
  /// Creates a new contact row for the given contact.
  pub fn new(contact: &'a CharacterContact, is_last: bool) -> Self {
    Self {
      contact,
      is_last,
    }
  }

  /// Renders the contact row.
  pub fn render(self) -> Element<'a, Message> {
    contact_row(self.contact, self.is_last)
  }
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
