use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use crate::{
  features::settings::features_tab::Group,
  i18n::Language,
  ui::{
    components::{
      eve_time::eve_time, icon::Icon, progress_bar::progress_bar, rule, status::dot, status_bar::status_bar,
    },
    style::{color, control, radius, spacing, typography},
  },
};

const CONTENT_PADDING: f32 = 48.0;
const FOOTER_SIDE_PADDING: f32 = 48.0;
const RAIL_WIDTH: f32 = 300.0;

#[derive(Clone, Debug)]
pub enum Message {
  Back,
  // The Features sub-step model derives its rail counts from the live feature flags the
  // per-group step tasks will own; this is the seam they dispatch when a flag flips.
  #[allow(dead_code)]
  JumpTo(usize),
  Next,
  // Selecting a language re-renders the whole wizard in that language on the next frame, before any
  // config is written (the view resolves keys against `pending_language`). The Language step task
  // (plxsyvtu) dispatches this from the language grid.
  #[allow(dead_code)]
  SelectLanguage(Language),
  Skip,
}

/// One concrete position in the wizard. The flat step list is Welcome, Language, one
/// [`Features`](Step::Features) step per [`Group`], then Storage and Finish; [`steps`] builds it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
  Features(Group),
  Finish,
  Language,
  Storage,
  Welcome,
}

impl Step {
  pub fn phase(self) -> Phase {
    match self {
      Step::Features(_) => Phase::Features,
      Step::Finish => Phase::Finish,
      Step::Language => Phase::Language,
      Step::Storage => Phase::Storage,
      Step::Welcome => Phase::Welcome,
    }
  }
}

/// A rail entry. Several [`Step`]s collapse into the single [`Features`](Phase::Features) phase, so
/// the rail shows five rows while the flat step list is longer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
  Features,
  Finish,
  Language,
  Storage,
  Welcome,
}

impl Phase {
  pub const ALL: [Phase; 5] = [
    Phase::Welcome,
    Phase::Language,
    Phase::Features,
    Phase::Storage,
    Phase::Finish,
  ];

  fn glyph(self) -> Icon {
    match self {
      Phase::Features => Icon::settings(),
      Phase::Finish => Icon::pulse(),
      Phase::Language => Icon::market(),
      Phase::Storage => Icon::archive(),
      Phase::Welcome => Icon::star(),
    }
  }

  fn label_key(self) -> &'static str {
    match self {
      Phase::Features => "wizard.phase.features.label",
      Phase::Finish => "wizard.phase.finish.label",
      Phase::Language => "wizard.phase.language.label",
      Phase::Storage => "wizard.phase.storage.label",
      Phase::Welcome => "wizard.phase.welcome.label",
    }
  }
}

#[derive(Debug)]
pub struct State {
  current: usize,
  // The in-progress language drives the rendered locale before any config write (ADR-0041 keeps the
  // committed locale fixed mid-session). The Language step task reads/writes this through
  // `Message::SelectLanguage`.
  #[allow(dead_code)]
  pending_language: Language,
  steps: Vec<Step>,
}

impl State {
  pub fn current_step(&self) -> Step {
    self.steps[self.current]
  }

  pub fn is_first(&self) -> bool {
    self.current == 0
  }

  pub fn is_last(&self) -> bool {
    self.current + 1 == self.steps.len()
  }

  // A step can advance only when its body is satisfied. Welcome, Language, and Finish are always
  // valid; the Features and Storage step tasks tighten this seam against their own state.
  fn can_advance(&self) -> bool {
    match self.current_step() {
      Step::Features(_) | Step::Storage => true,
      Step::Finish | Step::Language | Step::Welcome => true,
    }
  }

  // The rail jumps only to phases at or before the furthest-reached one. A phase is reachable once
  // its first step index has been visited (or passed).
  fn reachable(&self, phase: Phase) -> bool {
    self
      .first_index_of(phase)
      .is_some_and(|first| self.current >= first || self.current > self.last_index_of(phase))
  }

  fn first_index_of(&self, phase: Phase) -> Option<usize> {
    self.steps.iter().position(|step| step.phase() == phase)
  }

  fn last_index_of(&self, phase: Phase) -> usize {
    self.steps.iter().rposition(|step| step.phase() == phase).unwrap_or(0)
  }
}

impl Default for State {
  fn default() -> Self {
    State {
      current: 0,
      pending_language: Language::default(),
      steps: steps(),
    }
  }
}

/// The flat step list: Welcome, Language, one step per feature [`Group`], then Storage and Finish.
fn steps() -> Vec<Step> {
  let mut steps = vec![Step::Welcome, Step::Language];
  steps.extend(Group::ALL.into_iter().map(Step::Features));
  steps.push(Step::Storage);
  steps.push(Step::Finish);
  steps
}

pub fn update(state: &mut State, message: Message) {
  match message {
    Message::Back => {
      state.current = state.current.saturating_sub(1);
    }
    Message::JumpTo(index) => {
      if index < state.steps.len() && state.reachable(state.steps[index].phase()) {
        state.current = index;
      }
    }
    Message::Next => {
      if state.can_advance() && !state.is_last() {
        state.current += 1;
      }
    }
    Message::SelectLanguage(language) => {
      state.pending_language = language;
    }
    Message::Skip => {
      state.current = state.steps.len().saturating_sub(1);
    }
  }
}

pub fn view(state: &State) -> Element<'_, Message> {
  // The rail sits beside the content column; a Row lays them out left-to-right.
  let shell = Row::with_children(vec![rail(state), content(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  container(shell)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn rail(state: &State) -> Element<'_, Message> {
  let brand = rail_brand();
  let phases = Column::with_children(Phase::ALL.into_iter().map(|phase| rail_phase(state, phase)))
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let body = Column::with_children(vec![
    brand,
    container(phases)
      .padding(Padding {
        top: spacing::SPACE_4_5,
        right: spacing::SPACE_4_5,
        bottom: spacing::SPACE_4_5,
        left: spacing::SPACE_4_5,
      })
      .into(),
    Space::new().width(Length::Fill).height(Length::Fill).into(),
    rail_progress(state),
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  container(body)
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::NAVIGATION)),
      ..container::Style::default()
    })
    .into()
}

fn rail_brand<'a>() -> Element<'a, Message> {
  let eyebrow = text(t!("wizard.rail.eyebrow").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let title = text(t!("wizard.rail.title").into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));

  let label = Column::with_children(vec![eyebrow.into(), title.into()]).spacing(spacing::UNIT);

  let row = Row::with_children(vec![
    Icon::star().size(28.0).color(color::accent::PLASMA).render(),
    label.into(),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Center);

  Column::with_children(vec![
    container(row)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_6,
        right: spacing::SPACE_6,
        bottom: spacing::SPACE_4_5,
        left: spacing::SPACE_6,
      })
      .into(),
    rule::horizontal(),
  ])
  .width(Length::Fill)
  .into()
}

fn rail_phase(state: &State, phase: Phase) -> Element<'_, Message> {
  let current_phase = state.current_step().phase();
  let active = current_phase == phase;
  let done = state.current > state.last_index_of(phase);
  let reachable = state.reachable(phase);

  let chip = rail_chip(phase, active, done);

  let label = text(t!(phase.label_key()).into_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(if active {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    }));
  let sub = text(rail_phase_sub(phase).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let labels = Column::with_children(vec![label.into(), sub.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let chevron = if active {
    Icon::chevron_right().size(14.0).color(color::accent::PLASMA).render()
  } else {
    Space::new()
      .width(Length::Fixed(14.0))
      .height(Length::Fixed(14.0))
      .into()
  };

  let row = Row::with_children(vec![chip, labels.into(), chevron])
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
  });

  let mut entry = button(cell)
    .padding(0)
    .width(Length::Fill)
    .style(move |_, _| rail_phase_style(active));
  if reachable && let Some(first) = state.first_index_of(phase) {
    entry = entry.on_press(Message::JumpTo(first));
  }

  entry.into()
}

fn rail_chip<'a>(phase: Phase, active: bool, done: bool) -> Element<'a, Message> {
  let glyph = if done {
    Icon::check()
      .size(15.0)
      .color(color::on_fill(color::accent::PLASMA))
      .render()
  } else {
    let tint = if active {
      color::accent::PLASMA
    } else {
      color::text::tertiary()
    };
    phase.glyph().size(15.0).color(tint).render()
  };

  let (fill, border) = if done {
    (Some(color::accent::PLASMA), color::accent::PLASMA)
  } else if active {
    (
      Some(color::with_alpha(color::accent::PLASMA, 0.16)),
      color::accent::PLASMA,
    )
  } else {
    (None, color::rule_strong())
  };

  container(glyph)
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: fill.map(Background::Color),
      border: Border {
        color: border,
        width: 1.0,
        radius: 15.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn rail_progress(state: &State) -> Element<'_, Message> {
  let total = state.steps.len();
  let current = state.current + 1;

  let label = text(t!("wizard.rail.progress").into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let counter = text(
    t!(
      "wizard.rail.step_counter",
      current => format!("{current:02}"),
      total => format!("{total:02}"),
    )
    .into_owned(),
  )
  .font(typography::mono::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::accent::PLASMA));

  let header = Row::with_children(vec![
    label.into(),
    Space::new().width(Length::Fill).height(Length::Shrink).into(),
    counter.into(),
  ])
  .align_y(Vertical::Center);

  let fraction = current as f32 / total as f32;
  let bar = progress_bar(fraction, color::accent::PLASMA, 4.0);

  let body = Column::with_children(vec![header.into(), bar])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);

  Column::with_children(vec![
    rule::horizontal(),
    container(body)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_4_5,
        right: spacing::SPACE_6,
        bottom: spacing::SPACE_6,
        left: spacing::SPACE_6,
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn content(state: &State) -> Element<'_, Message> {
  let stage = scrollable(container(step_body(state)).width(Length::Fill).padding(CONTENT_PADDING))
    .style(control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  let column = Column::with_children(vec![stage.into(), footer(state), status(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  container(column).width(Length::Fill).height(Length::Fill).into()
}

// The per-step body seam. The sibling step tasks (Welcome+Language, Features, Storage, Finish) fill
// each arm; until then every step renders the same placeholder framed by the shared chrome.
fn step_body(state: &State) -> Element<'_, Message> {
  let _ = state.current_step();
  step_placeholder()
}

fn step_placeholder<'a>() -> Element<'a, Message> {
  container(
    text(t!("wizard.body.placeholder").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .into()
}

fn footer(state: &State) -> Element<'_, Message> {
  let mut back = button(footer_label(t!("wizard.footer.back").into_owned()))
    .padding(control::padding())
    .style(control::ghost_button);
  if !state.is_first() {
    back = back.on_press(Message::Back);
  }

  let mut children: Vec<Element<'_, Message>> = vec![
    back.into(),
    Space::new().width(Length::Fill).height(Length::Shrink).into(),
  ];

  if !state.is_last() {
    children.push(
      button(footer_label(t!("wizard.footer.skip").into_owned()))
        .padding(control::padding())
        .on_press(Message::Skip)
        .style(control::ghost_button)
        .into(),
    );
  }

  if state.is_last() {
    children.push(
      button(footer_label(t!("wizard.footer.open").into_owned()))
        .padding(control::padding())
        .on_press(Message::Next)
        .style(control::primary_button)
        .into(),
    );
  } else {
    let mut next = button(footer_label(next_label(state)))
      .padding(control::padding())
      .style(control::primary_button);
    if state.can_advance() {
      next = next.on_press(Message::Next);
    }
    children.push(next.into());
  }

  let row = Row::with_children(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

  Column::with_children(vec![
    rule::horizontal(),
    container(row)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_4_5,
        right: FOOTER_SIDE_PADDING,
        bottom: spacing::SPACE_4_5,
        left: FOOTER_SIDE_PADDING,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        ..container::Style::default()
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn footer_label<'a>(label: String) -> iced::widget::Text<'a> {
  text(label).font(typography::body::MEDIUM).size(typography::size::MD)
}

fn next_label(state: &State) -> String {
  let key = match state.current_step() {
    Step::Welcome => "wizard.footer.get_started",
    Step::Language => "wizard.footer.continue_to_features",
    Step::Features(_) if state.current == state.last_index_of(Phase::Features) => "wizard.footer.continue_to_storage",
    Step::Storage => "wizard.footer.review",
    Step::Features(_) | Step::Finish => "wizard.footer.next",
  };
  t!(key).into_owned()
}

fn rail_phase_sub(phase: Phase) -> std::borrow::Cow<'static, str> {
  match phase {
    Phase::Features => t!("wizard.phase.features.sub", count => Group::ALL.len()),
    Phase::Finish => t!("wizard.phase.finish.sub"),
    Phase::Language => t!("wizard.phase.language.sub"),
    Phase::Storage => t!("wizard.phase.storage.sub"),
    Phase::Welcome => t!("wizard.phase.welcome.sub"),
  }
}

fn rail_phase_style(active: bool) -> button::Style {
  let (background, border) = if active {
    (
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
      color::with_alpha(color::accent::PLASMA, 0.3),
    )
  } else {
    (None, color::with_alpha(color::accent::PLASMA, 0.0))
  };

  button::Style {
    background,
    border: Border {
      color: border,
      width: 1.0,
      radius: radius::NAV_CARD.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

fn status(state: &State) -> Element<'_, Message> {
  let session = Row::with_children(vec![
    dot(color::accent::PLASMA),
    text(t!("wizard.status.session").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let step = text(
    t!(
      "wizard.status.step_counter",
      current => state.current + 1,
      total => state.steps.len(),
    )
    .into_owned(),
  )
  .font(typography::mono::REGULAR)
  .size(typography::size::XS_PLUS)
  .style(typography::colored(color::accent::PLASMA));

  status_bar(vec![eve_time(chrono::Utc::now())], vec![session.into(), step.into()])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn state() -> State {
    State::default()
  }

  mod steps {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lays_welcome_language_features_storage_finish_in_order() {
      let steps = steps();

      assert_eq!(steps.first(), Some(&Step::Welcome));
      assert_eq!(steps.get(1), Some(&Step::Language));
      assert_eq!(steps.last(), Some(&Step::Finish));
    }

    #[test]
    fn it_expands_features_to_one_step_per_group() {
      let steps = steps();
      let feature_steps = steps.iter().filter(|step| step.phase() == Phase::Features).count();

      assert_eq!(feature_steps, Group::ALL.len());
    }

    #[test]
    fn its_length_is_two_plus_storage_finish_plus_one_per_group() {
      let steps = steps();

      assert_eq!(steps.len(), Group::ALL.len() + 4);
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn next_advances_one_step() {
      let mut state = state();

      update(&mut state, Message::Next);

      assert_eq!(state.current_step(), Step::Language);
    }

    #[test]
    fn back_returns_to_the_previous_step() {
      let mut state = state();
      update(&mut state, Message::Next);

      update(&mut state, Message::Back);

      assert_eq!(state.current_step(), Step::Welcome);
    }

    #[test]
    fn back_is_a_no_op_on_the_first_step() {
      let mut state = state();

      update(&mut state, Message::Back);

      assert!(state.is_first());
      assert_eq!(state.current_step(), Step::Welcome);
    }

    #[test]
    fn next_is_a_no_op_on_the_last_step() {
      let mut state = state();
      update(&mut state, Message::Skip);
      assert!(state.is_last());

      update(&mut state, Message::Next);

      assert!(state.is_last());
      assert_eq!(state.current_step(), Step::Finish);
    }

    #[test]
    fn skip_jumps_straight_to_finish() {
      let mut state = state();

      update(&mut state, Message::Skip);

      assert_eq!(state.current_step(), Step::Finish);
      assert!(state.is_last());
    }

    #[test]
    fn select_language_records_the_pending_choice() {
      let mut state = state();

      update(&mut state, Message::SelectLanguage(Language::De));

      assert_eq!(state.pending_language, Language::De);
    }

    #[test]
    fn jump_to_moves_to_a_reachable_phase_step() {
      let mut state = state();
      update(&mut state, Message::Next);
      update(&mut state, Message::Next);
      let welcome = state.first_index_of(Phase::Welcome).unwrap();

      update(&mut state, Message::JumpTo(welcome));

      assert_eq!(state.current_step(), Step::Welcome);
    }

    #[test]
    fn jump_to_ignores_an_unreached_phase() {
      let mut state = state();
      let finish = state.first_index_of(Phase::Finish).unwrap();

      update(&mut state, Message::JumpTo(finish));

      assert_eq!(state.current_step(), Step::Welcome);
    }
  }

  mod reachable {
    use super::*;

    #[test]
    fn the_current_phase_is_reachable() {
      let state = state();

      assert!(state.reachable(Phase::Welcome));
    }

    #[test]
    fn a_passed_phase_stays_reachable() {
      let mut state = state();
      update(&mut state, Message::Skip);

      assert!(state.reachable(Phase::Welcome));
      assert!(state.reachable(Phase::Features));
    }

    #[test]
    fn an_unreached_phase_is_not_reachable() {
      let state = state();

      assert!(!state.reachable(Phase::Finish));
      assert!(!state.reachable(Phase::Storage));
    }
  }

  mod current_step {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn the_rail_phase_tracks_the_current_step() {
      let mut state = state();
      assert_eq!(state.current_step().phase(), Phase::Welcome);

      update(&mut state, Message::Next);
      assert_eq!(state.current_step().phase(), Phase::Language);

      update(&mut state, Message::Next);
      assert_eq!(state.current_step().phase(), Phase::Features);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_first_step() {
      let state = state();

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_last_step() {
      let mut state = state();
      update(&mut state, Message::Skip);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_a_features_sub_step() {
      let mut state = state();
      update(&mut state, Message::Next);
      update(&mut state, Message::Next);

      let _el: Element<'_, Message> = view(&state);
    }
  }
}
