//! Stockpile card component: card with header, fill bar, and item rows.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, text},
};

use super::{super::StockpileWithStatus, pile_item_row};
use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::assets::stockpiles_tab::Message,
};

fn status_dot_color(pile: &StockpileWithStatus) -> Color {
  if pile.ready {
    color::text::SUCCESS
  } else if pile.overall_pct >= 0.6 {
    color::text::WARNING
  } else {
    color::text::DANGER
  }
}

fn border_color(pile: &StockpileWithStatus) -> Color {
  if pile.ready {
    color::status::ONLINE_MUTED
  } else {
    color::border::DEFAULT
  }
}

fn fill_bar(pct: f32, bar_fill_color: Color) -> Element<'static, Message> {
  container(
    container(Space::new())
      .width(Length::FillPortion((pct * 1000.0) as u16))
      .height(4.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(bar_fill_color)),
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(4.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::border::SUBTLE)),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn action_btn(label: &str, msg: Message, danger: bool) -> Element<'_, Message> {
  let text_color = if danger {
    color::text::DANGER
  } else {
    color::text::SECONDARY
  };
  let border_clr = if danger {
    color::text::DANGER
  } else {
    color::border::DEFAULT
  };

  button(
    text(label.to_string())
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .on_press(msg)
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 6.0,
    right: 6.0,
  })
  .style(move |_, _| button::Style {
    background: None,
    border: Border {
      color: border_clr,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn title_row(pile: &StockpileWithStatus, dot_color: Color, pct: f32, pct_color: Color) -> Element<'_, Message> {
  row([
    container(Space::new().width(8.0).height(8.0))
      .width(8.0)
      .height(8.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(dot_color)),
        border: Border {
          radius: 4.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(10.0).into(),
    text(pile.name.clone())
      .font(body::MEDIUM)
      .size(14.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(format!("{}%", (pct * 100.0).round() as u32))
      .font(mono::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(pct_color),
      })
      .into(),
    Space::new().width(8.0).into(),
    action_btn("Edit", Message::EditStockpile(pile.id), false),
    Space::new().width(4.0).into(),
    action_btn("Delete", Message::DeleteStockpile(pile.id), true),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .spacing(0.0)
  .into()
}

fn header(pile: &StockpileWithStatus) -> Element<'_, Message> {
  let dot_color = status_dot_color(pile);
  let pct_color = if pile.ready {
    color::text::SUCCESS
  } else {
    color::text::WARNING
  };
  let pct = pile.overall_pct.clamp(0.0, 1.0);
  let bar_fill_color = if pile.ready {
    color::text::SUCCESS
  } else {
    color::accent::PLASMA
  };

  let location_label = pile.location_name.as_deref().unwrap_or("All locations").to_string();

  column([
    title_row(pile, dot_color, pct, pct_color),
    Space::new().height(4.0).into(),
    text(location_label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(10.0).into(),
    fill_bar(pct, bar_fill_color),
  ])
  .spacing(0.0)
  .into()
}

/// Builder for a stockpile card.
pub struct Component<'a> {
  pile: &'a StockpileWithStatus,
}

impl<'a> Component<'a> {
  /// Creates a new stockpile card for the given pile.
  pub fn new(pile: &'a StockpileWithStatus) -> Self {
    Self {
      pile,
    }
  }

  /// Renders the stockpile card into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let pile = self.pile;
    let border_clr = border_color(pile);
    let hdr = header(pile);
    let item_rows: Vec<Element<'_, Message>> = pile
      .items
      .iter()
      .map(|item| pile_item_row::Component::new(item).render())
      .collect();

    let header_section = container(hdr)
      .width(Length::Fill)
      .padding(Padding {
        top: 14.0,
        bottom: 12.0,
        left: 18.0,
        right: 18.0,
      })
      .into();

    let items_section = container(column(item_rows).width(Length::Fill))
      .width(Length::Fill)
      .style(|_| container::Style {
        border: Border {
          color: color::border::SUBTLE,
          width: 1.0,
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into();

    container(column([header_section, items_section]).width(Length::Fill))
      .width(Length::Fill)
      .style(move |_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: border_clr,
          radius: 10.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }
}
