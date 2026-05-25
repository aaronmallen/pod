//! Progress% / attributes / completes-at readout column.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::super::{Message, fmt_sp, skill_data::AttrKey};
use crate::{
  format::fmt_eta,
  style::{color, spacing, typography::mono},
};

pub struct Component {
  pct: u32,
  sp_now: u64,
  sp_to: u64,
  primary: AttrKey,
  secondary: AttrKey,
  sp_min: u64,
  sp_day_k: u64,
  remaining_secs: u64,
}

impl Component {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    pct: u32,
    sp_now: u64,
    sp_to: u64,
    primary: AttrKey,
    secondary: AttrKey,
    sp_min: u64,
    sp_day_k: u64,
    remaining_secs: u64,
  ) -> Self {
    Self {
      pct,
      sp_now,
      sp_to,
      primary,
      secondary,
      sp_min,
      sp_day_k,
      remaining_secs,
    }
  }

  pub fn render(self) -> Element<'static, Message> {
    let attr_label = row([
      attr_chip(self.primary, true),
      Space::new().width(spacing::SPACE_2).into(),
      attr_chip(self.secondary, false),
    ])
    .align_y(Vertical::Center);

    column([
      readout_cell(
        "Progress",
        &format!("{}%", self.pct),
        Some(&format!("{} / {} SP", fmt_sp(self.sp_now), fmt_sp(self.sp_to))),
      ),
      Space::new().height(14.0).into(),
      readout_cell_element(
        "Attributes",
        attr_label.into(),
        Some(&format!("{} SP/min · {}K/day", self.sp_min, self.sp_day_k)),
      ),
      Space::new().height(14.0).into(),
      readout_cell("Completes at", &fmt_eta(self.remaining_secs), Some("EVE Time")),
    ])
    .width(Length::Fixed(240.0))
    .into()
  }
}

struct AttrChipColors {
  bg: iced::Color,
  fg: iced::Color,
  border: iced::Color,
}

fn attr_chip_colors(primary: bool) -> AttrChipColors {
  if primary {
    AttrChipColors {
      bg: color::accent::PLASMA_HIGHLIGHT,
      fg: color::accent::PLASMA,
      border: color::accent::PLASMA_BORDER,
    }
  } else {
    AttrChipColors {
      bg: color::state::HOVER_OVERLAY,
      fg: color::text::SECONDARY,
      border: color::border::SUBTLE,
    }
  }
}

pub fn attr_chip<'a>(key: AttrKey, primary: bool) -> Element<'a, Message> {
  let colors = attr_chip_colors(primary);
  let bg = colors.bg;
  let fg = colors.fg;
  let border_col = colors.border;

  container(
    text(key.short())
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 7.0,
    right: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: border_col,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn readout_cell<'a>(label: &str, value: &str, secondary: Option<&str>) -> Element<'a, Message> {
  let mut col = vec![
    text(label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text(value.to_string())
      .font(mono::MEDIUM)
      .size(15.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(sec) = secondary {
    col.push(Space::new().height(2.0).into());
    col.push(
      text(sec.to_string())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    );
  }
  column(col).width(Length::Shrink).into()
}

fn readout_cell_element<'a>(label: &str, value: Element<'a, Message>, secondary: Option<&str>) -> Element<'a, Message> {
  let mut col: Vec<Element<'_, Message>> = vec![
    text(label.to_uppercase())
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    value,
  ];
  if let Some(sec) = secondary {
    col.push(Space::new().height(2.0).into());
    col.push(
      text(sec.to_string())
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    );
  }
  column(col).width(Length::Shrink).into()
}
