use iced::{Background, Border, Color, Element, Padding, widget::button};

use crate::style::{color, radius};

pub struct Component;

impl Component {
  pub fn ghost<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content)
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 14.0,
        right: 14.0,
      })
      .style(|_, status| button::Style {
        background: match status {
          button::Status::Hovered | button::Status::Pressed => Some(Background::Color(Color {
            r: 0.957,
            g: 0.949,
            b: 0.925,
            a: 0.04,
          })),
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
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 14.0,
        right: 14.0,
      })
      .style(|_, status| button::Style {
        background: Some(Background::Color(match status {
          button::Status::Hovered | button::Status::Pressed => Color {
            r: 0.247,
            g: 0.722,
            b: 0.859,
            a: 0.85,
          },
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
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 14.0,
        right: 14.0,
      })
      .style(|_, status| button::Style {
        background: Some(Background::Color(match status {
          button::Status::Hovered | button::Status::Pressed => Color {
            r: 0.878,
            g: 0.459,
            b: 0.349,
            a: 0.85,
          },
          _ => color::status::DANGER,
        })),
        border: Border {
          radius: radius::CHIP.into(),
          ..Border::default()
        },
        text_color: Color {
          r: 0.102,
          g: 0.039,
          b: 0.035,
          a: 1.0,
        },
        ..button::Style::default()
      })
  }

  pub fn danger_ghost<'a, MSG: Clone + 'static>(content: impl Into<Element<'a, MSG>>) -> button::Button<'a, MSG> {
    button(content)
      .padding(Padding {
        top: 7.0,
        bottom: 7.0,
        left: 10.0,
        right: 10.0,
      })
      .style(|_, status| button::Style {
        background: match status {
          button::Status::Hovered | button::Status::Pressed => Some(Background::Color(Color {
            r: 0.878,
            g: 0.459,
            b: 0.349,
            a: 0.12,
          })),
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
        (true, _) => Some(Color {
          r: 0.957,
          g: 0.949,
          b: 0.925,
          a: 0.10,
        }),
        (false, button::Status::Hovered) => Some(Color {
          r: 0.957,
          g: 0.949,
          b: 0.925,
          a: 0.05,
        }),
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
      .padding(Padding {
        top: 6.0,
        bottom: 6.0,
        left: 8.0,
        right: 8.0,
      })
      .style(|_, status| button::Style {
        background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
          Some(Background::Color(Color {
            r: 0.957,
            g: 0.949,
            b: 0.925,
            a: 0.05,
          }))
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
