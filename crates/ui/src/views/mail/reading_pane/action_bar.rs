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

fn toolbar_btn_text_color(active: bool) -> iced::Color {
  if active {
    color::status::CAUTION
  } else {
    color::text::SECONDARY
  }
}

fn toolbar_btn_hover_text_color(danger: bool) -> iced::Color {
  if danger {
    color::status::DANGER
  } else {
    color::text::PRIMARY
  }
}

fn toolbar_btn_style(
  text_color: iced::Color,
  hover_text_color: iced::Color,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: if hovered {
        Some(Background::Color(color::state::HOVER_OVERLAY))
      } else {
        None
      },
      border: Border {
        radius: 6.0.into(),
        ..Border::default()
      },
      text_color: if hovered { hover_text_color } else { text_color },
      ..button::Style::default()
    }
  }
}

fn toolbar_divider<'a>() -> Element<'a, Message> {
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
  .into()
}

fn toolbar_date_text<'a>(date_label: &'a str, time: &'a str) -> Element<'a, Message> {
  text(format!("{} · {}", date_label, time))
    .font(crate::style::typography::mono::REGULAR)
    .size(10.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into()
}

fn mail_toolbar_btn<'a>(
  icon: Icon,
  label: &str,
  active: bool,
  danger: bool,
  msg: Option<Message>,
) -> Element<'a, Message> {
  let text_color = toolbar_btn_text_color(active);
  let hover_text_color = toolbar_btn_hover_text_color(danger);
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
    .style(toolbar_btn_style(text_color, hover_text_color));
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
    let toolbar_container = container(self.build_toolbar_row())
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 16.0,
        right: 24.0,
      })
      .width(Length::Fill);
    column([toolbar_container.into(), Separator::horizontal().render()]).into()
  }

  fn build_toolbar_row(&self) -> iced::widget::Row<'a, Message> {
    let star_color = if self.starred {
      color::status::CAUTION
    } else {
      color::text::SECONDARY
    };
    let star_label = if self.starred { "Starred" } else { "Star" };
    let snooze_color = if self.snoozed {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    };
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
      toolbar_divider(),
      mail_toolbar_btn(
        Icon::star().color(star_color),
        star_label,
        self.starred,
        false,
        Some(Message::StarToggle),
      ),
      mail_toolbar_btn(
        Icon::snooze().color(snooze_color),
        &self.snooze_label,
        self.snoozed,
        false,
        Some(Message::SnoozeToggle),
      ),
      mail_toolbar_btn(Icon::archive(), "Archive", false, false, Some(Message::ArchivePressed)),
      mail_toolbar_btn(Icon::trash(), "Delete", false, true, Some(Message::DeletePressed)),
      Space::new().width(Length::Fill).into(),
      toolbar_date_text(self.date_label, self.time),
    ])
    .spacing(0.0)
    .align_y(Vertical::Center)
  }
}
