use iced::{
  Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Theme,
  alignment::{Horizontal, Vertical},
  mouse,
  widget::{Canvas, Column, Row, Space, button, canvas, container, scrollable, stack, text},
};

use crate::ui::{
  components::{icon::Icon, text_input::TextInput},
  style::{color, control, radius, spacing, typography},
};

const ACCENT_BAR_WIDTH: f32 = 3.0;
const DASH_SEGMENTS: [f32; 2] = [4.0, 3.0];
const ENGINEERING_COMPLEX_TYPE_IDS: [i64; 3] = [35_825, 35_826, 35_827];
const LIST_HEIGHT: f32 = 230.0;
const PILL_RADIUS: f32 = 3.0;
const REFINERY_TYPE_IDS: [i64; 2] = [35_835, 35_836];
const SLOT_HEIGHT: f32 = 62.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activity {
  Manufacturing,
  Reaction,
  Science,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
  Fee,
  Me,
  Te,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RigRef {
  pub activity: Activity,
  pub fee: f64,
  pub me: f64,
  pub name: String,
  pub te: f64,
  pub type_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructureActivity {
  Engineering,
  Refinery,
}

impl StructureActivity {
  pub fn from_structure_type_id(type_id: i64) -> Option<StructureActivity> {
    if ENGINEERING_COMPLEX_TYPE_IDS.contains(&type_id) {
      Some(StructureActivity::Engineering)
    } else if REFINERY_TYPE_IDS.contains(&type_id) {
      Some(StructureActivity::Refinery)
    } else {
      None
    }
  }

  pub fn allows(self, activity: Activity) -> bool {
    match self {
      StructureActivity::Engineering => matches!(activity, Activity::Manufacturing | Activity::Science),
      StructureActivity::Refinery => activity == Activity::Reaction,
    }
  }
}

pub fn rigs_for_structure(rigs: impl IntoIterator<Item = RigRef>, structure_type_id: i64) -> Vec<RigRef> {
  match StructureActivity::from_structure_type_id(structure_type_id) {
    Some(class) => rigs.into_iter().filter(|rig| class.allows(rig.activity)).collect(),
    None => rigs.into_iter().collect(),
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RigSearch {
  generation: u64,
  highlight: Option<usize>,
  query: String,
  results: Vec<RigRef>,
  searching: bool,
}

impl RigSearch {
  pub fn accept_results(&mut self, generation: u64, results: Vec<RigRef>) -> bool {
    if generation != self.generation {
      return false;
    }

    self.highlight = None;
    self.results = results;
    self.searching = false;
    true
  }

  #[cfg_attr(
    not(test),
    expect(
      dead_code,
      reason = "Symmetric reset; the settings rig picker drops its search state on close rather than clearing in place."
    )
  )]
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
  pub fn highlighted(&self) -> Option<&RigRef> {
    self.highlight.and_then(|index| self.results.get(index))
  }

  pub fn query(&self) -> &str {
    &self.query
  }

  pub fn results(&self) -> &[RigRef] {
    &self.results
  }

  pub fn searching(&self) -> bool {
    self.searching
  }

  pub fn set_query(&mut self, query: String) -> u64 {
    self.generation = self.generation.wrapping_add(1);
    self.highlight = None;
    self.query = query;
    self.searching = true;
    self.generation
  }
}

#[derive(Clone, Copy)]
struct BonusLabels<'a> {
  fee: &'a str,
  me: &'a str,
  te: &'a str,
}

pub struct RigCombobox<'a, M> {
  clear_label: &'a str,
  empty_label: &'a str,
  fee_label: &'a str,
  highlight: Option<usize>,
  me_label: &'a str,
  on_clear: Option<M>,
  on_input: Option<Box<dyn Fn(String) -> M + 'a>>,
  on_pick: Option<Box<dyn Fn(RigRef) -> M + 'a>>,
  on_toggle: Option<M>,
  placeholder: &'a str,
  query: &'a str,
  results: Vec<RigRef>,
  searching: bool,
  searching_label: &'a str,
  selection: Option<RigRef>,
  te_label: &'a str,
  width: Length,
}

impl<M: Clone + 'static> Default for RigCombobox<'_, M> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a, M: Clone + 'static> RigCombobox<'a, M> {
  pub fn new() -> Self {
    Self {
      clear_label: "",
      empty_label: "",
      fee_label: "",
      highlight: None,
      me_label: "",
      on_clear: None,
      on_input: None,
      on_pick: None,
      on_toggle: None,
      placeholder: "",
      query: "",
      results: Vec::new(),
      searching: false,
      searching_label: "",
      selection: None,
      te_label: "",
      width: Length::Fill,
    }
  }

  pub fn clear_label(mut self, clear_label: &'a str) -> Self {
    self.clear_label = clear_label;
    self
  }

  pub fn empty_label(mut self, empty_label: &'a str) -> Self {
    self.empty_label = empty_label;
    self
  }

  pub fn fee_label(mut self, fee_label: &'a str) -> Self {
    self.fee_label = fee_label;
    self
  }

  pub fn highlight(mut self, highlight: Option<usize>) -> Self {
    self.highlight = highlight;
    self
  }

  pub fn me_label(mut self, me_label: &'a str) -> Self {
    self.me_label = me_label;
    self
  }

  pub fn on_clear(mut self, message: M) -> Self {
    self.on_clear = Some(message);
    self
  }

  pub fn on_input(mut self, on_input: impl Fn(String) -> M + 'a) -> Self {
    self.on_input = Some(Box::new(on_input));
    self
  }

  pub fn on_pick(mut self, on_pick: impl Fn(RigRef) -> M + 'a) -> Self {
    self.on_pick = Some(Box::new(on_pick));
    self
  }

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

  pub fn results(mut self, results: Vec<RigRef>) -> Self {
    self.results = results;
    self
  }

  pub fn searching(mut self, searching: bool) -> Self {
    self.searching = searching;
    self
  }

  pub fn searching_label(mut self, searching_label: &'a str) -> Self {
    self.searching_label = searching_label;
    self
  }

  pub fn selection(mut self, selection: Option<RigRef>) -> Self {
    self.selection = selection;
    self
  }

  pub fn te_label(mut self, te_label: &'a str) -> Self {
    self.te_label = te_label;
    self
  }

  pub fn width(mut self, width: Length) -> Self {
    self.width = width;
    self
  }

  fn labels(&self) -> BonusLabels<'a> {
    BonusLabels {
      fee: self.fee_label,
      me: self.me_label,
      te: self.te_label,
    }
  }

  pub fn trigger(self) -> Element<'a, M> {
    let labels = self.labels();
    let filled = self.selection.is_some();
    let card: Element<'a, M> = match &self.selection {
      Some(rig) => selected_card(rig, labels),
      None => empty_card(self.empty_label),
    };

    let mut field = button(card)
      .width(self.width)
      .height(Length::Fixed(SLOT_HEIGHT))
      .padding(Padding {
        top: spacing::SPACE_2_5,
        bottom: spacing::SPACE_2_5,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      });
    if let Some(message) = self.on_toggle {
      field = field.on_press(message);
    }
    let field = field.style(move |_, status| {
      let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
      if filled {
        button::Style {
          background: Some(Background::Color(color::surface::SUNKEN)),
          border: Border {
            color: if active { color::accent() } else { color::rule() },
            radius: radius::CONTROL.into(),
            width: 1.0,
          },
          text_color: color::text::PRIMARY,
          ..button::Style::default()
        }
      } else {
        button::Style {
          background: None,
          border: Border {
            color: Color::TRANSPARENT,
            radius: radius::CONTROL.into(),
            width: 0.0,
          },
          text_color: color::text::secondary(),
          ..button::Style::default()
        }
      }
    });

    if filled {
      field.into()
    } else {
      let band = color::rule_strong();
      stack(vec![field.into(), dashed_border(band)]).width(self.width).into()
    }
  }

  pub fn popover(self) -> Element<'a, M> {
    let labels = self.labels();
    let Self {
      clear_label,
      empty_label,
      highlight,
      on_clear,
      on_input,
      on_pick,
      placeholder,
      query,
      results,
      searching,
      searching_label,
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

    let selected_type = selection.as_ref().map(|rig| rig.type_id);
    let rows: Vec<Element<'a, M>> = match &on_pick {
      Some(on_pick) => results
        .iter()
        .enumerate()
        .map(|(index, rig)| {
          let selected = selected_type == Some(rig.type_id);
          result_row(rig, labels, highlight == Some(index), selected, &**on_pick)
        })
        .collect(),
      None => Vec::new(),
    };

    let list: Element<'a, M> = if rows.is_empty() {
      centered(status_label(if searching { searching_label } else { empty_label }))
    } else {
      scrollable(Column::with_children(rows).spacing(spacing::UNIT).width(Length::Fill))
        .style(control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fixed(LIST_HEIGHT))
        .into()
    };

    let mut body: Vec<Element<'a, M>> = vec![search, list];
    if let Some(message) = on_clear {
      body.push(footer(clear_label, message));
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

fn empty_card<'a, M: 'static>(empty_label: &str) -> Element<'a, M> {
  container(
    Column::new()
      .spacing(spacing::UNIT)
      .align_x(Horizontal::Center)
      .push(Icon::plus().color(color::text::secondary()).size(15.0).render::<M>())
      .push(
        text(empty_label.to_uppercase())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::secondary())),
      ),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

struct DashedBorder {
  color: Color,
}

impl<M> canvas::Program<M> for DashedBorder {
  type State = ();

  fn draw(
    &self,
    _state: &Self::State,
    renderer: &Renderer,
    _theme: &Theme,
    bounds: Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<canvas::Geometry> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    let path = canvas::Path::rounded_rectangle(
      Point::new(0.5, 0.5),
      Size::new((bounds.width - 1.0).max(0.0), (bounds.height - 1.0).max(0.0)),
      radius::CONTROL.into(),
    );
    frame.stroke(
      &path,
      canvas::Stroke {
        line_dash: canvas::LineDash {
          segments: &DASH_SEGMENTS,
          offset: 0,
        },
        ..canvas::Stroke::default().with_width(1.0).with_color(self.color)
      },
    );
    vec![frame.into_geometry()]
  }
}

fn dashed_border<'a, M: 'a>(color: Color) -> Element<'a, M> {
  Canvas::new(DashedBorder {
    color,
  })
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn footer<'a, M: Clone + 'a>(clear_label: &str, on_clear: M) -> Element<'a, M> {
  button(
    text(clear_label.to_owned())
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

fn primary_bonus(rig: &RigRef) -> (Kind, f64) {
  if rig.me != 0.0 {
    (Kind::Me, rig.me)
  } else if rig.te != 0.0 {
    (Kind::Te, rig.te)
  } else {
    (Kind::Fee, rig.fee)
  }
}

fn kind_color(kind: Kind) -> Color {
  match kind {
    Kind::Fee => color::status::WARNING,
    Kind::Me => color::accent(),
    Kind::Te => color::status::ONLINE,
  }
}

fn kind_label<'a>(kind: Kind, labels: BonusLabels<'a>) -> &'a str {
  match kind {
    Kind::Fee => labels.fee,
    Kind::Me => labels.me,
    Kind::Te => labels.te,
  }
}

fn format_pct(value: f64, kind: Kind) -> String {
  let decimals = if kind == Kind::Te { 0 } else { 1 };
  format!("{value:.decimals$}%")
}

fn bonus_pill<'a, M: 'a>(label: &str, band: Color) -> Element<'a, M> {
  if label.trim().is_empty() {
    return Space::new().into();
  }
  container(
    text(label.to_owned())
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

fn bonus_value<'a, M: 'a>(value: f64, kind: Kind) -> Element<'a, M> {
  text(format_pct(value, kind))
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(kind_color(kind)))
    .into()
}

fn result_row<'a, M: Clone + 'a>(
  rig: &RigRef,
  labels: BonusLabels<'a>,
  highlighted: bool,
  selected: bool,
  on_pick: &dyn Fn(RigRef) -> M,
) -> Element<'a, M> {
  let (kind, value) = primary_bonus(rig);
  let band = kind_color(kind);

  let heading = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .push(
      text(rig.name.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(if selected {
          color::accent()
        } else {
          color::text::PRIMARY
        }))
        .width(Length::Fill),
    )
    .push(bonus_pill(kind_label(kind, labels), band));

  let details = Column::with_children(vec![heading.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![details.into(), bonus_value(value, kind)])
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
    .on_press(on_pick(rig.clone()))
    .style(move |_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      let lit = highlighted || hover || selected;
      button::Style {
        background: lit.then(|| Background::Color(color::with_alpha(color::accent(), 0.12))),
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

fn selected_card<'a, M: 'a>(rig: &RigRef, labels: BonusLabels<'a>) -> Element<'a, M> {
  let (kind, value) = primary_bonus(rig);
  let band = kind_color(kind);

  let heading = Row::new()
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .push(
      text(rig.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(color::text::PRIMARY))
        .width(Length::Fill),
    )
    .push(bonus_pill(kind_label(kind, labels), band));

  let details = Column::new()
    .spacing(spacing::UNIT)
    .width(Length::Fill)
    .push(heading)
    .push(bonus_value(value, kind));

  let bar = container(Space::new())
    .width(Length::Fixed(ACCENT_BAR_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(band)),
      border: Border {
        radius: PILL_RADIUS.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  Row::with_children(vec![bar.into(), details.into()])
    .spacing(spacing::SPACE_2_5)
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

  fn sample(type_id: i64, name: &str, activity: Activity, me: f64, te: f64) -> RigRef {
    RigRef {
      activity,
      fee: 0.0,
      me,
      name: name.to_owned(),
      te,
      type_id,
    }
  }

  mod rig_combobox {
    use super::*;

    #[test]
    fn it_renders_a_popover_with_results_and_a_footer() {
      let results = vec![
        sample(
          1,
          "Standup M-Set Equipment Manufacturing ME I",
          Activity::Manufacturing,
          -2.0,
          0.0,
        ),
        sample(
          2,
          "Standup M-Set Basic Large Ship Manufacturing TE I",
          Activity::Manufacturing,
          0.0,
          -20.0,
        ),
      ];

      let _el: Element<'_, Message> = RigCombobox::new()
        .placeholder("Search rigs")
        .width(Length::Fill)
        .query("Manufacturing")
        .results(results)
        .me_label("ME")
        .te_label("TE")
        .fee_label("Fee")
        .on_input(Message::Input)
        .on_pick(|rig| Message::Picked(rig.type_id))
        .highlight(Some(0))
        .clear_label("Clear")
        .on_clear(Message::Cleared)
        .popover();
    }

    #[test]
    fn it_renders_a_trigger_with_a_selection() {
      let _el: Element<'_, Message> = RigCombobox::new()
        .me_label("ME")
        .selection(Some(sample(
          1,
          "Standup M-Set ME I",
          Activity::Manufacturing,
          -2.0,
          0.0,
        )))
        .on_toggle(Message::Cleared)
        .trigger();
    }

    #[test]
    fn it_renders_an_empty_trigger() {
      let _el: Element<'_, Message> = RigCombobox::new()
        .empty_label("Add rig")
        .on_toggle(Message::Cleared)
        .trigger();
    }

    #[test]
    fn it_renders_the_searching_state() {
      let _el: Element<'_, Message> = RigCombobox::new()
        .query("Reactor")
        .on_input(Message::Input)
        .on_pick(|rig| Message::Picked(rig.type_id))
        .searching_label("Searching")
        .searching(true)
        .popover();
    }
  }

  mod rig_search {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_results_only_for_the_current_generation() {
      let mut search = RigSearch::default();
      let stale = search.set_query("react".to_owned());
      let current = search.set_query("reactor".to_owned());

      let accepted_stale = search.accept_results(stale, vec![sample(1, "Stale", Activity::Reaction, -1.0, 0.0)]);
      let accepted_current = search.accept_results(current, vec![sample(2, "Current", Activity::Reaction, -1.0, 0.0)]);

      assert!(!accepted_stale);
      assert!(accepted_current);
      assert_eq!(search.results(), &[sample(2, "Current", Activity::Reaction, -1.0, 0.0)]);
      assert!(!search.searching());
    }

    #[test]
    fn it_bumps_the_generation_and_marks_searching() {
      let mut search = RigSearch::default();

      let generation = search.set_query("react".to_owned());

      assert_eq!(generation, 1);
      assert_eq!(search.generation(), 1);
      assert_eq!(search.query(), "react");
      assert!(search.searching());
    }

    #[test]
    fn it_clears_query_results_and_bumps_the_generation() {
      let mut search = RigSearch::default();
      let generation = search.set_query("react".to_owned());
      search.accept_results(generation, vec![sample(1, "Reactor", Activity::Reaction, -1.0, 0.0)]);

      search.clear();

      assert_eq!(search.query(), "");
      assert!(search.results().is_empty());
      assert!(!search.searching());
      assert_eq!(search.generation(), 2);
    }

    #[test]
    fn it_steps_the_highlight_within_result_bounds() {
      let mut search = RigSearch::default();
      let generation = search.set_query("react".to_owned());
      search.accept_results(
        generation,
        vec![
          sample(1, "A", Activity::Reaction, -1.0, 0.0),
          sample(2, "B", Activity::Reaction, -1.0, 0.0),
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

      assert_eq!(
        search.highlighted(),
        Some(&sample(1, "A", Activity::Reaction, -1.0, 0.0))
      );
    }
  }

  mod structure_activity {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_classifies_engineering_complexes_from_their_type_id() {
      for type_id in [35_825, 35_826, 35_827] {
        assert_eq!(
          StructureActivity::from_structure_type_id(type_id),
          Some(StructureActivity::Engineering)
        );
      }
    }

    #[test]
    fn it_classifies_refineries_from_their_type_id() {
      for type_id in [35_835, 35_836] {
        assert_eq!(
          StructureActivity::from_structure_type_id(type_id),
          Some(StructureActivity::Refinery)
        );
      }
    }

    #[test]
    fn it_returns_none_for_a_citadel_without_industry_slots() {
      assert_eq!(StructureActivity::from_structure_type_id(35_834), None);
    }

    #[test]
    fn it_offers_manufacturing_and_science_for_engineering_complexes() {
      let class = StructureActivity::Engineering;

      assert!(class.allows(Activity::Manufacturing));
      assert!(class.allows(Activity::Science));
      assert!(!class.allows(Activity::Reaction));
    }

    #[test]
    fn it_offers_only_reactions_for_refineries() {
      let class = StructureActivity::Refinery;

      assert!(class.allows(Activity::Reaction));
      assert!(!class.allows(Activity::Manufacturing));
      assert!(!class.allows(Activity::Science));
    }
  }

  mod rigs_for_structure {
    use pretty_assertions::assert_eq;

    use super::*;

    fn catalog() -> Vec<RigRef> {
      vec![
        sample(101, "Manufacturing ME", Activity::Manufacturing, -2.0, 0.0),
        sample(102, "Invention Optimization", Activity::Science, 0.0, -20.0),
        sample(103, "Composite Reactor ME", Activity::Reaction, -2.4, 0.0),
      ]
    }

    #[test]
    fn it_keeps_manufacturing_and_science_rigs_for_an_engineering_complex() {
      let offered = rigs_for_structure(catalog(), 35_825);

      let type_ids: Vec<i64> = offered.iter().map(|rig| rig.type_id).collect();
      assert_eq!(type_ids, vec![101, 102]);
    }

    #[test]
    fn it_keeps_only_reaction_rigs_for_a_refinery() {
      let offered = rigs_for_structure(catalog(), 35_836);

      let type_ids: Vec<i64> = offered.iter().map(|rig| rig.type_id).collect();
      assert_eq!(type_ids, vec![103]);
    }

    #[test]
    fn it_returns_every_rig_for_an_unclassified_structure() {
      let offered = rigs_for_structure(catalog(), 35_834);

      assert_eq!(offered.len(), 3);
    }
  }

  mod primary_bonus {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_material_efficiency_when_present() {
      let rig = sample(1, "ME", Activity::Manufacturing, -2.0, 0.0);

      assert_eq!(super::super::primary_bonus(&rig), (Kind::Me, -2.0));
    }

    #[test]
    fn it_falls_back_to_time_efficiency() {
      let rig = sample(1, "TE", Activity::Manufacturing, 0.0, -20.0);

      assert_eq!(super::super::primary_bonus(&rig), (Kind::Te, -20.0));
    }

    #[test]
    fn it_falls_back_to_the_install_fee() {
      let mut rig = sample(1, "Fee", Activity::Manufacturing, 0.0, 0.0);
      rig.fee = -10.0;

      assert_eq!(super::super::primary_bonus(&rig), (Kind::Fee, -10.0));
    }
  }
}
