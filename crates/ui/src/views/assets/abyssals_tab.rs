//! Abyssals tab — mutated module grid with stat rows and resizable filter sidebar.

pub mod abyssal_card;
pub mod card_grid;
pub mod filter_sidebar;
pub mod module_type_picker;
pub mod stat_ranges_panel;
pub mod stat_row;
pub mod tier_badge;
pub mod type_icon_tile;

use std::collections::HashMap;

use iced::{
  Element, Length, Point, Size, mouse,
  widget::{
    canvas::{self, Action, Frame, Geometry, Path, Stroke},
    image, row, stack,
  },
};
use pod_model::{AbyssalCategory, AbyssalStatViewModel, AbyssalViewModel};

use self::module_type_picker::{ModalEntry, ModalRow, ModalSection};
use super::State;
use crate::style::color;

const UNIT_SUFFIX_TABLE: &[(i32, &str)] = &[
  (71, " GJ"),
  (101, " m/s"),
  (105, " HP"),
  (108, " s"),
  (114, " kg"),
  (115, " tf"),
  (116, " MW"),
  (117, " km"),
  (121, " m\u{00b3}"),
  (124, "%"),
];

fn unit_suffix_for_id(unit_id: Option<i32>) -> &'static str {
  unit_id
    .and_then(|id| UNIT_SUFFIX_TABLE.iter().find(|&&(k, _)| k == id).map(|&(_, v)| v))
    .unwrap_or("")
}

pub fn dogma_unit_suffix(unit_id: Option<i32>) -> String {
  unit_suffix_for_id(unit_id).to_string()
}

/// Which endpoint of a stat slider range is being edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderEndpoint {
  Max,
  Min,
}

/// Messages produced by the abyssals tab.
#[derive(Clone, Debug)]
pub enum Message {
  /// The module-type picker modal was closed without a selection change.
  CloseTypeModal,
  /// The active filter was cleared.
  FilterReset,
  /// The MutaMarket logo or price was clicked — open mutamarket.com.
  OpenMutamarket,
  /// The module-type picker modal was opened.
  OpenTypeModal,
  /// Cursor moved during filter pane drag (cursor x position).
  PaneDrag(f32),
  /// Filter pane drag released.
  PaneDragEnd,
  /// Filter pane drag handle pressed.
  PaneDragStart,
  /// The card grid was scrolled; carries relative vertical offset (0.0–1.0).
  ScrollUpdate(f32),
  /// A slider value text field was committed (attribute_id, endpoint).
  SliderEditCommit(i32, SliderEndpoint),
  /// A slider value text field content changed (new text).
  SliderEditInput(String),
  /// A slider value label was clicked to begin editing (attribute_id, endpoint, current_value).
  SliderEditStart(i32, SliderEndpoint, f64),
  /// A per-stat maximum filter value changed (attribute_id, new_max).
  StatMaxFilterChanged(i32, f64),
  /// A per-stat minimum filter value changed (attribute_id, new_min).
  StatMinFilterChanged(i32, f64),
  /// A module source type was selected in the picker (None = all types).
  TypeSelected(Option<i32>),
}

pub(super) fn format_stat_value(value: f64, unit_suffix: &str) -> String {
  let formatted = format!("{:.2}", value);
  let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
  format!("{}{unit_suffix}", trimmed)
}

fn stat_roll_contribution(s: &AbyssalStatViewModel) -> Option<f64> {
  let delta = s.rolled_value - s.base_value;
  if delta.abs() < 1e-9 {
    return None;
  }
  let pct = if s.base_value.abs() > 1e-9 {
    delta / s.base_value * 100.0
  } else {
    0.0
  };
  Some(if s.high_is_good { pct } else { -pct })
}

pub fn roll_score(item: &AbyssalViewModel) -> f64 {
  if item.stats.is_empty() {
    return 0.0;
  }
  let sum: f64 = item.stats.iter().filter_map(stat_roll_contribution).sum();
  sum / item.stats.len() as f64
}

#[derive(Clone, Copy, PartialEq)]
enum DragTarget {
  Max,
  Min,
}

#[derive(Clone, Default)]
struct RangeSliderState {
  dragging: Option<DragTarget>,
}

struct RangeSliderProgram {
  attribute_id: i32,
  current_max: f64,
  current_min: f64,
  hi: f64,
  lo: f64,
}

impl canvas::Program<Message> for RangeSliderProgram {
  type State = RangeSliderState;

  fn update(
    &self,
    state: &mut Self::State,
    event: &canvas::Event,
    bounds: iced::Rectangle,
    cursor: mouse::Cursor,
  ) -> Option<Action<Message>> {
    let range = (self.hi - self.lo).max(1e-9);
    let thumb_r = 7.0f32;
    let inner_w = (bounds.width - thumb_r * 2.0).max(1.0);
    let val_at_x = |x: f32| -> f64 {
      let frac = ((x - thumb_r) / inner_w).clamp(0.0, 1.0) as f64;
      self.lo + frac * range
    };

    match event {
      canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
        let padded = iced::Rectangle {
          x: bounds.x - thumb_r,
          y: bounds.y,
          width: bounds.width + thumb_r * 2.0,
          height: bounds.height,
        };
        let pos = cursor.position_in(padded)?;
        let canvas_x = pos.x - thumb_r;
        let thumb_lo = thumb_r;
        let thumb_hi = thumb_r + inner_w;
        let x_min = (thumb_r + ((self.current_min - self.lo) / range) as f32 * inner_w).clamp(thumb_lo, thumb_hi);
        let x_max = (thumb_r + ((self.current_max - self.lo) / range) as f32 * inner_w).clamp(thumb_lo, thumb_hi);
        let target = if (canvas_x - x_min).abs() <= (canvas_x - x_max).abs() {
          DragTarget::Min
        } else {
          DragTarget::Max
        };
        state.dragging = Some(target);
        Some(Action::request_redraw())
      }
      canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
        if state.dragging.is_some() {
          state.dragging = None;
          return Some(Action::request_redraw());
        }
        None
      }
      canvas::Event::Mouse(mouse::Event::CursorMoved {
        ..
      }) => {
        let target = state.dragging?;
        let raw = cursor.position().unwrap_or_default();
        let canvas_x = raw.x - bounds.x;
        let new_val = val_at_x(canvas_x);
        let attr_id = self.attribute_id;
        let message = match target {
          DragTarget::Min => Message::StatMinFilterChanged(attr_id, new_val.clamp(self.lo, self.current_max)),
          DragTarget::Max => Message::StatMaxFilterChanged(attr_id, new_val.clamp(self.current_min, self.hi)),
        };
        Some(Action::publish(message))
      }
      _ => None,
    }
  }

  fn draw(
    &self,
    state: &Self::State,
    renderer: &iced::Renderer,
    _theme: &iced::Theme,
    bounds: iced::Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<Geometry<iced::Renderer>> {
    let mut frame = Frame::new(renderer, bounds.size());
    let w = frame.width();
    let h = frame.height();
    let cy = h / 2.0;
    let rail_h = 3.0f32;
    let thumb_r = 7.0f32;
    let range = (self.hi - self.lo).max(1e-9);
    let inner_w = (w - thumb_r * 2.0).max(0.0);
    let x_for = |v: f64| thumb_r + ((v - self.lo) / range).clamp(0.0, 1.0) as f32 * inner_w;
    let x_min = x_for(self.current_min);
    let x_max = x_for(self.current_max);

    let is_filtered = (self.current_min - self.lo).abs() > 1e-9 || (self.current_max - self.hi).abs() > 1e-9;

    let bg_rail = Path::rectangle(
      Point::new(thumb_r, cy - rail_h / 2.0),
      Size::new((w - thumb_r * 2.0).max(0.0), rail_h),
    );
    frame.fill(&bg_rail, color::border::SUBTLE);

    let segment_col = if is_filtered {
      color::text::ACCENT
    } else {
      color::with_alpha(color::text::PRIMARY, 0.22)
    };
    let active_w = (x_max - x_min).max(0.0);
    if active_w > 0.0 {
      let active_rail = Path::rectangle(Point::new(x_min, cy - rail_h / 2.0), Size::new(active_w, rail_h));
      frame.fill(&active_rail, segment_col);
    }

    for (x, dragging) in [
      (x_min, state.dragging == Some(DragTarget::Min)),
      (x_max, state.dragging == Some(DragTarget::Max)),
    ] {
      let r = if dragging { 8.0f32 } else { thumb_r };
      if dragging {
        let glow = Path::circle(Point::new(x, cy), r + 4.0);
        frame.fill(&glow, color::with_alpha(color::text::ACCENT, 0.22));
      }
      let thumb = Path::circle(Point::new(x, cy), r);
      frame.fill(&thumb, color::surface::BASE);
      let border_col = if dragging {
        color::text::ACCENT
      } else {
        color::text::PRIMARY
      };
      frame.stroke(&thumb, Stroke::default().with_color(border_col).with_width(2.0));
    }

    vec![frame.into_geometry()]
  }

  fn mouse_interaction(
    &self,
    state: &Self::State,
    bounds: iced::Rectangle,
    cursor: mouse::Cursor,
  ) -> mouse::Interaction {
    if state.dragging.is_some() || cursor.position_in(bounds).is_some() {
      mouse::Interaction::ResizingHorizontally
    } else {
      mouse::Interaction::default()
    }
  }
}

static MODAL_LAYOUT: &[&[ModalSection]] = &[
  &[
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Stasis Webifier",
          type_id: 47702,
        },
        ModalRow::Single {
          label: "Warp Scrambler",
          type_id: 47732,
        },
        ModalRow::Single {
          label: "Warp Disruptor",
          type_id: 47736,
        },
        ModalRow::Single {
          label: "Heavy Warp Scrambler",
          type_id: 56303,
        },
        ModalRow::Single {
          label: "Heavy Warp Disruptor",
          type_id: 56304,
        },
      ],
      title: "Electronic Warfare",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Magnetic Field Stabilizer",
          type_id: 49722,
        },
        ModalRow::Single {
          label: "Heat Sink",
          type_id: 49726,
        },
        ModalRow::Single {
          label: "Gyrostabilizer",
          type_id: 49730,
        },
        ModalRow::Single {
          label: "Entropic Radiation Sink",
          type_id: 49734,
        },
        ModalRow::Single {
          label: "Ballistic Control System",
          type_id: 49738,
        },
        ModalRow::Single {
          label: "Drone Damage Amplifier",
          type_id: 60482,
        },
        ModalRow::Single {
          label: "Siege Module",
          type_id: 56313,
        },
        ModalRow::Single {
          label: "Vorton Tuning System",
          type_id: 78621,
        },
        ModalRow::Single {
          label: "Fighter Support Unit",
          type_id: 60483,
        },
      ],
      title: "Weapon Upgrades",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Mining Laser",
          type_id: 90460,
        },
        ModalRow::Single {
          label: "Deep Core Mining Laser",
          type_id: 90483,
        },
        ModalRow::Single {
          label: "Modulated Deep Core Miner",
          type_id: 90474,
        },
      ],
      title: "Mining Lasers",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Strip Miner",
          type_id: 90493,
        },
        ModalRow::Single {
          label: "Deep Core Strip Miner",
          type_id: 90498,
        },
        ModalRow::Single {
          label: "Modulated Strip Miner",
          type_id: 90467,
        },
        ModalRow::Single {
          label: "Modulated Deep Core Strip Miner",
          type_id: 90487,
        },
      ],
      title: "Strip Miners",
    },
  ],
  &[
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Shield Booster",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47781,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47785,
            },
            ModalEntry {
              label: "Large",
              type_id: 47789,
            },
            ModalEntry {
              label: "X-Large",
              type_id: 47793,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56309,
            },
          ],
        },
        ModalRow::Family {
          name: "Ancillary Shield Booster",
          variants: &[
            ModalEntry {
              label: "Medium",
              type_id: 47836,
            },
            ModalEntry {
              label: "Large",
              type_id: 47838,
            },
            ModalEntry {
              label: "X-Large",
              type_id: 47840,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56310,
            },
          ],
        },
        ModalRow::Family {
          name: "Shield Extender",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47800,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47804,
            },
            ModalEntry {
              label: "Large",
              type_id: 47808,
            },
          ],
        },
      ],
      title: "Shield",
    },
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Armor Repairer",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47769,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47773,
            },
            ModalEntry {
              label: "Large",
              type_id: 47777,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56307,
            },
          ],
        },
        ModalRow::Family {
          name: "Ancillary Armor Repairer",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47842,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47844,
            },
            ModalEntry {
              label: "Large",
              type_id: 47846,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56308,
            },
          ],
        },
        ModalRow::Family {
          name: "Armor Plates",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47812,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47817,
            },
            ModalEntry {
              label: "Large",
              type_id: 47820,
            },
          ],
        },
      ],
      title: "Armor",
    },
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Afterburner",
          variants: &[
            ModalEntry {
              label: "1MN",
              type_id: 47749,
            },
            ModalEntry {
              label: "10MN",
              type_id: 47753,
            },
            ModalEntry {
              label: "100MN",
              type_id: 47757,
            },
            ModalEntry {
              label: "10000MN",
              type_id: 56305,
            },
          ],
        },
        ModalRow::Family {
          name: "Microwarpdrive",
          variants: &[
            ModalEntry {
              label: "5MN",
              type_id: 47740,
            },
            ModalEntry {
              label: "50MN",
              type_id: 47408,
            },
            ModalEntry {
              label: "500MN",
              type_id: 47745,
            },
            ModalEntry {
              label: "50000MN",
              type_id: 56306,
            },
          ],
        },
      ],
      title: "Propulsion",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Ice Mining Laser",
          type_id: 90502,
        },
        ModalRow::Single {
          label: "Ice Harvester",
          type_id: 90524,
        },
      ],
      title: "Ice Mining",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Gas Cloud Scoop",
          type_id: 90529,
        },
        ModalRow::Single {
          label: "Gas Cloud Harvester",
          type_id: 90593,
        },
      ],
      title: "Gas Harvesting",
    },
  ],
  &[
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Energy Neutralizer",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 47824,
            },
            ModalEntry {
              label: "Medium",
              type_id: 47828,
            },
            ModalEntry {
              label: "Heavy",
              type_id: 47832,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56312,
            },
          ],
        },
        ModalRow::Family {
          name: "Energy Nosferatu",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 48419,
            },
            ModalEntry {
              label: "Medium",
              type_id: 48423,
            },
            ModalEntry {
              label: "Heavy",
              type_id: 48427,
            },
            ModalEntry {
              label: "Capital",
              type_id: 56311,
            },
          ],
        },
        ModalRow::Family {
          name: "Cap Battery",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 48431,
            },
            ModalEntry {
              label: "Medium",
              type_id: 48435,
            },
            ModalEntry {
              label: "Large",
              type_id: 48439,
            },
          ],
        },
      ],
      title: "Engineering",
    },
    ModalSection {
      rows: &[
        ModalRow::Family {
          name: "Damage Control",
          variants: &[
            ModalEntry {
              label: "Regular",
              type_id: 52227,
            },
            ModalEntry {
              label: "Assault",
              type_id: 52230,
            },
          ],
        },
        ModalRow::Family {
          name: "EMP Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84442,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84438,
            },
            ModalEntry {
              label: "Large",
              type_id: 84434,
            },
          ],
        },
        ModalRow::Family {
          name: "Graviton Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84444,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84440,
            },
            ModalEntry {
              label: "Large",
              type_id: 84436,
            },
          ],
        },
        ModalRow::Family {
          name: "Plasma Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84443,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84439,
            },
            ModalEntry {
              label: "Large",
              type_id: 84435,
            },
          ],
        },
        ModalRow::Family {
          name: "Proton Smartbomb",
          variants: &[
            ModalEntry {
              label: "Small",
              type_id: 84445,
            },
            ModalEntry {
              label: "Medium",
              type_id: 84441,
            },
            ModalEntry {
              label: "Large",
              type_id: 84437,
            },
          ],
        },
        ModalRow::Family {
          name: "Combat Drone",
          variants: &[
            ModalEntry {
              label: "Light",
              type_id: 60478,
            },
            ModalEntry {
              label: "Medium",
              type_id: 60479,
            },
            ModalEntry {
              label: "Heavy",
              type_id: 60480,
            },
            ModalEntry {
              label: "Sentry",
              type_id: 60481,
            },
          ],
        },
      ],
      title: "Miscellaneous",
    },
    ModalSection {
      rows: &[
        ModalRow::Single {
          label: "Mining Drone",
          type_id: 90614,
        },
        ModalRow::Single {
          label: "Ice Harvesting Drone",
          type_id: 90618,
        },
        ModalRow::Single {
          label: "'Excavator' Mining Drone",
          type_id: 90621,
        },
        ModalRow::Single {
          label: "'Excavator' Ice Harvesting Drone",
          type_id: 90622,
        },
      ],
      title: "Mining Drones",
    },
  ],
];

static MODAL_SOURCE_PATTERNS: &[(i32, &str)] = &[
  (47702, "Stasis Webifier"),
  (47732, "Warp Scrambler"),
  (47736, "Warp Disruptor"),
  (56303, "Heavy Warp Scrambler"),
  (56304, "Heavy Warp Disruptor"),
  (49722, "Magnetic Field Stabilizer"),
  (49726, "Heat Sink"),
  (49730, "Gyrostabilizer"),
  (49734, "Entropic Radiation Sink"),
  (49738, "Ballistic Control System"),
  (60482, "Drone Damage Amplifier"),
  (56313, "Siege Module"),
  (78621, "Vorton Tuning System"),
  (60483, "Fighter Support Unit"),
  (90460, "Mining Laser"),
  (90483, "Deep Core Mining Laser"),
  (90474, "Modulated Deep Core Miner"),
  (90493, "Strip Miner"),
  (90498, "Deep Core Strip Miner"),
  (90467, "Modulated Strip Miner"),
  (90487, "Modulated Deep Core Strip Miner"),
  (47781, "Small Shield Booster"),
  (47785, "Medium Shield Booster"),
  (47789, "Large Shield Booster"),
  (47793, "X-Large Shield Booster"),
  (56309, "Capital Shield Booster"),
  (47836, "Medium Ancillary Shield Booster"),
  (47838, "Large Ancillary Shield Booster"),
  (47840, "X-Large Ancillary Shield Booster"),
  (56310, "Capital Ancillary Shield Booster"),
  (47800, "Small Shield Extender"),
  (47804, "Medium Shield Extender"),
  (47808, "Large Shield Extender"),
  (47769, "Small Armor Repairer"),
  (47773, "Medium Armor Repairer"),
  (47777, "Large Armor Repairer"),
  (56307, "Capital Armor Repairer"),
  (47842, "Small Ancillary Armor Repairer"),
  (47844, "Medium Ancillary Armor Repairer"),
  (47846, "Large Ancillary Armor Repairer"),
  (56308, "Capital Ancillary Armor Repairer"),
  (47749, "1MN Afterburner"),
  (47753, "10MN Afterburner"),
  (47757, "100MN Afterburner"),
  (56305, "10000MN Afterburner"),
  (47740, "5MN Microwarpdrive"),
  (47408, "50MN Microwarpdrive"),
  (47745, "500MN Microwarpdrive"),
  (56306, "50000MN Microwarpdrive"),
  (90502, "Ice Mining Laser"),
  (90524, "Ice Harvester"),
  (90529, "Gas Cloud Scoop"),
  (90593, "Gas Cloud Harvester"),
  (47824, "Small Energy Neutralizer"),
  (47828, "Medium Energy Neutralizer"),
  (47832, "Heavy Energy Neutralizer"),
  (56312, "Capital Energy Neutralizer"),
  (48419, "Small Energy Nosferatu"),
  (48423, "Medium Energy Nosferatu"),
  (48427, "Heavy Energy Nosferatu"),
  (56311, "Capital Energy Nosferatu"),
  (48431, "Small Cap Battery"),
  (48435, "Medium Cap Battery"),
  (48439, "Large Cap Battery"),
  (52227, "Damage Control"),
  (52230, "Assault Damage Control"),
  (84442, "Small EMP Smartbomb"),
  (84438, "Medium EMP Smartbomb"),
  (84434, "Large EMP Smartbomb"),
  (84444, "Small Graviton Smartbomb"),
  (84440, "Medium Graviton Smartbomb"),
  (84436, "Large Graviton Smartbomb"),
  (84443, "Small Plasma Smartbomb"),
  (84439, "Medium Plasma Smartbomb"),
  (84435, "Large Plasma Smartbomb"),
  (84445, "Small Proton Smartbomb"),
  (84441, "Medium Proton Smartbomb"),
  (84437, "Large Proton Smartbomb"),
  (90614, "Mining Drone"),
  (90618, "Ice Harvesting Drone"),
  (90621, "'Excavator' Mining Drone"),
  (90622, "'Excavator' Ice Harvesting Drone"),
];

/// State for the abyssals tab sub-view.
#[derive(Clone, Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct AbyssalsState {
  /// All loaded abyssal view models.
  pub abyssals: Vec<AbyssalViewModel>,
  /// All abyssal source-type categories from the SDE, sorted by definition order.
  pub categories: Vec<AbyssalCategory>,
  /// Whether the filter pane drag is in progress.
  pub filter_pane_dragging: bool,
  /// Last recorded x position during filter pane drag.
  pub filter_pane_last_drag_x: f32,
  /// Current width of the filter sidebar pane in pixels.
  pub filter_pane_width: f32,
  /// Whether the module-type picker modal is open.
  pub modal_open: bool,
  /// Cached portrait image handles keyed by character_id, pre-built to avoid
  /// per-render allocation which causes flickering.
  pub portrait_handles: HashMap<i64, image::Handle>,
  /// Selected source type ID for the stat-range filter (None = all types).
  pub selected_source_type_id: Option<i32>,
  /// Per-attribute-id filter range (min_val, max_val).
  pub stat_range_filters: HashMap<i32, (f64, f64)>,
  /// Which slider value label is currently being edited, if any.
  pub slider_editing: Option<(i32, SliderEndpoint)>,
  /// Current text in the active slider value edit field.
  pub slider_edit_text: String,
  /// Loaded EVE icons keyed by source_type_id.
  pub type_icons: HashMap<i32, image::Handle>,
  /// Number of cards currently rendered; grows by 25 on scroll past 85%.
  pub visible_count: usize,
}

impl Default for AbyssalsState {
  fn default() -> Self {
    Self {
      abyssals: Vec::new(),
      categories: Vec::new(),
      filter_pane_dragging: false,
      filter_pane_last_drag_x: 0.0,
      filter_pane_width: 220.0,
      modal_open: false,
      portrait_handles: HashMap::new(),
      selected_source_type_id: None,
      slider_edit_text: String::new(),
      slider_editing: None,
      stat_range_filters: HashMap::new(),
      type_icons: HashMap::new(),
      visible_count: 50,
    }
  }
}

/// Builder for the abyssals tab.
pub struct Component<'a> {
  state: &'a State,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new abyssals tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
      window_width: 1200.0,
    }
  }

  /// Sets the available window width used by the card grid.
  pub fn window_width(mut self, width: f32) -> Self {
    self.window_width = width;
    self
  }

  /// Renders the abyssals tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let sidebar_el = filter_sidebar::Component::new(state).render();
    let drag_handle = filter_sidebar::drag_handle();
    let grid_width = (self.window_width - state.abyssals.filter_pane_width).max(0.0);
    let grid_el = card_grid::Component::new(state).window_width(grid_width).render();

    let base = row([sidebar_el, drag_handle, grid_el])
      .width(Length::Fill)
      .height(Length::Fill);

    if state.abyssals.modal_open {
      stack([base.into(), module_type_picker::Component::new(state).render()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
      base.into()
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod roll_score {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_item(stats: Vec<AbyssalStatViewModel>) -> AbyssalViewModel {
      AbyssalViewModel {
        base_type_name: "Test Module".to_string(),
        character_id: 1,
        item_id: 100,
        location: "Jita".to_string(),
        muta_price_isk: None,
        mutaplasmid_color_hue: 220,
        mutaplasmid_tier: "Decayed".to_string(),
        source_type_id: 1,
        stats,
        type_id: 47804,
      }
    }

    fn make_stat(display_name: &str, base: f64, rolled: f64, high_is_good: bool) -> AbyssalStatViewModel {
      AbyssalStatViewModel {
        attribute_id: 1,
        base_value: base,
        display_name: display_name.to_string(),
        high_is_good,
        icon_id: None,
        max_mult: 1.5,
        min_mult: 0.7,
        rolled_value: rolled,
        unit_suffix: "".to_string(),
      }
    }

    #[test]
    fn it_returns_zero_for_empty_stats() {
      let item = make_item(vec![]);

      assert_eq!(roll_score(&item), 0.0);
    }

    #[test]
    fn it_returns_positive_for_good_rolls() {
      let stats = vec![
        make_stat("Damage", 100.0, 110.0, true),
        make_stat("CPU Use", 50.0, 45.0, false),
      ];
      let item = make_item(stats);

      let score = roll_score(&item);

      assert!(score > 0.0);
    }

    #[test]
    fn it_returns_negative_for_bad_rolls() {
      let stats = vec![
        make_stat("Damage", 100.0, 90.0, true),
        make_stat("CPU Use", 50.0, 55.0, false),
      ];
      let item = make_item(stats);

      let score = roll_score(&item);

      assert!(score < 0.0);
    }
  }

  mod format_stat_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_percentage_trimming_trailing_zero() {
      assert_eq!(format_stat_value(35.5, "%"), "35.5%");
    }

    #[test]
    fn it_formats_large_whole_values_without_decimals() {
      assert_eq!(format_stat_value(50_000.0, " kg"), "50000 kg");
    }

    #[test]
    fn it_formats_medium_whole_values_without_decimals() {
      assert_eq!(format_stat_value(1_500.0, " HP"), "1500 HP");
    }

    #[test]
    fn it_formats_values_trimming_trailing_zero() {
      assert_eq!(format_stat_value(25.5, " tf"), "25.5 tf");
    }

    #[test]
    fn it_formats_values_with_two_significant_decimals() {
      assert_eq!(format_stat_value(4.75, " GJ"), "4.75 GJ");
    }
  }
}
