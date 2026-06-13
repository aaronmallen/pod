use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, opaque, svg, text, text_input},
};

use super::shared;
use crate::{
  features::character_detail::{LoadState, Message, STANDINGS_SEARCH_INPUT_ID, StandingKind, StandingsRow},
  store::{
    repo::standings,
    search::{ChipKind, ParsedQuery},
  },
  ui::{
    components::{
      avatar::avatar,
      card,
      empty_state::{LoadStateView, empty_state, load_state_view},
      eyebrow::eyebrow,
      meter, rule,
      section_header::section_header,
    },
    style::{color, control, radius, spacing, typography},
  },
};

static CHECK_ICON: &[u8] = include_bytes!("../../../../assets/images/icons/check.svg");
static CLOSE_ICON: &[u8] = include_bytes!("../../../../assets/images/icons/close.svg");
static HELP_ICON: &[u8] = include_bytes!("../../../../assets/images/icons/help.svg");
static LOCK_ICON: &[u8] = include_bytes!("../../../../assets/images/icons/lock.svg");
static SEARCH_ICON: &[u8] = include_bytes!("../../../../assets/images/icons/search.svg");

const ACCESS_ICON_SIZE: f32 = 14.0;
const AVATAR_SIZE: f32 = 30.0;
const CHIPS_PER_ROW: usize = 4;
const CLOSE_ICON_SIZE: f32 = 14.0;

const EXAMPLES: &[(&str, &str)] = &[
  ("faction:caldari", "Caldari faction + corps"),
  ("faction:caldari,amarr", "Caldari OR Amarr"),
  ("corp:navy", "corps containing \"navy\""),
  ("faction:caldari -corp:\"sisters of eve\"", "Caldari, minus that corp"),
  ("level:4 division:security", "L4 security agents"),
  ("type:research field:caldari", "Caldari research agents"),
  ("system:jita reachable", "accessible agents near Jita"),
  ("\"mordu's legion\"", "phrase match"),
];

const HELP_ICON_SIZE: f32 = 15.0;
const INPUT_BOX_HEIGHT: f32 = 36.0;
const POPOVER_WIDTH: f32 = 380.0;
const SEARCH_ICON_SIZE: f32 = 14.0;
const STANDING_BAR_HEIGHT: f32 = 6.0;
const STANDING_BAR_WIDTH: f32 = 160.0;

pub(crate) fn body<'a>(
  catalog: &'a LoadState<Vec<StandingsRow>>,
  query: &'a str,
  has_filters: bool,
) -> Element<'a, Message> {
  let bar = search_bar(query, has_filters);
  let preview = query_preview(query);

  let rows = match catalog {
    LoadState::Loaded(rows) => rows,
    LoadState::Loading => {
      return stacked(
        bar,
        preview,
        load_state_view(LoadStateView::Loading("Loading standings\u{2026}")),
      );
    }
    LoadState::Error(error) => {
      return stacked(bar, preview, load_state_view(LoadStateView::Error(error)));
    }
  };

  if rows.is_empty() {
    return stacked(bar, preview, no_results(has_filters));
  }

  let mut sections: Vec<Element<'a, Message>> = Vec::new();
  for (kind, label) in [
    (StandingKind::Faction, "Factions"),
    (StandingKind::Corporation, "Corporations"),
    (StandingKind::Agent, "Agents"),
  ] {
    let group: Vec<&StandingsRow> = rows.iter().filter(|row| row.kind == kind && !is_other(row)).collect();
    if group.is_empty() {
      continue;
    }
    sections.push(section(label, &group, has_filters));
  }

  let other: Vec<&StandingsRow> = rows.iter().filter(|row| is_other(row)).collect();
  if !other.is_empty() {
    sections.push(section("Other", &other, has_filters));
  }

  let groups = Column::with_children(sections)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill);

  stacked(bar, preview, groups.into())
}

pub(crate) fn help_popover<'a>() -> Element<'a, Message> {
  let header = Row::with_children(vec![
    section_label("Filter syntax"),
    Space::new().width(Length::Fill).into(),
    icon_button(
      icon(CLOSE_ICON, CLOSE_ICON_SIZE, color::text::secondary()),
      Message::StandingsToggleHelp,
    ),
  ])
  .align_y(Vertical::Center);

  let intro = text(
    "Filter the standings catalog with plain text and key:value filters. Comma-separate values for OR, \
    repeat keys to AND, prefix with - to negate. Click any example to add it.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(muted_text);

  let examples = Column::with_children(
    EXAMPLES
      .iter()
      .map(|&(query, note)| example_row(query, note))
      .collect::<Vec<_>>(),
  )
  .spacing(spacing::SPACE_2);

  let keys = chip_row(standings::AVAILABLE_KEYS.iter().map(|&key| key_chip(key)).collect());

  let content = Column::with_children(vec![
    header.into(),
    intro.into(),
    examples.into(),
    section_label("Available keys"),
    keys,
  ])
  .spacing(spacing::SPACE_3);

  let card = container(content)
    .width(Length::Fixed(POPOVER_WIDTH))
    .padding(spacing::SPACE_3_5)
    .style(control::card);

  opaque(card)
}

fn accessibility_indicator<'a>(row: &StandingsRow) -> Option<Element<'a, Message>> {
  let accessible = row.accessible?;
  let (bytes, tint) = if accessible {
    (CHECK_ICON, color::status::ONLINE)
  } else {
    (LOCK_ICON, color::text::tertiary())
  };

  Some(icon(bytes, ACCESS_ICON_SIZE, tint))
}

fn chip_padding() -> Padding {
  Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
  }
}

fn chip_row<'a>(chips: Vec<Element<'a, Message>>) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  let mut current: Vec<Element<'a, Message>> = Vec::new();
  for chip in chips {
    current.push(chip);
    if current.len() == CHIPS_PER_ROW {
      rows.push(
        Row::with_children(std::mem::take(&mut current))
          .spacing(spacing::SPACE_2)
          .into(),
      );
    }
  }
  if !current.is_empty() {
    rows.push(Row::with_children(current).spacing(spacing::SPACE_2).into());
  }

  Column::with_children(rows).spacing(spacing::SPACE_2).into()
}

fn clear_button<'a>() -> Element<'a, Message> {
  button(eyebrow("Clear", Some(color::text::secondary())))
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
    })
    .on_press(Message::StandingsClearSearch)
    .style(clear_button_style)
    .into()
}

fn clear_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let border_color = match status {
    button::Status::Hovered | button::Status::Pressed => color::with_alpha(color::text::PRIMARY, 0.24),
    _ => color::with_alpha(color::text::PRIMARY, 0.12),
  };

  button::Style {
    background: None,
    text_color: color::text::secondary(),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..button::Style::default()
  }
}

fn code_chip<'a>(label: &'a str) -> Element<'a, Message> {
  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(chip_padding())
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.10))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.25),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn example_row<'a>(query: &'a str, note: &'a str) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      code_chip(query),
      text(note)
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(muted_text)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .padding(spacing::SPACE_2)
  .width(Length::Fill)
  .on_press(Message::StandingsInsertQuery(query.to_owned()))
  .style(example_button_style)
  .into()
}

fn example_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05)))
    }
    _ => None,
  };

  button::Style {
    background,
    text_color: color::text::secondary(),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn icon<'a>(bytes: &'static [u8], size: f32, tint: Color) -> Element<'a, Message> {
  svg(svg::Handle::from_memory(bytes))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .style(move |_, _| svg::Style {
      color: Some(tint),
    })
    .into()
}

fn icon_button<'a>(content: Element<'a, Message>, message: Message) -> Element<'a, Message> {
  button(content)
    .padding(spacing::SPACE_2)
    .on_press(message)
    .style(icon_button_style)
    .into()
}

fn icon_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
    }
    _ => None,
  };

  button::Style {
    background,
    text_color: color::text::secondary(),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn input_box_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  }
}

fn input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

fn is_other(row: &StandingsRow) -> bool {
  matches!(row.kind, StandingKind::Corporation | StandingKind::Agent) && row.faction_id.is_none()
}

fn key_chip<'a>(key: &str) -> Element<'a, Message> {
  container(
    text(format!("{key}:"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(muted_text),
  )
  .padding(chip_padding())
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.12),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn meta_line<'a>(row: &StandingsRow) -> Option<Element<'a, Message>> {
  if row.kind != StandingKind::Agent {
    return None;
  }

  let mut parts: Vec<String> = Vec::new();
  if let Some(level) = row.level {
    parts.push(format!("L{level}"));
  }
  if let Some(agent_type) = row.agent_type.as_deref() {
    parts.push(agent_type.to_owned());
  }
  if let Some(division) = row.division.as_deref() {
    parts.push(division.to_owned());
  }
  match (row.system.as_deref(), row.region.as_deref()) {
    (Some(system), Some(region)) => parts.push(format!("{system} \u{00b7} {region}")),
    (Some(system), None) => parts.push(system.to_owned()),
    _ => {}
  }

  if parts.is_empty() {
    return None;
  }

  Some(
    text(parts.join("  \u{00b7}  "))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  )
}

fn muted_text(_theme: &iced::Theme) -> text::Style {
  text::Style {
    color: Some(color::text::secondary()),
  }
}

fn no_results<'a>(has_filters: bool) -> Element<'a, Message> {
  if has_filters {
    load_state_view(LoadStateView::Empty(
      empty_state("No standings match").action("Clear filter", Message::StandingsClearSearch),
    ))
  } else {
    load_state_view(LoadStateView::Empty(empty_state("No standings catalog available")))
  }
}

fn preview_chip<'a>(label: &str, kind: &ChipKind) -> Element<'a, Message> {
  let (fg, bg, border) = match kind {
    ChipKind::Negated => (
      color::status::DANGER,
      color::with_alpha(color::status::DANGER, 0.10),
      color::with_alpha(color::status::DANGER, 0.35),
    ),
    ChipKind::KeyValue => (
      color::accent::PLASMA,
      color::with_alpha(color::accent::PLASMA, 0.10),
      color::with_alpha(color::accent::PLASMA, 0.35),
    ),
    ChipKind::FreeText => (
      color::text::PRIMARY,
      color::with_alpha(color::text::PRIMARY, 0.05),
      color::with_alpha(color::text::PRIMARY, 0.12),
    ),
  };

  container(
    text(label.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(fg),
      }),
  )
  .padding(chip_padding())
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: border,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn query_preview<'a>(query: &str) -> Option<Element<'a, Message>> {
  let parsed: ParsedQuery = standings::parse(query);
  let chips = parsed.display_chips();
  if chips.is_empty() {
    return None;
  }

  let mut children: Vec<Element<'a, Message>> = vec![eyebrow("Parsed", Some(color::text::tertiary()))];
  for (label, kind) in chips {
    children.push(preview_chip(&label, &kind));
  }

  Some(
    Row::with_children(children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .wrap()
      .into(),
  )
}

fn row_view<'a>(row: &StandingsRow, last: bool) -> Element<'a, Message> {
  let value = row.effective;
  let accent = shared::standing_color(value);

  let portrait = avatar(
    row.id,
    &row.name,
    Length::Fixed(AVATAR_SIZE),
    AVATAR_SIZE,
    row.image.path(),
  );

  let mut name_block: Vec<Element<'a, Message>> = vec![
    text(row.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if let Some(meta) = meta_line(row) {
    name_block.push(meta);
  }
  let name = Column::with_children(name_block)
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let raw = text(format!("{}{:.2} raw", if row.raw >= 0.0 { "+" } else { "" }, row.raw))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });

  let signed = text(format!("{}{:.2}", if value >= 0.0 { "+" } else { "" }, value))
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(accent),
    });

  let bar = meter::diverging(
    value,
    shared::STANDING_MAX,
    accent,
    STANDING_BAR_WIDTH,
    STANDING_BAR_HEIGHT,
  );

  let mut cluster: Vec<Element<'a, Message>> = vec![portrait, name.into()];
  if let Some(indicator) = accessibility_indicator(row) {
    cluster.push(indicator);
  }
  cluster.push(raw.into());
  cluster.push(signed.into());
  cluster.push(bar);

  let inner = Row::with_children(cluster)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };
  container(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
    })
    .style(move |_| shared::row_rule_style(border_bottom))
    .into()
}

fn search_bar<'a>(query: &str, has_filters: bool) -> Element<'a, Message> {
  let input = text_input("Filter\u{2026} try faction:caldari or -corp:\"sisters of eve\"", query)
    .id(STANDINGS_SEARCH_INPUT_ID)
    .on_input(Message::StandingsSearchChanged)
    .size(typography::size::MD)
    .padding(0)
    .style(input_style)
    .width(Length::Fill);

  let mut cluster: Vec<Element<'a, Message>> = vec![
    icon(SEARCH_ICON, SEARCH_ICON_SIZE, color::text::secondary()),
    input.into(),
  ];

  if !query.is_empty() {
    cluster.push(icon_button(
      icon(CLOSE_ICON, CLOSE_ICON_SIZE, color::text::secondary()),
      Message::StandingsClearSearch,
    ));
  }
  cluster.push(rule::vertical_alpha(18.0, 0.12));
  cluster.push(icon_button(
    icon(HELP_ICON, HELP_ICON_SIZE, color::text::secondary()),
    Message::StandingsToggleHelp,
  ));

  let input_box = container(
    Row::with_children(cluster)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .height(Length::Fixed(INPUT_BOX_HEIGHT))
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    right: 4.0,
    bottom: 0.0,
    left: spacing::SPACE_3,
  })
  .style(input_box_style);

  let mut row: Vec<Element<'a, Message>> = vec![input_box.into()];
  if has_filters {
    row.push(clear_button());
  }

  Row::with_children(row)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn section<'a>(label: &'a str, rows: &[&'a StandingsRow], has_filters: bool) -> Element<'a, Message> {
  let suffix = if has_filters { "matched" } else { "tracked" };
  let header = section_header(label, Some(&format!("{} {suffix}", rows.len())));

  let mut card_rows: Vec<Element<'a, Message>> = Vec::with_capacity(rows.len());
  for (index, row) in rows.iter().enumerate() {
    card_rows.push(row_view(row, index == rows.len() - 1));
  }
  let card = card::panel(Column::with_children(card_rows).width(Length::Fill), false);

  Column::with_children(vec![header, card])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn section_label<'a>(label: &str) -> Element<'a, Message> {
  eyebrow(label, Some(color::text::tertiary()))
}

fn stacked<'a>(
  bar: Element<'a, Message>,
  preview: Option<Element<'a, Message>>,
  content: Element<'a, Message>,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![bar];
  if let Some(preview) = preview {
    children.push(preview);
  }
  children.push(content);

  Column::with_children(children)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::images::{ImageKind, ImageState};

  fn row(id: i64, kind: StandingKind, name: &str, faction_id: Option<i64>, effective: f64) -> StandingsRow {
    StandingsRow {
      accessible: None,
      agent_type: None,
      division: None,
      effective,
      faction_id,
      id,
      image: ImageState::Stale {
        id,
        kind: ImageKind::CorporationLogo,
      },
      kind,
      level: None,
      name: name.to_owned(),
      raw: 0.0,
      region: None,
      system: None,
    }
  }

  fn agent(id: i64, name: &str, faction_id: Option<i64>, accessible: Option<bool>) -> StandingsRow {
    StandingsRow {
      accessible,
      agent_type: Some("BasicAgent".to_owned()),
      division: Some("Security".to_owned()),
      effective: 3.0,
      faction_id,
      id,
      image: ImageState::Stale {
        id,
        kind: ImageKind::CharacterPortrait,
      },
      kind: StandingKind::Agent,
      level: Some(4),
      name: name.to_owned(),
      raw: 1.0,
      region: Some("The Forge".to_owned()),
      system: Some("Jita".to_owned()),
    }
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_grouped_sections_in_the_default_view() {
      let rows = vec![
        row(500_001, StandingKind::Faction, "Caldari State", Some(500_001), 5.0),
        row(1_000_001, StandingKind::Corporation, "Caldari Navy", Some(500_001), 4.0),
        row(1_000_100, StandingKind::Corporation, "Doomheim", None, 0.0),
      ];
      let catalog = LoadState::Loaded(rows);

      let _el: Element<'_, Message> = body(&catalog, "", false);
    }

    #[test]
    fn it_renders_agent_rows_with_an_active_filter() {
      let rows = vec![agent(3_000_001, "Navy Sec Agent", Some(500_001), Some(true))];
      let catalog = LoadState::Loaded(rows);

      let _el: Element<'_, Message> = body(&catalog, "level:4", true);
    }

    #[test]
    fn it_renders_the_loading_and_error_and_empty_states() {
      let loading: LoadState<Vec<StandingsRow>> = LoadState::Loading;
      let error: LoadState<Vec<StandingsRow>> = LoadState::Error("boom".to_owned());
      let empty = LoadState::Loaded(Vec::new());

      let _loading: Element<'_, Message> = body(&loading, "", false);
      let _error: Element<'_, Message> = body(&error, "", false);
      let _empty: Element<'_, Message> = body(&empty, "faction:none", true);
    }
  }

  mod is_other {
    use super::*;

    #[test]
    fn it_flags_a_factionless_corp_as_other() {
      assert!(is_other(&row(1, StandingKind::Corporation, "Doomheim", None, 0.0)));
    }

    #[test]
    fn it_does_not_flag_a_factioned_corp() {
      assert!(!is_other(&row(
        1,
        StandingKind::Corporation,
        "Caldari Navy",
        Some(500_001),
        0.0
      )));
    }

    #[test]
    fn it_never_flags_a_faction_row() {
      assert!(!is_other(&row(1, StandingKind::Faction, "Caldari State", None, 0.0)));
    }
  }

  mod meta_line {
    use super::*;

    #[test]
    fn it_builds_a_line_for_an_agent() {
      assert!(meta_line(&agent(1, "A", Some(500_001), None)).is_some());
    }

    #[test]
    fn it_is_absent_for_a_faction() {
      assert!(meta_line(&row(1, StandingKind::Faction, "Caldari State", Some(500_001), 0.0)).is_none());
    }
  }

  mod help_popover {
    use super::*;

    #[test]
    fn it_renders() {
      let _el: Element<'_, Message> = help_popover();
    }
  }
}
