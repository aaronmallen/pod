use chrono::{DateTime, Utc};
use iced::{
  Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, Space, button, text},
};

use super::{Message, PickerPilot, State, fmt_duration, fmt_eta, fmt_sp, queue_remaining_seconds};
use crate::{
  config::Feature,
  features::{registry, roster},
  ui::{
    components::{
      header::{header as header_band, header_divider, stat_block},
      icon::Icon,
      picker::{
        PickerGroup, TriggerPortrait, picker_character_row, picker_dropdown as picker_dropdown_panel, picker_trigger,
        trigger_identity,
      },
    },
    style::{color, radius, spacing, typography},
  },
};

pub(super) fn header<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let queue = state.queue.as_slice();
  let total_sp = state
    .roster
    .iter()
    .find(|pilot| pilot.id == state.active)
    .map(|pilot| pilot.total_sp)
    .unwrap_or(0);
  let remaining = queue_remaining_seconds(queue, now);

  let left: Vec<Element<'a, Message>> = vec![
    character_picker(state),
    header_divider(),
    stat_block(
      "Total skill points",
      format!("{} SP", fmt_sp(total_sp)),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    queue_stat(queue.len(), remaining),
  ];

  let mut right: Vec<Element<'a, Message>> = vec![
    manage_plans_button(),
    Space::new().width(Length::Fixed(spacing::SPACE_2)).into(),
    compare_button(),
  ];
  if let Some(seconds) = remaining.filter(|secs| *secs > 0) {
    right.push(Space::new().width(Length::Fixed(spacing::SPACE_3)).into());
    right.push(stat_block(
      "Queue completes",
      format!("{} EVE", fmt_eta(now, seconds)),
      color::text::PRIMARY,
      None,
    ));
  }

  header_band(left, right)
}

fn manage_plans_button<'a>() -> Element<'a, Message> {
  let label = Row::with_children(vec![
    Icon::plans().size(16.0).color(color::text::secondary()).render(),
    text("Manage Plans")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center);

  button(label)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::OpenManagePlans)
    .style(|_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: None,
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, if hover { 0.25 } else { 0.1 }),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: if hover {
          color::text::PRIMARY
        } else {
          color::text::secondary()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn compare_button<'a>() -> Element<'a, Message> {
  let label = Row::with_children(vec![
    Icon::compare().size(16.0).color(color::text::secondary()).render(),
    text("Compare")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center);

  button(label)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(Message::OpenCompare)
    .style(|_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: None,
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, if hover { 0.25 } else { 0.1 }),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: if hover {
          color::text::PRIMARY
        } else {
          color::text::secondary()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn character_picker(state: &State) -> Element<'_, Message> {
  let active = state.roster.iter().find(|pilot| pilot.id == state.active);
  let name = active.map(|pilot| pilot.name.clone()).unwrap_or_default();
  let corp = active.map(|pilot| pilot.corp.clone()).unwrap_or_default();
  let portrait = TriggerPortrait {
    id: state.active,
    name: name.clone(),
    path: active.and_then(|pilot| pilot.portrait.path()),
  };

  picker_trigger(
    trigger_identity(name, corp, Some(portrait)),
    state.picker_open,
    Message::PickerToggled,
  )
}

pub(super) fn picker_dropdown(state: &State) -> Element<'_, Message> {
  let rows: Vec<Element<'_, Message>> = state
    .roster
    .iter()
    .map(|pilot| picker_row(pilot, pilot.id == state.active))
    .collect();

  let groups = vec![PickerGroup {
    title: None,
    items: rows,
  }];

  picker_dropdown_panel(groups)
}

fn picker_row(pilot: &PickerPilot, selected: bool) -> Element<'_, Message> {
  let total_sp: Element<'_, Message> = text(format!("{} SP", fmt_sp(pilot.total_sp)))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into();

  let required_scopes = registry::descriptor(Feature::SkillMonitoring).scopes;
  let needs_reauth = roster::needs_reauthorization(pilot.granted_scopes.as_deref(), required_scopes);

  picker_character_row(
    pilot.id,
    pilot.name.clone(),
    pilot.corp.to_uppercase(),
    pilot.portrait.path(),
    Some(total_sp),
    selected,
    needs_reauth.then(|| Feature::SkillMonitoring.noun()),
    Message::CharacterChanged(pilot.id),
  )
}

fn queue_stat<'a>(len: usize, remaining: Option<i64>) -> Element<'a, Message> {
  let noun = if len == 1 { "skill" } else { "skills" };
  let value = match remaining {
    Some(seconds) if seconds > 0 => fmt_duration(seconds),
    _ => "Empty".to_owned(),
  };
  let value_color = if is_low_queue(len, remaining) {
    color::status::DANGER
  } else {
    color::text::PRIMARY
  };
  stat_block(&format!("Queue · {len} {noun}"), value, value_color, None)
}

fn is_low_queue(len: usize, remaining: Option<i64>) -> bool {
  match remaining {
    Some(seconds) if seconds > 0 => len <= 1 && seconds < super::SECONDS_PER_DAY,
    _ => true,
  }
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone as _;

  use super::*;
  use crate::store::images;

  fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
  }

  fn pilot(id: i64, name: &str) -> PickerPilot {
    PickerPilot {
      corp: "TEST".to_owned(),
      granted_scopes: None,
      id,
      name: name.to_owned(),
      portrait: images::ImageState::Stale {
        id,
        kind: images::ImageKind::CharacterPortrait,
      },
      total_sp: 47_320_400,
    }
  }

  mod header {
    use super::*;

    #[test]
    fn it_renders_with_a_closed_picker() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, "Test Pilot"), pilot(7, "Wingmate")];

      let _el: Element<'_, Message> = header(&state, now());
    }

    #[test]
    fn it_renders_with_an_open_picker() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, "Test Pilot"), pilot(7, "Wingmate")];
      state.picker_open = true;

      let _el: Element<'_, Message> = header(&state, now());
    }

    #[test]
    fn it_renders_the_manage_plans_trigger() {
      let _el: Element<'_, Message> = manage_plans_button();
    }
  }

  mod is_low_queue {
    use super::*;

    #[test]
    fn it_does_not_flag_a_multi_skill_queue() {
      assert!(!is_low_queue(3, Some(3_600)));
    }

    #[test]
    fn it_does_not_flag_a_single_long_entry() {
      let two_days = 2 * 86_400;
      assert!(!is_low_queue(1, Some(two_days)));
    }

    #[test]
    fn it_flags_a_single_short_entry() {
      assert!(is_low_queue(1, Some(3_600)));
    }

    #[test]
    fn it_flags_an_empty_queue() {
      assert!(is_low_queue(0, None));
      assert!(is_low_queue(0, Some(0)));
    }
  }
}
