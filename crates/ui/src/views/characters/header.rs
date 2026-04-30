//! Header bar for the Characters window — tab strip with action button.
//!
//! The "Characters" and "Corporations" tabs live in this 92 px band.
//! The active tab gets a 2 px plasma underline that sits flush at the
//! bottom of the band, overlapping the bottom rule.

pub mod add_character_button;
pub mod count;
pub mod title;

pub use add_character_button::Component as AddCharacterButton;
pub use count::Component as Count;
use iced::{
  Background, Border, Element, Length, Padding, Shadow,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text},
};
pub use title::Component as Title;

use crate::style::{color, spacing, typography};

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

    let char_count_str = if self.is_filtered && chars_active {
      format!("{} / {}", self.char_visible, self.char_total)
    } else {
      self.char_total.to_string()
    };
    let corp_count_str = if self.is_filtered && corps_active {
      format!("{} / {}", self.corp_visible, self.corp_total)
    } else {
      self.corp_total.to_string()
    };

    let action_btn: Element<'static, Message> = if corps_active {
      render_add_corporation_button()
    } else {
      AddCharacterButton::new().render().map(|_| Message::AddCharacter)
    };

    let tabs_row = row([
      render_tab(
        "Characters",
        &char_count_str,
        chars_active,
        Message::TabSelected("characters".to_string()),
      ),
      render_tab(
        "Corporations",
        &corp_count_str,
        corps_active,
        Message::TabSelected("corporations".to_string()),
      ),
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

fn render_tab(label: &str, count: &str, is_active: bool, on_press: Message) -> Element<'static, Message> {
  let label_owned = label.to_string();
  let count_owned = count.to_string();

  let content = row([
    text(label_owned).font(typography::body::MEDIUM).size(20.0).into(),
    text(count_owned)
      .font(typography::mono::MEDIUM)
      .size(11.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::TERTIARY
        }),
      })
      .into(),
  ])
  .spacing(10.0)
  .align_y(Vertical::Center);

  let centered = container(content).height(Length::Fill).center_y(Length::Fill);

  let tab_btn = button(centered)
    .height(spacing::layout::HEADER_HEIGHT - 2.0)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: 2.0,
      right: 2.0,
    })
    .style(move |_, status| button::Style {
      text_color: match (is_active, status) {
        (true, _) | (_, button::Status::Hovered | button::Status::Pressed) => color::text::PRIMARY,
        _ => color::text::SECONDARY,
      },
      background: None,
      border: Border::default(),
      shadow: Shadow::default(),
      snap: false,
    })
    .on_press(on_press);

  let underline = container(iced::widget::Space::new().width(Length::Fill).height(2.0))
    .width(Length::Fill)
    .height(2.0)
    .style(move |_| container::Style {
      background: if is_active {
        Some(Background::Color(color::accent::PLASMA))
      } else {
        None
      },
      ..container::Style::default()
    });

  column([tab_btn.into(), underline.into()]).width(Length::Shrink).into()
}

fn render_add_corporation_button() -> Element<'static, Message> {
  use crate::{components, style::spacing};

  components::Button::ghost(
    row([
      text("+").font(typography::body::MEDIUM).size(13.0).into(),
      text("Add corporation").font(typography::body::MEDIUM).size(13.0).into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .on_press(Message::AddCorporation)
  .into()
}
