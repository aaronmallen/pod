use chrono::{DateTime, Utc};
use iced::{
  Element, Length,
  widget::{Column, Space, button, container, text},
};

use super::{Message, PickerPilot, State, fmt_duration, fmt_eta, fmt_sp, queue_remaining_seconds};
use crate::{
  config::Feature,
  features::{roster, shell::registry},
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      button::{Button, Size},
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

const HEADER_BUTTON_HEIGHT: f32 = 36.0;

const PLAN_MENU_WIDTH: f32 = 190.0;

pub(super) fn header<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let queue = state.queue.as_slice();
  let total_sp = state
    .roster
    .iter()
    .find(|pilot| pilot.id == state.active)
    .map(|pilot| pilot.total_sp)
    .unwrap_or(0);
  let remaining = queue_remaining_seconds(queue, now);

  let mut left: Vec<Element<'a, Message>> = vec![
    character_picker(state),
    header_divider(),
    stat_block(
      &t!("skills.header.total_sp_label"),
      t!("skills.header.sp_value", sp => fmt_sp(total_sp)).into_owned(),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    queue_stat(queue.len(), remaining),
  ];
  if let Some(seconds) = remaining.filter(|secs| *secs > 0) {
    left.push(header_divider());
    left.push(stat_block(
      &t!("skills.header.queue_completes_label"),
      t!("skills.header.eve_value", eta => fmt_eta(now, seconds)).into_owned(),
      color::text::PRIMARY,
      None,
    ));
  }

  let right: Vec<Element<'a, Message>> = vec![
    plan_dropdown(state),
    Space::new().width(Length::Fixed(spacing::SPACE_2)).into(),
    compare_button(),
  ];

  header_band(left, right)
}

fn plan_dropdown(state: &State) -> Element<'_, Message> {
  let trigger: Element<'_, Message> = Button::secondary(t!("skills.header.plan"))
    .icon(Icon::plans())
    .icon_right(Icon::chevron_down())
    .size(Size::Sm)
    .height(HEADER_BUTTON_HEIGHT)
    .on_press(Message::PlanMenuToggled)
    .into();

  let popover = state.plan_menu_open.then(plan_menu);
  AnchoredDropdown::new(trigger, popover)
    .on_dismiss(Message::PlanMenuDismissed)
    .popover_width(PLAN_MENU_WIDTH)
    .into()
}

fn plan_menu<'a>() -> Element<'a, Message> {
  let items = vec![
    plan_menu_item(
      t!("skills.header.new_template").into_owned(),
      Message::OpenPlanEditor(super::EditorSeed::NewTemplate),
    ),
    plan_menu_item(t!("skills.header.manage_plans").into_owned(), Message::OpenManagePlans),
  ];

  container(Column::with_children(items).width(Length::Fill))
    .width(Length::Fill)
    .padding(spacing::UNIT)
    .style(|_| container::Style {
      background: Some(iced::Background::Color(color::surface::RAISED)),
      border: iced::Border {
        color: color::rule_strong(),
        radius: radius::NAV_CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn plan_menu_item<'a>(label: String, on_press: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fill)
  .padding(iced::Padding {
    top: spacing::SPACE_2 + 2.0,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2 + 2.0,
    left: spacing::SPACE_3,
  })
  .on_press(on_press)
  .style(|_, status| {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| iced::Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      ..button::Style::default()
    }
  })
  .into()
}

fn compare_button<'a>() -> Element<'a, Message> {
  Button::secondary(t!("skills.header.compare"))
    .icon(Icon::compare())
    .size(Size::Sm)
    .height(HEADER_BUTTON_HEIGHT)
    .on_press(Message::OpenCompare)
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
  let total_sp: Element<'_, Message> = text(t!("skills.header.sp_value", sp => fmt_sp(pilot.total_sp)))
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
  let noun = if len == 1 {
    t!("skills.header.skill")
  } else {
    t!("skills.header.skills")
  };
  let value = match remaining {
    Some(seconds) if seconds > 0 => fmt_duration(seconds),
    _ => t!("skills.header.empty").into_owned(),
  };
  let value_color = if is_low_queue(len, remaining) {
    color::status::DANGER
  } else {
    color::text::PRIMARY
  };
  let label = t!("skills.header.queue_label", count => len, noun => noun);
  stat_block(&label, value, value_color, None)
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
    fn it_renders_the_plan_dropdown_closed_and_open() {
      let closed = State::new(42);
      let _el: Element<'_, Message> = plan_dropdown(&closed);

      let mut open = State::new(42);
      open.plan_menu_open = true;
      let _el: Element<'_, Message> = plan_dropdown(&open);
    }

    #[test]
    fn it_moves_the_eta_stat_into_the_left_cluster() {
      let mut state = State::new(42);
      state.roster = vec![pilot(42, "Test Pilot")];

      let _el: Element<'_, Message> = header(&state, now());
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
