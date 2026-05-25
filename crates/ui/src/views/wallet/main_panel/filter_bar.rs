//! Filter bar — search box and pill filter row.

use iced::{
  Border, Element, Length, Padding,
  widget::{Space, container, row},
};

use crate::{
  components::{PillFilter, SearchBox},
  style::{color, spacing},
  views::wallet::{Message, SideFilter, SignFilter, State, Tab, journal_tab, market_tab},
};

fn search_box(query: &str) -> Element<'_, Message> {
  SearchBox::new("Filter by ref, party, station…", query, Message::SearchChanged).render()
}

fn pill_group<'a, T>(options: Vec<(&'a str, T)>, active: &'a T, make_msg: fn(T) -> Message) -> Element<'a, Message>
where
  T: PartialEq + Clone + 'static,
{
  PillFilter::new(options, active, make_msg).render()
}

fn journal_pills(state: &State) -> Element<'_, Message> {
  pill_group(
    vec![
      ("All", SignFilter::All),
      ("In", SignFilter::In),
      ("Out", SignFilter::Out),
    ],
    &state.sign_filter,
    |s| Message::JournalTab(journal_tab::Message::SignFilterChanged(s)),
  )
}

fn market_pills(state: &State) -> Element<'_, Message> {
  pill_group(
    vec![
      ("All", SideFilter::All),
      ("Buy", SideFilter::Buy),
      ("Sell", SideFilter::Sell),
    ],
    &state.side_filter,
    |s| Message::MarketTab(market_tab::Message::SideFilterChanged(s)),
  )
}

fn tab_pills(state: &State) -> Option<Element<'_, Message>> {
  match state.active_tab {
    Tab::Journal => Some(journal_pills(state)),
    Tab::Market => Some(market_pills(state)),
    Tab::Contracts => None,
  }
}

fn filter_bar_container(items: Vec<Element<'_, Message>>) -> Element<'_, Message> {
  container(row(items).align_y(iced::alignment::Vertical::Center))
    .padding(Padding {
      top: 12.0,
      bottom: 12.0,
      left: spacing::SPACE_7,
      right: spacing::SPACE_7,
    })
    .width(Length::Fill)
    .style(|_| container::Style {
      border: Border {
        color: color::border::SUBTLE,
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

/// Builder for the wallet filter bar.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new filter bar component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the filter bar into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let mut items: Vec<Element<'_, Message>> = vec![search_box(&state.search_query)];
    if let Some(p) = tab_pills(state) {
      items.push(Space::new().width(spacing::SPACE_3).into());
      items.push(p);
    }
    filter_bar_container(items)
  }
}
