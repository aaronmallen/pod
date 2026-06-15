mod col_header;
mod empty_state;
mod footer;
mod row;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  widget::{Column, container},
};

use super::{
  Message,
  queue::{ComputedQueue, QueueSelection},
};
use crate::{
  store::model::CharacterSkillqueue,
  ui::style::{color, radius},
};

const QUEUE_SIDE_MARGIN: f32 = 28.0;

pub fn queue_section<'a>(
  computed: &'a ComputedQueue,
  head: Option<&'a CharacterSkillqueue>,
  selection: &'a QueueSelection,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let has_active = head.is_some_and(|entry| entry.start_date().is_some() && entry.finish_date().is_some());
  let skip_n = usize::from(has_active);

  if computed.items.len() <= skip_n {
    return empty_state::empty_state();
  }

  queue_list(computed, skip_n, selection, now)
}

fn queue_list<'a>(
  computed: &'a ComputedQueue,
  skip_n: usize,
  selection: &'a QueueSelection,
  now: DateTime<Utc>,
) -> Element<'a, Message> {
  let total_n = computed.items.len() - skip_n;

  let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(computed.items.len() + 2);
  children.push(col_header::col_header());
  for (display_index, item) in computed.items.iter().skip(skip_n).enumerate() {
    let selected = selection.contains(item.queue_position);
    children.push(row::row(item, display_index, selected, now));
  }
  children.push(footer::footer(total_n, computed.total_secs, selection.len(), now));

  let card = container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  container(card)
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: QUEUE_SIDE_MARGIN,
      bottom: QUEUE_SIDE_MARGIN,
      left: QUEUE_SIDE_MARGIN,
    })
    .into()
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;
  use crate::features::skills::queue::{Attr, ComputedQueueItem};

  fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
  }

  fn item(skill_name: &str) -> ComputedQueueItem {
    ComputedQueueItem {
      cum_start_secs: 0.0,
      duration_secs: 3_600.0,
      from_level: 0,
      group_name: "Gunnery".to_owned(),
      primary: Attr::Perception,
      progress: 0.0,
      queue_position: 0,
      rank: 1,
      secondary: Attr::Willpower,
      skill_name: skill_name.to_owned(),
      sp_needed: 250,
      sp_now: 0,
      sp_to: 250,
      to_level: 1,
    }
  }

  fn computed(n: usize) -> ComputedQueue {
    let items = (0..n).map(|i| item(&format!("Skill {i}"))).collect();
    ComputedQueue {
      items,
      sp_rate: 1.0,
      total_secs: 3_600.0 * n as f64,
      total_sp: 0,
    }
  }

  fn active_head() -> CharacterSkillqueue {
    CharacterSkillqueue {
      character_id: 42,
      finish_date: Some("2026-06-11T00:00:00Z".to_owned()),
      finished_level: 5,
      level_end_sp: None,
      level_start_sp: None,
      queue_position: 0,
      skill_id: 100,
      start_date: Some("2026-06-01T00:00:00Z".to_owned()),
      training_start_sp: None,
    }
  }

  fn paused_head() -> CharacterSkillqueue {
    CharacterSkillqueue {
      start_date: None,
      finish_date: None,
      ..active_head()
    }
  }

  #[test]
  fn it_renders_the_empty_state_when_only_the_active_head_remains() {
    let computed = computed(1);
    let head = active_head();
    let selection = QueueSelection::default();
    let _el: Element<'_, Message> = queue_section(&computed, Some(&head), &selection, now());
  }

  #[test]
  fn it_renders_the_empty_state_for_an_empty_queue() {
    let computed = ComputedQueue::default();
    let selection = QueueSelection::default();
    let _el: Element<'_, Message> = queue_section(&computed, None, &selection, now());
  }

  #[test]
  fn it_renders_the_list_excluding_the_active_head() {
    let computed = computed(2);
    let head = active_head();
    let selection = QueueSelection::default();
    let _el: Element<'_, Message> = queue_section(&computed, Some(&head), &selection, now());
  }

  #[test]
  fn it_includes_every_row_when_the_head_is_paused() {
    let computed = computed(2);
    let head = paused_head();
    let selection = QueueSelection::default();
    let _el: Element<'_, Message> = queue_section(&computed, Some(&head), &selection, now());
  }

  mod skip_n {
    use pretty_assertions::assert_eq;

    use super::{active_head, paused_head};
    use crate::store::model::CharacterSkillqueue;

    fn has_active(head: Option<&CharacterSkillqueue>) -> usize {
      let active = head.is_some_and(|entry| entry.start_date().is_some() && entry.finish_date().is_some());
      usize::from(active)
    }

    #[test]
    fn it_skips_one_for_a_dated_active_head() {
      assert_eq!(has_active(Some(&active_head())), 1);
    }

    #[test]
    fn it_skips_none_for_a_paused_head() {
      assert_eq!(has_active(Some(&paused_head())), 0);
    }

    #[test]
    fn it_skips_none_with_no_head() {
      assert_eq!(has_active(None), 0);
    }
  }
}
