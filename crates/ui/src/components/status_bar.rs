//! Status bar footer view for the main window.

pub mod esi_status;
pub mod eve_time;
pub mod refresh_button;
pub mod sync_indicator;
pub mod sync_state;

use iced::{
  Background, Color, Element, Length,
  widget::{column, container, row, stack},
};
pub use sync_state::SyncState;

use crate::{components, style::color};

/// Messages produced by the status bar.
#[derive(Clone, Debug)]
pub enum Message {
  /// The user pressed the Refresh button.
  RefreshPressed,
}

/// 28-px status bar footer view.
pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render<'a>(self, eve_time: &'a str, sync: &'a SyncState, esi_connected: bool) -> Element<'a, Message> {
    let is_syncing = sync.is_syncing();
    let progress = sync.progress();

    let bg = plasma_fill(is_syncing, progress);
    let content: Element<'a, Message> = row([
      eve_time::view(eve_time),
      components::Separator::vertical().render(),
      sync_indicator::view(is_syncing, progress, sync.secs_since_sync()),
      components::Separator::vertical().render(),
      refresh_button::view(is_syncing),
      components::Separator::vertical().render(),
      esi_status::view(esi_connected),
    ])
    .height(Length::Fill)
    .align_y(iced::alignment::Vertical::Center)
    .into();

    let top_border = container(iced::widget::Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.4))),
        ..container::Style::default()
      });

    let bar = container(stack(vec![bg, content]).width(Length::Fill).height(27.0))
      .width(Length::Fill)
      .height(27.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::NAVIGATION)),
        ..container::Style::default()
      });

    column([top_border.into(), bar.into()]).width(Length::Fill).into()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

fn plasma_fill<'a>(is_syncing: bool, progress: f32) -> Element<'a, Message> {
  if !is_syncing {
    return iced::widget::Space::new()
      .width(Length::Fill)
      .height(Length::Fill)
      .into();
  }

  let pct = (progress.clamp(0.0, 1.0) * 100.0) as u16;
  let rest = 100u16.saturating_sub(pct);

  if pct == 0 {
    iced::widget::Space::new()
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  } else if rest == 0 {
    container(iced::widget::Space::new().width(Length::Fill).height(Length::Fill))
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.18))),
        ..container::Style::default()
      })
      .into()
  } else {
    row([
      container(iced::widget::Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::FillPortion(pct))
        .height(Length::Fill)
        .style(|_| container::Style {
          background: Some(Background::Color(Color::from_rgba(0.247, 0.722, 0.859, 0.18))),
          ..container::Style::default()
        })
        .into(),
      iced::widget::Space::new()
        .width(Length::FillPortion(rest))
        .height(Length::Fill)
        .into(),
    ])
    .height(Length::Fill)
    .into()
  }
}
