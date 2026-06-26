use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use crate::{
  config::FeatureFlags,
  features::roster::OwnedPilot,
  sync::{self, Freshness, FreshnessSummary, JobKey, JobKind, Phase, Subject, freshness_of},
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

/// The disambiguated freshness a row renders, mirroring the shared `Freshness` vocabulary
/// (`crate::sync`) one-for-one, with the attention bucket split into its user-distinct conditions so
/// the row can carry a clear label. No bare "Queued" remains: a job the engine has not yet reported
/// on is `CatchingUp`, a transient backoff is `Refreshing` (calm), and only persistent failure /
/// blocked / re-auth land in an attention state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowState {
  Blocked,
  CatchingUp,
  Failed,
  Fresh,
  Reauth,
  Refreshing,
}

impl RowState {
  /// The shared freshness state this row maps onto, so a row's rendering always agrees with the
  /// chip's aggregate from the same `Freshness` vocabulary.
  fn freshness(self) -> Freshness {
    match self {
      RowState::Blocked | RowState::Failed | RowState::Reauth => Freshness::Attention,
      RowState::CatchingUp => Freshness::CatchingUp,
      RowState::Fresh => Freshness::Fresh,
      RowState::Refreshing => Freshness::Refreshing,
    }
  }
}

pub fn build_model(
  pilots: &[OwnedPilot],
  status: &sync::SyncStatus,
  features: &FeatureFlags,
  last_synced_secs: Option<u64>,
  pulse_on: bool,
) -> Model {
  let mut rows = Vec::with_capacity(pilots.len() * POPOVER_JOBS.len());
  let mut summary = FreshnessSummary::default();
  for_each_job(pilots, features, |pilot, label, key| {
    let (state, error) = row_state(status, &key);
    // Count from the very row the user sees, so the header/footer aggregate can never drift from the
    // disambiguated rows below it (and, sharing the `Freshness` vocabulary, from the chip).
    summary.record(state.freshness());
    rows.push(JobRow {
      character_color: pilot.color,
      character_name: pilot.name.clone(),
      error,
      label: label.to_owned(),
      next_in_secs: status.next_in_secs(&key),
      state,
    });
  });

  let total = summary.total;
  // Fresh — Synced OR Empty within interval — is the single up-to-date count both surfaces share, so
  // an empty-result endpoint is never undercounted the way a Done-only count undercounted it.
  let done = summary.fresh;
  // Persistent failures are the "retry pending" footer count; blocked / re-auth are attention but not
  // a retry, and a transient backoff is Refreshing, so neither inflates the error tally.
  let errors = rows.iter().filter(|row| row.state == RowState::Failed).count();
  let active = summary.refreshing;
  let queued = summary.catching_up;

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

pub fn job_stats(pilots: &[OwnedPilot], status: &sync::SyncStatus, features: &FeatureFlags) -> JobStats {
  let mut stats = JobStats::default();
  let summary = FreshnessSummary::from_keys(status, &collect_keys(pilots, features));
  for_each_job(pilots, features, |_pilot, _label, key| {
    if matches!(freshness_of(status, &key), Freshness::Attention) && is_error_phase(status, &key) {
      stats.errors += 1;
    }
  });
  stats.total = summary.total;
  stats.done = summary.fresh;
  stats.active = summary.refreshing;
  // Persistent failures surface as errors in the chip; blocked/needs-reauth/not-ready as attention.
  stats.attention = summary.attention.saturating_sub(stats.errors);
  stats
}

fn collect_keys(pilots: &[OwnedPilot], features: &FeatureFlags) -> Vec<JobKey> {
  let mut keys = Vec::with_capacity(pilots.len() * POPOVER_JOBS.len());
  for_each_job(pilots, features, |_pilot, _label, key| keys.push(key));
  keys
}

fn is_error_phase(status: &sync::SyncStatus, key: &JobKey) -> bool {
  matches!(status.phase(key), Some(Phase::Failed))
}

/// Sentinel reason the engine seeds for a credential flagged `needs_reauth`, so the row can surface
/// re-auth as its own user-actionable attention state rather than a generic "blocked".
const REAUTH_REASON: &str = "needs re-authentication";

/// Derive a row's disambiguated freshness state and its detail line from the shared phase, never
/// collapsing distinct conditions onto a bare "Queued":
///
/// - `Done`/`Empty` -> `Fresh` (the next-in countdown is rendered separately from `next_in_secs`).
/// - `Syncing`/`BackingOff` -> `Refreshing`; a transient backoff self-heals and stays calm, so it is
///   never an attention state — though its retry detail is kept for the sub-label.
/// - `None` (enrolled but never reported) -> `CatchingUp`.
/// - `Failed` -> `Failed`; `Blocked`/`NotReady` -> `Blocked`, except a `needs re-authentication`
///   reason, which becomes the distinct `Reauth` state.
pub fn row_state(status: &sync::SyncStatus, key: &JobKey) -> (RowState, Option<String>) {
  match status.phase(key) {
    None => (RowState::CatchingUp, None),
    Some(Phase::Done) => (RowState::Fresh, None),
    Some(Phase::Empty) => (RowState::Fresh, Some("No data".to_owned())),
    Some(Phase::Syncing) => (RowState::Refreshing, None),
    Some(Phase::BackingOff) => {
      let detail = status
        .retry_secs(key)
        .map(|secs| format!("Retrying in {secs}s"))
        .or_else(|| status.reason(key).map(str::to_owned));
      (RowState::Refreshing, detail)
    }
    Some(Phase::Failed) => (RowState::Failed, status.reason(key).map(str::to_owned)),
    Some(Phase::Blocked) => {
      let reason = status.reason(key);
      if reason == Some(REAUTH_REASON) {
        (RowState::Reauth, Some("Needs re-authentication".to_owned()))
      } else {
        (
          RowState::Blocked,
          reason.map(str::to_owned).or_else(|| Some("Blocked".to_owned())),
        )
      }
    }
    Some(Phase::NotReady) => (RowState::Blocked, Some("Waiting on dependencies".to_owned())),
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

fn for_each_job(pilots: &[OwnedPilot], features: &FeatureFlags, mut visit: impl FnMut(&OwnedPilot, &str, JobKey)) {
  for pilot in pilots {
    let subject = Subject::Character(pilot.id);
    for (kind, label) in POPOVER_JOBS {
      // The engine enrolls on sub-features (is_feature_enabled), so the row domain gates the same way:
      // no phantom "Queued" row exists for a job the engine will never service.
      if !kind.is_feature_enabled(features) {
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
  // A catching-up row is the calmest, faintest state (never-reported-yet); its border recedes the way
  // the old "Queued" row did.
  let catching_up = row.state == RowState::CatchingUp;

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
        color: color::with_alpha(color::text::PRIMARY, if catching_up { 0.05 } else { 0.1 }),
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
    RowState::Refreshing => ("Refreshing".to_owned(), color::text::secondary()),
    RowState::CatchingUp => ("Catching up".to_owned(), color::text::tertiary()),
    // Attention rows carry their detail in the sub-label, so the countdown column stays clear.
    RowState::Blocked | RowState::Failed | RowState::Reauth => (String::new(), color::text::tertiary()),
    // Fresh — the only state with a meaningful next-run deadline.
    RowState::Fresh => match row.next_in_secs {
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
    RowState::Blocked => ("∅", color::status::WARNING),
    RowState::CatchingUp => ("··", color::text::tertiary()),
    RowState::Failed => ("!", color::status::DANGER),
    RowState::Fresh => ("✓", color::text::secondary()),
    RowState::Reauth => ("⚿", color::status::WARNING),
    RowState::Refreshing => ("~", color::text::secondary()),
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
  let primary_color = if row.state == RowState::CatchingUp {
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

  // The sub-label carries the attention detail (failure reason / blocked / re-auth) in its state tone,
  // and otherwise names the character; a fresh "No data" empty result reads as a benign tertiary note.
  let (sub_text, sub_color) = match (row.state, &row.error) {
    (RowState::Failed, Some(message)) => (message.to_uppercase(), color::status::DANGER),
    (RowState::Failed, None) => (row.character_name.to_uppercase(), color::status::DANGER),
    (RowState::Blocked | RowState::Reauth, Some(message)) => (message.to_uppercase(), color::status::WARNING),
    (RowState::Blocked | RowState::Reauth, None) => (row.character_name.to_uppercase(), color::status::WARNING),
    (RowState::Fresh, Some(message)) => (message.to_uppercase(), color::text::tertiary()),
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
    RowState::Blocked | RowState::Reauth => color::status::WARNING,
    RowState::CatchingUp => color::text::tertiary(),
    RowState::Failed => color::status::DANGER,
    RowState::Fresh => color::status::ONLINE,
    RowState::Refreshing => {
      if pulse_on {
        color::accent::PLASMA
      } else {
        color::with_alpha(color::accent::PLASMA, PULSE_OFF)
      }
    }
  };

  let tone = if row.state == RowState::CatchingUp {
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
    RowState::Blocked | RowState::Reauth => (1.0, color::status::WARNING),
    RowState::CatchingUp => (0.0, color::accent::PLASMA),
    RowState::Failed => (1.0, color::status::DANGER),
    RowState::Fresh => (1.0, color::status::ONLINE),
    RowState::Refreshing => {
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

  fn all_off() -> FeatureFlags {
    let mut flags = FeatureFlags::default();
    for feature in crate::config::Feature::ALL {
      flags.set_enabled(feature, false);
    }
    flags
  }

  mod build_model {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

    fn seed_every_job(status: &mut sync::SyncStatus, id: i64, outcome: crate::sync::Outcome) {
      for (kind, _) in POPOVER_JOBS {
        status.apply(&Event::Seeded {
          key: JobKey::new(kind, Subject::Character(id)),
          outcome: outcome.clone(),
          next_in_secs: Some(3_600),
        });
      }
    }

    #[test]
    fn it_reads_an_all_fresh_roster_as_a_calm_up_to_date_idle_header() {
      let pilots = vec![pilot(1)];
      let mut status = sync::SyncStatus::new();
      seed_every_job(&mut status, 1, crate::sync::Outcome::synced());

      let model = build_model(&pilots, &status, &FeatureFlags::default(), Some(12), false);

      assert!(
        matches!(model.header, Header::Idle { .. }),
        "every job fresh means no active headline — the surface stays calm"
      );
      assert_eq!(model.done, model.total, "fresh count reaches the full total");
      assert_eq!(model.errors, 0);
      assert!(model.rows.iter().all(|row| row.state == RowState::Fresh));
    }

    #[test]
    fn it_counts_an_empty_outcome_as_fresh_in_the_done_total() {
      let pilots = vec![pilot(1)];
      let mut status = sync::SyncStatus::new();
      seed_every_job(&mut status, 1, crate::sync::Outcome::Empty);

      let model = build_model(&pilots, &status, &FeatureFlags::default(), None, false);

      assert_eq!(
        model.done, model.total,
        "an empty result is a fresh success and is never undercounted"
      );
    }

    #[test]
    fn it_never_emits_a_bare_queued_row_for_an_unreported_roster() {
      let pilots = vec![pilot(1)];
      let status = sync::SyncStatus::new();

      let model = build_model(&pilots, &status, &FeatureFlags::default(), None, false);

      assert!(
        model.rows.iter().all(|row| row.state == RowState::CatchingUp),
        "a cold launch shows catching-up rows, not the old ambiguous Queued"
      );
      assert_eq!(model.done, 0);
    }

    #[test]
    fn it_surfaces_a_persistent_failure_in_the_error_footer_count() {
      let pilots = vec![pilot(1)];
      let mut status = sync::SyncStatus::new();
      status.apply(&Event::Failed {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
        reason: "token expired".to_owned(),
      });

      let model = build_model(&pilots, &status, &FeatureFlags::default(), None, false);

      assert_eq!(model.errors, 1, "a persistent failure is one retry-pending error");
    }

    #[test]
    fn it_keeps_a_transient_backoff_out_of_the_error_count() {
      let pilots = vec![pilot(1)];
      let mut status = sync::SyncStatus::new();
      status.apply(&Event::BackingOff {
        key: JobKey::new(JobKind::CharacterProfile, Subject::Character(1)),
        retry_secs: 30,
      });

      let model = build_model(&pilots, &status, &FeatureFlags::default(), None, false);

      assert_eq!(model.errors, 0, "a self-healing backoff is refreshing, not an error");
      assert!(matches!(model.header, Header::Syncing { .. }));
    }

    #[test]
    fn it_drops_jobs_whose_feature_is_disabled() {
      let pilots = vec![pilot(1)];
      let status = sync::SyncStatus::new();

      let with_all = build_model(&pilots, &status, &FeatureFlags::default(), None, false);
      let with_none = build_model(&pilots, &status, &all_off(), None, false);

      assert!(with_none.rows.len() < with_all.rows.len());
    }

    #[test]
    fn it_emits_one_row_per_pilot_and_enabled_job() {
      let pilots = vec![pilot(1), pilot(2)];
      let status = sync::SyncStatus::new();

      let model = build_model(&pilots, &status, &FeatureFlags::default(), Some(5), false);

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

      let stats = job_stats(&pilots, &status, &FeatureFlags::default());

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
          row("Profile", RowState::Fresh, None),
          row("Telemetry", RowState::Refreshing, Some("Retrying in 30s")),
          row("Wallet", RowState::Failed, Some("token expired")),
          row("Clones", RowState::Blocked, Some("missing scope")),
          row("Skills", RowState::Reauth, Some("Needs re-authentication")),
          row("Contacts", RowState::CatchingUp, None),
        ],
        total: 6,
      };
      let _populated: Element<'_, ()> = sync_popover(&populated, ());

      let idle = Model {
        done: 4,
        errors: 0,
        header: Header::Idle {
          last_synced_secs: Some(125),
        },
        pulse_on: false,
        rows: vec![row("Profile", RowState::Fresh, None)],
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
        rows: (0..8).map(|_| row("Profile", RowState::Refreshing, None)).collect(),
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
    fn it_maps_a_synced_job_to_fresh() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::synced(),
      });

      let (state, _) = row_state(&status, &key());
      assert_eq!(state, RowState::Fresh);
      assert_eq!(state.freshness(), Freshness::Fresh);
    }

    #[test]
    fn it_maps_an_empty_outcome_to_a_benign_fresh_row() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::Empty,
      });

      assert_eq!(
        row_state(&status, &key()),
        (RowState::Fresh, Some("No data".to_owned())),
        "a successful empty sync is fresh data, never an attention chip"
      );
    }

    #[test]
    fn it_maps_a_running_job_to_refreshing() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Started {
        key: key(),
      });

      let (state, detail) = row_state(&status, &key());
      assert_eq!((state, detail), (RowState::Refreshing, None));
      assert_eq!(state.freshness(), Freshness::Refreshing);
    }

    #[test]
    fn it_maps_a_transient_backoff_to_refreshing_not_attention() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::BackingOff {
        key: key(),
        retry_secs: 30,
      });

      let (state, detail) = row_state(&status, &key());
      assert_eq!(
        (state, detail),
        (RowState::Refreshing, Some("Retrying in 30s".to_owned())),
        "a self-healing backoff reads as refreshing, never an attention state"
      );
      assert_eq!(state.freshness(), Freshness::Refreshing);
    }

    #[test]
    fn it_maps_an_unreported_enrolled_job_to_catching_up() {
      let status = sync::SyncStatus::new();

      let (state, detail) = row_state(&status, &key());
      assert_eq!(
        (state, detail),
        (RowState::CatchingUp, None),
        "a never-reported job catches up; it is never a bare Queued"
      );
      assert_eq!(state.freshness(), Freshness::CatchingUp);
    }

    #[test]
    fn it_maps_a_persistent_failure_to_an_attention_failed_row() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Failed {
        key: key(),
        reason: "token expired".to_owned(),
      });

      let (state, detail) = row_state(&status, &key());
      assert_eq!((state, detail), (RowState::Failed, Some("token expired".to_owned())));
      assert_eq!(state.freshness(), Freshness::Attention);
    }

    #[test]
    fn it_maps_a_blocked_outcome_to_an_attention_blocked_row() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Finished {
        key: key(),
        outcome: crate::sync::Outcome::Blocked {
          reason: "missing scope".to_owned(),
        },
      });

      let (state, detail) = row_state(&status, &key());
      assert_eq!((state, detail), (RowState::Blocked, Some("missing scope".to_owned())));
      assert_eq!(state.freshness(), Freshness::Attention);
    }

    #[test]
    fn it_distinguishes_a_needs_reauth_block_as_its_own_attention_state() {
      let mut status = sync::SyncStatus::new();

      status.apply(&Event::Seeded {
        key: key(),
        outcome: crate::sync::Outcome::Blocked {
          reason: REAUTH_REASON.to_owned(),
        },
        next_in_secs: None,
      });

      let (state, detail) = row_state(&status, &key());
      assert_eq!(
        (state, detail),
        (RowState::Reauth, Some("Needs re-authentication".to_owned())),
        "re-auth is user-actionable and surfaced distinctly from a generic block"
      );
      assert_eq!(state.freshness(), Freshness::Attention);
    }
  }
}
