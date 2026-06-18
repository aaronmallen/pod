use iced::{
  Element, Length, Padding,
  widget::{Column, container, scrollable, text},
};

use super::Outcome;
use crate::{
  config::Settings,
  features::industry::{PinnedStructure, PlannerFacility},
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      facility_combobox::{FacilityCombobox, FacilityRef, FacilitySearch},
      rule,
    },
    style::{color, spacing, typography},
  },
};

/// EVE ESI industry activity id for manufacturing.
const MANUFACTURING_ACTIVITY_ID: i64 = 1;
/// Facility ids at or above this are player-owned structures; below are NPC stations.
///
/// Only structures need pinning (see `pin_for`); station ids are well-known and never persisted.
const MIN_STRUCTURE_ID: i64 = 1_000_000_000_000;
const PANEL_SIDE_PADDING: f32 = 36.0;
/// EVE ESI industry activity id for reactions.
const REACTION_ACTIVITY_ID: i64 = 11;

const ACTIVITIES: [Activity; 2] = [
  Activity {
    blurb: "Pre-selects this facility when you install a manufacturing job from the planner.",
    id: MANUFACTURING_ACTIVITY_ID,
    name: "Manufacturing",
    placeholder: "Ask each install",
  },
  Activity {
    blurb: "Pre-selects this facility when you install a reaction job from the planner.",
    id: REACTION_ACTIVITY_ID,
    name: "Reactions",
    placeholder: "Ask each install",
  },
];

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
  Cleared {
    activity: i64,
  },
  FacilityPicked {
    activity: i64,
    facility: FacilityRef,
  },
  PickerToggled {
    activity: i64,
  },
  QueryChanged {
    activity: i64,
    query: String,
  },
  SearchResults {
    activity: i64,
    generation: u64,
    results: Vec<PlannerFacility>,
  },
  SelectionsResolved(Vec<(i64, PlannerFacility)>),
}

#[derive(Debug, Default)]
pub struct State {
  manufacturing: Picker,
  reactions: Picker,
}

impl State {
  pub fn from_settings(_settings: &Settings) -> Self {
    State::default()
  }

  fn picker(&self, activity: i64) -> &Picker {
    if activity == REACTION_ACTIVITY_ID {
      &self.reactions
    } else {
      &self.manufacturing
    }
  }

  fn picker_mut(&mut self, activity: i64) -> &mut Picker {
    if activity == REACTION_ACTIVITY_ID {
      &mut self.reactions
    } else {
      &mut self.manufacturing
    }
  }
}

#[derive(Clone, Copy, Debug)]
struct Activity {
  blurb: &'static str,
  id: i64,
  name: &'static str,
  placeholder: &'static str,
}

#[derive(Debug, Default)]
struct Picker {
  open: bool,
  search: FacilitySearch,
  selection: Option<FacilityRef>,
}

fn facility_ref(facility: &PlannerFacility, is_reaction: bool) -> FacilityRef {
  facility.to_ref(is_reaction)
}

/// Returns a pin only for player structures; NPC stations resolve from static data and yield `None`.
fn pin_for(facility: &FacilityRef) -> Option<PinnedStructure> {
  (facility.id >= MIN_STRUCTURE_ID).then(|| PinnedStructure {
    id: facility.id,
    name: facility.name.clone(),
    solar_system_id: facility.solar_system_id,
    type_id: facility.type_id,
  })
}

fn set_default(settings: &mut Settings, activity: i64, value: Option<i64>) {
  if activity == REACTION_ACTIVITY_ID {
    settings.industry_mut().set_reactions(value);
  } else {
    settings.industry_mut().set_manufacturing(value);
  }
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::Cleared {
      activity,
    } => {
      let picker = state.picker_mut(activity);
      picker.open = false;
      picker.selection = None;
      picker.search.clear();
      set_default(settings, activity, None);
      Outcome::Persist
    }
    Message::FacilityPicked {
      activity,
      facility,
    } => {
      let pin = pin_for(&facility);
      let picker = state.picker_mut(activity);
      picker.open = false;
      picker.selection = Some(facility.clone());
      picker.search.clear();
      set_default(settings, activity, Some(facility.id));
      match pin {
        Some(pin) => Outcome::IndustryPin(pin),
        None => Outcome::Persist,
      }
    }
    Message::PickerToggled {
      activity,
    } => {
      let picker = state.picker_mut(activity);
      picker.open = !picker.open;
      if !picker.open {
        picker.search.clear();
      }
      Outcome::None
    }
    Message::QueryChanged {
      activity,
      query,
    } => {
      // Typing into the always-visible trigger opens the picker for that activity, mirroring the planner.
      let picker = state.picker_mut(activity);
      picker.open = true;
      let generation = picker.search.set_query(query.clone());
      Outcome::IndustrySearch {
        activity,
        generation,
        query,
      }
    }
    Message::SearchResults {
      activity,
      generation,
      results,
    } => {
      let is_reaction = activity == REACTION_ACTIVITY_ID;
      let refs = results.iter().map(|f| facility_ref(f, is_reaction)).collect();
      state.picker_mut(activity).search.accept_results(generation, refs);
      Outcome::None
    }
    Message::SelectionsResolved(resolved) => {
      for (activity, facility) in resolved {
        let is_reaction = activity == REACTION_ACTIVITY_ID;
        state.picker_mut(activity).selection = Some(facility_ref(&facility, is_reaction));
      }
      Outcome::None
    }
  }
}

pub fn badge(settings: &Settings) -> String {
  let set =
    usize::from(settings.industry().manufacturing().is_some()) + usize::from(settings.industry().reactions().is_some());

  format!("{set}/{}", ACTIVITIES.len())
}

pub fn view<'a>(state: &'a State, _settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header();
  let body = panel_body(state);

  Column::with_children(vec![header, body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header<'a>() -> Element<'a, Message> {
  let title = text("Industry")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(
    "Set a default install structure per activity. Pod pre-selects it in the planner \u{2014} or leave \
      it on \u{201c}Ask each install\u{201d} to choose every time.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let band = container(identity).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn panel_body(state: &State) -> Element<'_, Message> {
  let mut sections: Vec<Element<'_, Message>> = Vec::with_capacity(ACTIVITIES.len());
  for activity in ACTIVITIES {
    sections.push(activity_section(state.picker(activity.id), activity));
  }

  let inner = container(
    Column::with_children(sections)
      .spacing(spacing::SPACE_6)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn activity_section(picker: &Picker, activity: Activity) -> Element<'_, Message> {
  let micro = text(activity.name)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent::PLASMA));
  let detail = text(activity.blurb)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let head = Column::with_children(vec![micro.into(), detail.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let id = activity.id;
  let trigger = FacilityCombobox::new()
    .placeholder(activity.placeholder)
    .selection(picker.selection.clone())
    .on_toggle(Message::PickerToggled {
      activity: id,
    })
    .trigger();

  // The facility popover floats below the trigger (width-matched) via AnchoredDropdown so opening it
  // never pushes the sibling activity sections down — it overlays the content instead.
  let dropdown =
    AnchoredDropdown::new(trigger, picker.open.then(|| popover(picker, activity))).on_dismiss(Message::PickerToggled {
      activity: id,
    });

  Column::with_children(vec![head.into(), dropdown.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}

fn popover(picker: &Picker, activity: Activity) -> Element<'_, Message> {
  let id = activity.id;
  let combobox = FacilityCombobox::new()
    .query(picker.search.query())
    .results(picker.search.results().to_vec())
    .on_input(move |query| Message::QueryChanged {
      activity: id,
      query,
    })
    .on_pick(move |facility: FacilityRef| Message::FacilityPicked {
      activity: id,
      facility,
    })
    .highlight(picker.search.highlight())
    .searching(picker.search.searching())
    .selection(picker.selection.clone())
    .on_clear(Message::Cleared {
      activity: id,
    })
    .popover();

  container(combobox)
    .width(Length::Fill)
    .style(|_| container::Style {
      shadow: crate::ui::style::shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn facility(id: i64) -> FacilityRef {
    FacilityRef {
      cost_index: Some(0.05),
      id,
      name: "Jita Keepstar".to_owned(),
      region: Some("The Forge".to_owned()),
      security_status: Some(0.9),
      solar_system: "Jita".to_owned(),
      solar_system_id: 30_000_142,
      type_id: Some(35_834),
    }
  }

  fn planner_facility(id: i64) -> PlannerFacility {
    PlannerFacility {
      id,
      manufacturing_index: Some(0.05),
      name: "Sotiyo".to_owned(),
      reaction_index: Some(0.04),
      region: Some("The Forge".to_owned()),
      security_status: Some(0.9),
      solar_system: Some("Jita".to_owned()),
      solar_system_id: 30_000_142,
      type_id: Some(35_827),
    }
  }

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_both_activities_when_each_has_a_default() {
      let mut settings = Settings::default();
      settings.industry_mut().set_manufacturing(Some(60_003_760));
      settings.industry_mut().set_reactions(Some(1_021_000_000_001));

      assert_eq!(badge(&settings), "2/2");
    }

    #[test]
    fn it_counts_only_the_activities_with_a_default() {
      let mut settings = Settings::default();
      settings.industry_mut().set_manufacturing(Some(60_003_760));

      assert_eq!(badge(&settings), "1/2");
    }

    #[test]
    fn it_counts_zero_defaults_for_a_fresh_config() {
      assert_eq!(badge(&Settings::default()), "0/2");
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_emits_a_search_outcome_for_the_activity() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(
        &mut state,
        Message::QueryChanged {
          activity: MANUFACTURING_ACTIVITY_ID,
          query: "Jita".to_owned(),
        },
        &mut settings,
      );

      assert_eq!(
        outcome,
        Outcome::IndustrySearch {
          activity: MANUFACTURING_ACTIVITY_ID,
          generation: 1,
          query: "Jita".to_owned(),
        }
      );
    }

    #[test]
    fn it_installs_results_only_for_the_current_generation() {
      let mut state = State::default();
      let mut settings = Settings::default();
      let Outcome::IndustrySearch {
        generation, ..
      } = update(
        &mut state,
        Message::QueryChanged {
          activity: MANUFACTURING_ACTIVITY_ID,
          query: "Sot".to_owned(),
        },
        &mut settings,
      )
      else {
        panic!("expected a search outcome");
      };

      update(
        &mut state,
        Message::SearchResults {
          activity: MANUFACTURING_ACTIVITY_ID,
          generation,
          results: vec![planner_facility(1_021_000_000_009)],
        },
        &mut settings,
      );

      assert_eq!(state.picker(MANUFACTURING_ACTIVITY_ID).search.results().len(), 1);
    }

    #[test]
    fn it_persists_a_station_id_and_signals_persist() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(
        &mut state,
        Message::FacilityPicked {
          activity: MANUFACTURING_ACTIVITY_ID,
          facility: facility(60_003_760),
        },
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(*settings.industry().manufacturing(), Some(60_003_760));
    }

    #[test]
    fn it_persists_a_structure_id_and_requests_a_pin() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let outcome = update(
        &mut state,
        Message::FacilityPicked {
          activity: REACTION_ACTIVITY_ID,
          facility: facility(1_021_000_000_001),
        },
        &mut settings,
      );

      assert!(matches!(outcome, Outcome::IndustryPin(_)));
      assert_eq!(*settings.industry().reactions(), Some(1_021_000_000_001));
    }

    #[test]
    fn it_removes_the_default_when_cleared() {
      let mut state = State::default();
      let mut settings = Settings::default();
      settings.industry_mut().set_manufacturing(Some(60_003_760));

      let outcome = update(
        &mut state,
        Message::Cleared {
          activity: MANUFACTURING_ACTIVITY_ID,
        },
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert_eq!(*settings.industry().manufacturing(), None);
    }

    #[test]
    fn it_seeds_the_trigger_display_from_resolved_selections() {
      let mut state = State::default();
      let mut settings = Settings::default();

      update(
        &mut state,
        Message::SelectionsResolved(vec![(REACTION_ACTIVITY_ID, planner_facility(1_021_000_000_009))]),
        &mut settings,
      );

      assert_eq!(
        state.picker(REACTION_ACTIVITY_ID).selection.as_ref().map(|f| f.id),
        Some(1_021_000_000_009)
      );
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_an_open_picker_with_results() {
      let mut state = State::default();
      let mut settings = Settings::default();
      update(
        &mut state,
        Message::PickerToggled {
          activity: MANUFACTURING_ACTIVITY_ID,
        },
        &mut settings,
      );
      let Outcome::IndustrySearch {
        generation, ..
      } = update(
        &mut state,
        Message::QueryChanged {
          activity: MANUFACTURING_ACTIVITY_ID,
          query: "Sot".to_owned(),
        },
        &mut settings,
      )
      else {
        panic!("expected a search outcome");
      };
      update(
        &mut state,
        Message::SearchResults {
          activity: MANUFACTURING_ACTIVITY_ID,
          generation,
          results: vec![planner_facility(1_021_000_000_009)],
        },
        &mut settings,
      );

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_industry_panel() {
      let state = State::default();
      let settings = Settings::default();

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
