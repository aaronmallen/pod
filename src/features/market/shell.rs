use iced::{
  Background, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, container, text},
};

use super::{Message, State, Tab, i18n::tr_static};
use crate::ui::{
  components::{
    anchored_dropdown::AnchoredDropdown,
    eyebrow::eyebrow_text,
    header::header as shared_header,
    icon::Icon,
    location_combobox::LocationCombobox,
    tab_select::{self, TabLayout},
  },
  style::{color, control::bordered_pane, spacing, typography},
};

const REGION_PICKER_WIDTH: f32 = 460.0;
const REGION_POPOVER_WIDTH: f32 = 460.0;

const SIDE_PADDING: f32 = 28.0;
const TAB_STRIP_HEIGHT: f32 = 48.0;

pub(super) fn shell(state: &State) -> Element<'_, Message> {
  let column = Column::with_children(vec![header_band(state), tab_bar(state), body(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  container(column)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn header_band(state: &State) -> Element<'_, Message> {
  // My Orders swaps the title band for a scope picker + live stat blocks; that content lives in the
  // my_orders module so the table and its header evolve together.
  if state.active_tab() == Tab::Orders {
    return super::my_orders::header(state);
  }

  let (title_key, kicker_key) = match state.active_tab() {
    Tab::Browse => ("market.browse_title", "market.browse_kicker"),
    Tab::Orders => ("market.orders_title", "market.orders_kicker"),
    Tab::Watchlist => ("market.watchlist_title", "market.watchlist_kicker"),
  };

  let left = vec![title_block(title_key, kicker_key)];
  let right = match state.active_tab() {
    Tab::Browse => vec![region_slot(state)],
    Tab::Orders | Tab::Watchlist => Vec::new(),
  };

  shared_header(left, right)
}

fn title_block<'a>(title_key: &str, kicker_key: &str) -> Element<'a, Message> {
  Column::with_children(vec![
    text(t!(title_key).into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    eyebrow_text(&t!(kicker_key), None).into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .into()
}

fn region_slot(state: &State) -> Element<'_, Message> {
  Column::with_children(vec![
    eyebrow_text(&t!("market.region_label"), None).into(),
    region_picker(state),
  ])
  .spacing(spacing::UNIT + 2.0)
  .into()
}

fn region_picker(state: &State) -> Element<'_, Message> {
  let trigger = LocationCombobox::new()
    .placeholder(tr_static("market.region_select_placeholder"))
    .selection(state.active_location().cloned())
    .on_toggle(Message::RegionPickerToggled)
    .width(Length::Fixed(REGION_PICKER_WIDTH))
    .trigger();

  let popover = state.region_picker_open().then(|| {
    LocationCombobox::new()
      .placeholder(tr_static("market.region_search_placeholder"))
      .query(state.region_query())
      .results(state.region_results().to_vec())
      .highlight(state.region_highlight())
      .searching(state.region_searching())
      .selection(state.active_location().cloned())
      .on_input(Message::RegionSearchChanged)
      .on_pick(Message::RegionPicked)
      .width(Length::Fill)
      .popover()
  });

  AnchoredDropdown::new(trigger, popover)
    .on_dismiss(Message::RegionPickerClosed)
    .popover_width(REGION_POPOVER_WIDTH)
    .into()
}

fn tab_bar(state: &State) -> Element<'_, Message> {
  let tabs = Tab::ORDER
    .into_iter()
    .map(|tab| {
      let selected = state.active_tab() == tab;
      tab_select::Tab {
        count: if tab == Tab::Orders {
          super::my_orders::outbid_badge(state)
        } else {
          String::new()
        },
        icon: Some(tab_icon(tab)),
        label: tab_label(tab),
        on_press: (!selected).then_some(Message::TabSelected(tab)),
        selected,
      }
    })
    .collect::<Vec<tab_select::Tab<'_, Message>>>();

  container(tab_select::tab_select_with(tabs, TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: SIDE_PADDING,
      bottom: 0.0,
      left: SIDE_PADDING,
    })
    .style(bordered_pane)
    .into()
}

fn tab_icon(tab: Tab) -> Icon {
  match tab {
    Tab::Browse => Icon::market_tree(),
    Tab::Orders => Icon::contracts(),
    Tab::Watchlist => Icon::star(),
  }
}

fn tab_label(tab: Tab) -> &'static str {
  match tab {
    Tab::Browse => tr_static("nav.market.browse"),
    Tab::Orders => tr_static("nav.market.orders"),
    Tab::Watchlist => tr_static("nav.market.watchlist"),
  }
}

fn body(state: &State) -> Element<'_, Message> {
  match state.active_tab() {
    Tab::Browse => super::browse::surface(state),
    Tab::Orders => super::my_orders::surface(state),
    Tab::Watchlist => super::watchlist::surface(state),
  }
}

pub(super) fn empty_state<'a>(icon: Icon, title_key: &str, body_key: &str) -> Element<'a, Message> {
  let stack = Column::with_children(vec![
    container(
      icon
        .size(44.0)
        .color(color::with_alpha(color::text::PRIMARY, 0.24))
        .render(),
    )
    .padding(Padding {
      top: 0.0,
      right: 0.0,
      bottom: spacing::SPACE_2,
      left: 0.0,
    })
    .into(),
    text(t!(title_key).into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!(body_key).into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::Word)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_x(iced::alignment::Horizontal::Center);

  container(container(stack).max_width(360.0))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(spacing::SPACE_6)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_renders_the_shell_for_each_tab() {
    for tab in Tab::ORDER {
      let mut state = State::new();
      state.select_tab_by_id(tab.id());
      let _el: Element<'_, Message> = shell(&state);
    }
  }
}
