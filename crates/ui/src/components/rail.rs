//! Left navigation rail — pure layout component.

use iced::{
  Background, Color, Element, Length, Padding,
  alignment::Horizontal,
  widget::{Space, column, container, row, svg},
};

use crate::style::{color, spacing};

const POD_MARK_SVG: &[u8] = include_bytes!("../../../../assets/logo/pod-mark.svg");

/// Pure layout component for the left navigation rail.
///
/// Accepts pre-built navigation item elements and renders them in a
/// vertical column below the Pod mark logo. Emits no messages of its
/// own — all interactivity is encoded in the passed-in elements.
pub struct Component<'a, MSG: Clone + 'a> {
  nav_items: Vec<Element<'a, MSG>>,
}

impl<'a, MSG: Clone + 'a> Component<'a, MSG> {
  /// Create a new rail with the given navigation item elements.
  pub fn new(nav_items: Vec<Element<'a, MSG>>) -> Self {
    Self {
      nav_items,
    }
  }

  /// Render the rail into an [`Element`].
  pub fn render(self) -> Element<'a, MSG> {
    let logo = svg(svg::Handle::from_memory(POD_MARK_SVG)).width(28.0).height(28.0);

    let logo_band = container(logo)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .width(Length::Fill)
      .height(spacing::layout::HEADER_HEIGHT - 1.0);

    let logo_border = container(Space::new().width(Length::Fill).height(1.0))
      .width(Length::Fill)
      .height(1.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      });

    let logo_header = column([logo_band.into(), logo_border.into()]);

    let nav_col = column(self.nav_items)
      .spacing(spacing::SPACE_1)
      .padding(Padding {
        top: spacing::SPACE_4,
        ..Padding::ZERO
      })
      .align_x(Horizontal::Center);

    let rail = container(
      column([
        logo_header.into(),
        nav_col.into(),
        Space::new().height(Length::Fill).into(),
      ])
      .align_x(Horizontal::Center)
      .height(Length::Fill),
    )
    .width(spacing::layout::RAIL_WIDTH)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::NAVIGATION)),
      ..container::Style::default()
    });

    let right_border = container(Space::new().width(1.0).height(Length::Fill))
      .width(1.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.4))),
        ..container::Style::default()
      });

    row([rail.into(), right_border.into()]).into()
  }
}
