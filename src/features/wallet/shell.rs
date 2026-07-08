use chrono::{DateTime, Utc};
use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Point,
  alignment::{Horizontal, Vertical},
  mouse,
  widget::{Column, Row, Space, button, container, image, mouse_area, scrollable, text},
};

use super::{
  ContractEntry, CorpDivision, HEADER_SIDE_PADDING, JournalEntry, MarketEntry, Message, PartyImage, Scope, SignFilter,
  State, Tab, fmt_isk, header, side_filter::side_filter,
};
use crate::{
  features::wallet::{LedgerKind, contract_detail},
  store::{images::IconResolution, model::BudgetOwner},
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      avatar::Avatar,
      backdrop,
      button::{Button, Size},
      context_menu,
      eyebrow::eyebrow_text,
      forbidden,
      glyph_badge::GlyphBadge,
      icon::Icon,
      modal_overlay::{modal_layers, stable_overlay},
      positioned_dropdown::positioned_dropdown,
      segmented::segment_button,
      tab_select,
      table_cell::TableCell,
      text_input::TextInput,
      virtual_list::{self, VirtualList, VirtualListConfig},
    },
    style::{
      color,
      control::{bordered_pane, sunken_pane},
      radius, spacing, typography,
    },
  },
};

const BUDGET_CHIP_RADIUS: f32 = 11.0;
const BUDGET_DOT_SIZE: f32 = 7.0;
const MARKET_ICON_BOX: f32 = 28.0;
const ROW_AVATAR: f32 = 22.0;

const PICKER_OVERLAY_TOP: f32 = spacing::layout::HEADER_HEIGHT + 6.0;
const PICKER_OVERLAY_LEFT: f32 = HEADER_SIDE_PADDING;
const TAB_STRIP_HEIGHT: f32 = 48.0;

const JOURNAL_RIGHT_COL_WIDTH: f32 = 120.0;

/// Nominal height of one ledger row, in pixels.
///
/// Ledger rows are content-driven (two-line party/amount cells, a 22px avatar),
/// so this is only an estimate for [`VirtualList`] offset math; overscan absorbs
/// the one- vs two-line variance so no visible gap can open.
const ESTIMATED_ROW_HEIGHT: f32 = 60.0;

const LEDGER_SCROLL_ID: &str = "wallet-ledger-scroll";

pub(super) fn shell(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let body = Column::with_children(vec![header::header(state, now), self::body(state, now)])
    .width(Length::Fill)
    .height(Length::Fill);

  let surface = container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  // On the selectable ledger tabs the feature-root base is wrapped in a
  // cursor-tracking `mouse_area` so a right-click can anchor its bulk-assign menu
  // at the pointer. `mouse_area`'s `on_move` reports the cursor relative to its own
  // bounds, so tracking at the base (which shares the overlay `Stack`'s origin)
  // yields coordinates in the same space the menu is laid out in. Tracking on the
  // inner table instead would offset every anchor by the header/tabs/hero above it,
  // collapsing the menu toward the feature-root top-left.
  let base: Element<'_, Message> = if matches!(state.tab, Tab::Journal | Tab::Market) {
    mouse_area(surface).on_move(Message::LedgerCursorMoved).into()
  } else {
    surface.into()
  };

  // Always render through the overlay `Stack` with `base` pinned at child[0],
  // even when no menu is active (empty `layers`). The ledger scrollable lives
  // inside `base`; if it were sometimes the root container and sometimes child[0]
  // of a Stack, Iced would drop its internal scroll offset on the reshape and snap
  // the list to the top the moment a menu opened. Keeping `base` at a stable tree
  // position preserves the scroll offset across every open/close.
  stable_overlay(base, overlay_layers(state))
}

fn overlay_layers(state: &State) -> Vec<Element<'_, Message>> {
  if state.budget_reconcile().is_some() {
    return modal_layers(
      Message::BudgetReconcileClosed,
      super::budget_view::reconcile_modal(state),
    );
  }

  if state.picker_open {
    let dropdown = positioned_dropdown(header::picker_dropdown(state), PICKER_OVERLAY_TOP, PICKER_OVERLAY_LEFT);
    return vec![backdrop::click_catcher(Message::PickerToggled), dropdown];
  }

  if let Some((anchor, picking)) = state.ledger_menu_open() {
    let overlay = if picking {
      bulk_assign_picker(state, anchor)
    } else {
      let count = state.ledger_selection_count();
      let title = if count == 1 {
        t!("wallet.shell.ledger_selection_one", count => count).into_owned()
      } else {
        t!("wallet.shell.ledger_selection_other", count => count).into_owned()
      };
      context_menu::context_menu(
        &title,
        vec![context_menu::Item::action(
          t!("wallet.shell.assign_to_budget").into_owned(),
          Message::LedgerBulkAssignOpened,
        )],
        anchor,
      )
    };
    return vec![backdrop::backdrop(Message::LedgerMenuDismissed), overlay];
  }

  Vec::new()
}

fn body(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  if let Some((id, name, missing)) = state.tab_scope_gate() {
    return Column::with_children(vec![
      tabs(state),
      forbidden::forbidden(tab_noun(state.tab), name, &missing, Message::ReauthRequested(id)),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
  }

  if state.tab == Tab::Budget {
    return Column::with_children(vec![tabs(state), super::budget_view::surface(state)])
      .width(Length::Fill)
      .height(Length::Fill)
      .into();
  }

  if state.tab == Tab::Wallets {
    return Column::with_children(vec![tabs(state), super::wallets_view::surface(state)])
      .width(Length::Fill)
      .height(Length::Fill)
      .into();
  }

  Column::with_children(vec![tabs(state), center(state, now)])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn center(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let mut head_children: Vec<Element<'_, Message>> = vec![super::hero::hero(state, now)];
  if matches!(state.active(), Scope::Corporation(_)) {
    head_children.push(division_strip(state));
  }
  head_children.push(filter_bar(state));
  let head = Column::with_children(head_children).width(Length::Fill);

  let mut children: Vec<Element<'_, Message>> = vec![head.into()];
  if let Some(pinned_header) = pinned_header(state) {
    children.push(pinned_header);
  }
  children.push(tab_body(state, now));

  Column::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn pinned_header<'a>(state: &State) -> Option<Element<'a, Message>> {
  match state.tab {
    Tab::Market => Some(market_header(state)),
    Tab::Contracts => Some(contract_header()),
    Tab::Budget | Tab::Journal | Tab::Wallets => None,
  }
}

fn division_strip(state: &State) -> Element<'_, Message> {
  let divisions = state.corp_divisions();

  let strip: Element<'_, Message> = if divisions.is_empty() {
    division_caption(super::i18n::tr_static("wallet.shell.no_divisions"))
  } else {
    scrollable(
      Row::with_children(
        divisions
          .iter()
          .map(|division| division_button(division, state.active_division()))
          .collect::<Vec<Element<'_, Message>>>(),
      )
      .width(Length::Shrink),
    )
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new()))
    .into()
  };

  container(Column::with_children(vec![strip]).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: HEADER_SIDE_PADDING,
      bottom: 0.0,
      left: HEADER_SIDE_PADDING,
    })
    .style(bordered_pane)
    .into()
}

fn division_button<'a>(division: &'a CorpDivision, active_division: i64) -> Element<'a, Message> {
  let active = division.division == active_division;
  let label = t!(
    "wallet.shell.division_label",
    division => division.label(),
    isk => fmt_isk(division.balance),
  )
  .into_owned();

  segment_button(
    label,
    active,
    Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
    },
    Message::DivisionSelected(division.division),
  )
}

fn division_caption<'a>(text_value: &str) -> Element<'a, Message> {
  container(
    text(text_value.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary())),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: 0.0,
    bottom: spacing::SPACE_2,
    left: 0.0,
  })
  .into()
}

fn tabs(state: &State) -> Element<'_, Message> {
  let journal_count = state.journal_total;
  let market_count = state.market_total;
  let contract_count = state.contract_total;

  let tabs = state
    .enabled_tabs()
    .iter()
    .map(|&tab| {
      let (label, count) = match tab {
        Tab::Wallets => (super::i18n::tr_static("wallet.shell.tab_wallets"), None),
        Tab::Journal => (super::i18n::tr_static("wallet.shell.tab_journal"), Some(journal_count)),
        Tab::Market => (
          super::i18n::tr_static("wallet.shell.tab_transactions"),
          Some(market_count),
        ),
        Tab::Contracts => (
          super::i18n::tr_static("wallet.shell.tab_contracts"),
          Some(contract_count),
        ),
        Tab::Budget => (super::i18n::tr_static("wallet.shell.tab_budget"), None),
      };
      (tab, label, count)
    })
    .map(|(tab, label, count)| {
      let selected = state.tab == tab;
      tab_select::Tab {
        count: count.map(|count| count.to_string()).unwrap_or_default(),
        icon: Some(tab_icon(tab)),
        label,
        on_press: (!selected).then_some(Message::TabSelected(tab)),
        selected,
      }
    })
    .collect::<Vec<tab_select::Tab<'_, Message>>>();

  container(tab_select::tab_select_with(tabs, tab_select::TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: HEADER_SIDE_PADDING,
      bottom: 0.0,
      left: HEADER_SIDE_PADDING,
    })
    .style(bordered_pane)
    .into()
}

fn tab_icon(tab: Tab) -> Icon {
  match tab {
    Tab::Budget => Icon::budget(),
    Tab::Contracts => Icon::contracts(),
    Tab::Journal => Icon::journal(),
    Tab::Market => Icon::market(),
    Tab::Wallets => Icon::wallet(),
  }
}

fn tab_noun(tab: Tab) -> &'static str {
  match tab {
    Tab::Budget => super::i18n::tr_static("wallet.shell.tab_noun_budget"),
    Tab::Contracts => super::i18n::tr_static("wallet.shell.tab_noun_contracts"),
    Tab::Journal => super::i18n::tr_static("wallet.shell.tab_noun_journal"),
    Tab::Market => super::i18n::tr_static("wallet.shell.tab_noun_transactions"),
    Tab::Wallets => super::i18n::tr_static("wallet.shell.tab_noun_wallets"),
  }
}

fn filter_bar(state: &State) -> Element<'_, Message> {
  let search = TextInput::new(
    super::i18n::tr_static("wallet.shell.search_placeholder"),
    &state.search,
    Message::SearchChanged,
  )
  .input_id(crate::features::shell::focus_search::wallet_search_id())
  .leading_icon(Icon::search())
  .width(Length::Fill)
  .render();

  let mut controls: Vec<Element<'_, Message>> = vec![search];
  if matches!(state.tab, Tab::Market | Tab::Contracts) {
    controls.push(side_filter::<Message>(state.side_filter(), Message::SideFilterChanged));
  }
  if state.tab != Tab::Contracts {
    controls.push(sign_control(state));
  }
  if matches!(state.tab, Tab::Journal | Tab::Market)
    && let Some(badge) = budget_filter_badge(state)
  {
    controls.push(badge);
  }
  if state.has_active_filters() {
    controls.push(clear_filters_button());
  }

  container(
    Row::with_children(controls)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_3,
    left: HEADER_SIDE_PADDING,
  })
  .style(bordered_pane)
  .into()
}

fn budget_filter_badge(state: &State) -> Option<Element<'_, Message>> {
  let filter = state.budget_filter()?;
  let (label, accent) = match filter.kind {
    super::BudgetFilterKind::Category(id) => {
      let envelope = state.budget_chips().meta.get(&id)?;
      (
        envelope.name.clone(),
        super::budget::tone_color(envelope.tone.as_deref()),
      )
    }
    super::BudgetFilterKind::Uncategorized => (
      t!("wallet.shell.uncategorized_only").into_owned(),
      color::status::WARNING,
    ),
  };

  let dot = container(Space::new())
    .width(Length::Fixed(7.0))
    .height(Length::Fixed(7.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(accent)),
      border: Border {
        radius: 3.5.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let clear = button(
    text("\u{00D7}")
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(0)
  .on_press(Message::BudgetFilterCleared)
  .style(|_, _| iced::widget::button::Style {
    background: Some(Background::Color(iced::Color::TRANSPARENT)),
    ..iced::widget::button::Style::default()
  });

  let pill = container(
    Row::with_children(vec![
      dot.into(),
      text(label)
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      clear.into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, 0.1))),
    border: Border {
      color: color::with_alpha(accent, 0.35),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  });

  Some(pill.into())
}

fn clear_filters_button<'a>() -> Element<'a, Message> {
  Button::secondary(t!("wallet.shell.clear_filters"))
    .size(Size::Sm)
    .on_press(Message::FiltersCleared)
    .into()
}

fn sign_control(state: &State) -> Element<'_, Message> {
  let segments = [
    (SignFilter::All, super::i18n::tr_static("wallet.shell.sign_all")),
    (SignFilter::In, super::i18n::tr_static("wallet.shell.sign_in")),
    (SignFilter::Out, super::i18n::tr_static("wallet.shell.sign_out")),
  ];

  let row = Row::with_children(
    segments
      .into_iter()
      .map(|(filter, label)| {
        let active = state.sign_filter == filter;
        segment_button(
          label,
          active,
          Padding {
            top: spacing::SPACE_2,
            right: spacing::SPACE_3,
            bottom: spacing::SPACE_2,
            left: spacing::SPACE_3,
          },
          Message::SignFilterChanged(filter),
        )
      })
      .collect::<Vec<Element<'_, Message>>>(),
  );

  container(row)
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn tab_body(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  match state.tab {
    Tab::Journal => journal_table(state, now),
    Tab::Market => market_table(state, now),
    Tab::Contracts => contracts_table(state, now),
    // The Budget and Wallets tabs take the dedicated full-width surface path in
    // `body`; these arms are unreachable from `center` but keep the match
    // exhaustive.
    Tab::Budget => empty_ledger(super::i18n::tr_static("wallet.shell.tab_budget")),
    Tab::Wallets => empty_ledger(super::i18n::tr_static("wallet.shell.tab_wallets")),
  }
}

fn journal_table(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let entries = super::filtered_journal(state);
  if entries.is_empty() {
    return empty_ledger(super::i18n::tr_static("wallet.shell.empty_journal"));
  }

  windowed_ledger(state, entries, move |entry| journal_row(state, entry, now))
}

fn windowed_ledger<'a, T, F>(state: &'a State, entries: Vec<&'a T>, render: F) -> Element<'a, Message>
where
  F: Fn(&'a T) -> Element<'a, Message> + 'a,
{
  let offset = state.tab_scroll_offset();
  virtual_list::responsive_window(move |viewport_height| {
    // Clamp the persisted offset to what the current row set can scroll to. When a
    // reload re-windows the list shorter (rows leaving an active filter after a
    // bulk assign), the stale larger offset would otherwise render past the end and
    // snap the view to the top; clamping holds position so the remaining rows shift
    // up into view, matching the offset the bare scrollable clamps itself to.
    let max_offset = VirtualListConfig::new(entries.len(), ESTIMATED_ROW_HEIGHT)
      .viewport_height(viewport_height)
      .max_scroll_offset();
    let config = VirtualListConfig::new(entries.len(), ESTIMATED_ROW_HEIGHT)
      .viewport_height(viewport_height)
      .scroll_offset(offset.min(max_offset));
    let list = VirtualList::new(config, |index| render(entries[index])).view();

    scrollable(list)
      .id(LEDGER_SCROLL_ID)
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::TabScrolled {
        absolute: viewport.absolute_offset().y,
        relative: viewport.relative_offset().y,
      })
      .into()
  })
}

fn journal_row<'a>(state: &'a State, entry: &'a JournalEntry, now: DateTime<Utc>) -> Element<'a, Message> {
  let (glyph, is_in) = super::journal_type_glyph(entry);
  let delta_color = if is_in {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let sign = if is_in { "+" } else { "\u{2212}" };
  let delta = format!("{sign}{}", fmt_isk(entry.amount.map(f64::abs)));

  let select = |cell| select_wrap(cell, LedgerKind::Journal, entry.owner, entry.id);
  let selected = state.journal_selected(entry.owner, entry.id);
  let mut cells = vec![
    select(GlyphBadge::new(glyph, is_in).render()),
    select(journal_left_col(entry)),
  ];
  if state.budget_enabled() {
    cells.push(
      container(budget_chip(state, entry.owner, LedgerKind::Journal, entry.id))
        .width(Length::Fixed(170.0))
        .into(),
    );
  }
  cells.push(select(journal_character_col(state, entry.owner)));
  cells.push(select(journal_right_col(
    &delta,
    delta_color,
    &fmt_relative(&entry.date, now),
  )));
  row_shell(cells, selected)
}

fn journal_left_col<'a>(entry: &'a JournalEntry) -> Element<'a, Message> {
  Column::with_children(vec![
    TableCell::new(party_label(entry).to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .color(color::text::PRIMARY)
      .view::<Message>(),
    TableCell::new(super::humanize_ref_type(&entry.ref_type))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .color(color::text::secondary())
      .view::<Message>(),
  ])
  .width(Length::Fill)
  .into()
}

fn journal_character_col(state: &State, owner: BudgetOwner) -> Element<'_, Message> {
  let name = owner_display_name(state, owner);
  let portrait = owner_portrait(state, owner);

  Avatar::new(owner.owner_id(), name, Length::Fixed(ROW_AVATAR), ROW_AVATAR, portrait)
    .radius(radius::SUBTLE)
    .view()
}

fn journal_right_col<'a>(delta: &str, delta_color: iced::Color, when: &str) -> Element<'a, Message> {
  Column::with_children(vec![
    TableCell::new(delta.to_owned())
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .align(Horizontal::Right)
      .color(delta_color)
      .view::<Message>(),
    TableCell::new(when.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .align(Horizontal::Right)
      .color(color::text::tertiary())
      .view::<Message>(),
  ])
  .width(Length::Fixed(JOURNAL_RIGHT_COL_WIDTH))
  .into()
}

fn market_header<'a>(state: &State) -> Element<'a, Message> {
  let mut columns = vec![
    (
      super::i18n::tr_static("wallet.shell.col_side"),
      Length::FillPortion(1),
      Horizontal::Left,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_item"),
      Length::FillPortion(3),
      Horizontal::Left,
    ),
  ];
  if state.budget_enabled() {
    columns.push((
      super::i18n::tr_static("wallet.shell.col_budget"),
      Length::FillPortion(2),
      Horizontal::Left,
    ));
  }
  columns.extend([
    (
      super::i18n::tr_static("wallet.shell.col_qty"),
      Length::FillPortion(1),
      Horizontal::Right,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_unit"),
      Length::FillPortion(2),
      Horizontal::Right,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_total"),
      Length::FillPortion(2),
      Horizontal::Right,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_location"),
      Length::FillPortion(2),
      Horizontal::Left,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_character"),
      Length::FillPortion(2),
      Horizontal::Left,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_when"),
      Length::FillPortion(1),
      Horizontal::Left,
    ),
  ]);
  table_header(&columns)
}

fn market_table(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let entries = super::filtered_market(state);
  if entries.is_empty() {
    return empty_ledger(super::i18n::tr_static("wallet.shell.empty_transactions"));
  }

  windowed_ledger(state, entries, move |entry| market_row(state, entry, now))
}

fn market_row<'a>(state: &'a State, entry: &'a MarketEntry, now: DateTime<Utc>) -> Element<'a, Message> {
  let (side_label, side_color) = if entry.is_buy {
    (super::i18n::tr_static("wallet.shell.side_buy"), color::status::DANGER)
  } else {
    (super::i18n::tr_static("wallet.shell.side_sell"), color::status::ONLINE)
  };

  let select = |cell| select_wrap(cell, LedgerKind::Market, entry.owner, entry.transaction_id);
  let selected = state.market_selected(entry.owner, entry.transaction_id);
  let mut cells = vec![
    select(side_badge(side_label, side_color, Length::FillPortion(1))),
    select(item_cell(&entry.item, &entry.type_icon, Length::FillPortion(3))),
  ];
  if state.budget_enabled() {
    cells.push(
      container(budget_chip(
        state,
        entry.owner,
        LedgerKind::Market,
        entry.transaction_id,
      ))
      .width(Length::FillPortion(2))
      .into(),
    );
  }
  cells.extend([
    select(mono_cell(
      &entry.quantity.to_string(),
      Length::FillPortion(1),
      Horizontal::Right,
      color::text::PRIMARY,
    )),
    select(mono_cell(
      &fmt_isk(Some(entry.unit_price)),
      Length::FillPortion(2),
      Horizontal::Right,
      color::text::secondary(),
    )),
    select(amount_cell(
      &fmt_isk(Some(entry.total)),
      Length::FillPortion(2),
      color::text::PRIMARY,
    )),
    select(mono_cell(
      &entry.location,
      Length::FillPortion(2),
      Horizontal::Left,
      color::text::secondary(),
    )),
    select(market_character_cell(state, entry, Length::FillPortion(2))),
    select(mono_cell(
      &fmt_relative(&entry.date, now),
      Length::FillPortion(1),
      Horizontal::Left,
      color::text::secondary(),
    )),
  ]);
  row_shell(cells, selected)
}

fn budget_chip<'a>(state: &'a State, owner: BudgetOwner, kind: LedgerKind, entry_id: i64) -> Element<'a, Message> {
  let chips = state.budget_chips();
  let assigned = state.budget_category_for(owner, kind, entry_id);
  let open = state.budget_picker() == Some((owner, kind, entry_id));
  let on_press = if open {
    Message::BudgetChipDismissed
  } else {
    Message::BudgetChipOpened(owner, kind, entry_id)
  };

  let trigger = match assigned.and_then(|id| chips.meta.get(&id)) {
    Some(envelope) => assigned_chip(
      &envelope.name,
      super::budget::tone_color(envelope.tone.as_deref()),
      on_press,
    ),
    None => assign_affordance(on_press),
  };

  let popover = open.then(|| budget_picker_popover(state, assigned));

  AnchoredDropdown::new(trigger, popover)
    .on_dismiss(Message::BudgetChipDismissed)
    .popover_width(budget_picker_width(chips))
    .into()
}

fn budget_dot<'a>(tone: iced::Color) -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(BUDGET_DOT_SIZE))
    .height(Length::Fixed(BUDGET_DOT_SIZE))
    .style(move |_| container::Style {
      background: Some(Background::Color(tone)),
      border: Border {
        radius: (BUDGET_DOT_SIZE / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn assigned_chip<'a>(name: &str, tone: iced::Color, on_press: Message) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      budget_dot(tone),
      text(name.to_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Icon::chevron_down()
        .size(typography::size::XS)
        .color(color::text::tertiary())
        .render(),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    right: spacing::SPACE_2,
    bottom: 3.0,
    left: spacing::SPACE_2,
  })
  .style(|_, status| {
    let active = matches!(
      status,
      iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
    );
    iced::widget::button::Style {
      background: Some(Background::Color(if active {
        color::with_alpha(color::text::PRIMARY, 0.05)
      } else {
        iced::Color::TRANSPARENT
      })),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: BUDGET_CHIP_RADIUS.into(),
      },
      text_color: color::text::secondary(),
      ..iced::widget::button::Style::default()
    }
  })
  .on_press(on_press)
  .into()
}

/// The needs-attention affordance for an uncategorized outflow: an amber pill
/// reading "+ Assign category" (iced has no dashed border, so a solid amber
/// hairline carries the same intent).
fn assign_affordance<'a>(on_press: Message) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      text("+")
        .font(typography::mono::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(color::status::WARNING))
        .into(),
      text(t!("wallet.shell.assign_category"))
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(color::status::WARNING))
        .into(),
    ])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    right: spacing::SPACE_2,
    bottom: 3.0,
    left: spacing::SPACE_2,
  })
  .style(|_, status| {
    let active = matches!(
      status,
      iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
    );
    iced::widget::button::Style {
      background: Some(Background::Color(color::with_alpha(
        color::status::WARNING,
        if active { 0.18 } else { 0.1 },
      ))),
      border: Border {
        color: color::with_alpha(color::status::WARNING, if active { 0.8 } else { 0.55 }),
        width: 1.0,
        radius: BUDGET_CHIP_RADIUS.into(),
      },
      text_color: color::status::WARNING,
      ..iced::widget::button::Style::default()
    }
  })
  .on_press(on_press)
  .into()
}

fn budget_picker_width(chips: &super::loaders::BudgetChips) -> f32 {
  const CHAR_WIDTH: f32 = 7.5;
  const CHROME: f32 = 72.0;

  let longest = chips
    .envelopes
    .iter()
    .flat_map(|group| {
      std::iter::once(group.name.chars().count()).chain(group.categories.iter().map(|cat| cat.name.chars().count()))
    })
    .max()
    .unwrap_or(0);

  (CHROME + longest as f32 * CHAR_WIDTH).clamp(220.0, 360.0)
}

fn budget_picker_popover<'a>(state: &'a State, current: Option<i64>) -> Element<'a, Message> {
  let chips = state.budget_chips();
  let mut rows: Vec<Element<'a, Message>> = Vec::new();

  for group in &chips.envelopes {
    rows.push(
      text(group.name.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
    for cat in &group.categories {
      let tint = super::budget::tone_color(cat.tone.as_deref());
      rows.push(budget_picker_row(
        &cat.name,
        current == Some(cat.id),
        tint,
        Message::BudgetChipAssigned(Some(cat.id)),
      ));
    }
  }

  container(
    scrollable(Column::with_children(rows).spacing(spacing::SPACE_2))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fixed(240.0)),
  )
  .padding(spacing::SPACE_3)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn bulk_assign_picker<'a>(state: &'a State, anchor: Point) -> Element<'a, Message> {
  let chips = state.budget_chips();
  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  // A Clear row mirrors the per-row chip's unassign: it routes through the
  // authoritative DB-side cross-owner delete so clearing a selection removes the
  // mark for every owner of each event, not just the loaded copies.
  rows.push(budget_picker_row(
    super::i18n::tr_static("wallet.shell.clear"),
    false,
    color::text::tertiary(),
    Message::LedgerBulkAssignChosen(None),
  ));
  for group in &chips.envelopes {
    rows.push(
      text(group.name.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
    for cat in &group.categories {
      let tint = super::budget::tone_color(cat.tone.as_deref());
      rows.push(budget_picker_row(
        &cat.name,
        false,
        tint,
        Message::LedgerBulkAssignChosen(Some(cat.id)),
      ));
    }
  }

  let panel = container(
    scrollable(Column::with_children(rows).spacing(spacing::SPACE_2))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fixed(240.0)),
  )
  .width(Length::Fixed(context_menu::MENU_WIDTH))
  .padding(spacing::SPACE_3)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  });

  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top: anchor.y.max(0.0),
      left: anchor.x.max(0.0),
      ..Padding::ZERO
    })
    .into()
}

fn budget_picker_row<'a>(label: &str, selected: bool, tint: iced::Color, on_press: Message) -> Element<'a, Message> {
  let dot = container(Space::new())
    .width(Length::Fixed(6.0))
    .height(Length::Fixed(6.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(tint)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let label_text = text(label.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(if selected {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    }));

  let check: Element<'a, Message> = if selected {
    text("\u{2713}")
      .font(typography::mono::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(tint))
      .into()
  } else {
    Space::new().into()
  };

  let row = Row::with_children(vec![
    dot.into(),
    label_text.into(),
    Space::new().width(Length::Fill).into(),
    check,
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  button(row)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
    })
    .width(Length::Fill)
    .style(move |_, _status| iced::widget::button::Style {
      background: if selected {
        Some(Background::Color(color::with_alpha(tint, 0.08)))
      } else {
        None
      },
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      text_color: if selected {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..iced::widget::button::Style::default()
    })
    .on_press(on_press)
    .into()
}

fn side_badge<'a>(label: &'a str, tint: iced::Color, width: Length) -> Element<'a, Message> {
  let chip = container(
    text(label)
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(tint),
      }),
  )
  .padding(Padding {
    top: 3.0,
    right: spacing::SPACE_2,
    bottom: 3.0,
    left: spacing::SPACE_2,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.12))),
    border: Border {
      color: color::with_alpha(tint, 0.3),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  });

  container(chip).width(width).align_x(Horizontal::Left).into()
}

fn item_cell<'a>(item: &str, type_icon_resolution: &IconResolution, width: Length) -> Element<'a, Message> {
  Row::with_children(vec![
    type_icon(type_icon_resolution),
    text(item.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .width(Length::Fill)
      .wrapping(text::Wrapping::Word)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(width)
  .into()
}

fn type_icon<'a>(resolution: &IconResolution) -> Element<'a, Message> {
  match resolution {
    IconResolution::Found(path) => container(
      image(image::Handle::from_path(path.clone()))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Contain),
    )
    .width(Length::Fixed(MARKET_ICON_BOX))
    .height(Length::Fixed(MARKET_ICON_BOX))
    .clip(true)
    .into(),
    IconResolution::Missing => container(Space::new())
      .width(Length::Fixed(MARKET_ICON_BOX))
      .height(Length::Fixed(MARKET_ICON_BOX))
      .into(),
  }
}

fn character_cell(state: &State, owner: BudgetOwner, width: Length) -> Element<'_, Message> {
  let name = owner_display_name(state, owner);
  let portrait = owner_portrait(state, owner);

  let swatch = Avatar::new(owner.owner_id(), &name, Length::Fixed(ROW_AVATAR), ROW_AVATAR, portrait)
    .radius(radius::SUBTLE)
    .view();

  Row::with_children(vec![
    swatch,
    text(name)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .width(Length::Fill)
      .wrapping(text::Wrapping::Word)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(width)
  .into()
}

/// A market row's Character cell. For a corp-on-behalf trade — one whose
/// `transaction_id` exists in both a character and a corp wallet — it shows the
/// trading character's portrait with the corp logo composited as a corner badge.
/// A purely personal trade renders the plain owner cell.
fn market_character_cell<'a>(state: &'a State, entry: &'a MarketEntry, width: Length) -> Element<'a, Message> {
  let Some((character_id, corporation_id)) = super::market_dual_wallet_owners(state, entry.transaction_id) else {
    return character_cell(state, entry.owner, width);
  };

  let owner = BudgetOwner::Character(character_id);
  let name = owner_display_name(state, owner);
  let swatch = Avatar::new(
    character_id,
    &name,
    Length::Fixed(ROW_AVATAR),
    ROW_AVATAR,
    owner_portrait(state, owner),
  )
  .radius(radius::SUBTLE)
  .corp_badge(corp_logo(state, corporation_id))
  .view();

  Row::with_children(vec![
    swatch,
    text(name)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .width(Length::Fill)
      .wrapping(text::Wrapping::Word)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(width)
  .into()
}

fn corp_logo(state: &State, corporation_id: i64) -> Option<std::path::PathBuf> {
  state
    .corporations
    .iter()
    .find(|corp| corp.id == corporation_id)
    .and_then(|corp| corp.logo.path())
}

fn contract_header<'a>() -> Element<'a, Message> {
  table_header(&[
    (
      super::i18n::tr_static("wallet.shell.col_type"),
      Length::FillPortion(2),
      Horizontal::Left,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_status"),
      Length::FillPortion(2),
      Horizontal::Left,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_issuer"),
      Length::FillPortion(2),
      Horizontal::Left,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_counterparty"),
      Length::FillPortion(2),
      Horizontal::Left,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_value"),
      Length::FillPortion(2),
      Horizontal::Right,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_collateral"),
      Length::FillPortion(2),
      Horizontal::Right,
    ),
    (
      super::i18n::tr_static("wallet.shell.col_when"),
      Length::FillPortion(1),
      Horizontal::Left,
    ),
  ])
}

fn contracts_table(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  if !state.has_contracts() {
    return no_source_state(
      super::i18n::tr_static("wallet.shell.tab_contracts"),
      super::i18n::tr_static("wallet.shell.no_contracts_synced"),
    );
  }

  let entries = super::filtered_contracts(state);
  if entries.is_empty() {
    return empty_ledger(super::i18n::tr_static("wallet.shell.empty_contracts"));
  }

  windowed_ledger(state, entries, move |entry| contract_row(entry, now))
}

fn contract_row<'a>(entry: &'a ContractEntry, now: DateTime<Utc>) -> Element<'a, Message> {
  let (counterparty_name, counterparty_id) = contract_counterparty(entry);
  let counterparty_image = contract_counterparty_image(entry);

  let row = row_shell(
    vec![
      body_cell(
        humanize_contract_type(&entry.r#type),
        Length::FillPortion(2),
        color::text::PRIMARY,
      ),
      contract_status_cell(&entry.derived_status(now), Length::FillPortion(2)),
      party_cell(
        Some(entry.issuer_id),
        entry.issuer.as_deref(),
        &entry.issuer_image,
        Length::FillPortion(2),
      ),
      party_cell(
        counterparty_id,
        counterparty_name,
        counterparty_image,
        Length::FillPortion(2),
      ),
      amount_cell(&fmt_isk(entry.value), Length::FillPortion(2), color::text::PRIMARY),
      mono_cell(
        &fmt_isk(entry.collateral),
        Length::FillPortion(2),
        Horizontal::Right,
        color::text::tertiary(),
      ),
      mono_cell(
        &fmt_relative(&entry.date_issued, now),
        Length::FillPortion(1),
        Horizontal::Left,
        color::text::secondary(),
      ),
    ],
    false,
  );

  mouse_area(row)
    .on_press(Message::ContractSelected(entry.contract_id))
    .interaction(mouse::Interaction::Pointer)
    .into()
}

fn contract_status_cell<'a>(status: &str, width: Length) -> Element<'a, Message> {
  let tint = contract_detail::contract_status_color(status);

  let row = Row::with_children(vec![
    container(Space::new())
      .width(Length::Fixed(6.0))
      .height(Length::Fixed(6.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(tint)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    text(status.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(row).width(width).into()
}

fn party_or_dash(name: Option<&str>) -> String {
  name
    .filter(|value| !value.is_empty())
    .map_or_else(|| "\u{2014}".to_owned(), str::to_owned)
}

fn contract_counterparty(entry: &ContractEntry) -> (Option<&str>, Option<i64>) {
  match entry.acceptor.as_deref().filter(|value| !value.is_empty()) {
    Some(name) => (Some(name), entry.acceptor_id),
    None => (entry.assignee.as_deref(), entry.assignee_id),
  }
}

fn contract_counterparty_image(entry: &ContractEntry) -> &PartyImage {
  match entry.acceptor.as_deref().filter(|value| !value.is_empty()) {
    Some(_) => &entry.acceptor_image,
    None => &entry.assignee_image,
  }
}

fn roster_portrait(state: &State, character_id: i64) -> Option<std::path::PathBuf> {
  state
    .roster
    .iter()
    .find(|pilot| pilot.id == character_id)
    .and_then(|pilot| pilot.portrait.path())
}

fn party_cell<'a>(id: Option<i64>, name: Option<&str>, image: &PartyImage, width: Length) -> Element<'a, Message> {
  let label = party_or_dash(name);
  let mut row_items: Vec<Element<'a, Message>> = Vec::new();

  if let Some(entity_id) = id.filter(|value| *value > 0) {
    let swatch = Avatar::new(
      entity_id,
      &label,
      Length::Fixed(ROW_AVATAR),
      ROW_AVATAR,
      image.path.clone(),
    )
    .radius(radius::SUBTLE)
    .view();
    row_items.push(swatch);
  }

  row_items.push(
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .width(Length::Fill)
      .wrapping(text::Wrapping::Word)
      .style(typography::colored(color::text::secondary()))
      .into(),
  );

  Row::with_children(row_items)
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(width)
    .into()
}

fn humanize_contract_type(value: &str) -> String {
  if value.is_empty() {
    return "\u{2014}".to_owned();
  }
  value
    .split('_')
    .map(|word| {
      let mut chars = word.chars();
      match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
      }
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn table_header<'a>(columns: &[(&str, Length, Horizontal)]) -> Element<'a, Message> {
  let cells: Vec<Element<'a, Message>> = columns
    .iter()
    .map(|(label, width, align)| eyebrow_text(label, None).width(*width).align_x(*align).into())
    .collect();

  container(Row::with_children(cells).spacing(spacing::SPACE_3).padding(Padding {
    top: spacing::SPACE_2_5,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_2_5,
    left: HEADER_SIDE_PADDING,
  }))
  .width(Length::Fill)
  .style(sunken_pane)
  .into()
}

fn row_shell<'a>(cells: Vec<Element<'a, Message>>, selected: bool) -> Element<'a, Message> {
  let background = selected.then(|| Background::Color(color::with_alpha(color::accent(), 0.1)));
  container(
    Row::with_children(cells)
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: HEADER_SIDE_PADDING,
    bottom: spacing::SPACE_3,
    left: HEADER_SIDE_PADDING,
  })
  .style(move |_| container::Style {
    background,
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.06),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

/// Wraps a ledger cell so a left-click selects the row (honoring the active
/// keyboard modifiers) and a right-click opens the bulk-assign menu. The envelope
/// chip is deliberately left unwrapped so its own press is never swallowed.
fn select_wrap<'a>(
  cell: Element<'a, Message>,
  kind: LedgerKind,
  owner: BudgetOwner,
  entry_id: i64,
) -> Element<'a, Message> {
  mouse_area(cell)
    .on_press(Message::LedgerRowClicked(kind, owner, entry_id))
    .on_right_press(Message::LedgerRowRightPressed(kind, owner, entry_id))
    .into()
}

fn sized_cell<'a>(cell: TableCell, width: Length) -> Element<'a, Message> {
  container(cell.view::<Message>()).width(width).into()
}

fn body_cell<'a>(value: impl Into<String>, width: Length, value_color: iced::Color) -> Element<'a, Message> {
  sized_cell(
    TableCell::new(value)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .color(value_color),
    width,
  )
}

fn mono_cell<'a>(value: &str, width: Length, align: Horizontal, value_color: iced::Color) -> Element<'a, Message> {
  sized_cell(
    TableCell::new(value)
      .font(typography::mono::REGULAR)
      .align(align)
      .color(value_color),
    width,
  )
}

fn amount_cell<'a>(value: &str, width: Length, value_color: iced::Color) -> Element<'a, Message> {
  sized_cell(
    TableCell::new(value)
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .align(Horizontal::Right)
      .color(value_color),
    width,
  )
}

fn party_label(entry: &JournalEntry) -> &str {
  if entry.description.is_empty() {
    "\u{2014}"
  } else {
    &entry.description
  }
}

fn owner_name(state: &State, character_id: i64) -> String {
  state
    .roster
    .iter()
    .find(|pilot| pilot.id == character_id)
    .map_or_else(|| format!("#{character_id}"), |pilot| pilot.name.clone())
}

fn owner_display_name(state: &State, owner: BudgetOwner) -> String {
  match owner {
    BudgetOwner::Character(id) => owner_name(state, id),
    BudgetOwner::Corporation(id) => state
      .corporations
      .iter()
      .find(|corp| corp.id == id)
      .map_or_else(|| format!("#{id}"), |corp| corp.name.clone()),
  }
}

fn owner_portrait(state: &State, owner: BudgetOwner) -> Option<std::path::PathBuf> {
  match owner {
    BudgetOwner::Character(id) => roster_portrait(state, id),
    BudgetOwner::Corporation(id) => state
      .corporations
      .iter()
      .find(|corp| corp.id == id)
      .and_then(|corp| corp.logo.path()),
  }
}

fn fmt_relative(iso: &str, now: DateTime<Utc>) -> String {
  let Ok(parsed) = DateTime::parse_from_rfc3339(iso) else {
    return iso.split('T').next().unwrap_or(iso).to_owned();
  };
  let delta = now.signed_duration_since(parsed.with_timezone(&Utc));
  let days = delta.num_days();
  let hours = delta.num_hours();
  let minutes = delta.num_minutes();
  if days >= 1 {
    t!("wallet.shell.relative_days", days => days).into_owned()
  } else if hours >= 1 {
    t!("wallet.shell.relative_hours", hours => hours).into_owned()
  } else if minutes >= 1 {
    t!("wallet.shell.relative_minutes", minutes => minutes).into_owned()
  } else {
    t!("wallet.shell.relative_just_now").into_owned()
  }
}

fn empty_ledger(message: &str) -> Element<'_, Message> {
  container(
    text(message.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6)
  .into()
}

fn no_source_state<'a>(title: &str, detail: &str) -> Element<'a, Message> {
  let body = Column::with_children(vec![
    text(title.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(detail.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_x(Horizontal::Center);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(spacing::SPACE_6)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn now() -> DateTime<Utc> {
    Utc::now()
  }

  mod contract_status_cell {
    use super::*;

    #[test]
    fn it_renders_each_status() {
      for status in [
        "outstanding",
        "in_progress",
        "finished",
        "expired",
        "outbid",
        "cancelled",
      ] {
        let _el: Element<'_, Message> = super::super::contract_status_cell(status, Length::FillPortion(2));
      }
    }
  }

  mod contracts_tab {
    use super::*;

    fn contract(is_buy: bool, status: &str, contract_type: &str) -> ContractEntry {
      ContractEntry {
        acceptor: None,
        acceptor_id: None,
        acceptor_image: PartyImage::default(),
        assignee: Some("Assignee Pilot".to_owned()),
        assignee_id: Some(98_765),
        assignee_image: PartyImage::default(),
        character_id: 1,
        collateral: Some(5_000.0),
        contract_id: 42,
        date_expired: None,
        date_issued: "2026-05-30T12:00:00Z".to_owned(),
        is_buy,
        issuer: Some("Issuer Pilot".to_owned()),
        issuer_id: 11_111,
        issuer_image: PartyImage::default(),
        item_names: Vec::new(),
        status: status.to_owned(),
        value: Some(200.0),
        r#type: contract_type.to_owned(),
      }
    }

    fn state_on_contracts() -> State {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Contracts;
      state
    }

    #[test]
    fn it_renders_a_populated_contract_list() {
      let mut state = state_on_contracts();
      state.contracts = vec![
        contract(true, "outstanding", "courier"),
        contract(false, "finished", "item_exchange"),
      ];
      state.recompute_derived();

      let _el: Element<'_, Message> = shell(&state, now());
      assert!(state.has_contracts());
    }

    #[test]
    fn it_renders_corporation_contracts_in_the_table() {
      let mut state = state_on_contracts();
      state.active = Scope::Corporation(98_000_001);
      state.contracts = vec![contract(false, "finished", "item_exchange")];
      state.recompute_derived();

      let _el: Element<'_, Message> = shell(&state, now());
      assert!(state.has_contracts());
    }

    #[test]
    fn it_renders_the_no_match_state_when_the_side_filter_excludes_all() {
      let mut state = state_on_contracts();
      state.contracts = vec![contract(true, "outstanding", "courier")];
      state.side_filter = crate::features::wallet::Side::Sell;
      state.recompute_derived();

      let _el: Element<'_, Message> = shell(&state, now());
      assert!(crate::features::wallet::filtered_contracts(&state).is_empty());
    }

    #[test]
    fn it_renders_the_not_synced_state_when_no_contracts_exist() {
      let state = state_on_contracts();

      let _el: Element<'_, Message> = shell(&state, now());
      assert!(!state.has_contracts());
    }
  }

  mod journal_tab {
    use pretty_assertions::assert_eq;

    use super::*;

    fn journal_entry(amount: Option<f64>, ref_type: &str) -> JournalEntry {
      JournalEntry {
        amount,
        balance: Some(5_000.0),
        character_id: 1,
        context_id: None,
        date: "2026-05-30T12:00:00Z".to_owned(),
        description: "Bounty payout".to_owned(),
        id: 1,
        owner: BudgetOwner::Character(1),
        reason: None,
        ref_type: ref_type.to_owned(),
      }
    }

    fn state_on_journal() -> State {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Journal;
      state
    }

    #[test]
    fn it_renders_a_glyph_badge_card_list() {
      let mut state = state_on_journal();
      state.journal = vec![
        journal_entry(Some(1_000.0), "bounty_prizes"),
        journal_entry(Some(-400.0), "brokers_fee"),
      ];
      state.recompute_derived();

      let _el: Element<'_, Message> = shell(&state, now());
      assert_eq!(crate::features::wallet::filtered_journal(&state).len(), 2);
    }

    #[test]
    fn it_renders_a_row_for_an_amountless_entry_via_ref_type_direction() {
      let mut state = state_on_journal();
      state.journal = vec![journal_entry(None, "agent_mission_reward")];
      state.recompute_derived();

      let _el: Element<'_, Message> = shell(&state, now());
    }

    #[test]
    fn it_renders_the_empty_state_when_no_entries_match() {
      let state = state_on_journal();

      let _el: Element<'_, Message> = shell(&state, now());
      assert!(crate::features::wallet::filtered_journal(&state).is_empty());
    }

    #[test]
    fn it_windows_a_large_ledger_with_a_scroll_offset_without_materializing_every_row() {
      let mut state = state_on_journal();
      state.journal = (0..2_000)
        .map(|index| {
          let mut entry = journal_entry(Some(index as f64), "bounty_prizes");
          entry.id = index;
          entry
        })
        .collect();
      state.tab_scroll_offset = 30_000.0;
      state.recompute_derived();

      let _el: Element<'_, Message> = shell(&state, now());
      assert_eq!(crate::features::wallet::filtered_journal(&state).len(), 2_000);
    }
  }

  mod ledger_menu_overlay {
    use super::*;

    fn populated_journal_state() -> State {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Journal;
      state.journal = (0..50)
        .map(|index| JournalEntry {
          amount: Some(index as f64),
          balance: Some(5_000.0),
          character_id: 1,
          context_id: None,
          date: "2026-05-30T12:00:00Z".to_owned(),
          description: "Bounty payout".to_owned(),
          id: index,
          owner: BudgetOwner::Character(1),
          reason: None,
          ref_type: "bounty_prizes".to_owned(),
        })
        .collect();
      state.tab_scroll_offset = 1_200.0;
      state.recompute_derived();
      state
    }

    fn open_bulk_assign_menu(state: &mut State) {
      state.ledger_menu = Some(crate::features::wallet::LedgerMenu {
        anchor: iced::Point::new(40.0, 120.0),
        picking: false,
        tab: Tab::Journal,
      });
    }

    fn root_children(el: &Element<'_, Message>) -> usize {
      let mut tree = iced::advanced::widget::Tree::new(el);
      tree.diff(el);
      tree.children.len()
    }

    fn base_tag(el: &Element<'_, Message>) -> iced::advanced::widget::tree::Tag {
      let mut tree = iced::advanced::widget::Tree::new(el);
      tree.diff(el);
      tree.children[0].tag
    }

    #[test]
    fn the_closed_shell_already_mounts_the_base_inside_a_stack() {
      let state = populated_journal_state();

      assert_eq!(root_children(&shell(&state, now())), 1);
    }

    #[test]
    fn opening_the_menu_only_adds_sibling_layers_above_the_base() {
      let mut state = populated_journal_state();
      open_bulk_assign_menu(&mut state);

      assert_eq!(root_children(&shell(&state, now())), 3);
    }

    #[test]
    fn opening_the_menu_keeps_the_ledger_base_at_a_stable_tree_position() {
      let mut closed = populated_journal_state();
      let closed_tag = base_tag(&shell(&closed, now()));

      open_bulk_assign_menu(&mut closed);
      let open_tag = base_tag(&shell(&closed, now()));

      assert_eq!(closed_tag, open_tag);
    }

    #[test]
    fn the_ledger_base_tracks_the_cursor_at_the_feature_root() {
      let mut wallets = populated_journal_state();
      wallets.tab = Tab::Wallets;
      let journal = populated_journal_state();

      assert_ne!(
        base_tag(&shell(&journal, now())),
        base_tag(&shell(&wallets, now())),
        "the journal base must be the cursor-tracking mouse_area, not a bare container"
      );
    }
  }

  mod market_tab {
    use super::*;
    use crate::store::images::IconResolution;

    fn market_entry(is_buy: bool) -> MarketEntry {
      MarketEntry {
        character_id: 1,
        corp_journal_twin_exists: true,
        date: "2026-05-30T12:00:00Z".to_owned(),
        is_buy,
        is_personal: true,
        item: "Tritanium".to_owned(),
        journal_ref_id: 0,
        location: "Jita IV - Moon 4".to_owned(),
        owner: BudgetOwner::Character(1),
        quantity: 1_000,
        total: 5_000.0,
        transaction_id: 1,
        type_icon: IconResolution::Missing,
        type_id: 34,
        unit_price: 5.0,
      }
    }

    fn state_on_market() -> State {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Market;
      state
    }

    #[test]
    fn it_renders_buy_and_sell_side_badges() {
      let mut state = state_on_market();
      state.market = vec![
        MarketEntry {
          transaction_id: 1,
          ..market_entry(true)
        },
        MarketEntry {
          transaction_id: 2,
          ..market_entry(false)
        },
      ];
      state.recompute_derived();

      let _el: Element<'_, Message> = shell(&state, now());
      assert_eq!(crate::features::wallet::filtered_market(&state).len(), 2);
    }

    #[test]
    fn it_renders_the_empty_state_when_no_transactions_match() {
      let state = state_on_market();

      let _el: Element<'_, Message> = shell(&state, now());
      assert!(crate::features::wallet::filtered_market(&state).is_empty());
    }
  }

  mod pinned_header {
    use super::*;

    #[test]
    fn it_pins_a_header_for_market_and_contracts_but_not_journal() {
      let mut state = State::new(crate::config::FeatureFlags::default());

      state.tab = Tab::Market;
      assert!(super::super::pinned_header(&state).is_some());

      state.tab = Tab::Contracts;
      assert!(super::super::pinned_header(&state).is_some());

      state.tab = Tab::Journal;
      assert!(super::super::pinned_header(&state).is_none());
    }
  }

  mod budget_gating {
    use super::*;

    fn budget_off() -> crate::config::FeatureFlags {
      let mut flags = crate::config::FeatureFlags::default();
      flags.set_sub_enabled(crate::config::SubFeature::Budget, false);
      flags
    }

    #[test]
    fn the_chip_is_suppressed_when_the_budget_sub_feature_is_off() {
      let on = State::new(crate::config::FeatureFlags::default());
      let off = State::new(budget_off());

      assert!(on.budget_enabled(), "Budget chips render while the sub-feature is on");
      assert!(
        !off.budget_enabled(),
        "the journal/market chip columns are dropped while Budget is off"
      );
    }

    #[test]
    fn the_market_header_renders_with_and_without_the_budget_column() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.tab = Tab::Market;
      let _with_budget: Element<'_, Message> = super::super::market_header(&state);

      state.sync_features(budget_off());
      let _without_budget: Element<'_, Message> = super::super::market_header(&state);
    }
  }

  mod shell {
    use super::*;

    #[test]
    fn it_renders_the_all_wallets_body() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> = shell(&state, now());
    }

    #[tokio::test]
    async fn it_renders_with_the_picker_open() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new(crate::config::FeatureFlags::default());
      let _ = crate::features::wallet::update(&mut state, Message::PickerToggled, &db);
      let _el: Element<'_, Message> = shell(&state, now());
    }
  }

  fn envelope(id: i64, name: &str) -> crate::features::wallet::loaders::Envelope {
    crate::features::wallet::loaders::Envelope {
      id,
      name: name.to_owned(),
      tone: Some("plasma".to_owned()),
    }
  }

  mod budget_filter_badge {
    use super::*;
    use crate::features::wallet::{BudgetFilter, BudgetFilterKind};

    #[test]
    fn it_renders_nothing_without_an_active_filter() {
      let state = State::new(crate::config::FeatureFlags::default());

      assert!(super::super::budget_filter_badge(&state).is_none());
    }

    #[test]
    fn it_renders_a_named_envelope_for_a_category_filter() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.budget_chips.meta.insert(7, envelope(7, "Bills"));
      state.budget_filter = Some(BudgetFilter {
        kind: BudgetFilterKind::Category(7),
        month: "2026-06".to_owned(),
      });

      assert!(super::super::budget_filter_badge(&state).is_some());
    }

    #[test]
    fn it_renders_nothing_when_a_category_filter_has_no_envelope_meta() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.budget_filter = Some(BudgetFilter {
        kind: BudgetFilterKind::Category(7),
        month: "2026-06".to_owned(),
      });

      assert!(super::super::budget_filter_badge(&state).is_none());
    }

    #[test]
    fn it_renders_the_uncategorized_pill() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.budget_filter = Some(BudgetFilter {
        kind: BudgetFilterKind::Uncategorized,
        month: "2026-06".to_owned(),
      });

      assert!(super::super::budget_filter_badge(&state).is_some());
    }
  }

  mod budget_picker_row {
    use super::*;

    #[test]
    fn it_renders_a_selected_and_an_unselected_row() {
      let _selected: Element<'_, Message> = super::super::budget_picker_row(
        "Bills",
        true,
        color::status::WARNING,
        Message::BudgetChipAssigned(Some(7)),
      );
      let _unselected: Element<'_, Message> = super::super::budget_picker_row(
        "Uncategorized",
        false,
        color::status::WARNING,
        Message::BudgetChipAssigned(None),
      );
    }
  }

  mod budget_chip {
    use super::*;

    #[test]
    fn it_renders_the_assign_affordance_when_unassigned() {
      let state = State::new(crate::config::FeatureFlags::default());

      let _el: Element<'_, Message> =
        super::super::budget_chip(&state, BudgetOwner::Character(1), LedgerKind::Journal, 1);
    }

    #[test]
    fn it_renders_a_settled_chip_when_assigned() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.budget_chips.meta.insert(7, envelope(7, "Bills"));
      state
        .budget_chips
        .resolution
        .journal_overrides
        .insert((BudgetOwner::Character(1), 1), 7);

      let _el: Element<'_, Message> =
        super::super::budget_chip(&state, BudgetOwner::Character(1), LedgerKind::Journal, 1);
    }

    #[test]
    fn it_renders_the_open_popover() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.budget_chips.meta.insert(7, envelope(7, "Bills"));
      state.budget_chips.envelopes = vec![crate::features::wallet::loaders::EnvelopeGroup {
        categories: vec![envelope(7, "Bills")],
        name: "Group".to_owned(),
      }];
      state.budget_picker = Some((BudgetOwner::Character(1), LedgerKind::Journal, 1));

      let _el: Element<'_, Message> =
        super::super::budget_chip(&state, BudgetOwner::Character(1), LedgerKind::Journal, 1);
    }
  }

  mod owner_display_name {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
      features::wallet::{RosterCorp, RosterPilot},
      store::images,
    };

    fn pilot(id: i64, name: &str) -> RosterPilot {
      RosterPilot {
        corp: "TST".to_owned(),
        granted_scopes: None,
        id,
        liquid: None,
        name: name.to_owned(),
        portrait: images::ImageState::Stale {
          id,
          kind: images::ImageKind::CharacterPortrait,
        },
      }
    }

    fn corp(id: i64, name: &str) -> RosterCorp {
      RosterCorp {
        id,
        liquid: None,
        logo: images::ImageState::Stale {
          id,
          kind: images::ImageKind::CorporationLogo,
        },
        name: name.to_owned(),
        ticker: "TSTC".to_owned(),
      }
    }

    #[test]
    fn it_names_a_character_from_the_roster() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.roster = vec![pilot(1, "Pilot One")];

      assert_eq!(
        super::super::owner_display_name(&state, BudgetOwner::Character(1)),
        "Pilot One"
      );
    }

    #[test]
    fn it_names_a_corporation_from_the_roster() {
      let mut state = State::new(crate::config::FeatureFlags::default());
      state.corporations = vec![corp(98, "Test Corp")];

      assert_eq!(
        super::super::owner_display_name(&state, BudgetOwner::Corporation(98)),
        "Test Corp"
      );
    }

    #[test]
    fn it_falls_back_to_the_id_for_unknown_owners() {
      let state = State::new(crate::config::FeatureFlags::default());

      assert_eq!(
        super::super::owner_display_name(&state, BudgetOwner::Character(5)),
        "#5"
      );
      assert_eq!(
        super::super::owner_display_name(&state, BudgetOwner::Corporation(7)),
        "#7"
      );
    }
  }
}
