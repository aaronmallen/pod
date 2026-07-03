use std::{collections::HashMap, path::PathBuf};

use chrono::{DateTime, Utc};
use iced::{
  Background, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Space, column, container, row, scrollable, text},
};

use crate::{
  features::skills::{
    plan_csv::{self, PlanCsvRow},
    plan_math::injectors_for_plan,
    plan_summary::{
      injector_section::injector_section, plan_totals_section::plan_totals_section,
      time_by_group_section::time_by_group_section, time_by_pair_section::time_by_pair_section,
    },
    queue::{Attr, ComputedQueue},
  },
  ui::{
    components::{
      button::{Button, Size},
      eyebrow::eyebrow,
      rule,
    },
    style::{color, spacing, typography},
  },
};

#[derive(Clone, Debug)]
pub enum Message {
  ExportCsvRequested,
  ExportPlanRequested,
  Saved,
}

pub fn update(computed: &ComputedQueue, message: Message) -> Task<Message> {
  match message {
    Message::ExportCsvRequested => {
      let contents = plan_csv::to_csv(&csv_rows(computed));
      Task::perform(save_file("skill-queue.csv".to_owned(), contents), |_| Message::Saved)
    }
    Message::ExportPlanRequested => {
      let contents = plan_text(computed);
      Task::perform(save_file("skill-plan.txt".to_owned(), contents), |_| Message::Saved)
    }
    Message::Saved => Task::none(),
  }
}

pub fn view<'a>(computed: &ComputedQueue, now: DateTime<Utc>) -> Element<'a, Message> {
  if computed.items.is_empty() {
    return empty_state();
  }

  let remaining_sp = remaining_queue_sp(computed);
  let steps = computed.items.len();

  let mut sections: Vec<Element<'a, Message>> = vec![plan_totals_section(
    computed.total_secs,
    remaining_sp,
    steps,
    false,
    now,
  )];

  if remaining_sp > 0 {
    let estimate = injectors_for_plan(remaining_sp, computed.total_sp.max(0) as u64);
    sections.push(rule::horizontal());
    sections.push(injector_section(estimate, remaining_sp));
  }

  let group_sec = group_seconds(computed);
  if !group_sec.is_empty() {
    sections.push(rule::horizontal());
    sections.push(time_by_group_section(&group_sec));
  }

  let pair_sec = pair_seconds(computed);
  if !pair_sec.is_empty() {
    sections.push(rule::horizontal());
    sections.push(time_by_pair_section(&pair_sec));
  }

  sections.push(Space::new().height(spacing::SPACE_6).into());

  let body = scrollable(column(sections).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .height(Length::Fill);

  column(vec![body.into(), footer()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn footer<'a>() -> Element<'a, Message> {
  let buttons = row(vec![
    Button::secondary(t!("skills.panel_queue.export_plan"))
      .size(Size::Sm)
      .block()
      .on_press(Message::ExportPlanRequested)
      .into(),
    Space::new().width(spacing::SPACE_2).into(),
    Button::secondary(t!("skills.panel_queue.export_csv"))
      .size(Size::Sm)
      .block()
      .on_press(Message::ExportCsvRequested)
      .into(),
  ])
  .width(Length::Fill)
  .align_y(Vertical::Center);

  container(buttons)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      ..container::Style::default()
    })
    .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  let body = column(vec![
    eyebrow(&t!("skills.panel_queue.empty_eyebrow"), None),
    Space::new().height(spacing::SPACE_2).into(),
    text(t!("skills.panel_queue.empty_body"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .align_x(Horizontal::Center)
  .max_width(260.0);

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: spacing::SPACE_6,
      bottom: spacing::SPACE_6,
      left: spacing::SPACE_6,
      right: spacing::SPACE_6,
    })
    .into()
}

/// Sums each item's still-needed `sp_needed`, not the character's `total_sp`.
fn remaining_queue_sp(computed: &ComputedQueue) -> u64 {
  computed.items.iter().map(|item| item.sp_needed).sum()
}

fn group_seconds(computed: &ComputedQueue) -> HashMap<String, f64> {
  let mut totals: HashMap<String, f64> = HashMap::new();
  for item in &computed.items {
    if item.duration_secs <= 0.0 {
      continue;
    }
    let group = if item.group_name.is_empty() {
      t!("skills.plan.group_other").into_owned()
    } else {
      item.group_name.clone()
    };
    *totals.entry(group).or_insert(0.0) += item.duration_secs;
  }
  totals
}

fn pair_seconds(computed: &ComputedQueue) -> HashMap<String, f64> {
  let mut totals: HashMap<String, f64> = HashMap::new();
  for item in &computed.items {
    if item.duration_secs <= 0.0 {
      continue;
    }
    let pair = format!("{} / {}", attr_long(item.primary), attr_long(item.secondary));
    *totals.entry(pair).or_insert(0.0) += item.duration_secs;
  }
  totals
}

fn attr_long(attr: Attr) -> String {
  match attr {
    Attr::Charisma => t!("skills.panel_attributes.attr_charisma"),
    Attr::Intelligence => t!("skills.panel_attributes.attr_intelligence"),
    Attr::Memory => t!("skills.panel_attributes.attr_memory"),
    Attr::Perception => t!("skills.panel_attributes.attr_perception"),
    Attr::Willpower => t!("skills.panel_attributes.attr_willpower"),
  }
  .into_owned()
}

fn plan_text(computed: &ComputedQueue) -> String {
  let mut out = computed
    .items
    .iter()
    .map(|item| format!("{} {}", item.skill_name, item.to_level))
    .collect::<Vec<_>>()
    .join("\n");
  out.push('\n');
  out
}

fn csv_rows(computed: &ComputedQueue) -> Vec<PlanCsvRow> {
  computed
    .items
    .iter()
    .map(|item| PlanCsvRow {
      skill: item.skill_name.clone(),
      group: item.group_name.clone(),
      primary: attr_long(item.primary),
      secondary: attr_long(item.secondary),
      level: item.to_level,
      sp: item.sp_needed as f64,
      duration_secs: item.duration_secs as i64,
    })
    .collect()
}

async fn save_file(default_name: String, contents: String) -> Option<PathBuf> {
  #[cfg(not(test))]
  {
    let handle = rfd::AsyncFileDialog::new()
      .set_title(t!("skills.plan.export_dialog_title").into_owned())
      .set_file_name(default_name)
      .save_file()
      .await?;
    let path = handle.path().to_path_buf();
    if std::fs::write(&path, contents).is_err() {
      return None;
    }
    Some(path)
  }
  #[cfg(test)]
  {
    let _ = (default_name, contents);
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::skills::queue::ComputedQueueItem;

  fn item(
    skill_name: &str,
    group_name: &str,
    primary: Attr,
    secondary: Attr,
    sp_needed: u64,
    secs: f64,
  ) -> ComputedQueueItem {
    ComputedQueueItem {
      cum_start_secs: 0.0,
      duration_secs: secs,
      from_level: 0,
      group_name: group_name.to_owned(),
      primary,
      progress: 0.0,
      queue_position: 0,
      rank: 1,
      secondary,
      skill_name: skill_name.to_owned(),
      sp_needed,
      sp_now: 0,
      sp_to: 0,
      to_level: 5,
    }
  }

  fn queue(items: Vec<ComputedQueueItem>) -> ComputedQueue {
    ComputedQueue {
      total_secs: items.iter().map(|i| i.duration_secs).sum(),
      total_sp: 50_000_000,
      sp_rate: 1_000.0,
      items,
    }
  }

  mod remaining_queue_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_the_remaining_needed_sp_not_the_character_total() {
      let computed = queue(vec![
        item(
          "Gunnery",
          "Gunnery",
          Attr::Perception,
          Attr::Willpower,
          128_000,
          3_600.0,
        ),
        item("Drones", "Drones", Attr::Memory, Attr::Perception, 256_000, 7_200.0),
      ]);
      assert_eq!(remaining_queue_sp(&computed), 384_000);
    }
  }

  mod group_seconds {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_folds_missing_group_names_into_other() {
      let computed = queue(vec![item(
        "Mystery",
        "",
        Attr::Perception,
        Attr::Willpower,
        100,
        3_600.0,
      )]);
      let totals = group_seconds(&computed);
      assert_eq!(totals.get(&t!("skills.plan.group_other").into_owned()), Some(&3_600.0));
    }

    #[test]
    fn it_accumulates_seconds_per_group() {
      let computed = queue(vec![
        item("A", "Gunnery", Attr::Perception, Attr::Willpower, 100, 3_600.0),
        item("B", "Gunnery", Attr::Perception, Attr::Willpower, 100, 1_800.0),
      ]);
      let totals = group_seconds(&computed);
      assert_eq!(totals.get("Gunnery"), Some(&5_400.0));
    }
  }

  mod pair_seconds {
    use super::*;

    #[test]
    fn it_keys_pairs_by_long_attribute_labels() {
      let computed = queue(vec![item(
        "A",
        "Gunnery",
        Attr::Perception,
        Attr::Willpower,
        100,
        3_600.0,
      )]);
      let totals = pair_seconds(&computed);
      assert!(totals.keys().any(|k| k.contains(" / ")));
    }
  }

  mod plan_text {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_writes_skill_name_and_level_lines() {
      let computed = queue(vec![
        item("Gunnery", "Gunnery", Attr::Perception, Attr::Willpower, 100, 3_600.0),
        item("Drones", "Drones", Attr::Memory, Attr::Perception, 100, 3_600.0),
      ]);
      assert_eq!(plan_text(&computed), "Gunnery 5\nDrones 5\n");
    }
  }

  mod csv_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_needed_sp_and_duration_into_rows() {
      let computed = queue(vec![item(
        "Gunnery",
        "Gunnery",
        Attr::Perception,
        Attr::Willpower,
        128_000,
        3_600.0,
      )]);
      let rows = csv_rows(&computed);
      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].sp, 128_000.0);
      assert_eq!(rows[0].duration_secs, 3_600);
      assert_eq!(rows[0].level, 5);
      assert_eq!(rows[0].primary, "Perception");
      assert_eq!(rows[0].secondary, "Willpower");
    }
  }

  mod update {
    use super::*;

    #[test]
    fn it_builds_a_save_task_for_csv_export() {
      let computed = queue(vec![item(
        "Gunnery",
        "Gunnery",
        Attr::Perception,
        Attr::Willpower,
        128_000,
        3_600.0,
      )]);

      let _task = update(&computed, Message::ExportCsvRequested);
    }

    #[test]
    fn it_builds_a_save_task_for_plan_export() {
      let computed = queue(vec![item(
        "Gunnery",
        "Gunnery",
        Attr::Perception,
        Attr::Willpower,
        128_000,
        3_600.0,
      )]);

      let _task = update(&computed, Message::ExportPlanRequested);
    }

    #[test]
    fn it_does_nothing_on_saved() {
      let computed = queue(vec![]);

      let _task = update(&computed, Message::Saved);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_empty_state_without_items() {
      let _el: Element<'_, Message> = view(&ComputedQueue::default(), Utc::now());
    }

    #[test]
    fn it_renders_the_breakdown_with_items() {
      let computed = queue(vec![item(
        "Gunnery",
        "Gunnery",
        Attr::Perception,
        Attr::Willpower,
        128_000,
        3_600.0,
      )]);
      let _el: Element<'_, Message> = view(&computed, Utc::now());
    }
  }
}
