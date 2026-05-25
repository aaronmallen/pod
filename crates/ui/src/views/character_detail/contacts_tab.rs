//! Contacts tab: address book with type filter and standing-coloured rows.

pub mod contact_row;
pub mod filter_control;

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  border::Radius,
  widget::{Space, column, container, row, text},
};
use pod_model::CharacterContact;

use crate::{
  components::{DataTable, LoadState},
  style::{color, typography::mono},
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
    filter_control::Component::new(filter).render(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into();
  let table = DataTable::new(visible.iter().copied(), |c, i, n| {
    contact_row::Component::new(c, i == n - 1).render()
  })
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
