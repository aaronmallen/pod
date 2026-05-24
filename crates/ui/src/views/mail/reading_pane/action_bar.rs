//! Reply/forward/star/archive/delete toolbar.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};

use super::Message;
use crate::{
  components::{Icon, Separator},
  style::{color, typography::body},
};

fn mail_toolbar_btn<'a>(
  icon: Icon,
  label: &str,
  active: bool,
  danger: bool,
  msg: Option<Message>,
) -> Element<'a, Message> {
  let text_color = if active {
    color::status::CAUTION
  } else {
    color::text::SECONDARY
  };
  let hover_text_color = if danger {
    color::status::DANGER
  } else {
    color::text::PRIMARY
  };
  let icon_el = icon.size(14.0).render::<Message>();
  let label_el = text(label.to_string())
    .font(body::MEDIUM)
    .size(12.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(text_color),
    });
  let inner = row([icon_el, label_el.into()]).spacing(6.0).align_y(Vertical::Center);
  let btn = button(inner)
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 10.0,
      right: 10.0,
    })
    .style(move |_, status| button::Style {
      background: match status {
        button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
        _ => None,
      },
      border: Border {
        radius: 6.0.into(),
        ..Border::default()
      },
      text_color: match status {
        button::Status::Hovered | button::Status::Pressed => hover_text_color,
        _ => text_color,
      },
      ..button::Style::default()
    });
  if let Some(m) = msg {
    btn.on_press(m).into()
  } else {
    btn.into()
  }
}

/// Builder for the reading pane action toolbar.
pub struct Component<'a> {
  starred: bool,
  snoozed: bool,
  snooze_label: String,
  date_label: &'a str,
  time: &'a str,
}

impl<'a> Component<'a> {
  /// Create a new action bar builder.
  pub fn new(
    starred: bool,
    snoozed: bool,
    snooze_label: impl Into<String>,
    date_label: &'a str,
    time: &'a str,
  ) -> Self {
    Self {
      starred,
      snoozed,
      snooze_label: snooze_label.into(),
      date_label,
      time,
    }
  }

  /// Render into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let toolbar_container = container(
      row([
        mail_toolbar_btn(Icon::reply(), "Reply", false, false, Some(Message::ReplyPressed)),
        mail_toolbar_btn(
          Icon::reply_all(),
          "Reply all",
          false,
          false,
          Some(Message::ForwardPressed),
        ),
        mail_toolbar_btn(Icon::forward(), "Forward", false, false, Some(Message::ForwardPressed)),
        container(
          container(Space::new().width(1.0).height(18.0)).style(|_| container::Style {
            background: Some(Background::Color(color::border::SUBTLE)),
            ..container::Style::default()
          }),
        )
        .padding(Padding {
          top: 0.0,
          bottom: 0.0,
          left: 6.0,
          right: 6.0,
        })
        .into(),
        mail_toolbar_btn(
          Icon::star().color(if self.starred {
            color::status::CAUTION
          } else {
            color::text::SECONDARY
          }),
          if self.starred { "Starred" } else { "Star" },
          self.starred,
          false,
          Some(Message::StarToggle),
        ),
        mail_toolbar_btn(
          Icon::snooze().color(if self.snoozed {
            color::accent::PLASMA
          } else {
            color::text::SECONDARY
          }),
          &self.snooze_label,
          self.snoozed,
          false,
          Some(Message::SnoozeToggle),
        ),
        mail_toolbar_btn(Icon::archive(), "Archive", false, false, Some(Message::ArchivePressed)),
        mail_toolbar_btn(Icon::trash(), "Delete", false, true, Some(Message::DeletePressed)),
        Space::new().width(Length::Fill).into(),
        text(format!("{} · {}", self.date_label, self.time))
          .font(crate::style::typography::mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
      ])
      .spacing(0.0)
      .align_y(Vertical::Center),
    )
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 16.0,
      right: 24.0,
    })
    .width(Length::Fill);
    column([toolbar_container.into(), Separator::horizontal().render()]).into()
  }
}
