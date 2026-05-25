//! Notifications tab: categorised list with unread indicators and filter.

pub mod filter_control;
pub mod notif_icon_box;
pub mod notif_row;

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, scrollable, text},
};
use pod_model::CharacterNotification;

use crate::{
  components::LoadState,
  style::{color, typography::mono},
  views::character_detail::{LoadState as DataState, Message, NotificationsFilter},
};

/// Builder for the notifications tab content.
pub struct Component<'a> {
  filter: &'a NotificationsFilter,
  filtered: &'a [CharacterNotification],
  notifications: &'a DataState<Vec<CharacterNotification>>,
  unread_count: usize,
}

impl<'a> Component<'a> {
  /// Creates a new notifications tab component.
  pub fn new(
    notifications: &'a DataState<Vec<CharacterNotification>>,
    filtered: &'a [CharacterNotification],
    unread_count: usize,
    filter: &'a NotificationsFilter,
  ) -> Self {
    Self {
      filter,
      filtered,
      notifications,
      unread_count,
    }
  }

  /// Renders the notifications tab.
  pub fn render(self) -> Element<'a, Message> {
    match self.notifications {
      DataState::Loading => LoadState::loading("Loading notifications…").render(),
      DataState::Error(e) => LoadState::error(e).render(),
      DataState::Loaded(_) => notifications_content(self.filtered, self.unread_count, self.filter),
    }
  }
}

fn notifications_card<'a>(visible: &[&'a CharacterNotification]) -> Element<'a, Message> {
  let mut notif_rows: Vec<Element<'_, Message>> = visible
    .iter()
    .enumerate()
    .map(|(i, n)| notif_row::Component::new(n, i == visible.len() - 1).render())
    .collect();
  if notif_rows.is_empty() {
    notif_rows.push(
      container(
        text("No notifications match your filter.")
          .font(mono::REGULAR)
          .size(13.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .padding(20.0)
      .width(Length::Fill)
      .into(),
    );
  }
  container(column(notif_rows))
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn notifications_content<'a>(
  notifications: &'a [CharacterNotification],
  unread_count: usize,
  filter: &'a NotificationsFilter,
) -> Element<'a, Message> {
  let visible: Vec<&CharacterNotification> = notifications.iter().collect();
  let eyebrow = notifications_eyebrow(visible.len(), unread_count, filter);
  let card = notifications_card(&visible);
  scrollable(
    column([eyebrow, card])
      .spacing(16.0)
      .padding(Padding {
        top: 24.0,
        bottom: 24.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
  )
  .height(Length::Fill)
  .into()
}

fn notifications_eyebrow<'a>(
  visible_count: usize,
  unread_count: usize,
  filter: &'a NotificationsFilter,
) -> Element<'a, Message> {
  let eyebrow_left = format!("Notifications · {visible_count}");
  let eyebrow_right = format!("{unread_count} unread");
  row([
    text(eyebrow_left.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(8.0).into(),
    text(eyebrow_right)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    filter_control::Component::new(filter).render(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}
