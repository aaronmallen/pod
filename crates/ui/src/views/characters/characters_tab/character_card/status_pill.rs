use iced::{
  Background, Border, Element, Padding,
  widget::{container, text},
};

use crate::style::{color, radius, typography};

pub enum Status {
  Docked,
  InSpace,
  Unknown,
}

impl Status {
  pub fn from_docked(docked: Option<bool>) -> Self {
    match docked {
      Some(true) => Self::Docked,
      Some(false) => Self::InSpace,
      None => Self::Unknown,
    }
  }

  fn label(&self) -> Option<&'static str> {
    match self {
      Self::Docked => Some("Docked"),
      Self::InSpace => Some("In space"),
      Self::Unknown => None,
    }
  }
}

pub struct Component {
  status: Status,
}

impl Component {
  pub fn new(status: Status) -> Self {
    Self {
      status,
    }
  }

  pub fn render<'a, MSG: 'a>(self) -> Option<Element<'a, MSG>> {
    let label = self.status.label()?;
    Some(
      container(text(label).font(typography::mono::REGULAR).size(9.0))
        .padding(Padding {
          top: 2.0,
          bottom: 2.0,
          left: 6.0,
          right: 6.0,
        })
        .style(|_| container::Style {
          background: Some(Background::Color(color::state::OVERLAY_DARK)),
          border: Border {
            radius: radius::CHIP.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
    )
  }
}
