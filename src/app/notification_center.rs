use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum NotificationTab {
  #[default]
  New,
  History,
}

pub(super) fn notifications_panel(app: &App, nav_location: config::NavLocation) -> Element<'_, Message> {
  let unread = app.notifications_unread;
  let new_count = app
    .notifications
    .iter()
    .filter(|notification| notification.read_at().is_none())
    .count();
  let history_count = app.notifications_history.len();
  let total = match app.notifications_tab {
    NotificationTab::New => new_count,
    NotificationTab::History => history_count,
  };

  let header = Row::with_children(vec![
    text(t!("shell.notifications.title"))
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    Space::new().width(Length::Fill).into(),
    mark_all_read_button(unread > 0),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2);

  let tabs = notifications_tab_strip(app.notifications_tab, new_count, history_count);

  let body = notifications_tab_body(app, app.notifications_tab);

  let mut children: Vec<Element<'_, Message>> = vec![header.into(), tabs, rule_line(), body];
  if let Some(footer_label) = notifications_footer_label(app.notifications_tab, total, history_count) {
    children.push(rule_line());
    children.push(
      text(footer_label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  let card = container(
    Column::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fixed(NOTIFICATIONS_PANEL_WIDTH)),
  )
  .width(Length::Fixed(NOTIFICATIONS_PANEL_WIDTH))
  .max_height(NOTIFICATIONS_PANEL_MAX_HEIGHT)
  .padding(spacing::SPACE_3_5)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: iced::Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::PANEL.into(),
    },
    shadow: shadow::CARD,
    ..container::Style::default()
  });

  let align_x = match nav_location {
    config::NavLocation::Left => Horizontal::Left,
    config::NavLocation::Right => Horizontal::Right,
  };
  let (pad_left, pad_right) = match nav_location {
    config::NavLocation::Left => (rail::RAIL_WIDTH + POPOVER_LEFT, POPOVER_LEFT),
    config::NavLocation::Right => (POPOVER_LEFT, rail::RAIL_WIDTH + POPOVER_LEFT),
  };
  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(align_x)
    .align_y(Vertical::Bottom)
    .padding(Padding {
      top: 0.0,
      right: pad_right,
      bottom: POPOVER_BOTTOM_OFFSET,
      left: pad_left,
    })
    .into()
}

pub(super) fn notifications_footer_label(tab: NotificationTab, total: usize, history_count: usize) -> Option<String> {
  match tab {
    NotificationTab::New => (total > 0).then(|| t!("shell.notifications.footer_unread", count => total).into_owned()),
    NotificationTab::History => {
      (history_count > 0).then(|| t!("shell.notifications.footer_total", count => total).into_owned())
    }
  }
}

pub(super) fn mark_all_read_button<'a>(enabled: bool) -> Element<'a, Message> {
  Button::ghost(t!("shell.notifications.mark_all_read").into_owned())
    .size(ButtonSize::Sm)
    .on_press_maybe(enabled.then_some(Message::MarkAllNotificationsRead))
    .into()
}

pub(super) fn static_text(value: std::borrow::Cow<'static, str>) -> &'static str {
  match value {
    std::borrow::Cow::Borrowed(text) => text,
    std::borrow::Cow::Owned(text) => Box::leak(text.into_boxed_str()),
  }
}

pub(super) fn notifications_tab_strip<'a>(
  active: NotificationTab,
  new_count: usize,
  total: usize,
) -> Element<'a, Message> {
  let tabs = vec![
    Tab {
      count: new_count.to_string(),
      icon: None,
      label: static_text(t!("shell.notifications.tab_new")),
      on_press: (active != NotificationTab::New).then_some(Message::SelectNotificationTab(NotificationTab::New)),
      selected: active == NotificationTab::New,
    },
    Tab {
      count: total.to_string(),
      icon: None,
      label: static_text(t!("shell.notifications.tab_history")),
      on_press: (active != NotificationTab::History)
        .then_some(Message::SelectNotificationTab(NotificationTab::History)),
      selected: active == NotificationTab::History,
    },
  ];
  container(tab_select_with(tabs, TabLayout::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(NOTIFICATIONS_TAB_STRIP_HEIGHT))
    .into()
}

pub(super) fn notifications_tab_body(app: &App, active: NotificationTab) -> Element<'_, Message> {
  match active {
    NotificationTab::New => notifications_new_body(app),
    NotificationTab::History => notifications_history_body(app),
  }
}

pub(super) fn notifications_new_body(app: &App) -> Element<'_, Message> {
  let rows: Vec<Element<'_, Message>> = app
    .notifications
    .iter()
    .filter(|notification| notification.read_at().is_none())
    .map(|notification| notification_history_row(app, notification))
    .collect();

  if rows.is_empty() {
    return notifications_empty_state(
      t!("shell.notifications.empty_new_title").into_owned(),
      t!("shell.notifications.empty_new_subtitle").into_owned(),
    );
  }

  scrollable(
    Column::with_children(rows)
      .spacing(spacing::UNIT / 2.0)
      .width(Length::Fill),
  )
  .height(Length::Shrink)
  .into()
}

pub(super) fn notifications_history_body(app: &App) -> Element<'_, Message> {
  if app.notifications_history.is_empty() {
    return notifications_empty_state(
      t!("shell.notifications.empty_history_title").into_owned(),
      t!("shell.notifications.empty_history_subtitle").into_owned(),
    );
  }

  let rows = &app.notifications_history;
  let offset = app.notifications_history_scroll;
  virtual_list::responsive_window(move |viewport_height| {
    let config = VirtualListConfig::new(rows.len(), NOTIFICATIONS_HISTORY_ROW_HEIGHT)
      .viewport_height(viewport_height)
      .scroll_offset(offset);
    let windowed = VirtualList::new(config, |index| notification_history_row(app, &rows[index])).view();
    scrollable(windowed)
      .style(control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::NotificationsHistoryScrolled {
        absolute: viewport.absolute_offset().y,
        relative: viewport.relative_offset().y,
      })
      .into()
  })
}

pub(super) fn notification_history_row<'a>(
  app: &'a App,
  notification: &'a store::model::Notification,
) -> Element<'a, Message> {
  let who = app
    .notification_names
    .get(&notification.owner())
    .map(String::as_str)
    .unwrap_or("");
  let when = relative_time(notification.created_at(), app.now);
  notification_row(
    notification,
    who,
    &when,
    true,
    Message::NotificationActivated(notification.id()),
  )
}

pub(super) fn notifications_empty_state(title: String, subtitle: String) -> Element<'static, Message> {
  container(
    Column::with_children(vec![
      text(title)
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
      text(subtitle)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_4_5)
  .align_x(Horizontal::Center)
  .into()
}

pub(super) fn notifications_toaster(app: &App) -> Option<Element<'_, Message>> {
  let views: Vec<ToastView<'_>> = app
    .toasts
    .iter()
    .map(|toast| ToastView {
      notification: &toast.notification,
      who: toast.who.as_str(),
    })
    .collect();
  toaster(
    &views,
    Message::NotificationActivated,
    Message::ToastDismissed,
    Message::ToastHover,
  )
}

pub(super) fn rule_line<'a>() -> Element<'a, Message> {
  container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

pub(super) fn relative_time(created_at: &str, now: DateTime<Utc>) -> String {
  let Ok(when) = DateTime::parse_from_rfc3339(created_at) else {
    return String::new();
  };
  let secs = (now - when.with_timezone(&Utc)).num_seconds().max(0);
  if secs < 45 {
    "now".to_owned()
  } else if secs < 3_600 {
    format!("{}m", secs / 60)
  } else if secs < 86_400 {
    format!("{}h", secs / 3_600)
  } else {
    format!("{}d", secs / 86_400)
  }
}

pub(super) fn drain_notifications_dirty(app: &mut App) -> Option<Task<Message>> {
  if !app.notifications_dirty {
    return None;
  }
  app.notifications_dirty = false;
  Some(refresh_notifications(app, true))
}

pub(super) fn refresh_notifications(app: &App, run_detectors: bool) -> Task<Message> {
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  let now = app.now;
  let features = feature_flags(app);
  let characters = owned_character_ids(app);
  let corporations = owned_corporation_ids(app);
  Task::perform(
    async move { Box::new(notifications::refresh(&db, now, &characters, &corporations, &features, run_detectors).await) },
    Message::NotificationsRefreshed,
  )
}

pub(super) async fn emit_captains_log_reminder(
  db: store::Database,
  date: String,
  character_ids: Vec<i64>,
) -> Option<store::model::Notification> {
  use crate::features::roster::captains_log::{prompts, rollup};

  if character_ids.is_empty() {
    return None;
  }

  let day = rollup::for_date(&db, &date).await.ok()?;
  let activity = captains_log_day_activity(&day);
  let completeness = prompts::completeness_for_day(&db, &date, &character_ids, &activity)
    .await
    .ok()?;
  if completeness.is_complete() {
    return None;
  }

  let reminder = store::model::NewNotification {
    body: t!("shell.notification.captains_log_body").into_owned(),
    dedup_key: format!("captains_log:{date}"),
    kind: store::model::NotificationKind::CaptainsLog,
    owner: store::model::NotificationOwner::Character(character_ids[0]),
    target: store::model::NotificationTarget {
      character: None,
      destination: store::model::NotificationDestination::CaptainsLog,
      sub: None,
    },
    title: t!("shell.notification.captains_log_title").into_owned(),
  };

  store::repo::notifications::emit(&db, &reminder).await.ok().flatten()
}

fn captains_log_day_activity(
  day: &crate::features::roster::captains_log::rollup::DayRollup,
) -> crate::features::roster::captains_log::prompts::DayActivity {
  use crate::features::roster::captains_log::prompts::{DayActivity, LossEngagement};

  let losses = day
    .combat
    .engagements
    .iter()
    .filter(|kill| !kill.is_kill)
    .map(|kill| LossEngagement {
      character_id: kill.character_id,
      killmail_id: kill.killmail_id,
    })
    .collect();

  DayActivity {
    engagement_count: day.combat.engagements.len() as u32,
    industry_count: day.industry.len() as u32,
    losses,
    skill_count: day.skills.len() as u32,
  }
}

pub(super) fn handle_captains_log_reminded(
  app: &mut App,
  emitted: Option<Box<store::model::Notification>>,
) -> Task<Message> {
  match emitted {
    Some(notification) => {
      enqueue_toast(app, *notification);
      refresh_notifications(app, false)
    }
    None => Task::none(),
  }
}

pub(super) fn is_notification_source(kind: JobKind) -> bool {
  matches!(
    kind,
    JobKind::CharacterCalendar
      | JobKind::CharacterIndustryJobs
      | JobKind::CharacterKillmails
      | JobKind::CharacterMail
      | JobKind::CharacterSkills
      | JobKind::CorporationIndustryJobs
      | JobKind::CorporationKillmails
      | JobKind::CorporationMiningExtractions
  )
}

pub(super) fn handle_notifications_refreshed(app: &mut App, snapshot: notifications::Snapshot) -> Task<Message> {
  let notifications::Snapshot {
    list,
    surfaced,
    unread,
    who,
  } = snapshot;
  let newest_changed = list.first().map(store::model::Notification::id)
    != app.notifications_history.first().map(store::model::Notification::id);
  app.notifications = list;
  app.notification_names = who;
  app.notifications_unread = unread;
  for notification in surfaced {
    enqueue_toast(app, notification);
  }
  if app.notifications_panel_open && newest_changed && !app.notifications_history.is_empty() {
    return reset_notifications_history(app);
  }
  Task::none()
}

pub(super) fn reset_notifications_history(app: &mut App) -> Task<Message> {
  app.notifications_history.clear();
  app.notifications_history_cursor = None;
  app.notifications_history_has_more = true;
  app.notifications_history_loading = false;
  app.notifications_history_scroll = 0.0;
  app.notifications_history_epoch = app.notifications_history_epoch.wrapping_add(1);
  load_more_notifications_history(app)
}

pub(super) fn load_more_notifications_history(app: &mut App) -> Task<Message> {
  if app.notifications_history_loading || !app.notifications_history_has_more {
    return Task::none();
  }
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  app.notifications_history_loading = true;
  let db = runtime.db.clone();
  let cursor = app.notifications_history_cursor.clone();
  let epoch = app.notifications_history_epoch;
  Task::perform(
    async move {
      let rows =
        store::repo::notifications::list_page(&db, cursor.as_ref(), store::repo::notifications::HISTORY_PAGE_SIZE)
          .await
          .unwrap_or_default();
      let who = notifications::resolve_names(&db, &rows).await;
      (rows, who)
    },
    move |(rows, who)| Message::NotificationsHistoryPageLoaded {
      epoch,
      rows,
      who,
    },
  )
}

pub(super) fn handle_notifications_history_page_loaded(
  app: &mut App,
  epoch: u64,
  rows: Vec<store::model::Notification>,
  who: std::collections::HashMap<store::model::NotificationOwner, String>,
) -> Task<Message> {
  if epoch != app.notifications_history_epoch {
    return Task::none();
  }
  app.notifications_history_loading = false;
  app.notifications_history_has_more = rows.len() as i64 == store::repo::notifications::HISTORY_PAGE_SIZE;
  if let Some(cursor) = store::model::HistoryCursor::from_page(&rows) {
    app.notifications_history_cursor = Some(cursor);
  }
  app.notification_names.extend(who);
  app.notifications_history.extend(rows);
  Task::none()
}

pub(super) fn handle_notifications_history_scrolled(app: &mut App, absolute: f32, relative: f32) -> Task<Message> {
  app.notifications_history_scroll = absolute;
  if relative < NOTIFICATIONS_HISTORY_SCROLL_THRESHOLD {
    return Task::none();
  }
  load_more_notifications_history(app)
}

pub(super) fn enqueue_toast(app: &mut App, notification: store::model::Notification) {
  let who = app
    .notification_names
    .get(&notification.owner())
    .cloned()
    .unwrap_or_default();
  app.toasts.push(ToastEntry {
    notification,
    paused: false,
    remaining: TOAST_MS,
    who,
  });
  let overflow = app.toasts.len().saturating_sub(TOAST_CAP);
  if overflow > 0 {
    app.toasts.drain(0..overflow);
  }
}

pub(super) fn handle_toggle_notifications_panel(app: &mut App) -> Task<Message> {
  app.notifications_panel_open = !app.notifications_panel_open;
  if app.notifications_panel_open {
    Task::batch([refresh_notifications(app, false), reset_notifications_history(app)])
  } else {
    Task::none()
  }
}

pub(super) fn handle_close_notifications_panel(app: &mut App) -> Task<Message> {
  app.notifications_panel_open = false;
  app.notifications_history.clear();
  app.notifications_history_cursor = None;
  app.notifications_history_has_more = false;
  app.notifications_history_loading = false;
  app.notifications_history_scroll = 0.0;
  app.notifications_history_epoch = app.notifications_history_epoch.wrapping_add(1);
  Task::none()
}

pub(super) fn handle_select_notification_tab(app: &mut App, tab: NotificationTab) -> Task<Message> {
  app.notifications_tab = tab;
  Task::none()
}

pub(super) fn handle_mark_all_notifications_read(app: &mut App) -> Task<Message> {
  // Stamp read_at on the cached rows (and zero the badge) before touching the DB, mirroring the
  // single-row mark path. Without this the New tab — which filters on read_at.is_none() — keeps
  // showing every row until a full refresh, even though the persisted state is read.
  let stamped = app.now.to_rfc3339();
  for notification in &mut app.notifications {
    if notification.read_at().is_none() {
      notification.read_at = Some(stamped.clone());
    }
  }
  // Stamp in-memory rather than reloading from DB to preserve the History tab's scroll and paging position.
  for notification in &mut app.notifications_history {
    if notification.read_at().is_none() {
      notification.read_at = Some(stamped.clone());
    }
  }
  app.notifications_unread = 0;
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  Task::perform(
    async move {
      let _ = store::repo::notifications::mark_all_read(&db).await;
    },
    |()| Message::ToastTick,
  )
}

pub(super) fn handle_notification_activated(app: &mut App, id: i64) -> Task<Message> {
  let target = app
    .notifications
    .iter()
    .find(|notification| notification.id() == id)
    .map(|notification| notification.target().clone());
  app.notifications_panel_open = false;
  app.toasts.retain(|toast| toast.notification.id() != id);
  let read = mark_notification_read(app, id);
  match target {
    Some(target) => Task::batch([read, navigate_to_notification_target(app, &target)]),
    None => read,
  }
}

pub(super) fn mark_notification_read(app: &mut App, id: i64) -> Task<Message> {
  if let Some(notification) = app.notifications.iter_mut().find(|n| n.id() == id)
    && notification.read_at().is_none()
  {
    notification.read_at = Some(app.now.to_rfc3339());
    app.notifications_unread = app.notifications_unread.saturating_sub(1);
  }
  if let Some(notification) = app.notifications_history.iter_mut().find(|n| n.id() == id)
    && notification.read_at().is_none()
  {
    notification.read_at = Some(app.now.to_rfc3339());
  }
  let Some(runtime) = app.runtime.as_ref() else {
    return Task::none();
  };
  let db = runtime.db.clone();
  Task::perform(
    async move {
      let _ = store::repo::notifications::mark_read(&db, id).await;
    },
    |()| Message::ToastTick,
  )
}

pub(super) fn navigate_to_notification_target(
  app: &mut App,
  target: &store::model::NotificationTarget,
) -> Task<Message> {
  use store::model::NotificationDestination;
  match target.destination {
    NotificationDestination::Assets => navigate_to_assets(app),
    NotificationDestination::Calendar => navigate_to_calendar(app, target.character),
    NotificationDestination::CaptainsLog => navigate_to_captains_log(app),
    NotificationDestination::CharacterDetail => match target.character {
      Some(id) => navigate_to_character_detail(app, id),
      None => handle_nav(app, rail::Destination::Roster),
    },
    NotificationDestination::Industry => navigate_to_industry(app, target.character),
    NotificationDestination::Mail => navigate_to_mail(app, target.character),
    NotificationDestination::Skills => {
      let owned = owned_character_ids(app);
      navigate_to_skills(app, target.character, owned)
    }
    NotificationDestination::Wallet => navigate_to_wallet(app),
  }
}

pub(super) fn handle_toast_tick(app: &mut App) -> Task<Message> {
  app.toasts.retain_mut(|toast| {
    if toast.paused {
      return true;
    }
    toast.remaining = toast.remaining.saturating_sub(TOAST_TICK);
    !toast.remaining.is_zero()
  });
  Task::none()
}

pub(super) fn handle_toast_dismissed(app: &mut App, id: i64) -> Task<Message> {
  app.toasts.retain(|toast| toast.notification.id() != id);
  mark_notification_read(app, id)
}

pub(super) fn handle_toast_hover(app: &mut App, id: i64, hovered: bool) -> Task<Message> {
  if let Some(toast) = app.toasts.iter_mut().find(|toast| toast.notification.id() == id) {
    toast.paused = hovered;
  }
  Task::none()
}

/// App-side mirror of the mail feature's wake flip (the feature module is private). Drops the
/// Snoozed label when the catalog still carries one, restores Inbox membership, and enqueues a
/// `mail.set_labels` outbox row. A no-op write (already in Inbox, never flipped) is skipped so the
/// outbox stays clean.
pub(super) async fn enqueue_wake_label_flip(db: &store::Database, character_id: i64, mail_id: i64) {
  use store::{model::OwnerType, repo::mail};

  let catalog = mail::labels(db, character_id).await.unwrap_or_default();
  let snoozed_id = catalog
    .iter()
    .find(|label| label.name().eq_ignore_ascii_case(SNOOZED_LABEL_NAME))
    .map(|label| label.label_id());

  let previous = mail::membership(db, character_id, mail_id).await.unwrap_or_default();
  let mut labels: Vec<i64> = previous.iter().copied().filter(|id| Some(*id) != snoozed_id).collect();
  if !labels.contains(&INBOX_LABEL_ID) {
    labels.push(INBOX_LABEL_ID);
  }
  if labels == previous {
    return;
  }

  for label_id in &previous {
    if !labels.contains(label_id) {
      let _ = mail::remove_membership(db, character_id, mail_id, *label_id).await;
    }
  }
  for label_id in &labels {
    if !previous.contains(label_id) {
      let _ = mail::add_membership(db, character_id, mail_id, *label_id).await;
    }
  }

  let payload = serde_json::json!({
    "character_id": character_id,
    "labels": labels,
    "mail_id": mail_id,
    "previous": previous,
  });
  let Ok(json) = serde_json::to_string(&payload) else {
    return;
  };
  let dedupe = format!("set_labels:{mail_id}");
  let _ = store::repo::infra::append(
    db,
    OwnerType::Character,
    character_id,
    "mail.set_labels",
    &json,
    Some(&dedupe),
  )
  .await;
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::test_support::*;

  mod notifications {
    use super::*;
    use crate::store::model::{
      Notification, NotificationDestination, NotificationKind, NotificationOwner, NotificationTarget,
    };

    fn notification(id: i64) -> Notification {
      Notification {
        body: "body".to_owned(),
        created_at: "2026-06-22T00:00:00+00:00".to_owned(),
        dedup_key: format!("skill:{id}"),
        id,
        kind: NotificationKind::Skill,
        owner: NotificationOwner::Character(42),
        read_at: None,
        target: NotificationTarget {
          character: Some(42),
          destination: NotificationDestination::Skills,
          sub: None,
        },
        title: "title".to_owned(),
      }
    }

    mod enqueue_toast {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_caps_visible_toasts_and_keeps_the_newest() {
        let mut app = test_app();

        for id in 1..=(TOAST_CAP as i64 + 2) {
          enqueue_toast(&mut app, notification(id));
        }

        assert_eq!(app.toasts.len(), TOAST_CAP);
        let ids: Vec<i64> = app.toasts.iter().map(|toast| toast.notification.id()).collect();
        assert_eq!(ids, vec![3, 4, 5], "the oldest are dropped, the newest are kept");
      }
    }

    mod handle_toast_tick {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_dismisses_a_toast_once_its_lifetime_elapses() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));
        app.toasts[0].remaining = TOAST_TICK;

        let _ = handle_toast_tick(&mut app);

        assert!(app.toasts.is_empty(), "a fully aged toast is removed");
      }

      #[test]
      fn it_leaves_a_paused_toast_untouched() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));
        app.toasts[0].paused = true;
        app.toasts[0].remaining = TOAST_TICK;

        let _ = handle_toast_tick(&mut app);

        assert_eq!(app.toasts.len(), 1, "hover pauses the countdown");
        assert_eq!(app.toasts[0].remaining, TOAST_TICK);
      }
    }

    mod handle_toast_hover {
      use super::*;

      #[test]
      fn it_pauses_and_resumes_the_hovered_toast() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));

        let _ = handle_toast_hover(&mut app, 1, true);
        assert!(app.toasts[0].paused);

        let _ = handle_toast_hover(&mut app, 1, false);
        assert!(!app.toasts[0].paused);
      }
    }

    mod handle_toast_dismissed {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_removes_the_toast_and_marks_the_row_read() {
        let mut app = test_app();
        enqueue_toast(&mut app, notification(1));
        app.notifications = vec![notification(1)];
        app.notifications_unread = 1;

        let _ = handle_toast_dismissed(&mut app, 1);

        assert!(app.toasts.is_empty());
        assert_eq!(app.notifications_unread, 0, "the X marks the dismissed row read");
        assert!(
          app.notifications[0].read_at().is_some(),
          "the row stays in the center as read history"
        );
      }
    }

    mod is_notification_source {
      use super::*;

      #[test]
      fn it_gates_to_the_seven_event_sources() {
        assert!(is_notification_source(JobKind::CharacterMail));
        assert!(is_notification_source(JobKind::CharacterSkills));
        assert!(is_notification_source(JobKind::CorporationMiningExtractions));
        assert!(!is_notification_source(JobKind::CharacterWallet));
        assert!(!is_notification_source(JobKind::MarketPrices));
      }
    }
  }

  mod notification_tabs {
    use pretty_assertions::assert_eq;

    use super::*;

    fn read_notification(id: i64) -> store::model::Notification {
      store::model::Notification {
        read_at: Some(Utc::now().to_rfc3339()),
        ..test_notification(id, store::model::NotificationDestination::Skills)
      }
    }

    fn unread_notification(id: i64) -> store::model::Notification {
      test_notification(id, store::model::NotificationDestination::Skills)
    }

    fn ids(rows: &[store::model::Notification], tab: NotificationTab) -> Vec<i64> {
      rows
        .iter()
        .filter(|notification| match tab {
          NotificationTab::New => notification.read_at().is_none(),
          NotificationTab::History => true,
        })
        .map(store::model::Notification::id)
        .collect()
    }

    #[test]
    fn it_filters_the_new_tab_to_unread_and_history_to_all() {
      let mut app = ready_app();
      app.notifications = vec![unread_notification(1), read_notification(2), unread_notification(3)];

      assert_eq!(
        ids(&app.notifications, NotificationTab::New),
        vec![1, 3],
        "the New tab lists only unread notifications"
      );
      assert_eq!(
        ids(&app.notifications, NotificationTab::History),
        vec![1, 2, 3],
        "the History tab lists every loaded notification"
      );
    }

    #[test]
    fn it_selects_a_tab_as_durable_app_state() {
      let mut app = ready_app();
      assert_eq!(
        app.notifications_tab,
        NotificationTab::New,
        "the panel opens on the New tab"
      );

      let _ = handle_select_notification_tab(&mut app, NotificationTab::History);
      assert_eq!(
        app.notifications_tab,
        NotificationTab::History,
        "selecting History sticks"
      );

      let _ = handle_select_notification_tab(&mut app, NotificationTab::New);
      assert_eq!(app.notifications_tab, NotificationTab::New, "selecting New sticks");
    }

    #[test]
    fn it_empties_the_new_tab_but_retains_history_after_mark_all_read() {
      let marked: Vec<store::model::Notification> = vec![unread_notification(1), unread_notification(2)]
        .into_iter()
        .map(|notification| store::model::Notification {
          read_at: Some(Utc::now().to_rfc3339()),
          ..notification
        })
        .collect();

      assert!(
        ids(&marked, NotificationTab::New).is_empty(),
        "the New tab is emptied once every row is read"
      );
      assert_eq!(
        ids(&marked, NotificationTab::History),
        vec![1, 2],
        "History keeps every notification"
      );
    }

    #[test]
    fn it_drops_read_rows_from_the_new_tab_after_mark_all_read() {
      let mut app = ready_app();
      app.notifications = vec![unread_notification(1), unread_notification(2)];
      app.notifications_unread = 2;

      let _ = handle_mark_all_notifications_read(&mut app);

      assert!(
        ids(&app.notifications, NotificationTab::New).is_empty(),
        "every cached row is marked read, so the New tab is empty"
      );
      assert_eq!(
        ids(&app.notifications, NotificationTab::History),
        vec![1, 2],
        "History still lists every notification after mark-all-read"
      );
      assert_eq!(app.notifications_unread, 0, "the unread badge clears");
    }

    #[test]
    fn it_stamps_read_at_on_the_history_rows_after_mark_all_read() {
      let mut app = ready_app();
      app.notifications = vec![unread_notification(1), unread_notification(2)];
      app.notifications_unread = 2;
      app.notifications_history = vec![unread_notification(1), unread_notification(2)];

      let _ = handle_mark_all_notifications_read(&mut app);

      assert!(
        app
          .notifications_history
          .iter()
          .all(|notification| notification.read_at().is_some()),
        "every loaded History row is stamped read so it renders without the unread dot"
      );
    }

    #[test]
    fn it_stamps_the_matching_history_row_on_a_single_mark_read() {
      let mut app = ready_app();
      app.notifications = vec![unread_notification(1), unread_notification(2)];
      app.notifications_unread = 2;
      app.notifications_history = vec![unread_notification(1), unread_notification(2)];

      let _ = mark_notification_read(&mut app, 1);

      assert!(
        app.notifications_history[0].read_at().is_some(),
        "the matching History row is stamped read"
      );
      assert!(
        app.notifications_history[1].read_at().is_none(),
        "unrelated History rows keep their unread state"
      );
    }

    #[test]
    fn it_shows_a_count_only_footer_without_a_clear_all_control() {
      assert_eq!(
        notifications_footer_label(NotificationTab::New, 3, 9),
        Some("3 unread".to_owned()),
        "the New footer reports the unread count"
      );
      assert_eq!(
        notifications_footer_label(NotificationTab::History, 12, 12),
        Some("12 total".to_owned()),
        "the History footer reports the loaded total"
      );
      assert_eq!(
        notifications_footer_label(NotificationTab::New, 0, 9),
        None,
        "the New footer hides when nothing is unread"
      );
      assert_eq!(
        notifications_footer_label(NotificationTab::History, 0, 0),
        None,
        "the History footer hides when nothing is loaded"
      );
    }

    #[test]
    fn it_renders_both_tabs_with_their_empty_states() {
      let mut app = ready_app();
      app.notifications = vec![read_notification(1)];
      app
        .notification_names
        .insert(store::model::NotificationOwner::Character(1), "Pilot 1".to_owned());

      app.notifications_tab = NotificationTab::New;
      let _ = notifications_panel(&app, config::NavLocation::Left);
      app.notifications_history = vec![read_notification(1)];
      app.notifications_tab = NotificationTab::History;
      let _ = notifications_panel(&app, config::NavLocation::Left);

      app.notifications.clear();
      app.notifications_history.clear();
      let _ = notifications_panel(&app, config::NavLocation::Left);
    }
  }

  mod notifications_history {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    fn history_notification(id: i64, created_at: &str) -> store::model::Notification {
      store::model::Notification {
        created_at: created_at.to_owned(),
        ..test_notification(id, store::model::NotificationDestination::Skills)
      }
    }

    fn full_page() -> Vec<store::model::Notification> {
      (0..store::repo::notifications::HISTORY_PAGE_SIZE)
        .map(|i| history_notification(i, &format!("2026-06-01T00:00:{:02}+00:00", i % 60)))
        .collect()
    }

    #[test]
    fn it_appends_a_page_and_advances_the_cursor() {
      let mut app = ready_app();
      app.notifications_history_epoch = 7;
      app.notifications_history_has_more = true;

      let page = vec![
        history_notification(3, "2026-06-03T00:00:00+00:00"),
        history_notification(2, "2026-06-02T00:00:00+00:00"),
      ];
      let _ = handle_notifications_history_page_loaded(&mut app, 7, page, HashMap::new());

      let ids: Vec<i64> = app
        .notifications_history
        .iter()
        .map(store::model::Notification::id)
        .collect();
      assert_eq!(ids, vec![3, 2], "the page is appended newest-first");
      assert_eq!(
        app.notifications_history_cursor,
        Some(store::model::HistoryCursor {
          created_at: "2026-06-02T00:00:00+00:00".to_owned(),
          id: 2,
        }),
        "the cursor advances to the last row of the page"
      );
      assert!(!app.notifications_history_loading, "the in-flight guard is cleared");
      assert!(
        !app.notifications_history_has_more,
        "a short page means no further pages remain"
      );
    }

    #[test]
    fn it_keeps_paging_while_pages_arrive_full() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;

      let _ = handle_notifications_history_page_loaded(&mut app, 0, full_page(), HashMap::new());

      assert_eq!(
        app.notifications_history.len() as i64,
        store::repo::notifications::HISTORY_PAGE_SIZE
      );
      assert!(
        app.notifications_history_has_more,
        "a full page leaves the door open for another"
      );
    }

    #[test]
    fn it_merges_resolved_who_names_for_the_paged_rows() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;
      let mut who = HashMap::new();
      who.insert(store::model::NotificationOwner::Character(1), "Vex Voronova".to_owned());

      let page = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      let _ = handle_notifications_history_page_loaded(&mut app, 0, page, who);

      assert_eq!(
        app
          .notification_names
          .get(&store::model::NotificationOwner::Character(1))
          .map(String::as_str),
        Some("Vex Voronova"),
        "the paged rows' author names are merged in"
      );
    }

    #[test]
    fn it_drops_a_page_captured_against_a_stale_epoch() {
      let mut app = ready_app();
      app.notifications_history_epoch = 5;
      app.notifications_history_loading = true;
      app.notifications_history_has_more = true;

      let page = vec![history_notification(9, "2026-06-09T00:00:00+00:00")];
      let _ = handle_notifications_history_page_loaded(&mut app, 4, page, HashMap::new());

      assert!(
        app.notifications_history.is_empty(),
        "a stale-epoch page is discarded, not appended"
      );
      assert!(
        app.notifications_history_loading,
        "the stale page does not clear the live in-flight guard"
      );
    }

    #[test]
    fn it_requests_a_page_only_past_the_scroll_threshold() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;
      app
        .notifications_history
        .push(history_notification(1, "2026-06-01T00:00:00+00:00"));

      let _ = handle_notifications_history_scrolled(&mut app, 120.0, 0.10);
      assert_eq!(app.notifications_history_scroll, 120.0, "the offset is tracked");
      assert!(!app.notifications_history_loading, "a shallow scroll triggers no fetch");
    }

    #[test]
    fn it_does_not_over_fetch_while_a_page_is_in_flight() {
      let mut app = ready_app();
      app.notifications_history_has_more = true;
      app.notifications_history_loading = true;

      let task = load_more_notifications_history(&mut app);

      assert!(app.notifications_history_loading);
      let _ = task;
    }

    #[test]
    fn it_does_not_fetch_once_the_last_page_is_reached() {
      let mut app = ready_app();
      app.notifications_history_has_more = false;
      app.notifications_history_loading = false;

      let _ = handle_notifications_history_scrolled(&mut app, 999.0, 0.99);

      assert!(
        !app.notifications_history_loading,
        "no fetch starts once has_more is false"
      );
    }

    #[test]
    fn it_resets_the_accumulator_and_bumps_the_epoch() {
      let mut app = ready_app();
      app.notifications_history = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      app.notifications_history_cursor = Some(store::model::HistoryCursor {
        created_at: "2026-06-01T00:00:00+00:00".to_owned(),
        id: 1,
      });
      app.notifications_history_scroll = 500.0;
      let before = app.notifications_history_epoch;

      let _ = reset_notifications_history(&mut app);

      assert!(app.notifications_history.is_empty(), "the accumulator clears");
      assert_eq!(
        app.notifications_history_cursor, None,
        "the cursor rewinds to the newest page"
      );
      assert_eq!(app.notifications_history_scroll, 0.0, "the scroll offset rewinds");
      assert_eq!(
        app.notifications_history_epoch,
        before.wrapping_add(1),
        "the epoch bumps so in-flight pages are invalidated"
      );
    }

    #[test]
    fn it_resets_history_when_a_refresh_brings_a_newer_head_row() {
      let mut app = ready_app();
      app.notifications_panel_open = true;
      app.notifications_history = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      let before = app.notifications_history_epoch;

      let snapshot = crate::features::shell::notifications::Snapshot {
        list: vec![history_notification(2, "2026-06-02T00:00:00+00:00")],
        surfaced: Vec::new(),
        unread: 1,
        who: HashMap::new(),
      };
      let _ = handle_notifications_refreshed(&mut app, snapshot);

      assert!(
        app.notifications_history.is_empty(),
        "History rewinds to the first page"
      );
      assert_eq!(
        app.notifications_history_epoch,
        before.wrapping_add(1),
        "the reset bumps the epoch"
      );
    }

    #[test]
    fn it_leaves_history_intact_when_a_refresh_brings_no_newer_head() {
      let mut app = ready_app();
      app.notifications_panel_open = true;
      app.notifications_history = vec![history_notification(2, "2026-06-02T00:00:00+00:00")];
      let before = app.notifications_history_epoch;

      let snapshot = crate::features::shell::notifications::Snapshot {
        list: vec![history_notification(2, "2026-06-02T00:00:00+00:00")],
        surfaced: Vec::new(),
        unread: 0,
        who: HashMap::new(),
      };
      let _ = handle_notifications_refreshed(&mut app, snapshot);

      assert_eq!(app.notifications_history.len(), 1, "History is untouched");
      assert_eq!(app.notifications_history_epoch, before, "the epoch is unchanged");
    }

    #[test]
    fn it_clears_history_state_on_panel_close() {
      let mut app = ready_app();
      app.notifications_panel_open = true;
      app.notifications_history = vec![history_notification(1, "2026-06-01T00:00:00+00:00")];
      app.notifications_history_has_more = true;
      let before = app.notifications_history_epoch;

      let _ = handle_close_notifications_panel(&mut app);

      assert!(!app.notifications_panel_open);
      assert!(app.notifications_history.is_empty(), "closing drops the accumulator");
      assert!(!app.notifications_history_has_more);
      assert_eq!(
        app.notifications_history_epoch,
        before.wrapping_add(1),
        "closing invalidates any in-flight page"
      );
    }
  }

  mod navigate_to_notification_target {
    use pretty_assertions::assert_eq;
    use store::model::{NotificationDestination, NotificationTarget};

    use super::*;

    fn target(destination: NotificationDestination, character: Option<i64>) -> NotificationTarget {
      NotificationTarget {
        character,
        destination,
        sub: None,
      }
    }

    #[test]
    fn it_routes_every_destination_to_its_route() {
      let mut app = ready_app();

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Assets, None));
      assert_eq!(app.route, Route::Assets);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Calendar, Some(1)));
      assert_eq!(app.route, Route::Calendar);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::CaptainsLog, None));
      assert_eq!(app.route, Route::CaptainsLog);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::CharacterDetail, Some(9)));
      assert_eq!(app.route, Route::CharacterDetail(9));

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Industry, Some(1)));
      assert_eq!(app.route, Route::Industry);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Mail, Some(1)));
      assert_eq!(app.route, Route::Mail);

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Skills, Some(1)));
      assert_eq!(app.route, Route::Skills(1));

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::Wallet, None));
      assert_eq!(app.route, Route::Wallet);
    }

    #[test]
    fn it_lands_a_character_less_character_detail_on_the_roster() {
      let mut app = ready_app();

      let _ = navigate_to_notification_target(&mut app, &target(NotificationDestination::CharacterDetail, None));

      assert_eq!(app.route, Route::Roster);
    }
  }

  mod handle_notification_activated {
    use super::*;

    #[test]
    fn it_marks_read_clears_the_toast_and_navigates_to_the_target() {
      let mut app = ready_app();
      app.notifications_unread = 1;
      app.notifications_panel_open = true;
      app
        .notifications
        .push(test_notification(5, store::model::NotificationDestination::Wallet));
      app.toasts.push(ToastEntry {
        notification: test_notification(5, store::model::NotificationDestination::Wallet),
        paused: false,
        remaining: TOAST_MS,
        who: String::new(),
      });

      let _ = handle_notification_activated(&mut app, 5);

      assert!(!app.notifications_panel_open, "the panel closes");
      assert!(app.toasts.is_empty(), "the matching toast is removed");
      assert_eq!(app.notifications_unread, 0, "the row is marked read");
      assert_eq!(app.route, Route::Wallet, "it navigates to the target");
      assert!(
        app.notifications[0].read_at().is_some(),
        "the activated row carries a read timestamp"
      );
    }

    #[test]
    fn it_only_marks_read_when_the_id_is_unknown() {
      let mut app = ready_app();
      app.route = Route::Roster;
      app.notifications_panel_open = true;

      let _ = handle_notification_activated(&mut app, 999);

      assert!(!app.notifications_panel_open);
      assert_eq!(app.route, Route::Roster, "no target means no navigation");
    }
  }

  mod enqueue_wake_label_flip {
    use super::*;
    use crate::store::{
      Database,
      model::{
        Alliance, Bloodline, Character, CharacterMail, CharacterMailBody, CharacterMailLabel, Corporation, Gender,
        OwnerType, Race,
      },
      repo::mail,
    };

    async fn seed_character(db: &Database, id: i64) {
      use crate::store::repo::character;

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
    }

    async fn store_unread(db: &Database, character_id: i64, mail_id: i64) {
      let header = CharacterMail {
        character_id,
        from_id: 95_000_001,
        from_name: "Sender".to_owned(),
        is_read: false,
        mail_id,
        subject: Some("Subject".to_owned()),
        timestamp: "2026-06-01T10:00:00Z".to_owned(),
        ..Default::default()
      };
      let body = CharacterMailBody {
        body: "<p>hi</p>".to_owned(),
        character_id,
        mail_id,
      };
      mail::upsert_complete(db, &header, &body, &[]).await.unwrap();
    }

    async fn insert_label(db: &Database, character_id: i64, label_id: i64, name: &str) {
      let label = CharacterMailLabel {
        character_id,
        color: None,
        label_id,
        name: name.to_owned(),
      };
      mail::insert_label(db, &label).await.unwrap();
    }

    async fn pending_set_labels(db: &Database) -> i64 {
      sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox WHERE kind = 'mail.set_labels'")
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn it_drops_snoozed_and_restores_inbox_then_enqueues_the_flip() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;
      insert_label(&db, 42, -9, SNOOZED_LABEL_NAME).await;
      mail::add_membership(&db, 42, 7, -9).await.unwrap();

      crate::app::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert!(!membership.contains(&-9), "the snoozed label is dropped");
      assert!(membership.contains(&INBOX_LABEL_ID), "inbox membership is restored");
      assert_eq!(pending_set_labels(&db).await, 1, "a single set_labels row is enqueued");
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_the_mail_is_already_only_in_inbox() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;
      mail::add_membership(&db, 42, 7, INBOX_LABEL_ID).await.unwrap();

      crate::app::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert_eq!(membership, vec![INBOX_LABEL_ID], "membership is unchanged");
      assert_eq!(pending_set_labels(&db).await, 0, "no outbox row is enqueued");
    }

    #[tokio::test]
    async fn it_adds_inbox_when_a_mail_carries_no_labels() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;

      crate::app::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert_eq!(membership, vec![INBOX_LABEL_ID], "inbox membership is added");
      assert_eq!(pending_set_labels(&db).await, 1, "a set_labels row is enqueued");
    }

    #[tokio::test]
    async fn it_preserves_unrelated_labels_alongside_inbox() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      store_unread(&db, 42, 7).await;
      insert_label(&db, 42, INBOX_LABEL_ID, "Inbox").await;
      insert_label(&db, 42, 5, "Keep").await;
      insert_label(&db, 42, -9, SNOOZED_LABEL_NAME).await;
      mail::add_membership(&db, 42, 7, 5).await.unwrap();
      mail::add_membership(&db, 42, 7, -9).await.unwrap();

      crate::app::enqueue_wake_label_flip(&db, 42, 7).await;

      let membership = mail::membership(&db, 42, 7).await.unwrap();
      assert!(membership.contains(&5), "the unrelated label is preserved");
      assert!(membership.contains(&INBOX_LABEL_ID), "inbox membership is restored");
      assert!(!membership.contains(&-9), "the snoozed label is dropped");
      let _ = OwnerType::Character;
    }
  }

  mod emit_captains_log_reminder {
    use pretty_assertions::assert_eq;

    use super::*;

    const DATE: &str = "2026-07-06";

    #[tokio::test]
    async fn it_emits_a_reminder_when_todays_log_is_incomplete() {
      let db = store::open_test().await.unwrap();

      let emitted = emit_captains_log_reminder(db, DATE.to_owned(), vec![42]).await;

      let notification = emitted.expect("an incomplete day surfaces a reminder");
      assert_eq!(notification.kind(), store::model::NotificationKind::CaptainsLog);
      assert_eq!(notification.dedup_key(), "captains_log:2026-07-06");
      assert_eq!(
        notification.target().destination,
        store::model::NotificationDestination::CaptainsLog,
        "clicking the reminder routes to the Captain's Log"
      );
    }

    #[tokio::test]
    async fn it_skips_the_reminder_once_the_log_is_complete() {
      let db = store::open_test().await.unwrap();
      store::repo::captains_log::upsert_answer(
        &db,
        DATE,
        store::repo::captains_log::AnswerKey::Goal,
        Some("Rat a few anoms in Delve."),
      )
      .await
      .unwrap();

      let emitted = emit_captains_log_reminder(db, DATE.to_owned(), vec![42]).await;

      assert!(emitted.is_none(), "a complete day raises no reminder");
    }

    #[tokio::test]
    async fn it_swallows_a_duplicate_same_day_emit() {
      let db = store::open_test().await.unwrap();

      let first = emit_captains_log_reminder(db.clone(), DATE.to_owned(), vec![42]).await;
      let second = emit_captains_log_reminder(db, DATE.to_owned(), vec![42]).await;

      assert!(first.is_some(), "the first emit surfaces the reminder");
      assert!(second.is_none(), "the same-day dedup_key drops the duplicate");
    }

    #[tokio::test]
    async fn it_does_nothing_without_any_owned_characters() {
      let db = store::open_test().await.unwrap();

      let emitted = emit_captains_log_reminder(db, DATE.to_owned(), Vec::new()).await;

      assert!(emitted.is_none(), "no owned characters means no reminder");
    }
  }
}
