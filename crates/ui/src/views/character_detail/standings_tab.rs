//! Standings tab: faction, corp, and agent standing rows with bar visualisation.

use iced::{
  Background, Border, Color, Element, Length, Padding, Theme,
  widget::{Space, column, container, row, scrollable, text},
};
use pod_model::CharacterStanding;

use crate::{
  style::{
    color,
    typography::{body, mono},
  },
  views::character_detail::{LoadState, Message},
};

/// Builder for the standings tab content.
pub struct Component<'a> {
  standings: &'a LoadState<Vec<CharacterStanding>>,
}

impl<'a> Component<'a> {
  /// Creates a new standings tab component.
  pub fn new(standings: &'a LoadState<Vec<CharacterStanding>>) -> Self {
    Self {
      standings,
    }
  }

  /// Renders the standings tab.
  pub fn render(self) -> Element<'a, Message> {
    match self.standings {
      LoadState::Loading => loading_state(),
      LoadState::Error(e) => error_state(e),
      LoadState::Loaded(standings) => standings_content(standings),
    }
  }
}

fn loading_state<'a>() -> Element<'a, Message> {
  container(
    text("Loading standings…")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(32.0)
  .width(Length::Fill)
  .center_x(Length::Fill)
  .into()
}

fn error_state<'a>(msg: &'a str) -> Element<'a, Message> {
  container(
    text(msg)
      .font(body::REGULAR)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(32.0)
  .width(Length::Fill)
  .center_x(Length::Fill)
  .into()
}

fn standings_content(standings: &[CharacterStanding]) -> Element<'_, Message> {
  let factions: Vec<&CharacterStanding> = standings.iter().filter(|s| s.from_type == "faction").collect();
  let corps: Vec<&CharacterStanding> = standings
    .iter()
    .filter(|s| s.from_type == "npc_corp" || s.from_type == "corporation")
    .collect();
  let agents: Vec<&CharacterStanding> = standings.iter().filter(|s| s.from_type == "agent").collect();

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
    sections.push(
      container(
        text("No standings data available.")
          .font(body::REGULAR)
          .size(13.0)
          .style(|_: &Theme| iced::widget::text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .padding(32.0)
      .into(),
    );
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
    .map(|(i, s)| standing_row(s, i == rows.len() - 1))
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

fn standing_row<'a>(standing: &'a CharacterStanding, is_last: bool) -> Element<'a, Message> {
  let v = standing.standing;
  let effective_color = standing_color(v);

  let name_el = text(standing.from_name.clone())
    .font(body::REGULAR)
    .size(13.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .width(Length::Fill);

  let raw_label = format!(
    "{}{:.2} raw",
    if standing.standing >= 0.0 { "+" } else { "" },
    standing.standing
  );
  let raw_el = container(
    text(raw_label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(90.0)
  .align_x(iced::alignment::Horizontal::Right);

  let eff_label = format!("{}{:.2}", if v >= 0.0 { "+" } else { "" }, v);
  let eff_el =
    container(
      text(eff_label)
        .font(mono::MEDIUM)
        .size(14.0)
        .style(move |_: &Theme| iced::widget::text::Style {
          color: Some(effective_color),
        }),
    )
    .width(60.0)
    .align_x(iced::alignment::Horizontal::Right);

  let bar_el = standing_bar(v);

  let inner = row([name_el.into(), raw_el.into(), eff_el.into(), bar_el])
    .align_y(iced::alignment::Vertical::Center)
    .spacing(16.0)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 16.0,
      right: 16.0,
    });

  container(inner)
    .width(Length::Fill)
    .style(move |_| container::Style {
      border: Border {
        color: if is_last {
          Color::TRANSPARENT
        } else {
          color::border::SUBTLE
        },
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn standing_color(v: f64) -> Color {
  if v >= 5.0 {
    color::status::ONLINE
  } else if v > 0.0 {
    Color::from_rgba(0.357, 0.725, 0.494, 0.65)
  } else if v >= -0.01 {
    color::text::SECONDARY
  } else if v > -5.0 {
    Color::from_rgba(0.878, 0.459, 0.349, 0.65)
  } else {
    color::status::DANGER
  }
}

fn standing_bar<'a>(value: f64) -> Element<'a, Message> {
  let bar_color = standing_color(value);
  let pct = (value.abs() / 10.0 * 50.0).min(50.0) as f32;
  let positive = value >= 0.0;
  let fill_width = (220.0 * pct / 100.0).max(0.0);

  let fill = container(Space::new().width(fill_width).height(6.0))
    .width(fill_width)
    .height(6.0)
    .style(move |_: &Theme| container::Style {
      background: Some(Background::Color(bar_color)),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let bar_inner: Element<'_, Message> = if positive {
    row([Space::new().width(110.0).height(6.0).into(), fill.into()])
      .height(6.0)
      .into()
  } else {
    let fill_start = 110.0 - fill_width;
    row([Space::new().width(fill_start).height(6.0).into(), fill.into()])
      .height(6.0)
      .into()
  };

  container(bar_inner)
    .width(220.0)
    .height(6.0)
    .style(|_: &Theme| container::Style {
      background: Some(Background::Color(Color::from_rgba(0.957, 0.949, 0.925, 0.05))),
      border: Border {
        radius: 3.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}
