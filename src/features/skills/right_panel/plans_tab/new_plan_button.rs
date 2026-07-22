use iced::Element;

use super::Message;
use crate::ui::components::{button::Button, icon::Icon};

pub fn new_plan_button<'a>() -> Element<'a, Message> {
  Button::primary(t!("skills.panel_plans.new_plan"))
    .icon(Icon::plus())
    .block()
    .on_press(Message::NewPlan)
    .into()
}
