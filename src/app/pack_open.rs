use std::{collections::VecDeque, path::PathBuf};

use iced::Border;

use super::*;
use crate::{services::pod_pack, ui::components::rule};

const MODAL_WIDTH: f32 = 440.0;

const FOOTER_PAD_X: f32 = 16.0;

const FOOTER_PAD_Y: f32 = 12.0;

const HEADER_PAD_BOTTOM: f32 = 14.0;

const HEADER_PAD_X: f32 = 20.0;

const HEADER_PAD_TOP: f32 = 18.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Format {
  BudgetRules,
  FacilityIntel,
  SkillPlan,
}

impl Format {
  fn label(self) -> String {
    match self {
      Self::BudgetRules => t!("shell.pack_open.format_budget_rules").into_owned(),
      Self::FacilityIntel => t!("shell.pack_open.format_facility_intel").into_owned(),
      Self::SkillPlan => t!("shell.pack_open.format_skill_plan").into_owned(),
    }
  }
}

#[derive(Clone, Debug)]
enum Detected {
  Error { message: String },
  Ready { format: Format, summary: String },
}

#[derive(Clone, Debug)]
pub(super) struct Prompt {
  content: String,
  detected: Detected,
  file_name: String,
}

#[derive(Debug, Default)]
enum Stage {
  #[default]
  Idle,
  Loading,
  Prompting(Prompt),
}

#[derive(Debug, Default)]
pub(super) struct State {
  queue: VecDeque<PathBuf>,
  stage: Stage,
}

impl State {
  pub(super) fn prompt(&self) -> Option<&Prompt> {
    match &self.stage {
      Stage::Prompting(prompt) => Some(prompt),
      _ => None,
    }
  }

  /// Only leaves `Idle`, so a file dropped while another is loading or already prompting stays queued instead of
  /// interrupting the prompt on screen.
  fn begin_next(&mut self) -> Option<PathBuf> {
    if !matches!(self.stage, Stage::Idle) {
      return None;
    }
    let path = self.queue.pop_front()?;
    self.stage = Stage::Loading;
    Some(path)
  }

  fn enqueue(&mut self, path: PathBuf) {
    self.queue.push_back(path);
  }

  fn resolve(&mut self) -> Option<Prompt> {
    match std::mem::replace(&mut self.stage, Stage::Idle) {
      Stage::Prompting(prompt) => Some(prompt),
      other => {
        self.stage = other;
        None
      }
    }
  }

  fn show(&mut self, prompt: Prompt) {
    self.stage = Stage::Prompting(prompt);
  }
}

pub(super) fn handle_pack_file_opened(app: &mut App, path: PathBuf) -> Task<Message> {
  app.pack_open.enqueue(path);
  advance(app)
}

pub(super) fn handle_pack_file_processed(app: &mut App, prompt: Prompt) -> Task<Message> {
  app.pack_open.show(prompt);
  Task::none()
}

pub(super) fn handle_pack_confirmed(app: &mut App) -> Task<Message> {
  let route = match app.pack_open.resolve() {
    Some(Prompt {
      content,
      detected: Detected::Ready {
        format, ..
      },
      ..
    }) => route_pack(app, format, content),
    _ => Task::none(),
  };
  Task::batch([route, advance(app)])
}

pub(super) fn handle_pack_declined(app: &mut App) -> Task<Message> {
  app.pack_open.resolve();
  advance(app)
}

pub(super) fn advance(app: &mut App) -> Task<Message> {
  if app.runtime.is_none() {
    return Task::none();
  }
  let Some(path) = app.pack_open.begin_next() else {
    return Task::none();
  };
  let file_name = file_display_name(&path);
  Task::perform(
    async move {
      let read = tokio::fs::read_to_string(&path).await;
      Box::new(classify(file_name, read))
    },
    Message::PackFileProcessed,
  )
}

fn route_pack(app: &mut App, format: Format, content: String) -> Task<Message> {
  match format {
    Format::BudgetRules => {
      let ensure = navigate_to_wallet(app);
      let open = open_budget_rules_window(app);
      let load = handle_budget_rules(app, wallet::budget_rules::Message::ImportFileLoaded(Some(content)));
      Task::batch([ensure, open, load])
    }
    Format::FacilityIntel => {
      let nav = handle_nav_to(app, rail::Destination::Settings, Some("facilities"));
      let run = handle_settings(
        app,
        settings::Message::Facility(settings::facility_tab::Message::ImportFileLoaded(Some(content))),
      );
      Task::batch([nav, run])
    }
    Format::SkillPlan => {
      let (character_id, seed) = match resolve_skills_target(&roster(app), app.selected_character) {
        Some(id) => (Some(id), skill_plan_editor::Seed::New),
        None => (None, skill_plan_editor::Seed::NewTemplate),
      };
      let (editor_id, open) = open_editor_window(app, character_id, seed);
      let load = match editor_id {
        Some(id) => handle_skill_plan_editor(app, id, skill_plan_editor::Message::ImportFileLoaded(Some(content))),
        None => Task::none(),
      };
      Task::batch([open, load])
    }
  }
}

fn file_display_name(path: &std::path::Path) -> String {
  path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// `file_name` is display-only; the format is decided by the pack's embedded tag (via `pod_pack::sniff`), not by
/// the file's name or extension.
fn classify(file_name: String, read: std::io::Result<String>) -> Prompt {
  let Ok(content) = read else {
    return Prompt {
      content: String::new(),
      detected: Detected::Error {
        message: t!("shell.pack_open.error_unreadable").into_owned(),
      },
      file_name,
    };
  };

  let detected = match pod_pack::sniff(&content) {
    Ok(tag) => detect_tag(&tag, &content),
    Err(error) => Detected::Error {
      message: decode_error_message(&error),
    },
  };

  Prompt {
    content,
    detected,
    file_name,
  }
}

fn detect_tag(tag: &str, content: &str) -> Detected {
  match tag {
    pod_pack::TAG_BUDGET_RULES => {
      match pod_pack::decode::<wallet::rule_pack::PackEnvelope>(
        pod_pack::TAG_BUDGET_RULES,
        wallet::rule_pack::PACK_VERSION,
        content,
      ) {
        Ok(pack) => ready(Format::BudgetRules, pack.rules.len()),
        Err(error) => Detected::Error {
          message: decode_error_message(&error),
        },
      }
    }
    pod_pack::TAG_FACILITY_INTEL => {
      match pod_pack::decode::<settings::facility_intel_share::PackEnvelope>(
        pod_pack::TAG_FACILITY_INTEL,
        settings::facility_intel_share::PACK_VERSION,
        content,
      ) {
        Ok(pack) => ready(Format::FacilityIntel, pack.facilities.len()),
        Err(error) => Detected::Error {
          message: decode_error_message(&error),
        },
      }
    }
    pod_pack::TAG_SKILL_PLAN => match skill_plan_editor::pack_skill_count(content) {
      Ok(count) => ready(Format::SkillPlan, count),
      Err(error) => Detected::Error {
        message: decode_error_message(&error),
      },
    },
    _ => Detected::Error {
      message: t!("shell.pack_open.error_unsupported").into_owned(),
    },
  }
}

fn ready(format: Format, count: usize) -> Detected {
  Detected::Ready {
    format,
    summary: summary_line(format, count),
  }
}

fn summary_line(format: Format, count: usize) -> String {
  match (format, count) {
    (Format::BudgetRules, 1) => t!("shell.pack_open.summary_budget_rules_one", count => count),
    (Format::BudgetRules, _) => t!("shell.pack_open.summary_budget_rules_other", count => count),
    (Format::FacilityIntel, 1) => t!("shell.pack_open.summary_facility_intel_one", count => count),
    (Format::FacilityIntel, _) => t!("shell.pack_open.summary_facility_intel_other", count => count),
    (Format::SkillPlan, 1) => t!("shell.pack_open.summary_skill_plan_one", count => count),
    (Format::SkillPlan, _) => t!("shell.pack_open.summary_skill_plan_other", count => count),
  }
  .into_owned()
}

fn decode_error_message(error: &pod_pack::DecodeError) -> String {
  match error {
    pod_pack::DecodeError::UnsupportedVersion {
      ..
    } => t!("shell.pack_open.error_version"),
    pod_pack::DecodeError::ChecksumMismatch {
      ..
    }
    | pod_pack::DecodeError::Json(_)
    | pod_pack::DecodeError::Truncated
    | pod_pack::DecodeError::WrongFormat {
      ..
    } => t!("shell.pack_open.error_corrupt"),
    pod_pack::DecodeError::Base64(_) | pod_pack::DecodeError::Inflate(_) | pod_pack::DecodeError::NotAPack => {
      t!("shell.pack_open.error_unsupported")
    }
  }
  .into_owned()
}

pub(super) fn overlay(prompt: &Prompt) -> Element<'_, Message> {
  let (eyebrow_text, eyebrow_color, body_text) = match &prompt.detected {
    Detected::Ready {
      format,
      summary,
    } => (format.label(), color::accent(), summary.clone()),
    Detected::Error {
      message,
    } => (
      t!("shell.pack_open.error_eyebrow").into_owned(),
      color::status::DANGER,
      message.clone(),
    ),
  };

  let eyebrow = text(eyebrow_text)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(eyebrow_color),
    });
  let title = text(prompt.file_name.clone())
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let body = text(body_text)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let header_block =
    container(Column::with_children(vec![eyebrow.into(), title.into(), body.into()]).spacing(spacing::SPACE_2))
      .width(Length::Fill)
      .padding(Padding {
        top: HEADER_PAD_TOP,
        right: HEADER_PAD_X,
        bottom: HEADER_PAD_BOTTOM,
        left: HEADER_PAD_X,
      });

  let mut buttons: Vec<Element<'_, Message>> = vec![Space::new().width(Length::Fill).into()];
  match &prompt.detected {
    Detected::Ready {
      ..
    } => {
      buttons.push(
        Button::ghost(t!("common.cancel").into_owned())
          .on_press(Message::PackDeclined)
          .into(),
      );
      buttons.push(
        Button::primary(t!("shell.pack_open.import").into_owned())
          .on_press(Message::PackConfirmed)
          .into(),
      );
    }
    Detected::Error {
      ..
    } => {
      buttons.push(
        Button::ghost(t!("shell.pack_open.dismiss").into_owned())
          .on_press(Message::PackDeclined)
          .into(),
      );
    }
  }

  let footer = container(
    Row::with_children(buttons)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: FOOTER_PAD_Y,
    right: FOOTER_PAD_X,
    bottom: FOOTER_PAD_Y,
    left: FOOTER_PAD_X,
  });

  let card =
    container(Column::with_children(vec![header_block.into(), rule::horizontal(), footer.into()]).width(Length::Fill))
      .width(Length::Fixed(MODAL_WIDTH))
      .clip(true)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::rule_strong(),
          width: 1.0,
          radius: radius::CARD.into(),
        },
        shadow: shadow::CARD,
        ..container::Style::default()
      });

  card.into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  fn budget_pack(rules: usize) -> String {
    let rule = wallet::rule_pack::PortableRule::default();
    let pack = wallet::rule_pack::build_pack(vec![rule; rules], "Test", "");
    wallet::rule_pack::encode_pack(&pack).unwrap()
  }

  fn facility_pack() -> String {
    let pack = settings::facility_intel_share::build_pack(Vec::new());
    settings::facility_intel_share::encode_pack(&pack).unwrap()
  }

  mod classify {
    use super::*;

    #[test]
    fn it_reports_a_ready_budget_pack_with_its_count() {
      let prompt = classify("rules.pbr".to_owned(), Ok(budget_pack(3)));

      match prompt.detected {
        Detected::Ready {
          format, ..
        } => assert_eq!(format, Format::BudgetRules),
        other => panic!("expected a ready budget pack, got {other:?}"),
      }
    }

    #[test]
    fn it_routes_by_content_not_extension() {
      let prompt = classify("mislabeled.pbr".to_owned(), Ok(facility_pack()));

      match prompt.detected {
        Detected::Ready {
          format, ..
        } => assert_eq!(format, Format::FacilityIntel),
        other => panic!("expected the sniffed facility intel format, got {other:?}"),
      }
    }

    #[test]
    fn it_surfaces_an_error_for_an_unreadable_file() {
      let prompt = classify(
        "missing.pbr".to_owned(),
        Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
      );

      assert!(matches!(prompt.detected, Detected::Error { .. }));
    }

    #[test]
    fn it_surfaces_an_error_for_a_tampered_pack() {
      let mut encoded = budget_pack(2);
      encoded.insert(4, 'A');

      let prompt = classify("rules.pbr".to_owned(), Ok(encoded));

      assert!(matches!(prompt.detected, Detected::Error { .. }));
    }

    #[test]
    fn it_surfaces_an_error_for_non_pack_text() {
      let prompt = classify("notes.pbr".to_owned(), Ok("just some plain text".to_owned()));

      assert!(matches!(prompt.detected, Detected::Error { .. }));
    }
  }

  mod queue {
    use super::*;

    #[tokio::test]
    async fn it_holds_files_delivered_before_the_runtime_is_ready() {
      let mut app = test_app();

      let _ = handle_pack_file_opened(&mut app, PathBuf::from("first.pbr"));
      let _ = handle_pack_file_opened(&mut app, PathBuf::from("second.psp"));

      assert!(matches!(app.pack_open.stage, Stage::Idle));
      assert_eq!(app.pack_open.queue.len(), 2);
    }

    #[tokio::test]
    async fn it_begins_loading_the_first_file_once_the_runtime_is_ready() {
      let mut app = test_app();
      let _ = handle_pack_file_opened(&mut app, PathBuf::from("first.pbr"));
      let _ = handle_pack_file_opened(&mut app, PathBuf::from("second.psp"));

      app.runtime = Some(test_runtime().await);
      let _ = advance(&mut app);

      assert!(matches!(app.pack_open.stage, Stage::Loading));
      assert_eq!(app.pack_open.queue.len(), 1);
    }

    #[tokio::test]
    async fn it_does_not_begin_a_second_file_while_one_is_prompting() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      app.pack_open.show(sample_prompt());
      app.pack_open.enqueue(PathBuf::from("second.psp"));

      let _ = advance(&mut app);

      assert!(matches!(app.pack_open.stage, Stage::Prompting(_)));
      assert_eq!(app.pack_open.queue.len(), 1);
    }
  }

  mod resolve {
    use super::*;

    #[tokio::test]
    async fn it_clears_the_prompt_when_declined() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      app.pack_open.show(sample_prompt());

      let _ = handle_pack_declined(&mut app);

      assert!(app.pack_open.prompt().is_none());
    }

    #[tokio::test]
    async fn it_clears_the_prompt_when_confirmed() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      app.pack_open.show(sample_prompt());

      let _ = handle_pack_confirmed(&mut app);

      assert!(app.pack_open.prompt().is_none());
    }

    #[tokio::test]
    async fn it_advances_to_the_next_queued_file_after_a_decision() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      app.pack_open.show(sample_prompt());
      app.pack_open.enqueue(PathBuf::from("next.pbr"));

      let _ = handle_pack_declined(&mut app);

      assert!(matches!(app.pack_open.stage, Stage::Loading));
      assert_eq!(app.pack_open.queue.len(), 0);
    }
  }

  mod route_pack {
    use super::*;

    #[tokio::test]
    async fn it_routes_a_budget_pack_to_the_budget_rules_window() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      app.pack_open.show(ready_prompt(Format::BudgetRules, budget_pack(2)));

      let _ = handle_pack_confirmed(&mut app);

      assert_eq!(app.route, Route::Wallet);
      assert!(app.budget_rules.is_some());
    }

    #[tokio::test]
    async fn it_routes_a_facility_pack_to_the_settings_facility_tab() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      app.pack_open.show(ready_prompt(Format::FacilityIntel, facility_pack()));

      let _ = handle_pack_confirmed(&mut app);

      assert_eq!(app.route, Route::Settings);
    }

    #[tokio::test]
    async fn it_routes_a_skill_plan_pack_to_the_editor() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      app.pack_open.show(ready_prompt(Format::SkillPlan, String::new()));

      let _ = handle_pack_confirmed(&mut app);

      assert!(!app.editors.is_empty());
    }

    #[tokio::test]
    async fn it_routes_nothing_when_the_prompt_is_an_error() {
      let mut app = ready_app();
      app.runtime = Some(test_runtime().await);
      let route_before = app.route;
      app.pack_open.show(sample_prompt());

      let _ = handle_pack_confirmed(&mut app);

      assert_eq!(app.route, route_before);
      assert!(app.budget_rules.is_none());
      assert!(app.editors.is_empty());
    }
  }

  mod overlay {
    use super::*;

    #[test]
    fn it_renders_the_confirm_prompt_for_a_ready_pack() {
      let prompt = ready_prompt(Format::BudgetRules, String::new());

      let _el: Element<'_, Message> = overlay(&prompt);
    }

    #[test]
    fn it_renders_the_dismiss_prompt_for_an_undecodable_pack() {
      let prompt = sample_prompt();

      let _el: Element<'_, Message> = overlay(&prompt);
    }
  }

  fn ready_prompt(format: Format, content: String) -> Prompt {
    Prompt {
      content,
      detected: ready(format, 2),
      file_name: "sample.pack".to_owned(),
    }
  }

  fn sample_prompt() -> Prompt {
    Prompt {
      content: String::new(),
      detected: Detected::Error {
        message: "boom".to_owned(),
      },
      file_name: "sample.pbr".to_owned(),
    }
  }
}
