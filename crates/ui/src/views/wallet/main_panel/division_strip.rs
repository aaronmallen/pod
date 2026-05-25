//! Division strip — scrollable row of corp division buttons.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{button, container, row, text},
};

use crate::{
  format,
  style::{color, typography::mono},
  views::wallet::{Message, State},
};

/// Builder for the corp division strip.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new division strip component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the division strip into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let btns: Vec<Element<'_, Message>> = (1u8..=7).map(|div| division_button(div, state)).collect();
    container(row(btns).spacing(0.0).width(Length::Fill))
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
}

fn division_label(div: u8, state: &State) -> String {
  let balance = state
    .corp_divisions
    .iter()
    .find(|(d, _)| *d == div)
    .map(|(_, bal)| *bal);
  if let Some(bal) = balance {
    format!("Div {} · {}", div, format::fmt_isk(bal))
  } else {
    format!("Division {div}")
  }
}

fn division_text_style(is_active: bool) -> impl Fn(&Theme) -> iced::widget::text::Style {
  move |_: &Theme| iced::widget::text::Style {
    color: Some(if is_active {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    }),
  }
}

fn division_button_style(is_active: bool) -> button::Style {
  button::Style {
    background: if is_active {
      Some(Background::Color(color::accent::PLASMA_SUBTLE))
    } else {
      None
    },
    border: Border {
      color: Color::TRANSPARENT,
      radius: 0.0.into(),
      width: 0.0,
    },
    text_color: if is_active {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    },
    ..button::Style::default()
  }
}

fn division_button<'a>(div: u8, state: &'a State) -> Element<'a, Message> {
  let is_active = div == state.active_division;
  let label = division_label(div, state);
  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(division_text_style(is_active)),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
  })
  .on_press(Message::DivisionSelected(div))
  .style(move |_, _| division_button_style(is_active))
  .into()
}
