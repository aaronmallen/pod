//! Saved plans list component.

pub mod empty_state;
pub mod from_queue_button;
pub mod loading_state;
pub mod new_plan_button;
pub mod plan_card;

pub use empty_state::Component as PlansEmptyState;
pub use from_queue_button::Component as FromQueueButton;
use iced::{
  Element, Length,
  widget::{Space, column, container},
};
pub use loading_state::Component as PlansLoadingState;
pub use new_plan_button::Component as NewPlanButton;
pub use plan_card::Component as PlanCard;
use pod_model::SkillPlan;

use crate::style::spacing;

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
      return PlansLoadingState::new().render();
    }

    if self.plans.is_empty() {
      return PlansEmptyState::new().render();
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
        NewPlanButton::new().render(),
        Space::new().width(spacing::SPACE_2).into(),
        FromQueueButton::new().render(),
      ])
      .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding(iced::Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    });

    column([list.into(), footer.into()]).width(Length::Fill).into()
  }
}
