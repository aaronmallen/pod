use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Row, Space, container, text},
};

use crate::ui::{
  components::status::{dot, format_since},
  style::{color, spacing, typography},
};

const BAR_HEIGHT: f32 = 2.0;
const BAR_WIDTH: f32 = 200.0;

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
  pub attention: usize,
  pub done: usize,
  pub errors: usize,
  pub last_synced_secs: Option<u64>,
  pub lifecycle: Lifecycle,
  pub percent: u8,
  pub pulse_on: bool,
  pub syncing: bool,
  pub total: usize,
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
  if state.syncing {
    if state.pulse_on {
      color::accent::PLASMA
    } else {
      color::accent::PLASMA_MUTED
    }
  } else if state.errors > 0 {
    color::status::DANGER
  } else if state.attention > 0 {
    color::status::WARNING
  } else {
    color::status::ONLINE
  }
}

fn active_label<'a, M>(state: &State) -> Element<'a, M>
where
  M: 'a,
{
  if state.syncing {
    syncing_label(state.done, state.total, state.percent)
  } else if state.errors > 0 {
    error_label(state.errors)
  } else if state.attention > 0 {
    attention_label(state.attention)
  } else {
    idle_label(state.last_synced_secs)
  }
}

fn attention_label<'a, M>(attention: usize) -> Element<'a, M>
where
  M: 'a,
{
  mono_text(format!("{attention} pending"), color::status::WARNING)
}

fn error_label<'a, M>(errors: usize) -> Element<'a, M>
where
  M: 'a,
{
  let noun = if errors == 1 { "error" } else { "errors" };
  mono_text(format!("{errors} sync {noun}"), color::status::DANGER)
}

fn fill_segment<'a, M>(width: Length) -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new().width(Length::Fill).height(Length::Fixed(BAR_HEIGHT)))
    .width(width)
    .height(Length::Fixed(BAR_HEIGHT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent::PLASMA)),
      ..container::Style::default()
    })
    .into()
}

fn idle_label<'a, M>(last_synced_secs: Option<u64>) -> Element<'a, M>
where
  M: 'a,
{
  match last_synced_secs {
    Some(secs) => Row::with_children(vec![
      mono_text("Synced", color::text::secondary()),
      mono_text("·", color::text::tertiary()),
      mono_text(format_since(secs), color::text::dim()),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into(),
    None => mono_text("Idle", color::text::dim()),
  }
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

fn progress_bar<'a, M>(percent: u8) -> Element<'a, M>
where
  M: 'a,
{
  let fill = u16::from(percent).clamp(1, 100);
  let rest = 100 - fill;
  let inner: Element<'a, M> = if rest == 0 {
    fill_segment(Length::Fill)
  } else {
    Row::with_children(vec![
      fill_segment(Length::FillPortion(fill)),
      Space::new()
        .width(Length::FillPortion(rest))
        .height(Length::Fixed(BAR_HEIGHT))
        .into(),
    ])
    .height(Length::Fixed(BAR_HEIGHT))
    .into()
  };

  container(inner)
    .width(Length::Fixed(BAR_WIDTH))
    .height(Length::Fixed(BAR_HEIGHT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      border: Border {
        radius: 1.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn read_only_label<'a, M>(hostname: Option<&str>) -> Element<'a, M>
where
  M: 'a,
{
  let content = match hostname {
    Some(hostname) => format!("Read-only \u{2014} open on {hostname}"),
    None => "Read-only".to_owned(),
  };
  mono_text(content, color::status::WARNING)
}

fn stopped_label<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  mono_text("Sync stopped", color::status::DANGER)
}

fn syncing_label<'a, M>(done: usize, total: usize, percent: u8) -> Element<'a, M>
where
  M: 'a,
{
  Row::with_children(vec![
    mono_text("Syncing", color::text::PRIMARY),
    progress_bar(percent),
    mono_text(format!("{done}/{total}"), color::text::secondary()),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod render {
    use super::*;

    #[test]
    fn it_renders_syncing_idle_error_and_attention_states() {
      let syncing = State {
        syncing: true,
        done: 5,
        total: 10,
        percent: 50,
        errors: 0,
        attention: 0,
        last_synced_secs: None,
        lifecycle: Lifecycle::Active,
        pulse_on: true,
      };
      let _syncing: Element<'_, ()> = sync_chip(syncing);

      let idle = State {
        syncing: false,
        done: 10,
        total: 10,
        percent: 100,
        errors: 0,
        attention: 0,
        last_synced_secs: Some(125),
        lifecycle: Lifecycle::Active,
        pulse_on: false,
      };
      let _idle: Element<'_, ()> = sync_chip(idle);

      let errored = State {
        syncing: false,
        done: 8,
        total: 10,
        percent: 80,
        errors: 2,
        attention: 0,
        last_synced_secs: None,
        lifecycle: Lifecycle::Active,
        pulse_on: false,
      };
      let _errored: Element<'_, ()> = sync_chip(errored);

      let attention = State {
        syncing: false,
        done: 8,
        total: 10,
        percent: 80,
        errors: 0,
        attention: 2,
        last_synced_secs: None,
        lifecycle: Lifecycle::Active,
        pulse_on: false,
      };
      let _attention: Element<'_, ()> = sync_chip(attention);
    }

    #[test]
    fn it_renders_stopped_and_read_only_lifecycle_states() {
      let stopped = State {
        syncing: false,
        done: 3,
        total: 10,
        percent: 30,
        errors: 0,
        attention: 0,
        last_synced_secs: Some(42),
        lifecycle: Lifecycle::Stopped,
        pulse_on: false,
      };
      let _stopped: Element<'_, ()> = sync_chip(stopped);

      let read_only = State {
        syncing: false,
        done: 0,
        total: 10,
        percent: 0,
        errors: 0,
        attention: 0,
        last_synced_secs: None,
        lifecycle: Lifecycle::ReadOnly {
          hostname: Some("nebula".to_owned()),
        },
        pulse_on: false,
      };
      let _read_only: Element<'_, ()> = sync_chip(read_only);
    }
  }
}
