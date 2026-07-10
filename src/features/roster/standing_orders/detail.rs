use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use super::{Message, ObjectiveView, PilotRef, ui};
use crate::{
  store::model::{ObjectiveStatus, ObjectiveThreadEntry},
  ui::{
    components::{
      button::{Button, Size},
      eyebrow::eyebrow,
      icon::Icon,
      rule,
    },
    style::{color, radius, spacing, typography},
  },
};

const HEADER_ICON_TILE: f32 = 40.0;
const PILOT_CHIP_SIZE: f32 = 26.0;

pub(super) fn view<'a>(view: &ObjectiveView, roster: &[PilotRef], confirm_delete: bool) -> Element<'a, Message> {
  Column::with_children(vec![
    back_button(),
    header_card(view, roster),
    actions(view, confirm_delete),
    marching_orders(),
    thread_section(view),
  ])
  .spacing(spacing::SPACE_6)
  .width(Length::Fill)
  .into()
}

fn back_button<'a>() -> Element<'a, Message> {
  Button::ghost(t!("standing_orders.detail.back"))
    .size(Size::Sm)
    .icon(Icon::chevron_left())
    .on_press(Message::BackToBoard)
    .into()
}

fn header_card<'a>(view: &ObjectiveView, roster: &[PilotRef]) -> Element<'a, Message> {
  let status = view.status();
  let accent = ui::accent_color(&view.model.accent);

  let kicker = match view.model.horizon.as_deref().filter(|value| !value.trim().is_empty()) {
    Some(horizon) => format!("{} \u{b7} {}", t!("standing_orders.detail.kicker"), horizon),
    None => t!("standing_orders.detail.kicker").into_owned(),
  };

  let heading = Column::with_children(vec![
    eyebrow(&kicker, Some(ui::identity())),
    text(view.model.title.clone())
      .font(typography::body::MEDIUM)
      .size(24.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .width(Length::Fill);

  let head = Row::with_children(vec![
    ui::target_tile(accent, HEADER_ICON_TILE, 22.0),
    heading.into(),
    ui::status_stamp(status),
  ])
  .spacing(spacing::SPACE_3_5)
  .align_y(Vertical::Top);

  let mut children: Vec<Element<'a, Message>> = vec![head.into()];

  if let Some(why) = view.model.why.as_deref().filter(|value| !value.trim().is_empty()) {
    children.push(
      text(why.to_owned())
        .font(typography::body::REGULAR)
        .size(15.5)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    );
  }

  if let Some(target) = view.model.target.as_deref().filter(|value| !value.trim().is_empty()) {
    children.push(done_when(target, accent));
  }

  if !view.pilots.is_empty() {
    children.push(pilots_row(view, roster));
  }

  let content = container(
    Column::with_children(children)
      .spacing(spacing::SPACE_3_5)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 22.0,
    right: 26.0,
    bottom: 22.0,
    left: 24.0,
  });

  let stripe = container(Space::new())
    .width(Length::Fixed(4.0))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(accent)),
      ..container::Style::default()
    });

  container(Row::with_children(vec![stripe.into(), content.into()]))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(accent, 0.4),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn done_when<'a>(target: &str, accent: Color) -> Element<'a, Message> {
  let label = Column::with_children(vec![
    eyebrow(&t!("standing_orders.detail.done_when"), None),
    text(target.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD + 1.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::UNIT / 2.0)
  .width(Length::Fill);

  container(
    Row::with_children(vec![Icon::tack().size(15.0).color(accent).render(), label.into()])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding([10.0, 14.0])
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: radius::NAV_CARD.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn pilots_row<'a>(view: &ObjectiveView, roster: &[PilotRef]) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![eyebrow(&t!("standing_orders.detail.pilots"), None)];
  for id in &view.pilots {
    children.push(ui::pilot_chip(roster, *id, PILOT_CHIP_SIZE));
  }

  Row::with_children(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .into()
}

fn actions<'a>(view: &ObjectiveView, confirm_delete: bool) -> Element<'a, Message> {
  let id = view.model.id;
  let mut children: Vec<Element<'a, Message>> = vec![
    Button::secondary(t!("standing_orders.action.edit"))
      .size(Size::Sm)
      .icon(Icon::pencil())
      .on_press(Message::EditPressed(id))
      .into(),
  ];

  if view.status() == ObjectiveStatus::Active {
    children.push(
      Button::secondary(t!("standing_orders.action.complete"))
        .size(Size::Sm)
        .icon(Icon::check())
        .on_press(Message::Complete(id))
        .into(),
    );
    children.push(
      Button::ghost(t!("standing_orders.action.cancel_objective"))
        .size(Size::Sm)
        .icon(Icon::block())
        .on_press(Message::Cancel(id))
        .into(),
    );
  } else {
    children.push(
      Button::secondary(t!("standing_orders.action.reopen"))
        .size(Size::Sm)
        .icon(Icon::forward())
        .on_press(Message::Reopen(id))
        .into(),
    );
  }

  children.push(Space::new().width(Length::Fill).into());
  children.push(delete_control(id, confirm_delete));

  Row::with_children(children)
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
}

fn delete_control<'a>(id: i64, confirm_delete: bool) -> Element<'a, Message> {
  if !confirm_delete {
    return Button::ghost(t!("standing_orders.action.delete"))
      .size(Size::Sm)
      .icon(Icon::trash())
      .on_press(Message::DeleteRequested)
      .into();
  }

  Row::with_children(vec![
    text(t!("standing_orders.detail.delete_confirm").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
    Button::ghost(t!("standing_orders.action.keep"))
      .size(Size::Sm)
      .on_press(Message::DeleteCancelled)
      .into(),
    Button::danger(t!("standing_orders.action.delete"))
      .size(Size::Sm)
      .on_press(Message::DeleteConfirmed(id))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn marching_orders<'a>() -> Element<'a, Message> {
  let empty = container(
    text(t!("standing_orders.marching.empty").into_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .padding([18.0, 20.0])
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: radius::PANEL.into(),
    },
    ..container::Style::default()
  });

  section(&t!("standing_orders.marching.kicker"), empty.into())
}

fn thread_section<'a>(view: &ObjectiveView) -> Element<'a, Message> {
  let accent = ui::accent_color(&view.model.accent);
  let count = view.thread.len();
  let phrase = if count == 1 {
    t!("standing_orders.thread.count_one", count => count)
  } else {
    t!("standing_orders.thread.count_other", count => count)
  };
  let title = format!("{} \u{b7} {}", t!("standing_orders.thread.kicker"), phrase);

  let body: Element<'a, Message> = if view.thread.is_empty() {
    container(
      text(t!("standing_orders.thread.empty").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fill)
    .padding([22.0, 20.0])
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: radius::PANEL.into(),
      },
      ..container::Style::default()
    })
    .into()
  } else {
    let entries: Vec<Element<'a, Message>> = view.thread.iter().map(|entry| thread_entry(entry, accent)).collect();
    Column::with_children(entries)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .into()
  };

  section(&title, body)
}

fn thread_entry<'a>(entry: &ObjectiveThreadEntry, accent: Color) -> Element<'a, Message> {
  let head = Row::with_children(vec![
    text(ui::human_date(&entry.date))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(ui::source_label(&entry.source_kind).to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom);

  let text_value = entry.text.clone().unwrap_or_default();
  let dot = container(Space::new())
    .width(Length::Fixed(10.0))
    .height(Length::Fixed(10.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: accent,
        width: 2.0,
        radius: 5.0.into(),
      },
      ..container::Style::default()
    });

  let body = Column::with_children(vec![
    head.into(),
    text(text_value)
      .font(typography::body::REGULAR)
      .size(typography::size::MD + 1.5)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ])
  .spacing(spacing::UNIT + 1.0)
  .width(Length::Fill);

  Row::with_children(vec![dot.into(), body.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Top)
    .into()
}

fn section<'a>(kicker: &str, body: Element<'a, Message>) -> Element<'a, Message> {
  let head = Row::with_children(vec![
    eyebrow(kicker, Some(ui::identity())),
    container(rule::horizontal()).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  Column::with_children(vec![head.into(), body])
    .spacing(spacing::SPACE_3)
    .width(Length::Fill)
    .into()
}
