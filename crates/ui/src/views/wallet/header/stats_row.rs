//! Stats row in the wallet header — liquid / assets / escrow / net cells.

use iced::{
  Element, Theme,
  widget::{row, text},
};

use crate::{
  components::HeadStat,
  format,
  style::{color, typography::mono},
  views::wallet::{Message, State},
};

fn separator_v<'a>() -> Element<'a, Message> {
  use iced::{
    Background,
    widget::{Space, container, container::Style},
  };

  use crate::style::color;

  container(Space::new().width(1.0).height(32.0))
    .width(1.0)
    .height(32.0)
    .style(|_| Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..Style::default()
    })
    .into()
}

/// Builder for the wallet header stats row.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new stats row component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the stats row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let liquid_el = HeadStat::new("Liquid ISK", format::fmt_isk(state.total_liquid())).render();
    let assets_el = HeadStat::new("Assets", format::fmt_isk(state.total_assets())).render();
    let escrow_el = HeadStat::new("Escrow", format::fmt_isk(state.total_escrow())).render();
    let income = state.journal_income;
    let spend = state.journal_spend;
    let net = income - spend;
    let net_color = if net >= 0.0 {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let net_str = format!("{}{}", if net >= 0.0 { "" } else { "−" }, format::fmt_isk(net.abs()));
    let net_accent: Element<'_, Message> = text(net_str)
      .font(mono::MEDIUM)
      .size(12.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(net_color),
      })
      .into();
    let net_el = HeadStat::new("30D NET", format::fmt_isk(income))
      .accent(net_accent)
      .render();

    use crate::style::spacing;

    row([
      liquid_el,
      separator_v(),
      assets_el,
      separator_v(),
      escrow_el,
      separator_v(),
      net_el,
    ])
    .spacing(spacing::SPACE_8)
    .align_y(iced::alignment::Vertical::Center)
    .width(iced::Length::Fill)
    .into()
  }
}
