use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use iced::{
  Background, Element, Length, Padding,
  widget::{Column, Row, container, scrollable},
};

use super::{
  HEADER_SIDE_PADDING, Message, Pane, Scope, State, Tab, abyssals, fmt_count, header, inventory, stockpiles, tracker,
  tree, values,
};
use crate::ui::{
  components::{
    add_tag_modal, backdrop, context_menu, forbidden,
    icon::Icon,
    modal_overlay::{modal_layers, stable_overlay},
    positioned_dropdown::positioned_dropdown,
    resizable_pane::pane_handle,
    rule,
    tab_select::{self, TabLayout},
  },
  style::{color, control, spacing},
};
const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const HELP_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 96.0;
const TAB_STRIP_HEIGHT: f32 = 48.0;

pub(super) fn shell(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let body = Column::with_children(vec![header::header(state), self::body(state, now)])
    .width(Length::Fill)
    .height(Length::Fill);

  let surface = container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  // On the Inventory tab the feature-root base is wrapped in a cursor-tracking `mouse_area` so a
  // right-click can anchor its context menu at the pointer: `mouse_area`'s `on_move` reports the
  // cursor relative to its own bounds, so tracking at the base (which shares the overlay `Stack`'s
  // origin) yields coordinates in the same space the menu is laid out in.
  let base: Element<'_, Message> = if state.tab() == Tab::Inventory {
    iced::widget::mouse_area(surface)
      .on_move(Message::InventoryCursorMoved)
      .into()
  } else {
    surface.into()
  };

  // Always render through the overlay `Stack` with `base` pinned at child[0], even when no overlay
  // is active (empty `layers`). The inventory scrollable lives inside `base`; if it were sometimes
  // the root container and sometimes child[0] of a `Stack`, Iced would drop its internal scroll
  // offset on the reshape and snap the list to the top the moment a menu opened.
  stable_overlay(base, overlay_layers(state))
}

fn overlay_layers(state: &State) -> Vec<Element<'_, Message>> {
  if state.picker_open {
    let dropdown = positioned_dropdown(header::picker_dropdown(state), PICKER_OVERLAY_TOP, PICKER_OVERLAY_LEFT);
    return vec![backdrop::click_catcher(Message::PickerToggled), dropdown];
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

    return vec![backdrop::click_catcher(Message::InventoryHelpToggled), popover.into()];
  }

  if state.tab() == Tab::Abyssals && state.abyssal_picker_open() {
    return modal_layers(Message::AbyssalPickerToggled, abyssals::picker_modal(state));
  }

  let stockpile_menu = (state.tab() == Tab::Stockpiles)
    .then(|| state.stockpile_context_menu())
    .flatten();
  if let Some(menu) = stockpile_menu {
    return vec![
      backdrop::backdrop(Message::StockpileContextMenuClosed),
      stockpiles::context_menu_view(menu),
    ];
  }

  if state.tab() == Tab::Inventory
    && let Some(anchor) = state.inventory_menu()
  {
    let count = state.inventory_selection_count();
    let title = if count == 1 {
      t!("assets.inventory.stack_count_one", count => count).into_owned()
    } else {
      t!("assets.inventory.stack_count_other", count => count).into_owned()
    };
    return vec![
      backdrop::backdrop(Message::InventoryMenuDismissed),
      context_menu::context_menu(
        &title,
        vec![context_menu::Item::action(
          t!("assets.inventory.edit_tags"),
          Message::OpenSelectionTagModal,
        )],
        anchor,
      ),
    ];
  }

  if state.tab() == Tab::Inventory
    && let Some(modal) = state.asset_tag_modal()
  {
    let (assigned, assignable) = state.asset_tag_modal_partition();
    return modal_layers(
      Message::AssetTagModal(add_tag_modal::AddTagMessage::Close),
      add_tag_modal::view(
        modal,
        state.asset_tag_modal_entity_name(),
        assigned,
        assignable,
        Message::AssetTagModal,
      ),
    );
  }

  if state.tab() == Tab::Inventory && state.saved_filter_modal_open() {
    return modal_layers(Message::SaveFilterCancelled, tree::save_filter_modal(state));
  }

  if state.tab() == Tab::Inventory
    && let Some(menu) = state.saved_filter_context_menu()
  {
    return vec![
      backdrop::backdrop(Message::SavedFilterContextMenuClosed),
      tree::context_menu_view(menu),
    ];
  }

  let multibuy_export = (state.tab() == Tab::Stockpiles)
    .then(|| state.stockpile_multibuy_export())
    .flatten()
    .and_then(|id| state.stockpiles().iter().find(|card| card.id == id));
  if let Some(card) = multibuy_export {
    return modal_layers(
      Message::StockpileMultibuyExportClosed,
      stockpiles::multibuy_export_overlay(card, state.stockpile_multibuy_mode(), state.stockpile_multibuy_copied()),
    );
  }

  Vec::new()
}

fn body(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  if let Some((id, name, missing)) = state.tab_scope_gate() {
    let noun = tab_noun(state.tab());
    return Column::with_children(vec![
      tab_strip(state),
      forbidden::forbidden(noun, name, &missing, Message::ReauthRequested(id)),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  Column::with_children(vec![tab_strip(state), tab_body(state, now)])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn location_tree(state: &State) -> Element<'_, Message> {
  container(tree::pane(state))
    .width(Length::Fixed(state.pane(Pane::Sidebar).width()))
    .height(Length::Fill)
    .style(control::sunken_pane)
    .into()
}

fn tab_strip(state: &State) -> Element<'_, Message> {
  let tabs = state
    .enabled_tabs()
    .iter()
    .map(|&t| {
      let count = match t {
        Tab::Inventory => fmt_count(state.inventory_total()),
        Tab::Abyssals => fmt_count(state.abyssal_total()),
        Tab::Stockpiles => tab_count(state.stockpiles().len()),
        Tab::Values => values_count(state),
        Tab::Tracker => String::new(),
      };
      tab(state, t, tab_noun(t), count)
    })
    .collect::<Vec<_>>();

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
    icon: Some(tab_icon(tab)),
    label,
    on_press: (!selected).then_some(Message::TabSelected(tab)),
    selected,
  }
}

fn tab_icon(tab: Tab) -> Icon {
  match tab {
    Tab::Abyssals => Icon::abyssals(),
    Tab::Inventory => Icon::inventory(),
    Tab::Stockpiles => Icon::stockpiles(),
    Tab::Tracker => Icon::tracker(),
    Tab::Values => Icon::values(),
  }
}

fn tab_noun(tab: Tab) -> &'static str {
  static ABYSSALS: OnceLock<String> = OnceLock::new();
  static INVENTORY: OnceLock<String> = OnceLock::new();
  static STOCKPILES: OnceLock<String> = OnceLock::new();
  static TRACKER: OnceLock<String> = OnceLock::new();
  static VALUES: OnceLock<String> = OnceLock::new();
  match tab {
    Tab::Abyssals => ABYSSALS.get_or_init(|| t!("assets.tabs.abyssals").into_owned()),
    Tab::Inventory => INVENTORY.get_or_init(|| t!("assets.tabs.inventory").into_owned()),
    Tab::Stockpiles => STOCKPILES.get_or_init(|| t!("assets.tabs.stockpiles").into_owned()),
    Tab::Tracker => TRACKER.get_or_init(|| t!("assets.tabs.tracker").into_owned()),
    Tab::Values => VALUES.get_or_init(|| t!("assets.tabs.values").into_owned()),
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

fn tab_body(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  match state.tab() {
    Tab::Inventory => inventory_body(state),
    Tab::Abyssals => abyssals_body(state),
    Tab::Stockpiles => iced::widget::mouse_area(
      container(stockpiles::body(state.stockpiles(), state.stockpile_expanded()))
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
      scrollable(tracker::body(state.nav(), state.chart_hover(), now))
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
  let mut table: Vec<Element<'_, Message>> = Vec::new();
  if inventory::has_rows(state) {
    table.push(inventory::header(state));
  }
  table.push(
    container(inventory::body(state))
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
  );

  let inventory = Column::with_children(vec![
    inventory::filter_bar(state),
    Column::with_children(table)
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
    .style(control::sunken_pane);

  let cards: Vec<&abyssals::AbyssalCard> = state.abyssals().iter().collect();
  let any_owned = !state.abyssals().is_empty() || !state.abyssal_source_types().is_empty();
  let results = container(abyssals::body(cards, any_owned, state.abyssal_scroll_offset()))
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
