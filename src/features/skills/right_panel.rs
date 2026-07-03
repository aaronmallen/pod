pub mod attributes_tab;
pub mod browser_tab;
pub mod plans_tab;
pub mod queue_tab;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length,
  widget::{Column, container},
};

use super::{attributes::AttrTabModel, queue::ComputedQueue};
use crate::ui::{
  components::{
    rule,
    tab_select::{Tab, TabLayout, tab_select_with},
  },
  style::color,
};

const TAB_STRIP_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RightTab {
  Attributes,
  Browse,
  Plans,
  #[default]
  Queue,
}

#[derive(Clone, Debug)]
pub enum Message {
  Browse(browser_tab::Message),
  Plans(plans_tab::Message),
  Queue(queue_tab::Message),
  TabSelected(RightTab),
}

pub struct Panel<'a> {
  pub attributes: Option<&'a AttrTabModel>,
  pub browse: &'a browser_tab::State,
  pub computed: &'a ComputedQueue,
  pub now: DateTime<Utc>,
  pub plans: &'a plans_tab::State,
  pub selection_count: usize,
  pub tab: RightTab,
}

impl<'a> Panel<'a> {
  pub fn render(self) -> Element<'a, Message> {
    let plans_count = if self.selection_count > 0 {
      self.selection_count.to_string()
    } else {
      String::new()
    };
    // Tab labels are borrowed for the strip's lifetime, so the resolved strings must outlive this
    // method; cache each once to hand `tab_select_with` `&'static str`s.
    static QUEUE_LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    static BROWSE_LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    static ATTRIBUTES_LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    static PLANS_LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let queue_label = QUEUE_LABEL.get_or_init(|| t!("skills.panel.tab_queue").into_owned());
    let browse_label = BROWSE_LABEL.get_or_init(|| t!("skills.panel.tab_browse").into_owned());
    let attributes_label = ATTRIBUTES_LABEL.get_or_init(|| t!("skills.panel.tab_attributes").into_owned());
    let plans_label = PLANS_LABEL.get_or_init(|| t!("skills.panel.tab_plans").into_owned());

    let strip = container(tab_select_with(
      vec![
        tab(queue_label, self.tab == RightTab::Queue, RightTab::Queue, String::new()),
        tab(
          browse_label,
          self.tab == RightTab::Browse,
          RightTab::Browse,
          String::new(),
        ),
        tab(plans_label, self.tab == RightTab::Plans, RightTab::Plans, plans_count),
        tab(
          attributes_label,
          self.tab == RightTab::Attributes,
          RightTab::Attributes,
          String::new(),
        ),
      ],
      TabLayout::Fill,
    ))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT));

    let body: Element<'a, Message> = match self.tab {
      RightTab::Attributes => attributes_tab::view::<Message>(self.attributes, self.now),
      RightTab::Browse => browser_tab::view(self.browse).map(Message::Browse),
      RightTab::Plans => plans_tab::view(self.plans, self.selection_count).map(Message::Plans),
      RightTab::Queue => queue_tab::view(self.computed, self.now).map(Message::Queue),
    };

    let panel = Column::with_children(vec![strip.into(), rule::horizontal(), body])
      .width(Length::Fill)
      .height(Length::Fill);

    container(panel)
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.1),
          width: 1.0,
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn tab<'a>(label: &'a str, selected: bool, target: RightTab, count: String) -> Tab<'a, Message> {
  Tab {
    count,
    icon: None,
    label,
    on_press: Some(Message::TabSelected(target)),
    selected,
  }
}
