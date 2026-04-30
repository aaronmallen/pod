//! Browse/Attrs/Plans tab container component.

pub mod attributes_tab;
pub mod browser_tab;
pub mod plans_tab;

pub use attributes_tab::Component as Attributes;
pub use browser_tab::Component as Browser;
use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{Space, button, column, container, scrollable, text},
};
pub use plans_tab::Component as Plans;

use super::{RightTab, State};
use crate::{
  components,
  style::{color, typography::body},
};

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
      .map(|(tab, label)| tab_btn(*tab, label, self.state.right_tab == *tab))
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

fn tab_btn(tab: RightTab, label: &'static str, is_active: bool) -> Element<'static, Message> {
  let btn = button(
    text(label)
      .font(body::MEDIUM)
      .size(13.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if is_active {
          color::text::PRIMARY
        } else {
          color::text::SECONDARY
        }),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 14.0,
    bottom: 12.0,
    left: 14.0,
    right: 14.0,
  })
  .on_press(Message::TabSelected(tab))
  .style(move |_, _| button::Style {
    background: None,
    border: Border::default(),
    text_color: if is_active {
      color::text::PRIMARY
    } else {
      color::text::SECONDARY
    },
    ..button::Style::default()
  });

  let underline = container(Space::new().width(Length::Fill).height(2.0))
    .width(Length::Fill)
    .height(2.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(if is_active {
        color::accent::PLASMA
      } else {
        Color::TRANSPARENT
      })),
      ..container::Style::default()
    });

  column([btn.into(), underline.into()]).width(Length::Fill).into()
}
