use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use crate::{
  config::Feature,
  features::character_manager::OwnedPilot,
  sync::{self, JobKey, JobKind, Phase, Subject},
  ui::{
    components::status::{dot, format_since},
    style::{color, radius, shadow, spacing, typography},
  },
};

const CARD_WIDTH: f32 = 460.0;
const CHAR_BAR_HEIGHT: f32 = 16.0;
const CHAR_BAR_WIDTH: f32 = 4.0;
const CLOSE_GLYPH: &str = "✕";
const COUNTDOWN_WIDTH: f32 = 72.0;
const GLYPH_WIDTH: f32 = 36.0;
const INSET_X: f32 = 16.0;
const LIST_MAX_HEIGHT: f32 = 360.0;
/// The per-pilot sync jobs the popover surfaces, paired with their display labels. A featureless job
/// (e.g. Profile) always runs and is always shown; a feature-gated job is only shown when its feature
/// is enabled.
const POPOVER_JOBS: [(JobKind, &str); 7] = [
  (JobKind::AssetSync, "Assets"),
  (JobKind::CharacterClones, "Clones"),
  (JobKind::CharacterContacts, "Contacts"),
  (JobKind::CharacterProfile, "Profile"),
  (JobKind::CharacterSkills, "Skills"),
  (JobKind::CharacterTelemetry, "Telemetry"),
  (JobKind::CharacterWallet, "Wallet"),
];
const PROGRESS_WIDTH: f32 = 96.0;
const PULSE_OFF: f32 = 0.4;
const QUEUED_OPACITY: f32 = 0.5;
const ROW_HEIGHT: f32 = 50.0;
const SCROLLBAR_WIDTH: f32 = 6.0;
const SYNCING_FILL: f32 = 0.45;
const TRACK_HEIGHT: f32 = 3.0;

#[derive(Clone, Debug)]
pub enum Header {
  Idle { last_synced_secs: Option<u64> },
  Syncing { active: usize, percent: u8, queued: usize },
}

#[derive(Clone, Debug)]
pub struct JobRow {
  pub character_color: Color,
  pub character_name: String,
  pub error: Option<String>,
  pub label: String,
  pub next_in_secs: Option<u64>,
  pub state: RowState,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JobStats {
  pub active: usize,
  pub attention: usize,
  pub done: usize,
  pub errors: usize,
  pub total: usize,
}

impl JobStats {
  pub fn in_progress(&self) -> bool {
    let settled = self.done + self.errors + self.attention;
    self.active > 0 || (self.errors == 0 && self.total > 0 && settled < self.total)
  }
}

#[derive(Clone, Debug)]
pub struct Model {
  pub done: usize,
  pub errors: usize,
  pub header: Header,
  pub pulse_on: bool,
  pub rows: Vec<JobRow>,
  pub total: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowState {
  Attention,
  Done,
  Empty,
  Error,
  Queued,
  Syncing,
}

pub fn build_model(
  pilots: &[OwnedPilot],
  status: &sync::SyncStatus,
  enabled: &[Feature],
  last_synced_secs: Option<u64>,
  pulse_on: bool,
) -> Model {
  let mut rows = Vec::with_capacity(pilots.len() * POPOVER_JOBS.len());
  for_each_job(pilots, enabled, |pilot, label, key| {
    let (state, error) = row_state(status, &key);
    rows.push(JobRow {
      character_color: pilot.color,
      character_name: pilot.name.clone(),
      error,
      label: label.to_owned(),
      next_in_secs: status.next_in_secs(&key),
      state,
    });
  });

  let total = rows.len();
  let done = rows.iter().filter(|row| row.state == RowState::Done).count();
  let errors = rows.iter().filter(|row| row.state == RowState::Error).count();
  let active = rows.iter().filter(|row| row.state == RowState::Syncing).count();
  let queued = rows.iter().filter(|row| row.state == RowState::Queued).count();

  let header = if active > 0 {
    let percent = (done * 100).checked_div(total).unwrap_or(100) as u8;
    Header::Syncing {
      active,
      percent,
      queued,
    }
  } else {
    Header::Idle {
      last_synced_secs,
    }
  };

  Model {
    done,
    errors,
    header,
    pulse_on,
    rows,
    total,
  }
}

pub fn job_stats(pilots: &[OwnedPilot], status: &sync::SyncStatus, enabled: &[Feature]) -> JobStats {
  let mut stats = JobStats::default();
  for_each_job(pilots, enabled, |_pilot, _label, key| {
    let (state, _) = row_state(status, &key);
    stats.total += 1;
    match state {
      RowState::Attention => stats.attention += 1,
      RowState::Done | RowState::Empty => stats.done += 1,
      RowState::Error => stats.errors += 1,
      RowState::Syncing => stats.active += 1,
      RowState::Queued => {}
    }
  });
  stats
}

pub fn row_state(status: &sync::SyncStatus, key: &JobKey) -> (RowState, Option<String>) {
  match status.phase(key) {
    None => (RowState::Queued, None),
    Some(Phase::Done) => (RowState::Done, None),
    Some(Phase::Syncing) => (RowState::Syncing, None),
    Some(Phase::Failed) => (RowState::Error, status.reason(key).map(str::to_owned)),
    Some(Phase::BackingOff) => {
      let detail = status
        .retry_secs(key)
        .map(|secs| format!("Backing off {secs}s"))
        .or_else(|| status.reason(key).map(str::to_owned));
      (RowState::Error, detail)
    }
    Some(Phase::Blocked) => (
      RowState::Attention,
      status
        .reason(key)
        .map(str::to_owned)
        .or_else(|| Some("Blocked".to_owned())),
    ),
    Some(Phase::Empty) => (RowState::Empty, Some("No data".to_owned())),
    Some(Phase::NotReady) => (RowState::Attention, Some("Waiting on dependencies".to_owned())),
  }
}

pub fn sync_popover<'a, M>(model: &Model, on_close: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let body = Column::with_children(vec![
    header(&model.header, model.pulse_on, on_close),
    list(model),
    footer(model),
  ]);

  container(body)
    .width(Length::Fixed(CARD_WIDTH))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

fn close_button<'a, M>(on_close: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  button(
    text(CLOSE_GLYPH)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(0)
  .on_press(on_close)
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    ..button::Style::default()
  })
  .into()
}

fn empty_state<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  container(eyebrow("No characters linked", color::text::tertiary()))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_6,
      right: INSET_X,
      bottom: spacing::SPACE_6,
      left: INSET_X,
    })
    .align_x(iced::alignment::Horizontal::Center)
    .into()
}

fn eyebrow<'a, M>(content: impl Into<String>, fill: Color) -> Element<'a, M>
where
  M: 'a,
{
  text(content.into().to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(move |_| text::Style {
      color: Some(fill),
    })
    .into()
}

fn footer<'a, M>(model: &Model) -> Element<'a, M>
where
  M: 'a,
{
  let mut left = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
  left = left.push(eyebrow(
    format!("{} / {} endpoints", model.done, model.total),
    color::text::tertiary(),
  ));
  if model.errors > 0 {
    left = left.push(eyebrow(
      format!("· {} retry pending", model.errors),
      color::status::DANGER,
    ));
  }

  let body = Row::with_children(vec![
    left.into(),
    Space::new().width(Length::Fill).into(),
    eyebrow("Auto every 8s", color::text::tertiary()),
  ])
  .align_y(Vertical::Center);

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_3_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn for_each_job(pilots: &[OwnedPilot], enabled: &[Feature], mut visit: impl FnMut(&OwnedPilot, &str, JobKey)) {
  for pilot in pilots {
    let subject = Subject::Character(pilot.id);
    for (kind, label) in POPOVER_JOBS {
      // A feature-gated job whose feature is disabled is not syncing, so it must not appear queued;
      // featureless jobs (e.g. Profile) always run and are always shown.
      if kind.feature().is_some_and(|feature| !enabled.contains(&feature)) {
        continue;
      }
      visit(pilot, label, JobKey::new(kind, subject));
    }
  }
}

fn format_next_in(secs: u64) -> String {
  if secs < 60 {
    format!("{secs}s")
  } else if secs < 3_600 {
    format!("{}m", secs / 60)
  } else {
    format!("{}h", secs / 3_600)
  }
}

fn header<'a, M>(header: &Header, pulse_on: bool, on_close: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let (dot_color, summary) = match header {
    Header::Idle {
      last_synced_secs,
    } => {
      let summary = match last_synced_secs {
        Some(secs) => format!("· last sync {}", format_since(*secs)),
        None => "· idle".to_string(),
      };
      (color::status::ONLINE, summary)
    }
    Header::Syncing {
      active,
      percent,
      queued,
    } => {
      let fill = if pulse_on {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::accent::PLASMA, PULSE_OFF)
      };
      (fill, format!("· {active} active · {queued} queued · {percent}%"))
    }
  };

  let body = Row::with_children(vec![
    dot(dot_color),
    eyebrow("Sync queue", color::text::PRIMARY),
    eyebrow(summary, color::text::tertiary()),
    Space::new().width(Length::Fill).into(),
    close_button(on_close),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: INSET_X,
      bottom: spacing::SPACE_3,
      left: INSET_X,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn job_row<'a, M>(row: &JobRow, pulse_on: bool) -> Element<'a, M>
where
  M: 'a,
{
  let queued = row.state == RowState::Queued;

  let body = Row::with_children(vec![
    row_marker(row, pulse_on),
    row_labels(row),
    row_countdown(row),
    row_progress(row, pulse_on),
    row_glyph(row),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: INSET_X,
      bottom: spacing::SPACE_2_5,
      left: INSET_X,
    })
    .style(move |_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if queued { 0.05 } else { 0.1 }),
        width: 1.0,
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn list<'a, M>(model: &Model) -> Element<'a, M>
where
  M: 'a,
{
  if model.rows.is_empty() {
    return empty_state();
  }

  let rows: Vec<Element<'a, M>> = model.rows.iter().map(|row| job_row(row, model.pulse_on)).collect();

  let content = Column::with_children(rows).width(Length::Fill);

  // The scrollable's clip sub-layer leaves accumulating "ghost" pixels on Wayland/wgpu, so it is
  // used only when the rows actually overflow; a plain column avoids the artifact when they fit.
  if !needs_scroll(model.rows.len()) {
    return content.into();
  }

  let list = scrollable(content)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Shrink)
    .direction(scrollable::Direction::Vertical(
      scrollable::Scrollbar::new()
        .width(SCROLLBAR_WIDTH)
        .scroller_width(SCROLLBAR_WIDTH),
    ));

  container(list).max_height(LIST_MAX_HEIGHT).into()
}

fn needs_scroll(rows: usize) -> bool {
  (rows as f32) * ROW_HEIGHT > LIST_MAX_HEIGHT
}

fn row_countdown<'a, M>(row: &JobRow) -> Element<'a, M>
where
  M: 'a,
{
  let (label, tone) = match row.state {
    RowState::Syncing => ("Syncing".to_owned(), color::text::secondary()),
    RowState::Queued => ("Queued".to_owned(), color::text::tertiary()),
    _ => match row.next_in_secs {
      Some(secs) => (format!("Next in {}", format_next_in(secs)), color::text::tertiary()),
      None => (String::new(), color::text::tertiary()),
    },
  };

  container(
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(tone),
      }),
  )
  .width(Length::Fixed(COUNTDOWN_WIDTH))
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn row_glyph<'a, M>(row: &JobRow) -> Element<'a, M>
where
  M: 'a,
{
  let (glyph, glyph_color) = match row.state {
    RowState::Attention => ("∅", color::status::WARNING),
    RowState::Done => ("✓", color::text::secondary()),
    RowState::Empty => ("∅", color::text::secondary()),
    RowState::Error => ("!", color::status::DANGER),
    RowState::Queued => ("··", color::text::tertiary()),
    RowState::Syncing => ("~", color::text::secondary()),
  };

  container(
    text(glyph)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(glyph_color),
      }),
  )
  .width(Length::Fixed(GLYPH_WIDTH))
  .align_x(iced::alignment::Horizontal::Right)
  .into()
}

fn row_labels<'a, M>(row: &JobRow) -> Element<'a, M>
where
  M: 'a,
{
  let primary_color = if row.state == RowState::Queued {
    color::text::secondary()
  } else {
    color::text::PRIMARY
  };

  let label = text(row.label.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(primary_color),
    });

  let (sub_text, sub_color) = match (row.state, &row.error) {
    (RowState::Error, Some(message)) => (message.to_uppercase(), color::status::DANGER),
    (RowState::Error, None) => (row.character_name.to_uppercase(), color::status::DANGER),
    (RowState::Attention, Some(message)) => (message.to_uppercase(), color::status::WARNING),
    (RowState::Attention, None) => (row.character_name.to_uppercase(), color::status::WARNING),
    (RowState::Empty, Some(message)) => (message.to_uppercase(), color::text::tertiary()),
    _ => (row.character_name.to_uppercase(), color::text::tertiary()),
  };

  let sub = text(sub_text)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(sub_color),
    });

  container(Column::with_children(vec![label.into(), sub.into()]).spacing(2.0))
    .width(Length::Fill)
    .into()
}

fn row_marker<'a, M>(row: &JobRow, pulse_on: bool) -> Element<'a, M>
where
  M: 'a,
{
  let dot_color = match row.state {
    RowState::Attention => color::status::WARNING,
    RowState::Done => color::status::ONLINE,
    RowState::Empty => color::status::ONLINE,
    RowState::Error => color::status::DANGER,
    RowState::Queued => color::text::tertiary(),
    RowState::Syncing => {
      if pulse_on {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::accent::PLASMA, PULSE_OFF)
      }
    }
  };

  let tone = if row.state == RowState::Queued {
    color::with_alpha(row.character_color, QUEUED_OPACITY)
  } else {
    row.character_color
  };

  Row::with_children(vec![dot(dot_color), tone_bar(tone)])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
}

fn row_progress<'a, M>(row: &JobRow, pulse_on: bool) -> Element<'a, M>
where
  M: 'a,
{
  let (fill_portion, fill_color) = match row.state {
    RowState::Attention => (1.0, color::status::WARNING),
    RowState::Done => (1.0, color::status::ONLINE),
    RowState::Empty => (1.0, color::status::ONLINE),
    RowState::Error => (1.0, color::status::DANGER),
    RowState::Queued => (0.0, color::accent::PLASMA),
    RowState::Syncing => {
      let plasma = if pulse_on {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::accent::PLASMA, PULSE_OFF)
      };
      (SYNCING_FILL, plasma)
    }
  };

  let track_inner: Element<'a, M> = if fill_portion <= 0.0 {
    Space::new()
      .width(Length::Fill)
      .height(Length::Fixed(TRACK_HEIGHT))
      .into()
  } else if fill_portion >= 1.0 {
    track_fill(Length::Fill, fill_color)
  } else {
    let fill = (fill_portion * 100.0) as u16;
    Row::with_children(vec![
      track_fill(Length::FillPortion(fill), fill_color),
      Space::new()
        .width(Length::FillPortion(100 - fill))
        .height(Length::Fixed(TRACK_HEIGHT))
        .into(),
    ])
    .height(Length::Fixed(TRACK_HEIGHT))
    .into()
  };

  container(track_inner)
    .width(Length::Fixed(PROGRESS_WIDTH))
    .height(Length::Fixed(TRACK_HEIGHT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.08))),
      border: Border {
        radius: 1.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn tone_bar<'a, M>(fill: Color) -> Element<'a, M>
where
  M: 'a,
{
  container(
    Space::new()
      .width(Length::Fixed(CHAR_BAR_WIDTH))
      .height(Length::Fixed(CHAR_BAR_HEIGHT)),
  )
  .width(Length::Fixed(CHAR_BAR_WIDTH))
  .height(Length::Fixed(CHAR_BAR_HEIGHT))
  .style(move |_| container::Style {
    background: Some(Background::Color(fill)),
    border: Border {
      radius: 1.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn track_fill<'a, M>(width: Length, fill: Color) -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new().width(Length::Fill).height(Length::Fixed(TRACK_HEIGHT)))
    .width(width)
    .height(Length::Fixed(TRACK_HEIGHT))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn pilot(id: i64) -> OwnedPilot {
    OwnedPilot {
      color: color::accent::PLASMA,
      granted_scopes: None,
      id,
      name: format!("Pilot {id}"),
    }
  }

  mod build_model {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_jobs_whose_feature_is_disabled() {
      let pilots = vec![pilot(1)];
      let status = sync::SyncStatus::new();

      let with_all = build_model(&pilots, &status, &Feature::ALL, None, false);
      let with_none = build_model(&pilots, &status, &[], None, false);

      assert!(with_none.rows.len() < with_all.rows.len());
    }

    #[test]
    fn it_emits_one_row_per_pilot_and_enabled_job() {
      let pilots = vec![pilot(1), pilot(2)];
      let status = sync::SyncStatus::new();

      let model = build_model(&pilots, &status, &Feature::ALL, Some(5), false);

      assert_eq!(model.total, model.rows.len());
      assert_eq!(model.rows.len(), pilots.len() * POPOVER_JOBS.len());
    }
  }

  mod job_stats {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

    #[test]
    fn it_counts_an_active_job_against_the_total() {
      let pilots = vec![pilot(1)];
      let mut status = sync::SyncStatus::new();
      status.apply(&Event::Started {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
      });

      let stats = job_stats(&pilots, &status, &Feature::ALL);

      assert_eq!(stats.active, 1);
      assert_eq!(stats.total, POPOVER_JOBS.len());
    }
  }

  mod needs_scroll {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_does_not_scroll_when_rows_fit_within_the_max_height() {
      assert_eq!(needs_scroll(0), false);
      assert_eq!(needs_scroll(7), false);
    }

    #[test]
    fn it_scrolls_when_rows_overflow_the_max_height() {
      assert_eq!(needs_scroll(8), true);
      assert_eq!(needs_scroll(20), true);
    }
  }

  mod render {
    use super::*;

    fn row(label: &str, state: RowState, error: Option<&str>) -> JobRow {
      JobRow {
        character_color: color::accent::PLASMA,
        character_name: "Cinder Vex".to_string(),
        error: error.map(str::to_string),
        label: label.to_string(),
        next_in_secs: Some(2_520),
        state,
      }
    }

    #[test]
    fn it_renders_all_row_states_and_the_empty_state() {
      let populated = Model {
        done: 1,
        errors: 1,
        header: Header::Syncing {
          active: 1,
          percent: 60,
          queued: 1,
        },
        pulse_on: true,
        rows: vec![
          row("Profile", RowState::Done, None),
          row("Telemetry", RowState::Syncing, None),
          row("Wallet", RowState::Error, Some("Backing off 30s")),
          row("Abyssals", RowState::Attention, Some("No data")),
          row("Profile", RowState::Queued, None),
        ],
        total: 5,
      };
      let _populated: Element<'_, ()> = sync_popover(&populated, ());

      let idle = Model {
        done: 4,
        errors: 0,
        header: Header::Idle {
          last_synced_secs: Some(125),
        },
        pulse_on: false,
        rows: vec![row("Profile", RowState::Done, None)],
        total: 4,
      };
      let _idle: Element<'_, ()> = sync_popover(&idle, ());

      let empty = Model {
        done: 0,
        errors: 0,
        header: Header::Idle {
          last_synced_secs: None,
        },
        pulse_on: false,
        rows: vec![],
        total: 0,
      };
      let _empty: Element<'_, ()> = sync_popover(&empty, ());

      let overflowing = Model {
        done: 0,
        errors: 0,
        header: Header::Syncing {
          active: 8,
          percent: 0,
          queued: 0,
        },
        pulse_on: true,
        rows: (0..8).map(|_| row("Profile", RowState::Syncing, None)).collect(),
        total: 8,
      };
      let _overflowing: Element<'_, ()> = sync_popover(&overflowing, ());
    }
  }

  mod row_state {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

    fn key() -> JobKey {
      JobKey::new(JobKind::CharacterProfile, Subject::Character(1))
    }

    #[test]
    fn it_maps_done_and_syncing_phases() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Started {
        key: key(),
      });
      assert_eq!(row_state(&status, &key()), (RowState::Syncing, None));

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::synced(),
      });
      assert_eq!(row_state(&status, &key()), (RowState::Done, None));
    }

    #[test]
    fn it_reads_an_unreported_job_as_queued() {
      let status = sync::SyncStatus::new();

      assert_eq!(row_state(&status, &key()), (RowState::Queued, None));
    }

    #[test]
    fn it_renders_a_backoff_countdown_as_error_text() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::BackingOff {
        key: key(),
        retry_secs: 30,
      });

      assert_eq!(
        row_state(&status, &key()),
        (RowState::Error, Some("Backing off 30s".to_owned()))
      );
    }

    #[test]
    fn it_surfaces_a_failure_reason_as_error_text() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Failed {
        key: key(),
        reason: "token expired".to_owned(),
      });

      assert_eq!(
        row_state(&status, &key()),
        (RowState::Error, Some("token expired".to_owned()))
      );
    }

    #[test]
    fn it_surfaces_an_empty_outcome_as_benign_and_a_blocked_outcome_as_attention() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::Empty,
      });
      assert_eq!(
        row_state(&status, &key()),
        (RowState::Empty, Some("No data".to_owned())),
        "a successful empty sync is benign, not an amber attention chip"
      );

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::Blocked {
          reason: "missing scope".to_owned(),
        },
      });
      assert_eq!(
        row_state(&status, &key()),
        (RowState::Attention, Some("missing scope".to_owned()))
      );
    }
  }
}
