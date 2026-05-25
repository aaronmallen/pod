use iced::{
  Background, Border, Element, Length, Padding, Task,
  widget::{column, container, row, scrollable, text},
};

use crate::{
  components::{self, PopOver},
  style::{color, radius, typography},
};

const AVAILABLE_KEYS: &[&str] = &["tag", "corp", "clone", "loc", "status", "training", "name"];

const HELP_EXAMPLES: &[(&str, &str)] = &[
  ("tag:pvp", "has tag PvP"),
  ("tag:cruiser,frigate", "cruiser OR frigate"),
  ("tag:pvp tag:caldari", "pvp AND caldari"),
  ("-tag:alt", "NOT tagged alt"),
  ("corp:cobalt", "corp contains \"cobalt\""),
  ("loc:jita", "in Jita"),
  ("status:in-space", "undocked"),
  ("training:idle", "queue empty"),
];

#[derive(Clone, Debug, Default)]
pub struct State {
  pub visible: bool,
}

impl State {
  pub fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
      Message::Open => {
        self.visible = true;
      }
      Message::Close => {
        self.visible = false;
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
  Open,
  Close,
  QueryInserted(String),
}

pub struct Component<'a> {
  all_tags: &'a [(i32, String, Option<String>)],
}

impl<'a> Component<'a> {
  pub fn new(_state: &'a State, all_tags: &'a [(i32, String, Option<String>)]) -> Self {
    Self {
      all_tags,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let close_btn = close_button();
    let header = components::PanelHeader::new("QUERY SYNTAX").action(close_btn).render();
    let example_rows = build_example_rows();
    let keys_row = build_keys_row();
    let tag_chips = build_tag_chips(self.all_tags);

    let body_items = build_body_items(example_rows, keys_row, tag_chips, self.all_tags);
    let body = scrollable(
      container(column(body_items).spacing(8.0).width(iced::Length::Fill)).padding(Padding {
        top: 10.0,
        bottom: 14.0,
        left: 14.0,
        right: 14.0,
      }),
    )
    .width(iced::Length::Fill);

    PopOver::new(body)
      .header(header)
      .max_height(Length::Fixed(480.0))
      .width(Length::Fixed(360.0))
      .render()
  }
}

fn close_button() -> Element<'static, Message> {
  components::Button::close(text("\u{00D7}").font(typography::body::MEDIUM).size(16.0).style(|_| {
    iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    }
  }))
  .width(18.0)
  .height(18.0)
  .on_press(Message::Close)
  .into()
}

fn example_row(q: &'static str, note: &'static str) -> Element<'static, Message> {
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
  .width(iced::Length::Fill)
  .on_press(Message::QueryInserted(q.to_string()))
  .into()
}

fn build_example_rows() -> Vec<Element<'static, Message>> {
  HELP_EXAMPLES.iter().map(|&(q, note)| example_row(q, note)).collect()
}

fn build_keys_row() -> Vec<Element<'static, Message>> {
  AVAILABLE_KEYS
    .iter()
    .map(|&k| code_chip(&format!("{k}:"), false))
    .collect()
}

fn tag_chip_button(name: &str) -> Element<'static, Message> {
  let q = format!("tag:{name}");
  let q2 = q.clone();
  components::Button::close(code_chip(&q, false))
    .on_press(Message::QueryInserted(q2))
    .into()
}

fn build_tag_chips(all_tags: &[(i32, String, Option<String>)]) -> Vec<Element<'static, Message>> {
  all_tags.iter().map(|(_, name, _)| tag_chip_button(name)).collect()
}

fn build_body_items<'a>(
  example_rows: Vec<Element<'a, Message>>,
  keys_row: Vec<Element<'a, Message>>,
  tag_chips: Vec<Element<'a, Message>>,
  all_tags: &'a [(i32, String, Option<String>)],
) -> Vec<Element<'a, Message>> {
  let mut body_items: Vec<Element<'a, Message>> = Vec::new();
  body_items.push(section_label("EXAMPLES"));
  body_items.extend(example_rows);
  body_items.push(components::Separator::horizontal().render());
  body_items.push(section_label("AVAILABLE KEYS"));
  body_items.push(row(keys_row).spacing(4.0).wrap().into());
  if !all_tags.is_empty() {
    body_items.push(components::Separator::horizontal().render());
    body_items.push(section_label(format!("YOUR TAGS ({})", all_tags.len())));
    body_items.push(row(tag_chips).spacing(4.0).wrap().into());
  }
  body_items
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
