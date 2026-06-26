use iced::{Element, widget::text};

use super::{HeadStats, Message, PickerPilot, State};
use crate::ui::{
  components::{
    header::{header as header_band, header_divider, stat_block},
    picker::{
      PickerGroup, TriggerPortrait, picker_character_row, picker_dropdown as picker_dropdown_panel, picker_trigger,
      trigger_identity,
    },
  },
  format::{fmt_isk_opt, fmt_sp_opt},
  style::{color, typography},
};

const SEC_STATUS_HIGH: f64 = 5.0;

pub(super) fn header(state: &State) -> Element<'_, Message> {
  let head = &state.head;

  let left: Vec<Element<'_, Message>> = vec![
    character_picker(state),
    header_divider(),
    stat_block(
      "Total SP",
      format!("{} SP", fmt_sp_opt(head.total_sp)),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      "Liquid",
      format!("{} ISK", fmt_isk_opt(head.liquid_isk)),
      color::text::PRIMARY,
      None,
    ),
    header_divider(),
    stat_block(
      "Sec Status",
      fmt_sec_status(head.sec_status),
      sec_status_color(head.sec_status),
      None,
    ),
    header_divider(),
    location_stat(head),
  ];

  header_band(left, Vec::new())
}

fn character_picker(state: &State) -> Element<'_, Message> {
  let active = state.roster.iter().find(|pilot| pilot.id == state.active());
  picker_trigger(
    trigger(active, state.active()),
    state.picker_open,
    Message::PickerToggled,
  )
}

pub(super) fn picker_dropdown(state: &State) -> Element<'_, Message> {
  let rows: Vec<Element<'_, Message>> = state
    .roster
    .iter()
    .map(|pilot| picker_row(pilot, pilot.id == state.active()))
    .collect();

  let groups = vec![PickerGroup {
    title: Some("Switch character".to_owned()),
    items: rows,
  }];

  picker_dropdown_panel(groups)
}

fn trigger(active: Option<&PickerPilot>, active_id: i64) -> Element<'_, Message> {
  let name = active.map(|pilot| pilot.name.as_str()).unwrap_or("");
  let corp = active.map(|pilot| pilot.corp.as_str()).unwrap_or("");
  let portrait = TriggerPortrait {
    id: active_id,
    name: name.to_owned(),
    path: active.and_then(|pilot| pilot.portrait.path()),
  };

  trigger_identity(name.to_owned(), corp.to_owned(), Some(portrait))
}

fn picker_row(pilot: &PickerPilot, selected: bool) -> Element<'_, Message> {
  let trailing: Element<'_, Message> = text(format!("{} SP", fmt_sp_opt(Some(pilot.total_sp))))
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into();

  picker_character_row(
    pilot.id,
    pilot.name.clone(),
    pilot.corp.to_uppercase(),
    pilot.portrait.path(),
    Some(trailing),
    selected,
    None,
    Message::CharacterChanged(pilot.id),
  )
}

fn location_stat(head: &HeadStats) -> Element<'_, Message> {
  let value = head.location.clone().unwrap_or_else(|| "\u{2014}".to_owned());
  let sub = head
    .location
    .as_ref()
    .map(|_| if head.docked { "docked" } else { "in space" });
  stat_block("Location", value, color::text::PRIMARY, sub)
}

fn fmt_sec_status(sec: Option<f64>) -> String {
  match sec {
    Some(value) => format!("{value:.1}"),
    None => "\u{2014}".to_owned(),
  }
}

fn sec_status_color(sec: Option<f64>) -> iced::Color {
  match sec {
    Some(value) if value >= SEC_STATUS_HIGH => color::status::ONLINE,
    Some(value) if value < 0.0 => color::status::DANGER,
    _ => color::text::PRIMARY,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod fmt_sec_status {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_one_decimal() {
      assert_eq!(fmt_sec_status(Some(4.83)), "4.8");
    }

    #[test]
    fn it_renders_an_em_dash_for_none() {
      assert_eq!(fmt_sec_status(None), "\u{2014}");
    }
  }

  mod header {
    use super::*;
    use crate::{config::Feature, features::roster::character_detail::HeadStats, store::images};

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

    #[test]
    fn it_renders_the_dropdown_with_an_open_picker() {
      let mut state = State::new(42, &Feature::ALL);
      state.roster = vec![pilot(42, "Test Pilot"), pilot(7, "Wingmate")];
      state.picker_open = true;

      let _el: Element<'_, Message> = picker_dropdown(&state);
    }

    #[test]
    fn it_renders_with_a_closed_picker() {
      let mut state = State::new(42, &Feature::ALL);
      state.roster = vec![pilot(42, "Test Pilot"), pilot(7, "Wingmate")];
      state.head = HeadStats {
        docked: true,
        liquid_isk: Some(1_000_000_000.0),
        location: Some("Jita".to_owned()),
        sec_status: Some(5.1),
        total_sp: Some(47_320_400),
      };

      let _el: Element<'_, Message> = header(&state);
    }
  }

  mod sec_status_color {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_highsec_as_positive() {
      assert_eq!(sec_status_color(Some(5.0)), color::status::ONLINE);
    }

    #[test]
    fn it_reads_negative_as_danger() {
      assert_eq!(sec_status_color(Some(-1.2)), color::status::DANGER);
    }

    #[test]
    fn it_reads_the_middle_band_as_ink() {
      assert_eq!(sec_status_color(Some(2.5)), color::text::PRIMARY);
      assert_eq!(sec_status_color(None), color::text::PRIMARY);
    }
  }
}
