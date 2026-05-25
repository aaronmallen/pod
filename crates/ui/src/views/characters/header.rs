//! Header bar for the Characters window — tab strip with action button.
//!
//! The "Characters" and "Corporations" tabs live in this 92 px band.
//! The active tab gets a 2 px plasma underline that sits flush at the
//! bottom of the band, overlapping the bottom rule.

pub mod add_character_button;
pub mod add_corporation_button;
pub mod count;
pub mod tab;
pub mod title;

pub use add_character_button::Component as AddCharacterButton;
pub use add_corporation_button::Component as AddCorporationButton;
pub use count::Component as Count;
use iced::{
  Background, Element, Length, Padding,
  widget::{Space, column, container, row},
};
pub use tab::Component as Tab;
pub use title::Component as Title;

use crate::style::{color, spacing};

/// State held by the header component.
#[derive(Clone, Debug, Default)]
pub struct State;

/// Messages emitted by the header.
#[derive(Clone, Debug)]
pub enum Message {
  /// The "Add character" button was pressed.
  AddCharacter,
  /// The "Add corporation" button was pressed.
  AddCorporation,
  /// A tab was selected; carries the tab id string.
  TabSelected(String),
}

/// Builder for the Characters window header.
pub struct Component {
  active_tab: String,
  char_total: usize,
  char_visible: usize,
  corp_total: usize,
  corp_visible: usize,
  is_filtered: bool,
}

impl Component {
  /// Creates a new header component.
  pub fn new(
    active_tab: impl Into<String>,
    char_visible: usize,
    char_total: usize,
    corp_visible: usize,
    corp_total: usize,
    is_filtered: bool,
  ) -> Self {
    Self {
      active_tab: active_tab.into(),
      char_total,
      char_visible,
      corp_total,
      corp_visible,
      is_filtered,
    }
  }

  /// Renders the header into an iced element.
  pub fn render(self) -> Element<'static, Message> {
    let chars_active = self.active_tab == "characters";
    let corps_active = self.active_tab == "corporations";

    let char_count_str = count_label(self.is_filtered && chars_active, self.char_visible, self.char_total);
    let corp_count_str = count_label(self.is_filtered && corps_active, self.corp_visible, self.corp_total);

    let action_btn: Element<'static, Message> = if corps_active {
      AddCorporationButton::new().render().map(|_| Message::AddCorporation)
    } else {
      AddCharacterButton::new().render().map(|_| Message::AddCharacter)
    };

    let tabs_row = row([
      Tab::new(
        "Characters",
        &char_count_str,
        chars_active,
        Message::TabSelected("characters".to_string()),
      )
      .render(),
      Tab::new(
        "Corporations",
        &corp_count_str,
        corps_active,
        Message::TabSelected("corporations".to_string()),
      )
      .render(),
      iced::widget::Space::new().width(Length::Fill).into(),
      container(action_btn).center_y(spacing::layout::HEADER_HEIGHT).into(),
    ])
    .spacing(spacing::SPACE_7)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::SPACE_8,
      right: spacing::SPACE_8,
    });

    let header = container(tabs_row)
      .width(Length::Fill)
      .height(spacing::layout::HEADER_HEIGHT);
    let border_line = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      });
    column([header.into(), border_line.into()]).width(Length::Fill).into()
  }
}

fn count_label(filtered: bool, visible: usize, total: usize) -> String {
  if filtered {
    format!("{} / {}", visible, total)
  } else {
    total.to_string()
  }
}
