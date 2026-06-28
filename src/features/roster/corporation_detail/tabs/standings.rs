use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, button, container, svg, text, text_input},
};

use super::shared;
use crate::{
  features::roster::corporation_detail::{LoadState, Message, STANDINGS_SEARCH_INPUT_ID, StandingKind, StandingsRow},
  store::{
    repo::standings,
    search::{ChipKind, ParsedQuery},
  },
  ui::{
    components::{
      avatar::avatar,
      empty_state::{LoadStateView, empty_state, load_state_view},
      eyebrow::eyebrow,
      meter,
      section_header::section_header,
      segmented::segment_button_style,
      virtual_list::{VirtualList, VirtualListConfig},
    },
    style::{color, radius, spacing, typography},
  },
};

static CHECK_ICON: &[u8] = include_bytes!("../../../../../assets/images/icons/check.svg");
static CLOSE_ICON: &[u8] = include_bytes!("../../../../../assets/images/icons/close.svg");
static LOCK_ICON: &[u8] = include_bytes!("../../../../../assets/images/icons/lock.svg");
static SEARCH_ICON: &[u8] = include_bytes!("../../../../../assets/images/icons/search.svg");

const ACCESS_ICON_SIZE: f32 = 14.0;
const AVATAR_SIZE: f32 = 30.0;
const CLOSE_ICON_SIZE: f32 = 14.0;
const ESTIMATED_ROW_HEIGHT: f32 = 48.0;
const INPUT_BOX_HEIGHT: f32 = 36.0;
const SEARCH_ICON_SIZE: f32 = 14.0;
const STANDING_BAR_HEIGHT: f32 = 6.0;
const STANDING_BAR_WIDTH: f32 = 160.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StandingsFilter {
  Agents,
  #[default]
  All,
  Corps,
  Factions,
  Other,
}

impl StandingsFilter {
  const SEGMENTS: [(StandingsFilter, &'static str); 5] = [
    (StandingsFilter::All, "roster.standings.filter_all"),
    (StandingsFilter::Factions, "roster.standings.filter_factions"),
    (StandingsFilter::Corps, "roster.standings.filter_corps"),
    (StandingsFilter::Agents, "roster.standings.filter_agents"),
    (StandingsFilter::Other, "roster.standings.filter_other"),
  ];

  pub fn surfaces_agents(self) -> bool {
    matches!(self, StandingsFilter::All | StandingsFilter::Agents)
  }

  fn matches(self, row: &StandingsRow) -> bool {
    match self {
      StandingsFilter::Agents => row.kind == StandingKind::Agent && !is_other(row),
      StandingsFilter::All => true,
      StandingsFilter::Corps => row.kind == StandingKind::Corporation && !is_other(row),
      StandingsFilter::Factions => row.kind == StandingKind::Faction,
      StandingsFilter::Other => is_other(row),
    }
  }
}

enum FlatItem<'a> {
  Header { count: usize, label: &'static str },
  Row { last: bool, row: &'a StandingsRow },
}

pub(crate) fn header<'a>(query: &'a str, filter: StandingsFilter, has_filters: bool) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![search_bar(query, has_filters)];
  if let Some(preview) = query_preview(query) {
    children.push(preview);
  }
  children.push(segmented(filter));

  Column::with_children(children)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill)
    .into()
}

pub(crate) fn body<'a>(
  catalog: &'a LoadState<Vec<StandingsRow>>,
  filter: StandingsFilter,
  has_filters: bool,
  viewport_height: f32,
  scroll_offset: f32,
) -> Element<'a, Message> {
  let rows = match catalog {
    LoadState::Loaded(rows) => rows,
    LoadState::Loading => {
      return load_state_view(LoadStateView::Loading(shared::static_text(t!(
        "roster.standings.loading"
      ))));
    }
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };

  if rows.is_empty() {
    return no_results(has_filters);
  }

  let items = flatten_sections(rows, filter);
  if items.is_empty() {
    return no_results(has_filters);
  }

  let config = VirtualListConfig::new(items.len(), ESTIMATED_ROW_HEIGHT)
    .viewport_height(viewport_height)
    .scroll_offset(scroll_offset);
  VirtualList::new(config, move |index| match &items[index] {
    FlatItem::Header {
      count,
      label,
    } => section_heading(label, *count, has_filters),
    FlatItem::Row {
      last,
      row,
    } => row_view(row, *last),
  })
  .spacing(spacing::SPACE_2_5)
  .view()
}

fn flatten_sections<'a>(rows: &'a [StandingsRow], filter: StandingsFilter) -> Vec<FlatItem<'a>> {
  let mut items: Vec<FlatItem<'a>> = Vec::new();
  let mut push_section = |label: &'static str, group: Vec<&'a StandingsRow>| {
    if group.is_empty() {
      return;
    }
    items.push(FlatItem::Header {
      count: group.len(),
      label,
    });
    let last = group.len() - 1;
    for (index, row) in group.into_iter().enumerate() {
      items.push(FlatItem::Row {
        last: index == last,
        row,
      });
    }
  };

  for (kind, label) in [
    (StandingKind::Faction, "roster.standings.section_factions"),
    (StandingKind::Corporation, "roster.standings.section_corporations"),
    (StandingKind::Agent, "roster.standings.section_agents"),
  ] {
    let group: Vec<&StandingsRow> = rows
      .iter()
      .filter(|row| row.kind == kind && !is_other(row) && filter.matches(row))
      .collect();
    push_section(label, group);
  }

  let other: Vec<&StandingsRow> = rows.iter().filter(|row| is_other(row) && filter.matches(row)).collect();
  push_section("roster.standings.section_other", other);

  items
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

fn clear_button<'a>() -> Element<'a, Message> {
  button(eyebrow(&t!("roster.standings.clear"), Some(color::text::secondary())))
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

fn no_results<'a>(has_filters: bool) -> Element<'a, Message> {
  if has_filters {
    let title = shared::static_text(t!("roster.standings.no_match"));
    let action = shared::static_text(t!("roster.standings.clear_filter"));
    load_state_view(LoadStateView::Empty(
      empty_state(title).action(action, Message::StandingsClearSearch),
    ))
  } else {
    let title = shared::static_text(t!("roster.standings.no_catalog"));
    load_state_view(LoadStateView::Empty(empty_state(title)))
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

  let mut children: Vec<Element<'a, Message>> =
    vec![eyebrow(&t!("roster.standings.parsed"), Some(color::text::tertiary()))];
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

  let raw_value = format!("{}{:.2}", if row.raw >= 0.0 { "+" } else { "" }, row.raw);
  let raw = text(t!("roster.standings.raw", value => raw_value))
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
  let placeholder = t!("roster.standings.search_placeholder").into_owned();
  let input = text_input(&placeholder, query)
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

fn segmented<'a>(active: StandingsFilter) -> Element<'a, Message> {
  let mut buttons: Vec<Element<'a, Message>> = Vec::with_capacity(StandingsFilter::SEGMENTS.len());
  for (filter, label) in StandingsFilter::SEGMENTS {
    let selected = filter == active;
    let label_color = if selected {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    };
    buttons.push(
      button(
        text(t!(label))
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(move |_| text::Style {
            color: Some(label_color),
          }),
      )
      .padding(Padding {
        top: spacing::UNIT + 1.0,
        right: spacing::SPACE_3,
        bottom: spacing::UNIT + 1.0,
        left: spacing::SPACE_3,
      })
      .on_press(Message::StandingsFilterChanged(filter))
      .style(move |_, status| segment_button_style(selected, status))
      .into(),
    );
  }

  let control = container(Row::with_children(buttons).spacing(2.0))
    .padding(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.08),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    });

  container(control)
    .width(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .into()
}

fn section_heading<'a>(label: &'a str, count: usize, has_filters: bool) -> Element<'a, Message> {
  let meta = if has_filters {
    t!("roster.standings.count_matched", count => count).into_owned()
  } else {
    t!("roster.standings.count_tracked", count => count).into_owned()
  };
  section_header(&t!(label), Some(&meta))
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
      raw: effective,
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
      raw: 3.0,
      region: Some("The Forge".to_owned()),
      system: Some("Jita".to_owned()),
    }
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_each_facet_filter() {
      let rows = vec![
        row(500_001, StandingKind::Faction, "Caldari State", Some(500_001), 5.0),
        row(1_000_001, StandingKind::Corporation, "Caldari Navy", Some(500_001), 4.0),
        row(1_000_100, StandingKind::Corporation, "Doomheim", None, 0.0),
        agent(3_000_001, "Navy Sec Agent", Some(500_001), Some(true)),
      ];
      let catalog = LoadState::Loaded(rows);

      for filter in [
        StandingsFilter::All,
        StandingsFilter::Factions,
        StandingsFilter::Corps,
        StandingsFilter::Agents,
        StandingsFilter::Other,
      ] {
        let _el: Element<'_, Message> = body(&catalog, filter, false, 600.0, 0.0);
      }
    }

    #[test]
    fn it_renders_grouped_sections_in_the_default_view() {
      let rows = vec![
        row(500_001, StandingKind::Faction, "Caldari State", Some(500_001), 5.0),
        row(1_000_001, StandingKind::Corporation, "Caldari Navy", Some(500_001), 4.0),
        row(1_000_100, StandingKind::Corporation, "Doomheim", None, 0.0),
      ];
      let catalog = LoadState::Loaded(rows);

      let _el: Element<'_, Message> = body(&catalog, StandingsFilter::All, false, 600.0, 0.0);
    }

    #[test]
    fn it_renders_the_loading_and_error_and_empty_states() {
      let loading: LoadState<Vec<StandingsRow>> = LoadState::Loading;
      let error: LoadState<Vec<StandingsRow>> = LoadState::Error("boom".to_owned());
      let empty = LoadState::Loaded(Vec::new());

      let _loading: Element<'_, Message> = body(&loading, StandingsFilter::All, false, 600.0, 0.0);
      let _error: Element<'_, Message> = body(&error, StandingsFilter::All, false, 600.0, 0.0);
      let _empty: Element<'_, Message> = body(&empty, StandingsFilter::All, true, 600.0, 0.0);
    }
  }

  mod filter {
    use pretty_assertions::assert_eq;

    use super::*;

    fn catalog() -> Vec<StandingsRow> {
      vec![
        row(500_001, StandingKind::Faction, "Caldari State", Some(500_001), 5.0),
        row(1_000_001, StandingKind::Corporation, "Caldari Navy", Some(500_001), 4.0),
        row(1_000_100, StandingKind::Corporation, "Doomheim", None, 0.0),
        agent(3_000_001, "Navy Sec Agent", Some(500_001), Some(true)),
        agent(3_000_002, "Rogue Agent", None, Some(false)),
      ]
    }

    fn matched(filter: StandingsFilter) -> usize {
      catalog().iter().filter(|row| filter.matches(row)).count()
    }

    #[test]
    fn it_keeps_only_the_other_bucket() {
      assert_eq!(matched(StandingsFilter::Other), 2);
    }

    #[test]
    fn it_passes_everything_for_all() {
      assert_eq!(matched(StandingsFilter::All), 5);
    }

    #[test]
    fn it_surfaces_agents_only_for_all_and_agents() {
      assert!(StandingsFilter::All.surfaces_agents());
      assert!(StandingsFilter::Agents.surfaces_agents());

      assert!(!StandingsFilter::Factions.surfaces_agents());
      assert!(!StandingsFilter::Corps.surfaces_agents());
      assert!(!StandingsFilter::Other.surfaces_agents());
    }
  }

  mod flatten_sections {
    use pretty_assertions::assert_eq;

    use super::*;

    fn labels(items: &[FlatItem<'_>]) -> Vec<&'static str> {
      items
        .iter()
        .filter_map(|item| match item {
          FlatItem::Header {
            label, ..
          } => Some(*label),
          FlatItem::Row {
            ..
          } => None,
        })
        .collect()
    }

    #[test]
    fn it_emits_a_header_then_its_rows_in_section_order() {
      let rows = vec![
        agent(3_000_001, "Navy Sec Agent", Some(500_001), Some(true)),
        row(500_001, StandingKind::Faction, "Caldari State", Some(500_001), 5.0),
        row(1_000_001, StandingKind::Corporation, "Caldari Navy", Some(500_001), 4.0),
        row(1_000_100, StandingKind::Corporation, "Doomheim", None, 0.0),
      ];

      let items = flatten_sections(&rows, StandingsFilter::All);

      assert_eq!(
        labels(&items),
        [
          "roster.standings.section_factions",
          "roster.standings.section_corporations",
          "roster.standings.section_agents",
          "roster.standings.section_other",
        ]
      );
      assert_eq!(items.len(), 4 + 4, "one header per section plus every visible row");
    }
  }

  mod header {
    use super::*;

    #[test]
    fn it_renders_the_search_bar_and_filter() {
      let _el: Element<'_, Message> = header("faction:caldari", StandingsFilter::All, true);
    }
  }

  mod preview_chip {
    use super::*;

    #[test]
    fn it_renders_each_chip_kind() {
      for kind in [ChipKind::Negated, ChipKind::KeyValue, ChipKind::FreeText] {
        let _el: Element<'_, Message> = super::super::preview_chip("label", &kind);
      }
    }
  }

  mod row_view {
    use super::*;

    #[test]
    fn it_renders_a_plain_row_and_an_agent_row() {
      let plain = row(1_000_001, StandingKind::Corporation, "Caldari Navy", Some(500_001), 4.0);
      let agent_row = agent(3_000_001, "Navy Sec Agent", Some(500_001), Some(true));

      let _plain: Element<'_, Message> = super::super::row_view(&plain, false);
      let _agent: Element<'_, Message> = super::super::row_view(&agent_row, true);
    }
  }
}
