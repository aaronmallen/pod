use iced::{
  Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Column, container, text},
};

use crate::{
  features::shell::window_chrome,
  ui::style::{color, spacing, typography},
};

// Placeholder until the wizard chrome/steps land (task okowsklt); the boot branch only needs the
// window/route to exist, so there are no real messages to dispatch yet.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Message {
  Noop,
}

#[derive(Debug, Default)]
pub struct State {}

pub fn update(_state: &mut State, _message: Message) {}

pub fn view(_state: &State) -> Element<'_, Message> {
  let eyebrow = text(t!("wizard.welcome.eyebrow").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::accent::PLASMA),
    });

  let title = text(t!("wizard.welcome.title").into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG);

  let lede = text(t!("wizard.welcome.lede").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let stage = container(
    Column::with_children(vec![eyebrow.into(), title.into(), lede.into()])
      .align_x(Horizontal::Center)
      .spacing(spacing::SPACE_3),
  )
  .padding(spacing::SPACE_6)
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center);

  container(stage)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(window_chrome::panel_style)
    .into()
}
