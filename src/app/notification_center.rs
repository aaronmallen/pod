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
