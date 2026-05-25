//! Implant suggestions section — ranked list of per-attribute implant
//! upgrades and the time each would save.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::{super::Message, fmt_time_short};
use crate::{
  plan_math::ImplantSaving,
  style::{
    color, spacing,
    typography::{body, mono},
  },
};

fn ghost_button_suggest() -> Element<'static, Message> {
  button(
    text("Suggest")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 4.0,
    bottom: 4.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(Message::ImplantSuggestionsToggled)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_SUBTLE,
      _ => Color::TRANSPARENT,
    })),
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_MUTED,
        _ => color::border::SUBTLE,
      },
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .into()
}

fn implant_saving_badge_style(is_first: bool) -> (Color, Color, Color) {
  let badge_bg = if is_first {
    color::accent::PLASMA_SUBTLE
  } else {
    color::with_alpha(color::text::SECONDARY, 0.08)
  };
  let badge_border = if is_first {
    color::accent::PLASMA_MUTED
  } else {
    color::border::SUBTLE
  };
  let badge_text_color = if is_first {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  };
  (badge_bg, badge_border, badge_text_color)
}

fn implant_saving_badge(badge_bg: Color, badge_border: Color, badge_text_color: Color) -> Element<'static, Message> {
  container(
    text("+1")
      .font(mono::MEDIUM)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(badge_text_color),
      }),
  )
  .width(28.0)
  .height(20.0)
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(badge_bg)),
    border: Border {
      color: badge_border,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn implant_saving_label(attr_name: &'static str, saved_str: String) -> Element<'static, Message> {
  column([
    text(attr_name)
      .font(body::MEDIUM)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("saves {saved_str}"))
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .into()
}

fn implant_saving_row(saving: &ImplantSaving, is_first: bool) -> Element<'static, Message> {
  let (badge_bg, badge_border, badge_text_color) = implant_saving_badge_style(is_first);
  let attr_name = saving.attr.label();
  let saved_str = fmt_time_short(saving.saved_sec);

  row([
    implant_saving_badge(badge_bg, badge_border, badge_text_color),
    Space::new().width(8.0).into(),
    implant_saving_label(attr_name, saved_str),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn implant_suggestions_header_row() -> Element<'static, Message> {
  row([
    text("IMPLANT SUGGESTIONS")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .width(Length::Fill)
      .into(),
    ghost_button_suggest(),
  ])
  .align_y(Vertical::Center)
  .into()
}

/// Builder for the implant suggestions section.
pub struct ImplantSuggestionsSection<'a> {
  /// Ranked list of per-attribute implant savings.
  pub savings: &'a [ImplantSaving],
  /// Whether the suggestions list is expanded.
  pub show: bool,
}

impl<'a> ImplantSuggestionsSection<'a> {
  /// Create a new `ImplantSuggestionsSection`.
  pub fn new(show: bool, savings: &'a [ImplantSaving]) -> Self {
    Self {
      savings,
      show,
    }
  }

  /// Render the section into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    items.push(implant_suggestions_header_row());
    items.push(Space::new().height(spacing::SPACE_3).into());

    if !self.show {
      items.push(
        text("See which implant upgrades save the most plan time.")
          .font(body::REGULAR)
          .size(11.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      );
    } else if self.savings.is_empty() {
      items.push(
        text("No savings \u{2014} implants already maxed for this plan\u{2019}s mix.")
          .font(body::REGULAR)
          .size(11.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      );
    } else {
      for (i, saving) in self.savings.iter().enumerate() {
        if i > 0 {
          items.push(Space::new().height(4.0).into());
        }
        items.push(implant_saving_row(saving, i == 0));
      }
    }

    container(column(items).width(Length::Fill))
      .padding(Padding {
        top: spacing::SPACE_3,
        bottom: spacing::SPACE_4,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}
