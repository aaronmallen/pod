use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use crate::ui::{
  components::{count_badge::count_badge, icon::Icon, text_input::TextInput},
  style::{color, control, radius, spacing, typography},
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
pub struct FacilityRef {
  pub cost_index: Option<f64>,
  pub id: i64,
  pub name: String,
  pub region: Option<String>,
  pub security_status: Option<f64>,
  pub solar_system: String,
  pub solar_system_id: i64,
  pub type_id: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FacilitySearch {
  generation: u64,
  highlight: Option<usize>,
  query: String,
  results: Vec<FacilityRef>,
  searching: bool,
}

impl FacilitySearch {
  /// Installs search results if `generation` matches the current one; returns `false` and discards
  /// results that arrive from a superseded query.
  pub fn accept_results(&mut self, generation: u64, results: Vec<FacilityRef>) -> bool {
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

  #[cfg(test)]
  pub fn generation(&self) -> u64 {
    self.generation
  }

  pub fn highlight(&self) -> Option<usize> {
    self.highlight
  }

  #[cfg(test)]
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

  #[cfg(test)]
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

  #[cfg(test)]
  pub fn highlighted(&self) -> Option<&FacilityRef> {
    self.highlight.and_then(|index| self.results.get(index))
  }

  pub fn query(&self) -> &str {
    &self.query
  }

  pub fn results(&self) -> &[FacilityRef] {
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

pub struct FacilityCombobox<'a, M> {
  highlight: Option<usize>,
  on_clear: Option<M>,
  on_input: Option<Box<dyn Fn(String) -> M + 'a>>,
  on_pick: Option<Box<dyn Fn(FacilityRef) -> M + 'a>>,
  on_toggle: Option<M>,
  placeholder: &'a str,
  query: &'a str,
  results: Vec<FacilityRef>,
  searching: bool,
  selection: Option<FacilityRef>,
  width: Length,
}

impl<M: Clone + 'static> Default for FacilityCombobox<'_, M> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a, M: Clone + 'static> FacilityCombobox<'a, M> {
  pub fn new() -> Self {
    Self {
      highlight: None,
      on_clear: None,
      on_input: None,
      on_pick: None,
      on_toggle: None,
      placeholder: "Search stations & structures\u{2026}",
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

  /// Sets the search-input handler used by [`FacilityCombobox::popover`].
  pub fn on_input(mut self, on_input: impl Fn(String) -> M + 'a) -> Self {
    self.on_input = Some(Box::new(on_input));
    self
  }

  /// Sets the row-selection handler used by [`FacilityCombobox::popover`].
  pub fn on_pick(mut self, on_pick: impl Fn(FacilityRef) -> M + 'a) -> Self {
    self.on_pick = Some(Box::new(on_pick));
    self
  }

  /// Sets the message emitted when the [`FacilityCombobox::trigger`] button is pressed (open/close).
  pub fn on_toggle(mut self, message: M) -> Self {
    self.on_toggle = Some(message);
    self
  }

  pub fn on_clear(mut self, message: M) -> Self {
    self.on_clear = Some(message);
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

  pub fn results(mut self, results: Vec<FacilityRef>) -> Self {
    self.results = results;
    self
  }

  pub fn searching(mut self, searching: bool) -> Self {
    self.searching = searching;
    self
  }

  pub fn selection(mut self, selection: Option<FacilityRef>) -> Self {
    self.selection = selection;
    self
  }

  pub fn width(mut self, width: Length) -> Self {
    self.width = width;
    self
  }

  /// The always-visible field showing the selected facility as a two-line card (or the empty placeholder).
  /// Pressing it toggles the [`FacilityCombobox::popover`] open; the popover (not this button) owns the search input.
  pub fn trigger(self) -> Element<'a, M> {
    let card: Element<'a, M> = match &self.selection {
      Some(facility) => selected_card(facility),
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
  /// clear/"Ask each install" footer. Hosts anchor this under the [`FacilityCombobox::trigger`] button so
  /// the two read as one combobox.
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
      text(t!("common.facility_combobox.title"))
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Space::new().width(Length::Fill).into(),
      count_badge(results.len() as i64, color::accent::PLASMA),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

    let selected_system = selection.as_ref().map(|facility| facility.solar_system_id);
    let rows: Vec<Element<'a, M>> = match &on_pick {
      Some(on_pick) => results
        .iter()
        .enumerate()
        .map(|(index, facility)| {
          let selected = selected_system == Some(facility.solar_system_id);
          result_row(facility, highlight == Some(index), selected, &**on_pick)
        })
        .collect(),
      None => Vec::new(),
    };

    let list: Element<'a, M> = if rows.is_empty() {
      centered(status_label(if searching {
        t!("common.facility_combobox.searching")
      } else if searchable(query) {
        t!("common.facility_combobox.no_results")
      } else {
        t!("common.facility_combobox.type_to_search")
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

fn cost_index_block<'a, M: 'a>(cost_index: Option<f64>) -> Element<'a, M> {
  let pct = cost_index.unwrap_or(0.0) * 100.0;
  Column::with_children(vec![
    text(t!("common.facility_combobox.cost_index"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(format!("{pct:.2}%"))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::accent::PLASMA))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_x(Horizontal::Right)
  .into()
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
    text(t!("common.facility_combobox.clear"))
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

fn location_subtitle(facility: &FacilityRef) -> Option<String> {
  let region = facility
    .region
    .as_deref()
    .map(str::trim)
    .filter(|region| !region.is_empty());
  let system = Some(facility.solar_system.trim()).filter(|system| !system.is_empty());

  match (region, system) {
    (Some(region), Some(system)) => Some(format!("{region} \u{00B7} {system}")),
    (Some(region), None) => Some(region.to_owned()),
    (None, Some(system)) => Some(system.to_owned()),
    (None, None) => None,
  }
}

fn result_row<'a, M: Clone + 'a>(
  facility: &FacilityRef,
  highlighted: bool,
  selected: bool,
  on_pick: &dyn Fn(FacilityRef) -> M,
) -> Element<'a, M> {
  let name = text(facility.name.clone())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(if selected {
      color::accent::PLASMA
    } else {
      color::text::PRIMARY
    }));

  let mut meta = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
  meta = meta.push(sec_pill(facility.security_status));
  if !facility.solar_system.trim().is_empty() {
    meta = meta.push(
      text(facility.solar_system.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }
  if let Some(region) = &facility.region {
    meta = meta.push(
      text(region.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }

  let details = Column::with_children(vec![name.into(), meta.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let pct = facility.cost_index.unwrap_or(0.0) * 100.0;
  let pct_label = text(format!("{pct:.2}%"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::secondary()));

  let row = Row::with_children(vec![details.into(), pct_label.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  button(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .on_press(on_pick(facility.clone()))
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

fn sec_pill<'a, M: 'a>(security_status: Option<f64>) -> Element<'a, M> {
  let Some(sec) = security_status else {
    return Space::new().into();
  };
  let (label, band) = if sec <= -0.9 {
    (t!("common.facility_combobox.jspace").into_owned(), WORMHOLE)
  } else if sec <= 0.0 {
    (format!("{sec:.1}"), color::status::DANGER)
  } else if sec < 0.5 {
    (format!("{sec:.1}"), color::status::WARNING)
  } else {
    (format!("{sec:.1}"), color::status::ONLINE)
  };

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
  .into()
}

fn selected_card<'a, M: 'a>(facility: &FacilityRef) -> Element<'a, M> {
  let heading = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .push(
      text(facility.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(color::text::PRIMARY))
        .width(Length::Fill), // bounded width is required for iced to wrap long names instead of clipping
    )
    .push(sec_pill(facility.security_status));

  let mut details = Column::new().spacing(spacing::UNIT).width(Length::Fill).push(heading);
  if let Some(subtitle) = location_subtitle(facility) {
    details = details.push(
      text(subtitle)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    );
  }

  Row::with_children(vec![details.into(), cost_index_block(facility.cost_index)])
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Top)
    .width(Length::Fill)
    .into()
}

fn status_label<'a, M: 'a>(label: impl text::IntoFragment<'a>) -> Element<'a, M> {
  text(label)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::tertiary()))
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

  fn sample(id: i64, name: &str) -> FacilityRef {
    FacilityRef {
      cost_index: Some(0.05),
      id,
      name: name.to_owned(),
      region: Some("The Forge".to_owned()),
      security_status: Some(0.9),
      solar_system: "Jita".to_owned(),
      solar_system_id: 30_000_142,
      type_id: Some(35_834),
    }
  }

  mod facility_combobox {
    use super::*;

    #[test]
    fn it_renders_a_popover_with_results_and_a_footer() {
      let results = vec![sample(1, "Jita Keepstar"), sample(2, "Perimeter Tranquility")];

      let _el: Element<'_, Message> = FacilityCombobox::new()
        .query("Jita")
        .results(results)
        .on_input(Message::Input)
        .on_pick(|f| Message::Picked(f.id))
        .highlight(Some(0))
        .on_clear(Message::Cleared)
        .popover();
    }

    #[test]
    fn it_renders_a_selected_trigger_without_a_region() {
      let mut facility = sample(1, "Jita Keepstar");
      facility.region = None;

      let _el: Element<'_, Message> = FacilityCombobox::new()
        .selection(Some(facility))
        .on_toggle(Message::Cleared)
        .trigger();
    }

    #[test]
    fn it_renders_a_trigger_with_a_selection() {
      let _el: Element<'_, Message> = FacilityCombobox::new()
        .selection(Some(sample(1, "Jita Keepstar")))
        .on_toggle(Message::Cleared)
        .trigger();
    }

    #[test]
    fn it_renders_an_empty_trigger() {
      let _el: Element<'_, Message> = FacilityCombobox::new()
        .placeholder("Ask each install")
        .on_toggle(Message::Cleared)
        .trigger();
    }

    #[test]
    fn it_renders_the_searching_state() {
      let _el: Element<'_, Message> = FacilityCombobox::new()
        .query("Jita")
        .on_input(Message::Input)
        .on_pick(|f| Message::Picked(f.id))
        .searching(true)
        .popover();
    }
  }

  mod facility_search {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_results_only_for_the_current_generation() {
      let mut search = FacilitySearch::default();
      let stale = search.set_query("Jita".to_owned());
      let current = search.set_query("Jita IV".to_owned());

      let accepted_stale = search.accept_results(stale, vec![sample(1, "Stale")]);
      let accepted_current = search.accept_results(current, vec![sample(2, "Current")]);

      assert!(!accepted_stale);
      assert!(accepted_current);
      assert_eq!(search.results(), &[sample(2, "Current")]);
      assert!(!search.searching());
    }

    #[test]
    fn it_bumps_the_generation_and_marks_searching_above_min_chars() {
      let mut search = FacilitySearch::default();

      let generation = search.set_query("Jita".to_owned());

      assert_eq!(generation, 1);
      assert_eq!(search.generation(), 1);
      assert_eq!(search.query(), "Jita");
      assert!(search.searching());
    }

    #[test]
    fn it_clears_query_results_and_bumps_the_generation() {
      let mut search = FacilitySearch::default();
      let generation = search.set_query("Jita".to_owned());
      search.accept_results(generation, vec![sample(1, "Jita Keepstar")]);

      search.clear();

      assert_eq!(search.query(), "");
      assert!(search.results().is_empty());
      assert!(!search.searching());
      assert_eq!(search.generation(), 2);
    }

    #[test]
    fn it_clears_results_below_min_chars_without_searching() {
      let mut search = FacilitySearch::default();
      let generation = search.set_query("Jita".to_owned());
      search.accept_results(generation, vec![sample(1, "Jita Keepstar")]);

      search.set_query("Ji".to_owned());

      assert!(search.results().is_empty());
      assert!(!search.searching());
    }

    #[test]
    fn it_clears_the_highlight_when_results_are_empty() {
      let mut search = FacilitySearch::default();

      search.highlight_next();

      assert_eq!(search.highlight(), None);
    }

    #[test]
    fn it_resolves_the_highlighted_result() {
      let mut search = FacilitySearch::default();
      let generation = search.set_query("Jita".to_owned());
      search.accept_results(generation, vec![sample(1, "A"), sample(2, "B")]);
      search.highlight_next();
      search.highlight_next();

      assert_eq!(search.highlighted(), Some(&sample(2, "B")));
    }

    #[test]
    fn it_steps_the_highlight_within_result_bounds() {
      let mut search = FacilitySearch::default();
      let generation = search.set_query("Jita".to_owned());
      search.accept_results(generation, vec![sample(1, "A"), sample(2, "B")]);

      search.highlight_next();
      assert_eq!(search.highlight(), Some(0));

      search.highlight_next();
      assert_eq!(search.highlight(), Some(1));

      search.highlight_next();
      assert_eq!(search.highlight(), Some(1));

      search.highlight_prev();
      assert_eq!(search.highlight(), Some(0));

      search.highlight_prev();
      assert_eq!(search.highlight(), Some(0));
    }
  }

  mod location_subtitle {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_joins_region_and_system_with_a_separator() {
      let facility = sample(1, "Jita Keepstar");

      assert_eq!(
        super::super::location_subtitle(&facility),
        Some("The Forge \u{00B7} Jita".to_owned())
      );
    }

    #[test]
    fn it_omits_the_separator_when_the_region_is_absent() {
      let mut facility = sample(1, "Jita Keepstar");
      facility.region = None;

      assert_eq!(super::super::location_subtitle(&facility), Some("Jita".to_owned()));
    }

    #[test]
    fn it_omits_the_separator_when_the_system_is_blank() {
      let mut facility = sample(1, "Jita Keepstar");
      facility.solar_system = "  ".to_owned();

      assert_eq!(super::super::location_subtitle(&facility), Some("The Forge".to_owned()));
    }

    #[test]
    fn it_returns_nothing_when_both_are_absent() {
      let mut facility = sample(1, "Jita Keepstar");
      facility.region = None;
      facility.solar_system = String::new();

      assert_eq!(super::super::location_subtitle(&facility), None);
    }
  }

  mod sec_pill {
    use super::*;

    #[test]
    fn it_renders_a_high_security_pill() {
      let _el: Element<'_, Message> = super::super::sec_pill(Some(0.9));
    }

    #[test]
    fn it_renders_a_low_security_pill() {
      let _el: Element<'_, Message> = super::super::sec_pill(Some(0.3));
    }

    #[test]
    fn it_renders_a_null_security_pill() {
      let _el: Element<'_, Message> = super::super::sec_pill(Some(-0.2));
    }

    #[test]
    fn it_renders_a_wormhole_pill() {
      let _el: Element<'_, Message> = super::super::sec_pill(Some(-1.0));
    }

    #[test]
    fn it_renders_nothing_without_a_security_status() {
      let _el: Element<'_, Message> = super::super::sec_pill(None);
    }
  }

  mod selected_card {
    use super::*;

    #[test]
    fn it_renders_a_long_player_structure_name() {
      let facility = sample(
        1,
        "Police Weapons Facility - Outer Ring Excavations Strategic Reserve Depot",
      );

      let _el: Element<'_, Message> = super::super::selected_card(&facility);
    }

    #[test]
    fn it_renders_an_npc_system_prefixed_facility() {
      let mut facility = sample(1, "Jita IV - Moon 4 - Caldari Navy Assembly Plant");
      facility.solar_system = "Jita".to_owned();
      facility.region = Some("The Forge".to_owned());

      let _el: Element<'_, Message> = super::super::selected_card(&facility);
    }
  }
}
