//! Wallet controller and view: multi-character ISK ledger.

pub mod contracts_tab;
pub mod header;
pub mod journal_tab;
pub mod main_panel;
pub mod mappings;
pub mod market_tab;
pub mod net_worth_hero;
pub mod right_rail;

use std::collections::HashMap;

pub use header::Component as Header;
use iced::{
  Background, Element, Event, Length, Padding, Subscription, mouse,
  widget::{Space, column, container, image, mouse_area, row},
};
pub use main_panel::Component as MainPanel;
pub use mappings::journal_type_glyph;
pub use net_worth_hero::Component as NetWorthHero;
use pod_model::missing_scopes;
pub use right_rail::Component as RightRail;

use crate::{
  components::{CharacterPicker, ScopeMissing, character_picker, scope_missing},
  format,
  style::{color, spacing},
};

/// A character's wallet summary.
#[derive(Clone, Debug)]
pub struct WalletCharacter {
  pub id: i64,
  pub name: String,
  pub corp: String,
  pub liquid: f64,
  pub assets: f64,
  pub escrow: f64,
  pub granted_scopes: Option<String>,
  pub portrait_tone: u16,
  pub portrait_handle: Option<image::Handle>,
}

/// A wallet journal entry.
#[derive(Clone, Debug)]
pub struct JournalEntry {
  pub id: String,
  pub who: i64,
  pub entry_type: String,
  pub delta: f64,
  pub ts_secs: u64,
  pub reference: String,
  pub party: String,
  pub location: String,
}

/// A filled market order.
#[derive(Clone, Debug)]
pub struct MarketEntry {
  pub id: String,
  pub who: i64,
  pub type_id: i32,
  pub side: String,
  pub qty: u64,
  pub item: String,
  pub unit: f64,
  pub total: f64,
  pub fee: f64,
  pub ts_secs: u64,
  pub location: String,
}

/// A player contract.
#[derive(Clone, Debug)]
pub struct ContractEntry {
  pub id: String,
  pub who: i64,
  pub kind: String,
  pub status: String,
  pub title: String,
  pub counterparty: String,
  pub price: f64,
  pub collateral: f64,
  pub ts_secs: u64,
  pub location: String,
}

/// Active wallet tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tab {
  Market,
  Contracts,
  Journal,
}

/// All/In/Out filter for the journal tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignFilter {
  All,
  In,
  Out,
}

/// All/Buy/Sell filter for the market tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideFilter {
  All,
  Buy,
  Sell,
}

/// Net-worth chart timeframe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Timeframe {
  W1,
  M1,
  M3,
  M6,
  Y1,
}

impl Timeframe {
  pub fn label(&self) -> &'static str {
    match self {
      Self::W1 => "1W",
      Self::M1 => "1M",
      Self::M3 => "3M",
      Self::M6 => "6M",
      Self::Y1 => "1Y",
    }
  }

  pub fn days(&self) -> usize {
    match self {
      Self::W1 => 7,
      Self::M1 => 30,
      Self::M3 => 90,
      Self::M6 => 180,
      Self::Y1 => 365,
    }
  }

  pub fn all() -> &'static [Timeframe] {
    &[
      Timeframe::W1,
      Timeframe::M1,
      Timeframe::M3,
      Timeframe::M6,
      Timeframe::Y1,
    ]
  }
}

/// Which pane divider is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraggingPane {
  RightRail,
}

/// Messages produced by the wallet controller.
#[derive(Clone, Debug)]
pub enum Message {
  AllCorpBalancesLoaded(Vec<(i64, f64)>),
  ReauthorizeCharacter(i64),
  AssetValuesLoaded(Vec<(i64, f64)>),
  CharacterPicker(character_picker::Message),
  ContractsLoaded(Vec<ContractEntry>),
  ContractsTab(contracts_tab::Message),
  CorpDataLoaded {
    divisions: Vec<(u8, f64)>,
    journal: Vec<JournalEntry>,
    market: Vec<MarketEntry>,
  },
  DivisionSelected(u8),
  ItemIconsLoaded(Vec<(i32, Vec<u8>)>),
  JournalLoaded(Vec<JournalEntry>),
  JournalTab(journal_tab::Message),
  MarketTab(market_tab::Message),
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart(DraggingPane),
  SearchChanged(String),
  TabSelected(Tab),
  TimeframeChanged(Timeframe),
  TransactionsLoaded(Vec<MarketEntry>),
}

/// Runtime state for the wallet controller.
pub struct State {
  pub active_division: u8,
  pub active_tab: Tab,
  pub all_corp_balances: Vec<(i64, f64)>,
  pub characters: Vec<WalletCharacter>,
  pub chart_series: Vec<f64>,
  pub contracts: Vec<ContractEntry>,
  pub corp_divisions: Vec<(u8, f64)>,
  pub corp_journal: Vec<JournalEntry>,
  pub corp_market: Vec<MarketEntry>,
  pub drag_origin: Option<(f32, f32)>,
  pub dragging_pane: Option<DraggingPane>,
  pub filtered_contracts: Vec<ContractEntry>,
  pub filtered_journal: Vec<JournalEntry>,
  pub filtered_market: Vec<MarketEntry>,
  pub item_icons: HashMap<i32, image::Handle>,
  pub journal: Vec<JournalEntry>,
  pub journal_income: f64,
  pub journal_spend: f64,
  pub market: Vec<MarketEntry>,
  pub net_worth_change: f64,
  pub net_worth_series: Vec<f64>,
  pub picker: CharacterPicker,
  pub right_rail_width: f32,
  pub search_query: String,
  pub side_filter: SideFilter,
  pub sign_filter: SignFilter,
  pub timeframe: Timeframe,
}

impl State {
  pub fn is_corp_selected(&self) -> bool {
    self.picker.selected_corporation_id().is_some()
  }

  pub fn selected_character(&self) -> Option<i64> {
    self.picker.selected_character_id()
  }

  pub fn selected_corporation(&self) -> Option<i64> {
    self.picker.selected_corporation_id()
  }

  pub fn total_assets(&self) -> f64 {
    if self.is_corp_selected() {
      return 0.0;
    }
    match self.selected_character() {
      None => self.characters.iter().map(|c| c.assets).sum(),
      Some(id) => self.characters.iter().find(|c| c.id == id).map_or(0.0, |c| c.assets),
    }
  }

  pub fn total_escrow(&self) -> f64 {
    if self.is_corp_selected() {
      return 0.0;
    }
    match self.selected_character() {
      None => self.characters.iter().map(|c| c.escrow).sum(),
      Some(id) => self.characters.iter().find(|c| c.id == id).map_or(0.0, |c| c.escrow),
    }
  }

  pub fn total_liquid(&self) -> f64 {
    if self.is_corp_selected() {
      return self
        .corp_divisions
        .iter()
        .find(|(d, _)| *d == self.active_division)
        .map_or(0.0, |(_, bal)| *bal);
    }
    match self.selected_character() {
      None => {
        let char_total: f64 = self.characters.iter().map(|c| c.liquid).sum();
        let corp_total: f64 = self.all_corp_balances.iter().map(|(_, bal)| bal).sum();
        char_total + corp_total
      }
      Some(id) => self.characters.iter().find(|c| c.id == id).map_or(0.0, |c| c.liquid),
    }
  }
}

/// View title for the wallet section.
pub fn title() -> &'static str {
  "Wallet"
}

/// Returns a subscription that tracks cursor movement during pane drag.
pub fn subscription(state: &State) -> Subscription<Message> {
  if state.dragging_pane.is_none() {
    return Subscription::none();
  }
  iced::event::listen_with(|event, _status, _id| match event {
    Event::Mouse(mouse::Event::CursorMoved {
      position,
    }) => Some(Message::PaneDrag(position.x)),
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::PaneDragEnd),
    _ => None,
  })
}

fn scope_gate(state: &State) -> Option<Element<'_, Message>> {
  let char_id = state.selected_character()?;
  let wc = state.characters.iter().find(|c| c.id == char_id)?;
  let granted_str = wc.granted_scopes.as_deref().unwrap_or("");
  let granted: Vec<&str> = if granted_str.is_empty() {
    Vec::new()
  } else {
    granted_str.split(' ').collect()
  };
  if missing_scopes(&granted, &["esi-wallet.read_character_wallet.v1"]).is_empty() {
    return None;
  }
  Some(ScopeMissing::new(char_id, "wallet").render().map(|m| match m {
    scope_missing::Message::ReauthorizePressed(id) => Message::ReauthorizeCharacter(id),
  }))
}

fn wallet_base<'a>(state: &'a State, window_width: f32) -> Element<'a, Message> {
  let right_w = effective_right_rail_width(state, window_width);
  let header = Header::new(state).render();
  let hero = NetWorthHero::new(state).render();
  let main = MainPanel::new(state).render();
  let right = RightRail::new(state).width(right_w).render();
  let body: Element<'_, Message> = column([
    hero,
    row([main, right_rail_drag_handle(), right])
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into();
  container(column([header, body]))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn picker_dropdown_overlay(state: &State) -> Option<Element<'_, Message>> {
  if !state.picker.is_open {
    return None;
  }
  let dropdown = state.picker.dropdown().map(Message::CharacterPicker);
  Some(
    container(dropdown)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(iced::alignment::Horizontal::Left)
      .padding(Padding {
        top: spacing::layout::HEADER_HEIGHT + 8.0,
        left: spacing::SPACE_8,
        ..Padding::ZERO
      })
      .into(),
  )
}

fn drag_capture_layer() -> Element<'static, Message> {
  mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
    .on_move(|pt| Message::PaneDrag(pt.x))
    .on_release(Message::PaneDragEnd)
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn drag_handle_inner() -> Element<'static, Message> {
  row([
    Space::new().width(1.5).height(Length::Fill).into(),
    container(Space::new().width(1.0).height(Length::Fill))
      .width(1.0)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::border::SUBTLE)),
        ..container::Style::default()
      })
      .into(),
    Space::new().width(1.5).height(Length::Fill).into(),
  ])
  .width(4.0)
  .height(Length::Fill)
  .into()
}

fn right_rail_drag_handle() -> Element<'static, Message> {
  mouse_area(drag_handle_inner())
    .on_press(Message::PaneDragStart(DraggingPane::RightRail))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn effective_right_rail_width(state: &State, window_width: f32) -> f32 {
  let content = window_width - (spacing::layout::RAIL_WIDTH + 1.0);
  state.right_rail_width.clamp(160.0, (content * 0.35).max(160.0))
}

/// Processes a wallet message and returns a task.
pub fn update(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::ReauthorizeCharacter(_) => {}
    Message::AllCorpBalancesLoaded(balances) => {
      state.all_corp_balances = balances;
    }
    Message::AssetValuesLoaded(values) => update_asset_values(state, values),
    Message::CharacterPicker(msg) => update_character_picker(state, msg),
    Message::ContractsLoaded(entries) => {
      state.contracts = entries;
    }
    Message::ContractsTab(_msg) => {}
    Message::CorpDataLoaded {
      divisions,
      journal,
      market,
    } => {
      state.corp_divisions = divisions;
      state.corp_journal = journal;
      state.corp_market = market;
    }
    Message::DivisionSelected(div) => {
      state.active_division = div;
    }
    Message::ItemIconsLoaded(icons) => update_item_icons(state, icons),
    Message::JournalLoaded(entries) => {
      state.journal = entries;
    }
    Message::JournalTab(msg) => match msg {
      journal_tab::Message::SignFilterChanged(sign) => {
        state.sign_filter = sign;
      }
    },
    Message::MarketTab(msg) => match msg {
      market_tab::Message::SideFilterChanged(side) => {
        state.side_filter = side;
      }
    },
    Message::PaneDrag(cursor_x) => update_pane_drag(state, cursor_x),
    Message::PaneDragEnd => {
      state.dragging_pane = None;
      state.drag_origin = None;
    }
    Message::PaneDragStart(pane) => {
      state.dragging_pane = Some(pane);
      state.drag_origin = None;
    }
    Message::SearchChanged(q) => {
      state.search_query = q;
    }
    Message::TabSelected(tab) => {
      state.active_tab = tab;
      state.search_query.clear();
    }
    Message::TimeframeChanged(tf) => {
      state.timeframe = tf;
    }
    Message::TransactionsLoaded(entries) => {
      state.market = entries;
    }
  }
  iced::Task::none()
}

fn update_asset_values(state: &mut State, values: Vec<(i64, f64)>) {
  for (char_id, total) in values {
    if let Some(c) = state.characters.iter_mut().find(|c| c.id == char_id) {
      c.assets = total;
    }
  }
}

fn update_character_picker(state: &mut State, msg: character_picker::Message) {
  if let character_picker::Message::Select(_) = &msg {
    state.corp_journal.clear();
    state.corp_market.clear();
    state.corp_divisions.clear();
    state.active_division = 1;
  }
  state.picker.update(msg);
}

fn update_item_icons(state: &mut State, icons: Vec<(i32, Vec<u8>)>) {
  for (type_id, bytes) in icons {
    state.item_icons.insert(type_id, image::Handle::from_bytes(bytes));
  }
}

fn update_pane_drag(state: &mut State, cursor_x: f32) {
  if let Some(DraggingPane::RightRail) = state.dragging_pane {
    let (start_x, start_w) = state.drag_origin.get_or_insert((cursor_x, state.right_rail_width));
    let delta = cursor_x - *start_x;
    state.right_rail_width = (*start_w - delta).clamp(160.0, 400.0);
  }
}

/// Format a timestamp offset (seconds ago) as a relative label.
pub fn ts_label(ts_secs: u64) -> String {
  format::fmt_dur_short(ts_secs) + " ago"
}

/// Builder for the wallet view.
pub struct Component<'a> {
  state: &'a State,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Create a new view builder for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_width: 1200.0,
    }
  }

  /// Set the window width for pane clamping.
  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  /// Consume the builder and return the finished [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    use iced::widget::stack;

    let state = self.state;

    if let Some(el) = scope_gate(state) {
      return el;
    }

    let base = wallet_base(state, self.window_width);
    let mut layers: Vec<Element<'_, Message>> = vec![base];

    if let Some(overlay) = picker_dropdown_overlay(state) {
      layers.push(overlay);
    }
    if state.dragging_pane.is_some() {
      layers.push(drag_capture_layer());
    }

    if layers.len() == 1 {
      layers.remove(0)
    } else {
      stack(layers).into()
    }
  }
}
