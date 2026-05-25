//! Stockpile form panel: side panel for creating/editing a stockpile.

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, scrollable, text, text_input},
};

use super::super::{StockpileForm, StockpileFormItem};
use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::assets::stockpiles_tab::Message,
};

fn text_field_style() -> impl Fn(&iced::Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style {
  |_, _| iced::widget::text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::border::DEFAULT,
      radius: 5.0.into(),
      width: 1.0,
    },
    icon: color::text::SECONDARY,
    placeholder: color::text::TERTIARY,
    value: color::text::PRIMARY,
    selection: color::state::SELECTION,
  }
}

fn form_item_row<'a>(idx: usize, item: &'a StockpileFormItem) -> Element<'a, Message> {
  row([
    text_input("Type ID", &item.type_id_text)
      .on_input(move |v| Message::FormItemTypeChanged(idx, v))
      .font(mono::REGULAR)
      .size(12.0)
      .width(Length::Fixed(100.0))
      .style(text_field_style())
      .into(),
    Space::new().width(8.0).into(),
    text_input("Qty", &item.qty_text)
      .on_input(move |v| Message::FormItemQtyChanged(idx, v))
      .font(mono::REGULAR)
      .size(12.0)
      .width(Length::Fixed(80.0))
      .style(text_field_style())
      .into(),
    Space::new().width(8.0).into(),
    button(
      text("×")
        .font(mono::REGULAR)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        }),
    )
    .on_press(Message::FormRemoveItem(idx))
    .padding(Padding {
      top: 4.0,
      bottom: 4.0,
      left: 8.0,
      right: 8.0,
    })
    .style(|_, _| button::Style {
      background: None,
      border: Border {
        color: color::border::DEFAULT,
        radius: 4.0.into(),
        width: 1.0,
      },
      text_color: color::text::SECONDARY,
      ..button::Style::default()
    })
    .into(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn name_fields(form: &StockpileForm) -> Element<'_, Message> {
  column([
    text("Name")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text_input("Stockpile name", &form.name)
      .on_input(Message::FormNameChanged)
      .font(body::REGULAR)
      .size(13.0)
      .style(text_field_style())
      .into(),
    Space::new().height(12.0).into(),
    text("Location ID (optional)")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(4.0).into(),
    text_input("e.g. 60003760", &form.location_id_text)
      .on_input(Message::FormLocationChanged)
      .font(mono::REGULAR)
      .size(12.0)
      .style(text_field_style())
      .into(),
  ])
  .spacing(0.0)
  .into()
}

fn add_item_btn() -> Element<'static, Message> {
  button(
    text("+ Add item")
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .on_press(Message::FormAddItem)
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 8.0,
  })
  .style(|_, _| button::Style {
    background: None,
    border: Border {
      color: color::accent::PLASMA_MUTED,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .into()
}

fn items_section(form: &StockpileForm) -> Element<'_, Message> {
  let item_rows: Vec<Element<'_, Message>> = form
    .items
    .iter()
    .enumerate()
    .map(|(idx, item)| form_item_row(idx, item))
    .collect();

  column([
    row([
      text("Items")
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .width(Length::Fill)
        .into(),
      add_item_btn(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .into(),
    Space::new().height(6.0).into(),
    column(item_rows).spacing(6.0).into(),
  ])
  .spacing(0.0)
  .into()
}

fn cancel_btn() -> Element<'static, Message> {
  button(
    text("Cancel")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .on_press(Message::FormCancel)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 16.0,
    right: 16.0,
  })
  .style(|_, _| button::Style {
    background: None,
    border: Border {
      color: color::border::DEFAULT,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
  .into()
}

fn save_btn() -> Element<'static, Message> {
  button(
    text("Save")
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::surface::BASE),
      }),
  )
  .on_press(Message::FormSave)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 20.0,
    right: 20.0,
  })
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::accent::PLASMA)),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::surface::BASE,
    ..button::Style::default()
  })
  .into()
}

fn footer(form: &StockpileForm) -> Element<'_, Message> {
  let mut footer_children: Vec<Element<'_, Message>> = Vec::new();
  if !form.error.is_empty() {
    footer_children.push(
      text(form.error.clone())
        .font(body::REGULAR)
        .size(11.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::DANGER),
        })
        .into(),
    );
  }
  footer_children.push(Space::new().width(Length::Fill).into());
  footer_children.push(cancel_btn());
  footer_children.push(Space::new().width(8.0).into());
  footer_children.push(save_btn());

  container(row(footer_children).align_y(iced::alignment::Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: 12.0,
      bottom: 16.0,
      left: 24.0,
      right: 24.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn form_title(form: &StockpileForm) -> &'static str {
  if form.editing_id.is_some() {
    "Edit stockpile"
  } else {
    "New stockpile"
  }
}

fn body_col(form: &StockpileForm) -> Element<'_, Message> {
  column([
    text(form_title(form))
      .font(body::MEDIUM)
      .size(16.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(16.0).into(),
    name_fields(form),
    Space::new().height(16.0).into(),
    items_section(form),
  ])
  .spacing(0.0)
  .into()
}

/// Builder for the stockpile form side panel.
pub struct Component<'a> {
  form: &'a StockpileForm,
}

impl<'a> Component<'a> {
  /// Creates a new form panel for the given form state.
  pub fn new(form: &'a StockpileForm) -> Self {
    Self {
      form,
    }
  }

  /// Renders the form panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let form = self.form;
    let body = body_col(form);

    container(
      column([
        container(scrollable(body).height(Length::Fill))
          .width(Length::Fill)
          .height(Length::Fill)
          .padding(Padding {
            top: 24.0,
            bottom: 0.0,
            left: 24.0,
            right: 24.0,
          })
          .into(),
        footer(form),
      ])
      .height(Length::Fill),
    )
    .width(Length::Fixed(400.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
  }
}
