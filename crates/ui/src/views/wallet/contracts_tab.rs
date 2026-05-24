//! Contracts table for the wallet main panel.

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Theme,
  alignment::{Horizontal, Vertical},
  widget::{Space, container, image, row, text},
};

use crate::{
  components::{DataTable, StatusBadge},
  format,
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::wallet::{ContractEntry, State, WalletCharacter, mappings, ts_label},
};

/// Messages produced by the contracts tab (reserved for future interactions).
#[derive(Clone, Debug)]
pub enum Message {}

const COL_STATUS: f32 = 130.0;
const COL_TYPE: f32 = 120.0;
const COL_COUNTERPARTY: f32 = 136.0;
const COL_LOCATION: f32 = 148.0;
const COL_PRICE: f32 = 96.0;
const COL_COLLATERAL: f32 = 96.0;
const COL_CHARACTER: f32 = 148.0;
const COL_WHEN: f32 = 84.0;
const ROW_PAD_H: f32 = spacing::SPACE_4;

fn type_label_for(kind: &str) -> String {
  match kind {
    "item_exchange" => "Item Exchange".to_string(),
    "courier" => "Courier".to_string(),
    "auction" => "Auction".to_string(),
    other => other.replace('_', " "),
  }
}

fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
  let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
  let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
  let m = l - c / 2.0;
  let (r, g, b) = if h < 60.0 {
    (c, x, 0.0)
  } else if h < 120.0 {
    (x, c, 0.0)
  } else if h < 180.0 {
    (0.0, c, x)
  } else if h < 240.0 {
    (0.0, x, c)
  } else if h < 300.0 {
    (x, 0.0, c)
  } else {
    (c, 0.0, x)
  };
  Color::from_rgb(r + m, g + m, b + m)
}

fn char_initials(name: &str) -> String {
  let words: Vec<&str> = name.split_whitespace().collect();
  match words.as_slice() {
    [] => String::new(),
    [only] => only
      .chars()
      .next()
      .map(|c| c.to_uppercase().to_string())
      .unwrap_or_default(),
    [first, .., last] => {
      let f = first
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
      let l = last
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
      format!("{f}{l}")
    }
  }
}

fn portrait_chip<'a>(name: &str, tone: u16, handle: Option<&'a image::Handle>) -> Element<'a, Message> {
  if let Some(h) = handle {
    return container(
      image::Image::new(h.clone())
        .width(18.0)
        .height(18.0)
        .content_fit(ContentFit::Cover),
    )
    .width(18.0)
    .height(18.0)
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        radius: 4.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into();
  }
  let hue = tone as f32;
  let l = 0.25 + (hue / 360.0) * 0.15;
  let bg = hsl_to_color(hue, 0.35, l);
  let initials = char_initials(name);
  container(
    text(initials)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::MEDIUM),
      }),
  )
  .width(18.0)
  .height(18.0)
  .center_x(18.0)
  .center_y(18.0)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn header_cell(label: &str, width: impl Into<Length>, align_right: bool) -> Element<'static, Message> {
  let t = text(label.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });
  let inner: Element<'static, Message> = if align_right {
    container(t).width(Length::Fill).align_x(Horizontal::Right).into()
  } else {
    t.into()
  };
  container(inner)
    .width(width)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: ROW_PAD_H,
      right: ROW_PAD_H,
    })
    .into()
}

fn header_row() -> Element<'static, Message> {
  let inner = row([
    header_cell("Status", COL_STATUS, false),
    header_cell("Type", COL_TYPE, false),
    header_cell("Title", Length::Fill, false),
    header_cell("Counterparty", COL_COUNTERPARTY, false),
    header_cell("Route / Loc", COL_LOCATION, false),
    header_cell("Price", COL_PRICE, true),
    header_cell("Collateral", COL_COLLATERAL, true),
    header_cell("Character", COL_CHARACTER, false),
    header_cell("When", COL_WHEN, true),
  ]);
  container(inner)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn status_cell(entry: &ContractEntry) -> Element<'_, Message> {
  let badge = StatusBadge::new(
    mappings::status_color_for(&entry.status),
    mappings::status_label_for(&entry.status),
  );
  container(badge.render())
    .width(COL_STATUS)
    .height(Length::Fill)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: ROW_PAD_H,
      right: ROW_PAD_H,
    })
    .into()
}

fn type_cell(kind: &str) -> Element<'_, Message> {
  container(
    text(type_label_for(kind).to_uppercase())
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(COL_TYPE)
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: ROW_PAD_H,
    right: ROW_PAD_H,
  })
  .into()
}

fn title_cell(title: &str) -> Element<'_, Message> {
  container(
    text(title)
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .clip(true)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: ROW_PAD_H,
    right: ROW_PAD_H,
  })
  .into()
}

fn counterparty_cell(cp: &str) -> Element<'_, Message> {
  container(
    text(cp)
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::STRONG),
      }),
  )
  .width(COL_COUNTERPARTY)
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .clip(true)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: ROW_PAD_H,
    right: ROW_PAD_H,
  })
  .into()
}

fn location_cell(loc: &str) -> Element<'_, Message> {
  container(
    text(loc)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(COL_LOCATION)
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .clip(true)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: ROW_PAD_H,
    right: ROW_PAD_H,
  })
  .into()
}

fn price_cell(price: f64) -> Element<'static, Message> {
  container(
    text(format::fmt_isk(price))
      .font(mono::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(COL_PRICE)
  .height(Length::Fill)
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: ROW_PAD_H,
    right: ROW_PAD_H,
  })
  .into()
}

fn collateral_cell(collateral: f64) -> Element<'static, Message> {
  let (label, c) = if collateral > 0.0 {
    (format::fmt_isk(collateral), color::text::WARNING)
  } else {
    ("\u{2014}".to_string(), color::text::TERTIARY)
  };
  container(
    text(label)
      .font(mono::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(c),
      }),
  )
  .width(COL_COLLATERAL)
  .height(Length::Fill)
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: ROW_PAD_H,
    right: ROW_PAD_H,
  })
  .into()
}

fn character_cell<'a>(char_info: Option<&'a WalletCharacter>) -> Element<'a, Message> {
  let inner: Element<'_, Message> = match char_info {
    Some(c) => row([
      portrait_chip(&c.name, c.portrait_tone, c.portrait_handle.as_ref()),
      Space::new().width(8.0).into(),
      text(&c.name)
        .font(body::REGULAR)
        .size(12.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .align_y(Vertical::Center)
    .into(),
    None => Space::new().width(Length::Fill).into(),
  };
  container(inner)
    .width(COL_CHARACTER)
    .height(Length::Fill)
    .align_y(Vertical::Center)
    .clip(true)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: ROW_PAD_H,
      right: ROW_PAD_H,
    })
    .into()
}

fn when_cell(ts_secs: u64) -> Element<'static, Message> {
  container(
    text(ts_label(ts_secs))
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(COL_WHEN)
  .height(Length::Fill)
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: ROW_PAD_H,
    right: ROW_PAD_H,
  })
  .into()
}

fn entry_row<'a>(entry: &'a ContractEntry, characters: &'a [WalletCharacter]) -> Element<'a, Message> {
  let char_info = characters.iter().find(|c| c.id == entry.who);
  let inner = row([
    status_cell(entry),
    type_cell(&entry.kind),
    title_cell(&entry.title),
    counterparty_cell(&entry.counterparty),
    location_cell(&entry.location),
    price_cell(entry.price),
    collateral_cell(entry.collateral),
    character_cell(char_info),
    when_cell(entry.ts_secs),
  ])
  .height(52.0)
  .align_y(Vertical::Center);

  container(inner)
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

/// Builder for the contracts table.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new contracts table component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the contracts table into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let chars = &self.state.characters;
    DataTable::new(self.state.filtered_contracts.iter(), |e, _, _| entry_row(e, chars))
      .header(header_row())
      .empty_message("No contracts match your filter.")
      .render()
  }
}
