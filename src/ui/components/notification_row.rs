use iced::{
  Background, Border, Element, Length,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, svg, text},
};

use crate::{
  store::model::{Notification, NotificationKind},
  ui::style::{color, radius, spacing, typography},
};

const CTA_RADIUS: f32 = 999.0;
const TILE_ICON_SIZE: f32 = 20.0;
const TILE_SIZE: f32 = 36.0;

static CALENDAR_ICON: &[u8] = include_bytes!("../../../assets/images/icons/calendar.svg");
static CAPTAINS_LOG_ICON: &[u8] = include_bytes!("../../../assets/images/icons/captains-log.svg");
static INDUSTRY_ICON: &[u8] = include_bytes!("../../../assets/images/icons/industry.svg");
static KILLMAIL_ICON: &[u8] = include_bytes!("../../../assets/images/icons/notif-combat.svg");
static MAIL_ICON: &[u8] = include_bytes!("../../../assets/images/icons/mail.svg");
static MARKET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/notif-market.svg");
static MOON_ICON: &[u8] = include_bytes!("../../../assets/images/icons/moon.svg");
static SKILL_ICON: &[u8] = include_bytes!("../../../assets/images/icons/skills.svg");
static WALLET_ICON: &[u8] = include_bytes!("../../../assets/images/icons/wallet.svg");

pub fn accent(kind: NotificationKind) -> iced::Color {
  match kind {
    NotificationKind::Calendar | NotificationKind::CaptainsLog | NotificationKind::ExtractionScheduled => {
      color::accent()
    }
    NotificationKind::ExtractionCracked
    | NotificationKind::Industry
    | NotificationKind::Outbid
    | NotificationKind::WalletGap => color::status::WARNING,
    NotificationKind::Killmail => color::status::DANGER,
    NotificationKind::Mail | NotificationKind::Skill | NotificationKind::WatchlistTarget => color::accent(),
  }
}

pub fn type_tile<'a, M>(kind: NotificationKind) -> Element<'a, M>
where
  M: 'a,
{
  let tint = accent(kind);
  container(
    svg(svg::Handle::from_memory(icon_for(kind)))
      .width(Length::Fixed(TILE_ICON_SIZE))
      .height(Length::Fixed(TILE_ICON_SIZE))
      .style(move |_, _| svg::Style {
        color: Some(tint),
      }),
  )
  .width(Length::Fixed(TILE_SIZE))
  .height(Length::Fixed(TILE_SIZE))
  .align_x(iced::alignment::Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.13))),
    border: Border {
      color: color::with_alpha(tint, 0.4),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

pub fn kind_label(kind: NotificationKind) -> &'static str {
  match kind {
    NotificationKind::Calendar => "Calendar event",
    NotificationKind::CaptainsLog => "Captain's log",
    NotificationKind::ExtractionCracked => "Moon pop",
    NotificationKind::ExtractionScheduled => "Moon extraction",
    NotificationKind::Industry => "Industry job",
    NotificationKind::Killmail => "Killmail",
    NotificationKind::Mail => "New mail",
    NotificationKind::Outbid => "Outbid",
    NotificationKind::Skill => "Skill complete",
    NotificationKind::WalletGap => "Wallet gap",
    NotificationKind::WatchlistTarget => "Watchlist target",
  }
}

pub fn notification_row<'a, M>(
  notification: &Notification,
  who: &str,
  relative_time: &str,
  unread_dot: bool,
  on_press: M,
) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let kind = notification.kind();
  let tint = accent(kind);
  let is_unread = notification.read_at().is_none();

  let tile = type_tile(kind);

  let label_row = Row::with_children(vec![
    text(kind_label(kind).to_owned())
      .font(typography::mono::SEMIBOLD)
      .size(typography::size::XS)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
    text(relative_time.to_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .align_y(Vertical::Center);

  let mut lines: Vec<Element<'a, M>> = vec![
    label_row.into(),
    text(notification.title().clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if !notification.body().is_empty() {
    lines.push(
      text(notification.body().clone())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
    );
  }
  if !who.is_empty() {
    lines.push(
      text(who.to_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }
  if matches!(kind, NotificationKind::Killmail) {
    lines.push(write_debrief_cta(color::accent()));
  }

  let body = Column::with_children(lines)
    .spacing(spacing::UNIT / 2.0)
    .width(Length::Fill);

  let mut row_children: Vec<Element<'a, M>> = vec![tile, body.into()];
  if unread_dot && is_unread {
    row_children.push(unread_marker());
  }

  let row = Row::with_children(row_children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Top);

  button(row)
    .width(Length::Fill)
    .padding(spacing::SPACE_2_5)
    .on_press(on_press)
    .style(move |_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      let background = if hovered {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05)))
      } else if is_unread {
        Some(Background::Color(color::with_alpha(color::accent(), 0.05)))
      } else {
        None
      };
      button::Style {
        background,
        border: Border {
          radius: radius::CONTROL.into(),
          ..Border::default()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn icon_for(kind: NotificationKind) -> &'static [u8] {
  match kind {
    NotificationKind::Calendar => CALENDAR_ICON,
    NotificationKind::CaptainsLog => CAPTAINS_LOG_ICON,
    NotificationKind::ExtractionCracked | NotificationKind::ExtractionScheduled => MOON_ICON,
    NotificationKind::Industry => INDUSTRY_ICON,
    NotificationKind::Killmail => KILLMAIL_ICON,
    NotificationKind::Mail => MAIL_ICON,
    NotificationKind::Outbid | NotificationKind::WatchlistTarget => MARKET_ICON,
    NotificationKind::Skill => SKILL_ICON,
    NotificationKind::WalletGap => WALLET_ICON,
  }
}

fn write_debrief_cta<'a, M>(tint: iced::Color) -> Element<'a, M>
where
  M: 'a,
{
  let label = format!("{} →", t!("shell.notification.killmail_write_debrief"));
  container(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(tint),
      }),
  )
  .padding([spacing::UNIT / 2.0, spacing::SPACE_2])
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.12))),
    border: Border {
      color: color::with_alpha(tint, 0.4),
      width: 1.0,
      radius: CTA_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn unread_marker<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  const DOT: f32 = 7.0;
  container(
    container(Space::new())
      .width(Length::Fixed(DOT))
      .height(Length::Fixed(DOT))
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent())),
        border: Border {
          radius: (DOT / 2.0).into(),
          ..Border::default()
        },
        ..container::Style::default()
      }),
  )
  .width(Length::Fixed(DOT))
  .height(Length::Fixed(DOT))
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::model::{NotificationDestination, NotificationOwner, NotificationTarget};

  fn sample(kind: NotificationKind, read: bool) -> Notification {
    Notification {
      body: "body text".to_owned(),
      created_at: "2026-06-22T00:00:00+00:00".to_owned(),
      dedup_key: "k".to_owned(),
      id: 1,
      kind,
      owner: NotificationOwner::Character(42),
      read_at: read.then(|| "2026-06-22T01:00:00+00:00".to_owned()),
      target: NotificationTarget {
        character: Some(42),
        destination: NotificationDestination::Skills,
        sub: None,
      },
      title: "title text".to_owned(),
    }
  }

  mod accent {
    use super::*;

    #[test]
    fn it_maps_every_kind_to_a_color() {
      for kind in [
        NotificationKind::Calendar,
        NotificationKind::ExtractionCracked,
        NotificationKind::ExtractionScheduled,
        NotificationKind::Industry,
        NotificationKind::Killmail,
        NotificationKind::Mail,
        NotificationKind::Skill,
      ] {
        let _ = accent(kind);
        let _ = icon_for(kind);
        let _ = kind_label(kind);
        let _tile: Element<'_, ()> = type_tile(kind);
      }
    }
  }

  mod notification_row {
    use super::*;

    #[test]
    fn it_renders_a_read_and_unread_row() {
      let _read: Element<'_, ()> =
        notification_row(&sample(NotificationKind::Skill, true), "Pilot", "2m ago", true, ());
      let _unread: Element<'_, ()> =
        notification_row(&sample(NotificationKind::Killmail, false), "Corp", "now", true, ());
      let _no_dot: Element<'_, ()> = notification_row(&sample(NotificationKind::Mail, false), "", "1h ago", false, ());
    }
  }
}
