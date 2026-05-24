//! Main panel — tab strip, division strip, filter bar, and active tab body.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, button, column, container, row, text},
};

use crate::{
  components::{PillFilter, SearchBox, TabStrip, tab_strip::TabItem},
  format,
  style::{color, spacing, typography::mono},
  views::wallet::{Message, SideFilter, SignFilter, State, Tab, journal_tab, market_tab},
};

fn tab_bar(state: &State) -> Element<'_, Message> {
  let tabs = vec![
    TabItem {
      label: "Market".to_string(),
      count: Some(state.filtered_market.len()),
    },
    TabItem {
      label: "Contracts".to_string(),
      count: Some(state.filtered_contracts.len()),
    },
    TabItem {
      label: "Journal".to_string(),
      count: Some(state.filtered_journal.len()),
    },
  ];
  let active_index = match state.active_tab {
    Tab::Market => 0,
    Tab::Contracts => 1,
    Tab::Journal => 2,
  };
  TabStrip::new(tabs).active(active_index).render(|i| {
    Message::TabSelected(match i {
      0 => Tab::Market,
      1 => Tab::Contracts,
      _ => Tab::Journal,
    })
  })
}

fn division_strip(state: &State) -> Element<'_, Message> {
  let btns: Vec<Element<'_, Message>> = (1u8..=7)
    .map(|div| {
      let is_active = div == state.active_division;
      let balance = state
        .corp_divisions
        .iter()
        .find(|(d, _)| *d == div)
        .map(|(_, bal)| *bal);
      let label = if let Some(bal) = balance {
        format!("Div {} · {}", div, format::fmt_isk(bal))
      } else {
        format!("Division {div}")
      };
      button(
        text(label)
          .font(mono::REGULAR)
          .size(10.0)
          .style(move |_: &Theme| iced::widget::text::Style {
            color: Some(if is_active {
              color::accent::PLASMA
            } else {
              color::text::SECONDARY
            }),
          }),
      )
      .padding(Padding {
        top: 8.0,
        bottom: 8.0,
        left: 14.0,
        right: 14.0,
      })
      .on_press(Message::DivisionSelected(div))
      .style(move |_, _| button::Style {
        background: if is_active {
          Some(Background::Color(color::accent::PLASMA_SUBTLE))
        } else {
          None
        },
        border: Border {
          color: Color::TRANSPARENT,
          radius: 0.0.into(),
          width: 0.0,
        },
        text_color: if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        },
        ..button::Style::default()
      })
      .into()
    })
    .collect();
  container(row(btns).spacing(0.0).width(Length::Fill))
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

fn search_box(query: &str) -> Element<'_, Message> {
  SearchBox::new("Filter by ref, party, station…", query, Message::SearchChanged).render()
}

fn pill_group<'a, T>(options: Vec<(&'a str, T)>, active: &'a T, make_msg: fn(T) -> Message) -> Element<'a, Message>
where
  T: PartialEq + Clone + 'static,
{
  PillFilter::new(options, active, make_msg).render()
}

fn filter_bar(state: &State) -> Element<'_, Message> {
  let pills: Option<Element<'_, Message>> = match state.active_tab {
    Tab::Journal => Some(pill_group(
      vec![
        ("All", SignFilter::All),
        ("In", SignFilter::In),
        ("Out", SignFilter::Out),
      ],
      &state.sign_filter,
      |s| Message::JournalTab(journal_tab::Message::SignFilterChanged(s)),
    )),
    Tab::Market => Some(pill_group(
      vec![
        ("All", SideFilter::All),
        ("Buy", SideFilter::Buy),
        ("Sell", SideFilter::Sell),
      ],
      &state.side_filter,
      |s| Message::MarketTab(market_tab::Message::SideFilterChanged(s)),
    )),
    Tab::Contracts => None,
  };

  let mut items: Vec<Element<'_, Message>> = vec![search_box(&state.search_query)];
  if let Some(p) = pills {
    items.push(Space::new().width(spacing::SPACE_3).into());
    items.push(p);
  }

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

/// Builder for the wallet main panel.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new main panel component.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the main panel into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let tab_bar_el = tab_bar(state);
    let filter_bar_el = filter_bar(state);
    let table: Element<'_, Message> = match state.active_tab {
      Tab::Contracts => crate::views::wallet::contracts_tab::Component::new(state)
        .render()
        .map(Message::ContractsTab),
      Tab::Journal => crate::views::wallet::journal_tab::Component::new(state)
        .render()
        .map(Message::JournalTab),
      Tab::Market => crate::views::wallet::market_tab::Component::new(state)
        .render()
        .map(Message::MarketTab),
    };
    let mut cols: Vec<Element<'_, Message>> = vec![tab_bar_el];
    if state.is_corp_selected() {
      cols.push(division_strip(state));
    }
    cols.push(filter_bar_el);
    cols.push(table);
    container(column(cols))
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into()
  }
}
