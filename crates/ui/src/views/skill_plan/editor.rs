//! Plan editor: header bar, entry rows, and center-column layout.

pub mod empty_state;
pub mod entry_row;
pub mod header;
pub mod import_export_panel;
pub mod plan_entry_list;

pub use empty_state::EmptyState;
pub use entry_row::EntryRow;
pub use header::EditorHeader;
use iced::{
  Element, Length, Padding,
  widget::{column, container, stack},
};
pub use import_export_panel::ImportExportPanel;
pub use plan_entry_list::PlanEntryList;

use super::Message;
use crate::{plan_math::ComputedPlan, style::spacing};

/// The full plan editor view, composing header, body, and optional overlays.
pub struct PlanEditor<'a> {
  computed: &'a ComputedPlan,
  dirty: bool,
  drag_hover_entry_id: Option<&'a str>,
  dragging_entry_id: Option<&'a str>,
  export_dropdown_open: bool,
  import_dropdown_open: bool,
  note_expanded: Option<&'a str>,
  picker_open: bool,
  plan_name: &'a str,
}

impl<'a> PlanEditor<'a> {
  /// Creates a new `PlanEditor`.
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    plan_name: &'a str,
    dirty: bool,
    picker_open: bool,
    import_dropdown_open: bool,
    export_dropdown_open: bool,
    computed: &'a ComputedPlan,
    note_expanded: Option<&'a str>,
    dragging_entry_id: Option<&'a str>,
    drag_hover_entry_id: Option<&'a str>,
  ) -> Self {
    Self {
      computed,
      dirty,
      drag_hover_entry_id,
      dragging_entry_id,
      export_dropdown_open,
      import_dropdown_open,
      note_expanded,
      picker_open,
      plan_name,
    }
  }

  /// Renders the full editor into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let header = EditorHeader::new(self.plan_name, self.dirty, self.picker_open).render();
    let import_open = self.import_dropdown_open;
    let export_open = self.export_dropdown_open;
    let body = self.render_body();
    let col = column([header, body]).height(Length::Fill).width(Length::Fill);

    if let Some(overlay) = ImportExportPanel::new(import_open, export_open).render() {
      stack([col.into(), overlay]).into()
    } else {
      col.into()
    }
  }

  fn render_body(self) -> Element<'a, Message> {
    if self.computed.items.is_empty() {
      return EmptyState::new().render();
    }

    let inner = PlanEntryList::new(
      self.computed,
      self.note_expanded,
      self.dragging_entry_id,
      self.drag_hover_entry_id,
    )
    .render();

    container(inner)
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_4,
        bottom: spacing::SPACE_4,
        left: spacing::SPACE_7,
        right: spacing::SPACE_7,
      })
      .into()
  }
}
