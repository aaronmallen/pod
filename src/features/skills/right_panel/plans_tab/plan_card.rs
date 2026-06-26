use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, text},
};

use super::{Message, PlanRow};
use crate::ui::{
  components::{chip::chip, rule},
  style::{color, radius, spacing, typography},
};

pub fn plan_card<'a>(plan: &'a PlanRow, first: bool, confirm: bool) -> Element<'a, Message> {
  let badge = badge(plan.remaining_steps);
  let info = info_col(&plan.name, plan.distinct_skills, &plan.updated);
  let actions: Element<'a, Message> = if confirm {
    confirm_row(plan.id)
  } else {
    action_row(plan.id)
  };

  let content = container(
    Row::with_children(vec![
      badge,
      Space::new().width(Length::Fixed(14.0)).into(),
      info,
      actions,
    ])
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .width(Length::Fill);

  if first {
    content.into()
  } else {
    Column::with_children(vec![rule::horizontal(), content.into()]).into()
  }
}

fn info_col<'a>(name: &'a str, distinct_skills: usize, updated: &'a str) -> Element<'a, Message> {
  let noun = if distinct_skills == 1 { "skill" } else { "skills" };
  let subtitle = format!("{} {} \u{00b7} {}", distinct_skills, noun, updated);

  Column::with_children(vec![
    text(name.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(subtitle)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill)
  .into()
}

fn badge<'a>(remaining_steps: usize) -> Element<'a, Message> {
  chip(remaining_steps.to_string(), Some(color::accent::PLASMA))
}

fn action_row<'a>(plan_id: i64) -> Element<'a, Message> {
  Row::with_children(vec![
    open_btn(plan_id),
    Space::new().width(Length::Fixed(spacing::SPACE_2)).into(),
    delete_btn(plan_id),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn confirm_row<'a>(plan_id: i64) -> Element<'a, Message> {
  Row::with_children(vec![
    text("Delete?")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().width(Length::Fixed(spacing::SPACE_2)).into(),
    confirm_delete_btn(plan_id),
    Space::new().width(Length::Fixed(spacing::UNIT)).into(),
    cancel_delete_btn(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn open_btn<'a>(plan_id: i64) -> Element<'a, Message> {
  button(
    text("Open")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::OpenPlan(plan_id))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: if active {
          color::accent::PLASMA
        } else {
          color::with_alpha(color::text::PRIMARY, 0.1)
        },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if active {
        color::accent::PLASMA
      } else {
        color::text::PRIMARY
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn delete_btn<'a>(plan_id: i64) -> Element<'a, Message> {
  button(
    text("\u{00d7}")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .on_press(Message::DeleteRequested(plan_id))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: if active {
          color::status::DANGER
        } else {
          Color::TRANSPARENT
        },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if active {
        color::status::DANGER
      } else {
        color::text::tertiary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn confirm_delete_btn<'a>(plan_id: i64) -> Element<'a, Message> {
  button(
    text("Confirm")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .on_press(Message::DeleteConfirmed(plan_id))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: active.then(|| Background::Color(color::with_alpha(color::status::DANGER, 0.12))),
      border: Border {
        color: color::status::DANGER,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::status::DANGER,
      ..button::Style::default()
    }
  })
  .into()
}

fn cancel_delete_btn<'a>() -> Element<'a, Message> {
  button(
    text("Cancel")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 5.0,
    bottom: 5.0,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .on_press(Message::DeleteCancelled)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if active { 0.25 } else { 0.1 }),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}
