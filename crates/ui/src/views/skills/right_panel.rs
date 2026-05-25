//! Browse/Attrs/Plans tab container component.

pub mod attributes_tab;
pub mod browser_tab;
pub mod plans_tab;
pub mod tab_button;

pub use attributes_tab::Component as Attributes;
pub use browser_tab::Component as Browser;
use iced::{
  Background, Element, Length,
  widget::{column, container, scrollable},
};
pub use plans_tab::Component as Plans;
pub use tab_button::TabButton;

use super::{RightTab, State};
use crate::{components, style::color};

/// Messages produced by the right panel.
#[derive(Clone, Debug)]
pub enum Message {
  AttributesTab(attributes_tab::Message),
  BrowserTab(browser_tab::Message),
  PlansTab(plans_tab::Message),
  TabSelected(RightTab),
}

pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  pub fn render(self) -> Element<'a, Message> {
    let tabs = [
      (RightTab::Browse, "Browse"),
      (RightTab::Attrs, "Attributes"),
      (RightTab::Plans, "Plans"),
    ];
    let tab_btns: Vec<Element<'_, Message>> = tabs
      .iter()
      .map(|(tab, label)| TabButton::new(*tab, label, self.state.right_tab == *tab).render())
      .collect();
    let tab_bar = column([
      row(tab_btns).width(Length::Fill).into(),
      components::Separator::horizontal().render(),
    ])
    .width(Length::Fill);
    let confirm_id = self.state.confirm_delete_plan_id.as_deref();
    let tab_content: Element<'_, Message> = match self.state.right_tab {
      RightTab::Browse => Browser::new(self.state).render().map(Message::BrowserTab),
      RightTab::Attrs => Attributes::new(self.state).render().map(|m| match m {}),
      RightTab::Plans => Plans::new(&self.state.plans, self.state.plans_loaded, confirm_id)
        .render()
        .map(Message::PlansTab),
    };
    let panel = column([tab_bar.into(), scrollable(tab_content).height(Length::Fill).into()]).height(Length::Fill);
    container(panel)
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        ..container::Style::default()
      })
      .into()
  }
}

fn row<'a>(items: Vec<Element<'a, Message>>) -> iced::widget::Row<'a, Message> {
  iced::widget::row(items)
}
