//! Neural remap call-to-action card component.

use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Space, column, container, text},
};
use pod_model::CharacterAttributes;

use super::{super::super::State, Message};
use crate::style::{
  color, spacing,
  typography::{body, mono},
};

/// Displays neural remap availability and cooldown information.
pub struct RemapCard {
  bonus_text: String,
  detail_text: String,
}

impl RemapCard {
  /// Constructs a `RemapCard` by deriving remap status from the given state.
  pub fn new(state: &State) -> Self {
    let (bonus_text, detail_text) = if let Some(attrs) = state.active_character().and_then(|c| c.attributes().as_ref())
    {
      (bonus_remap_text(attrs), detail_remap_text(attrs))
    } else {
      ("No remap data".to_string(), "Attributes not yet loaded".to_string())
    };

    Self {
      bonus_text,
      detail_text,
    }
  }

  /// Renders the remap card into an [`Element`].
  pub fn render(self) -> Element<'static, Message> {
    container(remap_card(self.bonus_text, self.detail_text))
      .padding(Padding {
        top: 14.0,
        bottom: 0.0,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}

fn bonus_remap_text(attrs: &CharacterAttributes) -> String {
  match attrs.bonus_remaps {
    0 => "No bonus remaps".to_string(),
    1 => "1 bonus available".to_string(),
    n => format!("{} bonuses available", n),
  }
}

fn detail_remap_text(attrs: &CharacterAttributes) -> String {
  match (&attrs.last_remap_date, &attrs.accrued_remap_cooldown_date) {
    (Some(last), Some(cd)) => format!(
      "Last remap {} · next available {}",
      last.get(..10).unwrap_or(last.as_str()),
      cd.get(..10).unwrap_or(cd.as_str())
    ),
    (Some(last), None) => format!("Last remap {}", last.get(..10).unwrap_or(last.as_str())),
    _ => "No remap history".to_string(),
  }
}

fn remap_card(bonus_text: String, detail_text: String) -> Element<'static, Message> {
  container(remap_text_col(bonus_text, detail_text))
    .padding(Padding {
      top: 14.0,
      bottom: 14.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA_SUBTLE)),
      border: Border {
        color: color::accent::PLASMA_MUTED,
        radius: 8.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn remap_text_col(bonus_text: String, detail_text: String) -> Element<'static, Message> {
  column([
    text("Neural remap")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Space::new().height(4.0).into(),
    text(bonus_text)
      .font(body::MEDIUM)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(2.0).into(),
    text(detail_text)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}
