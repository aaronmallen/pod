use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, scrollable, text},
};

use super::{Message, ObjectiveView, PilotRef, State, ui};
use crate::{
  store::model::ObjectiveStatus,
  ui::{
    components::{
      button::Button,
      eyebrow::eyebrow,
      icon::Icon,
      tab_select::{Tab, TabLayout, tab_select_with},
      virtual_list::{self, VirtualList, VirtualListConfig},
    },
    style::{color, control, radius, spacing, typography},
  },
};

const BANNER_ICON_TILE: f32 = 42.0;
const CARD_COLUMNS: usize = 2;
const ESTIMATED_ROW_HEIGHT: f32 = 184.0;
const CARD_ICON_TILE: f32 = 30.0;
const PILOT_CHIP_SIZE: f32 = 24.0;

pub(super) fn view(state: &State) -> Element<'_, Message> {
  if state.objectives.is_empty() {
    return Column::with_children(vec![banner(), empty_board()])
      .spacing(spacing::SPACE_6)
      .width(Length::Fill)
      .into();
  }

  Column::with_children(vec![banner(), tabs(state), tab_body(state)])
    .spacing(spacing::SPACE_4_5)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn banner<'a>() -> Element<'a, Message> {
  let identity = ui::identity();
  let tile = container(Icon::chevrons_up().size(24.0).color(identity).render())
    .width(Length::Fixed(BANNER_ICON_TILE))
    .height(Length::Fixed(BANNER_ICON_TILE))
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(identity, 0.14))),
      border: Border {
        color: color::with_alpha(identity, 0.45),
        width: 1.0,
        radius: radius::NAV_CARD.into(),
      },
      ..container::Style::default()
    });

  let copy = Column::with_children(vec![
    eyebrow(&t!("standing_orders.eyebrow"), Some(identity)),
    text(t!("standing_orders.banner.body").into_owned())
      .font(typography::body::REGULAR)
      .size(15.0)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .width(Length::Fill);

  let new_button: Element<'a, Message> = Button::primary(t!("standing_orders.action.new"))
    .icon(Icon::plus())
    .on_press(Message::NewPressed)
    .into();

  let row = Row::with_children(vec![tile.into(), copy.into(), new_button])
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 20.0,
      right: 24.0,
      bottom: 20.0,
      left: 24.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(identity, 0.4),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn tabs(state: &State) -> Element<'_, Message> {
  let descriptors = ObjectiveStatus::ALL
    .into_iter()
    .map(|status| Tab {
      count: state.count_of(status).to_string(),
      icon: None,
      label: tr_static(tab_key(status)),
      on_press: Some(Message::TabSelected(status)),
      selected: state.tab == status,
    })
    .collect();

  container(tab_select_with(descriptors, TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(42.0))
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 0.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn tab_body(state: &State) -> Element<'_, Message> {
  let items = state.with_status(state.tab);
  if items.is_empty() {
    return tab_empty();
  }
  windowed_grid(state, items)
}

fn windowed_grid<'a>(state: &'a State, items: Vec<&'a ObjectiveView>) -> Element<'a, Message> {
  let roster = &state.roster;
  let offset = state.scroll_offset;
  let total = items.len();
  virtual_list::responsive_window(move |viewport_height| {
    let base = || {
      VirtualListConfig::new(total, ESTIMATED_ROW_HEIGHT)
        .items_per_row(CARD_COLUMNS)
        .viewport_height(viewport_height)
    };
    let config = base().scroll_offset(offset.min(base().max_scroll_offset()));
    let list = VirtualList::new(config, |row| grid_row(&items, roster, row))
      .spacing(spacing::SPACE_3_5)
      .view();
    scrollable(list)
      .style(control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
      .into()
  })
}

fn grid_row(items: &[&ObjectiveView], roster: &[PilotRef], row: usize) -> Element<'static, Message> {
  let start = row * CARD_COLUMNS;
  let mut cells: Vec<Element<'static, Message>> = Vec::with_capacity(CARD_COLUMNS);
  for col in 0..CARD_COLUMNS {
    match items.get(start + col) {
      Some(view) => cells.push(card(view, roster)),
      None => cells.push(Space::new().width(Length::Fill).into()),
    }
  }
  Row::with_children(cells)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn card(view: &ObjectiveView, roster: &[PilotRef]) -> Element<'static, Message> {
  let status = view.status();
  let accent = ui::accent_color(&view.model.accent);
  let id = view.model.id;

  let mut head: Vec<Element<'static, Message>> = vec![ui::target_tile(accent, CARD_ICON_TILE, 17.0)];
  head.push(card_title_block(view, accent));
  head.push(ui::status_stamp(status));

  let mut body: Vec<Element<'static, Message>> = vec![
    Row::with_children(head)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Top)
      .into(),
  ];

  if let Some(why) = view.model.why.as_deref().filter(|value| !value.trim().is_empty()) {
    body.push(
      text(why.to_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }

  if let Some(target) = view.model.target.as_deref().filter(|value| !value.trim().is_empty()) {
    body.push(target_row(target, accent));
  }

  body.push(card_footer(view, roster));

  let inner = container(
    Column::with_children(body)
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 15.0,
    right: 18.0,
    bottom: 14.0,
    left: 16.0,
  });

  let stripe = container(
    container(Space::new())
      .width(Length::Fixed(4.0))
      .height(Length::Fill)
      .style(move |_| container::Style {
        background: Some(Background::Color(accent)),
        ..container::Style::default()
      }),
  )
  .align_x(Horizontal::Left);

  button(Stack::with_children(vec![inner.into(), stripe.into()]))
    .padding(Padding::ZERO)
    .width(Length::Fill)
    .on_press(Message::OpenObjective(id))
    .style(card_style)
    .into()
}

fn card_title_block(view: &ObjectiveView, accent: Color) -> Element<'static, Message> {
  let mut children: Vec<Element<'static, Message>> = vec![
    text(view.model.title.clone())
      .font(typography::body::MEDIUM)
      .size(17.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];

  if let Some(horizon) = view.model.horizon.as_deref().filter(|value| !value.trim().is_empty()) {
    children.push(
      text(horizon.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(move |_| text::Style {
          color: Some(color::with_alpha(accent, 0.9)),
        })
        .into(),
    );
  }

  Column::with_children(children)
    .spacing(spacing::UNIT + 1.0)
    .width(Length::Fill)
    .into()
}

fn target_row(target: &str, accent: Color) -> Element<'static, Message> {
  let row = Row::with_children(vec![
    Icon::tack().size(13.0).color(accent).render(),
    text(target.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding([7.0, 11.0])
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn card_footer(view: &ObjectiveView, roster: &[PilotRef]) -> Element<'static, Message> {
  let faces: Vec<Element<'static, Message>> = view
    .pilots
    .iter()
    .map(|id| ui::pilot_face(roster, *id, PILOT_CHIP_SIZE))
    .collect();

  let count = view.thread.len();
  let tint = if count > 0 {
    ui::identity()
  } else {
    color::text::tertiary()
  };
  let checkins = Row::with_children(vec![
    Icon::journal().size(12.0).color(tint).render(),
    text(ui::checkin_label(count))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_y(Vertical::Center);

  Row::with_children(vec![
    Row::with_children(faces).spacing(spacing::UNIT + 1.0).into(),
    Space::new().width(Length::Fill).into(),
    checkins.into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn card_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: if hovered { color::rule_strong() } else { color::rule() },
      width: 1.0,
      radius: radius::CARD.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn empty_board<'a>() -> Element<'a, Message> {
  let identity = ui::identity();
  let content = Column::with_children(vec![
    Icon::tracker().size(30.0).color(identity).render(),
    text(t!("standing_orders.empty.title").into_owned())
      .font(typography::body::MEDIUM)
      .size(16.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("standing_orders.empty.subtitle").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
    Button::primary(t!("standing_orders.action.new"))
      .icon(Icon::plus())
      .on_press(Message::NewPressed)
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_x(iced::alignment::Horizontal::Center)
  .width(Length::Fill);

  dashed_panel(content.into(), 40.0)
}

fn tab_empty<'a>() -> Element<'a, Message> {
  let content = text(t!("standing_orders.empty.filtered").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::tertiary()));
  dashed_panel(content.into(), 32.0)
}

fn dashed_panel<'a>(content: Element<'a, Message>, vertical: f32) -> Element<'a, Message> {
  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: vertical,
      right: 24.0,
      bottom: vertical,
      left: 24.0,
    })
    .align_x(iced::alignment::Horizontal::Center)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn tab_key(status: ObjectiveStatus) -> &'static str {
  match status {
    ObjectiveStatus::Active => "standing_orders.tab.active",
    ObjectiveStatus::Complete => "standing_orders.tab.complete",
    ObjectiveStatus::Cancelled => "standing_orders.tab.cancelled",
  }
}

fn tr_static(key: &str) -> &'static str {
  static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
  let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));
  if let Some(&interned) = cache.read().expect("standing orders i18n cache poisoned").get(key) {
    return interned;
  }

  let resolved: &'static str = Box::leak(t!(key).into_owned().into_boxed_str());
  cache
    .write()
    .expect("standing orders i18n cache poisoned")
    .entry(key.to_owned())
    .or_insert(resolved)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::model::Objective;

  fn view(populated: bool) -> ObjectiveView {
    ObjectiveView {
      model: Objective {
        accent: "#D9B252".to_owned(),
        cancelled_at: None,
        completed_at: None,
        created_at: "2026-07-10".to_owned(),
        horizon: populated.then(|| "Q3".to_owned()),
        id: 1,
        status: "active".to_owned(),
        target: populated.then(|| "Run industry jobs".to_owned()),
        title: "Become an Industrialist".to_owned(),
        why: populated.then(|| "Make ISK off industry".to_owned()),
      },
      pilots: Vec::new(),
      thread: Vec::new(),
      orders: Vec::new(),
    }
  }

  #[test]
  fn it_renders_a_card_with_and_without_optional_fields() {
    let _: Element<'static, Message> = card(&view(true), &[]);
    let _: Element<'static, Message> = card(&view(false), &[]);
  }

  #[test]
  fn it_builds_a_grid_row_padding_the_trailing_column() {
    let full = view(true);
    let items = vec![&full];
    let _: Element<'static, Message> = grid_row(&items, &[], 0);
  }
}
