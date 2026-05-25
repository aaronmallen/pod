use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{button, column, container, row, text},
};

use super::{CharacterEntry, CorporationEntry, Message, PickerSelection, portrait};
use crate::style::{color, spacing, typography as font};

pub struct CharacterPickerEntry<'a> {
  entry: &'a CharacterEntry,
  selected: bool,
}

impl<'a> CharacterPickerEntry<'a> {
  pub fn new(entry: &'a CharacterEntry, selected: bool) -> Self {
    Self {
      entry,
      selected,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let swatch = portrait::portrait_swatch(
      &self.entry.name,
      self.entry.tone,
      30.0,
      6.0,
      self.entry.portrait_handle.clone(),
    );

    let label_col: Element<'static, Message> = column([
      text(self.entry.name.clone())
        .font(font::body::MEDIUM)
        .size(14.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(self.entry.corp_name.to_uppercase())
        .font(font::mono::REGULAR)
        .size(10.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(2.0)
    .width(Length::Fill)
    .into();

    let inner: Element<'static, Message> = row([swatch, label_col])
      .spacing(spacing::SPACE_3)
      .align_y(iced::alignment::Vertical::Center)
      .width(Length::Fill)
      .into();

    let id = self.entry.id.unwrap_or(0);
    let selected = self.selected;
    button(inner)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: if selected { 12.0 } else { 14.0 },
        right: 14.0,
      })
      .width(Length::Fill)
      .style(picker_row_style(selected))
      .on_press(Message::Select(PickerSelection::Character(id)))
      .into()
  }
}

pub struct CorporationPickerEntry<'a> {
  entry: &'a CorporationEntry,
  selected: bool,
}

impl<'a> CorporationPickerEntry<'a> {
  pub fn new(entry: &'a CorporationEntry, selected: bool) -> Self {
    Self {
      entry,
      selected,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let swatch = portrait::portrait_swatch(&self.entry.ticker, 220, 30.0, 6.0, self.entry.icon_handle.clone());

    let label_col: Element<'static, Message> = column([
      text(self.entry.name.clone())
        .font(font::body::MEDIUM)
        .size(14.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(self.entry.ticker.to_uppercase())
        .font(font::mono::REGULAR)
        .size(10.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .spacing(2.0)
    .width(Length::Fill)
    .into();

    let inner: Element<'static, Message> = row([swatch, label_col])
      .spacing(spacing::SPACE_3)
      .align_y(iced::alignment::Vertical::Center)
      .width(Length::Fill)
      .into();

    let id = self.entry.id;
    let selected = self.selected;
    button(inner)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: if selected { 12.0 } else { 14.0 },
        right: 14.0,
      })
      .width(Length::Fill)
      .style(picker_row_style(selected))
      .on_press(Message::Select(PickerSelection::Corporation(id)))
      .into()
  }
}

pub struct AllPickerEntry {
  label: String,
  selected: bool,
}

impl AllPickerEntry {
  pub fn new(label: impl Into<String>, selected: bool) -> Self {
    Self {
      label: label.into(),
      selected,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let label: Element<'_, Message> = text(self.label)
      .font(font::body::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into();

    let inner: Element<'_, Message> = row([all_wallets_swatch(), label])
      .spacing(spacing::SPACE_3)
      .align_y(iced::alignment::Vertical::Center)
      .width(Length::Fill)
      .into();

    let selected = self.selected;
    button(inner)
      .padding(Padding {
        top: 10.0,
        bottom: 10.0,
        left: if selected { 12.0 } else { 14.0 },
        right: 14.0,
      })
      .width(Length::Fill)
      .style(picker_row_style(selected))
      .on_press(Message::Select(PickerSelection::All))
      .into()
  }
}

pub fn all_wallets_swatch() -> Element<'static, Message> {
  container(
    text("∑")
      .font(font::mono::REGULAR)
      .size(16.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fixed(30.0))
  .height(Length::Fixed(30.0))
  .style(|_| container::Style {
    background: Some(Background::Color(color::state::SUBTLE_FILL)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

pub fn picker_row_style(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let bg = match (selected, status) {
      (true, _) => Some(color::accent::PLASMA_SELECTED),
      (false, button::Status::Hovered | button::Status::Pressed) => Some(color::state::HOVER_OVERLAY),
      _ => None,
    };
    button::Style {
      background: bg.map(Background::Color),
      border: Border {
        color: if selected {
          color::accent::PLASMA
        } else {
          Color::TRANSPARENT
        },
        radius: 0.0.into(),
        width: if selected { 2.0 } else { 0.0 },
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  }
}
