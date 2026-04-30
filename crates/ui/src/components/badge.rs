use iced::{
  Background, Border, Color, Element, Padding, Theme,
  widget::{Space, container, row, text},
};

use crate::style::{color, radius, typography};

pub struct Component<'a> {
  kind: Kind<'a>,
}

enum Kind<'a> {
  Status(&'a str),
  Tag(&'a str),
}

impl<'a> Component<'a> {
  pub fn status(label: &'a str) -> Self {
    Self {
      kind: Kind::Status(label),
    }
  }

  pub fn tag(label: &'a str) -> Self {
    Self {
      kind: Kind::Tag(label),
    }
  }

  pub fn render<MSG: 'a>(self) -> Element<'a, MSG> {
    match self.kind {
      Kind::Status(label) => status_badge(label),
      Kind::Tag(label) => tag_badge(label),
    }
  }
}

fn status_badge<'a, MSG: 'a>(label: &'a str) -> Element<'a, MSG> {
  container(text(label).font(typography::mono::REGULAR).size(9.0))
    .padding(Padding {
      top: 2.0,
      bottom: 2.0,
      left: 6.0,
      right: 6.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.4,
      })),
      border: Border {
        radius: radius::CHIP.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn tag_badge<'a, MSG: 'a>(label: &'a str) -> Element<'a, MSG> {
  container(
    text(label)
      .font(typography::body::MEDIUM)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 8.0,
    right: 8.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(Color {
      r: 0.5,
      g: 0.5,
      b: 0.5,
      a: 0.08,
    })),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

/// Numeric count badge: highlights in accent color when unread > 0,
/// falls back to tertiary count, or empty space when both are zero.
pub struct CountBadge {
  count: u32,
  unread: u32,
}

impl CountBadge {
  pub fn new(count: u32) -> Self {
    Self {
      count,
      unread: 0,
    }
  }

  pub fn unread(mut self, unread: u32) -> Self {
    self.unread = unread;
    self
  }

  pub fn render<'a, MSG: 'a>(self) -> Element<'a, MSG> {
    if self.unread > 0 {
      text(self.unread.to_string())
        .font(typography::mono::MEDIUM)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::accent::PLASMA),
        })
        .into()
    } else if self.count > 0 {
      text(self.count.to_string())
        .font(typography::mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into()
    } else {
      Space::new().width(0.0).into()
    }
  }
}

/// Status dot + label badge: a colored dot next to an uppercase label.
pub struct StatusBadge {
  color: Color,
  label: String,
}

impl StatusBadge {
  pub fn new(color: Color, label: impl Into<String>) -> Self {
    Self {
      color,
      label: label.into(),
    }
  }

  pub fn render<'a, MSG: 'a>(self) -> Element<'a, MSG> {
    let c = self.color;
    let dot = container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(move |_| container::Style {
        background: Some(Background::Color(c)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      });
    let label = self.label.to_uppercase();
    row([
      dot.into(),
      Space::new().width(8.0).into(),
      text(label)
        .font(typography::mono::MEDIUM)
        .size(10.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(c),
        })
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .into()
  }
}

/// Glyph icon in a 24×24 rounded container; color chosen by income/expense direction.
pub struct GlyphBadge<'a> {
  glyph: &'a str,
  is_in: bool,
}

impl<'a> GlyphBadge<'a> {
  pub fn new(glyph: &'a str, is_in: bool) -> Self {
    Self {
      glyph,
      is_in,
    }
  }

  pub fn render<MSG: 'a>(self) -> Element<'a, MSG> {
    let glyph_color = if self.is_in {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let bg_color = if self.is_in {
      Color::from_rgba(0.357, 0.725, 0.494, 0.12)
    } else {
      Color::from_rgba(0.878, 0.459, 0.349, 0.12)
    };
    let glyph = self.glyph.to_string();
    container(
      text(glyph)
        .font(typography::mono::MEDIUM)
        .size(12.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(glyph_color),
        }),
    )
    .width(24.0)
    .height(24.0)
    .center_x(24.0)
    .center_y(24.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg_color)),
      border: Border {
        radius: radius::CHIP.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  }
}
