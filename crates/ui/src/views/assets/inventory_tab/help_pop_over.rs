use iced::{
  Background, Border, Element, Length, Padding, Task,
  widget::{column, container, row, scrollable, text},
};

use crate::{
  components::{self, PopOver},
  style::{color, radius, typography},
};

const HELP_EXAMPLES: &[(&str, &str)] = &[
  ("category:ship", "all ships"),
  ("region:\"The Forge\"", "in The Forge"),
  ("name:Tritanium", "name contains Tritanium"),
  ("category:ship -name:Rifter", "ships, not Rifters"),
  ("system:Jita type:stack", "stacks in Jita"),
  ("owner:me category:module", "my modules"),
];

const KEY_ENTRIES: &[(&str, &str, &str)] = &[
  ("name:", "n:", "type name (partial)"),
  ("group:", "g:", "group name (partial)"),
  ("category:", "cat:", "category key (exact)"),
  ("region:", "r:", "region name (exact)"),
  ("constellation:", "c:", "constellation (exact)"),
  ("system:", "s:", "system name (partial)"),
  ("location:", "loc:", "location name (partial)"),
  ("owner:", "", "character name or \"me\""),
  ("type:", "", "singleton  bpc  bpo  stack"),
];

#[derive(Clone, Debug, Default)]
pub struct State {
  pub visible: bool,
}

impl State {
  pub fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
      Message::Close => {
        self.visible = false;
      }
      Message::Open => {
        self.visible = true;
      }
      Message::QueryInserted(_) => {
        self.visible = false;
      }
    }
    Task::none()
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  Close,
  Open,
  QueryInserted(String),
}

pub struct Component;

impl Component {
  pub fn new() -> Self {
    Self
  }

  pub fn render(self) -> Element<'static, Message> {
    let close_btn = components::Button::close(text("\u{00D7}").font(typography::body::MEDIUM).size(16.0).style(|_| {
      iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }
    }))
    .width(18.0)
    .height(18.0)
    .on_press(Message::Close);

    let header = components::PanelHeader::new("FILTER SYNTAX").action(close_btn).render();

    let example_rows: Vec<Element<'static, Message>> = HELP_EXAMPLES
      .iter()
      .map(|&(q, note)| {
        components::Button::row(
          row([
            code_chip(q, true),
            text(note)
              .font(typography::body::REGULAR)
              .size(12.0)
              .style(|_| iced::widget::text::Style {
                color: Some(color::text::SECONDARY),
              })
              .into(),
          ])
          .spacing(12.0)
          .align_y(iced::alignment::Vertical::Center),
        )
        .width(Length::Fill)
        .on_press(Message::QueryInserted(q.to_string()))
        .into()
      })
      .collect();

    let key_rows: Vec<Element<'static, Message>> = KEY_ENTRIES
      .iter()
      .map(|&(key, alias, desc)| {
        let mut chips: Vec<Element<'static, Message>> = vec![code_chip(key, false)];
        if !alias.is_empty() {
          chips.push(code_chip(alias, false));
        }
        row([
          row(chips).spacing(4.0).width(Length::Fixed(130.0)).into(),
          text(desc)
            .font(typography::body::REGULAR)
            .size(12.0)
            .style(|_| iced::widget::text::Style {
              color: Some(color::text::SECONDARY),
            })
            .into(),
        ])
        .spacing(8.0)
        .align_y(iced::alignment::Vertical::Center)
        .into()
      })
      .collect();

    let mut body_items: Vec<Element<'static, Message>> = Vec::new();
    body_items.push(section_label("EXAMPLES"));
    body_items.extend(example_rows);
    body_items.push(components::Separator::horizontal().render());
    body_items.push(section_label("AVAILABLE KEYS"));
    body_items.extend(key_rows);

    let body = scrollable(
      container(column(body_items).spacing(8.0).width(Length::Fill)).padding(Padding {
        top: 10.0,
        bottom: 14.0,
        left: 14.0,
        right: 14.0,
      }),
    )
    .width(Length::Fill);

    PopOver::new(body)
      .header(header)
      .max_height(Length::Fixed(480.0))
      .width(Length::Fixed(360.0))
      .render()
  }
}

impl Default for Component {
  fn default() -> Self {
    Self::new()
  }
}

fn code_chip(label: &str, plasma: bool) -> Element<'static, Message> {
  let (bg, bd, fg) = if plasma {
    (
      color::accent::PLASMA_SUBTLE,
      color::accent::PLASMA_MUTED,
      color::accent::PLASMA,
    )
  } else {
    (
      color::state::HOVER_OVERLAY,
      color::border::SUBTLE,
      color::text::SECONDARY,
    )
  };
  container(
    text(label.to_owned())
      .font(typography::mono::MEDIUM)
      .size(11.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 6.0,
    right: 6.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: bd,
      width: 1.0,
      radius: radius::CHIP.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn section_label(title: impl ToString) -> Element<'static, Message> {
  text(title.to_string())
    .font(typography::mono::MEDIUM)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .into()
}
