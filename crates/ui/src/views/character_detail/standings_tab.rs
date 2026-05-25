//! Standings tab: faction, corp, and agent standing rows with bar visualisation.

pub mod standing_bar;
pub mod standing_row;

use iced::{
  Background, Border, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, scrollable, text},
};
use pod_model::CharacterStanding;
use standing_row::StandingRow;

use crate::{
  components::LoadState,
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::{LoadState as DataState, Message},
};

/// Builder for the standings tab content.
pub struct Component<'a> {
  standings: &'a DataState<Vec<CharacterStanding>>,
}

impl<'a> Component<'a> {
  /// Creates a new standings tab component.
  pub fn new(standings: &'a DataState<Vec<CharacterStanding>>) -> Self {
    Self {
      standings,
    }
  }

  /// Renders the standings tab.
  pub fn render(self) -> Element<'a, Message> {
    match self.standings {
      DataState::Loading => LoadState::loading("Loading standings…").render(),
      DataState::Error(e) => LoadState::error(e).render(),
      DataState::Loaded(standings) => standings_content(standings),
    }
  }
}

fn empty_standings_message<'a>() -> Element<'a, Message> {
  container(
    text("No standings data available.")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(32.0)
  .into()
}

fn filter_standings(standings: &[CharacterStanding]) -> [Vec<&CharacterStanding>; 3] {
  let factions = standings.iter().filter(|s| s.from_type == "faction").collect();
  let corps = standings
    .iter()
    .filter(|s| s.from_type == "npc_corp" || s.from_type == "corporation")
    .collect();
  let agents = standings.iter().filter(|s| s.from_type == "agent").collect();
  [factions, corps, agents]
}

fn section_eyebrow(label: &str, right: &str) -> Element<'static, Message> {
  let left_el = text(label.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });
  let right_el = text(right.to_uppercase())
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    });
  row([left_el.into(), Space::new().width(Length::Fill).into(), right_el.into()]).into()
}

fn standings_content(standings: &[CharacterStanding]) -> Element<'_, Message> {
  let [factions, corps, agents] = filter_standings(standings);
  let mut sections: Vec<Element<'_, Message>> = Vec::new();
  if !factions.is_empty() {
    sections.push(standings_section("Factions", &factions));
  }
  if !corps.is_empty() {
    sections.push(standings_section("Corps", &corps));
  }
  if !agents.is_empty() {
    sections.push(standings_section("Agents", &agents));
  }
  if sections.is_empty() {
    sections.push(empty_standings_message());
  }
  scrollable(
    column(sections)
      .spacing(24.0)
      .padding(Padding {
        top: 24.0,
        bottom: 24.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
  )
  .height(Length::Fill)
  .into()
}

fn standings_section<'a>(label: &'a str, rows: &[&'a CharacterStanding]) -> Element<'a, Message> {
  let count_label = format!("{} tracked", rows.len());
  let eyebrow: Element<'_, Message> = section_eyebrow(label, &count_label);
  let mut row_els: Vec<Element<'_, Message>> = rows
    .iter()
    .enumerate()
    .map(|(i, s)| StandingRow::new(s, i == rows.len() - 1).render())
    .collect();
  if row_els.is_empty() {
    row_els.push(Space::new().height(0.0).into());
  }
  let card = container(column(row_els))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::DEFAULT,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });
  column([eyebrow, card.into()]).spacing(10.0).into()
}
