pub mod attributes_tab;
pub mod browser_tab;
pub mod plans_tab;

use chrono::{DateTime, Utc};
use iced::{
  Background, Border, Element, Length,
  widget::{Column, container},
};

use super::attributes::AttrTabModel;
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
  #[default]
  Browse,
  Plans,
}

#[derive(Clone, Debug)]
pub enum Message {
  Browse(browser_tab::Message),
  Plans(plans_tab::Message),
  TabSelected(RightTab),
}

pub struct Panel<'a> {
  pub attributes: Option<&'a AttrTabModel>,
  pub browse: &'a browser_tab::State,
  pub now: DateTime<Utc>,
  pub plans: &'a plans_tab::State,
  pub tab: RightTab,
}

impl<'a> Panel<'a> {
  pub fn render(self) -> Element<'a, Message> {
    let strip = container(tab_select_with(
      vec![
        tab("Browse", self.tab == RightTab::Browse, RightTab::Browse),
        tab("Attributes", self.tab == RightTab::Attributes, RightTab::Attributes),
        tab("Plans", self.tab == RightTab::Plans, RightTab::Plans),
      ],
      TabLayout::Fill,
    ))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT));

    let body: Element<'a, Message> = match self.tab {
      RightTab::Attributes => attributes_tab::view::<Message>(self.attributes, self.now),
      RightTab::Browse => browser_tab::view(self.browse).map(Message::Browse),
      RightTab::Plans => plans_tab::view(self.plans).map(Message::Plans),
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

fn tab<'a>(label: &'a str, selected: bool, target: RightTab) -> Tab<'a, Message> {
  Tab {
    count: String::new(),
    label,
    on_press: Some(Message::TabSelected(target)),
    selected,
  }
}
