//! Wallet sidebar — scrollable source list with section headers and source rows.

pub mod section_header;
pub mod source_row;

pub use section_header::Component as SectionHeader;
pub use source_row::{CharComponent as CharRow, Component as SourceRow, ContainerComponent};

use iced::Element;

use crate::{format, views::wallet::{Message, State}};

/// Builder for the wallet sidebar.
pub struct Component<'a> {
  state: &'a State,
  width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new sidebar component.
  pub fn new(state: &'a State) -> Self {
    Self { state, width: 240.0 }
  }

  /// Sets the sidebar width.
  pub fn width(mut self, w: f32) -> Self {
    self.width = w;
    self
  }

  /// Renders the sidebar into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let items: Vec<Element<'_, Message>> = if state.selected_character().is_none() {
      character_items(state)
    } else {
      vec![
        SectionHeader::new("Personal Wallet").render(),
        SourceRow::new(
          "Master Wallet",
          state.sidebar_source == "personal:master",
          Message::SidebarSourceChanged("personal:master".to_string()),
        )
        .render(),
      ]
    };
    ContainerComponent::new(items).width(self.width).render()
  }
}

fn character_items<'a>(state: &'a State) -> Vec<Element<'a, Message>> {
  let mut items: Vec<Element<'_, Message>> = vec![SectionHeader::new("Characters").render()];
  for c in &state.characters {
    let src = format!("char:{}", c.id);
    let active = state.sidebar_source == src;
    let liquid = format::fmt_isk(c.liquid);
    items.push(
      CharRow::new(
        &c.name,
        c.portrait_tone,
        liquid,
        active,
        Message::SidebarSourceChanged(src),
      )
      .render(),
    );
  }
  items
}
