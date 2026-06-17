use std::cmp::Ordering;

use chrono::{DateTime, Duration, Timelike, Utc};
use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, mouse_area, scrollable, text},
};

use super::{
  Folder, Message, Scope, StandardFolder, State,
  loaders::{MessageLabel, strip_html_snippet},
};
use crate::{
  store::{self, Database, images, model::CharacterMail, repo::mail},
  ui::{
    components::{
      avatar::Avatar,
      chip::label_chip,
      empty_state::empty_state as shared_empty_state,
      icon::Icon,
      rule,
      section_header::section_header,
      text_input::TextInput,
      virtual_list::{self, VirtualList, VirtualListConfig},
    },
    style::{color, radius, spacing, typography},
  },
};

/// Rows fetched per page when the inbox/label listing is keyset-paginated.
///
/// Large enough to overfill a tall viewport on the first load, small enough that
/// the per-row body/label/overlay enrichment in [`key_to_row`] stays cheap.
pub(super) const MESSAGE_PAGE_SIZE: i64 = 60;

const SNIPPET_MAX_CHARS: usize = 120;

/// Nominal height of one message row, in pixels.
///
/// Mail rows are content-driven (a two-line subject, optional label chips), and the
/// interleaved day-bucket headers are shorter, so this is only an estimate for the
/// [`VirtualList`] offset math; overscan absorbs the variance.
const ESTIMATED_ROW_HEIGHT: f32 = 88.0;

const PINNED_LABEL: &str = "Pinned";

const AVATAR_SIZE: f32 = 36.0;

const INDICATOR_ICON_SIZE: f32 = 14.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DayBucket {
  Earlier,
  Today,
  Yesterday,
}

impl DayBucket {
  fn label(self) -> &'static str {
    match self {
      DayBucket::Today => "Today",
      DayBucket::Yesterday => "Yesterday",
      DayBucket::Earlier => "Earlier this week",
    }
  }

  fn rank(self) -> u8 {
    match self {
      DayBucket::Today => 0,
      DayBucket::Yesterday => 1,
      DayBucket::Earlier => 2,
    }
  }
}

// Hand-written so the chronological order (Today < Yesterday < Earlier) survives the alphabetical
// variant declaration a derived `Ord` would otherwise key off.
impl Ord for DayBucket {
  fn cmp(&self, other: &Self) -> Ordering {
    self.rank().cmp(&other.rank())
  }
}

impl PartialOrd for DayBucket {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageRow {
  pub bucket: DayBucket,
  pub character_id: i64,
  pub has_attachment: bool,
  pub important: bool,
  pub is_pinned: bool,
  pub is_read: bool,
  pub is_starred: bool,
  pub label_ids: Vec<i64>,
  pub labels: Vec<MessageLabel>,
  pub mail_id: i64,
  pub sender: String,
  pub sender_id: i64,
  pub sender_kind: SenderKind,
  pub sender_portrait: images::ImageState,
  pub snippet: String,
  pub subject: String,
  pub time: String,
  /// The raw RFC3339 send time, kept so the keyset paginator can build a cursor
  /// from the last loaded row (`time` is only the formatted clock label).
  pub timestamp: String,
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

/// The first screen of a folder: the (small, unbounded) pinned head plus the
/// first keyset page of the non-pinned tail.
///
/// Inbox and label folders are keyset-paginated — their backing queries are
/// unbounded and a large mailbox is the worst offender — so only `MESSAGE_PAGE_SIZE`
/// tail rows are materialized up front; the rest load on scroll. The other folders
/// (sent/drafts/overlay-backed/unified) are inherently bounded and load in one
/// shot with an empty pinned head and no further pages.
pub(super) struct FirstPage {
  pub has_more: bool,
  pub pinned: Vec<MessageRow>,
  pub tail: Vec<MessageRow>,
}

/// One entry in the flattened, windowed message list. Section headers share the
/// flat index space with the rows beneath them so the [`VirtualList`] windows over
/// `[Header, Row, Row, Header, Row, …]` uniformly.
enum ListItem<'a> {
  Header(&'a str),
  Row(&'a MessageRow),
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

pub(super) async fn load_first_page(db: &Database, scope: Scope, folder: Folder) -> FirstPage {
  let now = Utc::now();
  let now_iso = now.to_rfc3339();
  let Scope::Character(id) = scope;

  if let Some(label_id) = paginated_label(folder) {
    let mut pinned_headers = mail::pinned_visible_headers(db, id, &now_iso, label_id).await;
    let tail_headers = match label_id {
      Some(label_id) => mail::visible_headers_for_label_page(db, id, label_id, &now_iso, None, MESSAGE_PAGE_SIZE).await,
      None => {
        // The inbox hides a character's own sent mail from both the pinned head and
        // the tail (it lives in the Sent folder).
        pinned_headers = filter_inbox_self_sent(pinned_headers, id);
        let visible = mail::visible_headers_page(db, id, &now_iso, None, MESSAGE_PAGE_SIZE).await;
        filter_inbox_self_sent(visible, id)
      }
    };
    let pinned = headers_to_rows_owned(db, pinned_headers.unwrap_or_default(), now).await;
    let tail_headers = tail_headers.unwrap_or_default();
    let has_more = tail_headers.len() as i64 == MESSAGE_PAGE_SIZE;
    let tail = headers_to_rows_owned(db, tail_headers, now).await;
    return FirstPage {
      has_more,
      pinned,
      tail,
    };
  }

  // Bounded folders: a single fetch, pinned floated to the top in-Vec as before.
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
  FirstPage {
    has_more: false,
    pinned: Vec::new(),
    tail: rows,
  }
}

/// One more keyset page of the inbox/label tail past `cursor`.
pub(super) async fn load_messages_page(
  db: &Database,
  scope: Scope,
  folder: Folder,
  cursor: mail::MailCursor,
) -> Vec<MessageRow> {
  let now = Utc::now();
  let now_iso = now.to_rfc3339();
  let Scope::Character(id) = scope;
  let Some(label_id) = paginated_label(folder) else {
    return Vec::new();
  };
  let headers = match label_id {
    Some(label_id) => {
      mail::visible_headers_for_label_page(db, id, label_id, &now_iso, Some(&cursor), MESSAGE_PAGE_SIZE).await
    }
    None => {
      let visible = mail::visible_headers_page(db, id, &now_iso, Some(&cursor), MESSAGE_PAGE_SIZE).await;
      filter_inbox_self_sent(visible, id)
    }
  };
  headers_to_rows_owned(db, headers.unwrap_or_default(), now).await
}

/// One keyset page of search hits (subject/sender match) past `cursor`.
pub(super) async fn load_search_page(
  db: &Database,
  scope: Scope,
  folder: Folder,
  needle: &str,
  cursor: Option<mail::MailCursor>,
) -> Vec<MessageRow> {
  let now = Utc::now();
  let now_iso = now.to_rfc3339();
  let Scope::Character(id) = scope;

  // The unified folder searches the roster-wide view; a label folder scopes to that
  // label; every other folder searches the active character's visible mailbox
  // (matching the prior in-memory behaviour).
  if matches!(folder, Folder::Unified) {
    let hits = mail::search_visible_unified_page(db, &now_iso, needle, cursor.as_ref(), MESSAGE_PAGE_SIZE)
      .await
      .unwrap_or_default();
    let keys = hits.into_iter().map(unified_to_key).collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(keys.len());
    for key in keys {
      rows.push(key_to_row(db, key, now).await);
    }
    return rows;
  }

  let label_id = match folder {
    Folder::Label(label_id) => Some(label_id),
    _ => None,
  };
  let headers =
    mail::search_visible_headers_page(db, id, &now_iso, needle, label_id, cursor.as_ref(), MESSAGE_PAGE_SIZE)
      .await
      .unwrap_or_default();
  headers_to_rows_owned(db, headers, now).await
}

fn unified_to_key(m: crate::store::model::character_mail_view::UnifiedMail) -> MailKey {
  MailKey {
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
  }
}

/// The label id to paginate by, or `None` for the inbox path. Returns `None`
/// (i.e. "not a paginated folder") for the bounded folders.
///
/// Only the single-character inbox and label folders are keyset-paginated; the
/// unified folder merges across the roster via a different view and stays a single
/// bounded fetch, as do sent/drafts and the overlay-backed folders.
fn paginated_label(folder: Folder) -> Option<Option<i64>> {
  match folder {
    Folder::Standard(StandardFolder::Inbox) => Some(None),
    Folder::Label(label_id) => Some(Some(label_id)),
    _ => None,
  }
}

/// Inbox hides a character's own sent mail (it lives in the Sent folder instead).
fn filter_inbox_self_sent(
  visible: Result<Vec<CharacterMail>, store::Error>,
  character_id: i64,
) -> Result<Vec<CharacterMail>, store::Error> {
  visible.map(|headers| headers.into_iter().filter(|h| h.from_id() != character_id).collect())
}

async fn headers_to_rows_owned(db: &Database, headers: Vec<CharacterMail>, now: DateTime<Utc>) -> Vec<MessageRow> {
  let mut rows = Vec::with_capacity(headers.len());
  for header in headers {
    rows.push(key_to_row(db, header_to_key(header), now).await);
  }
  rows
}

fn header_to_key(h: CharacterMail) -> MailKey {
  MailKey {
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
  }
}

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
  let labels = super::loaders::resolve_message_labels(db, key.character_id, &label_ids).await;
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
    label_ids,
    labels,
    mail_id: key.mail_id,
    sender: key.sender,
    sender_id: key.sender_id,
    sender_kind: key.sender_kind,
    sender_portrait,
    snippet,
    subject: subject_or_no_subject(&key.subject),
    time: time_label(&key.timestamp),
    timestamp: key.timestamp,
  }
}

async fn unified_keys(db: &Database, now: &str) -> Vec<MailKey> {
  mail::visible_unified(db, now)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(unified_to_key)
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

pub(super) fn pane(state: &State, width: f32) -> Element<'_, Message> {
  let flat = flatten(state);

  let body: Element<'_, Message> = if flat.is_empty() {
    empty_state(state.search())
  } else {
    let offset = state.list_scroll_offset();
    virtual_list::responsive_window(move |viewport_height| {
      let config = VirtualListConfig::new(flat.len(), ESTIMATED_ROW_HEIGHT)
        .viewport_height(viewport_height)
        .scroll_offset(offset);
      let windowed = VirtualList::new(config, |index| match &flat[index] {
        ListItem::Header(label) => day_header(label),
        ListItem::Row(row) => message_row(row, state.selected() == Some(row.mail_id)),
      })
      .view();
      scrollable(windowed)
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_scroll(|viewport| Message::ListScrolled {
          absolute: viewport.absolute_offset().y,
          relative: viewport.relative_offset().y,
        })
        .into()
    })
  };

  let tracked = mouse_area(body).on_move(Message::LabelDragMoved);

  let column = Column::with_children(vec![search_box(state.search()), tracked.into()])
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

/// Flatten the current listing into the [`VirtualList`] index space.
///
/// When a search is active the source is the bounded search-results accumulator
/// ([`State::all_messages`]) and there is no pinned head; otherwise the pinned head
/// ([`State::pinned`]) precedes the day-bucketed non-pinned tail
/// ([`State::messages`]). Day-bucket section headers are interleaved as ordinary
/// indexable items so windowing places them at the right offsets.
fn flatten(state: &State) -> Vec<ListItem<'_>> {
  let searching = !state.search().trim().is_empty();
  if searching {
    flatten_rows(&[], state.all_messages())
  } else {
    flatten_rows(state.pinned(), state.messages())
  }
}

/// Interleave the pinned head and the day-bucketed tail into the flat index space.
///
/// A `Pinned` section header precedes the head when it is non-empty, then each
/// non-empty day bucket emits its header followed by its rows. Buckets are visited
/// in fixed [`DayBucket`] order so the visual grouping matches the listing.
fn flatten_rows<'a>(pinned: &'a [MessageRow], tail: &'a [MessageRow]) -> Vec<ListItem<'a>> {
  let mut items = Vec::with_capacity(pinned.len() + tail.len() + 4);

  if !pinned.is_empty() {
    items.push(ListItem::Header(PINNED_LABEL));
    items.extend(pinned.iter().map(ListItem::Row));
  }

  for bucket in [DayBucket::Today, DayBucket::Yesterday, DayBucket::Earlier] {
    let mut pushed_header = false;
    for row in tail.iter().filter(|r| r.bucket == bucket) {
      if !pushed_header {
        items.push(ListItem::Header(bucket.label()));
        pushed_header = true;
      }
      items.push(ListItem::Row(row));
    }
  }

  items
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
      chips = chips.push(label_chip::<Message>(&label.name, label.color.as_deref()));
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
  mouse_area(row_container)
    .on_press(Message::Selected(mail_id))
    .on_right_press(Message::LabelRowMenuOpened(mail_id))
    .into()
}

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

fn glyph(symbol: &str) -> Element<'_, Message> {
  text(symbol.to_owned())
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::status::DANGER),
    })
    .into()
}

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
      label_ids: Vec::new(),
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
      timestamp: "2026-06-01T10:00:00Z".to_owned(),
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

  mod message_row {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn full_row(
      mail_id: i64,
      sender_kind: SenderKind,
      is_read: bool,
      is_pinned: bool,
      is_starred: bool,
      important: bool,
      has_attachment: bool,
      labels: &[&str],
    ) -> MessageRow {
      MessageRow {
        bucket: DayBucket::Today,
        character_id: 42,
        has_attachment,
        important,
        is_pinned,
        is_read,
        is_starred,
        label_ids: Vec::new(),
        labels: labels
          .iter()
          .map(|name| MessageLabel {
            color: Some("#ff6600".to_owned()),
            name: (*name).to_owned(),
          })
          .collect(),
        mail_id,
        sender: "Vex Voronova".to_owned(),
        sender_id: 95_000_001,
        sender_kind,
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
        snippet: "Form up at Jita.".to_owned(),
        subject: "CTA tonight".to_owned(),
        time: "10:00".to_owned(),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
      }
    }

    #[test]
    fn it_renders_an_unread_pinned_important_selected_row_with_labels() {
      let row = full_row(
        1,
        SenderKind::Character,
        false,
        true,
        false,
        true,
        true,
        &["Fleet", "Ops"],
      );
      let _el: Element<'_, Message> = super::super::message_row(&row, true);
    }

    #[test]
    fn it_renders_a_read_starred_corp_row_unselected() {
      let row = full_row(2, SenderKind::Corp, true, false, true, false, false, &[]);
      let _el: Element<'_, Message> = super::super::message_row(&row, false);
    }

    #[test]
    fn it_renders_a_read_system_row() {
      let row = full_row(3, SenderKind::System, true, false, false, false, false, &[]);
      let _el: Element<'_, Message> = super::super::message_row(&row, false);
    }
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
  fn it_orders_day_buckets_chronologically_with_today_first() {
    assert!(DayBucket::Today < DayBucket::Yesterday);
    assert!(DayBucket::Yesterday < DayBucket::Earlier);

    let mut buckets = [DayBucket::Earlier, DayBucket::Today, DayBucket::Yesterday];
    buckets.sort();

    assert_eq!(buckets, [DayBucket::Today, DayBucket::Yesterday, DayBucket::Earlier]);
  }

  #[test]
  fn it_formats_the_clock_label() {
    assert_eq!(time_label("2026-06-15T09:07:00Z"), "09:07");
    assert_eq!(time_label("2026-06-15T22:45:00Z"), "22:45");
  }

  mod flatten {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Describe a flat item for assertions: `"#Label"` for a header, the mail id
    /// (as a string) for a row.
    fn describe(item: &ListItem<'_>) -> String {
      match item {
        ListItem::Header(label) => format!("#{label}"),
        ListItem::Row(row) => row.mail_id.to_string(),
      }
    }

    fn shape(items: &[ListItem<'_>]) -> Vec<String> {
      items.iter().map(describe).collect()
    }

    #[test]
    fn it_emits_a_header_only_for_each_non_empty_day_bucket() {
      let tail = vec![
        row(1, DayBucket::Today, false, "a", "s", "x"),
        row(2, DayBucket::Today, false, "b", "s", "x"),
        row(3, DayBucket::Earlier, false, "c", "s", "x"),
      ];

      let flat = flatten_rows(&[], &tail);

      assert_eq!(
        shape(&flat),
        ["#Today", "1", "2", "#Earlier this week", "3"],
        "the empty Yesterday bucket contributes no header"
      );
    }

    #[test]
    fn it_puts_the_pinned_head_ahead_of_the_day_buckets() {
      let pinned = vec![row(9, DayBucket::Earlier, true, "p", "s", "x")];
      let tail = vec![row(1, DayBucket::Today, false, "a", "s", "x")];

      let flat = flatten_rows(&pinned, &tail);

      assert_eq!(
        shape(&flat),
        ["#Pinned", "9", "#Today", "1"],
        "the pinned row keeps its own head and is not re-bucketed by its timestamp"
      );
    }

    #[test]
    fn it_omits_the_pinned_header_when_there_are_no_pins() {
      let tail = vec![row(1, DayBucket::Today, false, "a", "s", "x")];

      let flat = flatten_rows(&[], &tail);

      assert_eq!(shape(&flat), ["#Today", "1"]);
    }

    #[test]
    fn it_is_empty_for_an_empty_listing() {
      assert!(flatten_rows(&[], &[]).is_empty());
    }
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

    /// The full first-page listing as a flat, pinned-first Vec.
    ///
    /// In these small fixtures a page never overflows, so this reconstructs the
    /// whole folder the way the UI sees it (pinned head ahead of the tail) and lets
    /// the folder-derivation assertions stay focused on *which* mail is visible.
    async fn load_messages(db: &Database, scope: Scope, folder: Folder) -> Vec<MessageRow> {
      let page = load_first_page(db, scope, folder).await;
      let mut rows = page.pinned;
      rows.extend(page.tail);
      rows
    }

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
      assert_eq!(
        rows.first().unwrap().labels,
        vec![MessageLabel {
          color: Some("#fff".to_owned()),
          name: "Fleet".to_owned(),
        }]
      );
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
