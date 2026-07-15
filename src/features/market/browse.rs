use iced::{
  Background, Border, Color, ContentFit, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, image, scrollable, text},
};

use super::{
  Message, State,
  i18n::tr_static,
  tree::{self, MarketLeaf, MarketNode},
};
use crate::{
  clients::eve_image::Size,
  store::images::{self, IconResolution},
  ui::{
    components::{
      clip::clip_layer, icon::Icon, icon_tile::icon_tile, resizable_pane::pane_handle, text_input::TextInput,
    },
    format::fmt_isk_opt,
    style::{
      color,
      control::{scrollbar, sunken_pane},
      spacing, typography,
    },
  },
};

const ICON_SIZE: Size = Size::S64;
const BASE_INDENT: f32 = spacing::SPACE_3;
const INDENT_STEP: f32 = 18.0;
const CARET_SLOT: f32 = 12.0;
const CARET_ICON: f32 = 10.0;
const LEAF_ICON: f32 = 18.0;
const RAIL_WIDTH: f32 = 2.0;

pub(super) fn surface(state: &State) -> iced::Element<'_, Message> {
  Row::with_children(vec![
    tree_pane(state),
    pane_handle(Message::PaneDragStart),
    super::book_view::detail(state),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn tree_pane(state: &State) -> iced::Element<'_, Message> {
  let column = Column::with_children(vec![filter_bar(state), catalog(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  container(column)
    .width(Length::Fixed(state.tree_pane_width()))
    .height(Length::Fill)
    .style(sunken_pane)
    .into()
}

fn filter_bar(state: &State) -> iced::Element<'_, Message> {
  let mut field = TextInput::new(
    tr_static("market.filter_placeholder"),
    state.filter(),
    Message::FilterChanged,
  )
  .leading_icon(Icon::search())
  .background(color::surface::BASE)
  .font_size(typography::size::MD)
  .width(Length::Fill);

  if !state.filter().is_empty() {
    field = field.trailing(clear_button());
  }

  container(field.render())
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn clear_button<'a>() -> iced::Element<'a, Message> {
  button(
    container(Icon::close().size(13.0).color(color::text::secondary()).render())
      .width(Length::Fixed(22.0))
      .height(Length::Fixed(22.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(Message::FilterChanged(String::new()))
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then_some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
      border: Border {
        color: Color::TRANSPARENT,
        radius: 999.0.into(),
        width: 0.0,
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  })
  .into()
}

fn catalog(state: &State) -> iced::Element<'_, Message> {
  let query = state.filter();
  let searching = !query.trim().is_empty();
  let store = images::default_store();

  let mut rows: Vec<iced::Element<'_, Message>> = Vec::new();
  rows.push(rule_divider());

  if searching {
    let filtered = tree::filter_tree(state.tree(), query);
    for node in &filtered.roots {
      push_node(state, &store, &mut rows, node, 0, true);
    }
    if filtered.roots.is_empty() {
      return notice("market.filter_no_results");
    }
  } else if state.tree().roots.is_empty() {
    return notice("market.tree_empty");
  } else {
    for node in &state.tree().roots {
      push_node(state, &store, &mut rows, node, 0, false);
    }
  }

  rows.push(rule_divider());

  scrollable(Column::with_children(rows).width(Length::Fill))
    .style(scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn push_node<'a>(
  state: &State,
  store: &images::Store,
  rows: &mut Vec<iced::Element<'a, Message>>,
  node: &MarketNode,
  depth: usize,
  searching: bool,
) {
  let expanded = searching || state.is_expanded(node.id);
  rows.push(branch_row(node, depth, expanded));

  if !expanded {
    return;
  }

  for child in &node.children {
    push_node(state, store, rows, child, depth + 1, searching);
  }
  for leaf in &node.items {
    rows.push(leaf_row(state, store, leaf, depth + 1));
  }
}

fn branch_row<'a>(node: &MarketNode, depth: usize, expanded: bool) -> iced::Element<'a, Message> {
  let chevron = if expanded {
    Icon::chevron()
  } else {
    Icon::chevron_right()
  };
  let caret = container(chevron.size(CARET_ICON).color(color::text::secondary()).render())
    .width(Length::Fixed(CARET_SLOT))
    .align_x(Horizontal::Center);

  let name_font = if depth == 0 {
    typography::body::MEDIUM
  } else {
    typography::body::REGULAR
  };
  let name_color = if depth == 0 {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };

  let content = Row::with_children(vec![
    caret.into(),
    text(node.name.clone())
      .font(name_font)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(name_color))
      .width(Length::Fill)
      .into(),
    count_label(node.item_count),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  button(Row::with_children(vec![rail(false), body(content, depth)]).align_y(Vertical::Center))
    .width(Length::Fill)
    .on_press(Message::NodeToggled(node.id))
    .style(move |_, status| branch_style(status))
    .into()
}

fn leaf_row<'a>(state: &State, store: &images::Store, leaf: &MarketLeaf, depth: usize) -> iced::Element<'a, Message> {
  let selected = state.selected_type_id() == Some(leaf.type_id);
  let name_color = if selected {
    color::accent()
  } else {
    color::text::PRIMARY
  };

  let content = Row::with_children(vec![
    leaf_icon(store, leaf.type_id),
    text(leaf.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(typography::colored(name_color))
      .width(Length::Fill)
      .into(),
    price_label(leaf.best_sell),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  button(Row::with_children(vec![rail(selected), body(content, depth)]).align_y(Vertical::Center))
    .width(Length::Fill)
    .on_press(Message::ItemSelected(leaf.type_id))
    .style(move |_, status| leaf_style(selected, status))
    .into()
}

fn body<'a>(content: Row<'a, Message>, depth: usize) -> iced::Element<'a, Message> {
  container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT + 3.0,
      right: spacing::SPACE_3,
      bottom: spacing::UNIT + 3.0,
      left: BASE_INDENT + depth as f32 * INDENT_STEP,
    })
    .into()
}

fn rail<'a>(selected: bool) -> iced::Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(if selected {
        color::accent()
      } else {
        Color::TRANSPARENT
      })),
      ..container::Style::default()
    })
    .into()
}

fn leaf_icon<'a>(store: &images::Store, type_id: i64) -> iced::Element<'a, Message> {
  let content: iced::Element<'a, Message> = match store.resolve_type_icon(type_id, None, ICON_SIZE) {
    IconResolution::Found(path) => clip_layer(
      image(image::Handle::from_path(path))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
      Length::Fill,
      Length::Fill,
    ),
    IconResolution::Missing => Space::new().into(),
  };
  icon_tile(content, LEAF_ICON)
}

fn count_label<'a>(count: usize) -> iced::Element<'a, Message> {
  text(count.to_string())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn price_label<'a>(best_sell: Option<f64>) -> iced::Element<'a, Message> {
  text(fmt_isk_opt(best_sell))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn rule_divider<'a>() -> iced::Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn notice(key: &str) -> iced::Element<'static, Message> {
  container(
    text(t!(key).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_4_5)
  .into()
}

fn branch_style(status: button::Status) -> button::Style {
  let background = matches!(status, button::Status::Hovered)
    .then_some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.03)));
  button::Style {
    background,
    border: Border {
      radius: 0.0.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn leaf_style(selected: bool, status: button::Status) -> button::Style {
  let background = if selected {
    Some(Background::Color(color::with_alpha(color::accent(), 0.12)))
  } else if matches!(status, button::Status::Hovered) {
    Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
  } else {
    None
  };
  button::Style {
    background,
    border: Border {
      radius: 0.0.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::model::{ItemType, MarketGroup};

  fn sample_state() -> State {
    let groups = vec![
      MarketGroup {
        description: String::new(),
        has_types: false,
        icon_id: None,
        id: 1,
        name: "Ships".to_owned(),
        parent_id: None,
      },
      MarketGroup {
        description: String::new(),
        has_types: false,
        icon_id: None,
        id: 2,
        name: "Frigates".to_owned(),
        parent_id: Some(1),
      },
    ];
    let items = vec![ItemType {
      capacity: None,
      description: None,
      dogma_attributes: "[]".to_owned(),
      group_id: 0,
      icon_id: None,
      id: 587,
      market_group_id: Some(2),
      name: "Rifter".to_owned(),
      packaged_volume: None,
      portion_size: None,
      published: true,
      radius: None,
      volume: None,
    }];

    let mut state = State::new();
    super::super::update(
      &mut state,
      Message::TreeLoaded(Box::new(tree::build_market_tree(&groups, &items))),
    );
    state
  }

  #[test]
  fn it_renders_the_collapsed_catalog() {
    let state = sample_state();
    let _el: iced::Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_the_expanded_catalog_with_a_selection() {
    let mut state = sample_state();
    super::super::update(&mut state, Message::NodeToggled(1));
    super::super::update(&mut state, Message::NodeToggled(2));
    super::super::update(&mut state, Message::ItemSelected(587));

    let _el: iced::Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_a_filtered_catalog() {
    let mut state = sample_state();
    super::super::update(&mut state, Message::FilterChanged("rifter".to_owned()));

    let _el: iced::Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_the_no_results_notice() {
    let mut state = sample_state();
    super::super::update(&mut state, Message::FilterChanged("titan".to_owned()));

    let _el: iced::Element<'_, Message> = surface(&state);
  }

  #[test]
  fn it_renders_the_empty_catalog_notice() {
    let state = State::new();
    let _el: iced::Element<'_, Message> = surface(&state);
  }
}
