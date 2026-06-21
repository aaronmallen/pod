use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, responsive, scrollable, text},
};

use super::{
  Extraction, Message, State,
  jobs::{fmt_clock, fmt_day, fmt_duration, progress_bar, sec_pill},
  loaders::ExtractionState,
};
use crate::ui::{
  components::icon::Icon,
  style::{color, radius, spacing, typography},
};

const CARD_GAP: f32 = 18.0;
const CARD_MIN_WIDTH: f32 = 420.0;
const CONTENT_PADDING: f32 = 24.0;
const SECONDS_PER_DAY: i64 = 86_400;
const TILE_BOX: f32 = 40.0;

pub(super) fn tab<'a>(state: &'a State, now: DateTime<Utc>) -> Element<'a, Message> {
  let mut extractions = state.visible_extractions();
  // Sort soonest-arriving first; extractions without an arrival time fall to the end.
  extractions.sort_by(|a, b| match (a.arrival(), b.arrival()) {
    (Some(a), Some(b)) => a.cmp(&b),
    (Some(_), None) => std::cmp::Ordering::Less,
    (None, Some(_)) => std::cmp::Ordering::Greater,
    (None, None) => a.moon_id.cmp(&b.moon_id),
  });

  let body = Column::with_children(vec![header(extractions.len()), grid(extractions, now)])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill);

  scrollable(container(body).width(Length::Fill).padding(CONTENT_PADDING))
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn card<'a>(extraction: &'a Extraction, now: DateTime<Utc>) -> Element<'a, Message> {
  let state = extraction.state(now);
  let accent = state_color(state);

  let header = Row::with_children(vec![
    moon_tile(accent),
    identity(extraction),
    state_badge(state, accent),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let card = Column::with_children(vec![
    container(header)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_3_5,
        bottom: spacing::SPACE_3_5,
        left: spacing::SPACE_3_5,
        right: spacing::SPACE_3_5,
      })
      .style(|_| container::Style {
        border: Border {
          color: color::rule(),
          radius: 0.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into(),
    timeline(extraction, now, state, accent),
  ])
  .width(Length::Fill);

  let border = match state {
    ExtractionState::Imminent | ExtractionState::Ready => color::with_alpha(accent, 0.4),
    _ => color::rule(),
  };

  container(card)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: border,
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn grid<'a>(extractions: Vec<&'a Extraction>, now: DateTime<Utc>) -> Element<'a, Message> {
  if extractions.is_empty() {
    return container(
      text("No extraction timers for this scope.")
        .font(typography::body::REGULAR)
        .size(typography::size::LG)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .into();
  }

  responsive(move |size| {
    let per_row = per_row(size.width);
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for chunk in extractions.chunks(per_row) {
      let mut cells: Vec<Element<'a, Message>> = chunk.iter().map(|extraction| card(extraction, now)).collect();
      // Pad the final row so cards keep their column width instead of stretching to fill.
      while cells.len() < per_row {
        cells.push(Space::new().width(Length::Fill).into());
      }
      rows.push(Row::with_children(cells).spacing(CARD_GAP).width(Length::Fill).into());
    }

    Column::with_children(rows).spacing(CARD_GAP).width(Length::Fill).into()
  })
  .into()
}

fn header<'a>(count: usize) -> Element<'a, Message> {
  Row::with_children(vec![
    text("Moon extraction timers")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(format!("{count} active \u{00B7} corp scope"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}

fn identity<'a>(extraction: &'a Extraction) -> Element<'a, Message> {
  let mut meta: Vec<Element<'a, Message>> = vec![
    text(extraction.structure.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if let Some(system) = &extraction.system_name {
    meta.push(dot());
    meta.push(
      text(system.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }
  meta.push(sec_pill(extraction.security));

  Column::with_children(vec![
    text(extraction.moon_label())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Row::with_children(meta)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill)
  .into()
}

fn moon_tile<'a>(accent: Color) -> Element<'a, Message> {
  container(Icon::moon().color(accent).size(TILE_BOX * 0.5).render::<Message>())
    .width(Length::Fixed(TILE_BOX))
    .height(Length::Fixed(TILE_BOX))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(accent, 0.14))),
      border: Border {
        color: color::with_alpha(accent, 0.3),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn state_badge<'a>(state: ExtractionState, accent: Color) -> Element<'a, Message> {
  container(
    text(state.label().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(accent)),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(accent, 0.12))),
    border: Border {
      color: color::with_alpha(accent, 0.26),
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn state_color(state: ExtractionState) -> Color {
  match state {
    ExtractionState::Extracting => color::accent::PLASMA,
    ExtractionState::Fractured => color::status::DANGER,
    ExtractionState::Imminent => color::status::WARNING,
    ExtractionState::Ready => color::status::ONLINE,
  }
}

fn timeline<'a>(
  extraction: &'a Extraction,
  now: DateTime<Utc>,
  state: ExtractionState,
  accent: Color,
) -> Element<'a, Message> {
  let arrived = matches!(state, ExtractionState::Fractured | ExtractionState::Ready);
  let arrival_value = arrival_text(extraction, now, arrived);
  let arrival_color = if arrived { accent } else { color::text::PRIMARY };

  let fractured = matches!(state, ExtractionState::Fractured);
  let (decay_value, decay_color) = decay_split(extraction, now, fractured);

  let heads = Row::with_children(vec![
    countdown(
      if arrived { "Chunk arrived" } else { "Chunk arrives" },
      &arrival_value,
      arrival_color,
      Horizontal::Left,
    ),
    Space::new().width(Length::Fill).into(),
    countdown("Natural fracture", &decay_value, decay_color, Horizontal::Right),
  ])
  .width(Length::Fill);

  let started = started_text(extraction);

  Column::with_children(vec![
    heads.into(),
    progress_bar(extraction.progress(now), accent, 8.0, !arrived),
    text(started)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  })
  .width(Length::Fill)
  .into()
}

fn arrival_text(extraction: &Extraction, now: DateTime<Utc>, arrived: bool) -> String {
  match extraction.arrival() {
    Some(arrival) if arrived => format!("{} {}", fmt_day(arrival), fmt_clock(arrival)),
    Some(arrival) => fmt_duration((arrival - now).num_seconds().max(0)),
    None => "\u{2014}".to_owned(),
  }
}

fn decay_split(extraction: &Extraction, now: DateTime<Utc>, fractured: bool) -> (String, Color) {
  match extraction.decay() {
    _ if fractured => ("passed".to_owned(), color::status::DANGER),
    Some(decay) => {
      let remaining = (decay - now).num_seconds().max(0);
      let fill = if remaining < SECONDS_PER_DAY {
        color::status::WARNING
      } else {
        color::text::secondary()
      };
      (fmt_duration(remaining), fill)
    }
    None => ("\u{2014}".to_owned(), color::text::tertiary()),
  }
}

fn started_text(extraction: &Extraction) -> String {
  match extraction.start() {
    Some(start) => format!("started {}", fmt_day(start)),
    None => "started \u{2014}".to_owned(),
  }
}

fn countdown<'a>(label: &str, value: &str, value_color: Color, align: Horizontal) -> Element<'a, Message> {
  Column::with_children(vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text(value.to_owned())
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(value_color))
      .into(),
  ])
  .spacing(spacing::UNIT)
  .align_x(align)
  .into()
}

fn dot<'a>() -> Element<'a, Message> {
  text("\u{00B7}")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()))
    .into()
}

fn per_row(width: f32) -> usize {
  if width < CARD_MIN_WIDTH {
    return 1;
  }
  (((width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor() as usize).max(1)
}

#[cfg(test)]
mod tests {
  use super::{
    super::{EMPTY_INDUSTRY_SELECTION, FacilityDefaults, Tab},
    *,
  };

  fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-16T12:00:00Z")
      .unwrap()
      .with_timezone(&Utc)
  }

  fn required() -> Vec<&'static str> {
    Vec::new()
  }

  fn extraction(moon_id: i64, start: &str, arrival: &str, decay: &str) -> Extraction {
    Extraction {
      chunk_arrival_time: Some(arrival.to_owned()),
      corporation_id: 98,
      extraction_start_time: Some(start.to_owned()),
      moon_id,
      moon_name: Some(format!("Moon {moon_id}")),
      natural_decay_time: Some(decay.to_owned()),
      security: Some(0.4),
      structure: "Athanor Alpha".to_owned(),
      system_name: Some("Tama".to_owned()),
    }
  }

  fn state_with(extractions: Vec<Extraction>) -> State {
    let mut state = State::new(
      EMPTY_INDUSTRY_SELECTION,
      required(),
      crate::config::FeatureFlags::default(),
      FacilityDefaults::default(),
      None,
      false,
    );
    state.seed_extractions(extractions);
    state.seed_tab(Tab::Extractions);
    state
  }

  mod arrival_text {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_a_dash_when_no_arrival_is_known() {
      let mut extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-20T00:00:00Z",
        "2026-06-22T00:00:00Z",
      );
      extraction.chunk_arrival_time = None;

      assert_eq!(super::super::arrival_text(&extraction, now(), false), "\u{2014}");
    }

    #[test]
    fn it_renders_a_pending_chunk_as_a_countdown() {
      let extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-20T00:00:00Z",
        "2026-06-22T00:00:00Z",
      );

      assert_ne!(super::super::arrival_text(&extraction, now(), false), "\u{2014}");
    }

    #[test]
    fn it_renders_an_arrived_chunk_as_a_day_and_clock() {
      let extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-15T06:00:00Z",
        "2026-06-18T00:00:00Z",
      );

      assert!(super::super::arrival_text(&extraction, now(), true).contains(':'));
    }
  }

  mod decay_split {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_marks_a_fractured_timer_as_passed() {
      let extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-12T00:00:00Z",
        "2026-06-13T00:00:00Z",
      );

      let (value, color) = super::super::decay_split(&extraction, now(), true);

      assert_eq!(value, "passed");
      assert_eq!(color, color::status::DANGER);
    }

    #[test]
    fn it_renders_a_dash_when_no_decay_is_known() {
      let mut extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-15T00:00:00Z",
        "2026-06-20T00:00:00Z",
      );
      extraction.natural_decay_time = None;

      let (value, _) = super::super::decay_split(&extraction, now(), false);

      assert_eq!(value, "\u{2014}");
    }

    #[test]
    fn it_stays_neutral_when_decay_is_more_than_a_day_out() {
      let extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-15T00:00:00Z",
        "2026-06-20T00:00:00Z",
      );

      let (_, color) = super::super::decay_split(&extraction, now(), false);

      assert_eq!(color, color::text::secondary());
    }

    #[test]
    fn it_warns_when_decay_is_under_a_day_out() {
      let extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-15T00:00:00Z",
        "2026-06-16T18:00:00Z",
      );

      let (_, color) = super::super::decay_split(&extraction, now(), false);

      assert_eq!(color, color::status::WARNING);
    }
  }

  mod per_row {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_fits_multiple_cards_across_a_wide_viewport() {
      assert_eq!(super::per_row(900.0), 2);
    }

    #[test]
    fn it_fits_one_card_below_the_minimum_width() {
      assert_eq!(super::per_row(300.0), 1);
    }
  }

  mod started_text {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_renders_a_dash_when_no_start_is_known() {
      let mut extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-15T00:00:00Z",
        "2026-06-20T00:00:00Z",
      );
      extraction.extraction_start_time = None;

      assert_eq!(super::super::started_text(&extraction), "started \u{2014}");
    }

    #[test]
    fn it_renders_the_start_day() {
      let extraction = extraction(
        1,
        "2026-06-10T00:00:00Z",
        "2026-06-15T00:00:00Z",
        "2026-06-20T00:00:00Z",
      );

      assert!(super::super::started_text(&extraction).starts_with("started "));
    }
  }

  mod tab {
    use super::*;

    #[test]
    fn it_renders_a_card_in_each_derived_state() {
      // extracting (far arrival), imminent (<24h arrival), ready (arrived, not decayed), fractured (past decay).
      let extractions = vec![
        extraction(
          1,
          "2026-06-15T00:00:00Z",
          "2026-06-20T00:00:00Z",
          "2026-06-22T00:00:00Z",
        ),
        extraction(
          2,
          "2026-06-14T00:00:00Z",
          "2026-06-16T18:00:00Z",
          "2026-06-18T00:00:00Z",
        ),
        extraction(
          3,
          "2026-06-10T00:00:00Z",
          "2026-06-16T06:00:00Z",
          "2026-06-18T00:00:00Z",
        ),
        extraction(
          4,
          "2026-06-08T00:00:00Z",
          "2026-06-12T00:00:00Z",
          "2026-06-14T00:00:00Z",
        ),
      ];
      let state = state_with(extractions);

      let _el: Element<'_, Message> = tab(&state, now());
    }

    #[test]
    fn it_renders_an_empty_scope() {
      let state = state_with(Vec::new());

      let _el: Element<'_, Message> = tab(&state, now());
    }

    #[test]
    fn it_renders_cards_with_missing_timestamps() {
      let state = state_with(vec![Extraction {
        chunk_arrival_time: None,
        corporation_id: 98,
        extraction_start_time: None,
        moon_id: 9,
        moon_name: None,
        natural_decay_time: None,
        security: None,
        structure: "Unknown".to_owned(),
        system_name: None,
      }]);

      let _el: Element<'_, Message> = tab(&state, now());
    }
  }
}
