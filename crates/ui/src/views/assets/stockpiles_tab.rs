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

fn fmt_count(n: u64) -> String {
  if n >= 1_000_000 {
    format!("{:.1}M", n as f64 / 1_000_000.0)
  } else if n >= 1_000 {
    format!("{:.1}K", n as f64 / 1_000.0)
  } else {
    n.to_string()
  }
}

fn pile_item_fill_bar(pct: f32, bar_color: Color) -> Element<'static, Message> {
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
  .into()
}

fn pile_item_counts_ok(have_str: &str, target_str: &str) -> Element<'static, Message> {
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
}

fn pile_item_counts_short(have_str: &str, target_str: &str, need: i64) -> Element<'static, Message> {
  column([
    row([
      text(have_str.to_string())
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
}

fn pile_item_icon_placeholder() -> Element<'static, Message> {
  container(Space::new().width(22.0).height(22.0))
    .width(22.0)
    .height(22.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::DEFAULT)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn pile_item_name_and_bar(type_name: &str, pct: f32, bar_color: Color) -> Element<'_, Message> {
  column([
    text(type_name.to_string())
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    pile_item_fill_bar(pct, bar_color),
  ])
  .spacing(4.0)
  .width(Length::Fill)
  .into()
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

  let name_and_bar = pile_item_name_and_bar(&item.type_name, pct, bar_color);
  let counts_col = if ok {
    pile_item_counts_ok(&have_str, &target_str)
  } else {
    pile_item_counts_short(&have_str, &target_str, need)
  };

  container(
    row([
      pile_item_icon_placeholder(),
      Space::new().width(10.0).into(),
      name_and_bar,
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

fn card_fill_bar(pct: f32, bar_fill_color: Color) -> Element<'static, Message> {
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
  .into()
}

fn card_action_btn(label: &str, msg: Message, danger: bool) -> Element<'_, Message> {
  let text_color = if danger {
    color::text::DANGER
  } else {
    color::text::SECONDARY
  };
  let border_color = if danger {
    color::text::DANGER
  } else {
    color::border::DEFAULT
  };

  button(
    text(label.to_string())
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .on_press(msg)
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 6.0,
    right: 6.0,
  })
  .style(move |_, _| button::Style {
    background: None,
    border: Border {
      color: border_color,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn card_title_row(pile: &StockpileWithStatus, dot_color: Color, pct: f32, pct_color: Color) -> Element<'_, Message> {
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
    card_action_btn("Edit", Message::EditStockpile(pile.id), false),
    Space::new().width(4.0).into(),
    card_action_btn("Delete", Message::DeleteStockpile(pile.id), true),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .spacing(0.0)
  .into()
}

fn card_header(pile: &StockpileWithStatus) -> Element<'_, Message> {
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

  let location_label = pile
    .location_id
    .map(|l| format!("Location {}", l))
    .unwrap_or_else(|| "All locations".to_string());

  column([
    card_title_row(pile, dot_color, pct, pct_color),
    Space::new().height(4.0).into(),
    text(location_label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().height(10.0).into(),
    card_fill_bar(pct, bar_fill_color),
  ])
  .spacing(0.0)
  .into()
}

fn card_border_color(pile: &StockpileWithStatus) -> Color {
  if pile.ready {
    Color {
      r: 0.357,
      g: 0.725,
      b: 0.494,
      a: 0.35,
    }
  } else {
    color::border::DEFAULT
  }
}

fn stockpile_card(pile: &StockpileWithStatus) -> Element<'_, Message> {
  let border_color = card_border_color(pile);
  let header = card_header(pile);
  let item_rows: Vec<Element<'_, Message>> = pile.items.iter().map(pile_item_row).collect();

  let header_section = container(header)
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      bottom: 12.0,
      left: 18.0,
      right: 18.0,
    })
    .into();

  let items_section = container(column(item_rows).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into();

  container(column([header_section, items_section]).width(Length::Fill))
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
    selection: Color::from_rgba(0.247, 0.722, 0.859, 0.30),
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

fn form_name_fields(form: &StockpileForm) -> Element<'_, Message> {
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

fn form_items_section(form: &StockpileForm) -> Element<'_, Message> {
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

fn form_cancel_btn() -> Element<'static, Message> {
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

fn form_save_btn() -> Element<'static, Message> {
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

fn form_footer(form: &StockpileForm) -> Element<'_, Message> {
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
  footer_children.push(form_cancel_btn());
  footer_children.push(Space::new().width(8.0).into());
  footer_children.push(form_save_btn());

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

fn form_body_col(form: &StockpileForm) -> Element<'_, Message> {
  column([
    text(form_title(form))
      .font(body::MEDIUM)
      .size(16.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().height(16.0).into(),
    form_name_fields(form),
    Space::new().height(16.0).into(),
    form_items_section(form),
  ])
  .spacing(0.0)
  .into()
}

fn stockpile_form_panel(form: &StockpileForm) -> Element<'_, Message> {
  let body_col = form_body_col(form);

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
      form_footer(form),
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

fn new_stockpile_btn() -> Element<'static, Message> {
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
  .into()
}

fn stockpiles_toolbar(ready_count: usize, short_count: usize) -> Element<'static, Message> {
  container(
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
      new_stockpile_btn(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 18.0,
    left: 0.0,
    right: 0.0,
  })
  .into()
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

    let toolbar = stockpiles_toolbar(ready_count, short_count);

    let grid: Element<'_, Message> = if state.stockpiles.is_empty() {
      empty_state()
    } else {
      let cards: Vec<Element<'_, Message>> = state.stockpiles.iter().map(stockpile_card).collect();
      scrollable(column(cards).spacing(14.0).width(Length::Fill))
        .height(Length::Fill)
        .into()
    };

    let content = container(column([toolbar, grid]).width(Length::Fill).height(Length::Fill))
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
