mod shared;
pub(crate) mod standings;

use iced::{
  Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, container, responsive, scrollable, text},
};

use super::{Message, State};
use crate::ui::{
  components::{
    rule,
    tab_select::{Tab as SelectTab, TabLayout, tab_select_with},
  },
  style::{color, spacing, typography},
};

pub(super) const SCROLL_THRESHOLD: f32 = 0.85;

const TAB_BODY_PADDING: f32 = 28.0;
const TAB_STRIP_HEIGHT: f32 = 48.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
  Contacts,
  Killlog,
  Standings,
}

impl Tab {
  pub(super) const ORDER: [Tab; 3] = [Tab::Contacts, Tab::Killlog, Tab::Standings];

  fn label(self) -> &'static str {
    match self {
      Tab::Contacts => "Contacts",
      Tab::Killlog => "Kill Log",
      Tab::Standings => "Standings",
    }
  }

  fn placeholder_subtitle(self) -> &'static str {
    match self {
      Tab::Contacts => "Corporation contacts will appear here once contact sync ships.",
      Tab::Killlog => "The corporation kill log will appear here once killmail sync ships.",
      Tab::Standings => "Corporation standings will appear here once standings sync ships.",
    }
  }

  fn placeholder_title(self) -> &'static str {
    match self {
      Tab::Contacts => "No contacts yet",
      Tab::Killlog => "No kills yet",
      Tab::Standings => "No standings yet",
    }
  }
}

pub(super) fn tab_strip<'a>(active: Tab) -> Element<'a, Message> {
  let tabs: Vec<SelectTab<'a, Message>> = Tab::ORDER
    .into_iter()
    .map(|tab| {
      let selected = tab == active;
      SelectTab {
        count: String::new(),
        icon: None,
        label: tab.label(),
        on_press: (!selected).then_some(Message::TabChanged(tab)),
        selected,
      }
    })
    .collect();

  let strip = container(tab_select_with(tabs, TabLayout::Start))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT))
    .padding(Padding {
      top: 0.0,
      right: TAB_BODY_PADDING,
      bottom: 0.0,
      left: TAB_BODY_PADDING,
    });

  Column::with_children(vec![strip.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

pub(super) fn tab_body(state: &State) -> Element<'_, Message> {
  match state.active_tab() {
    Tab::Contacts => placeholder(Tab::Contacts),
    Tab::Killlog => placeholder(Tab::Killlog),
    Tab::Standings => windowed_standings(state),
  }
}

fn placeholder<'a>(active: Tab) -> Element<'a, Message> {
  let content = Column::with_children(vec![
    text(active.placeholder_title())
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(active.placeholder_subtitle())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_x(Horizontal::Center);

  container(content)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

/// Lays out the Standings tab: a hoisted, non-scrolling header above a height-filling scrollable whose sole content
/// is the virtualized body. The scrollable is nested inside `responsive` so the body builder receives the real
/// viewport height; the scrollbar's offset drives both the pagination threshold and the virtual window.
fn windowed_standings(state: &State) -> Element<'_, Message> {
  let side = Padding {
    top: 0.0,
    right: TAB_BODY_PADDING,
    bottom: 0.0,
    left: TAB_BODY_PADDING,
  };

  let header = container(standings::header(
    state.standings_query(),
    state.standings_filter(),
    state.standings_has_filters(),
  ))
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    bottom: spacing::SPACE_3_5,
    ..side
  });

  let scroll = responsive(move |size| {
    scrollable(
      container(standings::body(
        state.standings(),
        state.standings_filter(),
        state.standings_has_filters(),
        size.height,
        state.standings_scroll_offset(),
      ))
      .width(Length::Fill)
      .padding(Padding {
        top: 0.0,
        right: TAB_BODY_PADDING,
        bottom: spacing::SPACE_6,
        left: TAB_BODY_PADDING,
      }),
    )
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(|viewport| Message::StandingsScrolled {
      absolute: viewport.absolute_offset().y,
      relative: viewport.relative_offset().y,
    })
    .into()
  });

  Column::with_children(vec![header.into(), scroll.into()])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod tab_strip {
    use super::*;

    #[test]
    fn it_renders_in_order() {
      for tab in Tab::ORDER {
        let _el: Element<'_, Message> = tab_strip(tab);
      }
    }
  }
}
