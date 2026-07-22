//! The Settings › Telemetry panel: Pod's discoverable, opt-out telemetry surface
//! (spec mmmzstpq §7.6–§7.8).
//!
//! Telemetry is opt-OUT — every stream ships on by default and the master switch
//! turns the whole pipeline off forever. This panel renders, top to bottom:
//!
//! * the master "Share anonymous usage data" switch (`enabled`);
//! * four per-stream toggles (usage / performance / crashes / environment);
//! * a "never collected" trust list — the hard boundary of what can be sent;
//! * a LIVE sample payload: the exact pretty-printed
//!   [`crate::services::telemetry::contract::Batch`] Pod would POST right now, reflecting
//!   the current stream choices (disabled streams are omitted keys, never a
//!   `crashes` key in a session batch — §6.1);
//! * a read-only anonymous identifier card showing
//!   [`crate::clients::telemetry::anon_id`] of the machine id — the derived,
//!   never-stored sha256 hex. It cannot be reset from here.
//!
//! Every toggle mutates [`crate::config::TelemetryConfig`] on the shared
//! [`Settings`] and returns [`Outcome::Persist`], mirroring how the other toggle
//! tabs persist.

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, container, scrollable, text},
};

use super::Outcome;
use crate::{
  clients::telemetry::anon_id,
  config::Settings,
  services::telemetry::contract::{
    App, Batch, EnvironmentStream, Kind, PerformanceStream, PerformanceViewEntry, Streams, UsageEvent, UsageEventKind,
    UsageStream,
  },
  ui::{
    components::{icon::Icon, rule, toggle},
    style::{color, radius, spacing, typography},
  },
};

const DESCRIPTION_MAX_WIDTH: f32 = 560.0;
const PANEL_SIDE_PADDING: f32 = 36.0;
const PREVIEW_MAX_WIDTH: f32 = 660.0;
const SAMPLE_SESSION: &str = "s_1a2b3c4d";
const SAMPLE_SENT_AT: &str = "2026-06-25T14:32:08Z";

const STREAMS: [Stream; 4] = [
  Stream {
    id: StreamId::Usage,
    title: "settings.telemetry.stream_usage_title",
    desc: "settings.telemetry.stream_usage_desc",
  },
  Stream {
    id: StreamId::Performance,
    title: "settings.telemetry.stream_performance_title",
    desc: "settings.telemetry.stream_performance_desc",
  },
  Stream {
    id: StreamId::Crashes,
    title: "settings.telemetry.stream_crashes_title",
    desc: "settings.telemetry.stream_crashes_desc",
  },
  Stream {
    id: StreamId::Environment,
    title: "settings.telemetry.stream_environment_title",
    desc: "settings.telemetry.stream_environment_desc",
  },
];

const NEVER_COLLECTED: [&str; 6] = [
  "settings.telemetry.never_names",
  "settings.telemetry.never_tokens",
  "settings.telemetry.never_isk",
  "settings.telemetry.never_mail",
  "settings.telemetry.never_assets",
  "settings.telemetry.never_ip",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamId {
  Usage,
  Performance,
  Crashes,
  Environment,
}

impl StreamId {
  fn is_on(self, settings: &Settings) -> bool {
    let telemetry = settings.telemetry();
    match self {
      StreamId::Usage => *telemetry.usage(),
      StreamId::Performance => *telemetry.performance(),
      StreamId::Crashes => *telemetry.crashes(),
      StreamId::Environment => *telemetry.environment(),
    }
  }

  fn set(self, settings: &mut Settings, value: bool) {
    match self {
      StreamId::Usage => settings.telemetry_mut().set_usage(value),
      StreamId::Performance => settings.telemetry_mut().set_performance(value),
      StreamId::Crashes => settings.telemetry_mut().set_crashes(value),
      StreamId::Environment => settings.telemetry_mut().set_environment(value),
    };
  }
}

struct Stream {
  desc: &'static str,
  id: StreamId,
  title: &'static str,
}

#[derive(Clone, Debug)]
pub enum Message {
  EnabledToggled(bool),
  StreamToggled(StreamId, bool),
}

#[derive(Debug)]
pub struct State {
  anon_id: String,
}

impl State {
  pub fn from_settings(settings: &Settings) -> Self {
    let machine_id = settings.storage().machine_id().clone().unwrap_or_default();
    State {
      anon_id: anon_id(&machine_id),
    }
  }
}

pub fn update(_state: &mut State, message: Message, settings: &mut Settings) -> Outcome {
  match message {
    Message::EnabledToggled(value) => {
      settings.telemetry_mut().set_enabled(value);
      Outcome::Persist
    }
    Message::StreamToggled(stream, value) => {
      stream.set(settings, value);
      Outcome::Persist
    }
  }
}

pub fn badge(settings: &Settings) -> String {
  if *settings.telemetry().enabled() {
    t!("settings.telemetry.status_sharing").into_owned()
  } else {
    t!("settings.telemetry.status_off").into_owned()
  }
}

fn sample_batch(anon_id: &str, settings: &Settings) -> Batch {
  let telemetry = settings.telemetry();

  let usage = telemetry.usage().then(|| UsageStream {
    events: vec![
      UsageEvent {
        t: "2026-06-25T14:30:01Z".to_owned(),
        kind: UsageEventKind::ViewOpen,
        name: "wallet".to_owned(),
        on: None,
      },
      UsageEvent {
        t: "2026-06-25T14:31:02Z".to_owned(),
        kind: UsageEventKind::FeatureToggle,
        name: "skills.plan_optimizer".to_owned(),
        on: Some(true),
      },
    ],
  });

  let performance = telemetry.performance().then(|| PerformanceStream {
    views: vec![PerformanceViewEntry {
      name: "wallet".to_owned(),
      load_ms: 142,
      frame_p95_ms: 11,
    }],
    heap_mb: 84,
  });

  let environment = telemetry.environment().then(|| EnvironmentStream {
    os: "macos".to_owned(),
    os_version: "15".to_owned(),
    arch: "aarch64".to_owned(),
    window_size: "2560x1440".to_owned(),
    screen_size: "3440x1440".to_owned(),
    locale: "en".to_owned(),
    app_language: "en-us".to_owned(),
  });

  Batch {
    schema: crate::services::telemetry::contract::SCHEMA_VERSION,
    kind: Kind::Session,
    id: anon_id.to_owned(),
    session: SAMPLE_SESSION.to_owned(),
    app: App {
      version: env!("CARGO_PKG_VERSION").to_owned(),
      git_sha: option_env!("POD_GIT_SHA").map(str::to_owned),
      build_date: option_env!("POD_BUILD_DATE").map(str::to_owned),
    },
    sent_at: SAMPLE_SENT_AT.to_owned(),
    streams: Streams {
      usage,
      performance,
      environment,
      crashes: None,
    },
  }
}

fn sample_payload(anon_id: &str, settings: &Settings) -> String {
  serde_json::to_string_pretty(&sample_batch(anon_id, settings)).unwrap_or_default()
}

pub fn view<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header(settings);
  let body = scrollable(scroll_body(state, settings))
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  Column::with_children(vec![header, body.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header(settings: &Settings) -> Element<'_, Message> {
  let title = text(t!("settings.telemetry.title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(t!("settings.telemetry.panel_blurb"))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let top = Row::with_children(vec![identity.into(), share_badge(settings)])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5);

  let band = container(top).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn share_badge(settings: &Settings) -> Element<'_, Message> {
  let on = *settings.telemetry().enabled();
  let (fg, label) = if on {
    (
      color::status::ONLINE,
      super::i18n::tr_static("settings.telemetry.status_sharing"),
    )
  } else {
    (
      color::text::secondary(),
      super::i18n::tr_static("settings.telemetry.status_off"),
    )
  };

  let dot = container(iced::widget::Space::new())
    .width(Length::Fixed(7.0))
    .height(Length::Fixed(7.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(fg)),
      border: Border {
        radius: 3.5.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let label = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(fg));

  let row = Row::with_children(vec![dot.into(), label.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  container(row)
    .padding(Padding {
      top: spacing::UNIT + 2.0,
      right: spacing::SPACE_3,
      bottom: spacing::UNIT + 2.0,
      left: spacing::SPACE_3,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(fg, if on { 0.4 } else { 0.1 }),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn scroll_body<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let on = *settings.telemetry().enabled();

  let mut children: Vec<Element<'a, Message>> = vec![
    section_head(
      super::i18n::tr_static("settings.telemetry.section_sharing"),
      super::i18n::tr_static("settings.telemetry.section_sharing_note"),
      Some(Icon::pulse()),
    ),
    master_row(settings),
    section_head(
      super::i18n::tr_static("settings.telemetry.section_streams"),
      super::i18n::tr_static("settings.telemetry.section_streams_note"),
      None,
    ),
  ];
  for stream in &STREAMS {
    children.push(stream_row(stream, on, settings));
  }
  children.push(section_head(
    super::i18n::tr_static("settings.telemetry.section_never_collected"),
    super::i18n::tr_static("settings.telemetry.section_never_collected_note"),
    Some(Icon::shield()),
  ));
  children.push(never_collected_card());
  children.push(section_head(
    super::i18n::tr_static("settings.telemetry.section_what_gets_sent"),
    super::i18n::tr_static("settings.telemetry.section_what_gets_sent_note"),
    Some(Icon::upload()),
  ));
  children.push(sample_card(state, settings));
  children.push(section_head(
    super::i18n::tr_static("settings.telemetry.section_anonymous_identifier"),
    super::i18n::tr_static("settings.telemetry.section_anonymous_identifier_note"),
    Some(Icon::block()),
  ));
  children.push(id_card(state));

  let inner = container(
    Column::with_children(children)
      .width(Length::Fill)
      .spacing(spacing::UNIT),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 0.0,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  inner.into()
}

fn section_head<'a>(label: &'a str, note: &'a str, glyph: Option<Icon>) -> Element<'a, Message> {
  let eyebrow = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent()));
  let note = text(note)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![
    eyebrow.into(),
    container(note).max_width(DESCRIPTION_MAX_WIDTH).into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .width(Length::Fill);

  let mut row_children: Vec<Element<'a, Message>> = vec![labels.into()];
  if let Some(glyph) = glyph {
    row_children.push(glyph.size(18.0).color(color::text::secondary()).render());
  }
  let row = Row::with_children(row_children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3_5);

  let band = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: 0.0,
    bottom: spacing::SPACE_3,
    left: 0.0,
  });

  Column::with_children(vec![band.into(), rule::horizontal_alpha(0.18)])
    .width(Length::Fill)
    .into()
}

fn master_row(settings: &Settings) -> Element<'_, Message> {
  let on = *settings.telemetry().enabled();
  let title = text(t!("settings.telemetry.master_title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let desc = text(t!("settings.telemetry.master_desc"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![
    title.into(),
    container(desc).max_width(DESCRIPTION_MAX_WIDTH).into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    Icon::pulse()
      .size(22.0)
      .color(if on { color::accent() } else { color::text::secondary() })
      .render(),
    labels.into(),
    toggle::toggle(on, Message::EnabledToggled(!on)),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_3_5);

  container(row)
    .width(Length::Fill)
    .padding(spacing::SPACE_4_5)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::accent(), if on { 0.45 } else { 0.1 }),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn stream_row<'a>(stream: &'a Stream, master_on: bool, settings: &'a Settings) -> Element<'a, Message> {
  let on = stream.id.is_on(settings);
  let title = text(super::i18n::tr_static(stream.title))
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let desc = text(super::i18n::tr_static(stream.desc))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let labels = Column::with_children(vec![
    title.into(),
    container(desc).max_width(DESCRIPTION_MAX_WIDTH).into(),
  ])
  .spacing(spacing::UNIT)
  .width(Length::Fill);

  let control = if master_on {
    toggle::toggle(on, Message::StreamToggled(stream.id, !on))
  } else {
    toggle::toggle_disabled::<Message>(on)
  };
  let row = Row::with_children(vec![labels.into(), control])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_6);

  let cell = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::UNIT,
    bottom: spacing::SPACE_3,
    left: spacing::UNIT,
  });

  Column::with_children(vec![cell.into(), rule::horizontal_alpha(0.08)])
    .width(Length::Fill)
    .into()
}

fn never_collected_card<'a>() -> Element<'a, Message> {
  let rows: Vec<Element<'a, Message>> = NEVER_COLLECTED
    .iter()
    .map(|item| {
      let label = text(super::i18n::tr_static(item))
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY));
      Row::with_children(vec![
        Icon::block().size(16.0).color(color::status::DANGER).render(),
        label.into(),
      ])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_2_5)
      .into()
    })
    .collect();

  let body = Column::with_children(rows)
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill);

  container(body)
    .width(Length::Fill)
    .max_width(PREVIEW_MAX_WIDTH)
    .padding(spacing::SPACE_4_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::ONLINE, 0.04))),
      border: Border {
        color: color::with_alpha(color::status::ONLINE, 0.28),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn sample_card<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let on = *settings.telemetry().enabled();
  let eyebrow = text(t!("settings.telemetry.sample_eyebrow"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));
  let payload = text(sample_payload(&state.anon_id, settings))
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(if on {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    }));

  let body = Column::with_children(vec![eyebrow.into(), payload.into()]).spacing(spacing::SPACE_3);

  container(body)
    .width(Length::Fill)
    .max_width(PREVIEW_MAX_WIDTH)
    .padding(spacing::SPACE_4_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn id_card(state: &State) -> Element<'_, Message> {
  let eyebrow = text(t!("settings.telemetry.id_eyebrow"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()));
  let value = text(state.anon_id.clone())
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let note = text(t!("settings.telemetry.id_note"))
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));

  let body = Column::with_children(vec![
    eyebrow.into(),
    value.into(),
    container(note).max_width(DESCRIPTION_MAX_WIDTH).into(),
  ])
  .spacing(spacing::SPACE_2_5);

  container(body)
    .width(Length::Fill)
    .max_width(PREVIEW_MAX_WIDTH)
    .padding(spacing::SPACE_4_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn state() -> State {
    State::from_settings(&Settings::default())
  }

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_sharing_when_enabled() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);

      assert_eq!(badge(&Settings::default()), "Sharing");
    }

    #[test]
    fn it_reads_off_when_the_master_is_disabled() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);

      let mut settings = Settings::default();
      settings.telemetry_mut().set_enabled(false);

      assert_eq!(badge(&settings), "Off");
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn toggling_a_stream_off_persists_the_change() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(
        &mut state,
        Message::StreamToggled(StreamId::Usage, false),
        &mut settings,
      );

      assert_eq!(outcome, Outcome::Persist);
      assert!(!*settings.telemetry().usage());
      assert!(
        *settings.telemetry().performance(),
        "toggling usage must not touch the other streams"
      );
    }

    #[test]
    fn toggling_the_master_off_persists_the_change() {
      let mut state = state();
      let mut settings = Settings::default();

      let outcome = update(&mut state, Message::EnabledToggled(false), &mut settings);

      assert_eq!(outcome, Outcome::Persist);
      assert!(!*settings.telemetry().enabled());
    }

    #[test]
    fn each_stream_toggle_targets_its_own_flag() {
      let mut state = state();
      let mut settings = Settings::default();

      for stream in [
        StreamId::Usage,
        StreamId::Performance,
        StreamId::Crashes,
        StreamId::Environment,
      ] {
        update(&mut state, Message::StreamToggled(stream, false), &mut settings);
        assert!(!stream.is_on(&settings), "{stream:?} must follow its own toggle");
      }
    }
  }

  mod sample_payload {
    use pretty_assertions::assert_eq;
    use serde_json::Value;

    use super::*;

    #[test]
    fn it_is_a_valid_session_batch_carrying_the_derived_id() {
      let settings = Settings::default();
      let json = sample_payload("0123abcd", &settings);
      let value: Value = serde_json::from_str(&json).unwrap();

      assert_eq!(value["kind"], Value::from("session"));
      assert_eq!(value["id"], Value::from("0123abcd"));
      assert_eq!(value["schema"], Value::from(1));
    }

    #[test]
    fn a_session_batch_never_carries_a_crashes_key() {
      let mut settings = Settings::default();
      settings.telemetry_mut().set_crashes(true);

      let value: Value = serde_json::from_str(&sample_payload("id", &settings)).unwrap();
      let streams = value["streams"].as_object().unwrap();

      assert!(
        !streams.contains_key("crashes"),
        "a session preview never includes a crashes stream"
      );
    }

    #[test]
    fn a_disabled_stream_is_an_omitted_key_never_null() {
      let mut settings = Settings::default();
      settings.telemetry_mut().set_usage(false);

      let value: Value = serde_json::from_str(&sample_payload("id", &settings)).unwrap();
      let streams = value["streams"].as_object().unwrap();

      assert!(
        !streams.contains_key("usage"),
        "a disabled stream is omitted, never null"
      );
      assert!(streams.contains_key("performance"));
      assert!(streams.contains_key("environment"));
    }

    #[test]
    fn all_streams_off_leaves_an_empty_streams_object() {
      let mut settings = Settings::default();
      settings.telemetry_mut().set_usage(false);
      settings.telemetry_mut().set_performance(false);
      settings.telemetry_mut().set_environment(false);

      let value: Value = serde_json::from_str(&sample_payload("id", &settings)).unwrap();
      let streams = value["streams"].as_object().unwrap();

      assert!(
        streams.is_empty(),
        "every session stream off yields an empty streams object"
      );
    }

    #[test]
    fn the_id_matches_the_sender_anon_id_of_the_machine_id() {
      let mut settings = Settings::default();
      settings.storage_mut().set_machine_id(Some("pod-machine".to_owned()));
      let state = State::from_settings(&settings);

      let value: Value = serde_json::from_str(&sample_payload(&state.anon_id, &settings)).unwrap();

      assert_eq!(value["id"], Value::from(anon_id("pod-machine")));
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_with_sharing_on() {
      let state = state();
      let settings = Settings::default();
      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_with_sharing_off() {
      let mut settings = Settings::default();
      settings.telemetry_mut().set_enabled(false);
      let state = State::from_settings(&settings);

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
