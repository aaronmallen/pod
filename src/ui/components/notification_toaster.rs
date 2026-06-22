use iced::{
  Background, Border, Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, svg, text},
};

use crate::{
  store::model::Notification,
  ui::{
    components::notification_row::{accent, kind_label},
    style::{color, radius, shadow, spacing, typography},
  },
};

const ACCENT_BAR_WIDTH: f32 = 3.0;
const DISMISS_SIZE: f32 = 22.0;
const PROGRESS_HEIGHT: f32 = 2.0;
const TOAST_MARGIN: f32 = spacing::SPACE_3_5;
const TOAST_SPACING: f32 = spacing::SPACE_3;
const TOAST_WIDTH: f32 = 360.0;

static CLOSE_ICON: &[u8] = include_bytes!("../../../assets/images/icons/close.svg");

/// One toast's render inputs: the notification it surfaces, the resolved "who" name, and the fraction
/// of its lifetime remaining (1.0 = just enqueued, 0.0 = expired) that drives the life-progress bar.
pub struct ToastView<'a> {
  pub notification: &'a Notification,
  pub progress: f32,
  pub who: &'a str,
}

/// The bottom-right toast host: a full-size transparent layer that bottom-right-anchors a stacked
/// column of toast cards. Returns `None` when there is nothing to show so the caller can skip the
/// Stack layer entirely. `on_activate`/`on_dismiss`/`on_hover` are keyed by notification id.
pub fn toaster<'a, M, A, D, H>(
  toasts: &[ToastView<'a>],
  on_activate: A,
  on_dismiss: D,
  on_hover: H,
) -> Option<Element<'a, M>>
where
  M: Clone + 'a,
  A: Fn(i64) -> M,
  D: Fn(i64) -> M,
  H: Fn(i64, bool) -> M,
{
  if toasts.is_empty() {
    return None;
  }

  let cards: Vec<Element<'a, M>> = toasts
    .iter()
    .map(|toast| {
      card(
        toast,
        on_activate(toast.notification.id()),
        on_dismiss(toast.notification.id()),
        on_hover(toast.notification.id(), true),
        on_hover(toast.notification.id(), false),
      )
    })
    .collect();

  let column = Column::with_children(cards)
    .spacing(TOAST_SPACING)
    .align_x(Horizontal::Right);

  Some(
    container(column)
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(TOAST_MARGIN)
      .align_x(Horizontal::Right)
      .align_y(Vertical::Bottom)
      .into(),
  )
}

fn card<'a, M>(toast: &ToastView<'a>, on_activate: M, on_dismiss: M, on_enter: M, on_exit: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let notification = toast.notification;
  let kind = notification.kind();
  let tint = accent(kind);

  let label = text(kind_label(kind).to_owned())
    .font(typography::mono::SEMIBOLD)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(tint),
    });

  let mut lines: Vec<Element<'a, M>> = vec![
    label.into(),
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
  if !toast.who.is_empty() {
    lines.push(
      text(toast.who.to_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    );
  }

  let body = Column::with_children(lines)
    .spacing(spacing::UNIT / 2.0)
    .width(Length::Fill);

  // The whole body activates (mark read + navigate); only the X dismisses without reading.
  let activate_area = button(body)
    .width(Length::Fill)
    .padding(0)
    .on_press(on_activate)
    .style(|_, _| button::Style {
      background: Some(Background::Color(iced::Color::TRANSPARENT)),
      ..button::Style::default()
    });

  let dismiss = button(
    svg(svg::Handle::from_memory(CLOSE_ICON))
      .width(Length::Fixed(12.0))
      .height(Length::Fixed(12.0))
      .style(|_, _| svg::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .width(Length::Fixed(DISMISS_SIZE))
  .height(Length::Fixed(DISMISS_SIZE))
  .padding(spacing::UNIT)
  .on_press(on_dismiss)
  .style(|_, _| button::Style {
    background: Some(Background::Color(iced::Color::TRANSPARENT)),
    ..button::Style::default()
  });

  let content = Row::with_children(vec![activate_area.into(), dismiss.into()])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Top);

  let filled = (toast.progress.clamp(0.0, 1.0) * 100.0).max(0.0);
  let progress = container(
    container(Space::new())
      .width(Length::FillPortion(filled as u16))
      .height(Length::Fixed(PROGRESS_HEIGHT))
      .style(move |_| container::Style {
        background: Some(Background::Color(color::with_alpha(tint, 0.7))),
        ..container::Style::default()
      }),
  )
  .width(Length::Fill)
  .height(Length::Fixed(PROGRESS_HEIGHT));

  let inner = Column::with_children(vec![
    container(content).width(Length::Fill).padding(spacing::SPACE_3).into(),
    progress.into(),
  ]);

  let card = container(inner)
    .width(Length::Fixed(TOAST_WIDTH))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    });

  // The accent bar mirrors the design's `borderLeft: 3px solid`.
  let with_bar = Row::with_children(vec![
    container(Space::new())
      .width(Length::Fixed(ACCENT_BAR_WIDTH))
      .height(Length::Fill)
      .style(move |_| container::Style {
        background: Some(Background::Color(tint)),
        border: Border {
          radius: radius::SUBTLE.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    card.into(),
  ]);

  mouse_area(container(with_bar).width(Length::Fixed(TOAST_WIDTH + ACCENT_BAR_WIDTH)))
    .on_enter(on_enter)
    .on_exit(on_exit)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::model::{NotificationDestination, NotificationKind, NotificationOwner, NotificationTarget};

  fn sample() -> Notification {
    Notification {
      body: "body".to_owned(),
      created_at: "2026-06-22T00:00:00+00:00".to_owned(),
      dedup_key: "k".to_owned(),
      id: 7,
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

  mod toaster {
    use super::*;

    #[test]
    fn it_renders_nothing_when_empty() {
      let none: Option<Element<'_, ()>> = toaster(&[], |_| (), |_| (), |_, _| ());

      assert!(none.is_none());
    }

    #[test]
    fn it_renders_a_card_per_toast() {
      let notification = sample();
      let toasts = vec![ToastView {
        notification: &notification,
        progress: 0.5,
        who: "Pilot",
      }];

      let some: Option<Element<'_, ()>> = toaster(&toasts, |_| (), |_| (), |_, _| ());

      assert!(some.is_some());
    }
  }
}
