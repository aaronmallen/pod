//! Stockpiles tab — grid of stockpile cards with CRUD.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, scrollable, text, text_input},
};

use super::{State, StockpileForm, StockpileFormItem, StockpileWithStatus};
use crate::style::{
  color,
  typography::{body, mono},
};

/// Messages produced by the stockpiles tab.
#[derive(Clone, Debug)]
pub enum Message {
  NewStockpile,
  EditStockpile(i64),
  DeleteStockpile(i64),
  ConfirmDelete(i64),
  FormNameChanged(String),
  FormLocationChanged(String),
  FormItemTypeChanged(usize, String),
  FormItemQtyChanged(usize, String),
  FormAddItem,
  FormRemoveItem(usize),
  FormCancel,
  FormSave,
}

fn status_dot_color(pile: &StockpileWithStatus) -> Color {
  if pile.ready {
    color::text::SUCCESS
  } else if pile.overall_pct >= 0.6 {
    color::text::WARNING
  } else {
    color::text::DANGER
  }
}

fn pile_item_row<'a>(item: &'a super::StockpileItemStatus) -> Element<'a, Message> {
  let ok = item.have_quantity >= item.target_quantity as i64;
  let pct = item.pct.clamp(0.0, 1.0);
  let bar_color = if ok {
    color::text::SUCCESS
  } else if pct > 0.5 {
    color::text::WARNING
  } else {
    color::text::DANGER
  };
  let have_str = fmt_count(item.have_quantity as u64);
  let target_str = fmt_count(item.target_quantity as u64);
  let need = (item.target_quantity as i64 - item.have_quantity).max(0);

  let icon_placeholder = container(Space::new().width(22.0).height(22.0))
    .width(22.0)
    .height(22.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::DEFAULT)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let name_and_bar = column([
    text(item.type_name.clone())
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    container(
      container(Space::new())
        .width(Length::FillPortion((pct * 1000.0) as u16))
        .height(2.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(bar_color)),
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .height(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into(),
  ])
  .spacing(4.0)
  .width(Length::Fill);

  let counts_col: Element<'_, Message> = if ok {
    column([text(format!("{} / {}", have_str, target_str))
      .font(mono::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SUCCESS),
      })
      .into()])
    .align_x(iced::alignment::Horizontal::Right)
    .width(Length::Fixed(110.0))
    .into()
  } else {
    column([
      row([
        text(have_str)
          .font(mono::REGULAR)
          .size(12.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text(format!(" / {}", target_str))
          .font(mono::REGULAR)
          .size(12.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::TERTIARY),
          })
          .into(),
      ])
      .into(),
      text(format!("need {}", fmt_count(need as u64)))
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::DANGER),
        })
        .into(),
    ])
    .align_x(iced::alignment::Horizontal::Right)
    .width(Length::Fixed(110.0))
    .into()
  };

  container(
    row([
      icon_placeholder.into(),
      Space::new().width(10.0).into(),
      name_and_bar.into(),
      Space::new().width(10.0).into(),
      counts_col,
    ])
    .align_y(iced::alignment::Vertical::Center)
    .padding(Padding {
      top: 8.0,
      bottom: 8.0,
      left: 18.0,
      right: 18.0,
    }),
  )
  .width(Length::Fill)
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

fn stockpile_card(pile: &StockpileWithStatus) -> Element<'_, Message> {
  let dot_color = status_dot_color(pile);
  let pct_color = if pile.ready {
    color::text::SUCCESS
  } else {
    color::text::WARNING
  };
  let pct = pile.overall_pct.clamp(0.0, 1.0);
  let bar_fill_color = if pile.ready {
    color::text::SUCCESS
  } else {
    color::accent::PLASMA
  };

  let id = pile.id;
  let id_edit = pile.id;

  let header = column([
    row([
      container(Space::new().width(8.0).height(8.0))
        .width(8.0)
        .height(8.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(dot_color)),
          border: Border {
            radius: 4.0.into(),
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
      Space::new().width(10.0).into(),
      text(pile.name.clone())
        .font(body::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      text(format!("{}%", (pct * 100.0).round() as u32))
        .font(mono::REGULAR)
        .size(11.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(pct_color),
        })
        .into(),
      Space::new().width(8.0).into(),
      button(
        text("Edit")
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .on_press(Message::EditStockpile(id_edit))
      .padding(Padding {
        top: 3.0,
        bottom: 3.0,
        left: 6.0,
        right: 6.0,
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
      Space::new().width(4.0).into(),
      button(
        text("Delete")
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::DANGER),
          }),
      )
      .on_press(Message::DeleteStockpile(id))
      .padding(Padding {
        top: 3.0,
        bottom: 3.0,
        left: 6.0,
        right: 6.0,
      })
      .style(|_, _| button::Style {
        background: None,
        border: Border {
          color: color::text::DANGER,
          radius: 4.0.into(),
          width: 1.0,
        },
        text_color: color::text::DANGER,
        ..button::Style::default()
      })
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .spacing(0.0)
    .into(),
    Space::new().height(4.0).into(),
    text(
      pile
        .location_id
        .map(|l| format!("Location {}", l))
        .unwrap_or_else(|| "All locations".to_string()),
    )
    .font(mono::REGULAR)
    .size(10.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    })
    .into(),
    Space::new().height(10.0).into(),
    container(
      container(Space::new())
        .width(Length::FillPortion((pct * 1000.0) as u16))
        .height(4.0)
        .style(move |_| container::Style {
          background: Some(Background::Color(bar_fill_color)),
          ..container::Style::default()
        }),
    )
    .width(Length::Fill)
    .height(4.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      border: Border {
        radius: 2.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into(),
  ])
  .spacing(0.0);

  let item_rows: Vec<Element<'_, Message>> = pile.items.iter().map(pile_item_row).collect();

  let border_color = if pile.ready {
    Color {
      r: 0.357,
      g: 0.725,
      b: 0.494,
      a: 0.35,
    }
  } else {
    color::border::DEFAULT
  };

  container(
    column([
      container(header)
        .width(Length::Fill)
        .padding(Padding {
          top: 14.0,
          bottom: 12.0,
          left: 18.0,
          right: 18.0,
        })
        .into(),
      container(column(item_rows).width(Length::Fill))
        .width(Length::Fill)
        .style(|_| container::Style {
          border: Border {
            color: color::border::SUBTLE,
            width: 1.0,
            ..Border::default()
          },
          ..container::Style::default()
        })
        .into(),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: border_color,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn form_item_row<'a>(idx: usize, item: &'a StockpileFormItem) -> Element<'a, Message> {
  row([
    text_input("Type ID", &item.type_id_text)
      .on_input(move |v| Message::FormItemTypeChanged(idx, v))
      .font(mono::REGULAR)
      .size(12.0)
      .width(Length::Fixed(100.0))
      .style(|_, _| iced::widget::text_input::Style {
        background: Background::Color(color::surface::SUNKEN),
        border: Border {
          color: color::border::DEFAULT,
          radius: 5.0.into(),
          width: 1.0,
        },
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      })
      .into(),
    Space::new().width(8.0).into(),
    text_input("Qty", &item.qty_text)
      .on_input(move |v| Message::FormItemQtyChanged(idx, v))
      .font(mono::REGULAR)
      .size(12.0)
      .width(Length::Fixed(80.0))
      .style(|_, _| iced::widget::text_input::Style {
        background: Background::Color(color::surface::SUNKEN),
        border: Border {
          color: color::border::DEFAULT,
          radius: 5.0.into(),
          width: 1.0,
        },
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      })
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

fn stockpile_form_panel(form: &StockpileForm) -> Element<'_, Message> {
  let title = if form.editing_id.is_some() {
    "Edit stockpile"
  } else {
    "New stockpile"
  };

  let item_rows: Vec<Element<'_, Message>> = form
    .items
    .iter()
    .enumerate()
    .map(|(idx, item)| form_item_row(idx, item))
    .collect();

  let body_col = column([
    text(title)
      .font(body::MEDIUM)
      .size(16.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(16.0).into(),
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
      .style(|_, _| iced::widget::text_input::Style {
        background: Background::Color(color::surface::SUNKEN),
        border: Border {
          color: color::border::DEFAULT,
          radius: 5.0.into(),
          width: 1.0,
        },
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      })
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
      .style(|_, _| iced::widget::text_input::Style {
        background: Background::Color(color::surface::SUNKEN),
        border: Border {
          color: color::border::DEFAULT,
          radius: 5.0.into(),
          width: 1.0,
        },
        icon: color::text::SECONDARY,
        placeholder: color::text::TERTIARY,
        value: color::text::PRIMARY,
        selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
      })
      .into(),
    Space::new().height(16.0).into(),
    row([
      text("Items")
        .font(mono::REGULAR)
        .size(10.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .width(Length::Fill)
        .into(),
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
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .into(),
    Space::new().height(6.0).into(),
    column(item_rows).spacing(6.0).into(),
  ])
  .spacing(0.0);

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
  footer_children.push(
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
    .into(),
  );
  footer_children.push(Space::new().width(8.0).into());
  footer_children.push(
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
    .into(),
  );

  container(
    column([
      container(scrollable(body_col).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
          top: 24.0,
          bottom: 0.0,
          left: 24.0,
          right: 24.0,
        })
        .into(),
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
        .into(),
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

fn empty_state() -> Element<'static, Message> {
  container(
    text("No stockpiles yet. Create one with the button above.")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(48.0)
  .width(Length::Fill)
  .center_x(Length::Fill)
  .height(Length::Fill)
  .center_y(Length::Fill)
  .into()
}

fn fmt_count(n: u64) -> String {
  if n >= 1_000_000 {
    format!("{:.1}M", n as f64 / 1_000_000.0)
  } else if n >= 1_000 {
    format!("{:.1}K", n as f64 / 1_000.0)
  } else {
    n.to_string()
  }
}

/// Builder for the stockpiles tab.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new stockpiles tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the stockpiles tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;

    let ready_count = state.stockpiles.iter().filter(|p| p.ready).count();
    let short_count = state.stockpiles.iter().filter(|p| !p.ready).count();

    let toolbar = container(
      row([
        text("Stockpile targets")
          .font(body::MEDIUM)
          .size(16.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        Space::new().width(14.0).into(),
        text(format!("{} ready · {} short", ready_count, short_count))
          .font(mono::REGULAR)
          .size(10.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        Space::new().width(Length::Fill).into(),
        button(
          text("＋ New stockpile")
            .font(body::REGULAR)
            .size(12.0)
            .style(|_: &Theme| iced::widget::text::Style {
              color: Some(color::text::SECONDARY),
            }),
        )
        .on_press(Message::NewStockpile)
        .padding(Padding {
          top: 7.0,
          bottom: 7.0,
          left: 12.0,
          right: 12.0,
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
        .into(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .padding(Padding {
      top: 0.0,
      bottom: 18.0,
      left: 0.0,
      right: 0.0,
    });

    let grid: Element<'_, Message> = if state.stockpiles.is_empty() {
      empty_state()
    } else {
      let cards: Vec<Element<'_, Message>> = state.stockpiles.iter().map(stockpile_card).collect();
      scrollable(column(cards).spacing(14.0).width(Length::Fill))
        .height(Length::Fill)
        .into()
    };

    let content = container(column([toolbar.into(), grid]).width(Length::Fill).height(Length::Fill))
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill)
      .height(Length::Fill);

    if let Some(form) = &state.stockpile_form {
      row([content.into(), stockpile_form_panel(form)])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
      content.into()
    }
  }
}
