use chrono::Utc;
use iced::{
  Element, Length, Task,
  alignment::Vertical,
  widget::{Column, Row, container, text},
};

use super::{Message as Parent, State, eve_date};
use crate::ui::{
  components::{button::Button, icon::Icon},
  style::{color, spacing, typography},
};

const TITLE_SIZE: f32 = 20.0;

#[derive(Clone, Debug)]
pub enum Message {
  JumpToDay,
}

pub(super) fn update(_state: &mut State, message: Message) -> Task<Parent> {
  match message {
    Message::JumpToDay => Task::none(),
  }
}

pub(super) fn view(_state: &State) -> Element<'_, Parent> {
  let today = Utc::now().date_naive();

  let eyebrow = text(t!("captains_log.eyebrow").to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::accent()),
    });
  let title = text(t!("captains_log.title").into_owned())
    .font(typography::body::MEDIUM)
    .size(TITLE_SIZE)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let eve = text(eve_date::label(today))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let identity = Column::with_children(vec![eyebrow.into(), title.into(), eve.into()])
    .spacing(2.0)
    .width(Length::Fill);

  let back = Button::ghost_icon(Icon::chevron_left()).on_press(Parent::Exit);
  let jump = Button::secondary(t!("captains_log.jump_to_day")).on_press(Parent::Header(Message::JumpToDay));

  let row = Row::with_children(vec![back.into(), identity.into(), jump.into()])
    .spacing(spacing::SPACE_4_5)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::HEADER_HEIGHT))
    .padding(iced::Padding {
      top: 0.0,
      right: spacing::SPACE_6,
      bottom: 0.0,
      left: spacing::SPACE_6,
    })
    .align_y(Vertical::Center)
    .into()
}
