use iced::{Background, Border, Element, Padding, widget::button};

use crate::style::{color, component, radius};

pub struct Component;

impl Component {
  pub fn ghost<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content)
      .padding(component::button::PADDING_DEFAULT)
      .style(|_, status| button::Style {
        background: match status {
          button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
          _ => None,
        },
        border: Border {
          color: color::border::SUBTLE,
          radius: radius::CHIP.into(),
          width: 1.0,
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      })
  }

  pub fn primary<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content)
      .padding(component::button::PADDING_DEFAULT)
      .style(|_, status| button::Style {
        background: Some(Background::Color(match status {
          button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_HOVER,
          _ => color::accent::PLASMA,
        })),
        border: Border {
          radius: radius::CHIP.into(),
          ..Border::default()
        },
        text_color: color::surface::BASE,
        ..button::Style::default()
      })
  }

  pub fn danger<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content)
      .padding(component::button::PADDING_DEFAULT)
      .style(|_, status| button::Style {
        background: Some(Background::Color(match status {
          button::Status::Hovered | button::Status::Pressed => color::accent::DANGER_HOVER,
          _ => color::status::DANGER,
        })),
        border: Border {
          radius: radius::CHIP.into(),
          ..Border::default()
        },
        text_color: color::state::DANGER_FILL,
        ..button::Style::default()
      })
  }

  pub fn danger_ghost<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content)
      .padding(component::button::PADDING_GHOST)
      .style(|_, status| button::Style {
        background: match status {
          button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::status::DANGER_SUBTLE)),
          _ => None,
        },
        border: Border {
          radius: 5.0.into(),
          ..Border::default()
        },
        text_color: color::status::DANGER,
        ..button::Style::default()
      })
  }

  pub fn nav<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>, active: bool) -> button::Button<'a, MSG> {
    button(content).padding(Padding::new(10.0)).style(move |_, status| {
      let bg = match (active, status) {
        (true, _) => Some(color::state::ACTIVE_OVERLAY),
        (false, button::Status::Hovered) => Some(color::state::HOVER_OVERLAY),
        _ => None,
      };
      button::Style {
        background: bg.map(Background::Color),
        border: Border {
          radius: radius::NAVIGATION_ITEM.into(),
          ..Border::default()
        },
        ..button::Style::default()
      }
    })
  }

  pub fn close<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content).padding(0).style(|_, _| button::Style {
      background: None,
      text_color: color::text::SECONDARY,
      ..button::Style::default()
    })
  }

  pub fn row<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content)
      .padding(component::button::PADDING_ROW)
      .style(|_, status| button::Style {
        background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
          Some(Background::Color(color::state::HOVER_OVERLAY))
        } else {
          None
        },
        border: Border {
          radius: radius::CHIP.into(),
          ..Border::default()
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      })
  }
}
