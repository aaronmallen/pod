//! Saved plans list component.

pub mod plan_card;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Horizontal,
  widget::{Space, button, column, container, text},
};
pub use plan_card::Component as PlanCard;
use pod_model::SkillPlan;

use crate::style::{color, spacing, typography::body};

/// Messages produced by the plans tab.
#[derive(Clone, Debug)]
pub enum Message {
  NewPlan,
  FromQueue,
  OpenPlan(String),
  DeleteRequested(String),
  DeleteConfirmed(String),
  DeleteCancelled,
}

pub struct Component<'a> {
  plans: &'a [SkillPlan],
  plans_loaded: bool,
  confirm_delete_id: Option<&'a str>,
}

impl<'a> Component<'a> {
  pub fn new(plans: &'a [SkillPlan], plans_loaded: bool, confirm_delete_id: Option<&'a str>) -> Self {
    Self {
      plans,
      plans_loaded,
      confirm_delete_id,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    if !self.plans_loaded {
      return loading_state();
    }

    if self.plans.is_empty() {
      return empty_state();
    }

    let items: Vec<Element<'_, Message>> = self
      .plans
      .iter()
      .enumerate()
      .map(|(i, plan)| {
        let confirm = self.confirm_delete_id == Some(plan.id.as_str());
        PlanCard::new(plan, i == 0, confirm).render()
      })
      .collect();

    let list = column(items).width(Length::Fill);

    let footer = container(
      iced::widget::row([
        new_plan_btn(),
        Space::new().width(spacing::SPACE_2).into(),
        from_queue_btn(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    });

    column([list.into(), footer.into()]).width(Length::Fill).into()
  }
}

fn loading_state<'a>() -> Element<'a, Message> {
  container(
    text("Loading plans\u{2026}")
      .font(body::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .align_x(Horizontal::Center)
  .width(Length::Fill)
  .padding(Padding {
    top: 36.0,
    bottom: 36.0,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  container(
    column([
      text("No skill plans yet")
        .font(body::MEDIUM)
        .size(15.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(4.0).into(),
      text("Create your first plan to start optimizing your skill queue.")
        .font(body::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().height(spacing::SPACE_4).into(),
      iced::widget::row([
        new_plan_btn(),
        Space::new().width(spacing::SPACE_2).into(),
        from_queue_btn(),
      ])
      .align_y(iced::alignment::Vertical::Center)
      .into(),
    ])
    .align_x(Horizontal::Center),
  )
  .align_x(Horizontal::Center)
  .width(Length::Fill)
  .padding(Padding {
    top: 36.0,
    bottom: 36.0,
    left: spacing::SPACE_4,
    right: spacing::SPACE_4,
  })
  .into()
}

fn new_plan_btn<'a>() -> Element<'a, Message> {
  button(
    text("New plan")
      .font(body::MEDIUM)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::NewPlan)
  .style(|_, status| button::Style {
    background: Some(Background::Color(match status {
      button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA_SUBTLE,
      _ => iced::Color::TRANSPARENT,
    })),
    border: Border {
      color: color::accent::PLASMA_MUTED,
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
  .into()
}

fn from_queue_btn<'a>() -> Element<'a, Message> {
  button(
    text("From queue")
      .font(body::REGULAR)
      .size(12.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::FromQueue)
  .style(|_, status| button::Style {
    background: None,
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::border::DEFAULT,
        _ => color::border::SUBTLE,
      },
      radius: 6.0.into(),
      width: 1.0,
    },
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
  .into()
}
