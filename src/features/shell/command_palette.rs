use std::sync::OnceLock;

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Id, Row, Space, button, container, opaque, scrollable, svg, text, text_input},
};

use crate::{
  config::Feature,
  features::shell::nav_catalog,
  ui::{
    components::backdrop,
    style::{color, radius, spacing, typography},
  },
};

const INPUT_ID: &str = "command-palette-input";
const MAX_RESULTS: usize = 24;
const PANEL_MAX_HEIGHT: f32 = 360.0;
const PANEL_TOP_OFFSET: f32 = 120.0;
const PANEL_WIDTH: f32 = 560.0;
const ROW_ICON_SIZE: f32 = 17.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
  AddCharacter,
  ComposeMail,
  CreateStockpile,
  ManageContactSyncs,
  ManageSkillPlans,
  OpenSettings,
  SyncNow,
  ToggleHighContrast,
}

impl Command {
  pub const ALL: [Command; 8] = [
    Command::SyncNow,
    Command::OpenSettings,
    Command::AddCharacter,
    Command::ComposeMail,
    Command::CreateStockpile,
    Command::ManageContactSyncs,
    Command::ManageSkillPlans,
    Command::ToggleHighContrast,
  ];

  pub fn required_feature(self) -> Option<Feature> {
    match self {
      Command::ManageContactSyncs => Some(Feature::Contacts),
      _ => None,
    }
  }

  pub fn label(self) -> String {
    match self {
      Command::AddCharacter => t!("shell.command_palette.add_character"),
      Command::ComposeMail => t!("shell.command_palette.compose_mail"),
      Command::CreateStockpile => t!("shell.command_palette.create_stockpile"),
      Command::ManageContactSyncs => t!("shell.command_palette.manage_contact_syncs"),
      Command::ManageSkillPlans => t!("shell.command_palette.manage_skill_plans"),
      Command::OpenSettings => t!("shell.command_palette.open_settings"),
      Command::SyncNow => t!("shell.command_palette.sync_now"),
      Command::ToggleHighContrast => t!("shell.command_palette.toggle_high_contrast"),
    }
    .into_owned()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
  Command(Command),
  Detail(Entity),
  NavTo(nav_catalog::Section, Option<&'static str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entity {
  pub id: i64,
  pub kind: EntityKind,
  pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityKind {
  Character,
  Corporation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
  pub action: Action,
  pub detail: Option<String>,
  pub kind: Kind,
  pub label: String,
}

impl Entry {
  fn icon(&self) -> Option<&'static [u8]> {
    match &self.action {
      Action::Command(_) | Action::Detail(_) => None,
      Action::NavTo(section, None) => Some(section.icon()),
      Action::NavTo(section, Some(id)) => section
        .sub_sections
        .iter()
        .find(|sub| sub.id == *id)
        .map(|sub| sub.icon),
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
  Character,
  Command,
  Corporation,
  Section,
  Tab,
}

impl Kind {
  fn tag(self) -> String {
    match self {
      Kind::Character => t!("shell.command_palette.kind.character"),
      Kind::Command => t!("shell.command_palette.kind.command"),
      Kind::Corporation => t!("shell.command_palette.kind.corporation"),
      Kind::Section => t!("shell.command_palette.kind.section"),
      Kind::Tab => t!("shell.command_palette.kind.tab"),
    }
    .into_owned()
  }
}

#[derive(Clone, Debug, Default)]
pub struct State {
  pub query: String,
  pub selected: usize,
}

pub fn build_entries(
  enabled_features: &[Feature],
  characters: &[(i64, String)],
  corporations: &[(i64, String)],
  query: &str,
) -> Vec<Entry> {
  let needle = query.trim().to_lowercase();
  nav_entries(enabled_features, &needle)
    .into_iter()
    .chain(command_entries(enabled_features, &needle))
    .chain(entity_entries(characters, corporations, &needle))
    .take(MAX_RESULTS)
    .collect()
}

fn nav_entries(enabled_features: &[Feature], needle: &str) -> Vec<Entry> {
  let mut nav = Vec::new();
  for section in nav_catalog::visible_sections(enabled_features) {
    let label = section.label();
    let kicker = section.kicker();
    if matches(needle, &[&label, &kicker]) {
      nav.push(Entry {
        action: Action::NavTo(*section, section.sub_sections.first().map(|sub| sub.id)),
        detail: Some(kicker),
        kind: Kind::Section,
        label: label.clone(),
      });
    }
    for sub in section.sub_sections {
      let sub_label = sub.label();
      if matches(needle, &[&sub_label, &label]) {
        nav.push(Entry {
          action: Action::NavTo(*section, Some(sub.id)),
          detail: Some(label.clone()),
          kind: Kind::Tab,
          label: sub_label,
        });
      }
    }
  }
  nav
}

fn command_entries(enabled_features: &[Feature], needle: &str) -> Vec<Entry> {
  let mut commands = Vec::new();
  for command in Command::ALL {
    if command
      .required_feature()
      .is_some_and(|feature| !enabled_features.contains(&feature))
    {
      continue;
    }
    let label = command.label();
    if matches(needle, &[&label]) {
      commands.push(Entry {
        action: Action::Command(command),
        detail: None,
        kind: Kind::Command,
        label,
      });
    }
  }
  commands
}

fn entity_entries(characters: &[(i64, String)], corporations: &[(i64, String)], needle: &str) -> Vec<Entry> {
  let mut entities = Vec::new();
  for (id, name) in characters {
    if matches(needle, &[name]) {
      entities.push(Entry {
        action: Action::Detail(Entity {
          id: *id,
          kind: EntityKind::Character,
          name: name.clone(),
        }),
        detail: None,
        kind: Kind::Character,
        label: name.clone(),
      });
    }
  }
  for (id, name) in corporations {
    if matches(needle, &[name]) {
      entities.push(Entry {
        action: Action::Detail(Entity {
          id: *id,
          kind: EntityKind::Corporation,
          name: name.clone(),
        }),
        detail: None,
        kind: Kind::Corporation,
        label: name.clone(),
      });
    }
  }
  entities
}

pub fn input_id() -> Id {
  Id::new(INPUT_ID)
}

pub fn view<'a, M>(
  state: &'a State,
  entries: Vec<Entry>,
  on_query: impl Fn(String) -> M + 'a,
  on_select: impl Fn(usize) -> M + Clone + 'a,
  on_activate: impl Fn(usize) -> M + Clone + 'a,
  on_close: M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let search = text_input(placeholder(), &state.query)
    .id(input_id())
    .on_input(on_query)
    .padding(0)
    .size(15.0)
    .font(typography::body::REGULAR)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::tertiary(),
      placeholder: color::text::tertiary(),
      selection: color::accent_muted(),
      value: color::text::PRIMARY,
    });

  let escape_hint = text(t!("shell.command_palette.escape_hint").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let header = container(
    Row::with_children(vec![
      search.into(),
      container(escape_hint)
        .padding(Padding {
          top: 2.0,
          right: 6.0,
          bottom: 2.0,
          left: 6.0,
        })
        .style(|_| container::Style {
          border: Border {
            color: color::rule(),
            radius: radius::SUBTLE.into(),
            width: 1.0,
          },
          ..container::Style::default()
        })
        .into(),
    ])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: spacing::SPACE_6,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_6,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      radius: 0.0.into(),
      width: 0.0,
    },
    ..container::Style::default()
  });

  let header_rule = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    });

  let selected = state.selected;
  let results: Element<'a, M> = if entries.is_empty() {
    container(
      text(t!("shell.command_palette.no_matches").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .into()
  } else {
    let rows: Vec<Element<'a, M>> = entries
      .into_iter()
      .enumerate()
      .map(|(index, entry)| row(entry, index == selected, index, on_select.clone(), on_activate.clone()))
      .collect();
    scrollable(
      Column::with_children(rows)
        .spacing(2.0)
        .width(Length::Fill)
        .padding(spacing::UNIT + 2.0),
    )
    .height(Length::Shrink)
    .into()
  };

  let panel = container(Column::with_children(vec![header.into(), header_rule.into(), results]).width(Length::Fill))
    .width(Length::Fixed(PANEL_WIDTH))
    .max_width(PANEL_WIDTH)
    .max_height(PANEL_MAX_HEIGHT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  // Only the panel is opaque, not the fill container, so a click outside the
  // panel falls through to the backdrop below and closes the palette.
  let centered = container(opaque(panel))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Top)
    .padding(Padding {
      top: PANEL_TOP_OFFSET,
      right: 0.0,
      bottom: 0.0,
      left: 0.0,
    });

  iced::widget::stack![backdrop::backdrop(on_close), centered]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn matches(needle: &str, haystacks: &[&str]) -> bool {
  if needle.is_empty() {
    return true;
  }
  haystacks.iter().any(|hay| hay.to_lowercase().contains(needle))
}

fn placeholder() -> &'static str {
  static PLACEHOLDER: OnceLock<String> = OnceLock::new();
  PLACEHOLDER.get_or_init(|| t!("shell.command_palette.placeholder").into_owned())
}

fn row<'a, M>(
  entry: Entry,
  active: bool,
  index: usize,
  on_select: impl Fn(usize) -> M + 'a,
  on_activate: impl Fn(usize) -> M + 'a,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let icon_color = if active {
    color::accent()
  } else {
    color::text::secondary()
  };

  let icon_bytes = entry.icon();
  let kind_tag = entry.kind.tag();

  let mut cells: Vec<Element<'a, M>> = Vec::new();
  match icon_bytes {
    Some(bytes) => cells.push(
      svg(svg::Handle::from_memory(bytes))
        .width(Length::Fixed(ROW_ICON_SIZE))
        .height(Length::Fixed(ROW_ICON_SIZE))
        .style(move |_, _| svg::Style {
          color: Some(icon_color),
        })
        .into(),
    ),
    None => cells.push(Space::new().width(Length::Fixed(ROW_ICON_SIZE)).into()),
  }

  cells.push(
    text(entry.label)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  );

  if let Some(detail) = entry.detail {
    cells.push(
      text(detail)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  cells.push(Space::new().width(Length::Fill).into());
  cells.push(
    text(kind_tag)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  );

  let content = Row::with_children(cells)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3);

  iced::widget::mouse_area(
    button(content)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2_5,
        right: spacing::SPACE_3,
        bottom: spacing::SPACE_2_5,
        left: spacing::SPACE_3,
      })
      .on_press(on_activate(index))
      .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let background = if active {
          Some(Background::Color(color::with_alpha(color::accent(), 0.12)))
        } else if hovered {
          Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05)))
        } else {
          None
        };
        button::Style {
          background,
          border: Border {
            radius: radius::CONTROL.into(),
            ..Border::default()
          },
          ..button::Style::default()
        }
      }),
  )
  .on_enter(on_select(index))
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn features() -> Vec<Feature> {
    Feature::ALL.to_vec()
  }

  mod build_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_top_level_sections_for_a_blank_query() {
      let entries = build_entries(&features(), &[], &[], "");

      assert!(entries.iter().any(|e| e.kind == Kind::Section && e.label == "Wallet"));
      assert!(entries.iter().any(|e| e.kind == Kind::Tab));
    }

    #[test]
    fn it_matches_a_sub_tab_by_substring() {
      let entries = build_entries(&features(), &[], &[], "budg");

      assert!(
        entries.iter().any(|e| e.kind == Kind::Tab && e.label == "Budget"),
        "the Budget tab must match a partial query"
      );
    }

    #[test]
    fn it_matches_a_curated_command() {
      let entries = build_entries(&features(), &[], &[], "sync");

      assert!(entries.iter().any(|e| e.action == Action::Command(Command::SyncNow)));
    }

    #[test]
    fn it_matches_the_compose_mail_command_by_substring() {
      let entries = build_entries(&features(), &[], &[], "compose");

      assert!(
        entries
          .iter()
          .any(|e| e.action == Action::Command(Command::ComposeMail)),
        "the Compose mail command matches a partial query"
      );
    }

    #[test]
    fn it_matches_the_create_stockpile_command_by_substring() {
      let entries = build_entries(&features(), &[], &[], "stockpile");

      assert!(
        entries
          .iter()
          .any(|e| e.action == Action::Command(Command::CreateStockpile)),
        "the Create stockpile command matches a partial query"
      );
    }

    #[test]
    fn it_matches_the_manage_contact_syncs_command_by_substring() {
      let entries = build_entries(&features(), &[], &[], "contact sync");

      assert!(
        entries
          .iter()
          .any(|e| e.action == Action::Command(Command::ManageContactSyncs)),
        "the Manage contact syncs command matches a partial query"
      );
    }

    #[test]
    fn it_hides_the_manage_contact_syncs_command_when_contacts_is_disabled() {
      let enabled: Vec<Feature> = Feature::ALL.into_iter().filter(|f| *f != Feature::Contacts).collect();

      let entries = build_entries(&enabled, &[], &[], "contact sync");

      assert!(
        !entries
          .iter()
          .any(|e| e.action == Action::Command(Command::ManageContactSyncs)),
        "the Manage contact syncs command is hidden while the Contacts feature is disabled"
      );
    }

    #[test]
    fn it_matches_the_manage_skill_plans_command_by_substring() {
      let entries = build_entries(&features(), &[], &[], "skill plans");

      assert!(
        entries
          .iter()
          .any(|e| e.action == Action::Command(Command::ManageSkillPlans)),
        "the Manage skill plans command matches a partial query"
      );
    }

    #[test]
    fn it_matches_a_character_and_a_corporation() {
      let chars = vec![(42, "Jita Trader".to_owned())];
      let corps = vec![(98_000_001, "Test Corp".to_owned())];

      let by_char = build_entries(&features(), &chars, &corps, "jita");
      let by_corp = build_entries(&features(), &chars, &corps, "test corp");

      assert_eq!(by_char.iter().filter(|e| e.kind == Kind::Character).count(), 1);
      assert_eq!(by_corp.iter().filter(|e| e.kind == Kind::Corporation).count(), 1);
    }

    #[test]
    fn it_ranks_nav_before_commands_before_entities() {
      let chars = vec![(42, "Synapse".to_owned())];
      let entries = build_entries(&features(), &chars, &[], "s");
      let kinds: Vec<Kind> = entries.iter().map(|e| e.kind).collect();

      let first_command = kinds.iter().position(|k| *k == Kind::Command);
      let first_entity = kinds.iter().position(|k| *k == Kind::Character);
      let last_nav = kinds.iter().rposition(|k| matches!(k, Kind::Section | Kind::Tab));

      if let (Some(nav), Some(command)) = (last_nav, first_command) {
        assert!(nav < command, "nav results rank ahead of commands");
      }
      if let (Some(command), Some(entity)) = (first_command, first_entity) {
        assert!(command < entity, "commands rank ahead of entities");
      }
    }

    #[test]
    fn it_caps_the_result_count() {
      let chars: Vec<(i64, String)> = (0..50).map(|n| (n, format!("Pilot {n}"))).collect();
      let entries = build_entries(&features(), &chars, &[], "pilot");

      assert!(entries.len() <= MAX_RESULTS);
    }

    #[test]
    fn it_hides_sections_for_disabled_features() {
      let entries = build_entries(&[], &[], &[], "wallet");

      assert!(
        !entries.iter().any(|e| e.label == "Wallet"),
        "a gated section stays hidden when its feature is off"
      );
    }
  }

  mod command {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lists_every_command_in_all() {
      assert!(Command::ALL.contains(&Command::ComposeMail));
      assert!(Command::ALL.contains(&Command::CreateStockpile));
      assert!(Command::ALL.contains(&Command::ManageContactSyncs));
      assert!(Command::ALL.contains(&Command::ManageSkillPlans));
    }

    #[test]
    fn it_labels_the_detached_window_commands() {
      assert_eq!(Command::ComposeMail.label(), "Compose mail");
      assert_eq!(Command::CreateStockpile.label(), "Create stockpile");
      assert_eq!(Command::ManageContactSyncs.label(), "Manage contact syncs");
      assert_eq!(Command::ManageSkillPlans.label(), "Manage skill plans");
    }
  }

  mod entry {
    use super::*;

    #[test]
    fn it_borrows_a_section_icon_for_a_top_level_nav_entry() {
      let entries = build_entries(&features(), &[], &[], "wallet");
      let section = entries
        .iter()
        .find(|e| e.kind == Kind::Section && e.label == "Wallet")
        .expect("wallet section");

      assert!(section.icon().is_some());
    }

    #[test]
    fn it_has_no_icon_for_a_command() {
      let entry = Entry {
        action: Action::Command(Command::SyncNow),
        detail: None,
        kind: Kind::Command,
        label: "Sync now".to_owned(),
      };

      assert!(entry.icon().is_none());
    }
  }

  mod kind_tag {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_labels_every_kind() {
      assert_eq!(Kind::Character.tag(), "Character");
      assert_eq!(Kind::Command.tag(), "Command");
      assert_eq!(Kind::Corporation.tag(), "Corporation");
      assert_eq!(Kind::Section.tag(), "Section");
      assert_eq!(Kind::Tab.tag(), "Tab");
    }
  }

  mod rendering {
    use super::*;

    fn command_entry() -> Entry {
      Entry {
        action: Action::Command(Command::SyncNow),
        detail: Some("Command".to_owned()),
        kind: Kind::Command,
        label: "Sync now".to_owned(),
      }
    }

    fn nav_entry() -> Entry {
      build_entries(&features(), &[], &[], "wallet")
        .into_iter()
        .find(|entry| entry.kind == Kind::Section)
        .expect("a nav section entry")
    }

    #[test]
    fn it_builds_a_row_for_an_active_entry_with_an_icon() {
      let _row: Element<'_, ()> = row(nav_entry(), true, 0, |_| (), |_| ());
    }

    #[test]
    fn it_builds_a_row_for_an_inactive_iconless_entry() {
      let _row: Element<'_, ()> = row(command_entry(), false, 3, |_| (), |_| ());
    }

    #[test]
    fn it_builds_the_palette_view_with_results() {
      let state = State {
        query: "wallet".to_owned(),
        selected: 0,
      };
      let entries = build_entries(&features(), &[], &[], "wallet");

      let _view: Element<'_, ()> = view(&state, entries, |_| (), |_| (), |_| (), ());
    }

    #[test]
    fn it_builds_the_palette_view_with_no_matches() {
      let state = State {
        query: "zzqqxx-no-such-thing".to_owned(),
        selected: 0,
      };

      let _view: Element<'_, ()> = view(&state, Vec::new(), |_| (), |_| (), |_| (), ());
    }
  }
}
