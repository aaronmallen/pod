use iced::{
  Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use super::{Message, PlanRow};
use crate::ui::{
  components::{
    button::{Button, Size},
    icon::Icon,
    rule,
    step_badge::step_badge,
  },
  style::{color, spacing, typography},
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
  let count = distinct_skills.to_string();
  let key = if distinct_skills == 1 {
    "skills.panel_plans.card_subtitle_one"
  } else {
    "skills.panel_plans.card_subtitle_other"
  };
  let subtitle = t!(key, count => count, updated => updated).into_owned();

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
  step_badge(remaining_steps)
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
    text(t!("skills.panel_plans.delete_prompt"))
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
  Button::secondary(t!("skills.panel_plans.open"))
    .size(Size::Sm)
    .on_press(Message::OpenPlan(plan_id))
    .into()
}

fn delete_btn<'a>(plan_id: i64) -> Element<'a, Message> {
  Button::danger_icon(Icon::close())
    .size(Size::Sm)
    .on_press(Message::DeleteRequested(plan_id))
    .into()
}

fn confirm_delete_btn<'a>(plan_id: i64) -> Element<'a, Message> {
  Button::danger(t!("skills.panel_plans.confirm"))
    .size(Size::Sm)
    .on_press(Message::DeleteConfirmed(plan_id))
    .into()
}

fn cancel_delete_btn<'a>() -> Element<'a, Message> {
  Button::secondary(t!("skills.panel_plans.cancel"))
    .size(Size::Sm)
    .on_press(Message::DeleteCancelled)
    .into()
}
