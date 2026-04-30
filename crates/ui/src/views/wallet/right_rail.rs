//! Summary stats panel on the right side of the wallet view.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, scrollable, text},
};

use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::{Message, State, journal_type_glyph},
};

fn section_label(title: &'static str) -> Element<'static, Message> {
  container(
    text(title)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 20.0,
    bottom: 12.0,
    left: 20.0,
    right: 20.0,
  })
  .width(Length::Fill)
  .into()
}

fn divider() -> Element<'static, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn summary_stat_row(label: &'static str, value: String, value_color: Color) -> Element<'static, Message> {
  container(
    row([
      text(label.to_uppercase())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .width(Length::Fill)
        .into(),
      text(value)
        .font(mono::MEDIUM)
        .size(10.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(value_color),
        })
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 20.0,
    right: 20.0,
  })
  .width(Length::Fill)
  .into()
}

fn recent_activity_rows<'a>(state: &'a State) -> Vec<Element<'a, Message>> {
  state
    .filtered_journal
    .iter()
    .take(8)
    .map(|j| {
      let (_, is_in) = journal_type_glyph(&j.entry_type);
      let delta_color = if is_in {
        color::status::ONLINE
      } else {
        color::status::DANGER
      };
      let delta_str = format!("{}{}", if is_in { "+" } else { "−" }, format::fmt_isk(j.delta.abs()));
      container(
        row([
          text(&j.reference)
            .font(body::REGULAR)
            .size(11.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::SECONDARY),
            })
            .width(Length::Fill)
            .into(),
          text(delta_str)
            .font(mono::REGULAR)
            .size(10.0)
            .style(move |_: &Theme| iced::widget::text::Style {
              color: Some(delta_color),
            })
            .into(),
        ])
        .spacing(8.0)
        .align_y(iced::alignment::Vertical::Center),
      )
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 20.0,
        right: 20.0,
      })
      .width(Length::Fill)
      .into()
    })
    .collect()
}

/// Builder for the wallet right rail.
pub struct Component<'a> {
  state: &'a State,
  width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new right rail component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      width: 220.0,
    }
  }

  /// Sets the panel width.
  pub fn width(mut self, w: f32) -> Self {
    self.width = w;
    self
  }

  /// Renders the right rail into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let width = self.width;
    let state = self.state;
    let income = state.journal_income;
    let spend = state.journal_spend;
    let net = income - spend;
    let net_str = format!("{}{}", if net >= 0.0 { "+" } else { "−" }, format::fmt_isk(net.abs()));
    let net_color = if net >= 0.0 {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let content = scrollable(
      column([
        section_label("30-Day Summary"),
        summary_stat_row("Income", format::fmt_isk(income), color::status::ONLINE),
        summary_stat_row("Spend", format!("−{}", format::fmt_isk(spend)), color::status::DANGER),
        summary_stat_row("Net", net_str, net_color),
        divider(),
        section_label("Recent Activity"),
        column(recent_activity_rows(state)).width(Length::Fill).into(),
        Space::new().height(Length::Fill).into(),
      ])
      .width(Length::Fill),
    )
    .height(Length::Fill);
    container(content)
      .width(Length::Fixed(width))
      .height(Length::Fill)
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
}
