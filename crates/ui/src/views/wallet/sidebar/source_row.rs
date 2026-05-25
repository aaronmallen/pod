//! Individual clickable source button in the sidebar.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{button, container, row, text},
};

use crate::{
  components::avatar::{self, AvatarKind},
  style::{
    color,
    typography::{body, mono},
  },
  views::wallet::Message,
};

fn active_style(active: bool, status: button::Status) -> button::Style {
  button::Style {
    background: if active {
      Some(Background::Color(color::accent::PLASMA_SUBTLE))
    } else {
      match status {
        button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
        _ => None,
      }
    },
    border: Border {
      color: Color::TRANSPARENT,
      radius: 6.0.into(),
      width: 0.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn label_text_color(active: bool) -> iced::Color {
  if active {
    color::text::PRIMARY
  } else {
    color::text::STRONG
  }
}

fn mono_text_color(active: bool) -> iced::Color {
  if active {
    color::accent::PLASMA
  } else {
    color::text::SECONDARY
  }
}

fn row_inner<'a>(label: &'a str, mono_label: Option<&'a str>, active: bool) -> Element<'a, Message> {
  let label_color = label_text_color(active);
  let mut children: Vec<Element<'_, Message>> = vec![
    text(label)
      .font(if active { body::MEDIUM } else { body::REGULAR })
      .size(13.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(label_color),
      })
      .width(Length::Fill)
      .into(),
  ];
  if let Some(m) = mono_label {
    let mono_color = mono_text_color(active);
    children.push(
      text(m)
        .font(mono::REGULAR)
        .size(10.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(mono_color),
        })
        .into(),
    );
  }
  row(children)
    .spacing(10.0)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn char_row_inner<'a>(name: &'a str, tone: u16, liquid: String, active: bool) -> Element<'a, Message> {
  let swatch = avatar::Component::new(name, tone, 20.0, AvatarKind::Person).render::<Message>();
  let name_color = label_text_color(active);
  let liquid_color = mono_text_color(active);
  row([
    swatch,
    text(name)
      .font(if active { body::MEDIUM } else { body::REGULAR })
      .size(13.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(name_color),
      })
      .width(Length::Fill)
      .into(),
    text(liquid)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(liquid_color),
      })
      .into(),
  ])
  .spacing(10.0)
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

/// Builder for a simple sidebar source row button.
pub struct Component<'a> {
  label: &'a str,
  mono_label: Option<&'a str>,
  active: bool,
  msg: Message,
}

impl<'a> Component<'a> {
  /// Creates a new source row.
  pub fn new(label: &'a str, active: bool, msg: Message) -> Self {
    Self {
      label,
      mono_label: None,
      active,
      msg,
    }
  }

  /// Adds a monospace secondary label (e.g. ISK balance).
  pub fn mono_label(mut self, label: &'a str) -> Self {
    self.mono_label = Some(label);
    self
  }

  /// Renders the source row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let active = self.active;
    button(row_inner(self.label, self.mono_label, active))
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 16.0,
        right: 14.0,
      })
      .width(Length::Fill)
      .on_press(self.msg)
      .style(move |_, status| active_style(active, status))
      .into()
  }
}

/// Builder for a character sidebar row (with avatar swatch and ISK).
pub struct CharComponent<'a> {
  name: &'a str,
  tone: u16,
  liquid: String,
  active: bool,
  msg: Message,
}

impl<'a> CharComponent<'a> {
  /// Creates a new character row.
  pub fn new(name: &'a str, tone: u16, liquid: String, active: bool, msg: Message) -> Self {
    Self {
      name,
      tone,
      liquid,
      active,
      msg,
    }
  }

  /// Renders the character row into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let active = self.active;
    button(char_row_inner(self.name, self.tone, self.liquid, active))
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 16.0,
        right: 14.0,
      })
      .width(Length::Fill)
      .on_press(self.msg)
      .style(move |_, status| active_style(active, status))
      .into()
  }
}

/// A sidebar container that holds a scrollable list of source rows.
pub struct ContainerComponent<'a> {
  items: Vec<Element<'a, Message>>,
  width: f32,
}

impl<'a> ContainerComponent<'a> {
  /// Creates a new sidebar container with the given items.
  pub fn new(items: Vec<Element<'a, Message>>) -> Self {
    Self {
      items,
      width: 240.0,
    }
  }

  /// Sets the container width.
  pub fn width(mut self, w: f32) -> Self {
    self.width = w;
    self
  }

  /// Renders the container into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    use iced::widget::{column, scrollable};

    container(scrollable(column(self.items).width(Length::Fill)).height(Length::Fill))
      .width(Length::Fixed(self.width))
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::border::SUBTLE,
          width: 1.0,
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}
