mod budget;
mod budget_reflect;
mod budget_view;
mod header;
mod hero;
mod loaders;
mod selection;
mod shell;
mod side_filter;
mod wallets_view;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use iced::{Element, Task};

pub use self::{
  loaders::{ContractEntry, JournalEntry, MarketEntry, PartyImage},
  side_filter::Side,
};
pub(crate) use crate::ui::format::fmt_isk_opt as fmt_isk;
use crate::{
  features::contract_detail,
  store::{
    Database, images,
    model::{
      BudgetEntryKind, BudgetOwner, MatchMode, OwnerType, RuleField, RuleOp, character_financials::CharacterFinancials,
      character_wallet_period_summary::CharacterWalletPeriodSummary,
    },
    repo::{character, finance, infra, org},
  },
  sync::JobKind,
  ui::components::resizable_pane::PaneDrag,
  window_state,
};

/// The wallet-read roles whose holder grants the player visibility of a corp's
/// division balances — the same gate the corporation-wallet sync enforces. The
/// granting pilot's strongest such role drives the "via · " attribution caption.
const ACCOUNTING_ROLES: &[&str] = &["Director", "Accountant", "Junior_Accountant"];

const BUDGET_INSPECTOR_DEFAULT_WIDTH: f32 = 300.0;

const BUDGET_INSPECTOR_PANE_KEY: &str = "wallet.budget_inspector";

const DEFAULT_DIVISION: i64 = 1;

const HEADER_SIDE_PADDING: f32 = 28.0;

const HISTORY_MONTHS: usize = 6;

pub const PAGE_SIZE: usize = 50;

const RECENT_ACTIVITY_LIMIT: usize = 8;

const RIGHT_RAIL_DEFAULT_WIDTH: f32 = 280.0;

const RIGHT_RAIL_PANE_KEY: &str = "wallet.right_rail";

/// Fraction of the ledger a scroll must reach before the next cursor page is
/// fetched. The window only ever materializes the viewport's rows, so this only
/// gates how early the next DB page starts streaming in behind the scroll.
const SCROLL_LOAD_THRESHOLD: f32 = 0.8;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CategoryFlow {
  pub income: f64,
  pub ref_type: String,
  pub spend: f64,
}

impl CategoryFlow {
  pub fn label(&self) -> String {
    humanize_ref_type(&self.ref_type)
  }

  pub fn total(&self) -> f64 {
    self.income + self.spend
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Composition {
  pub asset_value: Option<f64>,
  pub escrow: Option<f64>,
  pub liquid: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositionSlice {
  pub id: i64,
  pub name: String,
  pub net_worth: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CorpDivision {
  pub balance: Option<f64>,
  pub division: i64,
  pub name: Option<String>,
}

impl CorpDivision {
  pub fn label(&self) -> String {
    self
      .name
      .clone()
      .unwrap_or_else(|| format!("Division {}", self.division))
  }
}

/// One accessible corporation's wallet, broken into its divisions, plus the
/// pilot/role that grants the player read access — the "via · " attribution on
/// the Wallets balances tab. Held for every owned corp so the All-scope view can
/// render the full per-corp breakdown without re-querying on tab select.
#[derive(Clone, Debug, PartialEq)]
pub struct CorpWalletSection {
  pub divisions: Vec<CorpDivision>,
  pub granted_by: Option<String>,
  pub id: i64,
  pub logo: images::ImageState,
  pub name: String,
  pub role: Option<String>,
  pub ticker: String,
}

impl CorpWalletSection {
  pub fn subtotal(&self) -> f64 {
    self.divisions.iter().filter_map(|division| division.balance).sum()
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JournalFlow {
  pub income: f64,
  pub net: f64,
  pub spend: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Loaded {
  chips: loaders::BudgetChips,
  contract_total: i64,
  contracts: Vec<ContractEntry>,
  corp_divisions: Vec<CorpDivision>,
  corporations: Vec<RosterCorp>,
  financials: Vec<CharacterFinancials>,
  journal: Vec<JournalEntry>,
  journal_total: i64,
  market: Vec<MarketEntry>,
  market_total: i64,
  net_worth_series: Vec<NetWorthPoint>,
  periods: Vec<CharacterWalletPeriodSummary>,
  right_rail_width: f32,
  roster: Vec<RosterPilot>,
  wallet_sections: Vec<CorpWalletSection>,
}

#[derive(Clone, Debug)]
pub struct BudgetLoad {
  history: Vec<crate::features::budget::MonthFlow>,
  scope: Scope,
  /// A category id to select once the reloaded view lands, used after adding a
  /// category so the new envelope opens in the inspector. `None` keeps the
  /// current selection.
  select: Option<i64>,
  view: budget::BudgetView,
}

/// A live drop slot while a category is being dragged in Budget edit mode.
/// `Category` drops the dragged envelope immediately before that row in its
/// group; `Group` appends it to the end of that group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetDropTarget {
  Category(i64),
  Group(i64),
}

/// Which trigger opened the Move Money popover. Both the row's Available pill
/// and the Inspector button open the same transfer, but only the trigger that
/// opened it floats the popover, so the two `AnchoredDropdown`s never stack two
/// copies for one source category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetMoveAnchor {
  Inspector,
  Pill,
}

/// Amount is prefilled with the source's `max(0, available)` when the popover
/// opens, then edited freely until a destination commits the transfer.
#[derive(Clone, Debug)]
pub struct BudgetMove {
  pub amount_draft: String,
  pub anchor: BudgetMoveAnchor,
  pub from_id: i64,
}

/// The right-click menu over the selected ledger rows. `picking` flips the menu
/// from its single "Assign to Budget" action to the reused envelope picker;
/// `tab` records which tab's selection the bulk assign applies to.
#[derive(Clone, Copy, Debug)]
struct LedgerMenu {
  anchor: iced::Point,
  picking: bool,
  tab: Tab,
}

#[derive(Clone, Debug)]
pub enum Message {
  BudgetAssignCancelled,
  BudgetAssignCommitted,
  BudgetAssignDraftChanged(String),
  BudgetAssignEditBegan(i64),
  BudgetAutoAssign,
  BudgetCategoryAdded(i64),
  BudgetCategoryDeleted(i64),
  BudgetCategoryHovered(Option<i64>),
  BudgetCategorySelected(i64),
  BudgetChipAssigned(Option<i64>),
  BudgetChipDismissed,
  BudgetChipOpened(BudgetOwner, BudgetEntryKind, i64),
  BudgetChipsReloaded(Box<loaders::BudgetChips>),
  BudgetCoverOverspending,
  BudgetDragStarted(i64),
  BudgetDropReleased,
  BudgetDropTargetEntered(BudgetDropTarget),
  BudgetDropTargetLeft,
  BudgetEditToggled,
  BudgetEditorAmountChanged(String),
  BudgetEditorByDateChanged(String),
  BudgetEditorCommitted,
  BudgetEditorKindSelected(budget::TargetKind),
  BudgetEditorNameChanged(String),
  BudgetEditorNoteChanged(String),
  BudgetEditorToggled,
  BudgetEditorToneSelected(String),
  BudgetFilterApplied(BudgetFilterKind),
  BudgetFilterCleared,
  BudgetGlobalRulesClosed,
  BudgetGlobalRulesOpened,
  BudgetGroupAdded,
  BudgetGroupDeleteRequested(i64),
  BudgetGroupDragStarted(i64),
  BudgetGroupRenameWritten,
  BudgetGroupRenamed(i64, String),
  BudgetGroupToggled(i64),
  BudgetInspectorDragEnd,
  BudgetInspectorDragged(f32),
  BudgetInspectorDragStart,
  BudgetInspectorTabSelected(budget::InspectorTab),
  BudgetLoaded(Box<BudgetLoad>),
  BudgetModeSelected(budget::Mode),
  BudgetMonthStepped(i32),
  BudgetMoveAmountChanged(String),
  BudgetMoveClosed,
  BudgetMoveCommitted(budget::MoveDest),
  BudgetMoveOpened(i64, BudgetMoveAnchor),
  BudgetQuickAssign(i64, f64),
  BudgetRangeSelected(budget::BudgetRange),
  BudgetRuleConditionAdded,
  BudgetRuleConditionFieldChanged(usize, RuleField),
  BudgetRuleConditionOpChanged(usize, RuleOp),
  BudgetRuleConditionRemoved(usize),
  BudgetRuleConditionValue2Changed(usize, String),
  BudgetRuleConditionValueChanged(usize, String),
  BudgetRuleDeleted(i64),
  BudgetRuleDragStarted(i64),
  BudgetRuleDropReleased,
  BudgetRuleDropTargetEntered(i64),
  BudgetRuleDropTargetLeft,
  BudgetRuleEditOpened(i64),
  BudgetRuleEditorAdvancedToggled,
  BudgetRuleEditorClosed,
  BudgetRuleEditorCommitted,
  BudgetRuleEditorMatchModeSelected(MatchMode),
  BudgetRuleEditorNameChanged(String),
  BudgetRuleEditorSearchChanged(String),
  BudgetRuleNewOpened(i64),
  BudgetRuleSelectToggled(Option<budget::RuleSelectKey>),
  BudgetRuleToggled(i64, bool),
  ChartHovered(Option<f32>),
  CloseContractDetail,
  ContractDetailLoaded(Box<Option<contract_detail::ContractDetail>>),
  ContractSelected(i64),
  DivisionSelected(i64),
  FiltersCleared,
  LedgerBulkAssignChosen(i64),
  LedgerBulkAssignOpened,
  LedgerCursorMoved(iced::Point),
  LedgerMenuDismissed,
  LedgerModifiersChanged(iced::keyboard::Modifiers),
  LedgerRowClicked(BudgetEntryKind, BudgetOwner, i64),
  LedgerRowRightPressed(BudgetEntryKind, BudgetOwner, i64),
  Loaded(Box<Loaded>),
  MoreLoaded(Box<MorePage>),
  PaneSettled(&'static str, f32),
  PickerToggled,
  RailDragEnd,
  RailDragged(f32),
  RailDragStart,
  ReauthRequested(i64),
  ScopeSelected(Scope),
  SearchChanged(String),
  SideFilterChanged(Side),
  SignFilterChanged(SignFilter),
  /// `relative` is the 0.0–1.0 scroll fraction that drives the cursor-pagination threshold;
  /// `absolute` is the pixel offset stored to window the visible ledger.
  TabScrolled {
    absolute: f32,
    relative: f32,
  },
  TabSelected(Tab),
  TimeframeSelected(Timeframe),
  WalletsSortSelected(WalletSort),
}

impl Message {
  /// Whether handling this message can surface new image-bearing rows, so the shell should recheck for stale
  /// icons/portraits. Interaction-only messages (scroll, hover, filter) return `false` to keep the staleness scan
  /// off the per-frame path.
  pub fn loads_data(&self) -> bool {
    matches!(
      self,
      Message::ContractDetailLoaded(_) | Message::Loaded(_) | Message::MoreLoaded(_)
    )
  }

  /// Whether this message belongs to the Budget surface, routed to
  /// [`handle_budget`] so the wallet dispatcher stays free of budget branching.
  fn is_budget(&self) -> bool {
    matches!(
      self,
      Message::BudgetAssignCancelled
        | Message::BudgetAssignCommitted
        | Message::BudgetAssignDraftChanged(_)
        | Message::BudgetAssignEditBegan(_)
        | Message::BudgetAutoAssign
        | Message::BudgetCategoryAdded(_)
        | Message::BudgetCategoryDeleted(_)
        | Message::BudgetCategorySelected(_)
        | Message::BudgetChipAssigned(_)
        | Message::BudgetChipDismissed
        | Message::BudgetChipOpened(..)
        | Message::BudgetChipsReloaded(_)
        | Message::BudgetCoverOverspending
        | Message::BudgetDragStarted(_)
        | Message::BudgetDropReleased
        | Message::BudgetDropTargetEntered(_)
        | Message::BudgetDropTargetLeft
        | Message::BudgetEditToggled
        | Message::BudgetEditorAmountChanged(_)
        | Message::BudgetEditorByDateChanged(_)
        | Message::BudgetEditorCommitted
        | Message::BudgetEditorKindSelected(_)
        | Message::BudgetEditorNameChanged(_)
        | Message::BudgetEditorNoteChanged(_)
        | Message::BudgetEditorToggled
        | Message::BudgetEditorToneSelected(_)
        | Message::BudgetGlobalRulesClosed
        | Message::BudgetGlobalRulesOpened
        | Message::BudgetGroupAdded
        | Message::BudgetGroupDeleteRequested(_)
        | Message::BudgetGroupDragStarted(_)
        | Message::BudgetGroupRenameWritten
        | Message::BudgetGroupRenamed(_, _)
        | Message::BudgetGroupToggled(_)
        | Message::BudgetInspectorTabSelected(_)
        | Message::BudgetLoaded(_)
        | Message::BudgetModeSelected(_)
        | Message::BudgetMonthStepped(_)
        | Message::BudgetMoveAmountChanged(_)
        | Message::BudgetMoveClosed
        | Message::BudgetMoveCommitted(_)
        | Message::BudgetMoveOpened(..)
        | Message::BudgetQuickAssign(_, _)
        | Message::BudgetRangeSelected(_)
        | Message::BudgetRuleConditionAdded
        | Message::BudgetRuleConditionFieldChanged(..)
        | Message::BudgetRuleConditionOpChanged(..)
        | Message::BudgetRuleConditionRemoved(_)
        | Message::BudgetRuleConditionValue2Changed(..)
        | Message::BudgetRuleConditionValueChanged(..)
        | Message::BudgetRuleDeleted(_)
        | Message::BudgetRuleDragStarted(_)
        | Message::BudgetRuleDropReleased
        | Message::BudgetRuleDropTargetEntered(_)
        | Message::BudgetRuleDropTargetLeft
        | Message::BudgetRuleEditOpened(_)
        | Message::BudgetRuleEditorAdvancedToggled
        | Message::BudgetRuleEditorClosed
        | Message::BudgetRuleEditorCommitted
        | Message::BudgetRuleEditorMatchModeSelected(_)
        | Message::BudgetRuleEditorNameChanged(_)
        | Message::BudgetRuleEditorSearchChanged(_)
        | Message::BudgetRuleNewOpened(_)
        | Message::BudgetRuleSelectToggled(_)
        | Message::BudgetRuleToggled(_, _)
    )
  }
}

#[derive(Clone, Debug, Default)]
pub struct MorePage {
  contracts: Vec<ContractEntry>,
  journal: Vec<JournalEntry>,
  market: Vec<MarketEntry>,
  scope: Scope,
  tab: Tab,
}

impl MorePage {
  fn contracts(contracts: Vec<ContractEntry>) -> Self {
    Self {
      contracts,
      ..Self::default()
    }
  }

  fn journal(journal: Vec<JournalEntry>) -> Self {
    Self {
      journal,
      ..Self::default()
    }
  }

  fn market(market: Vec<MarketEntry>) -> Self {
    Self {
      market,
      ..Self::default()
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetWorthPoint {
  pub date: String,
  pub liquid: f64,
  pub net_worth: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PeriodTotals {
  pub income: f64,
  pub net: f64,
  pub spend: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RosterCorp {
  pub id: i64,
  pub liquid: Option<f64>,
  pub logo: images::ImageState,
  pub name: String,
  pub ticker: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RosterPilot {
  pub corp: String,
  pub granted_scopes: Option<String>,
  pub id: i64,
  pub liquid: Option<f64>,
  pub name: String,
  pub portrait: images::ImageState,
}

/// A ledger filter driven from the Budget tab: show only the entries of a given
/// scope-keyed envelope (or the uncategorized ones) for a single month. Applies
/// to both the Journal and Transactions tables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetFilter {
  pub kind: BudgetFilterKind,
  pub month: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetFilterKind {
  Category(i64),
  Uncategorized,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scope {
  #[default]
  All,
  Character(i64),
  Corporation(i64),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SignFilter {
  #[default]
  All,
  In,
  Out,
}

/// Within-section ordering on the Wallets balances tab. Affects display only —
/// it never mutates the underlying balance data.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WalletSort {
  Ascending,
  #[default]
  Descending,
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  active_division: i64,
  budget: Option<budget::BudgetView>,
  budget_chips: loaders::BudgetChips,
  budget_collapsed: std::collections::HashSet<i64>,
  budget_dragging: Option<i64>,
  budget_drop_target: Option<BudgetDropTarget>,
  budget_edit_mode: bool,
  budget_editing: Option<budget::EditingCell>,
  budget_editor: Option<budget::CategoryDraft>,
  budget_filter: Option<BudgetFilter>,
  budget_global_rules_open: bool,
  budget_group_dragging: Option<i64>,
  budget_group_drop_target: Option<i64>,
  budget_history: Vec<crate::features::budget::MonthFlow>,
  budget_hovered_category: Option<i64>,
  budget_inspector: PaneDrag,
  budget_inspector_tab: budget::InspectorTab,
  budget_mode: budget::Mode,
  budget_month: String,
  budget_move: Option<BudgetMove>,
  budget_pending_group_delete: Option<i64>,
  budget_picker: Option<(BudgetOwner, BudgetEntryKind, i64)>,
  budget_range: budget::BudgetRange,
  budget_rule_dragging: Option<i64>,
  budget_rule_drop_target: Option<i64>,
  budget_rule_editor: Option<budget::RuleDraft>,
  budget_selected: Option<i64>,
  chart_hover: Option<f32>,
  contract_total: i64,
  contracts: Vec<ContractEntry>,
  corp_divisions: Vec<CorpDivision>,
  corporations: Vec<RosterCorp>,
  derived: Derived,
  dirty: bool,
  financials: Vec<CharacterFinancials>,
  journal: Vec<JournalEntry>,
  journal_selection: selection::RowSelection,
  journal_total: i64,
  ledger_cursor: Option<iced::Point>,
  ledger_menu: Option<LedgerMenu>,
  ledger_modifiers: iced::keyboard::Modifiers,
  loading_more: bool,
  market: Vec<MarketEntry>,
  market_selection: selection::RowSelection,
  market_total: i64,
  net_worth_series: Vec<NetWorthPoint>,
  periods: Vec<CharacterWalletPeriodSummary>,
  picker_open: bool,
  right_rail: PaneDrag,
  roster: Vec<RosterPilot>,
  search: String,
  selected_contract: Option<contract_detail::ContractDetail>,
  side_filter: Side,
  sign_filter: SignFilter,
  tab: Tab,
  tab_exhausted: bool,
  tab_scroll_offset: f32,
  timeframe: Timeframe,
  wallet_sections: Vec<CorpWalletSection>,
  wallets_sort: WalletSort,
}

impl State {
  pub fn new() -> Self {
    State {
      active: Scope::default(),
      active_division: DEFAULT_DIVISION,
      budget: None,
      budget_chips: loaders::BudgetChips::default(),
      budget_collapsed: std::collections::HashSet::new(),
      budget_dragging: None,
      budget_drop_target: None,
      budget_edit_mode: false,
      budget_editing: None,
      budget_editor: None,
      budget_filter: None,
      budget_global_rules_open: false,
      budget_group_dragging: None,
      budget_group_drop_target: None,
      budget_history: Vec::new(),
      budget_hovered_category: None,
      budget_inspector: PaneDrag::new(
        BUDGET_INSPECTOR_DEFAULT_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      )
      .right_anchored(true),
      budget_inspector_tab: budget::InspectorTab::default(),
      budget_mode: budget::Mode::default(),
      budget_month: budget::current_month(),
      budget_move: None,
      budget_pending_group_delete: None,
      budget_picker: None,
      budget_range: budget::BudgetRange::default(),
      budget_rule_dragging: None,
      budget_rule_drop_target: None,
      budget_rule_editor: None,
      budget_selected: None,
      chart_hover: None,
      contract_total: 0,
      contracts: Vec::new(),
      corp_divisions: Vec::new(),
      corporations: Vec::new(),
      derived: Derived::default(),
      dirty: false,
      financials: Vec::new(),
      journal: Vec::new(),
      journal_selection: selection::RowSelection::default(),
      journal_total: 0,
      ledger_cursor: None,
      ledger_menu: None,
      ledger_modifiers: iced::keyboard::Modifiers::default(),
      loading_more: false,
      market: Vec::new(),
      market_selection: selection::RowSelection::default(),
      market_total: 0,
      net_worth_series: Vec::new(),
      periods: Vec::new(),
      picker_open: false,
      right_rail: PaneDrag::new(
        RIGHT_RAIL_DEFAULT_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      )
      .right_anchored(true),
      roster: Vec::new(),
      search: String::new(),
      selected_contract: None,
      side_filter: Side::default(),
      sign_filter: SignFilter::default(),
      tab: Tab::default(),
      tab_exhausted: false,
      tab_scroll_offset: 0.0,
      timeframe: Timeframe::default(),
      wallet_sections: Vec::new(),
      wallets_sort: WalletSort::default(),
    }
  }

  pub fn with_restored_panes(mut self, ui: &window_state::UiState) -> Self {
    let host_width = ui.host_width("main", crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.right_rail =
      PaneDrag::from_store(ui, RIGHT_RAIL_PANE_KEY, RIGHT_RAIL_DEFAULT_WIDTH, host_width).right_anchored(true);
    self.budget_inspector = PaneDrag::from_store(
      ui,
      BUDGET_INSPECTOR_PANE_KEY,
      BUDGET_INSPECTOR_DEFAULT_WIDTH,
      host_width,
    )
    .right_anchored(true);
    self
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.right_rail.set_host_width(host_width);
    self.budget_inspector.set_host_width(host_width);
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn drain_dirty(&mut self, db: &Database) -> Option<Task<Message>> {
    if !self.dirty {
      return None;
    }
    self.dirty = false;
    Some(load(db))
  }

  #[cfg(test)]
  pub fn is_dirty(&self) -> bool {
    self.dirty
  }

  pub fn mark_dirty(&mut self, kind: JobKind) {
    if reload_kind(kind) {
      self.dirty = true;
    }
  }

  pub(super) fn scope_gate(&self) -> Option<(i64, &str, Vec<&'static str>)> {
    let Scope::Character(id) = self.active else {
      return None;
    };
    let pilot = self.roster.iter().find(|pilot| pilot.id == id)?;
    let required = crate::features::registry::descriptor(crate::config::Feature::Wallet).scopes;
    let missing = crate::ui::components::forbidden::missing_scopes(pilot.granted_scopes.as_deref(), required);
    if missing.is_empty() {
      return None;
    }
    Some((id, pilot.name.as_str(), missing))
  }

  pub fn active_division(&self) -> i64 {
    self.active_division
  }

  pub(super) fn budget(&self) -> Option<&budget::BudgetView> {
    self.budget.as_ref()
  }

  pub(super) fn budget_chips(&self) -> &loaders::BudgetChips {
    &self.budget_chips
  }

  /// Resolution order: a manual per-entry override wins, else the first enabled
  /// rule whose conditions match, else `None` (Ready-to-Assign).
  pub(super) fn budget_category_for(&self, owner: BudgetOwner, kind: BudgetEntryKind, entry_id: i64) -> Option<i64> {
    let resolution = &self.budget_chips.resolution;
    match kind {
      BudgetEntryKind::Journal => {
        let entry = self.journal.iter().find(|e| e.owner == owner && e.id == entry_id)?;
        resolution.resolve_target(kind, entry_id, &entry.match_target())
      }
      BudgetEntryKind::Market => {
        let entry = self
          .market
          .iter()
          .find(|e| e.owner == owner && e.transaction_id == entry_id)?;
        resolution.resolve_target(kind, entry_id, &entry.match_target())
      }
    }
  }

  pub(super) fn budget_filter(&self) -> Option<&BudgetFilter> {
    self.budget_filter.as_ref()
  }

  pub(super) fn budget_hovered_category(&self) -> Option<i64> {
    self.budget_hovered_category
  }

  /// How many loaded ledger entries in the selected budget month still need a
  /// category — the Review &amp; assign banner's count.
  pub(super) fn budget_uncategorized_count(&self) -> usize {
    let filter = BudgetFilter {
      kind: BudgetFilterKind::Uncategorized,
      month: self.budget_month.clone(),
    };
    let journal = self
      .journal
      .iter()
      .filter(|entry| journal_budget_match(entry, &filter, &self.budget_chips))
      .count();
    let market = self
      .market
      .iter()
      .filter(|entry| market_budget_match(entry, &filter, &self.budget_chips))
      .count();
    journal + market
  }

  pub(super) fn has_active_filters(&self) -> bool {
    self.budget_filter.is_some()
      || !self.search.is_empty()
      || self.sign_filter != SignFilter::All
      || self.side_filter != Side::All
  }

  pub(super) fn budget_collapsed(&self, group_id: i64) -> bool {
    self.budget_collapsed.contains(&group_id)
  }

  pub(super) fn budget_drop_target(&self) -> Option<BudgetDropTarget> {
    self.budget_drop_target
  }

  pub(super) fn budget_edit_mode(&self) -> bool {
    self.budget_edit_mode
  }

  pub(super) fn budget_editing(&self) -> Option<&budget::EditingCell> {
    self.budget_editing.as_ref()
  }

  pub(super) fn budget_editor(&self) -> Option<&budget::CategoryDraft> {
    self.budget_editor.as_ref()
  }

  pub(super) fn budget_global_rules_open(&self) -> bool {
    self.budget_global_rules_open
  }

  pub(super) fn budget_group_drop_target(&self) -> Option<i64> {
    self.budget_group_drop_target
  }

  pub(super) fn budget_history(&self) -> &[crate::features::budget::MonthFlow] {
    &self.budget_history
  }

  pub(super) fn budget_inspector_tab(&self) -> budget::InspectorTab {
    self.budget_inspector_tab
  }

  pub(super) fn budget_inspector_width(&self) -> f32 {
    self.budget_inspector.width()
  }

  pub(super) fn budget_is_past(&self) -> bool {
    self.budget_month.as_str() < budget::current_month().as_str()
  }

  /// Maps a target's index in [`Self::budget_match_targets`] to its manually-pinned
  /// category, so the rule editor's preview keeps hand-assigned rows out of a
  /// rule's reach. The iteration order MUST match `budget_match_targets` (journal
  /// then market, every row).
  pub(super) fn budget_manual_index(&self) -> std::collections::HashMap<usize, i64> {
    let resolution = &self.budget_chips.resolution;
    let mut map = std::collections::HashMap::new();
    let mut index = 0;
    for entry in &self.journal {
      if let Some(category) = resolution.journal_overrides.get(&(entry.owner, entry.id)) {
        map.insert(index, *category);
      }
      index += 1;
    }
    for entry in &self.market {
      if let Some(category) = resolution.market_overrides.get(&(entry.owner, entry.transaction_id)) {
        map.insert(index, *category);
      }
      index += 1;
    }
    map
  }

  pub(super) fn budget_mode(&self) -> budget::Mode {
    self.budget_mode
  }

  pub(super) fn budget_month(&self) -> &str {
    &self.budget_month
  }

  pub(super) fn budget_move(&self) -> Option<&BudgetMove> {
    self.budget_move.as_ref()
  }

  /// Every loaded ledger row flattened into a rule match target, for the rule
  /// editor's live preview and per-rule match counts. Includes both spending
  /// (outflows) and income (inflows), since rules match either. Drawn from the
  /// in-memory journal + market ledger (the loaded-and-paginated rows), mirroring
  /// the scope of the uncategorized-count surface.
  pub(super) fn budget_match_targets(&self) -> Vec<crate::features::budget::MatchTarget> {
    self
      .journal
      .iter()
      .map(loaders::JournalEntry::match_target)
      .chain(self.market.iter().map(loaders::MarketEntry::match_target))
      .collect()
  }

  pub(super) fn budget_pending_group_delete(&self) -> Option<i64> {
    self.budget_pending_group_delete
  }

  pub(super) fn budget_picker(&self) -> Option<(BudgetOwner, BudgetEntryKind, i64)> {
    self.budget_picker
  }

  pub(super) fn budget_range(&self) -> budget::BudgetRange {
    self.budget_range
  }

  pub(super) fn budget_rule_dragging(&self) -> Option<i64> {
    self.budget_rule_dragging
  }

  pub(super) fn budget_rule_drop_target(&self) -> Option<i64> {
    self.budget_rule_drop_target
  }

  pub(super) fn budget_rule_editor(&self) -> Option<&budget::RuleDraft> {
    self.budget_rule_editor.as_ref()
  }

  /// The live automation rules in priority order, loaded into the chips'
  /// resolution context.
  pub(super) fn budget_rules(&self) -> &[crate::store::model::Rule] {
    &self.budget_chips.resolution.rules
  }

  pub(super) fn budget_scope(&self) -> crate::store::model::BudgetScope {
    // There is exactly one budget. The active character/corporation selector is a
    // ledger-row filter, not a separate budget, so every budget read/write — the
    // picker, the per-row chips, the assignment, and the Budget tab — keys off the
    // single All budget regardless of which wallet the ledger is filtered to.
    crate::store::model::BudgetScope::All
  }

  pub(super) fn budget_selected(&self) -> Option<i64> {
    self.budget_selected
  }

  pub(super) fn category_flows(&self) -> &[CategoryFlow] {
    &self.derived.category_flows
  }

  pub fn corp_divisions(&self) -> &[CorpDivision] {
    &self.corp_divisions
  }

  pub(super) fn roster(&self) -> &[RosterPilot] {
    &self.roster
  }

  pub(super) fn wallet_sections(&self) -> &[CorpWalletSection] {
    &self.wallet_sections
  }

  pub(super) fn wallets_sort(&self) -> WalletSort {
    self.wallets_sort
  }

  pub(super) fn journal_selected(&self, owner: BudgetOwner, entry_id: i64) -> bool {
    self.journal_selection.contains((owner, entry_id))
  }

  pub(super) fn market_selected(&self, owner: BudgetOwner, entry_id: i64) -> bool {
    self.market_selection.contains((owner, entry_id))
  }

  pub(super) fn ledger_menu_open(&self) -> Option<(iced::Point, bool)> {
    self.ledger_menu.map(|menu| (menu.anchor, menu.picking))
  }

  pub(super) fn ledger_selection_count(&self) -> usize {
    match self.tab {
      Tab::Journal => self.journal_selection.len(),
      Tab::Market => self.market_selection.len(),
      Tab::Budget | Tab::Contracts | Tab::Wallets => 0,
    }
  }

  pub fn has_contracts(&self) -> bool {
    !self.contracts.is_empty()
  }

  pub(super) fn journal_flow(&self) -> JournalFlow {
    self.derived.journal_flow
  }

  pub(super) fn recent_activity(&self) -> Vec<&JournalEntry> {
    self
      .derived
      .recent_activity_indices
      .iter()
      .map(|&index| &self.journal[index])
      .collect()
  }

  pub(super) fn selected_contract(&self) -> Option<&contract_detail::ContractDetail> {
    self.selected_contract.as_ref()
  }

  pub fn side_filter(&self) -> Side {
    self.side_filter
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    let mut keys: Vec<(images::ImageKind, i64)> = Vec::new();
    keys.extend(self.roster.iter().filter_map(|pilot| pilot.portrait.stale_key()));
    keys.extend(self.corporations.iter().filter_map(|corp| corp.logo.stale_key()));
    keys.extend(
      self
        .wallet_sections
        .iter()
        .filter_map(|section| section.logo.stale_key()),
    );
    for contract in &self.contracts {
      keys.extend(contract.acceptor_image.stale.iter().copied());
      keys.extend(contract.assignee_image.stale.iter().copied());
      keys.extend(contract.issuer_image.stale.iter().copied());
    }
    if let Some(detail) = &self.selected_contract {
      keys.extend(detail.stale_images());
    }
    let mut seen = std::collections::HashSet::new();
    keys.retain(|key| seen.insert(*key));
    keys
  }

  pub fn tab_scroll_offset(&self) -> f32 {
    self.tab_scroll_offset
  }

  pub fn timeframe(&self) -> Timeframe {
    self.timeframe
  }

  fn corp_balance_total(&self) -> Option<f64> {
    let balances: Vec<f64> = self
      .corp_divisions
      .iter()
      .filter_map(|division| division.balance)
      .collect();
    if balances.is_empty() {
      None
    } else {
      Some(balances.iter().sum())
    }
  }

  fn owned_corp_liquid(&self) -> Option<f64> {
    sum_option(self.corporations.iter().map(|corp| corp.liquid))
  }

  fn recompute_derived(&mut self) {
    let query = self.search.to_lowercase();

    let budget_filter = self.budget_filter.as_ref();

    let journal_indices: Vec<usize> = self
      .journal
      .iter()
      .enumerate()
      .filter(|(_, entry)| journal_matches(entry, self.sign_filter, &query))
      .filter(|(_, entry)| budget_filter.is_none_or(|filter| journal_budget_match(entry, filter, &self.budget_chips)))
      .map(|(index, _)| index)
      .collect();

    let market_indices: Vec<usize> = self
      .market
      .iter()
      .enumerate()
      .filter(|(_, entry)| market_matches(entry, self.sign_filter, self.side_filter, &query))
      .filter(|(_, entry)| budget_filter.is_none_or(|filter| market_budget_match(entry, filter, &self.budget_chips)))
      .map(|(index, _)| index)
      .collect();

    let contract_indices: Vec<usize> = self
      .contracts
      .iter()
      .enumerate()
      .filter(|(_, entry)| contract_matches(entry, self.side_filter, &query))
      .map(|(index, _)| index)
      .collect();

    let matched: Vec<&JournalEntry> = journal_indices.iter().map(|&index| &self.journal[index]).collect();
    let journal_flow = journal_flow(&matched);
    let category_flows = category_flows(&matched);

    let mut recent_activity_indices = journal_indices.clone();
    recent_activity_indices.sort_by(|&a, &b| self.journal[b].date.cmp(&self.journal[a].date));
    recent_activity_indices.truncate(RECENT_ACTIVITY_LIMIT);

    self.derived = Derived {
      category_flows,
      contract_indices,
      journal_flow,
      journal_indices,
      market_indices,
      recent_activity_indices,
    };
  }

  fn scope_ids(&self) -> Vec<i64> {
    match self.active {
      Scope::All => self.roster.iter().map(|pilot| pilot.id).collect(),
      Scope::Character(id) => vec![id],
      Scope::Corporation(_) => Vec::new(),
    }
  }

  // Owned corporations whose division wallets the All scope unions into the
  // ledger; only the All view merges corp rows, so the keyed scopes contribute
  // none here (the per-corp view loads its own divisions separately).
  fn corp_scope_ids(&self) -> Vec<i64> {
    match self.active {
      Scope::All => self.corporations.iter().map(|corp| corp.id).collect(),
      Scope::Character(_) | Scope::Corporation(_) => Vec::new(),
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  Budget,
  Contracts,
  Journal,
  Market,
  #[default]
  Wallets,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Timeframe {
  HalfYear,
  Month,
  #[default]
  Quarter,
  Week,
  Year,
}

impl Timeframe {
  pub fn all() -> [Timeframe; 5] {
    [
      Timeframe::Week,
      Timeframe::Month,
      Timeframe::Quarter,
      Timeframe::HalfYear,
      Timeframe::Year,
    ]
  }

  pub fn days(self) -> usize {
    match self {
      Timeframe::HalfYear => 180,
      Timeframe::Month => 30,
      Timeframe::Quarter => 90,
      Timeframe::Week => 7,
      Timeframe::Year => 365,
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Timeframe::HalfYear => "6M",
      Timeframe::Month => "1M",
      Timeframe::Quarter => "3M",
      Timeframe::Week => "1W",
      Timeframe::Year => "1Y",
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContractLoad {
  Character(i64),
  Corporation(i64),
}

/// Cached filter/derived view data. The `*_indices` fields index into `journal`/`market`/`contracts`, so any mutation
/// of those vecs must be followed by `recompute_derived()` or the indices go stale (out-of-bounds panic or wrong rows).
#[derive(Debug, Default)]
struct Derived {
  category_flows: Vec<CategoryFlow>,
  contract_indices: Vec<usize>,
  journal_flow: JournalFlow,
  journal_indices: Vec<usize>,
  market_indices: Vec<usize>,
  recent_activity_indices: Vec<usize>,
}

pub fn load(db: &Database) -> Task<Message> {
  Task::perform(load_wallet(db.clone(), Scope::All, DEFAULT_DIVISION), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

fn reload(db: &Database, scope: Scope, division: i64) -> Task<Message> {
  Task::perform(load_wallet(db.clone(), scope, division), |loaded| {
    Message::Loaded(Box::new(loaded))
  })
}

fn load_budget(
  db: &Database,
  scope: Scope,
  budget_scope: crate::store::model::BudgetScope,
  month: String,
) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      let view = budget::load(&db, budget_scope, &month).await;
      let history = crate::features::budget::monthly_history(&db, budget_scope, &month, HISTORY_MONTHS).await;
      (view, history)
    },
    move |(view, history)| {
      Message::BudgetLoaded(Box::new(BudgetLoad {
        history,
        scope,
        select: None,
        view,
      }))
    },
  )
}

fn reload_budget(state: &State, db: &Database) -> Task<Message> {
  load_budget(db, state.active, state.budget_scope(), state.budget_month.clone())
}

fn reload_budget_chips(state: &State, db: &Database) -> Task<Message> {
  let scope = state.budget_scope();
  let db = db.clone();
  Task::perform(async move { loaders::load_budget_chips(&db, scope).await }, |c| {
    Message::BudgetChipsReloaded(Box::new(c))
  })
}

fn reload_kind(kind: JobKind) -> bool {
  matches!(
    kind,
    JobKind::CharacterWallet | JobKind::CorporationWallet | JobKind::MarketPrices | JobKind::NetWorthSnapshot
  )
}

fn load_more(state: &mut State, db: &Database) -> Task<Message> {
  if state.loading_more || state.tab_exhausted {
    return Task::none();
  }

  let scope = state.active;
  let tab = state.tab;

  if matches!(tab, Tab::Budget | Tab::Wallets) {
    return Task::none();
  }

  if let Scope::Corporation(corp_id) = scope {
    if tab != Tab::Contracts {
      return Task::none();
    }
    let cursor = state
      .contracts
      .last()
      .map(|entry| (entry.date_issued.clone(), entry.contract_id));
    let db = db.clone();
    let limit = PAGE_SIZE as i64;
    state.loading_more = true;
    return Task::perform(
      async move { loaders::load_corp_contracts_page(&db, corp_id, cursor, limit).await },
      move |contracts| more_page(scope, tab, MorePage::contracts(contracts)),
    );
  }

  let scope_ids = state.scope_ids();
  let corp_scope_ids = state.corp_scope_ids();
  if scope_ids.is_empty() && corp_scope_ids.is_empty() {
    return Task::none();
  }

  let db = db.clone();
  let limit = PAGE_SIZE as i64;
  state.loading_more = true;
  match tab {
    Tab::Journal => {
      let cursor = state.journal.last().map(|entry| entry.id);
      Task::perform(
        async move { loaders::load_journal_page(&db, &scope_ids, &corp_scope_ids, cursor, limit).await },
        move |journal| more_page(scope, tab, MorePage::journal(journal)),
      )
    }
    Tab::Market => {
      let cursor = state.market.last().map(|entry| entry.transaction_id);
      Task::perform(
        async move { loaders::load_market_page(&db, &scope_ids, &corp_scope_ids, cursor, limit).await },
        move |market| more_page(scope, tab, MorePage::market(market)),
      )
    }
    Tab::Contracts => {
      let cursor = state
        .contracts
        .last()
        .map(|entry| (entry.date_issued.clone(), entry.contract_id));
      Task::perform(
        async move { loaders::load_contracts_page(&db, &scope_ids, cursor, limit).await },
        move |contracts| more_page(scope, tab, MorePage::contracts(contracts)),
      )
    }
    // The Budget and Wallets tabs paginate nothing; their balance/envelope data
    // is already fully in State, so there is no cursor to advance.
    Tab::Budget | Tab::Wallets => Task::none(),
  }
}

fn more_page(scope: Scope, tab: Tab, mut page: MorePage) -> Message {
  page.scope = scope;
  page.tab = tab;
  Message::MoreLoaded(Box::new(page))
}

fn budget_begin_assign(state: &mut State, category_id: i64) -> Task<Message> {
  if state.budget_is_past() {
    return Task::none();
  }
  let draft = state
    .budget
    .as_ref()
    .and_then(|view| view.category(category_id))
    .filter(|category| category.assigned != 0.0)
    .map(|category| crate::ui::format::fmt_isk(category.assigned))
    .unwrap_or_default();
  state.budget_editing = Some(budget::EditingCell {
    category_id,
    draft,
  });
  Task::none()
}

fn budget_commit_assign(state: &mut State, db: &Database) -> Task<Message> {
  let Some(editing) = state.budget_editing.take() else {
    return Task::none();
  };
  let value = crate::ui::format::parse_isk(&editing.draft);
  let category_id = editing.category_id;
  budget_persist_then_reload(state, db, move |db, _scope, month| {
    Box::pin(async move { budget::persist_assignment(&db, category_id, &month, value).await })
  })
}

fn budget_quick_assign(state: &mut State, db: &Database, category_id: i64, value: f64) -> Task<Message> {
  state.budget_editing = None;
  budget_persist_then_reload(state, db, move |db, _scope, month| {
    Box::pin(async move { budget::persist_assignment(&db, category_id, &month, value).await })
  })
}

/// Opens the Move Money popover anchored on `category_id`, selecting it and
/// prefilling the amount with its `max(0, available)`. Blocked for past months,
/// like inline assign. A no-op when the category is not in the current view.
fn budget_open_move(state: &mut State, category_id: i64, anchor: BudgetMoveAnchor) -> Task<Message> {
  if state.budget_is_past() {
    return Task::none();
  }
  let Some(available) = state
    .budget
    .as_ref()
    .and_then(|view| view.category(category_id))
    .map(budget::Category::available)
  else {
    return Task::none();
  };
  state.budget_selected = Some(category_id);
  state.budget_editor = None;
  state.budget_move = Some(BudgetMove {
    amount_draft: crate::ui::format::fmt_isk(available.max(0.0)),
    anchor,
    from_id: category_id,
  });
  Task::none()
}

/// Commits the open Move Money transfer to `to`, then reloads so RTA, carry and
/// available re-derive. Amounts that do not parse to a positive whole ISK leave
/// the popover open (the destination list is inert until the amount is valid),
/// so a non-positive amount is a guarded no-op rather than a silent close.
fn budget_commit_move(state: &mut State, db: &Database, to: budget::MoveDest) -> Task<Message> {
  let Some((from_id, amount)) = state
    .budget_move
    .as_ref()
    .map(|m| (m.from_id, crate::ui::format::parse_isk(&m.amount_draft)))
  else {
    return Task::none();
  };
  if amount.round() <= 0.0 {
    return Task::none();
  }
  let Some(view) = state.budget.clone() else {
    return Task::none();
  };
  state.budget_move = None;
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    let view = view.clone();
    Box::pin(async move { budget::move_money(&db, &view, from_id, to, amount).await })
  })
}

/// Opens the rule editor seeded from an existing rule by id. A no-op when the
/// rule is no longer in the loaded set. Shared entry point reused by D's global
/// rules manager.
fn budget_open_rule_editor(state: &mut State, rule_id: i64) -> Task<Message> {
  if let Some(rule) = state.budget_rules().iter().find(|rule| rule.id() == rule_id) {
    state.budget_rule_editor = Some(budget::RuleDraft::from_rule(rule));
  }
  Task::none()
}

fn budget_open_global_rules(state: &mut State) -> Task<Message> {
  state.budget_global_rules_open = true;
  state.budget_rule_dragging = None;
  state.budget_rule_drop_target = None;
  Task::none()
}

/// Commits a drag-reorder of the global rule list: removes the dragged rule and
/// re-inserts it ahead of the drop-target rule, persists the new priority via A's
/// reorder repo, then re-derives so the winning rule for contested entries updates
/// immediately. A drop onto the dragged rule itself (or with nothing dragging) is a
/// no-op.
fn budget_rule_drop_released(state: &mut State, db: &Database) -> Task<Message> {
  let drop = state
    .budget_rule_dragging
    .take()
    .zip(state.budget_rule_drop_target.take());
  let Some((dragged, before)) = drop else {
    return Task::none();
  };
  if dragged == before {
    return Task::none();
  }
  let mut ordered: Vec<i64> = state
    .budget_rules()
    .iter()
    .map(|rule| rule.id())
    .filter(|id| *id != dragged)
    .collect();
  let position = ordered.iter().position(|id| *id == before).unwrap_or(ordered.len());
  ordered.insert(position, dragged);
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    Box::pin(async move {
      let _ = crate::store::repo::budget::reorder_rules(&db, &ordered).await;
    })
  })
}

fn budget_mutate_condition(
  state: &mut State,
  index: usize,
  mutate: impl FnOnce(&mut crate::store::model::RuleCondition),
) {
  if let Some(condition) = state
    .budget_rule_editor
    .as_mut()
    .and_then(|draft| draft.conditions.get_mut(index))
  {
    mutate(condition);
  }
}

fn budget_close_rule_select(state: &mut State) {
  if let Some(draft) = state.budget_rule_editor.as_mut() {
    draft.open_select = None;
  }
}

/// Threads the simple search box into the draft's first text-contains condition,
/// inserting one at the front when the rule has none yet.
fn budget_set_rule_search(state: &mut State, value: String) {
  let Some(draft) = state.budget_rule_editor.as_mut() else {
    return;
  };
  match draft.search_index() {
    Some(index) => draft.conditions[index].value = value,
    None => draft.conditions.insert(
      0,
      crate::store::model::RuleCondition {
        field: RuleField::Text,
        op: RuleOp::Contains,
        value,
        value2: None,
      },
    ),
  }
}

/// Persists the open rule draft (create or update) then re-derives so matching
/// outflows re-file into the category and the per-row chips refresh. A draft with
/// no active condition is a guarded no-op (the Save button is also disabled).
fn budget_commit_rule(state: &mut State, db: &Database) -> Task<Message> {
  let Some(draft) = state.budget_rule_editor.take() else {
    return Task::none();
  };
  let active: Vec<crate::store::model::RuleCondition> = draft
    .conditions
    .iter()
    .filter(|c| crate::features::budget::is_active_condition(c))
    .cloned()
    .collect();
  if active.is_empty() {
    return Task::none();
  }
  let name = budget_effective_rule_name(state, &draft);
  let position = i64::try_from(state.budget_rules().len()).unwrap_or(i64::MAX);
  let category_id = draft.category_id;
  let enabled = draft.enabled;
  let match_mode = draft.match_mode;
  let rule_id = draft.rule_id;

  budget_persist_then_reload(state, db, move |db, budget_scope, _month| {
    Box::pin(async move {
      persist_rule_draft(
        &db,
        budget_scope,
        rule_id,
        category_id,
        enabled,
        match_mode,
        name,
        position,
        active,
      )
      .await;
    })
  })
}

/// Writes a rule draft to A's repo: updates the existing row (or creates one) then
/// replaces its full condition set. Shared by the commit handler and its test so
/// the persisted shape is exercised directly.
#[allow(clippy::too_many_arguments)]
async fn persist_rule_draft(
  db: &Database,
  scope: crate::store::model::BudgetScope,
  rule_id: Option<i64>,
  category_id: i64,
  enabled: bool,
  match_mode: MatchMode,
  name: String,
  position: i64,
  conditions: Vec<crate::store::model::RuleCondition>,
) {
  let rule_id = match rule_id {
    Some(id) => {
      let _ = crate::store::repo::budget::update_rule(
        db,
        &crate::store::model::Rule {
          category_id,
          conditions: Vec::new(),
          enabled,
          id,
          match_mode,
          name,
        },
      )
      .await;
      Some(id)
    }
    None => crate::store::repo::budget::create_rule(
      db,
      &crate::store::repo::budget::NewRule {
        category_id,
        enabled,
        match_mode,
        name,
        position,
        scope,
      },
    )
    .await
    .ok()
    .map(|rule| rule.id()),
  };
  if let Some(rule_id) = rule_id {
    let _ = crate::store::repo::budget::replace_rule_conditions(db, rule_id, &conditions).await;
  }
}

/// The name a draft would persist: the user's name when set, else B's auto-suggestion,
/// else a fallback. Resolves Type/Character labels from the journal humanizer and the
/// roster.
fn budget_effective_rule_name(state: &State, draft: &budget::RuleDraft) -> String {
  if draft.name_edited && !draft.name.trim().is_empty() {
    return draft.name.clone();
  }
  let rule = crate::store::model::Rule {
    category_id: draft.category_id,
    conditions: draft.conditions.clone(),
    enabled: draft.enabled,
    id: draft.rule_id.unwrap_or(0),
    match_mode: draft.match_mode,
    name: draft.name.clone(),
  };
  let suggested = crate::features::budget::suggest_name(
    &rule,
    |token| Some(crate::features::budget::humanize_ref_type(token)),
    |key| budget_character_name(state, key),
  );
  if suggested.trim().is_empty() {
    "Untitled rule".to_owned()
  } else {
    suggested
  }
}

fn budget_character_name(state: &State, key: &str) -> Option<String> {
  let id = key.trim().parse::<i64>().ok()?;
  state
    .roster
    .iter()
    .find(|pilot| pilot.id == id)
    .map(|pilot| pilot.name.clone())
    .or_else(|| {
      state
        .corporations
        .iter()
        .find(|corp| corp.id == id)
        .map(|corp| corp.name.clone())
    })
}

/// Toggles a rule's enabled flag then re-derives so its effect appears/disappears
/// across activity and chips.
fn budget_toggle_rule(state: &mut State, db: &Database, rule_id: i64, enabled: bool) -> Task<Message> {
  let Some(mut rule) = state.budget_rules().iter().find(|rule| rule.id() == rule_id).cloned() else {
    return Task::none();
  };
  rule.enabled = enabled;
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    Box::pin(async move {
      let _ = crate::store::repo::budget::update_rule(&db, &rule).await;
    })
  })
}

/// Deletes a rule (its conditions cascade) then re-derives. Closes the editor if it
/// was editing the deleted rule.
fn budget_delete_rule(state: &mut State, db: &Database, rule_id: i64) -> Task<Message> {
  if state
    .budget_rule_editor
    .as_ref()
    .is_some_and(|draft| draft.rule_id == Some(rule_id))
  {
    state.budget_rule_editor = None;
  }
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    Box::pin(async move {
      let _ = crate::store::repo::budget::delete_rule(&db, rule_id).await;
    })
  })
}

fn budget_auto_assign(state: &mut State, db: &Database) -> Task<Message> {
  let Some(view) = state.budget.clone() else {
    return Task::none();
  };
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    let view = view.clone();
    Box::pin(async move { budget::auto_assign(&db, &view).await })
  })
}

fn budget_cover_overspending(state: &mut State, db: &Database) -> Task<Message> {
  let Some(view) = state.budget.clone() else {
    return Task::none();
  };
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    let view = view.clone();
    Box::pin(async move { budget::cover_overspending(&db, &view).await })
  })
}

fn budget_toggle_editor(state: &mut State) -> Task<Message> {
  if state.budget_editor.is_some() {
    state.budget_editor = None;
    return Task::none();
  }
  let Some(selected) = state.budget_selected else {
    return Task::none();
  };
  budget_seed_editor(state, selected);
  Task::none()
}

/// Loads `category_id`'s editor draft from the current view so the inspector can
/// edit its metadata and target. A no-op when the category is not in the view.
fn budget_seed_editor(state: &mut State, category_id: i64) {
  let draft = state.budget.as_ref().and_then(|view| {
    view.groups.iter().find_map(|group| {
      group
        .categories
        .iter()
        .enumerate()
        .find(|(_, category)| category.id == category_id)
        .map(|(position, category)| budget::CategoryDraft::from_category(group.id, position as i64, category))
    })
  });
  if draft.is_some() {
    state.budget_editor = draft;
  }
}

fn budget_commit_editor(state: &mut State, db: &Database) -> Task<Message> {
  let Some(draft) = state.budget_editor.take() else {
    return Task::none();
  };
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    let draft = draft.clone();
    Box::pin(async move {
      let now = chrono::Utc::now().to_rfc3339();
      let row = draft.to_category_row(now.clone(), now);
      budget::persist_category_edit(&db, &row, &draft.to_target()).await;
    })
  })
}

/// The end position (count of existing categories) of `group_id` in the current
/// view, so a freshly-added category appends after the rest. Zero when the group
/// or view is absent.
fn budget_group_end_position(state: &State, group_id: i64) -> i64 {
  let Some(view) = state.budget.as_ref() else {
    return 0;
  };
  match view.groups.iter().find(|group| group.id == group_id) {
    Some(group) => group.categories.len() as i64,
    None => 0,
  }
}

fn budget_add_category(state: &State, db: &Database, group_id: i64) -> Task<Message> {
  let position = budget_group_end_position(state, group_id);
  let scope = state.active;
  let budget_scope = state.budget_scope();
  let month = state.budget_month.clone();
  let db = db.clone();
  Task::perform(
    async move {
      let new_id = budget::add_category(&db, group_id, position).await;
      let view = budget::load(&db, budget_scope, &month).await;
      let history = crate::features::budget::monthly_history(&db, budget_scope, &month, HISTORY_MONTHS).await;
      (new_id, view, history)
    },
    move |(new_id, view, history)| {
      Message::BudgetLoaded(Box::new(BudgetLoad {
        history,
        scope,
        select: new_id,
        view,
      }))
    },
  )
}

fn budget_delete_category(state: &mut State, db: &Database, category_id: i64) -> Task<Message> {
  if state.budget_selected == Some(category_id) {
    state.budget_selected = None;
    state.budget_editor = None;
  }
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    Box::pin(async move { budget::delete_category(&db, category_id).await })
  })
}

fn budget_add_group(state: &State, db: &Database) -> Task<Message> {
  let position = state.budget.as_ref().map_or(0, |view| view.groups.len() as i64);
  budget_persist_then_reload(state, db, move |db, scope, _month| {
    Box::pin(async move {
      budget::add_group(&db, scope, position).await;
    })
  })
}

fn budget_request_group_delete(state: &mut State, db: &Database, group_id: i64) -> Task<Message> {
  let empty = state
    .budget
    .as_ref()
    .and_then(|view| view.groups.iter().find(|group| group.id == group_id))
    .is_some_and(|group| group.categories.is_empty());
  // Empty groups delete immediately; a populated group cascades its categories,
  // so the first click arms a confirmation and the second click commits it.
  if !empty && state.budget_pending_group_delete != Some(group_id) {
    state.budget_pending_group_delete = Some(group_id);
    return Task::none();
  }
  budget_commit_group_delete(state, db, group_id)
}

fn budget_commit_group_delete(state: &mut State, db: &Database, group_id: i64) -> Task<Message> {
  state.budget_pending_group_delete = None;
  let clear_selection = state
    .budget
    .as_ref()
    .and_then(|view| view.groups.iter().find(|group| group.id == group_id))
    .zip(state.budget_selected)
    .is_some_and(|(group, selected)| group.categories.iter().any(|category| category.id == selected));
  if clear_selection {
    state.budget_selected = None;
    state.budget_editor = None;
  }
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    Box::pin(async move { budget::delete_group(&db, group_id).await })
  })
}

fn budget_rename_group(state: &mut State, db: &Database, group_id: i64, name: String) -> Task<Message> {
  if let Some(group) = state
    .budget
    .as_mut()
    .and_then(|view| view.groups.iter_mut().find(|group| group.id == group_id))
  {
    group.name = name.clone();
  }
  let db = db.clone();
  Task::perform(async move { budget::rename_group(&db, group_id, &name).await }, |()| {
    Message::BudgetGroupRenameWritten
  })
}

fn budget_drop_released(state: &mut State, db: &Database) -> Task<Message> {
  // One release message serves both drags; a group drag in progress routes to the
  // group handler rather than the category-reorder path below.
  if state.budget_group_dragging.is_some() {
    return budget_group_drop_released(state, db);
  }
  let drop = state.budget_dragging.take().zip(state.budget_drop_target.take());
  let Some((dragged, target)) = drop else {
    return Task::none();
  };
  let (target_group, before) = match target {
    BudgetDropTarget::Category(category_id) => {
      let group_id = state
        .budget
        .as_ref()
        .and_then(|view| group_id_of_category(view, category_id));
      let Some(group_id) = group_id else {
        return Task::none();
      };
      (group_id, Some(category_id))
    }
    BudgetDropTarget::Group(group_id) => (group_id, None),
  };
  let Some(view) = state.budget.as_mut() else {
    return Task::none();
  };
  if !view.move_category(dragged, target_group, before) {
    return Task::none();
  }
  let reordered = view.clone();
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    let reordered = reordered.clone();
    Box::pin(async move { budget::persist_order(&db, &reordered).await })
  })
}

fn budget_group_drop_released(state: &mut State, db: &Database) -> Task<Message> {
  let drop = state
    .budget_group_dragging
    .take()
    .zip(state.budget_group_drop_target.take());
  let Some((dragged, target)) = drop else {
    return Task::none();
  };
  let Some(view) = state.budget.as_mut() else {
    return Task::none();
  };
  if !view.move_group(dragged, Some(target)) {
    return Task::none();
  }
  let reordered = view.clone();
  budget_persist_then_reload(state, db, move |db, _scope, _month| {
    let reordered = reordered.clone();
    Box::pin(async move { budget::persist_group_order(&db, &reordered).await })
  })
}

fn group_id_of_category(view: &budget::BudgetView, category_id: i64) -> Option<i64> {
  view
    .groups
    .iter()
    .find(|group| group.categories.iter().any(|category| category.id == category_id))
    .map(|group| group.id)
}

/// Runs a budget mutation, then reloads the derived view for the active scope and
/// month. The mutation closure receives an owned DB handle, the budget scope, and
/// the month key so it can persist whatever the message requires.
fn budget_persist_then_reload<F>(state: &State, db: &Database, mutate: F) -> Task<Message>
where
  F: FnOnce(
      Database,
      crate::store::model::BudgetScope,
      String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    + Send
    + 'static,
{
  let scope = state.active;
  let budget_scope = state.budget_scope();
  let month = state.budget_month.clone();
  let db = db.clone();
  Task::perform(
    async move {
      mutate(db.clone(), budget_scope, month.clone()).await;
      let view = budget::load(&db, budget_scope, &month).await;
      let history = crate::features::budget::monthly_history(&db, budget_scope, &month, HISTORY_MONTHS).await;
      (view, history)
    },
    move |(view, history)| {
      Message::BudgetLoaded(Box::new(BudgetLoad {
        history,
        scope,
        select: None,
        view,
      }))
    },
  )
}

fn handle_filter(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::BudgetFilterCleared => state.budget_filter = None,
    Message::FiltersCleared => {
      state.budget_filter = None;
      state.search.clear();
      state.sign_filter = SignFilter::All;
      state.side_filter = Side::All;
    }
    Message::SearchChanged(query) => state.search = query,
    Message::SideFilterChanged(side) => state.side_filter = side,
    Message::SignFilterChanged(filter) => state.sign_filter = filter,
    _ => return Task::none(),
  }
  state.tab_scroll_offset = 0.0;
  state.recompute_derived();
  Task::none()
}

fn handle_rail(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::BudgetInspectorDragEnd => {
      state.budget_inspector.end();
      Task::done(Message::PaneSettled(
        BUDGET_INSPECTOR_PANE_KEY,
        state.budget_inspector.ratio(),
      ))
    }
    Message::BudgetInspectorDragged(x) => {
      state.budget_inspector.drag_to(x);
      Task::none()
    }
    Message::BudgetInspectorDragStart => {
      state.budget_inspector.start();
      Task::none()
    }
    Message::RailDragEnd => {
      state.right_rail.end();
      Task::done(Message::PaneSettled(RIGHT_RAIL_PANE_KEY, state.right_rail.ratio()))
    }
    Message::RailDragged(x) => {
      state.right_rail.drag_to(x);
      Task::none()
    }
    Message::RailDragStart => {
      state.right_rail.start();
      Task::none()
    }
    _ => Task::none(),
  }
}

fn handle_close_contract_detail(state: &mut State) -> Task<Message> {
  state.selected_contract = None;
  Task::none()
}

fn handle_contract_detail_loaded(state: &mut State, detail: Option<contract_detail::ContractDetail>) -> Task<Message> {
  state.selected_contract = detail;
  Task::none()
}

fn handle_contract_selected(state: &State, db: &Database, contract_id: i64) -> Task<Message> {
  match contract_loader_target(state, contract_id) {
    Some(ContractLoad::Character(character_id)) => {
      let db = db.clone();
      Task::perform(
        async move { contract_detail::load_for_character(&db, character_id, contract_id).await },
        |detail| Message::ContractDetailLoaded(Box::new(detail)),
      )
    }
    Some(ContractLoad::Corporation(corporation_id)) => {
      let db = db.clone();
      Task::perform(
        async move { contract_detail::load_for_corporation(&db, corporation_id, contract_id).await },
        |detail| Message::ContractDetailLoaded(Box::new(detail)),
      )
    }
    None => Task::none(),
  }
}

/// Handles the `Message::Budget*` family. Split out of [`update`] so the wallet
/// dispatcher does not absorb the budget surface's branching. Editor-field
/// setters route through [`mutate_editor`] to keep this a flat dispatch.
fn handle_budget(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::BudgetAssignCancelled => {
      state.budget_editing = None;
      Task::none()
    }
    Message::BudgetAssignCommitted => budget_commit_assign(state, db),
    Message::BudgetAssignDraftChanged(draft) => {
      if let Some(editing) = state.budget_editing.as_mut() {
        editing.draft = draft;
      }
      Task::none()
    }
    Message::BudgetAssignEditBegan(category_id) => budget_begin_assign(state, category_id),
    Message::BudgetAutoAssign => budget_auto_assign(state, db),
    Message::BudgetCategorySelected(id) => budget_select_category(state, id),
    Message::BudgetCoverOverspending => budget_cover_overspending(state, db),
    Message::BudgetGlobalRulesClosed => {
      state.budget_global_rules_open = false;
      state.budget_rule_dragging = None;
      state.budget_rule_drop_target = None;
      Task::none()
    }
    Message::BudgetGlobalRulesOpened => budget_open_global_rules(state),
    Message::BudgetGroupToggled(group_id) => budget_toggle_group(state, group_id),
    Message::BudgetInspectorTabSelected(tab) => {
      state.budget_inspector_tab = tab;
      Task::none()
    }
    // The Budget-tab view and the ledger picker's `budget_chips` are separate
    // sources of truth, so any structural edit (add/delete/rename/reorder a
    // category or group) that reloads the view must also refresh the chips —
    // otherwise the new category stays invisible to the picker until an
    // unrelated event (e.g. leaving and re-entering the tab) reloads them.
    Message::BudgetLoaded(load) => budget_apply_loaded(state, *load).chain(reload_budget_chips(state, db)),
    Message::BudgetModeSelected(mode) => {
      state.budget_mode = mode;
      Task::none()
    }
    Message::BudgetMonthStepped(delta) => {
      state.budget_month = budget::shift_month(&state.budget_month, delta);
      state.budget_editing = None;
      state.budget_move = None;
      reload_budget(state, db)
    }
    Message::BudgetMoveAmountChanged(draft) => {
      if let Some(open) = state.budget_move.as_mut() {
        open.amount_draft = draft;
      }
      Task::none()
    }
    Message::BudgetMoveClosed => {
      state.budget_move = None;
      Task::none()
    }
    Message::BudgetMoveCommitted(to) => budget_commit_move(state, db, to),
    Message::BudgetMoveOpened(category_id, anchor) => budget_open_move(state, category_id, anchor),
    Message::BudgetQuickAssign(category_id, value) => budget_quick_assign(state, db, category_id, value),
    Message::BudgetRangeSelected(range) => {
      state.budget_range = range;
      Task::none()
    }
    other => handle_budget_rule(state, other, db),
  }
}

/// The Automation tab + rule editor messages: opening/closing the editor, editing
/// its draft, and committing/toggling/deleting rules through A's repo. Split off
/// [`handle_budget`] to keep its complexity bounded; unmatched messages fall
/// through to [`handle_budget_chip`].
fn handle_budget_rule(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::BudgetRuleConditionAdded => budget_rule_condition_added(state),
    Message::BudgetRuleConditionFieldChanged(index, field) => budget_rule_condition_field_changed(state, index, field),
    Message::BudgetRuleConditionOpChanged(index, op) => budget_rule_condition_op_changed(state, index, op),
    Message::BudgetRuleConditionRemoved(index) => budget_rule_condition_removed(state, index),
    Message::BudgetRuleConditionValue2Changed(index, value) => {
      budget_mutate_condition(state, index, |condition| condition.value2 = Some(value));
      Task::none()
    }
    Message::BudgetRuleConditionValueChanged(index, value) => {
      budget_mutate_condition(state, index, |condition| condition.value = value);
      budget_close_rule_select(state);
      Task::none()
    }
    Message::BudgetRuleDeleted(rule_id) => budget_delete_rule(state, db, rule_id),
    Message::BudgetRuleEditOpened(rule_id) => budget_open_rule_editor(state, rule_id),
    Message::BudgetRuleEditorClosed => {
      state.budget_rule_editor = None;
      Task::none()
    }
    Message::BudgetRuleEditorCommitted => budget_commit_rule(state, db),
    Message::BudgetRuleEditorSearchChanged(value) => {
      budget_set_rule_search(state, value);
      Task::none()
    }
    Message::BudgetRuleNewOpened(category_id) => {
      state.budget_rule_editor = Some(budget::RuleDraft::new(category_id));
      Task::none()
    }
    Message::BudgetRuleToggled(rule_id, enabled) => budget_toggle_rule(state, db, rule_id, enabled),
    other => handle_budget_rule_editor(state, other, db),
  }
}

/// The rule-editor field/drag messages, split off [`handle_budget_rule`] so the
/// dispatcher's complexity stays bounded. Unmatched messages fall through to
/// [`handle_budget_chip`].
fn handle_budget_rule_editor(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::BudgetRuleDragStarted(rule_id) => {
      state.budget_rule_dragging = Some(rule_id);
      state.budget_rule_drop_target = None;
      Task::none()
    }
    Message::BudgetRuleDropReleased => budget_rule_drop_released(state, db),
    Message::BudgetRuleDropTargetEntered(rule_id) => {
      if state.budget_rule_dragging.is_some() {
        state.budget_rule_drop_target = Some(rule_id);
      }
      Task::none()
    }
    Message::BudgetRuleDropTargetLeft => {
      if state.budget_rule_dragging.is_none() {
        state.budget_rule_drop_target = None;
      }
      Task::none()
    }
    Message::BudgetRuleEditorAdvancedToggled => {
      if let Some(draft) = state.budget_rule_editor.as_mut() {
        draft.show_advanced = !draft.show_advanced;
      }
      Task::none()
    }
    Message::BudgetRuleEditorMatchModeSelected(mode) => {
      if let Some(draft) = state.budget_rule_editor.as_mut() {
        draft.match_mode = mode;
      }
      Task::none()
    }
    Message::BudgetRuleEditorNameChanged(name) => {
      if let Some(draft) = state.budget_rule_editor.as_mut() {
        draft.name = name;
        draft.name_edited = true;
      }
      Task::none()
    }
    Message::BudgetRuleSelectToggled(key) => {
      if let Some(draft) = state.budget_rule_editor.as_mut() {
        draft.open_select = if draft.open_select == key { None } else { key };
      }
      Task::none()
    }
    other => handle_budget_chip(state, other, db),
  }
}

/// Appends a default Party condition to the open rule draft. Split off
/// [`handle_budget_rule`].
fn budget_rule_condition_added(state: &mut State) -> Task<Message> {
  if let Some(draft) = state.budget_rule_editor.as_mut() {
    draft
      .conditions
      .push(crate::features::budget::new_condition(RuleField::Party));
  }
  Task::none()
}

/// Replaces a condition with a fresh default for the newly-chosen field, closing
/// the open select. Split off [`handle_budget_rule`].
fn budget_rule_condition_field_changed(state: &mut State, index: usize, field: RuleField) -> Task<Message> {
  budget_mutate_condition(state, index, |condition| {
    *condition = crate::features::budget::new_condition(field);
  });
  budget_close_rule_select(state);
  Task::none()
}

/// Sets a condition's operator, seeding the upper bound when switching to
/// Between, then closes the open select. Split off [`handle_budget_rule`].
fn budget_rule_condition_op_changed(state: &mut State, index: usize, op: RuleOp) -> Task<Message> {
  budget_mutate_condition(state, index, |condition| {
    condition.op = op;
    if op == RuleOp::Between && condition.value2.is_none() {
      condition.value2 = Some(String::new());
    }
  });
  budget_close_rule_select(state);
  Task::none()
}

/// Removes a condition, keeping at least one. Split off [`handle_budget_rule`].
fn budget_rule_condition_removed(state: &mut State, index: usize) -> Task<Message> {
  if let Some(draft) = state.budget_rule_editor.as_mut()
    && draft.conditions.len() > 1
    && index < draft.conditions.len()
  {
    draft.conditions.remove(index);
  }
  Task::none()
}

/// The per-entry chip messages (open/dismiss/assign the envelope picker and
/// apply a reloaded chip set), split off [`handle_budget`] to keep its
/// complexity bounded. Unmatched messages fall through to [`handle_budget_edit`].
fn handle_budget_chip(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::BudgetChipAssigned(choice) => budget_chip_assigned(state, db, choice),
    Message::BudgetChipDismissed => {
      state.budget_picker = None;
      Task::none()
    }
    Message::BudgetChipOpened(owner, kind, id) => {
      state.budget_picker = Some((owner, kind, id));
      if state.budget_chips.envelopes.is_empty() {
        return reload_budget_chips(state, db);
      }
      Task::none()
    }
    Message::BudgetChipsReloaded(chips) => {
      state.budget_chips = *chips;
      // An assignment can change which rows match an active category/uncategorized
      // filter, so refresh the derived indices.
      if state.budget_filter.is_some() {
        state.recompute_derived();
      }
      Task::none()
    }
    other => handle_budget_edit(state, other, db),
  }
}

/// Selects a category, clearing any open editor and — in edit mode — re-seeding
/// the inspector editor from the selection. Split off [`handle_budget`].
fn budget_select_category(state: &mut State, id: i64) -> Task<Message> {
  state.budget_selected = Some(id);
  state.budget_editor = None;
  state.budget_inspector_tab = budget::InspectorTab::Detail;
  // In edit mode the inspector shows the category editor, so seed its draft
  // from the freshly-selected category.
  if state.budget_edit_mode {
    budget_seed_editor(state, id);
  }
  Task::none()
}

/// Toggles a group's collapsed state. Split off [`handle_budget`].
fn budget_toggle_group(state: &mut State, group_id: i64) -> Task<Message> {
  if !state.budget_collapsed.remove(&group_id) {
    state.budget_collapsed.insert(group_id);
  }
  Task::none()
}

/// Applies a freshly-loaded budget view, ignoring loads whose scope no longer
/// matches the active one and re-seeding an open editor against the new
/// positions. Split off [`handle_budget`] to keep its complexity bounded.
fn budget_apply_loaded(state: &mut State, load: BudgetLoad) -> Task<Message> {
  let BudgetLoad {
    history,
    scope,
    select,
    view,
  } = load;
  if scope != state.active {
    return Task::none();
  }
  if let Some(id) = select {
    state.budget_selected = Some(id);
  } else if state.budget_selected.is_none() {
    state.budget_selected = view.first_category_id();
  }
  state.budget = Some(view);
  state.budget_history = history;
  // Re-seed an open editor against the reloaded positions so a later
  // metadata commit cannot revert an order change made while it was open.
  if state.budget_editor.is_some()
    && let Some(selected) = state.budget_selected
  {
    budget_seed_editor(state, selected);
  }
  Task::none()
}

/// Persists (or clears) the picked envelope for the open chip's entry, cascading
/// the assignment to a market trade's journal twin so both rows stay in sync.
/// Split off [`handle_budget`] to keep its complexity bounded.
fn budget_chip_assigned(state: &mut State, db: &Database, choice: Option<i64>) -> Task<Message> {
  let Some((owner, kind, entry_id)) = state.budget_picker.take() else {
    return Task::none();
  };
  let scope = state.budget_scope();
  // A market trade and its journal twin are one event: assigning either one
  // cascades to the other so both rows stay in sync and the trade is counted
  // against the chosen envelope exactly once. The twin shares the row's owner.
  let counterpart = budget_cascade_target(state, owner, kind, entry_id);
  let db = db.clone();
  Task::perform(
    async move {
      for (kind, entry_id) in std::iter::once((kind, entry_id)).chain(counterpart) {
        match choice {
          Some(category_id) => {
            let _ = crate::features::budget::assign_entry(&db, scope, owner, kind, entry_id, category_id).await;
          }
          None => {
            let _ = crate::store::repo::budget::delete_entry_assignment(&db, scope, owner, kind, entry_id).await;
          }
        }
      }
      loaders::load_budget_chips(&db, scope).await
    },
    |c| Message::BudgetChipsReloaded(Box::new(c)),
  )
}

/// The edit-mode messages (drag-drop reorder + group/category CRUD), split off
/// [`handle_budget`] so the dispatcher's complexity stays bounded. Unmatched
/// messages fall through to [`handle_budget_editor`].
fn handle_budget_edit(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::BudgetCategoryAdded(group_id) => budget_add_category(state, db, group_id),
    Message::BudgetCategoryDeleted(category_id) => budget_delete_category(state, db, category_id),
    Message::BudgetDragStarted(category_id) => {
      state.budget_dragging = Some(category_id);
      state.budget_drop_target = None;
      state.budget_group_dragging = None;
      state.budget_group_drop_target = None;
      Task::none()
    }
    Message::BudgetDropReleased => budget_drop_released(state, db),
    Message::BudgetDropTargetEntered(target) => {
      if state.budget_dragging.is_some() {
        state.budget_drop_target = Some(target);
      } else if let (Some(_), BudgetDropTarget::Group(group_id)) = (state.budget_group_dragging, target) {
        // A group drag reuses the group headers as drop targets; hovering one marks
        // it as the group the dragged group will land before.
        state.budget_group_drop_target = Some(group_id);
      }
      Task::none()
    }
    Message::BudgetDropTargetLeft => {
      // While a drag is active the last-entered target is preserved through the
      // release, so leaving a row never clears it (the next enter replaces it).
      // Only stale highlights outside a drag are cleared.
      if state.budget_dragging.is_none() && state.budget_group_dragging.is_none() {
        state.budget_drop_target = None;
        state.budget_group_drop_target = None;
      }
      Task::none()
    }
    Message::BudgetEditToggled => {
      state.budget_edit_mode = !state.budget_edit_mode;
      state.budget_dragging = None;
      state.budget_drop_target = None;
      state.budget_group_dragging = None;
      state.budget_group_drop_target = None;
      state.budget_pending_group_delete = None;
      Task::none()
    }
    Message::BudgetGroupAdded => budget_add_group(state, db),
    Message::BudgetGroupDeleteRequested(group_id) => budget_request_group_delete(state, db, group_id),
    Message::BudgetGroupDragStarted(group_id) => {
      state.budget_group_dragging = Some(group_id);
      state.budget_group_drop_target = None;
      state.budget_dragging = None;
      state.budget_drop_target = None;
      Task::none()
    }
    Message::BudgetGroupRenameWritten => Task::none(),
    Message::BudgetGroupRenamed(group_id, name) => budget_rename_group(state, db, group_id, name),
    other => handle_budget_editor(state, other, db),
  }
}

/// The inspector category/target editor messages, split off [`handle_budget`] so
/// neither dispatcher carries the whole budget message family.
fn handle_budget_editor(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::BudgetEditorAmountChanged(text) => mutate_editor(state, |editor| {
      editor.target_amount = crate::ui::format::parse_isk(&text);
      editor.target_amount_text = text;
    }),
    Message::BudgetEditorByDateChanged(text) => mutate_editor(state, |editor| editor.by_date = text),
    Message::BudgetEditorCommitted => budget_commit_editor(state, db),
    Message::BudgetEditorKindSelected(kind) => mutate_editor(state, |editor| editor.target_kind = kind),
    Message::BudgetEditorNameChanged(text) => mutate_editor(state, |editor| editor.name = text),
    Message::BudgetEditorNoteChanged(text) => mutate_editor(state, |editor| editor.note = text),
    Message::BudgetEditorToggled => budget_toggle_editor(state),
    Message::BudgetEditorToneSelected(tone) => mutate_editor(state, |editor| editor.tone = Some(tone)),
    _ => Task::none(),
  }
}

fn mutate_editor(state: &mut State, edit: impl FnOnce(&mut budget::CategoryDraft)) -> Task<Message> {
  if let Some(editor) = state.budget_editor.as_mut() {
    edit(editor);
  }
  Task::none()
}

/// Switches the active scope, resetting per-scope view and budget state and
/// firing a fresh wallet (and, on the Budget tab, budget) reload. A no-op when
/// the scope is unchanged. Split off [`update`] to keep its complexity bounded.
fn handle_scope_selected(state: &mut State, db: &Database, scope: Scope) -> Task<Message> {
  state.picker_open = false;
  if scope == state.active {
    return Task::none();
  }
  state.active = scope;
  state.active_division = DEFAULT_DIVISION;
  state.corp_divisions = Vec::new();
  state.tab_scroll_offset = 0.0;
  state.budget = None;
  state.budget_filter = None;
  state.budget_picker = None;
  state.budget_selected = None;
  state.budget_editing = None;
  state.budget_editor = None;
  state.journal_selection.clear();
  state.market_selection.clear();
  state.ledger_menu = None;
  let budget_task = if state.tab == Tab::Budget {
    reload_budget(state, db)
  } else {
    Task::none()
  };
  reload(db, scope, state.active_division).chain(budget_task)
}

/// The ledger multi-select messages (modifier tracking, row click/right-click,
/// the context menu, and the bulk "Assign to Budget" action), split off
/// [`update`] to keep its complexity bounded.
fn handle_ledger(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::LedgerModifiersChanged(modifiers) => {
      state.ledger_modifiers = modifiers;
      Task::none()
    }
    Message::LedgerCursorMoved(point) => {
      state.ledger_cursor = Some(point);
      Task::none()
    }
    Message::LedgerRowClicked(kind, owner, entry_id) => {
      let order = ledger_order(state, kind);
      let click =
        selection::ClickKind::from_modifiers(state.ledger_modifiers.command(), state.ledger_modifiers.shift());
      ledger_selection_mut(state, kind).apply((owner, entry_id), click, &order);
      Task::none()
    }
    Message::LedgerRowRightPressed(kind, owner, entry_id) => {
      // Right-clicking a row outside the current selection makes it the selection
      // so the menu always acts on what the user pointed at.
      let key = (owner, entry_id);
      if !ledger_selection(state, kind).contains(key) {
        let order = ledger_order(state, kind);
        ledger_selection_mut(state, kind).apply(key, selection::ClickKind::Plain, &order);
      }
      if let Some(anchor) = state.ledger_cursor {
        state.ledger_menu = Some(LedgerMenu {
          anchor,
          picking: false,
          tab: state.tab,
        });
      }
      Task::none()
    }
    Message::LedgerMenuDismissed => {
      state.ledger_menu = None;
      Task::none()
    }
    Message::LedgerBulkAssignOpened => {
      if let Some(menu) = state.ledger_menu.as_mut() {
        menu.picking = true;
      }
      // The picker reuses the envelope chip set; load it if it has not been yet.
      if state.budget_chips.envelopes.is_empty() {
        return reload_budget_chips(state, db);
      }
      Task::none()
    }
    Message::LedgerBulkAssignChosen(category_id) => budget_bulk_assign(state, db, category_id),
    _ => Task::none(),
  }
}

fn ledger_selection(state: &State, kind: BudgetEntryKind) -> &selection::RowSelection {
  match kind {
    BudgetEntryKind::Journal => &state.journal_selection,
    BudgetEntryKind::Market => &state.market_selection,
  }
}

fn ledger_selection_mut(state: &mut State, kind: BudgetEntryKind) -> &mut selection::RowSelection {
  match kind {
    BudgetEntryKind::Journal => &mut state.journal_selection,
    BudgetEntryKind::Market => &mut state.market_selection,
  }
}

/// Drops selected rows that a reload or filter change has removed from the live
/// ledgers so a stale selection never points at rows the user can no longer see.
fn prune_ledger_selections(state: &mut State) {
  let journal_order = ledger_order(state, BudgetEntryKind::Journal);
  state.journal_selection.prune(&journal_order);
  let market_order = ledger_order(state, BudgetEntryKind::Market);
  state.market_selection.prune(&market_order);
}

fn ledger_order(state: &State, kind: BudgetEntryKind) -> Vec<selection::RowKey> {
  match kind {
    BudgetEntryKind::Journal => filtered_journal(state)
      .iter()
      .map(|entry| (entry.owner, entry.id))
      .collect(),
    BudgetEntryKind::Market => filtered_market(state)
      .iter()
      .map(|entry| (entry.owner, entry.transaction_id))
      .collect(),
  }
}

/// Assigns every selected row in the menu's tab to `category_id` in one action,
/// looping the same per-row assign-plus-cascade path the chip uses so the result
/// is identical to assigning each row individually. Assignment is owner-keyed at
/// the single All budget, so it is correct under every ledger filter.
fn budget_bulk_assign(state: &mut State, db: &Database, category_id: i64) -> Task<Message> {
  let Some(menu) = state.ledger_menu.take() else {
    return Task::none();
  };
  let kind = match menu.tab {
    Tab::Journal => BudgetEntryKind::Journal,
    Tab::Market => BudgetEntryKind::Market,
    Tab::Budget | Tab::Contracts | Tab::Wallets => return Task::none(),
  };
  let scope = state.budget_scope();
  let order = ledger_order(state, kind);
  let selected = ledger_selection(state, kind).ordered(&order);

  // Expand each selected row to itself plus its market/journal cascade twin so a
  // bulk assign mirrors the per-row chip exactly.
  let mut targets: Vec<(BudgetOwner, BudgetEntryKind, i64)> = Vec::new();
  for (owner, entry_id) in selected {
    targets.push((owner, kind, entry_id));
    if let Some((twin_kind, twin_id)) = budget_cascade_target(state, owner, kind, entry_id) {
      targets.push((owner, twin_kind, twin_id));
    }
  }
  ledger_selection_mut(state, kind).clear();
  if targets.is_empty() {
    return Task::none();
  }

  let db = db.clone();
  Task::perform(
    async move {
      for (owner, kind, entry_id) in targets {
        let _ = crate::features::budget::assign_entry(&db, scope, owner, kind, entry_id, category_id).await;
      }
      loaders::load_budget_chips(&db, scope).await
    },
    |chips| Message::BudgetChipsReloaded(Box::new(chips)),
  )
}

/// Switches the active corporation wallet division, reloading rows for it. A
/// no-op outside a corporation scope or when the division is unchanged. Split
/// off [`update`].
fn handle_division_selected(state: &mut State, db: &Database, division: i64) -> Task<Message> {
  if !matches!(state.active, Scope::Corporation(_)) || division == state.active_division {
    return Task::none();
  }
  state.active_division = division;
  state.tab_scroll_offset = 0.0;
  reload(db, state.active, division)
}

/// Appends a freshly-paged set of rows, ignoring stale pages and marking the
/// tab exhausted when nothing new arrived. Split off [`update`].
fn handle_more_loaded(state: &mut State, page: MorePage) -> Task<Message> {
  state.loading_more = false;
  let MorePage {
    contracts,
    journal,
    market,
    tab,
    scope,
  } = page;
  if scope != state.active || tab != state.tab {
    return Task::none();
  }
  let appended = journal.len() + market.len() + contracts.len();
  state.journal.extend(journal);
  state.market.extend(market);
  state.contracts.extend(contracts);
  state.tab_exhausted = appended == 0;
  state.recompute_derived();
  prune_ledger_selections(state);
  Task::none()
}

/// Records the scroll offset and pages in more rows once the viewport nears the
/// end of the list. Split off [`update`].
fn handle_tab_scrolled(state: &mut State, db: &Database, absolute: f32, relative: f32) -> Task<Message> {
  state.tab_scroll_offset = absolute;
  if relative < SCROLL_LOAD_THRESHOLD {
    return Task::none();
  }
  load_more(state, db)
}

/// Switches the active content tab, resetting its scroll/paging state and
/// reloading the Budget surface when it is selected. Split off [`update`].
fn handle_tab_selected(state: &mut State, db: &Database, tab: Tab) -> Task<Message> {
  state.tab = tab;
  state.tab_scroll_offset = 0.0;
  state.tab_exhausted = false;
  state.ledger_menu = None;
  if tab == Tab::Budget {
    return reload_budget(state, db);
  }
  Task::none()
}

pub fn update(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  if message.is_budget() {
    return handle_budget(state, message, db);
  }
  match message {
    Message::ChartHovered(fraction) => {
      state.chart_hover = fraction;
      Task::none()
    }
    Message::CloseContractDetail => handle_close_contract_detail(state),
    Message::ContractDetailLoaded(detail) => handle_contract_detail_loaded(state, *detail),
    Message::ContractSelected(contract_id) => handle_contract_selected(state, db, contract_id),
    Message::DivisionSelected(division) => handle_division_selected(state, db, division),
    msg @ (Message::LedgerBulkAssignChosen(_)
    | Message::LedgerBulkAssignOpened
    | Message::LedgerCursorMoved(_)
    | Message::LedgerMenuDismissed
    | Message::LedgerModifiersChanged(_)
    | Message::LedgerRowClicked(..)
    | Message::LedgerRowRightPressed(..)) => handle_ledger(state, msg, db),
    Message::Loaded(loaded) => {
      let Loaded {
        chips,
        contract_total,
        contracts,
        corp_divisions,
        corporations,
        financials,
        journal,
        journal_total,
        market,
        market_total,
        net_worth_series,
        periods,
        right_rail_width,
        roster,
        wallet_sections,
      } = *loaded;
      state.budget_chips = chips;
      state.contract_total = contract_total;
      state.contracts = contracts;
      state.corp_divisions = corp_divisions;
      state.corporations = corporations;
      state.financials = financials;
      state.journal = journal;
      state.journal_total = journal_total;
      state.market = market;
      state.market_total = market_total;
      state.net_worth_series = net_worth_series;
      state.periods = periods;
      if !state.right_rail.is_active() {
        state.right_rail.set_ratio_from_store(right_rail_width);
      }
      state.roster = roster;
      state.wallet_sections = wallet_sections;
      state.loading_more = false;
      state.tab_exhausted = false;
      state.recompute_derived();
      prune_ledger_selections(state);
      Task::none()
    }
    Message::MoreLoaded(page) => handle_more_loaded(state, *page),
    Message::PaneSettled(..) => Task::none(),
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    msg @ (Message::BudgetInspectorDragEnd
    | Message::BudgetInspectorDragged(_)
    | Message::BudgetInspectorDragStart
    | Message::RailDragEnd
    | Message::RailDragged(_)
    | Message::RailDragStart) => handle_rail(state, msg),
    Message::ReauthRequested(_) => Task::none(),
    Message::ScopeSelected(scope) => handle_scope_selected(state, db, scope),
    msg @ (Message::BudgetFilterCleared
    | Message::FiltersCleared
    | Message::SearchChanged(_)
    | Message::SideFilterChanged(_)
    | Message::SignFilterChanged(_)) => handle_filter(state, msg),
    Message::BudgetCategoryHovered(category_id) => {
      state.budget_hovered_category = category_id;
      Task::none()
    }
    Message::BudgetFilterApplied(kind) => {
      state.budget_filter = Some(BudgetFilter {
        kind,
        month: state.budget_month.clone(),
      });
      state.tab = Tab::Journal;
      state.tab_scroll_offset = 0.0;
      state.recompute_derived();
      Task::none()
    }
    Message::TabScrolled {
      absolute,
      relative,
    } => handle_tab_scrolled(state, db, absolute, relative),
    Message::TabSelected(tab) => handle_tab_selected(state, db, tab),
    Message::TimeframeSelected(timeframe) => {
      state.timeframe = timeframe;
      state.chart_hover = None;
      Task::none()
    }
    Message::WalletsSortSelected(sort) => {
      state.wallets_sort = sort;
      Task::none()
    }
    // The Budget surface is dispatched by `handle_budget` via the `is_budget`
    // guard above; this arm only keeps the match exhaustive.
    _ => Task::none(),
  }
}

pub fn view(state: &State, now: DateTime<Utc>) -> Element<'_, Message> {
  let base = shell::shell(state, now);
  match state.selected_contract() {
    Some(detail) => contract_detail::overlay(base, detail, Message::CloseContractDetail),
    None => base,
  }
}

/// True when `event` is an Escape key press, used by the modal-dismiss
/// subscriptions to map the key to their cancel message.
fn is_escape_pressed(event: &iced::Event) -> bool {
  matches!(
    event,
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
      key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
      ..
    })
  )
}

pub fn subscription(state: &State) -> iced::Subscription<Message> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.right_rail.is_active() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      crate::ui::components::resizable_pane::drag_event(event, Message::RailDragged, Message::RailDragEnd)
    }));
  }
  if state.budget_inspector.is_active() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      crate::ui::components::resizable_pane::drag_event(
        event,
        Message::BudgetInspectorDragged,
        Message::BudgetInspectorDragEnd,
      )
    }));
  }
  subs.extend(escape_dismiss_subs(state));
  subs.extend(drag_release_subs(state));
  if matches!(state.tab, Tab::Journal | Tab::Market) {
    subs.push(iced::event::listen_with(|event, _status, _id| match event {
      iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) => {
        Some(Message::LedgerModifiersChanged(modifiers))
      }
      _ => None,
    }));
  }
  iced::Subscription::batch(subs)
}

/// The Escape-to-dismiss subscriptions whose presence depends on which modal or
/// overlay surface is open. Split off [`subscription`] to keep its complexity
/// bounded.
fn escape_dismiss_subs(state: &State) -> Vec<iced::Subscription<Message>> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.selected_contract.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::CloseContractDetail)
    }));
  }
  if state.budget_editing.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::BudgetAssignCancelled)
    }));
  }
  if state.budget_global_rules_open && state.budget_rule_editor.is_none() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::BudgetGlobalRulesClosed)
    }));
  }
  if state.ledger_menu.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_escape_pressed(&event).then_some(Message::LedgerMenuDismissed)
    }));
  }
  subs
}

/// True when `event` is a left mouse-button release, used by the budget
/// drag-and-drop surfaces to settle a drop.
fn is_left_released(event: &iced::Event) -> bool {
  matches!(
    event,
    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
  )
}

/// The left-release subscriptions that settle an in-progress budget category or
/// rule drag. Split off [`subscription`] to keep its complexity bounded.
fn drag_release_subs(state: &State) -> Vec<iced::Subscription<Message>> {
  let mut subs: Vec<iced::Subscription<Message>> = Vec::new();
  if state.budget_dragging.is_some() || state.budget_group_dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_left_released(&event).then_some(Message::BudgetDropReleased)
    }));
  }
  if state.budget_rule_dragging.is_some() {
    subs.push(iced::event::listen_with(|event, _status, _id| {
      is_left_released(&event).then_some(Message::BudgetRuleDropReleased)
    }));
  }
  subs
}

async fn load_wallet(db: Database, scope: Scope, division: i64) -> Loaded {
  let roster = load_roster(&db).await;
  let corporations = load_corporations(&db).await;
  let wallet_sections = load_wallet_sections(&db).await;
  let financials = finance::financials_all(&db).await.unwrap_or_default();
  let periods = finance::wallet_period_summaries_all(&db).await.unwrap_or_default();

  let scope_ids = resolve_scope_ids(scope, &roster, &corporations);

  let (journal, market, contracts, corp_divisions, journal_total, market_total, contract_total) = match scope {
    Scope::Corporation(corp_id) => {
      let limit = PAGE_SIZE as i64;
      let journal = loaders::load_corp_journal(&db, corp_id, division).await;
      let market = loaders::load_corp_market(&db, corp_id, division).await;
      let contracts = loaders::load_corp_contracts_page(&db, corp_id, None, limit).await;
      let corp_divisions = load_corp_divisions(&db, corp_id).await;
      let journal_total = finance::count_journal_for_corporation(&db, corp_id, division)
        .await
        .unwrap_or(0);
      let market_total = finance::count_transactions_for_corporation(&db, corp_id, division)
        .await
        .unwrap_or(0);
      let contract_total = finance::count_contracts_for_corporation(&db, corp_id)
        .await
        .unwrap_or(0);
      (
        journal,
        market,
        contracts,
        corp_divisions,
        journal_total,
        market_total,
        contract_total,
      )
    }
    Scope::All | Scope::Character(_) => {
      let limit = PAGE_SIZE as i64;
      let corp_scope_ids: Vec<i64> = match scope {
        Scope::All => corporations.iter().map(|corp| corp.id).collect(),
        _ => Vec::new(),
      };
      let journal = loaders::load_journal_page(&db, &scope_ids, &corp_scope_ids, None, limit).await;
      let market = loaders::load_market_page(&db, &scope_ids, &corp_scope_ids, None, limit).await;
      let contracts = loaders::load_contracts_page(&db, &scope_ids, None, limit).await;
      let (mut journal_total, mut market_total, contract_total) = count_character_totals(&db, &scope_ids).await;
      for &corp_id in &corp_scope_ids {
        journal_total += finance::count_journal_for_corporation_all_divisions(&db, corp_id)
          .await
          .unwrap_or(0);
        market_total += finance::count_transactions_for_corporation_all_divisions(&db, corp_id)
          .await
          .unwrap_or(0);
      }
      (
        journal,
        market,
        contracts,
        Vec::new(),
        journal_total,
        market_total,
        contract_total,
      )
    }
  };

  let net_worth_series = load_net_worth_series(&db, scope, &scope_ids, &corporations).await;

  let right_rail_width = window_state::load()
    .panes
    .get(RIGHT_RAIL_PANE_KEY)
    .copied()
    .unwrap_or(RIGHT_RAIL_DEFAULT_WIDTH);

  // The budget is always the single All budget; the wallet scope only filters the
  // ledger rows, never which budget the chips/picker resolve against.
  let chips = loaders::load_budget_chips(&db, crate::store::model::BudgetScope::All).await;

  Loaded {
    chips,
    contract_total,
    contracts,
    corp_divisions,
    corporations,
    financials,
    journal,
    journal_total,
    market,
    market_total,
    net_worth_series,
    periods,
    right_rail_width,
    roster,
    wallet_sections,
  }
}

async fn count_character_totals(db: &Database, scope_ids: &[i64]) -> (i64, i64, i64) {
  let mut journal_total = 0;
  let mut market_total = 0;
  let mut contract_total = 0;
  for &id in scope_ids {
    journal_total += finance::count_journal_for_character(db, id).await.unwrap_or(0);
    market_total += finance::count_transactions_for_character(db, id).await.unwrap_or(0);
    contract_total += finance::count_contracts_for_character(db, id).await.unwrap_or(0);
  }
  (journal_total, market_total, contract_total)
}

async fn load_corp_divisions(db: &Database, corporation_id: i64) -> Vec<CorpDivision> {
  crate::store::repo::finance::divisions(db, corporation_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| CorpDivision {
      balance: row.balance(),
      division: row.division(),
      name: row.name().clone(),
    })
    .collect()
}

async fn load_net_worth_series(
  db: &Database,
  scope: Scope,
  scope_ids: &[i64],
  corporations: &[RosterCorp],
) -> Vec<NetWorthPoint> {
  let since = (Utc::now() - Duration::days(Timeframe::Year.days() as i64))
    .format("%Y-%m-%d")
    .to_string();

  match scope {
    Scope::Character(id) => finance::for_character_since(db, id, &since)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|row| NetWorthPoint {
        date: row.date().clone(),
        liquid: row.liquid(),
        net_worth: row.net_worth(),
      })
      .collect(),
    Scope::All => {
      let _ = scope_ids;
      let mut by_date: std::collections::BTreeMap<String, (f64, f64)> = std::collections::BTreeMap::new();
      for row in finance::combined_series_since(db, &since).await.unwrap_or_default() {
        if let Some(net_worth) = row.net_worth() {
          let entry = by_date.entry(row.date().clone()).or_insert((0.0, 0.0));
          entry.0 += net_worth;
          entry.1 += row.liquid().unwrap_or(0.0);
        }
      }
      for corp in corporations {
        for row in finance::for_corporation_since(db, corp.id, &since)
          .await
          .unwrap_or_default()
        {
          let entry = by_date.entry(row.date().clone()).or_insert((0.0, 0.0));
          entry.0 += row.net_worth();
          entry.1 += row.liquid();
        }
      }
      by_date
        .into_iter()
        .map(|(date, (net_worth, liquid))| NetWorthPoint {
          date,
          liquid,
          net_worth,
        })
        .collect()
    }
    Scope::Corporation(id) => finance::for_corporation_since(db, id, &since)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|row| NetWorthPoint {
        date: row.date().clone(),
        liquid: row.liquid(),
        net_worth: row.net_worth(),
      })
      .collect(),
  }
}

fn resolve_scope_ids(scope: Scope, roster: &[RosterPilot], _corporations: &[RosterCorp]) -> Vec<i64> {
  match scope {
    Scope::All => roster.iter().map(|pilot| pilot.id).collect(),
    Scope::Character(id) => vec![id],
    Scope::Corporation(_) => Vec::new(),
  }
}

async fn load_roster(db: &Database) -> Vec<RosterPilot> {
  let characters = character::all_owned(db).await.unwrap_or_default();
  let financials = finance::financials_all(db).await.unwrap_or_default();
  let liquid_by_id: std::collections::HashMap<i64, Option<f64>> =
    financials.iter().map(|row| (row.character_id, row.liquid)).collect();
  let credentials = infra::all(db).await.unwrap_or_default();
  let scopes_by_id: std::collections::HashMap<i64, Option<String>> = credentials
    .into_iter()
    .filter(|cred| cred.owner_type() == OwnerType::Character)
    .map(|cred| (cred.owner_id(), cred.scopes().clone()))
    .collect();

  let mut roster = Vec::with_capacity(characters.len());
  for character in &characters {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|c| c.ticker().to_owned())
      .unwrap_or_default();
    let portrait = images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      character.id(),
    );
    roster.push(RosterPilot {
      corp,
      granted_scopes: scopes_by_id.get(&character.id()).cloned().flatten(),
      id: character.id(),
      liquid: liquid_by_id.get(&character.id()).copied().flatten(),
      name: character.name().to_owned(),
      portrait,
    });
  }
  roster
}

async fn load_corporations(db: &Database) -> Vec<RosterCorp> {
  let corporations = org::all_owned_corporations(db).await.unwrap_or_default();
  let mut roster = Vec::with_capacity(corporations.len());
  for corp in corporations {
    let liquid = sum_option(
      finance::divisions(db, corp.id())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|division| division.balance()),
    );
    let logo = images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, corp.id());
    roster.push(RosterCorp {
      id: corp.id(),
      liquid,
      logo,
      name: corp.name().to_owned(),
      ticker: corp.ticker().to_owned(),
    });
  }
  roster
}

async fn load_wallet_sections(db: &Database) -> Vec<CorpWalletSection> {
  let corporations = org::all_owned_corporations(db).await.unwrap_or_default();
  let mut sections = Vec::with_capacity(corporations.len());
  for corp in corporations {
    let divisions = load_corp_divisions(db, corp.id()).await;
    let (granted_by, role) = attribution(db, &corp).await;
    let logo = images::resolve(&images::default_store(), images::ImageKind::CorporationLogo, corp.id());
    sections.push(CorpWalletSection {
      divisions,
      granted_by,
      id: corp.id(),
      logo,
      name: corp.name().to_owned(),
      role,
      ticker: corp.ticker().to_owned(),
    });
  }
  sections
}

/// Resolve the pilot and accounting role that grant read access to a corp's
/// wallet: the corp's authorizing character, paired with the strongest
/// wallet-read role they hold (Director, then Accountant, then Junior
/// Accountant — the same precedence the corp-wallet sync gates on).
async fn attribution(db: &Database, corp: &crate::store::model::OwnedCorporation) -> (Option<String>, Option<String>) {
  let Some(authorized_by) = corp.authorized_by() else {
    return (None, None);
  };
  let name = character::get(db, authorized_by)
    .await
    .ok()
    .flatten()
    .map(|character| character.name().to_owned());
  let roles = org::for_corporation(db, corp.id()).await.unwrap_or_default();
  let role = strongest_accounting_role(|wanted| {
    roles
      .iter()
      .any(|member| member.character_id() == authorized_by && member.role() == wanted)
  });
  (name, role)
}

/// The strongest wallet-read role (Director, then Accountant, then Junior
/// Accountant) for which `holds` reports true, humanized for display. Split off
/// [`attribution`] so the precedence is unit-testable without a database.
fn strongest_accounting_role(holds: impl Fn(&str) -> bool) -> Option<String> {
  ACCOUNTING_ROLES
    .iter()
    .find(|wanted| holds(wanted))
    .map(|role| humanize_role(role))
}

fn humanize_role(role: &str) -> String {
  role.replace('_', " ")
}

pub fn scope_liquid(state: &State) -> Option<f64> {
  match state.active {
    Scope::All => combined_liquid(state),
    Scope::Character(_) => {
      let ids: std::collections::HashSet<i64> = state.scope_ids().into_iter().collect();
      sum_option(
        state
          .financials
          .iter()
          .filter(|row| ids.contains(&row.character_id))
          .map(|row| row.liquid),
      )
    }
    Scope::Corporation(_) => state.corp_balance_total(),
  }
}

/// Liquid ISK across every owned character and corporation, independent of the active scope.
pub fn combined_liquid(state: &State) -> Option<f64> {
  let ids: std::collections::HashSet<i64> = state.roster.iter().map(|pilot| pilot.id).collect();
  let character_liquid = sum_option(
    state
      .financials
      .iter()
      .filter(|row| ids.contains(&row.character_id))
      .map(|row| row.liquid),
  );
  sum_option([character_liquid, state.owned_corp_liquid()].into_iter())
}

// Kept: tested aggregation over loaded `state.periods`, awaiting the period-totals UI wiring; deletion would orphan the loaded `periods` data.
#[allow(dead_code)]
pub fn period_totals(state: &State) -> PeriodTotals {
  let ids: std::collections::HashSet<i64> = state.scope_ids().into_iter().collect();
  let mut totals = PeriodTotals::default();
  for summary in state.periods.iter().filter(|row| ids.contains(&row.character_id)) {
    totals.income += summary.income;
    totals.spend += summary.spend;
  }
  totals.net = totals.income - totals.spend;
  totals
}

pub fn timeframe_window(timeframe: Timeframe, today: NaiveDate) -> (NaiveDate, NaiveDate) {
  let span = timeframe.days().saturating_sub(1) as i64;
  (today - Duration::days(span), today)
}

pub fn sliced_series(state: &State, today: NaiveDate) -> &[NetWorthPoint] {
  let (start, _) = timeframe_window(state.timeframe, today);
  let cutoff = start.format("%Y-%m-%d").to_string();
  let series = &state.net_worth_series;
  let start = series.partition_point(|point| point.date.as_str() < cutoff.as_str());
  &series[start..]
}

pub fn series_change(series: &[NetWorthPoint]) -> f64 {
  match (series.first(), series.last()) {
    (Some(first), Some(last)) if series.len() >= 2 => last.net_worth - first.net_worth,
    _ => 0.0,
  }
}

pub fn series_current(series: &[NetWorthPoint]) -> Option<f64> {
  series.last().map(|point| point.net_worth)
}

pub fn scope_composition(state: &State) -> Composition {
  let ids: std::collections::HashSet<i64> = state.scope_ids().into_iter().collect();
  let rows: Vec<&CharacterFinancials> = state
    .financials
    .iter()
    .filter(|row| ids.contains(&row.character_id))
    .collect();
  Composition {
    asset_value: sum_option(rows.iter().map(|row| row.asset_value)),
    escrow: sum_option(rows.iter().map(|row| row.escrow)),
    liquid: scope_liquid(state),
  }
}

pub fn composition_stack(state: &State) -> Vec<CompositionSlice> {
  if !matches!(state.active, Scope::All) {
    return Vec::new();
  }
  let financials_by_id: std::collections::HashMap<i64, &_> =
    state.financials.iter().map(|row| (row.character_id, row)).collect();
  let mut slices: Vec<CompositionSlice> = state
    .roster
    .iter()
    .filter_map(|pilot| {
      let net_worth = financials_by_id.get(&pilot.id).and_then(|row| row.net_worth)?;
      Some(CompositionSlice {
        id: pilot.id,
        name: pilot.name.clone(),
        net_worth,
      })
    })
    .collect();
  slices.sort_by(|a, b| b.net_worth.total_cmp(&a.net_worth));
  slices
}

fn sum_option(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
  let present: Vec<f64> = values.flatten().collect();
  if present.is_empty() {
    None
  } else {
    Some(present.iter().sum())
  }
}

pub fn filtered_journal(state: &State) -> Vec<&JournalEntry> {
  state
    .derived
    .journal_indices
    .iter()
    .map(|&index| &state.journal[index])
    .collect()
}

pub fn filtered_market(state: &State) -> Vec<&MarketEntry> {
  state
    .derived
    .market_indices
    .iter()
    .map(|&index| &state.market[index])
    .collect()
}

/// Resolves which detail loader a clicked contract row needs.
///
/// Corporation scope always loads from the corp tables. Under character or all
/// scope the row's own `character_id` is used, so an all-wallets list still
/// loads each contract from the character that owns it.
fn contract_loader_target(state: &State, contract_id: i64) -> Option<ContractLoad> {
  if let Scope::Corporation(corporation_id) = state.active {
    return Some(ContractLoad::Corporation(corporation_id));
  }
  state
    .contracts
    .iter()
    .find(|entry| entry.contract_id == contract_id)
    .map(|entry| ContractLoad::Character(entry.character_id))
}

pub fn filtered_contracts(state: &State) -> Vec<&ContractEntry> {
  state
    .derived
    .contract_indices
    .iter()
    .map(|&index| &state.contracts[index])
    .collect()
}

fn contract_matches(entry: &ContractEntry, side: Side, query: &str) -> bool {
  match side {
    Side::Buy if !entry.is_buy => return false,
    Side::Sell if entry.is_buy => return false,
    _ => {}
  }
  if query.is_empty() {
    return true;
  }
  entry.contract_id.to_string().contains(query)
    || entry.r#type.to_lowercase().contains(query)
    || entry.status.to_lowercase().contains(query)
    || entry
      .issuer
      .as_deref()
      .is_some_and(|name| name.to_lowercase().contains(query))
    || entry
      .assignee
      .as_deref()
      .is_some_and(|name| name.to_lowercase().contains(query))
    || entry
      .acceptor
      .as_deref()
      .is_some_and(|name| name.to_lowercase().contains(query))
}

fn journal_matches(entry: &JournalEntry, sign: SignFilter, query: &str) -> bool {
  match sign {
    SignFilter::In if !entry.is_income() => return false,
    SignFilter::Out if !entry.amount.is_some_and(|amount| amount < 0.0) => return false,
    _ => {}
  }
  if !query.is_empty() && !entry.match_text().to_lowercase().contains(query) {
    return false;
  }
  true
}

/// The matched counterpart of a budget assignment, so an assignment cascades
/// across a market trade's two records. A transaction links to its journal twin
/// via `journal_ref_id`; a `market_transaction` journal entry links to its
/// transaction via `context_id`. The lookup is scoped to the row's `owner` so a
/// corp-mirrored trade (the same EVE id in a character and a corp wallet) never
/// cascades onto the other owner's twin. Returns `None` when the entry has no
/// 100% match.
fn budget_cascade_target(
  state: &State,
  owner: BudgetOwner,
  kind: BudgetEntryKind,
  entry_id: i64,
) -> Option<(BudgetEntryKind, i64)> {
  match kind {
    BudgetEntryKind::Market => {
      let transaction = state
        .market
        .iter()
        .find(|entry| entry.owner == owner && entry.transaction_id == entry_id)?;
      (transaction.journal_ref_id != 0).then_some((BudgetEntryKind::Journal, transaction.journal_ref_id))
    }
    BudgetEntryKind::Journal => {
      let entry = state
        .journal
        .iter()
        .find(|entry| entry.owner == owner && entry.id == entry_id)?;
      if entry.ref_type != "market_transaction" {
        return None;
      }
      entry
        .context_id
        .map(|transaction_id| (BudgetEntryKind::Market, transaction_id))
    }
  }
}

/// Whether a journal entry satisfies an active Budget filter: in the filter's
/// month, and either assigned to the filtered category or — for the
/// uncategorized filter — an unassigned entry (inflow or outflow) that still
/// needs a category. Market-transaction journal twins are excluded; their trade
/// is reviewed and assigned from the Transactions table instead.
fn journal_budget_match(entry: &JournalEntry, filter: &BudgetFilter, chips: &loaders::BudgetChips) -> bool {
  if crate::features::budget::month_key(&entry.date).as_deref() != Some(filter.month.as_str()) {
    return false;
  }
  let assigned = chips
    .resolution
    .resolve_target(BudgetEntryKind::Journal, entry.id, &entry.match_target());
  match filter.kind {
    BudgetFilterKind::Category(id) => assigned == Some(id),
    BudgetFilterKind::Uncategorized => {
      assigned.is_none() && entry.ref_type != "market_transaction" && entry.amount.is_some()
    }
  }
}

/// Whether a transaction satisfies an active Budget filter: in the filter's
/// month, and either assigned to the filtered category or — for the
/// uncategorized filter — an unassigned trade. Both buys and sells can be
/// assigned to any category.
fn market_budget_match(entry: &MarketEntry, filter: &BudgetFilter, chips: &loaders::BudgetChips) -> bool {
  if crate::features::budget::month_key(&entry.date).as_deref() != Some(filter.month.as_str()) {
    return false;
  }
  let assigned = chips
    .resolution
    .resolve_target(BudgetEntryKind::Market, entry.transaction_id, &entry.match_target());
  match filter.kind {
    BudgetFilterKind::Category(id) => assigned == Some(id),
    BudgetFilterKind::Uncategorized => assigned.is_none(),
  }
}

fn humanize_ref_type(ref_type: &str) -> String {
  crate::features::budget::humanize_ref_type(ref_type)
}

pub fn journal_type_glyph(entry: &JournalEntry) -> (&'static str, bool) {
  use crate::ui::components::glyph_badge::{GLYPH_EXPENSE, GLYPH_INCOME};

  let is_in = match entry.amount {
    Some(amount) if amount != 0.0 => amount > 0.0,
    _ => is_income_ref_type(&entry.ref_type),
  };
  if is_in {
    (GLYPH_INCOME, true)
  } else {
    (GLYPH_EXPENSE, false)
  }
}

fn is_income_ref_type(ref_type: &str) -> bool {
  is_known_income_ref_type(ref_type)
    || (!ref_type.contains("fee") && !ref_type.contains("tax") && !ref_type.contains("cost"))
}

fn is_known_income_ref_type(ref_type: &str) -> bool {
  matches!(
    ref_type,
    "bounty_prizes"
      | "agent_mission_reward"
      | "agent_mission_time_bonus_reward"
      | "market_transaction"
      | "insurance"
      | "player_donation"
      | "contract_price_payment_corp"
      | "lp_store"
      | "project_reward"
      | "industry_job_tax"
  )
}

pub fn journal_flow(entries: &[&JournalEntry]) -> JournalFlow {
  let mut flow = JournalFlow::default();
  for entry in entries {
    match entry.amount {
      Some(amount) if amount > 0.0 => flow.income += amount,
      Some(amount) if amount < 0.0 => flow.spend += -amount,
      _ => {}
    }
  }
  flow.net = flow.income - flow.spend;
  flow
}

pub fn category_flows(entries: &[&JournalEntry]) -> Vec<CategoryFlow> {
  let mut by_type: std::collections::HashMap<&str, CategoryFlow> = std::collections::HashMap::new();
  for entry in entries {
    let bucket = by_type.entry(entry.ref_type.as_str()).or_insert_with(|| CategoryFlow {
      ref_type: entry.ref_type.clone(),
      ..CategoryFlow::default()
    });
    match entry.amount {
      Some(amount) if amount > 0.0 => bucket.income += amount,
      Some(amount) if amount < 0.0 => bucket.spend += -amount,
      _ => {}
    }
  }
  let mut flows: Vec<CategoryFlow> = by_type.into_values().filter(|flow| flow.total() > 0.0).collect();
  flows.sort_by(|a, b| b.total().total_cmp(&a.total()));
  flows
}

fn market_matches(entry: &MarketEntry, sign: SignFilter, side: Side, query: &str) -> bool {
  match sign {
    SignFilter::In if entry.is_buy => return false,
    SignFilter::Out if !entry.is_buy => return false,
    _ => {}
  }
  match side {
    Side::Buy if !entry.is_buy => return false,
    Side::Sell if entry.is_buy => return false,
    _ => {}
  }
  query.is_empty() || entry.item.to_lowercase().contains(query) || entry.location.to_lowercase().contains(query)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn pilot(id: i64, liquid: Option<f64>) -> RosterPilot {
    RosterPilot {
      corp: "TST".to_owned(),
      granted_scopes: None,
      id,
      liquid,
      name: format!("Pilot {id}"),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
    }
  }

  fn financials(character_id: i64, liquid: Option<f64>) -> CharacterFinancials {
    CharacterFinancials {
      character_id,
      liquid,
      asset_value: None,
      escrow: None,
      net_worth: liquid,
    }
  }

  fn corp_division(division: i64, name: Option<&str>, balance: Option<f64>) -> CorpDivision {
    CorpDivision {
      balance,
      division,
      name: name.map(str::to_owned),
    }
  }

  fn corp(id: i64, name: &str) -> RosterCorp {
    RosterCorp {
      id,
      liquid: None,
      logo: corp_logo_stale(id),
      name: name.to_owned(),
      ticker: "TSTC".to_owned(),
    }
  }

  fn corp_with_liquid(id: i64, liquid: Option<f64>) -> RosterCorp {
    RosterCorp {
      id,
      liquid,
      logo: corp_logo_stale(id),
      name: format!("Corp {id}"),
      ticker: "TSTC".to_owned(),
    }
  }

  fn corp_logo_stale(id: i64) -> images::ImageState {
    images::ImageState::Stale {
      id,
      kind: images::ImageKind::CorporationLogo,
    }
  }

  fn load_wallet_for_test() -> Loaded {
    Loaded {
      chips: loaders::BudgetChips::default(),
      contract_total: 0,
      contracts: Vec::new(),
      corp_divisions: Vec::new(),
      corporations: Vec::new(),
      financials: Vec::new(),
      journal: Vec::new(),
      journal_total: 0,
      market: Vec::new(),
      market_total: 0,
      net_worth_series: Vec::new(),
      periods: Vec::new(),
      right_rail_width: 280.0,
      roster: Vec::new(),
      wallet_sections: Vec::new(),
    }
  }

  fn journal_entry(character_id: i64, amount: Option<f64>, ref_type: &str, description: &str) -> JournalEntry {
    JournalEntry {
      amount,
      balance: None,
      character_id,
      context_id: None,
      date: "2026-05-30T12:00:00Z".to_owned(),
      description: description.to_owned(),
      id: 1,
      owner: BudgetOwner::Character(character_id),
      reason: None,
      ref_type: ref_type.to_owned(),
    }
  }

  fn market_entry(character_id: i64, is_buy: bool, item: &str, location: &str) -> MarketEntry {
    MarketEntry {
      character_id,
      date: "2026-05-30T12:00:00Z".to_owned(),
      is_buy,
      item: item.to_owned(),
      journal_ref_id: 0,
      location: location.to_owned(),
      owner: BudgetOwner::Character(character_id),
      quantity: 1,
      total: 1.0,
      transaction_id: 1,
      type_icon: images::IconResolution::Missing,
      type_id: 34,
      unit_price: 1.0,
    }
  }

  fn contract_entry(character_id: i64, is_buy: bool, status: &str, contract_type: &str) -> ContractEntry {
    ContractEntry {
      acceptor: None,
      acceptor_id: None,
      acceptor_image: PartyImage::default(),
      assignee: Some("Assignee Pilot".to_owned()),
      assignee_id: Some(98_765),
      assignee_image: PartyImage::default(),
      character_id,
      collateral: Some(5_000.0),
      contract_id: 12_345,
      date_expired: None,
      date_issued: "2026-05-30T12:00:00Z".to_owned(),
      is_buy,
      issuer: Some("Issuer Pilot".to_owned()),
      issuer_id: 11_111,
      issuer_image: PartyImage::default(),
      status: status.to_owned(),
      value: Some(200.0),
      r#type: contract_type.to_owned(),
    }
  }

  fn contract_detail_fixture() -> contract_detail::ContractDetail {
    contract_detail::ContractDetail {
      acceptor: None,
      availability: "Public".to_owned(),
      bids: Vec::new(),
      buyout: None,
      collateral: None,
      contract_id: 12_345,
      days_to_complete: Some(0),
      expiry: contract_detail::ExpiryView {
        future: true,
        label: "Open".to_owned(),
        title: "Expires",
      },
      headline: 200.0,
      headline_label: "Price",
      issued_time: "2026-05-30T12:00:00Z".to_owned(),
      issuer: contract_detail::PartyView {
        name: "Issuer Pilot".to_owned(),
        portrait: images::ImageState::Fresh("/tmp/p.jpg".into()),
        role: "Issuer",
        sub: None,
      },
      items: Vec::new(),
      items_value: 0.0,
      kind: contract_detail::ContractKind::ItemExchange,
      location_name: "Jita IV - Moon 4".to_owned(),
      route: None,
      status: "outstanding".to_owned(),
      title: "Test Contract".to_owned(),
      volume: 0.0,
    }
  }

  fn period(character_id: i64, income: f64, spend: f64) -> CharacterWalletPeriodSummary {
    CharacterWalletPeriodSummary {
      character_id,
      period: "2026-05".to_owned(),
      income,
      spend,
      net: income - spend,
    }
  }

  fn nw_point(date: &str, net_worth: f64) -> NetWorthPoint {
    NetWorthPoint {
      date: date.to_owned(),
      liquid: 0.0,
      net_worth,
    }
  }

  mod handle_budget {
    use super::*;

    fn budget_view() -> budget::BudgetView {
      budget::BudgetView {
        groups: Vec::new(),
        month: "2026-06".to_owned(),
        overspent: 0.0,
        pool: 0.0,
        ready_to_assign: 0.0,
      }
    }

    #[tokio::test]
    async fn it_selects_a_category_and_clears_the_editor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::BudgetCategorySelected(7), &db);

      assert_eq!(state.budget_selected, Some(7));
      assert!(state.budget_editor.is_none());
    }

    #[tokio::test]
    async fn it_toggles_a_group_collapsed_then_expanded() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::BudgetGroupToggled(3), &db);
      assert!(state.budget_collapsed.contains(&3));

      let _ = update(&mut state, Message::BudgetGroupToggled(3), &db);
      assert!(!state.budget_collapsed.contains(&3));
    }

    #[tokio::test]
    async fn it_opens_and_dismisses_the_chip_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.budget_chips.envelopes = vec![loaders::EnvelopeGroup {
        categories: Vec::new(),
        name: "Group".to_owned(),
      }];

      let _ = update(
        &mut state,
        Message::BudgetChipOpened(BudgetOwner::Character(1), BudgetEntryKind::Journal, 9),
        &db,
      );
      assert_eq!(
        state.budget_picker,
        Some((BudgetOwner::Character(1), BudgetEntryKind::Journal, 9))
      );

      let _ = update(&mut state, Message::BudgetChipDismissed, &db);
      assert!(state.budget_picker.is_none());
    }

    #[tokio::test]
    async fn it_applies_a_reloaded_chip_set() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.budget_filter = Some(BudgetFilter {
        kind: BudgetFilterKind::Uncategorized,
        month: "2026-06".to_owned(),
      });

      let mut chips = loaders::BudgetChips::default();
      chips.meta.insert(
        1,
        loaders::Envelope {
          id: 1,
          name: "Bills".to_owned(),
          tone: None,
        },
      );

      let _ = update(&mut state, Message::BudgetChipsReloaded(Box::new(chips)), &db);

      assert!(state.budget_chips.meta.contains_key(&1));
    }

    #[tokio::test]
    async fn it_assigns_the_open_chip_to_a_category() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.budget_picker = Some((BudgetOwner::Character(1), BudgetEntryKind::Journal, 9));

      let _ = update(&mut state, Message::BudgetChipAssigned(Some(7)), &db);

      assert!(state.budget_picker.is_none());
    }

    #[tokio::test]
    async fn it_clears_the_open_chip_when_assigned_to_nothing() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.budget_picker = Some((BudgetOwner::Character(1), BudgetEntryKind::Market, 9));

      let _ = update(&mut state, Message::BudgetChipAssigned(None), &db);

      assert!(state.budget_picker.is_none());
    }

    #[tokio::test]
    async fn it_no_ops_assigning_when_no_chip_is_open() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::BudgetChipAssigned(Some(7)), &db);

      assert!(state.budget_picker.is_none());
    }

    #[tokio::test]
    async fn it_applies_a_loaded_view_for_the_active_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::BudgetLoaded(Box::new(BudgetLoad {
          history: Vec::new(),
          scope: Scope::All,
          select: Some(5),
          view: budget_view(),
        })),
        &db,
      );

      assert_eq!(state.budget_selected, Some(5));
      assert!(state.budget.is_some());
    }

    #[tokio::test]
    async fn it_ignores_a_loaded_view_for_a_stale_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::All;

      let _ = update(
        &mut state,
        Message::BudgetLoaded(Box::new(BudgetLoad {
          history: Vec::new(),
          scope: Scope::Character(1),
          select: Some(5),
          view: budget_view(),
        })),
        &db,
      );

      assert!(state.budget.is_none());
    }

    #[tokio::test]
    async fn it_sets_the_mode_and_range() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::BudgetModeSelected(budget::Mode::Reflect), &db);
      assert_eq!(state.budget_mode, budget::Mode::Reflect);

      let _ = update(
        &mut state,
        Message::BudgetRangeSelected(budget::BudgetRange::default()),
        &db,
      );
    }

    #[tokio::test]
    async fn it_cancels_an_assign_edit() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.budget_editing = Some(budget::EditingCell {
        category_id: 1,
        draft: "100".to_owned(),
      });

      let _ = update(&mut state, Message::BudgetAssignCancelled, &db);

      assert!(state.budget_editing.is_none());
    }
  }

  mod budget_review {
    use pretty_assertions::assert_eq;

    use super::*;

    // The test ledger rows all belong to character 1, so their overrides key on
    // that owner — matching the owner the chip/filter resolve from each row.
    fn chips(journal: &[(i64, i64)], market: &[(i64, i64)]) -> loaders::BudgetChips {
      let key = |entries: &[(i64, i64)]| {
        entries
          .iter()
          .map(|&(entry_id, category_id)| ((BudgetOwner::Character(1), entry_id), category_id))
          .collect()
      };
      loaders::BudgetChips {
        envelopes: Vec::new(),
        meta: std::collections::HashMap::new(),
        resolution: crate::features::budget::ResolutionContext {
          journal_overrides: key(journal),
          market_overrides: key(market),
          ref_overrides: std::collections::HashMap::new(),
          rules: Vec::new(),
          slug_to_id: std::collections::HashMap::new(),
        },
      }
    }

    #[test]
    fn it_cascades_a_transaction_assignment_to_its_journal_twin() {
      let mut state = State::new();
      let mut transaction = market_entry(1, true, "Tritanium", "Jita");
      transaction.transaction_id = 500;
      transaction.journal_ref_id = 10;
      state.market = vec![transaction];

      assert_eq!(
        budget_cascade_target(&state, BudgetOwner::Character(1), BudgetEntryKind::Market, 500),
        Some((BudgetEntryKind::Journal, 10))
      );
    }

    #[test]
    fn it_does_not_cascade_to_another_owners_twin_sharing_the_eve_id() {
      let mut state = State::new();
      let mut character = market_entry(1, true, "Tritanium", "Jita");
      character.transaction_id = 500;
      character.journal_ref_id = 10;
      let mut corp = market_entry(1, true, "Tritanium", "Jita");
      corp.transaction_id = 500;
      corp.journal_ref_id = 20;
      corp.owner = BudgetOwner::Corporation(98_000_001);
      state.market = vec![character, corp];

      assert_eq!(
        budget_cascade_target(
          &state,
          BudgetOwner::Corporation(98_000_001),
          BudgetEntryKind::Market,
          500
        ),
        Some((BudgetEntryKind::Journal, 20))
      );
    }

    #[test]
    fn it_cascades_a_journal_twin_assignment_to_its_transaction() {
      let mut state = State::new();
      let mut twin = journal_entry(1, Some(-100.0), "market_transaction", "Buy");
      twin.id = 10;
      twin.context_id = Some(500);
      state.journal = vec![twin];

      assert_eq!(
        budget_cascade_target(&state, BudgetOwner::Character(1), BudgetEntryKind::Journal, 10),
        Some((BudgetEntryKind::Market, 500))
      );
    }

    #[test]
    fn it_does_not_cascade_a_plain_journal_entry() {
      let mut state = State::new();
      let mut fee = journal_entry(1, Some(-50.0), "brokers_fee", "Fee");
      fee.id = 11;
      fee.context_id = Some(500);
      state.journal = vec![fee];

      assert_eq!(
        budget_cascade_target(&state, BudgetOwner::Character(1), BudgetEntryKind::Journal, 11),
        None
      );
    }

    #[test]
    fn it_filters_the_journal_to_an_assigned_category_for_the_month() {
      let mut state = State::new();
      let mut assigned = journal_entry(1, Some(-100.0), "manufacturing", "In");
      assigned.id = 1;
      assigned.date = "2026-06-05T00:00:00Z".to_owned();
      let mut other = journal_entry(1, Some(-200.0), "manufacturing", "Out");
      other.id = 2;
      other.date = "2026-06-06T00:00:00Z".to_owned();
      state.journal = vec![assigned, other];
      state.budget_chips = chips(&[(1, 42)], &[]);
      state.budget_filter = Some(BudgetFilter {
        kind: BudgetFilterKind::Category(42),
        month: "2026-06".to_owned(),
      });
      state.recompute_derived();

      let filtered = filtered_journal(&state);

      assert_eq!(filtered.len(), 1);
      assert_eq!(filtered[0].id, 1);
    }

    #[test]
    fn it_counts_uncategorized_entries_for_the_selected_month() {
      let mut state = State::new();
      let mut buy = journal_entry(1, Some(-100.0), "manufacturing", "Buy");
      buy.id = 1;
      buy.date = "2026-06-05T00:00:00Z".to_owned();
      // Inflows are assignable now, so an unassigned one also needs a category.
      let mut inflow = journal_entry(1, Some(900.0), "bounty_prizes", "Bounty");
      inflow.id = 2;
      inflow.date = "2026-06-05T00:00:00Z".to_owned();
      // A market_transaction twin is excluded (assigned from the Transactions tab).
      let mut twin = journal_entry(1, Some(-300.0), "market_transaction", "Twin");
      twin.id = 3;
      twin.date = "2026-06-05T00:00:00Z".to_owned();
      // Out-of-month entries never count.
      let mut last_month = journal_entry(1, Some(-100.0), "manufacturing", "Old");
      last_month.id = 4;
      last_month.date = "2026-05-30T00:00:00Z".to_owned();
      state.journal = vec![buy, inflow, twin, last_month];
      state.budget_month = "2026-06".to_owned();

      assert_eq!(state.budget_uncategorized_count(), 2);
    }
  }

  mod category_flows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_categories_with_no_signed_movement() {
      let entries = [
        journal_entry(1, None, "unknown", "No amount"),
        journal_entry(1, Some(0.0), "unknown", "Zero"),
        journal_entry(1, Some(75.0), "bounty_prizes", "Bounty"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows.len(), 1);
      assert_eq!(flows[0].ref_type, "bounty_prizes");
    }

    #[test]
    fn it_groups_by_ref_type_and_orders_by_combined_magnitude() {
      let entries = [
        journal_entry(1, Some(100.0), "bounty_prizes", "Bounty"),
        journal_entry(1, Some(50.0), "bounty_prizes", "Bounty"),
        journal_entry(1, Some(-1_000.0), "market_transaction", "Buy"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows.len(), 2);
      assert_eq!(flows[0].ref_type, "market_transaction");
      assert_eq!(flows[0].spend, 1_000.0);
      assert_eq!(flows[0].income, 0.0);
      assert_eq!(flows[1].ref_type, "bounty_prizes");
      assert_eq!(flows[1].income, 150.0);
      assert_eq!(flows[1].total(), 150.0);
    }

    #[test]
    fn it_humanizes_the_ref_type_label() {
      let entries = [journal_entry(1, Some(10.0), "agent_mission_reward", "Mission")];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows[0].label(), "Agent Mission Reward");
    }

    #[test]
    fn it_splits_a_single_category_into_in_and_out() {
      let entries = [
        journal_entry(1, Some(300.0), "corporation_account_withdrawal", "In"),
        journal_entry(1, Some(-100.0), "corporation_account_withdrawal", "Out"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flows = super::category_flows(&refs);

      assert_eq!(flows.len(), 1);
      assert_eq!(flows[0].income, 300.0);
      assert_eq!(flows[0].spend, 100.0);
      assert_eq!(flows[0].total(), 400.0);
    }
  }

  mod composition_stack {
    use pretty_assertions::assert_eq;

    use super::*;

    fn financials_nw(character_id: i64, net_worth: Option<f64>) -> CharacterFinancials {
      CharacterFinancials {
        character_id,
        liquid: None,
        asset_value: None,
        escrow: None,
        net_worth,
      }
    }

    #[test]
    fn it_drops_characters_with_no_synced_net_worth() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.financials = vec![financials_nw(1, Some(100.0)), financials_nw(2, None)];
      state.active = Scope::All;

      let stack = super::composition_stack(&state);

      assert_eq!(stack.len(), 1);
      assert_eq!(stack[0].id, 1);
    }

    #[test]
    fn it_is_empty_outside_all_wallets_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials_nw(1, Some(100.0))];
      state.active = Scope::Character(1);

      assert!(super::composition_stack(&state).is_empty());
    }

    #[test]
    fn it_orders_characters_by_net_worth_descending() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None), pilot(3, None)];
      state.financials = vec![
        financials_nw(1, Some(100.0)),
        financials_nw(2, Some(300.0)),
        financials_nw(3, Some(200.0)),
      ];
      state.active = Scope::All;

      let stack = super::composition_stack(&state);

      assert_eq!(stack.iter().map(|s| s.id).collect::<Vec<_>>(), vec![2, 3, 1]);
    }
  }

  mod contract_loader_target {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_none_when_no_row_matches() {
      let state = State::new();

      assert_eq!(super::contract_loader_target(&state, 999), None);
    }

    #[test]
    fn it_targets_the_active_corporation_under_a_corp_scope() {
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);

      assert_eq!(
        super::contract_loader_target(&state, 12_345),
        Some(ContractLoad::Corporation(98_000_001))
      );
    }

    #[test]
    fn it_targets_the_owning_character_under_an_all_scope() {
      let mut state = State::new();
      state.contracts = vec![contract_entry(7, false, "finished", "item_exchange")];

      assert_eq!(
        super::contract_loader_target(&state, 12_345),
        Some(ContractLoad::Character(7))
      );
    }
  }

  mod contract_matches {
    use super::*;

    #[test]
    fn it_composes_side_and_query() {
      let sell = contract_entry(1, false, "outstanding", "item_exchange");

      assert!(super::contract_matches(&sell, Side::Sell, "exchange"));
      assert!(!super::contract_matches(&sell, Side::Buy, "exchange"));
    }

    #[test]
    fn it_filters_by_side() {
      let buy = contract_entry(1, true, "outstanding", "courier");
      let sell = contract_entry(1, false, "outstanding", "item_exchange");

      assert!(super::contract_matches(&buy, Side::Buy, ""));
      assert!(!super::contract_matches(&sell, Side::Buy, ""));
      assert!(super::contract_matches(&sell, Side::Sell, ""));
      assert!(!super::contract_matches(&buy, Side::Sell, ""));
      assert!(super::contract_matches(&buy, Side::All, ""));
      assert!(super::contract_matches(&sell, Side::All, ""));
    }

    #[test]
    fn it_matches_the_query_against_type_status_and_parties() {
      let entry = contract_entry(1, false, "outstanding", "item_exchange");

      assert!(super::contract_matches(&entry, Side::All, "exchange"));
      assert!(super::contract_matches(&entry, Side::All, "outstanding"));
      assert!(super::contract_matches(&entry, Side::All, "issuer"));
      assert!(super::contract_matches(&entry, Side::All, "assignee"));
      assert!(super::contract_matches(&entry, Side::All, "12345"));
      assert!(!super::contract_matches(&entry, Side::All, "courier"));
    }
  }

  mod corp_balance_total {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_none_when_no_division_has_a_balance() {
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![corp_division(1, Some("Master"), None)];

      assert_eq!(state.corp_balance_total(), None);
    }

    #[test]
    fn it_sums_the_synced_division_balances() {
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![
        corp_division(1, Some("Master"), Some(1_000.0)),
        corp_division(2, None, Some(250.0)),
        corp_division(3, None, None),
      ];

      assert_eq!(state.corp_balance_total(), Some(1_250.0));
    }
  }

  mod corp_division_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_a_division_number_label_when_unnamed() {
      assert_eq!(corp_division(4, None, None).label(), "Division 4");
    }

    #[test]
    fn it_uses_the_synced_name_when_present() {
      assert_eq!(corp_division(2, Some("Trading"), None).label(), "Trading");
    }
  }

  mod wallets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_cold_opens_on_the_wallets_tab() {
      let state = State::new();

      assert_eq!(state.tab, Tab::Wallets);
    }

    #[tokio::test]
    async fn it_toggles_the_section_sort_order() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::WalletsSortSelected(WalletSort::Ascending), &db);

      assert_eq!(state.wallets_sort(), WalletSort::Ascending);
    }

    #[tokio::test]
    async fn it_keeps_the_wallets_tab_off_pagination() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(10.0))];
      state.active = Scope::All;
      state.tab = Tab::Wallets;

      let _ = update(
        &mut state,
        Message::TabScrolled {
          absolute: 100.0,
          relative: 0.99,
        },
        &db,
      );

      assert!(!state.loading_more);
    }

    #[test]
    fn it_subtotals_a_section_over_present_division_balances() {
      let section = CorpWalletSection {
        divisions: vec![
          corp_division(1, Some("Master Wallet"), Some(100.0)),
          corp_division(2, Some("Operations"), None),
          corp_division(3, Some("Reserve"), Some(50.0)),
        ],
        granted_by: Some("Pilot 1".to_owned()),
        id: 98_000_001,
        logo: corp_logo_stale(98_000_001),
        name: "Test Corp".to_owned(),
        role: Some("Director".to_owned()),
        ticker: "TSTC".to_owned(),
      };

      assert_eq!(section.subtotal(), 150.0);
    }
  }

  mod filtered_contracts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_applies_the_active_side_filter() {
      let mut state = State::new();
      state.contracts = vec![
        contract_entry(1, true, "outstanding", "courier"),
        contract_entry(1, false, "finished", "item_exchange"),
      ];

      state.side_filter = Side::Buy;
      state.recompute_derived();
      assert_eq!(super::filtered_contracts(&state).len(), 1);
      assert!(super::filtered_contracts(&state)[0].is_buy);

      state.side_filter = Side::Sell;
      state.recompute_derived();
      assert_eq!(super::filtered_contracts(&state).len(), 1);
      assert!(!super::filtered_contracts(&state)[0].is_buy);

      state.side_filter = Side::All;
      state.recompute_derived();
      assert_eq!(super::filtered_contracts(&state).len(), 2);
    }

    #[test]
    fn it_is_empty_when_no_contracts_are_synced() {
      let state = State::new();

      assert!(super::filtered_contracts(&state).is_empty());
      assert!(!state.has_contracts());
    }
  }

  mod integration {
    use super::*;

    #[tokio::test]
    async fn it_drives_every_pane_off_db_only() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          chips: loaders::BudgetChips::default(),
          contract_total: 0,
          contracts: vec![],
          corp_divisions: vec![],
          corporations: vec![corp(98_000_001, "Test Corp")],
          financials: vec![financials(1, Some(100.0)), financials(2, Some(50.0))],
          journal: vec![journal_entry(1, Some(10.0), "bounty_prizes", "ratting")],
          journal_total: 1,
          market: vec![market_entry(1, true, "Tritanium", "Jita IV")],
          market_total: 1,
          net_worth_series: vec![nw_point("2026-06-01", 150.0), nw_point("2026-06-02", 175.0)],
          periods: vec![period(1, 100.0, 40.0)],
          right_rail_width: 280.0,
          roster: vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))],
          wallet_sections: vec![],
        })),
        &db,
      );

      let interactions = [
        Message::PickerToggled,
        Message::ScopeSelected(Scope::Character(1)),
        Message::ScopeSelected(Scope::Corporation(98_000_001)),
        Message::DivisionSelected(2),
        Message::ScopeSelected(Scope::All),
        Message::TabSelected(Tab::Market),
        Message::TabSelected(Tab::Journal),
        Message::SignFilterChanged(SignFilter::In),
        Message::SearchChanged("tritanium".to_owned()),
        Message::TimeframeSelected(Timeframe::Year),
        Message::ChartHovered(Some(0.5)),
      ];
      for message in interactions {
        let _ = update(&mut state, message, &db);
        let _el: Element<'_, Message> = view(&state, Utc::now());
      }
    }

    #[test]
    fn it_renders_across_every_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.corporations = vec![corp(98_000_001, "Test Corp")];

      for scope in [Scope::All, Scope::Character(1), Scope::Corporation(98_000_001)] {
        state.active = scope;
        let _el: Element<'_, Message> = view(&state, Utc::now());
      }
    }
  }

  mod journal_flow {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ignores_null_and_zero_amounts() {
      let entries = [
        journal_entry(1, None, "unknown", "No amount"),
        journal_entry(1, Some(0.0), "zero", "Zero"),
        journal_entry(1, Some(-100.0), "tax", "Tax"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flow = super::journal_flow(&refs);

      assert_eq!(flow.income, 0.0);
      assert_eq!(flow.spend, 100.0);
      assert_eq!(flow.net, -100.0);
    }

    #[test]
    fn it_is_zero_for_an_empty_page() {
      assert_eq!(super::journal_flow(&[]), JournalFlow::default());
    }

    #[test]
    fn it_sums_income_and_spend_and_nets_them() {
      let entries = [
        journal_entry(1, Some(1_000.0), "bounty_prizes", "Bounty"),
        journal_entry(1, Some(250.0), "agent_mission_reward", "Mission"),
        journal_entry(1, Some(-400.0), "market_transaction", "Buy"),
      ];
      let refs: Vec<&JournalEntry> = entries.iter().collect();

      let flow = super::journal_flow(&refs);

      assert_eq!(flow.income, 1_250.0);
      assert_eq!(flow.spend, 400.0);
      assert_eq!(flow.net, 850.0);
    }
  }

  mod journal_matches {
    use super::*;

    #[test]
    fn it_composes_sign_and_query() {
      let entry = journal_entry(1, Some(500.0), "bounty_prizes", "Serpentis bounty");

      assert!(super::journal_matches(&entry, SignFilter::In, "serpentis"));
      assert!(!super::journal_matches(&entry, SignFilter::Out, "serpentis"));
    }

    #[test]
    fn it_keeps_income_for_the_in_sign_and_drops_spend() {
      let income = journal_entry(1, Some(100.0), "bounty_prizes", "Bounty");
      let spend = journal_entry(1, Some(-100.0), "market_transaction", "Buy");

      assert!(super::journal_matches(&income, SignFilter::In, ""));
      assert!(!super::journal_matches(&spend, SignFilter::In, ""));
    }

    #[test]
    fn it_keeps_spend_for_the_out_sign_and_drops_income() {
      let income = journal_entry(1, Some(100.0), "bounty_prizes", "Bounty");
      let spend = journal_entry(1, Some(-100.0), "market_transaction", "Buy");

      assert!(!super::journal_matches(&income, SignFilter::Out, ""));
      assert!(super::journal_matches(&spend, SignFilter::Out, ""));
    }

    #[test]
    fn it_matches_the_query_against_ref_type_and_description() {
      let entry = journal_entry(
        1,
        Some(1.0),
        "agent_mission_reward",
        "Distribution run for Sister Alitura",
      );

      assert!(super::journal_matches(&entry, SignFilter::All, "alitura"));
      assert!(super::journal_matches(&entry, SignFilter::All, "mission"));
      assert!(!super::journal_matches(&entry, SignFilter::All, "tritanium"));
    }

    #[test]
    fn it_matches_the_humanized_ref_type_label() {
      let entry = journal_entry(1, Some(1.0), "daily_goal_payouts", "Payout");

      assert!(super::journal_matches(&entry, SignFilter::All, "daily"));
    }

    #[test]
    fn it_matches_the_reason_text() {
      let mut entry = journal_entry(1, Some(-1.0), "player_donation", "Donation");
      entry.reason = Some("Loot buyback settlement".to_owned());

      assert!(super::journal_matches(&entry, SignFilter::All, "buyback"));
    }
  }

  mod journal_type_glyph {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ui::components::glyph_badge::{GLYPH_EXPENSE, GLYPH_INCOME};

    #[test]
    fn it_classifies_fee_tax_and_cost_ref_types_as_expense() {
      for ref_type in [
        "brokers_fee",
        "transaction_tax",
        "reprocessing_tax",
        "jump_clone_activation_cost",
      ] {
        let entry = journal_entry(1, None, ref_type, "Charge");

        assert_eq!(
          super::journal_type_glyph(&entry),
          (GLYPH_EXPENSE, false),
          "{ref_type} should read as an expense"
        );
      }
    }

    #[test]
    fn it_falls_back_to_the_ref_type_when_the_amount_is_absent() {
      let income = journal_entry(1, None, "bounty_prizes", "Bounty");
      let expense = journal_entry(1, None, "brokers_fee", "Fee");

      assert_eq!(super::journal_type_glyph(&income), (GLYPH_INCOME, true));
      assert_eq!(super::journal_type_glyph(&expense), (GLYPH_EXPENSE, false));
    }

    #[test]
    fn it_reads_a_negative_amount_as_expense() {
      let entry = journal_entry(1, Some(-400.0), "market_transaction", "Buy");

      assert_eq!(super::journal_type_glyph(&entry), (GLYPH_EXPENSE, false));
    }

    #[test]
    fn it_reads_a_positive_amount_as_income() {
      let entry = journal_entry(1, Some(1_000.0), "player_donation", "Gift");

      assert_eq!(super::journal_type_glyph(&entry), (GLYPH_INCOME, true));
    }
  }

  mod load_more {
    use super::*;

    async fn ready_state(tab: Tab) -> State {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;
      state.tab = tab;
      state
    }

    #[tokio::test]
    async fn it_no_ops_for_a_corporation_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Journal).await;
      state.active = Scope::Corporation(98_000_001);

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_for_a_corporation_scope_on_a_non_contract_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Market).await;
      state.active = Scope::Corporation(98_000_001);

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_when_no_characters_are_in_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::All;
      state.tab = Tab::Journal;

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_when_the_tab_is_exhausted() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Journal).await;
      state.tab_exhausted = true;

      let _ = super::load_more(&mut state, &db);

      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_no_ops_while_a_page_is_already_loading() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Journal).await;
      state.loading_more = true;

      let _ = super::load_more(&mut state, &db);

      assert!(state.loading_more);
    }

    #[tokio::test]
    async fn it_starts_a_corp_contracts_page_load() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Contracts).await;
      state.active = Scope::Corporation(98_000_001);

      let _task = super::load_more(&mut state, &db);

      assert!(state.loading_more);
    }

    #[tokio::test]
    async fn it_starts_a_corp_contracts_page_load_from_the_last_entry_cursor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = ready_state(Tab::Contracts).await;
      state.active = Scope::Corporation(98_000_001);
      state.contracts = vec![contract_entry(1, false, "finished", "item_exchange")];

      let _task = super::load_more(&mut state, &db);

      assert!(state.loading_more);
    }

    #[tokio::test]
    async fn it_starts_a_page_load_for_each_tab() {
      let db = crate::store::open_test().await.unwrap();

      for tab in [Tab::Journal, Tab::Market, Tab::Contracts] {
        let mut state = ready_state(tab).await;

        let _task = super::load_more(&mut state, &db);

        assert!(state.loading_more, "starting a {tab:?} page marks the state loading");
      }
    }

    #[tokio::test]
    async fn it_starts_a_page_load_from_the_last_entry_cursor_for_each_tab() {
      let db = crate::store::open_test().await.unwrap();

      let mut journal = ready_state(Tab::Journal).await;
      journal.journal = vec![journal_entry(1, Some(1.0), "player_trading", "trade")];
      let _journal_task = super::load_more(&mut journal, &db);
      assert!(journal.loading_more);

      let mut market = ready_state(Tab::Market).await;
      market.market = vec![market_entry(1, true, "Tritanium", "Jita")];
      let _market_task = super::load_more(&mut market, &db);
      assert!(market.loading_more);

      let mut contracts = ready_state(Tab::Contracts).await;
      contracts.contracts = vec![contract_entry(1, false, "finished", "item_exchange")];
      let _contracts_task = super::load_more(&mut contracts, &db);
      assert!(contracts.loading_more);
    }
  }

  mod load_net_worth_series {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_character(db: &Database, id: i64) {
      use crate::store::{
        model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
        repo::character::insert_with_org,
      };

      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Test Pilot");
      insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_adds_owned_corp_net_worth_into_the_all_wallets_series() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let today = Utc::now().format("%Y-%m-%d").to_string();
      crate::store::repo::finance::upsert(&db, 42, &today, 100.0, None, None, 175.0)
        .await
        .unwrap();
      sqlx::query(
        "INSERT INTO corporation_net_worth_snapshot (corporation_id, date, liquid, net_worth) VALUES (?, ?, ?, ?)",
      )
      .bind(90_000_001_i64)
      .bind(&today)
      .bind(1_250.0)
      .bind(1_250.0)
      .execute(&db.0)
      .await
      .unwrap();

      let corporations = vec![corp(90_000_001, "Test Corp")];
      let series = super::load_net_worth_series(&db, Scope::All, &[42], &corporations).await;

      assert_eq!(series.len(), 1);
      assert_eq!(series[0].net_worth, 175.0 + 1_250.0);
    }

    #[tokio::test]
    async fn it_is_empty_for_a_corp_scope() {
      let db = crate::store::open_test().await.unwrap();

      assert!(
        super::load_net_worth_series(&db, Scope::Corporation(1), &[], &[])
          .await
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_is_empty_for_every_scope_without_snapshots() {
      let db = crate::store::open_test().await.unwrap();

      assert!(super::load_net_worth_series(&db, Scope::All, &[], &[]).await.is_empty());
      assert!(
        super::load_net_worth_series(&db, Scope::Character(42), &[42], &[])
          .await
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_maps_a_characters_recent_snapshots() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let today = Utc::now().format("%Y-%m-%d").to_string();
      crate::store::repo::finance::upsert(&db, 42, &today, 100.0, None, None, 175.0)
        .await
        .unwrap();

      let series = super::load_net_worth_series(&db, Scope::Character(42), &[42], &[]).await;

      assert_eq!(series.len(), 1);
      assert_eq!(series[0].net_worth, 175.0);
    }

    #[tokio::test]
    async fn it_maps_a_corporations_recent_snapshots() {
      let db = crate::store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      let today = Utc::now().format("%Y-%m-%d").to_string();
      sqlx::query(
        "INSERT INTO corporation_net_worth_snapshot (corporation_id, date, liquid, net_worth) VALUES (?, ?, ?, ?)",
      )
      .bind(90_000_001_i64)
      .bind(&today)
      .bind(1_250.0)
      .bind(1_250.0)
      .execute(&db.0)
      .await
      .unwrap();

      let series = super::load_net_worth_series(&db, Scope::Corporation(90_000_001), &[], &[]).await;

      assert_eq!(series.len(), 1);
      assert_eq!(series[0].net_worth, 1_250.0);
    }
  }

  mod load_roster {
    #[tokio::test]
    async fn it_yields_an_empty_roster_against_a_bare_store() {
      let db = crate::store::open_test().await.unwrap();

      assert!(super::super::load_roster(&db).await.is_empty());
    }
  }

  mod loads_data {
    use super::*;

    #[test]
    fn it_does_not_flag_an_interaction_message() {
      assert!(!Message::TabSelected(Tab::Market).loads_data());
      assert!(!Message::SearchChanged("rifter".to_owned()).loads_data());
      assert!(!Message::ChartHovered(Some(0.5)).loads_data());
    }

    #[test]
    fn it_flags_a_load_message_for_an_image_recheck() {
      assert!(Message::Loaded(Box::new(load_wallet_for_test())).loads_data());
    }
  }

  mod mark_dirty {
    use super::*;

    #[test]
    fn it_ignores_a_kind_the_wallet_does_not_render() {
      let mut state = State::new();

      state.mark_dirty(JobKind::AssetSync);

      assert!(!state.is_dirty());
    }

    #[test]
    fn it_marks_the_wallet_dirty_for_a_ledger_kind() {
      let mut state = State::new();

      state.mark_dirty(JobKind::CharacterWallet);

      assert!(state.is_dirty());
    }
  }

  mod market_matches {
    use super::*;

    #[test]
    fn it_filters_by_side() {
      let buy = market_entry(1, true, "Tritanium", "Jita");
      let sell = market_entry(1, false, "Tritanium", "Jita");

      assert!(super::market_matches(&buy, SignFilter::All, Side::Buy, ""));
      assert!(!super::market_matches(&sell, SignFilter::All, Side::Buy, ""));
      assert!(super::market_matches(&sell, SignFilter::All, Side::Sell, ""));
      assert!(!super::market_matches(&buy, SignFilter::All, Side::Sell, ""));
      assert!(super::market_matches(&buy, SignFilter::All, Side::All, ""));
      assert!(super::market_matches(&sell, SignFilter::All, Side::All, ""));
    }

    #[test]
    fn it_keeps_sells_for_in_and_buys_for_out() {
      let buy = market_entry(1, true, "Tritanium", "Jita");
      let sell = market_entry(1, false, "Tritanium", "Jita");

      assert!(super::market_matches(&sell, SignFilter::In, Side::All, ""));
      assert!(!super::market_matches(&buy, SignFilter::In, Side::All, ""));
      assert!(super::market_matches(&buy, SignFilter::Out, Side::All, ""));
      assert!(!super::market_matches(&sell, SignFilter::Out, Side::All, ""));
    }

    #[test]
    fn it_matches_the_query_against_item_and_location() {
      let entry = market_entry(1, false, "Tritanium", "Jita IV - Moon 4");

      assert!(super::market_matches(&entry, SignFilter::All, Side::All, "trit"));
      assert!(super::market_matches(&entry, SignFilter::All, Side::All, "jita"));
      assert!(!super::market_matches(&entry, SignFilter::All, Side::All, "veldspar"));
    }
  }

  mod period_totals {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_zero_when_no_in_scope_character_has_period_rows() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.periods = vec![period(9, 100.0, 40.0)];
      state.active = Scope::All;

      let totals = super::period_totals(&state);

      assert_eq!(totals, PeriodTotals::default());
    }

    #[test]
    fn it_reflects_only_the_selected_character_in_single_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.periods = vec![period(1, 100.0, 40.0), period(2, 200.0, 80.0)];
      state.active = Scope::Character(1);

      let totals = super::period_totals(&state);

      assert_eq!(totals.income, 100.0);
      assert_eq!(totals.spend, 40.0);
      assert_eq!(totals.net, 60.0);
    }

    #[test]
    fn it_sums_income_and_spend_across_the_in_scope_characters() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.periods = vec![period(1, 100.0, 40.0), period(1, 50.0, 10.0), period(2, 200.0, 80.0)];
      state.active = Scope::All;

      let totals = super::period_totals(&state);

      assert_eq!(totals.income, 350.0);
      assert_eq!(totals.spend, 130.0);
      assert_eq!(totals.net, 220.0);
    }
  }

  mod reload_kind {
    use super::*;

    #[test]
    fn it_feeds_the_wallet_for_every_ledger_and_derive_kind() {
      assert!(reload_kind(JobKind::CharacterWallet));
      assert!(reload_kind(JobKind::CorporationWallet));
      assert!(reload_kind(JobKind::MarketPrices));
      assert!(reload_kind(JobKind::NetWorthSnapshot));
    }

    #[test]
    fn it_ignores_kinds_the_wallet_does_not_render() {
      assert!(!reload_kind(JobKind::AssetSync));
      assert!(!reload_kind(JobKind::CharacterSkills));
      assert!(!reload_kind(JobKind::CharacterProfile));
    }
  }

  mod resolve_scope_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_every_pilot_id_for_all_scope() {
      let roster = [pilot(1, None), pilot(2, None)];

      assert_eq!(super::resolve_scope_ids(Scope::All, &roster, &[]), vec![1, 2]);
    }

    #[test]
    fn it_returns_no_character_ids_for_a_corp_scope() {
      let roster = [pilot(1, None)];

      assert!(super::resolve_scope_ids(Scope::Corporation(98_000_001), &roster, &[]).is_empty());
    }

    #[test]
    fn it_returns_the_single_id_for_a_character_scope() {
      let roster = [pilot(1, None), pilot(2, None)];

      assert_eq!(super::resolve_scope_ids(Scope::Character(2), &roster, &[]), vec![2]);
    }
  }

  mod scope_composition {
    use pretty_assertions::assert_eq;

    use super::*;

    fn financials_full(character_id: i64, liquid: f64, assets: f64, escrow: f64) -> CharacterFinancials {
      CharacterFinancials {
        character_id,
        liquid: Some(liquid),
        asset_value: Some(assets),
        escrow: Some(escrow),
        net_worth: Some(liquid + assets + escrow),
      }
    }

    #[test]
    fn it_is_none_per_figure_when_no_in_scope_character_has_it() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials(1, None)];
      state.active = Scope::All;

      let composition = super::scope_composition(&state);

      assert_eq!(composition.liquid, None);
      assert_eq!(composition.asset_value, None);
      assert_eq!(composition.escrow, None);
    }

    #[test]
    fn it_sums_each_figure_across_the_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.financials = vec![
        financials_full(1, 100.0, 50.0, 10.0),
        financials_full(2, 200.0, 25.0, 5.0),
      ];
      state.active = Scope::All;

      let composition = super::scope_composition(&state);

      assert_eq!(composition.liquid, Some(300.0));
      assert_eq!(composition.asset_value, Some(75.0));
      assert_eq!(composition.escrow, Some(15.0));
    }
  }

  mod scope_gate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_does_not_gate_a_character_with_the_wallet_scopes() {
      let granted = crate::features::registry::descriptor(crate::config::Feature::Wallet)
        .scopes
        .join(" ");
      let mut granted_pilot = pilot(1, None);
      granted_pilot.granted_scopes = Some(granted);
      let mut state = State::new();
      state.roster = vec![granted_pilot];
      state.active = Scope::Character(1);

      assert!(state.scope_gate().is_none());
    }

    #[test]
    fn it_does_not_gate_the_all_or_corporation_scopes() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;

      assert!(state.scope_gate().is_none());

      state.active = Scope::Corporation(99);

      assert!(state.scope_gate().is_none());
    }

    #[test]
    fn it_gates_a_character_scope_missing_the_wallet_scopes() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::Character(1);

      let gate = state.scope_gate().expect("missing scope should gate");

      assert_eq!(gate.0, 1);
      assert!(!gate.2.is_empty());
    }
  }

  mod scope_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_every_pilot_for_all_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.active = Scope::All;

      assert_eq!(state.scope_ids(), vec![1, 2]);
    }

    #[test]
    fn it_returns_no_character_ids_for_a_corp_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.active = Scope::Corporation(98_000_001);

      assert!(state.scope_ids().is_empty());
    }

    #[test]
    fn it_returns_the_single_id_for_a_character_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(2, None)];
      state.active = Scope::Character(2);

      assert_eq!(state.scope_ids(), vec![2]);
    }
  }

  mod scope_liquid {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_owned_corporation_balances_under_the_all_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];
      state.corporations = vec![corp_with_liquid(98_000_001, Some(700.0))];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), Some(800.0));
    }

    #[test]
    fn it_excludes_corporation_balances_under_a_character_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];
      state.corporations = vec![corp_with_liquid(98_000_001, Some(700.0))];
      state.active = Scope::Character(1);

      assert_eq!(super::scope_liquid(&state), Some(100.0));
    }

    #[test]
    fn it_excludes_out_of_scope_characters() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.active = Scope::Character(1);

      assert_eq!(super::scope_liquid(&state), Some(100.0));
    }

    #[test]
    fn it_includes_corporation_balances_even_when_no_character_has_liquid() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials(1, None)];
      state.corporations = vec![corp_with_liquid(98_000_001, Some(250.0))];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), Some(250.0));
    }

    #[test]
    fn it_returns_none_when_no_in_scope_character_has_a_synced_balance() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.financials = vec![financials(1, None)];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), None);
    }

    #[test]
    fn it_sums_liquid_across_the_in_scope_characters() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.active = Scope::All;

      assert_eq!(super::scope_liquid(&state), Some(150.0));
    }

    #[test]
    fn it_uses_summed_division_balances_for_a_corp_scope() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![
        corp_division(1, Some("Master"), Some(500.0)),
        corp_division(2, None, Some(200.0)),
      ];

      assert_eq!(super::scope_liquid(&state), Some(700.0));
    }
  }

  mod series_change {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_last_minus_first() {
      let series = [nw_point("a", 100.0), nw_point("b", 130.0), nw_point("c", 160.0)];

      assert_eq!(super::series_change(&series), 60.0);
    }

    #[test]
    fn it_is_negative_when_net_worth_falls() {
      let series = [nw_point("a", 200.0), nw_point("b", 150.0)];

      assert_eq!(super::series_change(&series), -50.0);
    }

    #[test]
    fn it_is_zero_for_a_single_point_or_empty_series() {
      assert_eq!(super::series_change(&[nw_point("a", 5.0)]), 0.0);
      assert_eq!(super::series_change(&[]), 0.0);
    }
  }

  mod sliced_series {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    use super::*;

    fn day(date: &str) -> NaiveDate {
      NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn it_does_not_widen_the_window_when_points_are_sparse() {
      let mut state = State::new();
      state.net_worth_series = vec![
        nw_point("2026-01-01", 1.0),
        nw_point("2026-05-30", 2.0),
        nw_point("2026-06-09", 3.0),
      ];
      state.timeframe = Timeframe::Week;

      let sliced = super::sliced_series(&state, day("2026-06-09"));

      assert_eq!(sliced.len(), 1);
      assert_eq!(sliced[0].net_worth, 3.0);
    }

    #[test]
    fn it_keeps_only_points_within_the_timeframe_window() {
      let mut state = State::new();
      state.net_worth_series = (1..=20)
        .map(|d| nw_point(&format!("2026-06-{d:02}"), d as f64))
        .collect();
      state.timeframe = Timeframe::Week;

      let sliced = super::sliced_series(&state, day("2026-06-20"));

      assert_eq!(sliced.len(), 7);
      assert_eq!(sliced[0].net_worth, 14.0);
      assert_eq!(sliced[6].net_worth, 20.0);
    }

    #[test]
    fn it_returns_the_whole_series_when_it_fits_inside_the_window() {
      let mut state = State::new();
      state.net_worth_series = vec![nw_point("2026-06-01", 1.0), nw_point("2026-06-02", 2.0)];
      state.timeframe = Timeframe::Year;

      assert_eq!(super::sliced_series(&state, day("2026-06-02")).len(), 2);
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fresh_portrait() -> images::ImageState {
      images::ImageState::Fresh(std::path::PathBuf::from("/cache/characters/1.jpg"))
    }

    #[test]
    fn it_collects_the_stale_portrait_and_logo_keys() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.corporations = vec![corp(98_000_001, "Corp")];

      let keys = state.stale_images();

      assert!(keys.contains(&(images::ImageKind::CharacterPortrait, 1)));
      assert!(keys.contains(&(images::ImageKind::CorporationLogo, 98_000_001)));
    }

    #[test]
    fn it_deduplicates_repeated_keys() {
      let mut state = State::new();
      state.roster = vec![pilot(1, None), pilot(1, None)];

      assert_eq!(state.stale_images(), vec![(images::ImageKind::CharacterPortrait, 1)]);
    }

    #[test]
    fn it_is_empty_when_every_model_image_is_fresh() {
      let mut state = State::new();
      state.roster = vec![RosterPilot {
        corp: "TST".to_owned(),
        granted_scopes: None,
        id: 1,
        liquid: None,
        name: "Pilot 1".to_owned(),
        portrait: fresh_portrait(),
      }];
      state.corporations = vec![RosterCorp {
        id: 98_000_001,
        liquid: None,
        logo: images::ImageState::Fresh(std::path::PathBuf::from("/cache/corporations/98000001.png")),
        name: "Corp".to_owned(),
        ticker: "TSTC".to_owned(),
      }];

      assert!(state.stale_images().is_empty());
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_appends_a_more_page_matching_the_active_scope_and_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.loading_more = true;
      let (tab, scope) = (state.tab, state.active);

      let _ = update(
        &mut state,
        Message::MoreLoaded(Box::new(MorePage {
          contracts: vec![],
          journal: vec![journal_entry(7, Some(5.0), "bounty", "kill")],
          market: vec![],
          tab,
          scope,
        })),
        &db,
      );

      assert_eq!(state.journal.len(), 1);
      assert!(!state.loading_more);
      assert!(!state.tab_exhausted);
    }

    #[tokio::test]
    async fn it_does_not_load_a_page_for_a_shallow_scroll() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;

      let _ = update(
        &mut state,
        Message::TabScrolled {
          absolute: 100.0,
          relative: 0.2,
        },
        &db,
      );

      assert!(!state.loading_more, "a shallow scroll does not page");
    }

    #[tokio::test]
    async fn it_drops_a_more_page_for_a_stale_scope_or_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.loading_more = true;
      let tab = state.tab;

      let _ = update(
        &mut state,
        Message::MoreLoaded(Box::new(MorePage {
          contracts: vec![],
          journal: vec![journal_entry(7, Some(5.0), "bounty", "kill")],
          market: vec![],
          tab,
          scope: Scope::Character(99),
        })),
        &db,
      );

      assert!(state.journal.is_empty());
      assert!(!state.loading_more);
    }

    #[tokio::test]
    async fn it_ignores_a_division_selection_outside_corp_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::All;

      let _ = update(&mut state, Message::DivisionSelected(3), &db);

      assert_eq!(state.active_division, DEFAULT_DIVISION);
    }

    #[tokio::test]
    async fn it_loads_the_next_page_when_scrolled_near_the_bottom() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.roster = vec![pilot(1, None)];
      state.active = Scope::All;
      state.tab = Tab::Journal;

      let _ = update(
        &mut state,
        Message::TabScrolled {
          absolute: 9_000.0,
          relative: 0.9,
        },
        &db,
      );

      assert!(state.loading_more, "a deep scroll starts the next cursor page");
    }

    #[tokio::test]
    async fn it_marks_the_tab_exhausted_when_a_more_page_is_empty() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.loading_more = true;
      let (tab, scope) = (state.tab, state.active);

      let _ = update(
        &mut state,
        Message::MoreLoaded(Box::new(MorePage {
          contracts: vec![],
          journal: vec![],
          market: vec![],
          tab,
          scope,
        })),
        &db,
      );

      assert!(state.tab_exhausted);
    }

    #[tokio::test]
    async fn it_no_ops_on_a_settled_pane() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PaneSettled(RIGHT_RAIL_PANE_KEY, 320.0), &db);

      assert!(!state.right_rail.is_active());
    }

    #[tokio::test]
    async fn it_no_ops_when_the_selected_scope_is_already_active() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.picker_open = true;
      let active = state.active;

      let _ = update(&mut state, Message::ScopeSelected(active), &db);

      assert!(!state.picker_open);
      assert_eq!(state.active, active);
    }

    #[tokio::test]
    async fn it_opens_and_closes_the_contract_detail_modal() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::ContractDetailLoaded(Box::new(Some(contract_detail_fixture()))),
        &db,
      );
      assert!(state.selected_contract.is_some());

      let _ = update(&mut state, Message::CloseContractDetail, &db);
      assert!(state.selected_contract.is_none());
    }

    #[tokio::test]
    async fn it_records_the_chart_hover_fraction() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::ChartHovered(Some(0.25)), &db);

      assert_eq!(state.chart_hover, Some(0.25));
    }

    #[tokio::test]
    async fn it_records_the_loaded_roster_and_ledgers() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::Loaded(Box::new(Loaded {
          chips: loaders::BudgetChips::default(),
          contract_total: 0,
          contracts: vec![],
          corp_divisions: vec![],
          corporations: vec![],
          financials: vec![financials(7, Some(10.0))],
          journal: vec![],
          journal_total: 0,
          market: vec![],
          market_total: 0,
          net_worth_series: vec![],
          periods: vec![],
          right_rail_width: 280.0,
          roster: vec![pilot(7, Some(10.0))],
          wallet_sections: vec![],
        })),
        &db,
      );

      assert_eq!(state.roster, vec![pilot(7, Some(10.0))]);
      assert_eq!(state.financials.len(), 1);
    }

    #[tokio::test]
    async fn it_records_the_search_and_sign_filter() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::SearchChanged("tritanium".to_owned()), &db);
      assert_eq!(state.search, "tritanium");

      let _ = update(&mut state, Message::SignFilterChanged(SignFilter::In), &db);
      assert_eq!(state.sign_filter, SignFilter::In);
    }

    #[tokio::test]
    async fn it_records_the_selected_division_in_corp_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::Corporation(98_000_001);

      let _ = update(&mut state, Message::DivisionSelected(3), &db);

      assert_eq!(state.active_division, 3);
    }

    #[tokio::test]
    async fn it_records_the_selected_scope_and_closes_the_picker() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.picker_open = true;

      let _ = update(&mut state, Message::ScopeSelected(Scope::Character(42)), &db);

      assert_eq!(state.active, Scope::Character(42));
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_records_the_side_filter() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::SideFilterChanged(Side::Buy), &db);

      assert_eq!(state.side_filter, Side::Buy);
      assert_eq!(state.tab_scroll_offset(), 0.0);
    }

    #[tokio::test]
    async fn it_resets_the_scroll_offset_when_the_tab_changes() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.tab_scroll_offset = 4_200.0;

      let _ = update(&mut state, Message::TabSelected(Tab::Journal), &db);
      assert_eq!(state.tab_scroll_offset(), 0.0);
    }

    #[tokio::test]
    async fn it_resizes_the_right_rail_through_a_drag() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let start = state.right_rail.width();

      let _ = update(&mut state, Message::RailDragStart, &db);
      let _ = update(&mut state, Message::RailDragged(500.0), &db);
      let _ = update(&mut state, Message::RailDragged(540.0), &db);
      let _ = update(&mut state, Message::RailDragEnd, &db);

      assert_eq!(state.right_rail.width(), start - 40.0);
    }

    #[tokio::test]
    async fn it_resizes_the_budget_inspector_through_a_drag() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let start = state.budget_inspector_width();

      let _ = update(&mut state, Message::BudgetInspectorDragStart, &db);
      let _ = update(&mut state, Message::BudgetInspectorDragged(500.0), &db);
      let _ = update(&mut state, Message::BudgetInspectorDragged(540.0), &db);
      let _ = update(&mut state, Message::BudgetInspectorDragEnd, &db);

      assert_eq!(state.budget_inspector_width(), start - 40.0);
    }

    #[tokio::test]
    async fn it_selects_a_timeframe_and_clears_the_chart_hover() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.chart_hover = Some(0.5);

      let _ = update(&mut state, Message::TimeframeSelected(Timeframe::Year), &db);

      assert_eq!(state.timeframe, Timeframe::Year);
      assert_eq!(state.chart_hover, None);
    }

    #[tokio::test]
    async fn it_switches_the_active_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::TabSelected(Tab::Journal), &db);
      assert_eq!(state.tab, Tab::Journal);
    }

    #[tokio::test]
    async fn it_toggles_the_picker_open_and_closed() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(state.picker_open);

      let _ = update(&mut state, Message::PickerToggled, &db);
      assert!(!state.picker_open);
    }

    #[tokio::test]
    async fn it_tracks_the_absolute_scroll_offset_for_windowing() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(
        &mut state,
        Message::TabScrolled {
          absolute: 1_234.0,
          relative: 0.5,
        },
        &db,
      );

      assert_eq!(
        state.tab_scroll_offset(),
        1_234.0,
        "the pixel offset is stored so the virtual list can window the ledger"
      );
    }

    #[tokio::test]
    async fn selecting_a_contract_row_leaves_the_modal_closed_until_the_load_resolves() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.contracts = vec![contract_entry(7, false, "finished", "item_exchange")];

      let _ = update(&mut state, Message::ContractSelected(12_345), &db);

      assert!(state.selected_contract.is_none());
    }

    #[tokio::test]
    async fn selecting_a_corp_scope_resets_the_active_division_to_the_master() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.active = Scope::Corporation(1);
      state.active_division = 5;
      state.corp_divisions = vec![corp_division(5, None, Some(1.0))];

      let _ = update(&mut state, Message::ScopeSelected(Scope::Corporation(98_000_001)), &db);

      assert_eq!(state.active, Scope::Corporation(98_000_001));
      assert_eq!(state.active_division, DEFAULT_DIVISION);
      assert!(state.corp_divisions.is_empty());
    }

    #[tokio::test]
    async fn selecting_an_unknown_contract_row_is_a_no_op() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();

      let _ = update(&mut state, Message::ContractSelected(999), &db);

      assert!(state.selected_contract.is_none());
    }
  }

  mod budget_handlers {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::wallet::budget;

    fn category(id: i64) -> budget::Category {
      budget::Category {
        activity: -50.0,
        assigned: 400.0,
        avg_assigned: 100.0,
        carry: 200.0,
        id,
        last_assigned: 120.0,
        name: format!("Category {id}"),
        note: Some("note".to_owned()),
        spent_last: 80.0,
        target: budget::Target {
          amount: 1_000.0,
          by_date: None,
          kind: budget::TargetKind::Monthly,
        },
        tone: Some("plasma".to_owned()),
      }
    }

    fn state_with_view() -> State {
      let mut state = State::new();
      state.tab = Tab::Budget;
      state.budget = Some(budget::BudgetView {
        groups: vec![budget::Group {
          categories: vec![category(1)],
          id: 10,
          name: "Bills".to_owned(),
        }],
        month: budget::current_month(),
        overspent: 0.0,
        pool: 5_000.0,
        ready_to_assign: 1_500.0,
      });
      state.budget_selected = Some(1);
      state
    }

    #[tokio::test]
    async fn it_toggles_the_plan_reflect_mode_and_edit_mode() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(&mut state, Message::BudgetModeSelected(budget::Mode::Reflect), &db);
      assert_eq!(state.budget_mode(), budget::Mode::Reflect);

      let _ = update(&mut state, Message::BudgetEditToggled, &db);
      assert!(state.budget_edit_mode());
    }

    #[tokio::test]
    async fn it_selects_the_reflect_flow_range() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(
        &mut state,
        Message::BudgetRangeSelected(budget::BudgetRange::ThreeMonths),
        &db,
      );

      assert_eq!(state.budget_range(), budget::BudgetRange::ThreeMonths);
    }

    #[tokio::test]
    async fn it_collapses_and_expands_a_group() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(&mut state, Message::BudgetGroupToggled(10), &db);
      assert!(state.budget_collapsed(10));

      let _ = update(&mut state, Message::BudgetGroupToggled(10), &db);
      assert!(!state.budget_collapsed(10));
    }

    #[tokio::test]
    async fn it_opens_an_assigned_editor_seeded_with_the_current_value() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(&mut state, Message::BudgetAssignEditBegan(1), &db);

      let editing = state.budget_editing().expect("editor open");
      assert_eq!(editing.category_id, 1);
      assert_eq!(editing.draft, crate::ui::format::fmt_isk(400.0));
    }

    #[tokio::test]
    async fn it_does_not_open_an_assigned_editor_for_a_past_month() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      state.budget_month = budget::shift_month(&budget::current_month(), -2);

      let _ = update(&mut state, Message::BudgetAssignEditBegan(1), &db);

      assert!(state.budget_editing().is_none());
    }

    #[tokio::test]
    async fn it_edits_and_cancels_the_assigned_draft() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetAssignEditBegan(1), &db);

      let _ = update(&mut state, Message::BudgetAssignDraftChanged("2.5m".to_owned()), &db);
      assert_eq!(state.budget_editing().unwrap().draft, "2.5m");

      let _ = update(&mut state, Message::BudgetAssignCancelled, &db);
      assert!(state.budget_editing().is_none());
    }

    #[tokio::test]
    async fn it_opens_the_inspector_editor_for_the_selection() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(&mut state, Message::BudgetEditorToggled, &db);
      let editor = state.budget_editor().expect("editor open");
      assert_eq!(editor.category_id, 1);
      assert_eq!(editor.name, "Category 1");

      let _ = update(&mut state, Message::BudgetEditorNameChanged("Renamed".to_owned()), &db);
      assert_eq!(state.budget_editor().unwrap().name, "Renamed");

      let _ = update(&mut state, Message::BudgetEditorToggled, &db);
      assert!(state.budget_editor().is_none());
    }

    #[tokio::test]
    async fn it_updates_the_editor_target_fields() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetEditorToggled, &db);

      let _ = update(
        &mut state,
        Message::BudgetEditorKindSelected(budget::TargetKind::GoalBy),
        &db,
      );
      let _ = update(&mut state, Message::BudgetEditorAmountChanged("220b".to_owned()), &db);
      let _ = update(
        &mut state,
        Message::BudgetEditorByDateChanged("Jan 2028".to_owned()),
        &db,
      );
      let _ = update(
        &mut state,
        Message::BudgetEditorNoteChanged("hull fund".to_owned()),
        &db,
      );
      let _ = update(&mut state, Message::BudgetEditorToneSelected("warning".to_owned()), &db);

      let editor = state.budget_editor().unwrap();
      assert_eq!(editor.target_kind, budget::TargetKind::GoalBy);
      assert_eq!(editor.target_amount, 220_000_000_000.0);
      assert_eq!(editor.by_date, "Jan 2028");
      assert_eq!(editor.note, "hull fund");
      assert_eq!(editor.tone.as_deref(), Some("warning"));
    }

    #[tokio::test]
    async fn it_selects_a_category_and_clears_the_editor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetEditorToggled, &db);

      let _ = update(&mut state, Message::BudgetCategorySelected(1), &db);

      assert_eq!(state.budget_selected(), Some(1));
      assert!(state.budget_editor().is_none());
    }

    #[tokio::test]
    async fn it_steps_the_month_and_drops_any_open_editor() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetAssignEditBegan(1), &db);

      let _ = update(&mut state, Message::BudgetMonthStepped(-1), &db);

      assert_eq!(state.budget_month(), budget::shift_month(&budget::current_month(), -1));
      assert!(state.budget_editing().is_none());
    }

    #[tokio::test]
    async fn it_records_a_loaded_view_for_the_active_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      state.tab = Tab::Budget;
      let load = BudgetLoad {
        history: Vec::new(),
        scope: state.active,
        select: None,
        view: state_with_view().budget.unwrap(),
      };

      let _ = update(&mut state, Message::BudgetLoaded(Box::new(load)), &db);

      assert!(state.budget().is_some());
      assert_eq!(state.budget_selected(), Some(1));
    }

    #[tokio::test]
    async fn it_ignores_a_loaded_view_for_a_stale_scope() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = State::new();
      let load = BudgetLoad {
        history: Vec::new(),
        scope: Scope::Character(999),
        select: None,
        view: state_with_view().budget.unwrap(),
      };

      let _ = update(&mut state, Message::BudgetLoaded(Box::new(load)), &db);

      assert!(state.budget().is_none());
    }

    #[tokio::test]
    async fn it_dispatches_the_persist_and_mutation_messages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      // Each of these returns a persist+reload task; dispatching exercises the
      // synchronous handler path. The editor edits drop the inline assign editor.
      let _ = update(&mut state, Message::BudgetAssignEditBegan(1), &db);
      let _ = update(&mut state, Message::BudgetAssignCommitted, &db);
      assert!(state.budget_editing().is_none());

      let _ = update(&mut state, Message::BudgetQuickAssign(1, 250.0), &db);
      let _ = update(&mut state, Message::BudgetAutoAssign, &db);
      let _ = update(&mut state, Message::BudgetCoverOverspending, &db);
      let _ = update(&mut state, Message::BudgetEditorCommitted, &db);
    }

    #[tokio::test]
    async fn it_opens_the_move_popover_prefilled_with_the_available_amount() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(&mut state, Message::BudgetMoveOpened(1, BudgetMoveAnchor::Pill), &db);

      let open = state.budget_move().expect("move popover open");
      assert_eq!(open.from_id, 1);
      // available = carry 200 + assigned 400 + activity −50 = 550.
      assert_eq!(open.amount_draft, crate::ui::format::fmt_isk(550.0));
      assert_eq!(state.budget_selected(), Some(1));
    }

    #[tokio::test]
    async fn it_blocks_opening_the_move_popover_in_a_past_month() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      state.budget_month = budget::shift_month(&budget::current_month(), -1);

      let _ = update(&mut state, Message::BudgetMoveOpened(1, BudgetMoveAnchor::Pill), &db);

      assert!(state.budget_move().is_none());
    }

    #[tokio::test]
    async fn it_closes_the_move_popover() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetMoveOpened(1, BudgetMoveAnchor::Pill), &db);

      let _ = update(&mut state, Message::BudgetMoveClosed, &db);

      assert!(state.budget_move().is_none());
    }

    #[tokio::test]
    async fn it_clears_the_move_popover_on_commit() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetMoveOpened(1, BudgetMoveAnchor::Pill), &db);

      // Dispatching the commit runs the synchronous handler path (state mutation
      // + persist task build) without executing the reload task.
      let _ = update(
        &mut state,
        Message::BudgetMoveCommitted(budget::MoveDest::ReadyToAssign),
        &db,
      );

      assert!(state.budget_move().is_none());
    }

    fn state_with_two_categories() -> State {
      let mut state = state_with_view();
      state.budget = Some(budget::BudgetView {
        groups: vec![
          budget::Group {
            categories: vec![category(1), category(2)],
            id: 10,
            name: "Bills".to_owned(),
          },
          budget::Group {
            categories: vec![category(3)],
            id: 20,
            name: "Wants".to_owned(),
          },
        ],
        month: budget::current_month(),
        overspent: 0.0,
        pool: 5_000.0,
        ready_to_assign: 1_500.0,
      });
      state.budget_edit_mode = true;
      state
    }

    #[tokio::test]
    async fn it_arms_and_clears_a_drag() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();

      let _ = update(&mut state, Message::BudgetDragStarted(2), &db);
      assert_eq!(state.budget_dragging, Some(2));

      let _ = update(
        &mut state,
        Message::BudgetDropTargetEntered(BudgetDropTarget::Category(1)),
        &db,
      );
      assert_eq!(state.budget_drop_target, Some(BudgetDropTarget::Category(1)));
    }

    #[tokio::test]
    async fn it_reorders_the_in_memory_view_on_drop() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      let _ = update(&mut state, Message::BudgetDragStarted(2), &db);
      let _ = update(
        &mut state,
        Message::BudgetDropTargetEntered(BudgetDropTarget::Group(20)),
        &db,
      );

      let _ = update(&mut state, Message::BudgetDropReleased, &db);

      let view = state.budget().unwrap();
      assert_eq!(view.groups[0].categories.iter().map(|c| c.id).collect::<Vec<_>>(), [1]);
      assert_eq!(
        view.groups[1].categories.iter().map(|c| c.id).collect::<Vec<_>>(),
        [3, 2]
      );
      assert!(state.budget_dragging.is_none());
      assert!(state.budget_drop_target.is_none());
    }

    #[tokio::test]
    async fn it_drops_without_a_target_as_a_no_op() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      let _ = update(&mut state, Message::BudgetDragStarted(2), &db);

      let _ = update(&mut state, Message::BudgetDropReleased, &db);

      let view = state.budget().unwrap();
      assert_eq!(
        view.groups[0].categories.iter().map(|c| c.id).collect::<Vec<_>>(),
        [1, 2]
      );
    }

    #[tokio::test]
    async fn it_keeps_the_drop_target_when_the_cursor_leaves_during_a_drag() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      let _ = update(&mut state, Message::BudgetDragStarted(2), &db);
      let _ = update(
        &mut state,
        Message::BudgetDropTargetEntered(BudgetDropTarget::Category(1)),
        &db,
      );

      // Leaving the row mid-drag must not drop the target the release will read.
      let _ = update(&mut state, Message::BudgetDropTargetLeft, &db);

      assert_eq!(state.budget_drop_target, Some(BudgetDropTarget::Category(1)));
    }

    #[tokio::test]
    async fn it_reorders_groups_on_a_group_drop() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      let _ = update(&mut state, Message::BudgetGroupDragStarted(20), &db);
      assert_eq!(state.budget_group_dragging, Some(20));

      let _ = update(
        &mut state,
        Message::BudgetDropTargetEntered(BudgetDropTarget::Group(10)),
        &db,
      );
      assert_eq!(state.budget_group_drop_target, Some(10));

      let _ = update(&mut state, Message::BudgetDropReleased, &db);

      let view = state.budget().unwrap();
      assert_eq!(view.groups.iter().map(|g| g.id).collect::<Vec<_>>(), [20, 10]);
      assert!(state.budget_group_dragging.is_none());
      assert!(state.budget_group_drop_target.is_none());
    }

    #[tokio::test]
    async fn it_arms_then_confirms_a_populated_group_delete() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();

      let _ = update(&mut state, Message::BudgetGroupDeleteRequested(10), &db);
      assert_eq!(state.budget_pending_group_delete(), Some(10));
      assert!(state.budget().unwrap().groups.iter().any(|g| g.id == 10));

      let _ = update(&mut state, Message::BudgetGroupDeleteRequested(10), &db);
      assert!(state.budget_pending_group_delete().is_none());
    }

    #[tokio::test]
    async fn it_clears_the_selection_when_its_group_is_deleted() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      state.budget_selected = Some(2);

      let _ = update(&mut state, Message::BudgetGroupDeleteRequested(10), &db);
      let _ = update(&mut state, Message::BudgetGroupDeleteRequested(10), &db);

      assert!(state.budget_selected().is_none());
    }

    #[tokio::test]
    async fn it_renames_a_group_in_memory_immediately() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();

      let _ = update(&mut state, Message::BudgetGroupRenamed(10, "Fixed".to_owned()), &db);

      assert_eq!(state.budget().unwrap().groups[0].name, "Fixed");
    }

    #[tokio::test]
    async fn it_clears_drag_state_when_leaving_edit_mode() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      let _ = update(&mut state, Message::BudgetDragStarted(2), &db);
      state.budget_pending_group_delete = Some(10);

      let _ = update(&mut state, Message::BudgetEditToggled, &db);

      assert!(state.budget_dragging.is_none());
      assert!(state.budget_pending_group_delete().is_none());
    }

    #[tokio::test]
    async fn it_seeds_the_editor_when_selecting_in_edit_mode() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();

      let _ = update(&mut state, Message::BudgetCategorySelected(2), &db);

      let editor = state.budget_editor().expect("editor seeded");
      assert_eq!(editor.category_id, 2);
    }

    #[test]
    fn it_finds_the_end_position_of_a_group() {
      let state = state_with_two_categories();

      assert_eq!(budget_group_end_position(&state, 10), 2);
      assert_eq!(budget_group_end_position(&state, 20), 1);
      assert_eq!(budget_group_end_position(&state, 999), 0);
    }

    #[tokio::test]
    async fn it_dispatches_the_edit_mode_crud_messages() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();

      // Each builds a persist+reload task; dispatching exercises the synchronous
      // handler path (state mutation, position lookup) without executing the task.
      let _ = update(&mut state, Message::BudgetCategoryAdded(10), &db);
      let _ = update(&mut state, Message::BudgetGroupAdded, &db);

      let _ = update(&mut state, Message::BudgetCategoryDeleted(2), &db);
      assert!(state.budget_selected() != Some(2));
    }

    #[tokio::test]
    async fn it_clears_the_selection_when_its_category_is_deleted() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      state.budget_selected = Some(2);

      let _ = update(&mut state, Message::BudgetCategoryDeleted(2), &db);

      assert!(state.budget_selected().is_none());
    }

    #[tokio::test]
    async fn it_deletes_an_empty_group_without_a_confirmation() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_two_categories();
      if let Some(view) = state.budget.as_mut() {
        view.groups[1].categories.clear();
      }

      let _ = update(&mut state, Message::BudgetGroupDeleteRequested(20), &db);

      assert!(state.budget_pending_group_delete().is_none());
    }

    #[tokio::test]
    async fn it_switches_the_inspector_tab() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(
        &mut state,
        Message::BudgetInspectorTabSelected(budget::InspectorTab::Automation),
        &db,
      );

      assert_eq!(state.budget_inspector_tab(), budget::InspectorTab::Automation);
    }

    #[tokio::test]
    async fn it_resets_the_inspector_tab_when_selecting_a_category() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      state.budget_inspector_tab = budget::InspectorTab::Automation;

      let _ = update(&mut state, Message::BudgetCategorySelected(1), &db);

      assert_eq!(state.budget_inspector_tab(), budget::InspectorTab::Detail);
    }

    #[tokio::test]
    async fn it_opens_and_closes_the_rule_editor_for_a_new_rule() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);
      assert_eq!(state.budget_rule_editor().map(|draft| draft.category_id), Some(1));
      assert!(state.budget_rule_editor().is_some_and(|draft| draft.rule_id.is_none()));

      let _ = update(&mut state, Message::BudgetRuleEditorClosed, &db);
      assert!(state.budget_rule_editor().is_none());
    }

    #[tokio::test]
    async fn it_threads_the_search_box_into_the_first_text_condition() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);

      let _ = update(
        &mut state,
        Message::BudgetRuleEditorSearchChanged("Cerberus".to_owned()),
        &db,
      );

      assert_eq!(state.budget_rule_editor().unwrap().search_value(), "Cerberus");
    }

    #[tokio::test]
    async fn it_adds_and_removes_advanced_conditions() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);

      let _ = update(&mut state, Message::BudgetRuleConditionAdded, &db);
      assert_eq!(state.budget_rule_editor().unwrap().conditions.len(), 2);

      let _ = update(&mut state, Message::BudgetRuleConditionRemoved(1), &db);
      assert_eq!(state.budget_rule_editor().unwrap().conditions.len(), 1);
    }

    #[tokio::test]
    async fn it_keeps_at_least_one_condition_when_removing() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);

      let _ = update(&mut state, Message::BudgetRuleConditionRemoved(0), &db);

      assert_eq!(state.budget_rule_editor().unwrap().conditions.len(), 1);
    }

    #[tokio::test]
    async fn it_changes_a_condition_field_and_seeds_its_default_op() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);

      let _ = update(
        &mut state,
        Message::BudgetRuleConditionFieldChanged(0, RuleField::Amount),
        &db,
      );

      let condition = &state.budget_rule_editor().unwrap().conditions[0];
      assert_eq!(condition.field(), RuleField::Amount);
      assert_eq!(condition.op(), RuleOp::GreaterThan);
    }

    #[tokio::test]
    async fn it_tracks_the_open_select_dropdown() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);

      let key = budget::RuleSelectKey::Field(0);
      let _ = update(&mut state, Message::BudgetRuleSelectToggled(Some(key)), &db);
      assert_eq!(state.budget_rule_editor().unwrap().open_select, Some(key));

      let _ = update(&mut state, Message::BudgetRuleSelectToggled(Some(key)), &db);
      assert!(state.budget_rule_editor().unwrap().open_select.is_none());
    }

    #[tokio::test]
    async fn it_persists_a_created_rule_and_closes_the_editor() {
      let db = crate::store::open_test().await.unwrap();
      let scope = crate::store::model::BudgetScope::All;
      let group = crate::store::repo::budget::create_group(
        &db,
        &crate::store::repo::budget::NewGroup {
          name: "Bills".to_owned(),
          position: 0,
          scope,
        },
      )
      .await
      .unwrap();
      let category = crate::store::repo::budget::create_category(
        &db,
        &crate::store::repo::budget::NewCategory {
          group_id: group.id,
          name: "SRP".to_owned(),
          note: None,
          position: 0,
          tone: Some("plasma".to_owned()),
        },
      )
      .await
      .unwrap();

      persist_rule_draft(
        &db,
        scope,
        None,
        category.id,
        true,
        MatchMode::All,
        "Cerberus".to_owned(),
        0,
        vec![crate::store::model::RuleCondition {
          field: RuleField::Text,
          op: RuleOp::Contains,
          value: "Cerberus".to_owned(),
          value2: None,
        }],
      )
      .await;

      let rules = crate::store::repo::budget::list_rules(&db, scope).await.unwrap();
      assert_eq!(rules.len(), 1);
      assert_eq!(rules[0].category_id(), category.id);
      assert_eq!(rules[0].name(), "Cerberus");
      assert_eq!(rules[0].conditions().len(), 1);
    }

    #[tokio::test]
    async fn it_updates_an_existing_rule_in_place() {
      let db = crate::store::open_test().await.unwrap();
      let scope = crate::store::model::BudgetScope::All;
      let group = crate::store::repo::budget::create_group(
        &db,
        &crate::store::repo::budget::NewGroup {
          name: "Bills".to_owned(),
          position: 0,
          scope,
        },
      )
      .await
      .unwrap();
      let category = crate::store::repo::budget::create_category(
        &db,
        &crate::store::repo::budget::NewCategory {
          group_id: group.id,
          name: "SRP".to_owned(),
          note: None,
          position: 0,
          tone: None,
        },
      )
      .await
      .unwrap();
      let created = crate::store::repo::budget::create_rule(
        &db,
        &crate::store::repo::budget::NewRule {
          category_id: category.id,
          enabled: true,
          match_mode: MatchMode::All,
          name: "Old".to_owned(),
          position: 0,
          scope,
        },
      )
      .await
      .unwrap();

      persist_rule_draft(
        &db,
        scope,
        Some(created.id()),
        category.id,
        false,
        MatchMode::Any,
        "New".to_owned(),
        0,
        vec![crate::store::model::RuleCondition {
          field: RuleField::Item,
          op: RuleOp::Contains,
          value: "Missile".to_owned(),
          value2: None,
        }],
      )
      .await;

      let rules = crate::store::repo::budget::list_rules(&db, scope).await.unwrap();
      assert_eq!(rules.len(), 1);
      assert_eq!(rules[0].name(), "New");
      assert!(!rules[0].enabled());
      assert_eq!(rules[0].conditions().len(), 1);
      assert_eq!(rules[0].conditions()[0].value(), "Missile");
    }

    #[tokio::test]
    async fn it_closes_the_editor_on_commit() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);

      let _ = update(&mut state, Message::BudgetRuleEditorCommitted, &db);

      assert!(state.budget_rule_editor().is_none());
    }

    #[tokio::test]
    async fn it_edits_the_open_drafts_metadata() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);

      let _ = update(
        &mut state,
        Message::BudgetRuleEditorNameChanged("My rule".to_owned()),
        &db,
      );
      let _ = update(
        &mut state,
        Message::BudgetRuleEditorMatchModeSelected(MatchMode::Any),
        &db,
      );
      let _ = update(&mut state, Message::BudgetRuleEditorAdvancedToggled, &db);

      let draft = state.budget_rule_editor().unwrap();
      assert_eq!(draft.name, "My rule");
      assert!(draft.name_edited);
      assert_eq!(draft.match_mode, MatchMode::Any);
      assert!(draft.show_advanced);
    }

    #[tokio::test]
    async fn it_edits_a_conditions_values() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();
      let _ = update(&mut state, Message::BudgetRuleNewOpened(1), &db);
      let _ = update(
        &mut state,
        Message::BudgetRuleConditionFieldChanged(0, RuleField::Amount),
        &db,
      );
      let _ = update(
        &mut state,
        Message::BudgetRuleConditionOpChanged(0, RuleOp::Between),
        &db,
      );

      let _ = update(
        &mut state,
        Message::BudgetRuleConditionValueChanged(0, "100m".to_owned()),
        &db,
      );
      let _ = update(
        &mut state,
        Message::BudgetRuleConditionValue2Changed(0, "1b".to_owned()),
        &db,
      );

      let condition = &state.budget_rule_editor().unwrap().conditions[0];
      assert_eq!(condition.op(), RuleOp::Between);
      assert_eq!(condition.value(), "100m");
      assert_eq!(condition.value2.as_deref(), Some("1b"));
    }

    #[tokio::test]
    async fn it_tracks_a_rule_drag_and_drop_target() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      let _ = update(&mut state, Message::BudgetRuleDragStarted(1), &db);
      assert_eq!(state.budget_rule_dragging(), Some(1));

      let _ = update(&mut state, Message::BudgetRuleDropTargetEntered(2), &db);
      assert_eq!(state.budget_rule_drop_target(), Some(2));

      let _ = update(&mut state, Message::BudgetRuleDropTargetLeft, &db);
      // The target only clears once the drag itself ends.
      assert_eq!(state.budget_rule_drop_target(), Some(2));
    }

    #[tokio::test]
    async fn it_ignores_drop_target_changes_without_an_active_drag() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = state_with_view();

      // No drag is in progress, so entering a target is ignored and leaving one
      // clears any stale target.
      let _ = update(&mut state, Message::BudgetRuleDropTargetEntered(2), &db);
      assert!(state.budget_rule_drop_target().is_none());

      let _ = update(&mut state, Message::BudgetRuleDropTargetLeft, &db);
      assert!(state.budget_rule_drop_target().is_none());
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_corp_scope_with_divisions() {
      let mut state = State::new();
      state.corporations = vec![corp(98_000_001, "Test Corp")];
      state.active = Scope::Corporation(98_000_001);
      state.corp_divisions = vec![
        corp_division(1, Some("Master Wallet"), Some(1_000.0)),
        corp_division(2, None, Some(250.0)),
      ];

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_a_corp_scope_with_no_divisions_synced() {
      let mut state = State::new();
      state.corporations = vec![corp(98_000_001, "Test Corp")];
      state.active = Scope::Corporation(98_000_001);

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_a_loaded_state() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0))];
      state.financials = vec![financials(1, Some(100.0))];

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_empty_state_before_any_load() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }

    #[test]
    fn it_renders_the_hero_graph_with_a_net_worth_series() {
      let mut state = State::new();
      state.roster = vec![pilot(1, Some(100.0)), pilot(2, Some(50.0))];
      state.financials = vec![financials(1, Some(100.0)), financials(2, Some(50.0))];
      state.net_worth_series = (0..40).map(|i| nw_point("2026-06-01", 100.0 + i as f64)).collect();
      state.chart_hover = Some(0.5);
      state.timeframe = Timeframe::Month;

      let _el: Element<'_, Message> = view(&state, Utc::now());
    }
  }

  mod load_wallet {
    use super::*;

    #[tokio::test]
    async fn it_loads_an_empty_all_scope_off_a_fresh_db() {
      let db = crate::store::open_test().await.unwrap();

      let loaded = super::super::load_wallet(db, Scope::All, DEFAULT_DIVISION).await;

      assert!(loaded.journal.is_empty());
      assert_eq!(loaded.journal_total, 0);
    }

    #[tokio::test]
    async fn it_loads_a_corporation_scope_off_a_fresh_db() {
      let db = crate::store::open_test().await.unwrap();

      let loaded = super::super::load_wallet(db, Scope::Corporation(98_000_001), DEFAULT_DIVISION).await;

      assert!(loaded.contracts.is_empty());
      assert_eq!(loaded.contract_total, 0);
    }

    #[tokio::test]
    async fn it_loads_a_character_scope_off_a_fresh_db() {
      let db = crate::store::open_test().await.unwrap();

      let loaded = super::super::load_wallet(db, Scope::Character(1), DEFAULT_DIVISION).await;

      assert!(loaded.market.is_empty());
    }
  }

  mod is_escape_pressed {
    #[test]
    fn it_ignores_a_non_escape_event() {
      let event = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left));

      assert!(!super::super::is_escape_pressed(&event));
    }
  }

  mod subscription {
    use super::*;

    #[test]
    fn it_batches_no_listeners_for_an_idle_state() {
      let state = State::new();

      let _sub: iced::Subscription<Message> = super::super::subscription(&state);
    }

    #[test]
    fn it_registers_the_modal_dismiss_listeners() {
      let mut state = State::new();
      state.budget_editing = Some(budget::EditingCell {
        category_id: 1,
        draft: String::new(),
      });
      state.budget_dragging = Some(1);

      let _sub: iced::Subscription<Message> = super::super::subscription(&state);
    }
  }

  mod ledger_selection {
    use super::*;

    fn journal_state() -> State {
      let mut state = State::new();
      state.tab = Tab::Journal;
      let mut first = journal_entry(1, Some(10.0), "bounty_prizes", "first");
      first.id = 1;
      let mut second = journal_entry(1, Some(20.0), "bounty_prizes", "second");
      second.id = 2;
      let mut third = journal_entry(1, Some(30.0), "bounty_prizes", "third");
      third.id = 3;
      state.journal = vec![first, second, third];
      state.recompute_derived();
      state
    }

    #[tokio::test]
    async fn it_selects_a_row_on_a_plain_click() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = journal_state();

      let _ = update(
        &mut state,
        Message::LedgerRowClicked(BudgetEntryKind::Journal, BudgetOwner::Character(1), 2),
        &db,
      );

      assert!(state.journal_selected(BudgetOwner::Character(1), 2));
    }

    #[tokio::test]
    async fn it_extends_the_selection_with_a_shift_click() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = journal_state();
      let _ = update(
        &mut state,
        Message::LedgerRowClicked(BudgetEntryKind::Journal, BudgetOwner::Character(1), 1),
        &db,
      );

      let _ = update(
        &mut state,
        Message::LedgerModifiersChanged(iced::keyboard::Modifiers::SHIFT),
        &db,
      );
      let _ = update(
        &mut state,
        Message::LedgerRowClicked(BudgetEntryKind::Journal, BudgetOwner::Character(1), 3),
        &db,
      );

      assert!(state.journal_selected(BudgetOwner::Character(1), 1));
      assert!(state.journal_selected(BudgetOwner::Character(1), 2));
      assert!(state.journal_selected(BudgetOwner::Character(1), 3));
    }

    #[tokio::test]
    async fn it_opens_the_bulk_menu_on_a_right_press() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = journal_state();
      state.ledger_cursor = Some(iced::Point::new(12.0, 34.0));

      let _ = update(
        &mut state,
        Message::LedgerRowRightPressed(BudgetEntryKind::Journal, BudgetOwner::Character(1), 1),
        &db,
      );

      assert!(state.ledger_menu_open().is_some());
      assert!(state.journal_selected(BudgetOwner::Character(1), 1));
      assert_eq!(state.ledger_selection_count(), 1);
    }

    #[tokio::test]
    async fn it_clears_the_selection_and_menu_after_a_bulk_assign() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = journal_state();
      let _ = update(
        &mut state,
        Message::LedgerRowClicked(BudgetEntryKind::Journal, BudgetOwner::Character(1), 1),
        &db,
      );
      state.ledger_cursor = Some(iced::Point::new(0.0, 0.0));
      let _ = update(
        &mut state,
        Message::LedgerRowRightPressed(BudgetEntryKind::Journal, BudgetOwner::Character(1), 1),
        &db,
      );
      let _ = update(&mut state, Message::LedgerBulkAssignOpened, &db);

      let _ = update(&mut state, Message::LedgerBulkAssignChosen(7), &db);

      assert!(!state.journal_selected(BudgetOwner::Character(1), 1));
      assert!(state.ledger_menu_open().is_none());
    }

    #[tokio::test]
    async fn it_dismisses_the_menu() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = journal_state();
      state.ledger_cursor = Some(iced::Point::new(0.0, 0.0));
      let _ = update(
        &mut state,
        Message::LedgerRowRightPressed(BudgetEntryKind::Journal, BudgetOwner::Character(1), 1),
        &db,
      );

      let _ = update(&mut state, Message::LedgerMenuDismissed, &db);

      assert!(state.ledger_menu_open().is_none());
    }
  }

  mod budget_rule_helpers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_character_name_from_the_roster() {
      let mut state = State::new();
      state.roster = vec![pilot(42, Some(1.0))];

      assert_eq!(budget_character_name(&state, "42"), Some("Pilot 42".to_owned()));
    }

    #[test]
    fn it_falls_back_to_the_corporation_roster_for_a_name() {
      let mut state = State::new();
      state.corporations = vec![corp(98, "Test Corp")];

      assert_eq!(budget_character_name(&state, " 98 "), Some("Test Corp".to_owned()));
    }

    #[test]
    fn it_returns_none_for_an_unknown_or_unparsable_key() {
      let state = State::new();

      assert_eq!(budget_character_name(&state, "not-an-id"), None);
      assert_eq!(budget_character_name(&state, "404"), None);
    }

    #[test]
    fn it_uses_the_users_name_when_edited() {
      let state = State::new();
      let mut draft = budget::RuleDraft::new(1);
      draft.name = "My rule".to_owned();
      draft.name_edited = true;

      assert_eq!(budget_effective_rule_name(&state, &draft), "My rule");
    }

    #[test]
    fn it_falls_back_to_untitled_when_nothing_resolves() {
      let state = State::new();
      let draft = budget::RuleDraft::new(1);

      assert_eq!(budget_effective_rule_name(&state, &draft), "Untitled rule");
    }
  }

  mod strongest_accounting_role {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_picks_director_over_weaker_roles() {
      let role = strongest_accounting_role(|wanted| matches!(wanted, "Director" | "Accountant"));

      assert_eq!(role, Some("Director".to_owned()));
    }

    #[test]
    fn it_humanizes_a_junior_accountant_role() {
      let role = strongest_accounting_role(|wanted| wanted == "Junior_Accountant");

      assert_eq!(role, Some("Junior Accountant".to_owned()));
    }

    #[test]
    fn it_is_none_without_an_accounting_role() {
      let role = strongest_accounting_role(|_| false);

      assert_eq!(role, None);
    }
  }

  mod budget_rule_drop_released {
    use super::*;

    fn rule(id: i64) -> crate::store::model::Rule {
      crate::store::model::Rule {
        category_id: 1,
        conditions: Vec::new(),
        enabled: true,
        id,
        match_mode: MatchMode::All,
        name: format!("rule {id}"),
      }
    }

    fn rules_state() -> State {
      let mut state = State::new();
      state.budget_chips.resolution.rules = vec![rule(1), rule(2), rule(3)];
      state
    }

    #[tokio::test]
    async fn it_clears_the_drag_state_on_a_real_reorder() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = rules_state();
      state.budget_rule_dragging = Some(3);
      state.budget_rule_drop_target = Some(1);

      let _ = super::super::budget_rule_drop_released(&mut state, &db);

      assert!(state.budget_rule_dragging.is_none());
      assert!(state.budget_rule_drop_target.is_none());
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_dropped_on_itself() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = rules_state();
      state.budget_rule_dragging = Some(2);
      state.budget_rule_drop_target = Some(2);

      let _ = super::super::budget_rule_drop_released(&mut state, &db);

      assert!(state.budget_rule_dragging.is_none());
    }
  }
}
