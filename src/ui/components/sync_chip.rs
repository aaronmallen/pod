use iced::{
  Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, container, text},
};

use crate::{
  sync::FreshnessSummary,
  ui::{
    components::status::{dot, format_since},
    style::{color, spacing, typography},
  },
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Lifecycle {
  #[default]
  Active,
  ReadOnly {
    hostname: Option<String>,
  },
  Stopped,
}

#[derive(Clone, Debug)]
pub struct State {
  pub errors: usize,
  pub last_synced_secs: Option<u64>,
  pub lifecycle: Lifecycle,
  pub pulse_on: bool,
  pub summary: FreshnessSummary,
}

impl State {
  fn attention(&self) -> usize {
    self.summary.attention + self.errors
  }

  fn refreshing(&self) -> bool {
    self.summary.refreshing > 0
  }
}

pub fn sync_chip<'a, M>(state: State) -> Element<'a, M>
where
  M: 'a,
{
  let (dot_color, label) = match &state.lifecycle {
    Lifecycle::ReadOnly {
      hostname,
    } => (color::status::WARNING, read_only_label(hostname.as_deref())),
    Lifecycle::Stopped => (color::status::DANGER, stopped_label()),
    Lifecycle::Active => (active_dot_color(&state), active_label(&state)),
  };

  container(
    Row::with_children(vec![dot(dot_color), label])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_3_5,
    bottom: 0.0,
    left: spacing::SPACE_3_5,
  })
  .width(Length::Fill)
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .into()
}

fn active_dot_color(state: &State) -> Color {
  if state.attention() > 0 {
    if state.errors > 0 {
      color::status::DANGER
    } else {
      color::status::WARNING
    }
  } else if state.refreshing() && state.pulse_on {
    color::accent::PLASMA
  } else if state.refreshing() {
    color::accent::PLASMA_MUTED
  } else {
    color::status::ONLINE
  }
}

fn active_label<'a, M>(state: &State) -> Element<'a, M>
where
  M: 'a,
{
  let attention = state.attention();
  if attention > 0 {
    attention_label(attention, state.errors)
  } else if state.summary.catching_up > 0 {
    catching_up_label(state.summary.catching_up)
  } else {
    up_to_date_label(state)
  }
}

fn attention_label<'a, M>(attention: usize, errors: usize) -> Element<'a, M>
where
  M: 'a,
{
  let fill = if errors > 0 {
    color::status::DANGER
  } else {
    color::status::WARNING
  };
  mono_text(t!("common.sync_chip.attention", count => attention).into_owned(), fill)
}

fn catching_up_label<'a, M>(left: usize) -> Element<'a, M>
where
  M: 'a,
{
  mono_text(
    t!("common.sync_chip.catching_up", count => left).into_owned(),
    color::accent::PLASMA,
  )
}

fn mono_text<'a, M>(content: impl text::IntoFragment<'a>, fill: Color) -> Element<'a, M>
where
  M: 'a,
{
  text(content)
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(move |_| text::Style {
      color: Some(fill),
    })
    .into()
}

fn read_only_label<'a, M>(hostname: Option<&str>) -> Element<'a, M>
where
  M: 'a,
{
  let content = match hostname {
    Some(hostname) => t!("common.sync_chip.read_only_host", hostname => hostname).into_owned(),
    None => t!("common.sync_chip.read_only").into_owned(),
  };
  mono_text(content, color::status::WARNING)
}

fn stopped_label<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  mono_text(t!("common.sync_chip.stopped"), color::status::DANGER)
}

fn up_to_date_label<'a, M>(state: &State) -> Element<'a, M>
where
  M: 'a,
{
  let headline = mono_text(t!("common.sync_chip.up_to_date"), color::status::ONLINE);
  // The quiet last-sync time is a steady aside: it only appears once everything has truly settled,
  // so a routine mid-refresh never decorates the calm headline with a churning timestamp.
  match state.last_synced_secs {
    Some(secs) if state.summary.is_up_to_date() => Row::with_children(vec![
      headline,
      mono_text("\u{00b7}", color::text::tertiary()),
      mono_text(format_since(secs), color::text::dim()),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into(),
    _ => headline,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn state(summary: FreshnessSummary, errors: usize) -> State {
    State {
      errors,
      last_synced_secs: None,
      lifecycle: Lifecycle::Active,
      pulse_on: false,
      summary,
    }
  }

  fn fresh(total: usize) -> FreshnessSummary {
    FreshnessSummary {
      fresh: total,
      total,
      ..FreshnessSummary::default()
    }
  }

  mod headline {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_attention_above_catching_up_and_up_to_date() {
      let summary = FreshnessSummary {
        attention: 2,
        catching_up: 3,
        fresh: 5,
        total: 10,
        ..FreshnessSummary::default()
      };
      let state = state(summary, 0);

      assert_eq!(state.attention(), 2);
      assert_eq!(active_dot_color(&state), color::status::WARNING);
      let _label: Element<'_, ()> = active_label(&state);
    }

    #[test]
    fn it_treats_persistent_errors_as_attention_with_danger() {
      let summary = FreshnessSummary {
        attention: 1,
        fresh: 4,
        total: 5,
        ..FreshnessSummary::default()
      };
      let state = state(summary, 2);

      assert_eq!(state.attention(), 3, "errors fold into the need-attention count");
      assert_eq!(active_dot_color(&state), color::status::DANGER);
      let _label: Element<'_, ()> = active_label(&state);
    }

    #[test]
    fn it_reads_catching_up_only_when_no_attention() {
      let summary = FreshnessSummary {
        catching_up: 4,
        fresh: 1,
        total: 5,
        ..FreshnessSummary::default()
      };
      let state = state(summary, 0);

      assert_eq!(state.attention(), 0);
      assert_eq!(state.summary.catching_up, 4);
      let _label: Element<'_, ()> = active_label(&state);
    }

    #[test]
    fn it_reads_up_to_date_when_everything_is_fresh() {
      let state = state(fresh(7), 0);

      assert_eq!(state.attention(), 0);
      assert_eq!(state.summary.catching_up, 0);
      assert!(state.summary.is_up_to_date());
      let _label: Element<'_, ()> = active_label(&state);
    }

    #[test]
    fn it_stays_up_to_date_while_a_job_is_mid_refresh() {
      let summary = FreshnessSummary {
        fresh: 6,
        refreshing: 1,
        total: 7,
        ..FreshnessSummary::default()
      };
      let state = state(summary, 0);

      assert_eq!(
        state.attention(),
        0,
        "a routine refresh never raises the attention headline"
      );
      assert_eq!(state.summary.catching_up, 0, "and never reads as catching up");
      assert!(
        !state.summary.is_up_to_date(),
        "the strict settled check still sees the in-flight refresh"
      );

      // The headline words are still the calm Up-to-date state; only the dot pulses.
      let _label: Element<'_, ()> = active_label(&state);
    }
  }

  mod dot {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pulses_plasma_only_while_refreshing() {
      let summary = FreshnessSummary {
        fresh: 6,
        refreshing: 1,
        total: 7,
        ..FreshnessSummary::default()
      };

      let mut refreshing = state(summary, 0);
      refreshing.pulse_on = true;
      assert_eq!(active_dot_color(&refreshing), color::accent::PLASMA);

      refreshing.pulse_on = false;
      assert_eq!(active_dot_color(&refreshing), color::accent::PLASMA_MUTED);
    }

    #[test]
    fn it_rests_online_when_nothing_is_in_flight() {
      let mut calm = state(fresh(4), 0);
      calm.pulse_on = true;

      assert_eq!(
        active_dot_color(&calm),
        color::status::ONLINE,
        "a settled system never pulses, regardless of the tick"
      );
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_stopped_and_read_only_lifecycle_states() {
      let stopped = State {
        lifecycle: Lifecycle::Stopped,
        ..state(fresh(10), 0)
      };
      let _stopped: Element<'_, ()> = sync_chip(stopped);

      let read_only = State {
        lifecycle: Lifecycle::ReadOnly {
          hostname: Some("nebula".to_owned()),
        },
        ..state(fresh(10), 0)
      };
      let _read_only: Element<'_, ()> = sync_chip(read_only);
    }

    #[test]
    fn it_renders_every_active_headline_state() {
      let refreshing = State {
        last_synced_secs: None,
        pulse_on: true,
        ..state(
          FreshnessSummary {
            fresh: 5,
            refreshing: 5,
            total: 10,
            ..FreshnessSummary::default()
          },
          0,
        )
      };
      let _refreshing: Element<'_, ()> = sync_chip(refreshing);

      let idle = State {
        last_synced_secs: Some(125),
        ..state(fresh(10), 0)
      };
      let _idle: Element<'_, ()> = sync_chip(idle);

      let errored = State {
        ..state(
          FreshnessSummary {
            fresh: 8,
            total: 10,
            ..FreshnessSummary::default()
          },
          2,
        )
      };
      let _errored: Element<'_, ()> = sync_chip(errored);

      let attention = State {
        ..state(
          FreshnessSummary {
            attention: 2,
            fresh: 8,
            total: 10,
            ..FreshnessSummary::default()
          },
          0,
        )
      };
      let _attention: Element<'_, ()> = sync_chip(attention);

      let catching_up = State {
        ..state(
          FreshnessSummary {
            catching_up: 3,
            fresh: 7,
            total: 10,
            ..FreshnessSummary::default()
          },
          0,
        )
      };
      let _catching_up: Element<'_, ()> = sync_chip(catching_up);
    }
  }
}
