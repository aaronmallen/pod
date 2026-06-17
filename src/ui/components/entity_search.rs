#![allow(dead_code)]

use std::path::PathBuf;

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, text, text_input},
};

use crate::ui::{
  components::{
    avatar::Avatar,
    chip::Chip,
    icon::Icon,
    text_input::{self as text_input_component, TextInput},
  },
  style::{color, control, radius, spacing, typography},
};

const DROPDOWN_MAX_HEIGHT: f32 = 264.0;
const FIELD_HEIGHT: f32 = 42.0;
const RESULT_AVATAR: f32 = 30.0;
const ROUNDED_RADIUS: f32 = RESULT_AVATAR * 0.2;
const ROUND_RADIUS: f32 = RESULT_AVATAR;
const ROW_LIMIT: usize = 7;
const SEARCH_MIN_CHARS: usize = 3;
const VALUE_AVATAR: f32 = 36.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
  Alliance,
  Character,
  Corporation,
}

impl EntityKind {
  pub fn avatar_radius(self) -> f32 {
    match self {
      Self::Character => ROUND_RADIUS,
      _ => ROUNDED_RADIUS,
    }
  }

  pub fn is_round(self) -> bool {
    matches!(self, Self::Character)
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::Alliance => "Alliance",
      Self::Character => "Character",
      Self::Corporation => "Corporation",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRef {
  pub id: i64,
  pub kind: EntityKind,
  pub name: String,
  pub portrait: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EntitySearch {
  generation: u64,
  query: String,
  results: Vec<EntityRef>,
  searching: bool,
}

impl EntitySearch {
  /// Installs search results if `generation` matches the current one; returns `false` and discards
  /// results that arrive from a superseded query.
  pub fn accept_results(&mut self, generation: u64, results: Vec<EntityRef>) -> bool {
    if generation != self.generation {
      return false;
    }

    self.results = results;
    self.searching = false;
    true
  }

  pub fn clear(&mut self) {
    self.generation = self.generation.wrapping_add(1);
    self.query.clear();
    self.results.clear();
    self.searching = false;
  }

  pub fn generation(&self) -> u64 {
    self.generation
  }

  pub fn query(&self) -> &str {
    &self.query
  }

  pub fn results(&self) -> &[EntityRef] {
    &self.results
  }

  pub fn searching(&self) -> bool {
    self.searching
  }

  /// Updates the query, bumps the generation counter, and returns the new generation.
  ///
  /// Callers should tag their async search request with the returned generation and pass it back
  /// to `accept_results` so stale responses are discarded.
  pub fn set_query(&mut self, query: String) -> u64 {
    self.generation = self.generation.wrapping_add(1);
    let active = query.trim().chars().count() >= SEARCH_MIN_CHARS;
    self.query = query;
    if active {
      self.searching = true;
    } else {
      self.results.clear();
      self.searching = false;
    }
    self.generation
  }
}

pub struct MultiSelect<'a, M> {
  chips: &'a [EntityRef],
  exclude: &'a [String],
  inline: bool,
  on_input: Box<dyn Fn(String) -> M + 'a>,
  on_pick: Box<dyn Fn(EntityRef) -> M + 'a>,
  on_remove: Box<dyn Fn(usize) -> M + 'a>,
  on_submit: Option<M>,
  placeholder: &'a str,
  query: &'a str,
  results: &'a [EntityRef],
  searching: bool,
}

impl<'a, M: Clone + 'static> MultiSelect<'a, M> {
  pub fn new(
    query: &'a str,
    chips: &'a [EntityRef],
    results: &'a [EntityRef],
    on_input: impl Fn(String) -> M + 'a,
    on_pick: impl Fn(EntityRef) -> M + 'a,
    on_remove: impl Fn(usize) -> M + 'a,
  ) -> Self {
    Self {
      chips,
      exclude: &[],
      inline: false,
      on_input: Box::new(on_input),
      on_pick: Box::new(on_pick),
      on_remove: Box::new(on_remove),
      on_submit: None,
      placeholder: "Search entities\u{2026}",
      query,
      results,
      searching: false,
    }
  }

  pub fn exclude(mut self, names: &'a [String]) -> Self {
    self.exclude = names;
    self
  }

  /// Renders the chips and input bare (no bordered field box), for hosts that already supply their own field
  /// chrome — e.g. the mail compose To/Cc rows, which place the recipient picker inline inside a labelled field row.
  pub fn inline(mut self, inline: bool) -> Self {
    self.inline = inline;
    self
  }

  pub fn on_submit(mut self, message: M) -> Self {
    self.on_submit = Some(message);
    self
  }

  pub fn placeholder(mut self, placeholder: &'a str) -> Self {
    self.placeholder = placeholder;
    self
  }

  pub fn searching(mut self, searching: bool) -> Self {
    self.searching = searching;
    self
  }

  pub fn view(self) -> Element<'a, M> {
    let mut chips = Row::new().spacing(spacing::UNIT + 2.0).align_y(Vertical::Center);
    for (index, chip) in self.chips.iter().enumerate() {
      chips = chips.push(
        Chip::new(chip.name.clone(), None)
          .on_remove((self.on_remove)(index))
          .view(),
      );
    }

    let input: Element<'a, M> = if self.inline {
      let mut input = text_input(self.placeholder, self.query)
        .on_input(self.on_input)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .padding(0.0)
        .width(Length::Fill)
        .style(text_input_component::inner_style());
      if let Some(message) = self.on_submit {
        input = input.on_submit(message);
      }
      input.into()
    } else {
      let mut input = TextInput::new(self.placeholder, self.query, self.on_input)
        .leading_icon(Icon::search())
        .font_size(typography::size::MD)
        .width(Length::Fill);
      if let Some(message) = self.on_submit {
        input = input.on_submit(message);
      }
      input.render()
    };
    chips = chips.push(input);

    let chosen_names: Vec<&str> = self.chips.iter().map(|chip| chip.name.as_str()).collect();
    let dropdown = dropdown(
      self.results,
      &chosen_names,
      self.exclude,
      self.searching,
      searchable(self.query),
      self.on_pick,
    );

    let field = if self.inline {
      chips.into()
    } else {
      field(chips.into(), false)
    };

    Column::with_children(vec![field, dropdown]).width(Length::Fill).into()
  }
}

pub struct SingleSelect<'a, M> {
  exclude: &'a [String],
  on_change: Box<dyn Fn(Option<EntityRef>) -> M + 'a>,
  on_input: Box<dyn Fn(String) -> M + 'a>,
  open: bool,
  placeholder: &'a str,
  query: &'a str,
  results: &'a [EntityRef],
  searching: bool,
  value: Option<&'a EntityRef>,
}

impl<'a, M: Clone + 'static> SingleSelect<'a, M> {
  pub fn new(
    query: &'a str,
    value: Option<&'a EntityRef>,
    results: &'a [EntityRef],
    on_input: impl Fn(String) -> M + 'a,
    on_change: impl Fn(Option<EntityRef>) -> M + 'a,
  ) -> Self {
    Self {
      exclude: &[],
      on_change: Box::new(on_change),
      on_input: Box::new(on_input),
      open: true,
      placeholder: "Search entities\u{2026}",
      query,
      results,
      searching: false,
      value,
    }
  }

  pub fn exclude(mut self, names: &'a [String]) -> Self {
    self.exclude = names;
    self
  }

  pub fn open(mut self, open: bool) -> Self {
    self.open = open;
    self
  }

  pub fn placeholder(mut self, placeholder: &'a str) -> Self {
    self.placeholder = placeholder;
    self
  }

  pub fn searching(mut self, searching: bool) -> Self {
    self.searching = searching;
    self
  }

  pub fn view(self) -> Element<'a, M> {
    if let Some(value) = self.value {
      return chosen_card(value, (self.on_change)(None));
    }

    let input = TextInput::new(self.placeholder, self.query, self.on_input)
      .leading_icon(Icon::search())
      .font_size(typography::size::MD)
      .width(Length::Fill);

    let dropdown = if self.open {
      dropdown(
        self.results,
        &[],
        self.exclude,
        self.searching,
        searchable(self.query),
        move |entity| (self.on_change)(Some(entity)),
      )
    } else {
      Space::new().width(Length::Shrink).height(Length::Shrink).into()
    };

    Column::with_children(vec![field(input.render(), self.open), dropdown])
      .width(Length::Fill)
      .into()
  }
}

fn chosen_card<'a, M: Clone + 'a>(value: &'a EntityRef, on_clear: M) -> Element<'a, M> {
  let identity = Column::with_children(vec![
    text(value.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    type_label(value.kind),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let change = button(
    text("Change")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    bottom: 6.0,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
    top: 6.0,
  })
  .on_press(on_clear)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if hover { color::rule_strong() } else { color::rule() },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..button::Style::default()
    }
  });

  container(
    Row::with_children(vec![entity_avatar(value, VALUE_AVATAR), identity.into(), change.into()])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    bottom: 11.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
    top: 11.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.35),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn dropdown<'a, M: Clone + 'a>(
  results: &'a [EntityRef],
  chosen: &[&str],
  exclude: &[String],
  searching: bool,
  searchable: bool,
  on_pick: impl Fn(EntityRef) -> M + 'a,
) -> Element<'a, M> {
  let matches: Vec<&'a EntityRef> = results
    .iter()
    .filter(|entity| !chosen.iter().any(|name| *name == entity.name))
    .filter(|entity| !exclude.iter().any(|name| name == &entity.name))
    .take(ROW_LIMIT)
    .collect();

  if matches.is_empty() {
    if searching {
      return wrap_dropdown(status_row("Searching\u{2026}"));
    }
    // Suppress the dropdown entirely until the query is long enough to search; only a real, completed search that
    // returned nothing surfaces "No matches", so the resting/just-opened picker shows no stray empty panel.
    if searchable {
      return wrap_dropdown(no_matches());
    }
    return Space::new().width(Length::Shrink).height(Length::Shrink).into();
  }

  let mut column = Column::new().width(Length::Fill);
  for entity in matches {
    column = column.push(result_row(entity, &on_pick));
  }

  wrap_dropdown(
    scrollable(column)
      .height(Length::Shrink)
      .style(control::scrollbar)
      .into(),
  )
}

fn entity_avatar<'a, M: 'a>(entity: &EntityRef, size: f32) -> Element<'a, M> {
  Avatar::new(
    entity.id,
    entity.name.clone(),
    Length::Fixed(size),
    size,
    entity.portrait.clone(),
  )
  .radius(entity.kind.avatar_radius())
  .view()
}

fn field<'a, M: 'a>(content: Element<'a, M>, focused: bool) -> Element<'a, M> {
  let border = if focused { color::accent::PLASMA } else { color::rule() };

  container(content)
    .width(Length::Fill)
    .height(Length::Fixed(FIELD_HEIGHT))
    .align_y(Vertical::Center)
    .padding(Padding {
      bottom: 0.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
      top: 0.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: border,
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn no_matches<'a, M: 'a>() -> Element<'a, M> {
  container(
    text("No matches")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Center)
  .padding(Padding {
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
    top: spacing::SPACE_3,
  })
  .into()
}

fn result_row<'a, M: Clone + 'a>(entity: &'a EntityRef, on_pick: &impl Fn(EntityRef) -> M) -> Element<'a, M> {
  let identity = Column::with_children(vec![
    text(entity.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    type_label(entity.kind),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![entity_avatar(entity, RESULT_AVATAR), identity.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

  mouse_area(container(row).width(Length::Fill).padding(Padding {
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
    top: spacing::SPACE_2,
  }))
  .on_press(on_pick(entity.clone()))
  .into()
}

/// Whether `query` is long enough for a search to have run. Mirrors the `SEARCH_MIN_CHARS` gate in
/// [`EntitySearch::set_query`] so the dropdown stays empty (rather than showing "No matches") until a real search
/// could have produced results.
fn searchable(query: &str) -> bool {
  query.trim().chars().count() >= SEARCH_MIN_CHARS
}

fn status_row<'a, M: 'a>(label: &str) -> Element<'a, M> {
  container(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
    top: spacing::SPACE_2,
  })
  .into()
}

fn type_label<'a, M: 'a>(kind: EntityKind) -> Element<'a, M> {
  text(kind.label().to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into()
}

fn wrap_dropdown<'a, M: 'a>(content: Element<'a, M>) -> Element<'a, M> {
  container(
    container(content)
      .width(Length::Fill)
      .max_height(DROPDOWN_MAX_HEIGHT)
      .padding(spacing::UNIT)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::rule_strong(),
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    bottom: 0.0,
    left: 0.0,
    right: 0.0,
    top: spacing::UNIT + 2.0,
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug, Eq, PartialEq)]
  enum Message {
    Changed(Option<EntityRef>),
    Input(String),
    Picked(EntityRef),
    Removed(usize),
  }

  fn sample(kind: EntityKind, id: i64, name: &str) -> EntityRef {
    EntityRef {
      id,
      kind,
      name: name.to_owned(),
      portrait: None,
    }
  }

  mod entity_kind {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rounds_characters_and_squares_other_kinds() {
      assert!(EntityKind::Character.is_round());
      assert!(!EntityKind::Corporation.is_round());
      assert!(!EntityKind::Alliance.is_round());

      assert_eq!(EntityKind::Character.avatar_radius(), ROUND_RADIUS);
      assert_eq!(EntityKind::Corporation.avatar_radius(), ROUNDED_RADIUS);
    }
  }

  mod entity_search {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_results_only_for_the_current_generation() {
      let mut search = EntitySearch::default();
      let stale = search.set_query("Vex".to_owned());
      let current = search.set_query("Vexor".to_owned());

      let accepted_stale = search.accept_results(stale, vec![sample(EntityKind::Character, 1, "Stale")]);
      let accepted_current = search.accept_results(current, vec![sample(EntityKind::Character, 2, "Current")]);

      assert!(!accepted_stale);
      assert!(accepted_current);
      assert_eq!(search.results(), &[sample(EntityKind::Character, 2, "Current")]);
      assert!(!search.searching());
    }

    #[test]
    fn it_bumps_the_generation_and_marks_searching_above_min_chars() {
      let mut search = EntitySearch::default();

      let generation = search.set_query("Vex".to_owned());

      assert_eq!(generation, 1);
      assert_eq!(search.generation(), 1);
      assert_eq!(search.query(), "Vex");
      assert!(search.searching());
    }

    #[test]
    fn it_clears_query_results_and_bumps_the_generation() {
      let mut search = EntitySearch::default();
      let generation = search.set_query("Vex".to_owned());
      search.accept_results(generation, vec![sample(EntityKind::Character, 95, "Vex")]);

      search.clear();

      assert_eq!(search.query(), "");
      assert!(search.results().is_empty());
      assert!(!search.searching());
      assert_eq!(search.generation(), 2);
    }

    #[test]
    fn it_clears_results_below_min_chars_without_searching() {
      let mut search = EntitySearch::default();
      let generation = search.set_query("Vex".to_owned());
      search.accept_results(generation, vec![sample(EntityKind::Character, 95, "Vex")]);

      search.set_query("Ve".to_owned());

      assert!(search.results().is_empty());
      assert!(!search.searching());
    }
  }

  mod multi_select {
    use super::*;

    #[test]
    fn it_renders_a_resting_field_without_a_no_matches_panel() {
      let chips: Vec<EntityRef> = Vec::new();
      let results: Vec<EntityRef> = Vec::new();

      let _el: Element<'_, Message> = MultiSelect::new(
        "Ve",
        &chips,
        &results,
        Message::Input,
        Message::Picked,
        Message::Removed,
      )
      .searching(false)
      .view();
    }

    #[test]
    fn it_renders_an_empty_recipient_field() {
      let chips: Vec<EntityRef> = Vec::new();
      let results: Vec<EntityRef> = Vec::new();

      let _el: Element<'_, Message> =
        MultiSelect::new("", &chips, &results, Message::Input, Message::Picked, Message::Removed).view();
    }

    #[test]
    fn it_renders_recipient_chips_and_results() {
      let chips = vec![sample(EntityKind::Character, 95, "Vex Voronova")];
      let results = vec![sample(EntityKind::Corporation, 96, "Vex Holdings")];

      let _el: Element<'_, Message> = MultiSelect::new(
        "Vex",
        &chips,
        &results,
        Message::Input,
        Message::Picked,
        Message::Removed,
      )
      .searching(false)
      .view();
    }

    #[test]
    fn it_renders_the_searching_status() {
      let chips: Vec<EntityRef> = Vec::new();
      let results: Vec<EntityRef> = Vec::new();

      let _el: Element<'_, Message> = MultiSelect::new(
        "Vex",
        &chips,
        &results,
        Message::Input,
        Message::Picked,
        Message::Removed,
      )
      .searching(true)
      .view();
    }
  }

  mod searchable {
    use super::*;

    #[test]
    fn it_is_false_until_the_query_reaches_the_minimum_length() {
      assert!(!searchable(""));
      assert!(!searchable("  "));
      assert!(!searchable("Ve"));

      assert!(searchable("Vex"));
      assert!(searchable("  Vex  "));
    }
  }

  mod single_select {
    use super::*;

    #[test]
    fn it_excludes_names_already_in_use() {
      let results = vec![sample(EntityKind::Character, 95, "Vex Voronova")];
      let exclude = vec!["Vex Voronova".to_owned()];

      let _el: Element<'_, Message> = SingleSelect::new("Vex", None, &results, Message::Input, Message::Changed)
        .exclude(&exclude)
        .view();
    }

    #[test]
    fn it_renders_no_matches_only_after_a_searchable_query_returns_nothing() {
      let results: Vec<EntityRef> = Vec::new();

      let _el: Element<'_, Message> = SingleSelect::new("Zzz", None, &results, Message::Input, Message::Changed)
        .open(true)
        .view();
    }

    #[test]
    fn it_renders_results() {
      let results = vec![
        sample(EntityKind::Character, 95, "Vex Voronova"),
        sample(EntityKind::Corporation, 96, "Vex Holdings"),
      ];

      let _el: Element<'_, Message> = SingleSelect::new("Vex", None, &results, Message::Input, Message::Changed).view();
    }

    #[test]
    fn it_renders_the_chosen_card_when_a_value_is_set() {
      let value = sample(EntityKind::Character, 95, "Vex Voronova");
      let results: Vec<EntityRef> = Vec::new();

      let _el: Element<'_, Message> =
        SingleSelect::new("", Some(&value), &results, Message::Input, Message::Changed).view();
    }

    #[test]
    fn it_renders_the_empty_search_field() {
      let results: Vec<EntityRef> = Vec::new();

      let _el: Element<'_, Message> = SingleSelect::new("", None, &results, Message::Input, Message::Changed).view();
    }

    #[test]
    fn it_suppresses_the_dropdown_for_a_resting_short_query() {
      let results: Vec<EntityRef> = Vec::new();

      let _el: Element<'_, Message> = SingleSelect::new("Ve", None, &results, Message::Input, Message::Changed)
        .open(true)
        .view();
    }
  }
}
