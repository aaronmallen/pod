use std::collections::HashSet;

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Horizontal,
  widget::{button, column, container, mouse_area, row, scrollable, text, text_input},
};

use crate::{
  components,
  style::{color, radius, shadow, spacing, typography},
};

pub struct State {
  pub entity_id: i64,
  pub entity_name: String,
  pub entity_type: String,
  pub existing_tags: Vec<(i32, String, Option<String>)>,
  pub highlighted: usize,
  pub input_id: iced::widget::Id,
  pub query: String,
}

impl State {
  pub fn new(
    entity_id: i64,
    entity_type: impl Into<String>,
    entity_name: impl Into<String>,
    existing_tags: Vec<(i32, String, Option<String>)>,
  ) -> Self {
    Self {
      entity_id,
      entity_name: entity_name.into(),
      entity_type: entity_type.into(),
      existing_tags,
      highlighted: 0,
      input_id: iced::widget::Id::unique(),
      query: String::new(),
    }
  }
}

#[derive(Clone, Debug)]
pub enum Message {
  Close,
  CommitHighlighted,
  Confirm(String),
  Highlighted(usize),
  MoveDown,
  MoveUp,
  QueryChanged(String),
  Remove(i32),
}

pub struct Item {
  pub is_create: bool,
  pub name: String,
  pub count: Option<usize>,
}

pub fn compute_items(state: &State, corpus: &[(String, usize)]) -> Vec<Item> {
  let trimmed = state.query.trim().to_string();
  let lc = trimmed.to_lowercase();
  let existing_lower = existing_tags_lowercase(&state.existing_tags);
  let filtered = filter_corpus(corpus, &existing_lower, &lc);
  filtered_to_items(filtered, &trimmed, &lc)
}

fn existing_tags_lowercase(tags: &[(i32, String, Option<String>)]) -> HashSet<String> {
  tags.iter().map(|(_, n, _)| n.to_lowercase()).collect()
}

fn filter_corpus(corpus: &[(String, usize)], existing_lower: &HashSet<String>, lc: &str) -> Vec<(String, usize)> {
  corpus
    .iter()
    .filter(|(name, _)| {
      let n = name.to_lowercase();
      !existing_lower.contains(&n) && (lc.is_empty() || n.contains(lc))
    })
    .cloned()
    .collect()
}

fn filtered_to_items(filtered: Vec<(String, usize)>, trimmed: &str, lc: &str) -> Vec<Item> {
  let exact = filtered.iter().any(|(name, _)| name.to_lowercase() == lc);
  let can_create = !trimmed.is_empty() && !exact;
  let mut items: Vec<Item> = filtered
    .into_iter()
    .map(|(n, c)| Item {
      is_create: false,
      name: n,
      count: Some(c),
    })
    .collect();
  if can_create {
    items.push(Item {
      is_create: true,
      name: trimmed.to_string(),
      count: None,
    });
  }
  items
}

pub struct Component<'a> {
  state: &'a State,
  corpus: Vec<(String, usize)>,
  window_width: f32,
  window_height: f32,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State, corpus: Vec<(String, usize)>) -> Self {
    Self {
      state,
      corpus,
      window_width: spacing::layout::WINDOW_DEFAULT_WIDTH,
      window_height: spacing::layout::WINDOW_DEFAULT_HEIGHT,
    }
  }

  pub fn window_size(mut self, w: f32, h: f32) -> Self {
    self.window_width = w;
    self.window_height = h;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let items = compute_items(self.state, &self.corpus);
    let highlighted = resolve_highlighted(self.state.highlighted, items.len());

    let dialog_width = (self.window_width - 48.0).min(440.0);
    let max_height = (self.window_height - 48.0).min(560.0);

    let children = build_dialog_children(self.state, items, highlighted);

    let dialog: Element<'_, Message> = container(column(children))
      .width(Length::Fixed(dialog_width))
      .max_height(max_height)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::DEFAULT,
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        shadow: shadow::POPOVER,
        ..container::Style::default()
      })
      .into();

    container(dialog)
      .align_x(Horizontal::Center)
      .center_y(Length::Fill)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }
}

fn build_dialog_children<'a>(state: &'a State, items: Vec<Item>, highlighted: usize) -> Vec<Element<'a, Message>> {
  let header = render_modal_header(state);
  let input_area = render_input_area(state);
  let list_area = render_list_area(items, highlighted, state);
  let footer = render_modal_footer();
  let existing_section = render_existing_tags(state);

  let mut children: Vec<Element<'_, Message>> = vec![header, components::Separator::horizontal().render()];
  if let Some(existing) = existing_section {
    children.push(existing);
    children.push(components::Separator::horizontal().render());
  }
  children.push(input_area);
  children.push(components::Separator::horizontal().render());
  children.push(list_area);
  children.push(components::Separator::horizontal().render());
  children.push(footer);
  children
}

fn input_pill<'a>(state: &'a State) -> Element<'a, Message> {
  let plus: Element<'a, Message> = text("+")
    .size(16.0)
    .font(typography::body::MEDIUM)
    .style(|_| text::Style {
      color: Some(color::accent::PLASMA),
    })
    .into();

  let input_widget = text_input("Search or create a tag\u{2026}", &state.query)
    .id(state.input_id.clone())
    .on_input(Message::QueryChanged)
    .font(typography::body::REGULAR)
    .size(14.0)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::state::SELECTION,
    })
    .padding(Padding::ZERO);

  container(
    row([plus, input_widget.into()])
      .spacing(spacing::SPACE_2_5)
      .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .align_y(iced::alignment::Vertical::Center)
  .height(38.0)
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::accent::PLASMA,
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn item_kind_badge(is_create: bool) -> Element<'static, Message> {
  let kind_color = if is_create {
    color::accent::PLASMA
  } else {
    color::text::TERTIARY
  };
  let kind_label = if is_create { "NEW" } else { "TAG" };

  container(
    text(kind_label)
      .font(typography::mono::REGULAR)
      .size(9.0)
      .style(move |_| text::Style {
        color: Some(kind_color),
      }),
  )
  .width(28.0)
  .into()
}

fn item_name_label(item: &Item) -> Element<'static, Message> {
  let display = if item.is_create {
    format!("Create \"{}\"", item.name)
  } else {
    item.name.clone()
  };

  container(
    text(display)
      .font(typography::body::REGULAR)
      .size(14.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fill)
  .into()
}

fn item_row_content(item: Item) -> iced::widget::Row<'static, Message> {
  let kind_el = item_kind_badge(item.is_create);
  let name_el = item_name_label(&item);

  let mut row_children: Vec<Element<'static, Message>> = vec![kind_el, name_el];

  if let Some(count) = item.count {
    row_children.push(
      text(count.to_string())
        .font(typography::mono::REGULAR)
        .size(11.0)
        .style(|_| text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    );
  }

  row(row_children)
    .spacing(spacing::SPACE_3)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
}

fn render_existing_tags<'a>(state: &'a State) -> Option<Element<'a, Message>> {
  if state.existing_tags.is_empty() {
    return None;
  }

  let chips: Vec<Element<'a, Message>> = state
    .existing_tags
    .iter()
    .map(|(tag_id, name, _)| render_tag_chip(*tag_id, name))
    .collect();

  Some(
    container(
      column([
        text("CURRENT TAGS")
          .font(typography::mono::REGULAR)
          .size(9.0)
          .style(|_| text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        row(chips).spacing(spacing::SPACE_1).wrap().into(),
      ])
      .spacing(spacing::SPACE_2),
    )
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into(),
  )
}

fn render_input_area(state: &State) -> Element<'_, Message> {
  let input_inner = input_pill(state);

  container(input_inner)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn render_item<'a>(i: usize, item: Item, is_highlighted: bool) -> Element<'a, Message> {
  let tag_name = item.name.clone();
  let inner = item_row_content(item);

  let btn = button(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: 9.0,
      bottom: 9.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::Confirm(tag_name))
    .style(move |_, status| {
      let active = is_highlighted || matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: if active {
          Some(Background::Color(color::accent::PLASMA_SELECTED))
        } else {
          None
        },
        border: Border {
          radius: radius::CHIP.into(),
          ..Border::default()
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      }
    });

  mouse_area(btn).on_enter(Message::Highlighted(i)).into()
}

fn render_list_area<'a>(items: Vec<Item>, highlighted: usize, state: &State) -> Element<'a, Message> {
  let list_body: Element<'a, Message> = if items.is_empty() {
    let msg = if !state.existing_tags.is_empty() && state.query.trim().is_empty() {
      "All tags already applied"
    } else {
      "Type to create a new tag"
    };
    container(
      text(msg)
        .font(typography::mono::REGULAR)
        .size(10.0)
        .style(|_| text::Style {
          color: Some(color::text::TERTIARY),
        }),
    )
    .padding(Padding {
      top: spacing::SPACE_7,
      bottom: spacing::SPACE_7,
      left: 0.0,
      right: 0.0,
    })
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .into()
  } else {
    let rows: Vec<Element<'a, Message>> = items
      .into_iter()
      .enumerate()
      .map(|(i, item)| render_item(i, item, i == highlighted))
      .collect();

    scrollable(column(rows).width(Length::Fill)).height(220.0).into()
  };

  container(list_body).padding(6.0).width(Length::Fill).into()
}

fn render_modal_footer<'a>() -> Element<'a, Message> {
  container(
    text("\u{2191}\u{2193} navigate  \u{00b7}  \u{21b5} apply  \u{00b7}  esc close")
      .font(typography::mono::REGULAR)
      .size(9.0)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .width(Length::Fill)
  .into()
}

fn render_modal_header(state: &State) -> Element<'_, Message> {
  container(
    row([
      column([
        text("ADD TAG")
          .font(typography::mono::REGULAR)
          .size(9.0)
          .style(|_| text::Style {
            color: Some(color::text::SECONDARY),
          })
          .into(),
        text(state.entity_name.clone())
          .font(typography::body::MEDIUM)
          .size(16.0)
          .style(|_| text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
      ])
      .spacing(spacing::SPACE_1)
      .into(),
      iced::widget::Space::new().width(Length::Fill).into(),
      button(
        text("×")
          .size(20.0)
          .font(typography::body::REGULAR)
          .style(|_| text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .on_press(Message::Close)
      .padding(0)
      .style(|_, _| button::Style::default())
      .into(),
    ])
    .align_y(iced::alignment::Vertical::Top),
  )
  .padding(Padding {
    top: spacing::SPACE_4,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_5,
    right: spacing::SPACE_4,
  })
  .width(Length::Fill)
  .into()
}

fn render_tag_chip<'a>(tag_id: i32, name: &str) -> Element<'a, Message> {
  button(
    row([
      text(name.to_string())
        .font(typography::body::MEDIUM)
        .size(11.0)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      text("×")
        .font(typography::body::REGULAR)
        .size(13.0)
        .style(|_| text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
    ])
    .spacing(4.0)
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 6.0,
  })
  .on_press(Message::Remove(tag_id))
  .style(|_, status| button::Style {
    background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
      Some(Background::Color(color::status::DANGER_SUBTLE))
    } else {
      Some(Background::Color(color::state::TAG_FILL))
    },
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..button::Style::default()
  })
  .into()
}

fn resolve_highlighted(highlighted: usize, item_count: usize) -> usize {
  if item_count > 0 {
    highlighted.min(item_count - 1)
  } else {
    0
  }
}
