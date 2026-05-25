//! Unified section component: combined inbox block with account count label and separator.

use iced::{
  Element, Length, Padding,
  widget::{Space, column, container, text},
};

use crate::{
  components::{self, section_label},
  style::{color, typography::mono},
  views::mail::folder_pane::Message,
};

/// Builder for the unified inbox section at the top of the folder sidebar.
pub struct Component<'a> {
  account_count: usize,
  all_inboxes_btn: Element<'a, Message>,
}

impl<'a> Component<'a> {
  /// Creates a new unified section with the given inbox button and account count.
  pub fn new(all_inboxes_btn: Element<'a, Message>, account_count: usize) -> Self {
    Self {
      account_count,
      all_inboxes_btn,
    }
  }

  /// Renders the unified section.
  pub fn render(self) -> Element<'a, Message> {
    let mailbox_label = text(format!("{} mailboxes combined", self.account_count))
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &iced::Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      });
    let inner = container(
      column([
        section_label("Unified"),
        self.all_inboxes_btn,
        Space::new().height(8.0).into(),
        mailbox_label.into(),
      ])
      .spacing(0.0),
    )
    .padding(Padding {
      top: 16.0,
      bottom: 14.0,
      left: 16.0,
      right: 16.0,
    })
    .width(Length::Fill);
    column([inner.into(), components::Separator::horizontal().render()]).into()
  }
}
