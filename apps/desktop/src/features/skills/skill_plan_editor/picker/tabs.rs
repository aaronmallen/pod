use iced::{Background, Border, Element, Length, widget::container};

use super::{super::Message, PickerTab};
use crate::ui::{
  components::tab_select::{Tab, TabLayout, tab_select_with},
  style::color,
};

const TAB_HEIGHT: f32 = 40.0;

pub(in crate::features::skills::skill_plan_editor) fn tabs<'a>(active: PickerTab) -> Element<'a, Message> {
  let tabs: Vec<Tab<'a, Message>> = PickerTab::ALL
    .iter()
    .map(|&tab| Tab {
      count_danger: false,
      count: String::new(),
      icon: None,
      label: tab.label(),
      on_press: Some(Message::PickerTabSelected(tab)),
      selected: tab == active,
    })
    .collect();

  container(tab_select_with(tabs, TabLayout::Fill))
    .width(Length::Fill)
    .height(TAB_HEIGHT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        radius: 0.0.into(),
        width: 0.0,
      },
      ..container::Style::default()
    })
    .into()
}
