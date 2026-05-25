//! Neural attribute display component.

pub mod attr_row;
pub mod rate_grid;

pub use attr_row::Component as AttrRow;
use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Space, column, container, text},
};
pub use rate_grid::RateGrid;

use super::super::{State, skill_data::AttrKey};
use crate::{
  components,
  style::{
    color, spacing,
    typography::{body, mono},
  },
};

/// Messages produced by the attributes tab.
#[derive(Clone, Debug)]
pub enum Message {}

pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let attr_pair = self
      .state
      .queue
      .first()
      .and_then(|q| super::super::skill_data::find_skill(&q.skill_name, &self.state.skill_groups))
      .map(|(s, _)| (s.primary, s.secondary))
      .or_else(|| {
        self
          .state
          .active_character()
          .and_then(|c| c.active_training())
          .and_then(|t| t.skill_name.as_ref())
          .and_then(|n| super::super::skill_data::find_skill(n, &self.state.skill_groups))
          .map(|(s, _)| (s.primary, s.secondary))
      });
    let active_primary = attr_pair.map(|(p, _)| p);
    let active_secondary = attr_pair.map(|(_, s)| s);
    let total_pts: u32 = AttrKey::ALL.iter().map(|k| self.state.attr_value(*k)).sum();
    let attr_bars = bar_items(self.state, total_pts, active_primary, active_secondary);

    column([
      column(attr_bars).width(Length::Fill).into(),
      RateGrid::new(self.state, active_primary, active_secondary).render(),
      remap_cta(self.state),
      Space::new().height(spacing::SPACE_4).into(),
    ])
    .width(Length::Fill)
    .into()
  }
}

fn bar_items<'a>(
  state: &'a State,
  total_pts: u32,
  active_primary: Option<AttrKey>,
  active_secondary: Option<AttrKey>,
) -> Vec<Element<'a, Message>> {
  let mut bars: Vec<Element<'_, Message>> = vec![section_header(total_pts)];
  for (i, key) in AttrKey::ALL.iter().enumerate() {
    let is_primary = active_primary == Some(*key);
    let is_secondary = active_secondary == Some(*key);
    let accent = if is_primary {
      color::accent::PLASMA
    } else if is_secondary {
      color::accent::PLASMA_HOVER
    } else {
      color::text::PRIMARY
    };
    if i > 0 {
      bars.push(components::Separator::horizontal().render());
    }
    bars.push(AttrRow::new(*key, state.attr_value(*key), accent, is_primary, is_secondary).render());
  }
  bars
}

fn section_header<'a>(total_pts: u32) -> Element<'a, Message> {
  container(
    column([
      text("Neural attributes")
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().height(4.0).into(),
      text(format!("{} pts allocated", total_pts))
        .font(mono::REGULAR)
        .size(11.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .width(Length::Fill),
  )
  .padding(Padding {
    top: 14.0,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .width(Length::Fill)
  .into()
}

fn remap_cta<'a>(state: &'a State) -> Element<'a, Message> {
  let (remap_bonus_text, remap_detail_text) =
    if let Some(attrs) = state.active_character().and_then(|c| c.attributes().as_ref()) {
      let bonus_str = match attrs.bonus_remaps {
        0 => "No bonus remaps".to_string(),
        1 => "1 bonus available".to_string(),
        n => format!("{} bonuses available", n),
      };
      let detail_str = match (&attrs.last_remap_date, &attrs.accrued_remap_cooldown_date) {
        (Some(last), Some(cd)) => format!(
          "Last remap {} · next available {}",
          last.get(..10).unwrap_or(last.as_str()),
          cd.get(..10).unwrap_or(cd.as_str())
        ),
        (Some(last), None) => format!("Last remap {}", last.get(..10).unwrap_or(last.as_str())),
        _ => "No remap history".to_string(),
      };
      (bonus_str, detail_str)
    } else {
      ("No remap data".to_string(), "Attributes not yet loaded".to_string())
    };

  container(remap_card(remap_bonus_text, remap_detail_text))
    .padding(Padding {
      top: 14.0,
      bottom: 0.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
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
