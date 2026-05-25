//! Implant slot row component: numbered slot with icon and name.

use std::collections::HashMap;

use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, text},
};
use pod_model::CharacterImplant;

use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::Message,
};

/// Builder for a single implant slot row.
pub struct Component<'a> {
  icons: &'a HashMap<i32, image::Handle>,
  implant: Option<&'a CharacterImplant>,
  slot_num: usize,
}

impl<'a> Component<'a> {
  /// Creates a new implant slot row for the given slot number and optional implant.
  pub fn new(slot_num: usize, implant: Option<&'a CharacterImplant>, icons: &'a HashMap<i32, image::Handle>) -> Self {
    Self {
      icons,
      implant,
      slot_num,
    }
  }

  /// Renders the implant slot row.
  pub fn render(self) -> Element<'a, Message> {
    implant_slot_row(self.slot_num, self.implant, self.icons)
  }
}

/// Builds a full grid of implant slot rows for a clone.
pub fn slot_grid<'a>(
  implants: &'a [CharacterImplant],
  cols: usize,
  icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  const SLOT_COUNT: usize = 10;
  let by_slot: std::collections::HashMap<usize, &CharacterImplant> = implants.iter().map(|i| (i.slot, i)).collect();
  let slot_rows: Vec<Element<'a, Message>> = (1..=SLOT_COUNT)
    .map(|slot_idx| implant_slot_row(slot_idx, by_slot.get(&slot_idx).copied(), icons))
    .collect();
  wrap_slot_grid(slot_rows, cols)
}

fn wrap_slot_grid<'a>(slot_rows: Vec<Element<'a, Message>>, cols: usize) -> Element<'a, Message> {
  let padding = Padding {
    top: 4.0,
    bottom: 4.0,
    left: 8.0,
    right: 8.0,
  };
  if cols == 2 {
    let mut grid_rows: Vec<Element<'a, Message>> = Vec::new();
    let mut iter = slot_rows.into_iter();
    while let Some(left) = iter.next() {
      let right = iter.next();
      let mut row_els: Vec<Element<'a, Message>> = vec![left];
      if let Some(r) = right {
        row_els.push(r);
      } else {
        row_els.push(Space::new().width(Length::Fill).into());
      }
      grid_rows.push(row(row_els).spacing(0.0).width(Length::Fill).into());
    }
    container(column(grid_rows).spacing(0.0).padding(padding))
      .width(Length::Fill)
      .into()
  } else {
    container(column(slot_rows).spacing(0.0).padding(padding))
      .width(Length::Fill)
      .into()
  }
}

fn implant_slot_row<'a>(
  slot_num: usize,
  implant: Option<&'a CharacterImplant>,
  icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  let slot_label_color = if implant.is_some() {
    color::accent::PLASMA
  } else {
    color::text::TERTIARY
  };
  let slot_num_el = text(format!("{slot_num:02}"))
    .font(mono::REGULAR)
    .size(10.0)
    .style(move |_: &Theme| iced::widget::text::Style {
      color: Some(slot_label_color),
    });
  let icon_box = implant_icon_box(implant, icons);
  let name_el = implant_name_el(implant);
  let inner = row([
    container(slot_num_el).width(22.0).into(),
    icon_box,
    Space::new().width(12.0).into(),
    name_el,
  ])
  .align_y(iced::alignment::Vertical::Center)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 8.0,
    right: 8.0,
  });
  container(inner)
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn implant_icon_box<'a>(
  implant: Option<&'a CharacterImplant>,
  icons: &'a HashMap<i32, image::Handle>,
) -> Element<'a, Message> {
  if let Some(imp) = implant {
    implant_icon_filled(imp, icons)
  } else {
    implant_icon_empty()
  }
}

fn implant_icon_empty<'a>() -> Element<'a, Message> {
  container(Space::new().width(32.0).height(32.0))
    .width(32.0)
    .height(32.0)
    .style(|_| container::Style {
      background: None,
      border: Border {
        color: color::border::SUBTLE,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn implant_icon_filled<'a>(imp: &'a CharacterImplant, icons: &'a HashMap<i32, image::Handle>) -> Element<'a, Message> {
  if let Some(handle) = icons.get(&imp.type_id) {
    return container(
      image(handle.clone())
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .content_fit(ContentFit::Cover),
    )
    .width(32.0)
    .height(32.0)
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        color: color::accent::PLASMA_BORDER,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into();
  }
  container(Space::new().width(32.0).height(32.0))
    .width(32.0)
    .height(32.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA_SELECTED)),
      border: Border {
        color: color::accent::PLASMA_BORDER,
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn implant_name_el<'a>(implant: Option<&'a CharacterImplant>) -> Element<'a, Message> {
  if let Some(imp) = implant {
    text(imp.name.clone())
      .font(body::REGULAR)
      .size(12.5)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into()
  } else {
    text("— empty slot —")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::GHOST),
      })
      .width(Length::Fill)
      .into()
  }
}
