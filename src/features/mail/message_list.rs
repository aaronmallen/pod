use chrono::{DateTime, Duration, Timelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, mouse_area, scrollable, text},
};

use super::{Folder, Message, Scope, StandardFolder, State, loaders::strip_html_snippet};
use crate::{
  store::{Database, images, repo::mail},
  ui::{
    components::{
      avatar::Avatar, chip::chip, empty_state::empty_state as shared_empty_state, icon::Icon, rule,
      section_header::section_header, text_input::TextInput,
    },
    style::{color, radius, spacing, typography},
  },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageRow {
  pub bucket: DayBucket,
  pub character_id: i64,
  pub has_attachment: bool,
  pub important: bool,
  pub is_pinned: bool,
  pub is_read: bool,
  pub is_starred: bool,
  pub labels: Vec<String>,
  pub mail_id: i64,
  pub sender: String,
  pub sender_id: i64,
  pub sender_kind: SenderKind,
  pub sender_portrait: images::ImageState,
  pub snippet: String,
  pub subject: String,
  pub time: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SenderKind {
  Character,
  Corp,
  System,
}

impl SenderKind {
  fn from_flags(from_corp: bool, from_system: bool) -> Self {
    if from_system {
      SenderKind::System
    } else if from_corp {
      SenderKind::Corp
    } else {
      SenderKind::Character
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DayBucket {
  Today,
  Yesterday,
  Earlier,
}

impl DayBucket {
  fn label(self) -> &'static str {
    match self {
      DayBucket::Today => "Today",
      DayBucket::Yesterday => "Yesterday",
      DayBucket::Earlier => "Earlier this week",
    }
  }
}

struct MailKey {
  character_id: i64,
  has_attachment: bool,
  important: bool,
  is_read: bool,
  mail_id: i64,
  sender: String,
  sender_id: i64,
  sender_kind: SenderKind,
  subject: String,
  timestamp: String,
}

pub(super) async fn load_messages(db: &Database, scope: Scope, folder: Folder) -> Vec<MessageRow> {
  let now = Utc::now();
  let now_iso = now.to_rfc3339();

  let Scope::Character(id) = scope;
  let keys: Vec<MailKey> = if matches!(folder, Folder::Unified) {
    unified_keys(db, &now_iso).await
  } else {
    character_keys(db, id, folder, &now_iso).await
  };

  let mut rows = Vec::with_capacity(keys.len());
  for key in keys {
    if matches!(folder, Folder::Unified)
      && !unified_in_folder(db, key.character_id, key.mail_id, folder, &now_iso).await
    {
      continue;
    }
    rows.push(key_to_row(db, key, now).await);
  }

  rows.sort_by_key(|r| !r.is_pinned);
  rows
}

pub(super) async fn load_all_messages(db: &Database, scope: Scope, folder: Folder) -> Vec<MessageRow> {
  let now = Utc::now();

  let Scope::Character(id) = scope;
  let keys: Vec<MailKey> = if matches!(folder, Folder::Unified) {
    all_unified_keys(db).await
  } else {
    all_character_keys(db, id).await
  };

  let mut rows = Vec::with_capacity(keys.len());
  for key in keys {
    rows.push(key_to_row(db, key, now).await);
  }

  rows.sort_by_key(|r| !r.is_pinned);
  rows
}

const SNIPPET_MAX_CHARS: usize = 120;

fn snippet_preview(body: &str) -> String {
  if body.chars().count() <= SNIPPET_MAX_CHARS {
    return body.to_owned();
  }
  let cutoff: String = body.chars().take(SNIPPET_MAX_CHARS).collect();
  let trimmed = match cutoff.rfind(char::is_whitespace) {
    Some(pos) => cutoff[..pos].to_owned(),
    None => cutoff,
  };
  format!("{trimmed}\u{2026}")
}

async fn key_to_row(db: &Database, key: MailKey, now: DateTime<Utc>) -> MessageRow {
  let overlay = mail::overlay_state(db, key.character_id, key.mail_id)
    .await
    .unwrap_or_default();
  let label_ids = mail::membership(db, key.character_id, key.mail_id)
    .await
    .unwrap_or_default();
  let labels = resolve_label_names(db, key.character_id, &label_ids).await;
  let snippet = snippet_preview(&strip_html_snippet(
    &mail::body(db, key.character_id, key.mail_id)
      .await
      .ok()
      .flatten()
      .map(|b| b.body().clone())
      .unwrap_or_default(),
  ));

  let sender_portrait = super::loaders::resolve_sender_portrait(key.sender_id);

  MessageRow {
    bucket: day_bucket(&key.timestamp, now),
    character_id: key.character_id,
    has_attachment: key.has_attachment,
    important: key.important,
    is_pinned: overlay.is_pinned,
    is_read: key.is_read,
    is_starred: overlay.is_starred,
    labels,
    mail_id: key.mail_id,
    sender: key.sender,
    sender_id: key.sender_id,
    sender_kind: key.sender_kind,
    sender_portrait,
    snippet,
    subject: subject_or_no_subject(&key.subject),
    time: time_label(&key.timestamp),
  }
}

async fn all_unified_keys(db: &Database) -> Vec<MailKey> {
  mail::unified(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|m| MailKey {
      character_id: m.character_id,
      has_attachment: m.has_attachment,
      important: m.important,
      is_read: m.is_read,
      mail_id: m.mail_id,
      sender: m.from_name,
      sender_id: m.from_id,
      sender_kind: SenderKind::from_flags(m.from_corp, m.from_system),
      subject: m.subject.unwrap_or_default(),
      timestamp: m.timestamp,
    })
    .collect()
}

async fn all_character_keys(db: &Database, character_id: i64) -> Vec<MailKey> {
  mail::headers(db, character_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|h| MailKey {
      character_id: h.character_id(),
      has_attachment: h.has_attachment(),
      important: h.important(),
      is_read: h.is_read(),
      mail_id: h.mail_id(),
      sender: h.from_name().clone(),
      sender_id: h.from_id(),
      sender_kind: SenderKind::from_flags(h.from_corp(), h.from_system()),
      subject: h.subject().clone().unwrap_or_default(),
      timestamp: h.timestamp().clone(),
    })
    .collect()
}

async fn unified_keys(db: &Database, now: &str) -> Vec<MailKey> {
  mail::visible_unified(db, now)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|m| MailKey {
      character_id: m.character_id,
      has_attachment: m.has_attachment,
      important: m.important,
      is_read: m.is_read,
      mail_id: m.mail_id,
      sender: m.from_name,
      sender_id: m.from_id,
      sender_kind: SenderKind::from_flags(m.from_corp, m.from_system),
      subject: m.subject.unwrap_or_default(),
      timestamp: m.timestamp,
    })
    .collect()
}

async fn character_keys(db: &Database, character_id: i64, folder: Folder, now: &str) -> Vec<MailKey> {
  let headers = match folder {
    Folder::Unified | Folder::Standard(StandardFolder::Inbox) => {
      let visible = mail::visible_headers(db, character_id, now).await.unwrap_or_default();
      visible.into_iter().filter(|h| h.from_id() != character_id).collect()
    }
    Folder::Standard(StandardFolder::Sent) => mail::headers(db, character_id)
      .await
      .unwrap_or_default()
      .into_iter()
      .filter(|h| h.from_id() == character_id)
      .collect(),
    Folder::Standard(StandardFolder::Drafts) => Vec::new(),
    Folder::Label(label_id) => mail::visible_headers_for_label(db, character_id, label_id, now)
      .await
      .unwrap_or_default(),
    Folder::Standard(standard_folder) => {
      let ids = overlay_folder_ids(db, character_id, standard_folder, now).await;
      let all = mail::headers(db, character_id).await.unwrap_or_default();
      all.into_iter().filter(|h| ids.contains(&h.mail_id())).collect()
    }
  };

  headers
    .into_iter()
    .map(|h| MailKey {
      character_id: h.character_id(),
      has_attachment: h.has_attachment(),
      important: h.important(),
      is_read: h.is_read(),
      mail_id: h.mail_id(),
      sender: h.from_name().clone(),
      sender_id: h.from_id(),
      sender_kind: SenderKind::from_flags(h.from_corp(), h.from_system()),
      subject: h.subject().clone().unwrap_or_default(),
      timestamp: h.timestamp().clone(),
    })
    .collect()
}

async fn unified_in_folder(db: &Database, character_id: i64, mail_id: i64, folder: Folder, now: &str) -> bool {
  match folder {
    Folder::Unified | Folder::Standard(StandardFolder::Inbox) => true,
    Folder::Standard(StandardFolder::Sent | StandardFolder::Drafts) => false,
    Folder::Label(label_id) => mail::membership(db, character_id, mail_id)
      .await
      .unwrap_or_default()
      .contains(&label_id),
    Folder::Standard(standard_folder) => overlay_folder_ids(db, character_id, standard_folder, now)
      .await
      .contains(&mail_id),
  }
}

async fn overlay_folder_ids(db: &Database, character_id: i64, standard_folder: StandardFolder, now: &str) -> Vec<i64> {
  match standard_folder {
    StandardFolder::Archive => mail::folder_mail_ids(db, character_id, "archive")
      .await
      .unwrap_or_default(),
    StandardFolder::Trash => mail::folder_mail_ids(db, character_id, "trash")
      .await
      .unwrap_or_default(),
    StandardFolder::Starred => mail::starred_mail_ids(db, character_id).await.unwrap_or_default(),
    StandardFolder::Snoozed => mail::all_snoozed_mails(db, character_id)
      .await
      .unwrap_or_default()
      .into_iter()
      .filter(|s| s.snooze_until().as_str() > now)
      .map(|s| s.mail_id())
      .collect(),
    StandardFolder::Inbox | StandardFolder::Sent | StandardFolder::Drafts => Vec::new(),
  }
}

async fn resolve_label_names(db: &Database, character_id: i64, label_ids: &[i64]) -> Vec<String> {
  if label_ids.is_empty() {
    return Vec::new();
  }
  let catalog = mail::labels(db, character_id).await.unwrap_or_default();
  label_ids
    .iter()
    .filter_map(|id| {
      catalog
        .iter()
        .find(|l| l.label_id() == *id)
        .map(|l| l.name().to_owned())
    })
    .collect()
}

fn subject_or_no_subject(subject: &str) -> String {
  if subject.trim().is_empty() {
    "(no subject)".to_owned()
  } else {
    subject.to_owned()
  }
}

fn day_bucket(timestamp: &str, now: DateTime<Utc>) -> DayBucket {
  let Ok(ts) = DateTime::parse_from_rfc3339(timestamp) else {
    return DayBucket::Earlier;
  };
  let ts = ts.with_timezone(&Utc);
  let today = now.date_naive();
  let day = ts.date_naive();
  if day == today {
    DayBucket::Today
  } else if day == (now - Duration::days(1)).date_naive() {
    DayBucket::Yesterday
  } else {
    DayBucket::Earlier
  }
}

fn time_label(timestamp: &str) -> String {
  match DateTime::parse_from_rfc3339(timestamp) {
    Ok(ts) => {
      let ts = ts.with_timezone(&Utc);
      format!("{:02}:{:02}", ts.hour(), ts.minute())
    }
    Err(_) => timestamp.to_owned(),
  }
}

pub(super) fn pane<'a>(state: &'a State, rows: &'a [MessageRow], width: f32) -> Element<'a, Message> {
  let query = state.search().to_lowercase();
  let source: &[MessageRow] = if query.is_empty() { rows } else { state.all_messages() };
  let filtered: Vec<&MessageRow> = source.iter().filter(|r| matches_query(r, &query)).collect();

  let mut body = Column::new().width(Length::Fill);
  if filtered.is_empty() {
    body = body.push(empty_state(state.search()));
  } else {
    for bucket in [DayBucket::Today, DayBucket::Yesterday, DayBucket::Earlier] {
      let group: Vec<&MessageRow> = filtered.iter().copied().filter(|r| r.bucket == bucket).collect();
      if group.is_empty() {
        continue;
      }
      body = body.push(day_header(bucket.label()));
      for row in group {
        body = body.push(message_row(row, state.selected() == Some(row.mail_id)));
      }
    }
  }

  let list = scrollable(body)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  let column = Column::with_children(vec![search_box(state.search()), list.into()])
    .width(Length::Fill)
    .height(Length::Fill);

  let surface = container(column)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    });

  container(Row::with_children(vec![surface.into(), right_rule()]).width(Length::Fill))
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .into()
}

fn matches_query(row: &MessageRow, query: &str) -> bool {
  if query.is_empty() {
    return true;
  }
  row.subject.to_lowercase().contains(query)
    || row.sender.to_lowercase().contains(query)
    || row.snippet.to_lowercase().contains(query)
}

fn search_box(query: &str) -> Element<'_, Message> {
  let field = TextInput::new("Search mail", query, Message::SearchChanged)
    .leading_icon(Icon::search())
    .icon_size(16.0)
    .icon_spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .render();

  let field = container(field).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2 * 2.0,
    right: spacing::SPACE_2 * 2.0,
  });

  Column::with_children(vec![field.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn day_header(label: &str) -> Element<'_, Message> {
  container(section_header(label, None))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2 - 2.0,
      left: spacing::SPACE_2 * 2.0,
      right: spacing::SPACE_2 * 2.0,
    })
    .into()
}

fn message_row(row: &MessageRow, selected: bool) -> Element<'_, Message> {
  let mut sender_line = Row::new().spacing(spacing::SPACE_2 - 2.0).align_y(Vertical::Center);
  if let Some(icon) = sender_kind_icon(row.sender_kind) {
    sender_line = sender_line.push(icon);
  }
  sender_line = sender_line.push(
    text(row.sender.clone())
      .size(typography::size::MD)
      .font(if row.is_read {
        typography::body::REGULAR
      } else {
        typography::body::MEDIUM
      })
      .style(move |_| text::Style {
        color: Some(if row.is_read {
          color::text::secondary()
        } else {
          color::text::PRIMARY
        }),
      }),
  );

  let sender = Row::with_children(vec![
    container(sender_line).width(Length::Fill).into(),
    text(row.time.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let mut subject_row = Row::new().spacing(spacing::SPACE_2 - 2.0).align_y(Vertical::Center);
  if row.is_pinned {
    subject_row = subject_row.push(glyph("\u{1f4cc}"));
  } else if row.is_starred {
    subject_row = subject_row.push(glyph("\u{2605}"));
  }
  if row.important {
    subject_row = subject_row.push(importance_flag());
  }
  subject_row = subject_row.push(
    text(row.subject.clone())
      .size(typography::size::MD)
      .font(if row.is_read {
        typography::body::REGULAR
      } else {
        typography::body::MEDIUM
      })
      .style(move |_| text::Style {
        color: Some(if row.is_read {
          color::text::secondary()
        } else {
          color::text::PRIMARY
        }),
      }),
  );
  if row.has_attachment {
    subject_row = subject_row.push(attachment_indicator());
  }

  let snippet = text(row.snippet.clone())
    .size(typography::size::SM)
    .wrapping(text::Wrapping::Word)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let mut content = Column::with_children(vec![sender.into(), subject_row.into(), snippet.into()])
    .spacing(spacing::SPACE_2 / 2.0 - 1.0)
    .width(Length::Fill);

  if !row.labels.is_empty() {
    let mut chips = Row::new().spacing(spacing::SPACE_2 / 2.0);
    for label in &row.labels {
      chips = chips.push(label_chip(label));
    }
    content = content.push(container(chips).padding(Padding {
      top: spacing::SPACE_2,
      bottom: 0.0,
      left: 0.0,
      right: 0.0,
    }));
  }

  let avatar = unread_avatar(row);

  let body = Row::with_children(vec![avatar, content.into()])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill);

  let plasma = color::accent::PLASMA;
  let left_border = if selected { plasma } else { iced::Color::TRANSPARENT };

  let row_container = container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_2 * 2.0,
      right: spacing::SPACE_2 * 2.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(if selected {
        color::with_alpha(plasma, 0.08)
      } else {
        iced::Color::TRANSPARENT
      })),
      border: Border {
        color: left_border,
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    });

  let mail_id = row.mail_id;
  mouse_area(row_container).on_press(Message::Selected(mail_id)).into()
}

const AVATAR_SIZE: f32 = 36.0;

fn unread_avatar(row: &MessageRow) -> Element<'_, Message> {
  let avatar = Avatar::new(
    row.sender_id,
    row.sender.clone(),
    Length::Fixed(AVATAR_SIZE),
    AVATAR_SIZE,
    row.sender_portrait.path(),
  )
  .border(color::with_alpha(color::text::PRIMARY, 0.1), 1.0)
  .radius(radius::CONTROL)
  .view::<Message>();

  if !row.is_read {
    let dot =
      container(Space::new().width(Length::Fixed(10.0)).height(Length::Fixed(10.0))).style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          color: color::surface::BASE,
          radius: 5.0.into(),
          width: 2.0,
        },
        ..container::Style::default()
      });
    iced::widget::stack(vec![avatar, dot.into()]).into()
  } else {
    avatar
  }
}

fn label_chip(label: &str) -> Element<'_, Message> {
  chip(label.to_uppercase(), None)
}

fn glyph(symbol: &str) -> Element<'_, Message> {
  text(symbol.to_owned())
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::status::DANGER),
    })
    .into()
}

const INDICATOR_ICON_SIZE: f32 = 14.0;

fn sender_kind_icon<'a>(kind: SenderKind) -> Option<Element<'a, Message>> {
  let icon = match kind {
    SenderKind::Character => return None,
    SenderKind::Corp => Icon::notif_corp(),
    SenderKind::System => Icon::notif_system(),
  };
  Some(icon.size(INDICATOR_ICON_SIZE).color(color::text::secondary()).render())
}

fn importance_flag<'a>() -> Element<'a, Message> {
  text("\u{2691}")
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::status::WARNING),
    })
    .into()
}

fn attachment_indicator<'a>() -> Element<'a, Message> {
  text("\u{1f4ce}")
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into()
}

fn empty_state(query: &str) -> Element<'_, Message> {
  let state = if query.trim().is_empty() {
    shared_empty_state("No messages.")
  } else {
    shared_empty_state("No messages match this search.").subtitle(query)
  };
  state.render()
}

fn right_rule<'a>() -> Element<'a, Message> {
  rule::vertical_fill(0.1)
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone;
  use pretty_assertions::assert_eq;

  use super::*;

  fn row(mail_id: i64, bucket: DayBucket, is_pinned: bool, subject: &str, sender: &str, snippet: &str) -> MessageRow {
    MessageRow {
      bucket,
      character_id: 42,
      is_pinned,
      is_read: false,
      is_starred: false,
      has_attachment: false,
      important: false,
      sender_kind: SenderKind::Character,
      labels: Vec::new(),
      mail_id,
      sender: sender.to_owned(),
      sender_id: 95_000_001,
      sender_portrait: images::ImageState::Stale {
        id: 95_000_001,
        kind: images::ImageKind::CharacterPortrait,
      },
      snippet: snippet.to_owned(),
      subject: subject.to_owned(),
      time: "10:00".to_owned(),
    }
  }

  #[test]
  fn it_keeps_a_short_body_as_the_full_snippet_preview() {
    assert_eq!(snippet_preview("Form up at Jita."), "Form up at Jita.");
  }

  #[test]
  fn it_truncates_a_long_snippet_on_a_word_boundary_with_an_ellipsis() {
    let body = "alpha ".repeat(40);

    let preview = snippet_preview(&body);

    assert!(preview.ends_with('\u{2026}'));
    assert!(preview.chars().count() <= SNIPPET_MAX_CHARS + 1);
  }

  #[test]
  fn it_classifies_sender_kind_from_flags() {
    assert_eq!(SenderKind::from_flags(false, false), SenderKind::Character);
    assert_eq!(SenderKind::from_flags(true, false), SenderKind::Corp);
    assert_eq!(SenderKind::from_flags(false, true), SenderKind::System);
    assert_eq!(SenderKind::from_flags(true, true), SenderKind::System);
  }

  #[test]
  fn it_shows_sender_kind_icon_only_for_corp_and_system() {
    assert!(sender_kind_icon(SenderKind::Character).is_none());
    assert!(sender_kind_icon(SenderKind::Corp).is_some());
    assert!(sender_kind_icon(SenderKind::System).is_some());
  }

  #[test]
  fn it_buckets_by_calendar_day() {
    let now = Utc.with_ymd_and_hms(2026, 6, 15, 14, 0, 0).unwrap();

    assert_eq!(day_bucket("2026-06-15T09:00:00Z", now), DayBucket::Today);
    assert_eq!(day_bucket("2026-06-14T23:00:00Z", now), DayBucket::Yesterday);
    assert_eq!(day_bucket("2026-06-10T09:00:00Z", now), DayBucket::Earlier);
    assert_eq!(day_bucket("not-a-date", now), DayBucket::Earlier);
  }

  #[test]
  fn it_formats_the_clock_label() {
    assert_eq!(time_label("2026-06-15T09:07:00Z"), "09:07");
    assert_eq!(time_label("2026-06-15T22:45:00Z"), "22:45");
  }

  #[test]
  fn it_matches_subject_sender_and_snippet_case_insensitively() {
    let r = row(
      1,
      DayBucket::Today,
      false,
      "CTA tonight",
      "Vex Voronova",
      "form up at Jita",
    );

    assert!(matches_query(&r, "cta"));
    assert!(matches_query(&r, "voronova"));
    assert!(matches_query(&r, "jita"));
    assert!(matches_query(&r, ""));
    assert!(!matches_query(&r, "wormhole"));
  }

  #[test]
  fn it_resolves_an_unfetched_sender_portrait_to_a_path_less_stale_state() {
    assert!(super::super::loaders::resolve_sender_portrait(0).path().is_none());
    assert!(super::super::loaders::resolve_sender_portrait(-1).path().is_none());
  }

  mod load {
    use pretty_assertions::assert_eq;

    use super::super::*;
    use crate::store::{
      self,
      model::{
        Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, CharacterMailLabel,
        CharacterMailLabelMembership, Corporation, Gender, OwnerType, Race,
      },
      repo::{character, infra, mail},
    };

    const CHAR: i64 = 42;
    const LABEL: i64 = 5000;

    async fn seed_character(db: &Database, id: i64) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    async fn store_mail(db: &Database, mail_id: i64, from_id: i64, is_read: bool) {
      let header = CharacterMail {
        character_id: CHAR,
        from_id,
        from_name: "Vex Voronova".to_owned(),
        is_read,
        mail_id,
        subject: Some(format!("Subject {mail_id}")),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
        ..Default::default()
      };
      let body = CharacterMailBody {
        body: "<p>Form up.</p>".to_owned(),
        character_id: CHAR,
        mail_id,
      };
      mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
    }

    async fn store_flagged_mail(
      db: &Database,
      mail_id: i64,
      from_id: i64,
      has_attachment: bool,
      important: bool,
      from_corp: bool,
      from_system: bool,
    ) {
      let header = CharacterMail {
        character_id: CHAR,
        from_id,
        from_name: "Vex Voronova".to_owned(),
        is_read: false,
        mail_id,
        subject: Some(format!("Subject {mail_id}")),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
        has_attachment,
        important,
        from_corp,
        from_system,
      };
      let body = CharacterMailBody {
        body: "<p>Form up.</p>".to_owned(),
        character_id: CHAR,
        mail_id,
      };
      mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
    }

    #[tokio::test]
    async fn it_carries_the_header_indicator_flags_onto_the_row() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, CHAR).await;

      store_flagged_mail(&db, 10, 95_000_001, false, false, false, false).await;
      store_flagged_mail(&db, 11, 95_000_001, true, true, false, false).await;
      store_flagged_mail(&db, 12, 95_000_001, false, false, true, false).await;
      store_flagged_mail(&db, 13, 95_000_001, false, false, false, true).await;

      let rows = load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Inbox)).await;
      let by_id = |id: i64| rows.iter().find(|r| r.mail_id == id).unwrap();

      let plain = by_id(10);
      assert!(!plain.has_attachment);
      assert!(!plain.important);
      assert_eq!(plain.sender_kind, SenderKind::Character);

      let flagged = by_id(11);
      assert!(flagged.has_attachment);
      assert!(flagged.important);

      assert_eq!(by_id(12).sender_kind, SenderKind::Corp);
      assert_eq!(by_id(13).sender_kind, SenderKind::System);
    }

    async fn seed_fixtures(db: &Database) {
      seed_character(db, CHAR).await;
      store_mail(db, 1, 95_000_001, false).await;
      store_mail(db, 2, CHAR, true).await;
      store_mail(db, 3, 95_000_001, false).await;
      store_mail(db, 4, 95_000_001, false).await;
      store_mail(db, 5, 95_000_001, false).await;
      store_mail(db, 6, 95_000_001, false).await;

      mail::set_triage(db, CHAR, 3, true, true).await.unwrap();
      mail::assign_folder(db, CHAR, 4, "archive", None, false).await.unwrap();

      mail::replace_labels_for_character(
        db,
        CHAR,
        &[CharacterMailLabel {
          character_id: CHAR,
          color: Some("#fff".to_owned()),
          label_id: LABEL,
          name: "Fleet".to_owned(),
        }],
      )
      .await
      .unwrap();
      mail::replace_membership_for_character(
        db,
        CHAR,
        &[CharacterMailLabelMembership {
          character_id: CHAR,
          label_id: LABEL,
          mail_id: 5,
        }],
      )
      .await
      .unwrap();

      let until = (Utc::now() + Duration::days(1)).to_rfc3339();
      mail::upsert_snoozed_mail(db, CHAR, 6, &until).await.unwrap();
    }

    fn ids(rows: &[MessageRow]) -> Vec<i64> {
      let mut ids: Vec<i64> = rows.iter().map(|r| r.mail_id).collect();
      ids.sort_unstable();
      ids
    }

    #[tokio::test]
    async fn it_derives_the_inbox_excluding_sent_and_archived_and_snoozed() {
      let db = store::open_test().await.unwrap();
      seed_fixtures(&db).await;

      let rows = load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Inbox)).await;

      assert_eq!(ids(&rows), vec![1, 3, 5]);
      assert_eq!(rows.first().unwrap().mail_id, 3);
      assert!(rows.first().unwrap().is_pinned);
    }

    #[tokio::test]
    async fn it_derives_the_sent_folder() {
      let db = store::open_test().await.unwrap();
      seed_fixtures(&db).await;

      let rows = load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Sent)).await;

      assert_eq!(ids(&rows), vec![2]);
    }

    #[tokio::test]
    async fn it_derives_the_overlay_backed_folders() {
      let db = store::open_test().await.unwrap();
      seed_fixtures(&db).await;

      let starred = load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Starred)).await;
      assert_eq!(ids(&starred), vec![3]);

      let archive = load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Archive)).await;
      assert_eq!(ids(&archive), vec![4]);

      let snoozed = load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Snoozed)).await;
      assert_eq!(ids(&snoozed), vec![6]);

      assert!(
        load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Trash))
          .await
          .is_empty()
      );
      assert!(
        load_messages(&db, Scope::Character(CHAR), Folder::Standard(StandardFolder::Drafts))
          .await
          .is_empty()
      );
    }

    #[tokio::test]
    async fn it_derives_a_label_folder_with_resolved_label_names() {
      let db = store::open_test().await.unwrap();
      seed_fixtures(&db).await;

      let rows = load_messages(&db, Scope::Character(CHAR), Folder::Label(LABEL)).await;

      assert_eq!(ids(&rows), vec![5]);
      assert_eq!(rows.first().unwrap().labels, vec!["Fleet".to_owned()]);
    }

    #[tokio::test]
    async fn it_combines_for_the_unified_folder_and_scopes_other_folders_to_the_character() {
      let db = store::open_test().await.unwrap();
      seed_fixtures(&db).await;

      let unified = load_messages(&db, Scope::Character(CHAR), Folder::Unified).await;
      assert_eq!(
        ids(&unified),
        vec![1, 2, 3, 5],
        "the unified folder combines the roster's mail"
      );

      let labelled = load_messages(&db, Scope::Character(CHAR), Folder::Label(LABEL)).await;
      assert_eq!(
        ids(&labelled),
        vec![5],
        "a non-unified folder is scoped to the active character"
      );
    }
  }
}
