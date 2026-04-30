//! Skill plan editor: shared message type and sub-module declarations.

pub mod editor;
pub mod picker;
pub mod summary;

/// Which pane boundary is being dragged in the plan editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneEdge {
  Picker,
  Summary,
}

#[derive(Debug, Clone)]
pub enum Message {
  SkillPicked(String, u8),
  PickerToggled,
  PickerTabChanged(usize),
  PickerSearchChanged(String),
  PickerGroupToggled(String),
  EntryRemoved(String),
  EntryPriorityChanged(String, crate::plan_math::Priority),
  EntryNoteChanged(String, String),
  EntryDragStart(String),
  EntryDragHover(String),
  EntryDragEnd,
  NameChanged(String),
  SaveRequested,
  SaveCompleted,
  CloseRequested,
  ConfirmClose,
  CancelClose,
  ImportDropdownToggled,
  ImportFromClipboard,
  ImportFromFile,
  ExportDropdownToggled,
  ExportToClipboard,
  ExportToFile,
  ExportPathChosen(Option<std::path::PathBuf>),
  ImportPathChosen(Option<std::path::PathBuf>),
  OptimizerRequested,
  OptimizerCompleted(Option<crate::plan_math::RemapResult>),
  ImplantSetChanged(crate::plan_math::ImplantSet),
  ImplantSuggestionsToggled,
  PaneDragStart(PaneEdge),
  PaneDrag(f32),
  PaneDragEnd,
  PlanLoaded(Option<pod_model::SkillPlan>),
  ShipMasteryChanged(i32, u8),
  ShipSelected(i32, String, u8),
  ModuleSelected(i32, String),
  ShipsLoaded(Vec<pod_model::ItemTypeSummary>),
  ModulesLoaded(Vec<pod_model::ItemTypeSummary>),
  CertificatesLoaded(Vec<pod_model::Certificate>),
  SkillGroupsLoaded(Vec<pod_model::SkillGroupDef>),
  AllCertsLoaded(Vec<pod_model::Certificate>),
  CertProficiencyChanged(i32, u8),
  CertSelected(i32, String, u8),
  AttrsLoaded {
    base_attrs: crate::plan_math::BaseAttrs,
    current_effective_attrs: crate::plan_math::BaseAttrs,
    clone_data_missing: bool,
  },
}
