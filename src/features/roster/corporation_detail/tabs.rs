pub(crate) mod contacts;
pub(crate) mod killlog;
mod shared;
pub(crate) mod standings;

use iced::{
  Element, Length, Padding,
  widget::{Column, container, responsive, scrollable},
};

use super::{Message, State};
use crate::ui::{
  components::{
    rule,
    tab_select::{Tab as SelectTab, TabLayout, tab_select_with},
  },
  style::spacing,
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
      Tab::Contacts => static_text(t!("roster.corporation.tab_contacts")),
      Tab::Killlog => static_text(t!("roster.corporation.tab_killlog")),
      Tab::Standings => static_text(t!("roster.corporation.tab_standings")),
    }
  }
}

fn static_text(value: std::borrow::Cow<'static, str>) -> &'static str {
  match value {
    std::borrow::Cow::Borrowed(text) => text,
    std::borrow::Cow::Owned(text) => Box::leak(text.into_boxed_str()),
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
    Tab::Contacts => windowed_contacts(state),
    Tab::Killlog => windowed_killlog(state),
    Tab::Standings => windowed_standings(state),
  }
}

/// Lays out the Contacts tab: a hoisted, non-scrolling header (search box + entity-type facet) above a
/// height-filling scrollable whose sole content is the virtualized body. The scrollable is nested inside
/// `responsive` so the body builder receives the real viewport height; the scrollbar's offset drives both the
/// pagination threshold and the virtual window.
fn windowed_contacts(state: &State) -> Element<'_, Message> {
  let side = Padding {
    top: 0.0,
    right: TAB_BODY_PADDING,
    bottom: 0.0,
    left: TAB_BODY_PADDING,
  };

  let header = container(contacts::header(
    state.contacts(),
    state.contact_filter(),
    state.contacts_query(),
  ))
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_6,
    bottom: spacing::SPACE_3_5,
    ..side
  });

  let scroll = responsive(move |size| {
    scrollable(
      container(contacts::body(
        state.contacts(),
        state.contact_sort(),
        size.height,
        state.contacts_scroll_offset(),
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
    .on_scroll(|viewport| Message::ContactsScrolled {
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

/// Lays out the Kill Log tab: an optional hoisted, non-scrolling header above a height-filling scrollable whose sole
/// content is the virtualized body. The header is absent in the loading/error/empty states (the body renders those as
/// a full-height placeholder). The scrollable is nested inside `responsive` so the body builder receives the real
/// viewport height; the scrollbar's offset drives the virtual window.
fn windowed_killlog(state: &State) -> Element<'_, Message> {
  let side = Padding {
    top: 0.0,
    right: TAB_BODY_PADDING,
    bottom: 0.0,
    left: TAB_BODY_PADDING,
  };

  let scroll = responsive(move |size| {
    scrollable(
      container(killlog::body(
        state.killlog(),
        state.killlog_filter(),
        size.height,
        state.killlog_scroll_offset(),
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
    .on_scroll(|viewport| Message::KilllogScrolled {
      absolute: viewport.absolute_offset().y,
      relative: viewport.relative_offset().y,
    })
    .into()
  });

  let mut children: Vec<Element<'_, Message>> = Vec::with_capacity(2);
  if let Some(header) = killlog::header(state.killlog(), state.killlog_filter()) {
    children.push(
      container(header)
        .width(Length::Fill)
        .padding(Padding {
          top: spacing::SPACE_6,
          bottom: spacing::SPACE_3_5,
          ..side
        })
        .into(),
    );
  }
  children.push(scroll.into());

  Column::with_children(children)
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
