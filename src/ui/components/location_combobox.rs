#![allow(dead_code)]

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use crate::{
  features::assets::{LocationRef, LocationTier},
  ui::{
    components::{count_badge::count_badge, icon::Icon, text_input::TextInput},
    style::{color, control, radius, spacing, typography},
  },
};

const LIST_HEIGHT: f32 = 230.0;
const PILL_RADIUS: f32 = 3.0;
const SEARCH_MIN_CHARS: usize = 3;
/// Wormhole-band tint (design `#B98BD9`) for J-space security pills.
const WORMHOLE: Color = Color {
  r: 0.725,
  g: 0.545,
  b: 0.851,
  a: 1.0,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationSearch {
  generation: u64,
  highlight: Option<usize>,
  query: String,
  results: Vec<LocationRef>,
  searching: bool,
}

impl LocationSearch {
  /// Installs search results if `generation` matches the current one; returns `false` and discards
  /// results that arrive from a superseded query.
  pub fn accept_results(&mut self, generation: u64, results: Vec<LocationRef>) -> bool {
    if generation != self.generation {
      return false;
    }

    self.highlight = None;
    self.results = results;
    self.searching = false;
    true
  }

  pub fn clear(&mut self) {
    self.generation = self.generation.wrapping_add(1);
    self.highlight = None;
    self.query.clear();
    self.results.clear();
    self.searching = false;
  }

  pub fn generation(&self) -> u64 {
    self.generation
  }

  pub fn highlight(&self) -> Option<usize> {
    self.highlight
  }

  pub fn highlight_next(&mut self) {
    if self.results.is_empty() {
      self.highlight = None;
      return;
    }
    self.highlight = Some(match self.highlight {
      Some(index) if index + 1 < self.results.len() => index + 1,
      Some(index) => index,
      None => 0,
    });
  }

  pub fn highlight_prev(&mut self) {
    if self.results.is_empty() {
      self.highlight = None;
      return;
    }
    self.highlight = Some(match self.highlight {
      Some(0) | None => 0,
      Some(index) => index - 1,
    });
  }

  pub fn highlighted(&self) -> Option<&LocationRef> {
    self.highlight.and_then(|index| self.results.get(index))
  }

  pub fn query(&self) -> &str {
    &self.query
  }

  pub fn results(&self) -> &[LocationRef] {
    &self.results
  }

  pub fn searchable(&self) -> bool {
    searchable(&self.query)
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
    self.highlight = None;
    let active = searchable(&query);
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

pub struct LocationCombobox<'a, M> {
  highlight: Option<usize>,
  on_clear: Option<M>,
  on_input: Option<Box<dyn Fn(String) -> M + 'a>>,
  on_pick: Option<Box<dyn Fn(LocationRef) -> M + 'a>>,
  on_toggle: Option<M>,
  placeholder: &'a str,
  query: &'a str,
  results: Vec<LocationRef>,
  searching: bool,
  selection: Option<LocationRef>,
  width: Length,
}

impl<M: Clone + 'static> Default for LocationCombobox<'_, M> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a, M: Clone + 'static> LocationCombobox<'a, M> {
  pub fn new() -> Self {
    Self {
      highlight: None,
      on_clear: None,
      on_input: None,
      on_pick: None,
      on_toggle: None,
      placeholder: "Search any location\u{2026}",
      query: "",
      results: Vec::new(),
      searching: false,
      selection: None,
      width: Length::Fill,
    }
  }

  pub fn highlight(mut self, highlight: Option<usize>) -> Self {
    self.highlight = highlight;
    self
  }

  pub fn on_clear(mut self, message: M) -> Self {
    self.on_clear = Some(message);
    self
  }

  /// Sets the search-input handler used by [`LocationCombobox::popover`].
  pub fn on_input(mut self, on_input: impl Fn(String) -> M + 'a) -> Self {
    self.on_input = Some(Box::new(on_input));
    self
  }

  /// Sets the row-selection handler used by [`LocationCombobox::popover`].
  pub fn on_pick(mut self, on_pick: impl Fn(LocationRef) -> M + 'a) -> Self {
    self.on_pick = Some(Box::new(on_pick));
    self
  }

  /// Sets the message emitted when the [`LocationCombobox::trigger`] button is pressed (open/close).
  pub fn on_toggle(mut self, message: M) -> Self {
    self.on_toggle = Some(message);
    self
  }

  pub fn placeholder(mut self, placeholder: &'a str) -> Self {
    self.placeholder = placeholder;
    self
  }

  pub fn query(mut self, query: &'a str) -> Self {
    self.query = query;
    self
  }

  pub fn results(mut self, results: Vec<LocationRef>) -> Self {
    self.results = results;
    self
  }

  pub fn searching(mut self, searching: bool) -> Self {
    self.searching = searching;
    self
  }

  pub fn selection(mut self, selection: Option<LocationRef>) -> Self {
    self.selection = selection;
    self
  }

  pub fn width(mut self, width: Length) -> Self {
    self.width = width;
    self
  }

  /// The always-visible field showing the selected location as a two-line card (or the empty placeholder).
  /// Pressing it toggles the [`LocationCombobox::popover`] open; the popover (not this button) owns the search input.
  pub fn trigger(self) -> Element<'a, M> {
    let card: Element<'a, M> = match &self.selection {
      Some(location) => selected_card(location),
      None => empty_card(self.placeholder),
    };

    let row = Row::new()
      .spacing(spacing::SPACE_3_5)
      .align_y(Vertical::Center)
      .width(Length::Fill)
      .push(card)
      .push(Icon::chevron().color(color::text::secondary()).size(14.0).render::<M>());

    let mut field = button(row).width(self.width).padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    });
    if let Some(message) = self.on_toggle {
      field = field.on_press(message);
    }
    field
      .style(|_, status| {
        let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
          background: Some(Background::Color(color::surface::SUNKEN)),
          border: Border {
            color: if active {
              color::accent::PLASMA
            } else {
              color::rule_strong()
            },
            radius: radius::CONTROL.into(),
            width: 1.0,
          },
          text_color: color::text::PRIMARY,
          ..button::Style::default()
        }
      })
      .into()
  }

  /// The floating results popover: a search input, a result-count chip, the result rows, and an optional
  /// clear footer. Hosts anchor this under the [`LocationCombobox::trigger`] button so the two read as one
  /// combobox.
  pub fn popover(self) -> Element<'a, M> {
    let Self {
      highlight,
      on_clear,
      on_input,
      on_pick,
      placeholder,
      query,
      results,
      searching,
      selection,
      width,
      ..
    } = self;

    let search: Element<'a, M> = match on_input {
      Some(on_input) => TextInput::new(placeholder, query, on_input)
        .leading_icon(Icon::search().color(color::text::secondary()))
        .background(color::surface::SUNKEN)
        .width(Length::Fill)
        .render(),
      None => Space::new().into(),
    };

    let header = Row::with_children(vec![
      text("Locations")
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Space::new().width(Length::Fill).into(),
      count_badge(results.len() as i64, color::accent::PLASMA),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

    let selected_id = selection.as_ref().map(|location| location.id);
    let rows: Vec<Element<'a, M>> = match &on_pick {
      Some(on_pick) => results
        .iter()
        .enumerate()
        .map(|(index, location)| {
          let selected = selected_id == Some(location.id);
          result_row(location, highlight == Some(index), selected, &**on_pick)
        })
        .collect(),
      None => Vec::new(),
    };

    let list: Element<'a, M> = if rows.is_empty() {
      centered(status_label(if searching {
        "Searching\u{2026}"
      } else if searchable(query) {
        "No locations found."
      } else {
        "Type to search any location."
      }))
    } else {
      scrollable(Column::with_children(rows).spacing(spacing::UNIT).width(Length::Fill))
        .style(control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fixed(LIST_HEIGHT))
        .into()
    };

    let mut body: Vec<Element<'a, M>> = vec![search, header.into(), list];
    if let Some(message) = on_clear {
      body.push(footer(message));
    }

    container(
      Column::with_children(body)
        .spacing(spacing::SPACE_2)
        .width(Length::Fill),
    )
    .width(width)
    .padding(spacing::SPACE_2)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }
}

fn centered<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
  container(content)
    .width(Length::Fill)
    .height(Length::Fixed(LIST_HEIGHT))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn context_label(location: &LocationRef) -> Option<String> {
  location
    .context
    .as_deref()
    .map(str::trim)
    .filter(|context| !context.is_empty())
    .map(str::to_owned)
}

fn empty_card<'a, M: 'a>(placeholder: &str) -> Element<'a, M> {
  container(
    text(placeholder.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .clip(true)
  .into()
}

fn footer<'a, M: Clone + 'a>(on_clear: M) -> Element<'a, M> {
  button(
    text("Clear selection")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
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
      text_color: color::text::secondary(),
      ..button::Style::default()
    }
  })
  .into()
}

fn result_row<'a, M: Clone + 'a>(
  location: &LocationRef,
  highlighted: bool,
  selected: bool,
  on_pick: &dyn Fn(LocationRef) -> M,
) -> Element<'a, M> {
  let name = text(location.name.clone())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(if selected {
      color::accent::PLASMA
    } else {
      color::text::PRIMARY
    }))
    .width(Length::Fill);

  let mut heading = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .push(tier_tag(location.tier))
    .push(name);
  if let Some(pill) = sec_pill(location.tier, location.security_status) {
    heading = heading.push(pill);
  }

  let mut details = Column::new().spacing(spacing::UNIT).width(Length::Fill).push(heading);
  if let Some(context) = context_label(location) {
    details = details.push(
      text(context)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }

  button(details)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .on_press(on_pick(location.clone()))
    .style(move |_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      let lit = highlighted || hover || selected;
      button::Style {
        background: lit.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
        border: Border {
          radius: radius::CONTROL.into(),
          ..Border::default()
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      }
    })
    .into()
}

fn searchable(query: &str) -> bool {
  query.trim().chars().count() >= SEARCH_MIN_CHARS
}

/// Renders a security pill only for tiers that carry a security band (system and below); region and
/// constellation tiers are suppressed even when a `security_status` value is present.
fn sec_pill<'a, M: 'a>(tier: Option<LocationTier>, security_status: Option<f64>) -> Option<Element<'a, M>> {
  if !tier.is_some_and(LocationTier::has_security) {
    return None;
  }
  let sec = security_status?;
  let (label, band) = if sec <= -0.9 {
    ("J-space".to_owned(), WORMHOLE)
  } else if sec <= 0.0 {
    (format!("{sec:.1}"), color::status::DANGER)
  } else if sec < 0.5 {
    (format!("{sec:.1}"), color::status::WARNING)
  } else {
    (format!("{sec:.1}"), color::status::ONLINE)
  };

  Some(
    container(
      text(label)
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(band)),
    )
    .padding(Padding {
      top: 1.0,
      bottom: 1.0,
      left: spacing::UNIT + 2.0,
      right: spacing::UNIT + 2.0,
    })
    .style(move |_| container::Style {
      border: Border {
        color: color::with_alpha(band, 0.5),
        radius: PILL_RADIUS.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into(),
  )
}

fn selected_card<'a, M: 'a>(location: &LocationRef) -> Element<'a, M> {
  let mut heading = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .push(tier_tag(location.tier))
    .push(
      text(location.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(color::text::PRIMARY))
        .width(Length::Fill), // bounded width is required for iced to wrap long names instead of clipping
    );
  if let Some(pill) = sec_pill(location.tier, location.security_status) {
    heading = heading.push(pill);
  }

  let mut details = Column::new().spacing(spacing::UNIT).width(Length::Fill).push(heading);
  if let Some(context) = context_label(location) {
    details = details.push(
      text(context)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }

  details.into()
}

fn status_label<'a, M: 'a>(label: &str) -> Element<'a, M> {
  text(label.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

/// The colored band assigned to each location tier so the leading level tag is scannable at a glance.
fn tier_color(tier: Option<LocationTier>) -> Color {
  match tier {
    Some(LocationTier::Region) => color::chart::GOLD,
    Some(LocationTier::Constellation) => color::chart::VIOLET,
    Some(LocationTier::System) => color::status::ONLINE,
    Some(LocationTier::Station) => color::accent::PLASMA,
    Some(LocationTier::Structure) => color::status::WARNING,
    None => color::text::tertiary(),
  }
}

fn tier_tag<'a, M: 'a>(tier: Option<LocationTier>) -> Element<'a, M> {
  let Some(tier) = tier else {
    return Space::new().into();
  };
  let band = tier_color(Some(tier));

  container(
    text(tier.label())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(band)),
  )
  .padding(Padding {
    top: 1.0,
    bottom: 1.0,
    left: spacing::UNIT + 2.0,
    right: spacing::UNIT + 2.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(band, 0.12))),
    border: Border {
      color: color::with_alpha(band, 0.5),
      radius: PILL_RADIUS.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug, Eq, PartialEq)]
  enum Message {
    Cleared,
    Input(String),
    Picked(i64),
  }

  fn sample(id: i64, name: &str, tier: LocationTier) -> LocationRef {
    LocationRef {
      context: Some("The Forge \u{00B7} Jita".to_owned()),
      id,
      name: name.to_owned(),
      security_status: Some(0.9),
      tier: Some(tier),
    }
  }

  mod location_combobox {
    use super::*;

    #[test]
    fn it_renders_a_popover_with_results_and_a_footer() {
      let results = vec![
        sample(10_000_002, "The Forge", LocationTier::Region),
        sample(30_000_142, "Jita", LocationTier::System),
        sample(1_000_000_000_000, "Jita Trade Hub", LocationTier::Structure),
      ];

      let _el: Element<'_, Message> = LocationCombobox::new()
        .query("Jita")
        .results(results)
        .on_input(Message::Input)
        .on_pick(|l| Message::Picked(l.id))
        .highlight(Some(0))
        .on_clear(Message::Cleared)
        .popover();
    }

    #[test]
    fn it_renders_a_trigger_with_a_selection() {
      let _el: Element<'_, Message> = LocationCombobox::new()
        .selection(Some(sample(
          60_003_760,
          "Jita IV - Moon 4 - CNAP",
          LocationTier::Station,
        )))
        .on_toggle(Message::Cleared)
        .trigger();
    }

    #[test]
    fn it_renders_an_empty_trigger() {
      let _el: Element<'_, Message> = LocationCombobox::new()
        .placeholder("Pick a location")
        .on_toggle(Message::Cleared)
        .trigger();
    }

    #[test]
    fn it_renders_the_searching_state() {
      let _el: Element<'_, Message> = LocationCombobox::new()
        .query("Jita")
        .on_input(Message::Input)
        .on_pick(|l| Message::Picked(l.id))
        .searching(true)
        .popover();
    }
  }

  mod location_search {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_results_only_for_the_current_generation() {
      let mut search = LocationSearch::default();
      let stale = search.set_query("Jita".to_owned());
      let current = search.set_query("Jita IV".to_owned());

      let accepted_stale = search.accept_results(stale, vec![sample(30_000_142, "Stale", LocationTier::System)]);
      let accepted_current = search.accept_results(current, vec![sample(30_000_142, "Current", LocationTier::System)]);

      assert!(!accepted_stale);
      assert!(accepted_current);
      assert_eq!(search.results(), &[sample(30_000_142, "Current", LocationTier::System)]);
      assert!(!search.searching());
    }

    #[test]
    fn it_bumps_the_generation_and_marks_searching_above_min_chars() {
      let mut search = LocationSearch::default();

      let generation = search.set_query("Jita".to_owned());

      assert_eq!(generation, 1);
      assert_eq!(search.generation(), 1);
      assert_eq!(search.query(), "Jita");
      assert!(search.searching());
    }

    #[test]
    fn it_clears_results_below_min_chars_without_searching() {
      let mut search = LocationSearch::default();
      let generation = search.set_query("Jita".to_owned());
      search.accept_results(generation, vec![sample(30_000_142, "Jita", LocationTier::System)]);

      search.set_query("Ji".to_owned());

      assert!(search.results().is_empty());
      assert!(!search.searching());
    }

    #[test]
    fn it_steps_the_highlight_within_result_bounds() {
      let mut search = LocationSearch::default();
      let generation = search.set_query("Jita".to_owned());
      search.accept_results(
        generation,
        vec![
          sample(30_000_142, "A", LocationTier::System),
          sample(30_000_144, "B", LocationTier::System),
        ],
      );

      search.highlight_next();
      assert_eq!(search.highlight(), Some(0));

      search.highlight_next();
      assert_eq!(search.highlight(), Some(1));

      search.highlight_next();
      assert_eq!(search.highlight(), Some(1));

      search.highlight_prev();
      assert_eq!(search.highlight(), Some(0));
    }
  }

  mod sec_pill {
    use super::*;

    #[test]
    fn it_renders_nothing_for_a_region() {
      let pill: Option<Element<'_, Message>> = super::super::sec_pill(Some(LocationTier::Region), Some(0.9));

      assert!(pill.is_none());
    }

    #[test]
    fn it_renders_nothing_without_a_security_status() {
      let pill: Option<Element<'_, Message>> = super::super::sec_pill(Some(LocationTier::System), None);

      assert!(pill.is_none());
    }

    #[test]
    fn it_renders_a_pill_for_a_dockable_tier() {
      let pill: Option<Element<'_, Message>> = super::super::sec_pill(Some(LocationTier::Structure), Some(-1.0));

      assert!(pill.is_some());
    }
  }
}
