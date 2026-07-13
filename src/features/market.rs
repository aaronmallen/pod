mod book;
mod browse;
mod i18n;
mod my_orders;
mod outbid;
mod shell;
mod tree;
mod watchlist;

use std::collections::HashSet;

use iced::{Element, Task};

use crate::{
  clients::{self, esi, http},
  store::{Database, repo::sde},
};

#[derive(Clone, Debug)]
pub enum Message {
  TabSelected(Tab),
  TreeLoaded(Box<tree::MarketTree>),
  BookLoaded(Box<book::OrderBook>),
  NodeToggled(i64),
  FilterChanged(String),
  ItemSelected(i64),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
  tab: Tab,
  tree: tree::MarketTree,
  book: Option<book::OrderBook>,
  expanded: HashSet<i64>,
  filter: String,
  selected: Option<i64>,
}

impl State {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn active_tab(&self) -> Tab {
    self.tab
  }

  pub fn tree(&self) -> &tree::MarketTree {
    &self.tree
  }

  pub fn filter(&self) -> &str {
    &self.filter
  }

  pub fn is_expanded(&self, id: i64) -> bool {
    self.expanded.contains(&id)
  }

  pub fn selected_type_id(&self) -> Option<i64> {
    self.selected
  }

  #[expect(dead_code, reason = "Read by the Phase 3/4 right-pane order-book render task.")]
  pub fn book(&self) -> Option<&book::OrderBook> {
    self.book.as_ref()
  }

  pub fn select_tab_by_id(&mut self, id: &str) -> bool {
    match Tab::from_id(id) {
      Some(tab) => {
        self.tab = tab;
        true
      }
      None => false,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
  #[default]
  Browse,
  Orders,
  Watchlist,
}

impl Tab {
  pub const ORDER: [Tab; 3] = [Tab::Browse, Tab::Orders, Tab::Watchlist];

  pub fn from_id(id: &str) -> Option<Tab> {
    match id {
      "browse" => Some(Tab::Browse),
      "orders" => Some(Tab::Orders),
      "watchlist" => Some(Tab::Watchlist),
      _ => None,
    }
  }

  pub fn id(self) -> &'static str {
    match self {
      Tab::Browse => "browse",
      Tab::Orders => "orders",
      Tab::Watchlist => "watchlist",
    }
  }
}

pub fn load(db: &Database) -> Task<Message> {
  Task::perform(load_tree(db.clone()), |tree| Message::TreeLoaded(Box::new(tree)))
}

async fn load_tree(db: Database) -> tree::MarketTree {
  let groups = sde::all_market_groups(&db).await.unwrap_or_default();
  let items = sde::all_item_types(&db).await.unwrap_or_default();
  tree::build_market_tree(&groups, &items)
}

#[expect(
  dead_code,
  reason = "Wired for the Phase 3/4 right-pane render; no (region, type) selection exists to drive it yet."
)]
pub fn load_book(db: &Database, region_id: i64, type_id: i64) -> Task<Message> {
  Task::perform(fetch_book(db.clone(), region_id, type_id), |book| {
    Message::BookLoaded(Box::new(book))
  })
}

async fn fetch_book(db: Database, region_id: i64, type_id: i64) -> book::OrderBook {
  let Ok(esi) = public_esi(&db) else {
    return book::OrderBook::default();
  };
  let mut orders = esi.market().sell_orders(region_id, type_id).await.unwrap_or_default();
  orders.extend(esi.market().buy_orders(region_id, type_id).await.unwrap_or_default());
  book::build_order_book(orders)
}

fn public_esi(db: &Database) -> Result<esi::Client, clients::Error> {
  let http = http::Client::builder(http::Cache::new(db.clone())).build();
  esi::Client::builder(http).user_agent(clients::user_agent()).build()
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::TabSelected(tab) => {
      state.tab = tab;
      Task::none()
    }
    Message::TreeLoaded(tree) => {
      state.tree = *tree;
      Task::none()
    }
    Message::BookLoaded(book) => {
      state.book = Some(*book);
      Task::none()
    }
    Message::NodeToggled(id) => {
      if !state.expanded.remove(&id) {
        state.expanded.insert(id);
      }
      Task::none()
    }
    Message::FilterChanged(query) => {
      state.filter = query;
      Task::none()
    }
    Message::ItemSelected(type_id) => {
      state.selected = Some(type_id);
      Task::none()
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  shell::shell(state)
}

pub fn subscription(_state: &State) -> iced::Subscription<Message> {
  iced::Subscription::none()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod tab {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_every_tab_through_its_id() {
      for tab in Tab::ORDER {
        assert_eq!(Tab::from_id(tab.id()), Some(tab));
      }
    }

    #[test]
    fn it_rejects_an_unknown_id() {
      assert_eq!(Tab::from_id("nope"), None);
    }
  }

  mod state {
    use super::*;

    #[test]
    fn it_defaults_to_the_browse_tab() {
      assert_eq!(State::new().active_tab(), Tab::Browse);
    }

    #[test]
    fn it_selects_a_tab_by_id() {
      let mut state = State::new();

      assert!(state.select_tab_by_id("watchlist"));
      assert_eq!(state.active_tab(), Tab::Watchlist);
    }

    #[test]
    fn it_ignores_an_unknown_tab_id() {
      let mut state = State::new();

      assert!(!state.select_tab_by_id("nope"));
      assert_eq!(state.active_tab(), Tab::Browse);
    }
  }

  mod update {
    use super::*;

    #[test]
    fn it_switches_the_active_tab() {
      let mut state = State::new();

      let _ = update(&mut state, Message::TabSelected(Tab::Orders));

      assert_eq!(state.active_tab(), Tab::Orders);
    }

    #[test]
    fn it_toggles_a_node_open_and_closed() {
      let mut state = State::new();

      let _ = update(&mut state, Message::NodeToggled(7));
      assert!(state.is_expanded(7));

      let _ = update(&mut state, Message::NodeToggled(7));
      assert!(!state.is_expanded(7));
    }

    #[test]
    fn it_stores_the_filter_query() {
      let mut state = State::new();

      let _ = update(&mut state, Message::FilterChanged("rifter".to_owned()));

      assert_eq!(state.filter(), "rifter");
    }

    #[test]
    fn it_selects_an_item_by_type_id() {
      let mut state = State::new();

      let _ = update(&mut state, Message::ItemSelected(587));

      assert_eq!(state.selected_type_id(), Some(587));
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_each_tab() {
      for tab in Tab::ORDER {
        let mut state = State::new();
        state.tab = tab;
        let _el: Element<'_, Message> = view(&state);
      }
    }
  }
}
