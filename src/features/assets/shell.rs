use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Column, Row, Stack, container, scrollable},
};

use super::{
  HEADER_SIDE_PADDING, Message, Pane, Scope, State, Tab, abyssals, fmt_count, header, inventory, stockpiles, tracker,
  tree, values,
};
use crate::ui::{
  components::{
    backdrop,
    resizable_pane::pane_handle,
    rule,
    tab_select::{self, TabLayout},
  },
  style::{color, spacing},
};
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const HELP_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 96.0;
const TAB_STRIP_HEIGHT: f32 = 48.0;

pub(super) fn shell(state: &State, _now: DateTime<Utc>) -> Element<'_, Message> {
  let body = Column::with_children(vec![header::header(state), self::body(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  let base = container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  if state.picker_open {
    let dropdown = container(header::picker_dropdown(state)).padding(Padding {
      top: PICKER_OVERLAY_TOP,
      left: PICKER_OVERLAY_LEFT,
      ..Padding::ZERO
    });

    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::PickerToggled),
      dropdown.into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  if state.tab() == Tab::Inventory && state.inventory_help_open() {
    let popover = container(
      Row::with_children(vec![
        iced::widget::Space::new().width(Length::Fill).into(),
        inventory::help_popover(),
      ])
      .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: HELP_OVERLAY_TOP,
      right: HEADER_SIDE_PADDING,
      ..Padding::ZERO
    });

    return Stack::with_children(vec![
      base.into(),
      backdrop::click_catcher(Message::InventoryHelpToggled),
      popover.into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  if state.tab() == Tab::Abyssals && state.abyssal_picker_open() {
    return Stack::with_children(vec![
      base.into(),
      backdrop::backdrop(Message::AbyssalPickerToggled),
      abyssals::picker_modal(state),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  let stockpile_menu = (state.tab() == Tab::Stockpiles)
    .then(|| state.stockpile_context_menu())
    .flatten();
  if let Some(menu) = stockpile_menu {
    return Stack::with_children(vec![
      base.into(),
      backdrop::backdrop(Message::StockpileContextMenuClosed),
      stockpiles::context_menu_view(menu),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  base.into()
}

fn body(state: &State) -> Element<'_, Message> {
  Column::with_children(vec![tab_strip(state), tab_body(state)])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn location_tree(state: &State) -> Element<'_, Message> {
  container(tree::pane(state))
    .width(Length::Fixed(state.pane(Pane::Sidebar).width()))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn tab_strip(state: &State) -> Element<'_, Message> {
  let tabs = vec![
    tab(state, Tab::Inventory, "Inventory", tab_count(state.inventory().len())),
    tab(state, Tab::Abyssals, "Abyssals", tab_count(state.abyssals().len())),
    tab(
      state,
      Tab::Stockpiles,
      "Stockpiles",
      tab_count(state.stockpiles().len()),
    ),
    tab(state, Tab::Values, "Values", values_count(state)),
    tab(state, Tab::Tracker, "Tracker", String::new()),
  ];

  let strip = container(tab_select::tab_select_with(tabs, TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: HEADER_SIDE_PADDING,
      bottom: 0.0,
      left: HEADER_SIDE_PADDING,
    });

  Column::with_children(vec![strip.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn tab<'a>(state: &State, tab: Tab, label: &'a str, count: String) -> tab_select::Tab<'a, Message> {
  let selected = state.tab() == tab;
  tab_select::Tab {
    count,
    label,
    on_press: (!selected).then_some(Message::TabSelected(tab)),
    selected,
  }
}

fn tab_count(len: usize) -> String {
  fmt_count(len as i64)
}

fn values_count(state: &State) -> String {
  let count = match state.active() {
    Scope::All => state.roster().len() as i64,
    Scope::Character(_) => 1,
    Scope::Corporation(_) => 0,
  };
  fmt_count(count)
}

fn tab_body(state: &State) -> Element<'_, Message> {
  match state.tab() {
    Tab::Inventory => inventory_body(state),
    Tab::Abyssals => abyssals_body(state),
    Tab::Stockpiles => iced::widget::mouse_area(
      container(stockpiles::body(
        state.stockpiles(),
        state.stockpile_editor(),
        state.stockpile_import(),
      ))
      .width(Length::Fill)
      .height(Length::Fill),
    )
    .on_move(Message::StockpileCursorMoved)
    .into(),
    Tab::Values => container(
      scrollable(values::body(state.values()))
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into(),
    Tab::Tracker => container(
      scrollable(tracker::body(state.nav()))
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into(),
  }
}

fn inventory_body(state: &State) -> Element<'_, Message> {
  let inventory = Column::with_children(vec![
    inventory::filter_bar(state),
    container(
      scrollable(inventory::body(state))
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_scroll(|viewport| Message::InventoryScrolled(viewport.relative_offset().y)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  Row::with_children(vec![
    location_tree(state),
    pane_handle(Message::PaneDragStart(Pane::Sidebar)),
    inventory.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn abyssals_body(state: &State) -> Element<'_, Message> {
  let rail = container(abyssals::filter_rail(state))
    .width(Length::Fixed(state.pane(Pane::AbyssalsFilter).width()))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  let cards: Vec<&abyssals::AbyssalCard> = state.abyssals().iter().take(state.abyssal_visible_count()).collect();
  let any_owned = !state.abyssals().is_empty() || !state.abyssal_source_types().is_empty();
  let results = container(
    scrollable(abyssals::body(&cards, any_owned))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::AbyssalGridScrolled(viewport.relative_offset().y)),
  )
  .width(Length::Fill)
  .height(Length::Fill);

  Row::with_children(vec![
    rail.into(),
    pane_handle(Message::PaneDragStart(Pane::AbyssalsFilter)),
    results.into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}
