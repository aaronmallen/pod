pub mod empty_state;
pub mod from_queue_button;
pub mod from_selected_button;
pub mod new_plan_button;
pub mod plan_card;

use iced::{
  Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container},
};

use crate::{
  features::skills::plan_math::{self, PlanStep},
  store::{Database, repo::skills},
  ui::{
    components::{
      card::card,
      empty_state::{LoadStateView, load_state_view},
    },
    style::spacing,
  },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanRow {
  pub distinct_skills: usize,
  pub id: i64,
  pub name: String,
  pub remaining_steps: usize,
  pub updated: String,
}

#[derive(Clone, Debug)]
pub enum Message {
  DeleteCancelled,
  DeleteConfirmed(i64),
  DeleteRequested(i64),
  FromQueue,
  FromSelected,
  Loaded(Vec<PlanRow>),
  NewPlan,
  OpenPlan(i64),
}

#[derive(Debug, Default)]
pub struct State {
  confirm_delete: Option<i64>,
  loaded: bool,
  plans: Vec<PlanRow>,
}

impl State {
  pub fn new() -> Self {
    State::default()
  }
}

pub fn load(db: &Database, character_id: i64) -> iced::Task<Message> {
  iced::Task::perform(load_plans(db.clone(), character_id), Message::Loaded)
}

pub fn update(state: &mut State, message: Message, db: &Database, character_id: i64) -> iced::Task<Message> {
  match message {
    Message::DeleteCancelled => {
      state.confirm_delete = None;
      iced::Task::none()
    }
    Message::DeleteConfirmed(plan_id) => {
      state.confirm_delete = None;
      let db = db.clone();
      iced::Task::perform(
        async move {
          if let Err(error) = skills::delete(&db, plan_id).await {
            tracing::error!(plan_id, %error, "failed to delete skill plan");
          }
          load_plans(db, character_id).await
        },
        Message::Loaded,
      )
    }
    Message::DeleteRequested(plan_id) => {
      state.confirm_delete = Some(plan_id);
      iced::Task::none()
    }
    Message::FromQueue => iced::Task::none(),
    Message::FromSelected => iced::Task::none(),
    Message::Loaded(plans) => {
      state.plans = plans;
      state.loaded = true;
      if let Some(id) = state.confirm_delete
        && !state.plans.iter().any(|plan| plan.id == id)
      {
        state.confirm_delete = None;
      }
      iced::Task::none()
    }
    Message::NewPlan => iced::Task::none(),
    Message::OpenPlan(_) => iced::Task::none(),
  }
}

pub fn view(state: &State, selection_count: usize) -> Element<'_, Message> {
  if !state.loaded {
    // `LoadStateView::Loading` borrows its message for the returned element's lifetime, so the
    // resolved string must outlive this function; cache it once to hand a `&'static str`.
    static LOADING_MESSAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let loading = LOADING_MESSAGE.get_or_init(|| t!("skills.panel_plans.loading").into_owned());
    return load_state_view(LoadStateView::Loading(loading));
  }

  if state.plans.is_empty() {
    // The empty-state component already renders its own centered "New plan" /
    // "From queue" pair, so the full footer (which renders the same pair) is
    // omitted here to avoid duplicate buttons. Only the "From selected"
    // affordance is surfaced, and only when a selection exists.
    let mut children: Vec<Element<'_, Message>> = vec![empty_state::empty_state()];
    if shows_empty_state_from_selected(selection_count) {
      children.push(from_selected_footer(selection_count));
    }
    return Column::with_children(children).width(Length::Fill).into();
  }

  let cards: Vec<Element<'_, Message>> = state
    .plans
    .iter()
    .enumerate()
    .map(|(index, plan)| {
      let confirm = state.confirm_delete == Some(plan.id);
      plan_card::plan_card(plan, index == 0, confirm)
    })
    .collect();

  let items: Vec<Element<'_, Message>> = vec![
    card(Column::with_children(cards).width(Length::Fill)),
    footer(selection_count),
  ];

  Column::with_children(items).width(Length::Fill).into()
}

fn footer<'a>(selection_count: usize) -> Element<'a, Message> {
  let mut row: Vec<Element<'a, Message>> = vec![
    new_plan_button::new_plan_button(),
    Space::new().width(Length::Fixed(spacing::SPACE_2)).into(),
    from_queue_button::from_queue_button(),
  ];
  if selection_count > 0 {
    row.push(Space::new().width(Length::Fixed(spacing::SPACE_2)).into());
    row.push(from_selected_button::from_selected_button(selection_count));
  }

  container(Row::with_children(row).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .into()
}

/// Whether the empty state should surface the "From selected" affordance.
///
/// The empty state never renders the duplicate "New plan" / "From queue" pair
/// (the empty-state component already provides them), so the only conditional
/// footer button is "From selected", shown when a skill selection exists.
fn shows_empty_state_from_selected(selection_count: usize) -> bool {
  selection_count > 0
}

fn from_selected_footer<'a>(selection_count: usize) -> Element<'a, Message> {
  container(from_selected_button::from_selected_button(selection_count))
    .align_x(Horizontal::Center)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .into()
}

async fn load_plans(db: Database, character_id: i64) -> Vec<PlanRow> {
  use crate::store::repo::character;

  let plans = skills::for_character(&db, character_id).await.unwrap_or_default();

  // Fetch the character's trained levels once for the whole loader so remaining
  // counts reflect per-character progress without an N+1 over plans.
  let trained: std::collections::HashMap<i64, u8> = character::skills(&db, character_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|skill| (skill.skill_id(), skill.trained_skill_level().clamp(0, 5) as u8))
    .collect();

  let mut rows = Vec::with_capacity(plans.len());
  for plan in plans {
    let steps: Vec<PlanStep> = skills::entries(&db, plan.id())
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|entry| PlanStep {
        skill_id: entry.skill_id(),
        to_level: entry.to_level().clamp(0, 5) as u8,
      })
      .collect();

    rows.push(PlanRow {
      distinct_skills: plan_math::distinct_skills(&steps),
      id: plan.id(),
      name: plan.name().to_owned(),
      remaining_steps: plan_math::remaining_steps(&steps, &trained),
      updated: fmt_plan_date(plan.updated_at()),
    });
  }
  rows
}

fn fmt_plan_date(updated_at: &str) -> String {
  const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
  ];
  match chrono::DateTime::parse_from_rfc3339(updated_at) {
    Ok(dt) => {
      use chrono::Datelike as _;
      let date = dt.naive_utc().date();
      let month = MONTHS[(date.month() as usize).saturating_sub(1).min(11)];
      format!("{} {} '{:02}", date.day(), month, date.year() % 100)
    }
    Err(_) => updated_at.get(..10).unwrap_or(updated_at).to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn row(id: i64, name: &str, remaining_steps: usize) -> PlanRow {
    PlanRow {
      distinct_skills: remaining_steps,
      id,
      name: name.to_owned(),
      remaining_steps,
      updated: "2 Jun '26".to_owned(),
    }
  }

  mod fmt_plan_date {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_date_prefix_on_an_unparsable_string() {
      assert_eq!(fmt_plan_date("not-a-date-string"), "not-a-date");
    }

    #[test]
    fn it_formats_an_rfc3339_timestamp_as_a_compact_date() {
      assert_eq!(fmt_plan_date("2026-06-02T13:45:00Z"), "2 Jun '26");
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_arms_and_disarms_the_delete_confirm() {
      let mut state = State::new();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::DeleteRequested(7), &db, 42);
      assert_eq!(state.confirm_delete, Some(7));

      let _ = update(&mut state, Message::DeleteCancelled, &db, 42);
      assert_eq!(state.confirm_delete, None);
    }

    #[tokio::test]
    async fn it_marks_loaded_and_drops_a_stale_confirm_arm() {
      let mut state = State::new();
      let db = crate::store::open_test().await.unwrap();
      state.confirm_delete = Some(99);

      let _ = update(&mut state, Message::Loaded(vec![row(1, "Combat", 3)]), &db, 42);

      assert!(state.loaded);
      assert_eq!(state.plans.len(), 1);
      assert_eq!(state.confirm_delete, None, "confirm for a missing plan is dropped");
    }

    #[tokio::test]
    async fn the_editor_seam_messages_are_no_ops() {
      let mut state = State::new();
      let db = crate::store::open_test().await.unwrap();

      let _ = update(&mut state, Message::NewPlan, &db, 42);
      let _ = update(&mut state, Message::FromQueue, &db, 42);
      let _ = update(&mut state, Message::FromSelected, &db, 42);
      let _ = update(&mut state, Message::OpenPlan(3), &db, 42);

      assert!(state.plans.is_empty());
      assert_eq!(state.confirm_delete, None);
    }
  }

  mod shows_empty_state_from_selected {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_hides_the_from_selected_footer_without_a_selection() {
      // No selection => only the empty-state component renders, so its centered
      // "New plan" / "From queue" pair is the *only* button pair (no duplicate
      // footer pair).
      assert_eq!(shows_empty_state_from_selected(0), false);
    }

    #[test]
    fn it_shows_the_from_selected_footer_with_a_selection() {
      assert_eq!(shows_empty_state_from_selected(3), true);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_plan_list_with_an_armed_confirm() {
      let mut state = State::new();
      state.loaded = true;
      state.plans = vec![row(1, "Combat", 5), row(2, "Industry", 0)];
      state.confirm_delete = Some(2);

      let _el: Element<'_, Message> = view(&state, 2);
    }

    #[test]
    fn it_renders_the_empty_state_without_the_duplicate_footer_when_loaded_with_no_plans() {
      let mut state = State::new();
      state.loaded = true;

      // With no selection the empty state omits the footer entirely, so the
      // "New plan" / "From queue" pair appears exactly once (in the empty-state
      // component itself).
      assert!(!shows_empty_state_from_selected(0));
      let _el: Element<'_, Message> = view(&state, 0);
    }

    #[test]
    fn it_renders_the_from_selected_button_over_the_empty_state_with_a_selection() {
      let mut state = State::new();
      state.loaded = true;

      let _el: Element<'_, Message> = view(&state, 3);
    }

    #[test]
    fn it_renders_the_loading_state_before_first_load() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state, 0);
    }
  }
}
