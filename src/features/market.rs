mod browse;
mod i18n;
mod my_orders;
mod shell;
mod tree;
mod watchlist;

use iced::{Element, Task};

use crate::store::{Database, repo::sde};

#[derive(Clone, Debug)]
pub enum Message {
  TabSelected(Tab),
  TreeLoaded(Box<tree::MarketTree>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
  tab: Tab,
  tree: tree::MarketTree,
}

impl State {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn active_tab(&self) -> Tab {
    self.tab
  }

  #[expect(dead_code, reason = "Read by the Phase 3 left-pane render task.")]
  pub fn tree(&self) -> &tree::MarketTree {
    &self.tree
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
