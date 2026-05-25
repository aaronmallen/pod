//! Training-rate grid component for the attributes tab.

use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Space, column, container, row, text},
};

use super::{
  super::super::{State, fmt_sp, skill_data::AttrKey},
  Message,
};
use crate::{
  format::sp_per_sec,
  style::{color, spacing, typography::mono},
};

/// Displays a grid of SP/hr training rates grouped by attribute pair and
/// skill category.
pub struct RateGrid<'a> {
  /// The active primary attribute for the current skill, if any.
  active_primary: Option<AttrKey>,
  /// The active secondary attribute for the current skill, if any.
  active_secondary: Option<AttrKey>,
  /// Application state providing attribute values.
  state: &'a State,
}

impl<'a> RateGrid<'a> {
  /// Creates a new [`RateGrid`] for the given state and active attribute pair.
  pub fn new(state: &'a State, active_primary: Option<AttrKey>, active_secondary: Option<AttrKey>) -> Self {
    Self {
      active_primary,
      active_secondary,
      state,
    }
  }

  /// Renders the rate grid into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let left = rate_pairs_col(
      &[
        (AttrKey::Perception, AttrKey::Willpower, "Combat"),
        (AttrKey::Memory, AttrKey::Perception, "Drones"),
        (AttrKey::Willpower, AttrKey::Charisma, "Trade"),
      ],
      self.state,
      self.active_primary,
      self.active_secondary,
    );
    let right = rate_pairs_col(
      &[
        (AttrKey::Intelligence, AttrKey::Memory, "Engineering"),
        (AttrKey::Intelligence, AttrKey::Perception, "Navigation"),
        (AttrKey::Charisma, AttrKey::Willpower, "Social"),
      ],
      self.state,
      self.active_primary,
      self.active_secondary,
    );
    container(
      container(column([
        text("Training rate by attribute pair")
          .font(mono::REGULAR)
          .size(9.0)
          .style(|_| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().height(10.0).into(),
        row([left, Space::new().width(8.0).into(), right]).into(),
      ]))
      .padding(Padding::new(14.0))
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::border::SUBTLE,
          radius: 8.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      }),
    )
    .padding(Padding {
      top: spacing::SPACE_4,
      bottom: 0.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
  }
}

fn rate_pairs_col<'a>(
  pairs: &[(AttrKey, AttrKey, &'static str)],
  state: &'a State,
  active_primary: Option<AttrKey>,
  active_secondary: Option<AttrKey>,
) -> Element<'a, Message> {
  let cells: Vec<Element<'_, Message>> = pairs
    .iter()
    .flat_map(|(p, s, label)| {
      let is_active = active_primary == Some(*p) && active_secondary == Some(*s);
      [
        rate_cell(state, *p, *s, label, is_active),
        Space::new().height(8.0).into(),
      ]
    })
    .collect();
  column(cells).width(Length::Fill).into()
}

fn rate_cell<'a>(
  state: &State,
  primary: AttrKey,
  secondary: AttrKey,
  label: &'static str,
  is_active: bool,
) -> Element<'a, Message> {
  let rate_hr = (sp_per_sec(state.attr_value(primary), state.attr_value(secondary)) * 3600.0).round() as u64;
  container(rate_cell_col(label, rate_hr, primary, secondary, is_active))
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 10.0,
      right: 10.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(if is_active {
        color::accent::PLASMA_SELECTED
      } else {
        color::surface::RAISED
      })),
      border: Border {
        color: if is_active {
          color::state::SELECTION
        } else {
          color::border::SUBTLE
        },
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .width(Length::Fill)
    .into()
}

fn rate_cell_col(
  label: &'static str,
  rate_hr: u64,
  primary: AttrKey,
  secondary: AttrKey,
  is_active: bool,
) -> Element<'static, Message> {
  column([
    text(label)
      .font(mono::REGULAR)
      .size(9.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        }),
      })
      .into(),
    text(fmt_sp(rate_hr))
      .font(mono::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(format!("SP/hr · {}+{}", primary.short(), secondary.short()))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  ])
  .spacing(3.0)
  .into()
}
