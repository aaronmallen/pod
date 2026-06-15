pub mod empty_state;
pub mod from_queue_button;
pub mod from_selected_button;
pub mod new_plan_button;
pub mod plan_card;

use iced::{
  Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container},
};

use crate::{
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
  pub entry_count: usize,
  pub id: i64,
  pub name: String,
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
    return load_state_view(LoadStateView::Loading("Loading plans\u{2026}"));
  }

  if state.plans.is_empty() {
    if selection_count == 0 {
      return empty_state::empty_state();
    }
    return Column::with_children(vec![empty_state::empty_state(), footer(selection_count)])
      .width(Length::Fill)
      .into();
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

async fn load_plans(db: Database, character_id: i64) -> Vec<PlanRow> {
  let plans = skills::for_character(&db, character_id).await.unwrap_or_default();

  let mut rows = Vec::with_capacity(plans.len());
  for plan in plans {
    let entry_count = skills::entries(&db, plan.id())
      .await
      .map(|entries| entries.len())
      .unwrap_or(0);
    rows.push(PlanRow {
      entry_count,
      id: plan.id(),
      name: plan.name().to_owned(),
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

  fn row(id: i64, name: &str, entry_count: usize) -> PlanRow {
    PlanRow {
      entry_count,
      id,
      name: name.to_owned(),
      updated: "2 Jun '26".to_owned(),
    }
  }

  mod fmt_plan_date {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_an_rfc3339_timestamp_as_a_compact_date() {
      assert_eq!(fmt_plan_date("2026-06-02T13:45:00Z"), "2 Jun '26");
    }

    #[test]
    fn it_falls_back_to_the_date_prefix_on_an_unparsable_string() {
      assert_eq!(fmt_plan_date("not-a-date-string"), "not-a-date");
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

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_loading_state_before_first_load() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state, 0);
    }

    #[test]
    fn it_renders_the_empty_state_when_loaded_with_no_plans() {
      let mut state = State::new();
      state.loaded = true;

      let _el: Element<'_, Message> = view(&state, 0);
    }

    #[test]
    fn it_renders_the_from_selected_button_over_the_empty_state_with_a_selection() {
      let mut state = State::new();
      state.loaded = true;

      let _el: Element<'_, Message> = view(&state, 3);
    }

    #[test]
    fn it_renders_a_plan_list_with_an_armed_confirm() {
      let mut state = State::new();
      state.loaded = true;
      state.plans = vec![row(1, "Combat", 5), row(2, "Industry", 0)];
      state.confirm_delete = Some(2);

      let _el: Element<'_, Message> = view(&state, 2);
    }
  }
}
