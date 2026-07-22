use super::*;

pub(super) fn sync_model(app: &App) -> Model {
  let pilots = roster(app);
  let last_synced_secs = app.last_synced.map(|at| (app.now - at).num_seconds().max(0) as u64);
  sync_popover::build_model(
    &pilots,
    &app.status,
    &feature_flags(app),
    last_synced_secs,
    app.sync_tick,
  )
}

pub(super) fn expected_job_stats(app: &App) -> JobStats {
  sync_popover::job_stats(&roster(app), &app.status, &feature_flags(app))
}

pub(super) fn chip_freshness(stats: &JobStats) -> FreshnessSummary {
  let catching_up = stats
    .total
    .saturating_sub(stats.done + stats.active + stats.attention + stats.errors);
  FreshnessSummary {
    attention: stats.attention,
    catching_up,
    fresh: stats.done,
    refreshing: stats.active,
    total: stats.total,
  }
}

pub(super) fn engine_syncing(app: &App) -> bool {
  syncing_with(&app.engine_state, &expected_job_stats(app))
}

pub(super) fn syncing_with(engine_state: &EngineState, stats: &JobStats) -> bool {
  !engine_state.settled() && stats.in_progress()
}

pub(super) fn chip_lifecycle(app: &App) -> sync_chip::Lifecycle {
  match &app.engine_state {
    EngineState::Stopped {
      ..
    } => sync_chip::Lifecycle::Stopped,
    EngineState::ReadOnly {
      held_by,
    } => sync_chip::Lifecycle::ReadOnly {
      hostname: held_by.as_ref().map(|holder| holder.hostname.clone()),
    },
    EngineState::Idle | EngineState::Running => sync_chip::Lifecycle::Active,
  }
}

pub(super) fn status_affordance(state: &EngineState) -> Option<Element<'static, Message>> {
  let (label, message) = match state {
    EngineState::Stopped {
      ..
    } => (t!("shell.status.restart_sync").into_owned(), Message::RestartSync),
    EngineState::ReadOnly {
      ..
    } => (t!("shell.takeover.take_over").into_owned(), Message::TakeOver),
    EngineState::Idle | EngineState::Running => return None,
  };
  let action = Button::primary(label).size(ButtonSize::Sm).on_press(message);
  Some(
    container(action)
      .padding(region_padding())
      .height(Length::Fill)
      .align_y(Vertical::Center)
      .into(),
  )
}

pub(super) fn status_bar_view(app: &App) -> Element<'_, Message> {
  let stats = expected_job_stats(app);
  let chip = sync_chip::State {
    errors: stats.errors,
    last_synced_secs: app.last_synced.map(|at| (app.now - at).num_seconds().max(0) as u64),
    lifecycle: chip_lifecycle(app),
    pulse_on: app.sync_tick,
    summary: chip_freshness(&stats),
  };

  let eve = container(eve_time(app.now))
    .padding(region_padding())
    .height(Length::Fill)
    .align_y(Vertical::Center);

  let open = app.sync_popover_open;
  let chip = container(sync_chip::sync_chip(chip))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: open.then(|| Background::Color(color::with_alpha(color::accent(), CHIP_OPEN_TINT_ALPHA))),
      ..container::Style::default()
    });
  let chip = mouse_area(chip).on_press(Message::ToggleSyncPopover);

  let mut children = vec![eve.into(), separator(), chip.into()];
  if let Some(affordance) = status_affordance(&app.engine_state) {
    children.push(affordance);
    children.push(separator());
  }
  if let Some(outbox) = outbox_indicator(&app.outbox) {
    children.push(separator());
    children.push(outbox);
  }
  children.push(separator());
  children.push(esi_status(app.esi_connected));

  let row = Row::with_children(children)
    .height(Length::Fill)
    .align_y(Vertical::Center);

  let bar = container(row)
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::STATUS_BAR_HEIGHT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::NAVIGATION)),
      ..container::Style::default()
    });

  let top_border = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      ..container::Style::default()
    });

  Column::with_children(vec![top_border.into(), bar.into()])
    .width(Length::Fill)
    .into()
}

pub(super) fn outbox_indicator(outbox: &sync::OutboxStatus) -> Option<Element<'_, Message>> {
  let pending = outbox.pending();
  let failed = outbox.failed();
  if pending == 0 && failed == 0 {
    return None;
  }

  let dot_color = if failed > 0 {
    color::status::DANGER
  } else {
    color::accent()
  };

  let mut parts: Vec<Element<'_, Message>> = vec![
    dot(dot_color),
    text(t!("shell.status.mutations").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::dim()),
      })
      .into(),
  ];
  if pending > 0 {
    parts.push(
      text(format!("\u{21bb} {pending}"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::accent()),
        })
        .into(),
    );
  }
  if failed > 0 {
    parts.push(
      text(format!("\u{2715} {failed}"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
    );
  }

  Some(
    container(
      Row::with_children(parts)
        .spacing(spacing::SPACE_2)
        .align_y(Vertical::Center),
    )
    .padding(region_padding())
    .height(Length::Fill)
    .align_y(Vertical::Center)
    .into(),
  )
}

pub(super) fn dot<'a>(fill: iced::Color) -> Element<'a, Message> {
  const DOT_SIZE: f32 = 6.0;
  container(
    Space::new()
      .width(Length::Fixed(DOT_SIZE))
      .height(Length::Fixed(DOT_SIZE)),
  )
  .width(Length::Fixed(DOT_SIZE))
  .height(Length::Fixed(DOT_SIZE))
  .style(move |_| container::Style {
    background: Some(Background::Color(fill)),
    border: iced::Border {
      radius: (DOT_SIZE / 2.0).into(),
      ..iced::Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  mod outbox_indicator {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::sync::Event;

    #[test]
    fn it_folds_a_sync_event_into_the_apps_outbox_aggregate() {
      let mut app = test_app();

      let _ = update(
        &mut app,
        Message::Sync(Event::OutboxInflight {
          id: 1,
        }),
      );
      let _ = update(
        &mut app,
        Message::Sync(Event::OutboxFailed {
          id: 2,
          reason: "boom".to_owned(),
        }),
      );

      assert_eq!(app.outbox.pending(), 1);
      assert_eq!(app.outbox.failed(), 1);
      assert_eq!(
        app.status.phase(&sync::JobKey::new(
          sync::JobKind::CharacterProfile,
          sync::Subject::Character(1)
        )),
        None,
        "outbox events do not enter the job-keyed status"
      );
    }

    #[test]
    fn it_is_absent_when_the_outbox_is_quiet() {
      let outbox = sync::OutboxStatus::new();

      assert!(
        super::outbox_indicator(&outbox).is_none(),
        "an idle outbox adds no chrome"
      );
    }

    #[test]
    fn it_renders_when_a_row_has_failed() {
      let mut outbox = sync::OutboxStatus::new();
      outbox.apply(&Event::OutboxFailed {
        id: 1,
        reason: "403 Forbidden".to_owned(),
      });

      assert!(super::outbox_indicator(&outbox).is_some());
    }

    #[test]
    fn it_renders_when_a_row_is_pending() {
      let mut outbox = sync::OutboxStatus::new();
      outbox.apply(&Event::OutboxInflight {
        id: 1,
      });

      assert!(super::outbox_indicator(&outbox).is_some());
    }
  }
}
