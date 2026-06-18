use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use crate::{
  features::assets::{Message, State},
  ui::{
    components::rule,
    style::{color, radius, spacing, typography},
  },
};

const COLUMN_GAP: f32 = 44.0;

const SECTION_GAP: f32 = 24.0;

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

static MODAL_SOURCE_PATTERNS: &[(i64, &str)] = &[
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

struct ModalEntry {
  label: &'static str,
  type_id: i64,
}

enum ModalRow {
  Family {
    name: &'static str,
    variants: &'static [ModalEntry],
  },
  Single {
    label: &'static str,
    type_id: i64,
  },
}

struct ModalSection {
  rows: &'static [ModalRow],
  title: &'static str,
}

pub(in crate::features::assets) fn modal(state: &State) -> Element<'_, Message> {
  let selected = state.abyssal_filters().source_type_id;

  let columns: Vec<Element<'_, Message>> = MODAL_LAYOUT
    .iter()
    .map(|sections| modal_column(sections, selected))
    .collect();

  let mut body_row: Vec<Element<'_, Message>> = Vec::new();
  for (index, column) in columns.into_iter().enumerate() {
    if index > 0 {
      body_row.push(Space::new().width(COLUMN_GAP).into());
    }
    body_row.push(column);
  }

  let body = scrollable(
    container(Row::with_children(body_row))
      .padding(Padding {
        top: spacing::SPACE_6,
        right: spacing::SPACE_6 + spacing::SPACE_2,
        bottom: spacing::SPACE_6 + spacing::UNIT,
        left: spacing::SPACE_6 + spacing::SPACE_2,
      })
      .width(Length::Fill),
  )
  .style(crate::ui::style::control::scrollbar)
  .height(Length::Fill);

  let panel = container(
    Column::with_children(vec![
      header(selected),
      rule::horizontal(),
      body.into(),
      rule::horizontal(),
      footer(),
    ])
    .width(Length::Fill)
    .height(Length::Fill),
  )
  .max_width(1180.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.15),
      radius: radius::PANEL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  container(panel).center(Length::Fill).padding(spacing::SPACE_6).into()
}

pub(in crate::features::assets) fn modal_selected_label(type_id: i64) -> Option<String> {
  for sections in MODAL_LAYOUT {
    for section in *sections {
      for row in section.rows {
        match row {
          ModalRow::Single {
            label,
            type_id: tid,
          } if *tid == type_id => return Some((*label).to_owned()),
          ModalRow::Family {
            name,
            variants,
          } => {
            for variant in *variants {
              if variant.type_id == type_id {
                return Some(format!("{name} ({})", variant.label));
              }
            }
          }
          ModalRow::Single {
            ..
          } => {}
        }
      }
    }
  }
  None
}

fn header(selected: Option<i64>) -> Element<'static, Message> {
  let subtitle = selected
    .and_then(modal_selected_label)
    .unwrap_or_else(|| "Pick a module type".to_owned());

  let mut items: Vec<Element<'static, Message>> = vec![
    Column::with_children(vec![
      text("Filter by module type")
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(spacing::UNIT / 2.0).into(),
      text(subtitle)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    ])
    .width(Length::Fill)
    .into(),
  ];

  if selected.is_some() {
    items.push(
      button(
        text("Clear")
          .font(typography::body::REGULAR)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          }),
      )
      .padding(Padding {
        top: spacing::UNIT + 1.0,
        right: spacing::SPACE_2_5,
        bottom: spacing::UNIT + 1.0,
        left: spacing::SPACE_2_5,
      })
      .on_press(Message::AbyssalSourceTypeSelected(None))
      .style(|_, _| button::Style {
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, 0.12),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: color::text::secondary(),
        ..button::Style::default()
      })
      .into(),
    );
    items.push(Space::new().width(spacing::SPACE_2).into());
  }

  items.push(
    button(
      text("\u{00d7}")
        .font(typography::mono::REGULAR)
        .size(typography::size::LG)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    )
    .on_press(Message::AbyssalTypeModalClosed)
    .style(|_, _| button::Style {
      text_color: color::text::secondary(),
      ..button::Style::default()
    })
    .into(),
  );

  container(Row::with_children(items).align_y(Vertical::Center))
    .padding(spacing::SPACE_3_5)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn footer() -> Element<'static, Message> {
  container(
    Row::with_children(vec![
      text("esc \u{00b7} close")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .width(Length::Fill)
        .into(),
      button(
        text("Done")
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::surface::BASE),
          }),
      )
      .padding(Padding {
        top: spacing::SPACE_2,
        right: spacing::SPACE_3 + spacing::UNIT,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3 + spacing::UNIT,
      })
      .on_press(Message::AbyssalTypeModalClosed)
      .style(|_, _| button::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        border: Border {
          radius: radius::CONTROL.into(),
          ..Border::default()
        },
        text_color: color::surface::BASE,
        ..button::Style::default()
      })
      .into(),
    ])
    .align_y(Vertical::Center),
  )
  .padding(spacing::SPACE_3)
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn modal_column(sections: &'static [ModalSection], selected: Option<i64>) -> Element<'static, Message> {
  let blocks: Vec<Element<'static, Message>> = sections
    .iter()
    .map(|section| {
      container(modal_section(section, selected))
        .padding(Padding {
          bottom: SECTION_GAP,
          ..Padding::ZERO
        })
        .into()
    })
    .collect();

  Column::with_children(blocks).width(Length::Fill).into()
}

fn modal_section(section: &'static ModalSection, selected: Option<i64>) -> Element<'static, Message> {
  let title = Column::with_children(vec![
    text(section.title)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
    Space::new().height(spacing::UNIT + 2.0).into(),
    rule::horizontal(),
    Space::new().height(spacing::SPACE_2).into(),
  ])
  .width(Length::Fill);

  let rows: Vec<Element<'static, Message>> = section
    .rows
    .iter()
    .map(|row| match row {
      ModalRow::Single {
        label,
        type_id,
      } => single_row(label, *type_id, selected == Some(*type_id)),
      ModalRow::Family {
        name,
        variants,
      } => family_row(name, variants, selected),
    })
    .collect();

  Column::with_children(vec![
    title.into(),
    Column::with_children(rows)
      .spacing(spacing::UNIT / 2.0)
      .width(Length::Fill)
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn single_row(label: &'static str, type_id: i64, selected: bool) -> Element<'static, Message> {
  let (background, text_color, border_color) = if selected {
    (
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
      color::accent::PLASMA,
      color::accent::PLASMA,
    )
  } else {
    (None, color::text::PRIMARY, Color::TRANSPARENT)
  };

  button(
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(text_color),
      })
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  })
  .on_press(Message::AbyssalSourceTypeSelected(Some(type_id)))
  .style(move |_, _| button::Style {
    background,
    border: Border {
      color: border_color,
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn family_row(name: &'static str, variants: &'static [ModalEntry], selected: Option<i64>) -> Element<'static, Message> {
  let any_selected = variants.iter().any(|variant| selected == Some(variant.type_id));
  let (background, border_color) = if any_selected {
    (
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
      color::with_alpha(color::accent::PLASMA, 0.3),
    )
  } else {
    (None, Color::TRANSPARENT)
  };

  let chips: Vec<Element<'static, Message>> = variants
    .iter()
    .map(|variant| variant_chip(variant.label, variant.type_id, selected == Some(variant.type_id)))
    .collect();

  container(
    Column::with_children(vec![
      text(name)
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(spacing::SPACE_2).into(),
      Row::with_children(chips).spacing(spacing::UNIT).wrap().into(),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  })
  .style(move |_| container::Style {
    background,
    border: Border {
      color: border_color,
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn variant_chip(label: &'static str, type_id: i64, selected: bool) -> Element<'static, Message> {
  let (background, border_color, text_color) = if selected {
    (
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.14))),
      color::accent::PLASMA,
      color::accent::PLASMA,
    )
  } else {
    (
      Some(Background::Color(color::surface::BASE)),
      color::with_alpha(color::text::PRIMARY, 0.12),
      color::text::secondary(),
    )
  };

  button(
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(text_color),
      }),
  )
  .padding(Padding {
    top: spacing::UNIT,
    right: spacing::SPACE_2,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2,
  })
  .on_press(Message::AbyssalSourceTypeSelected(Some(type_id)))
  .style(move |_, _| button::Style {
    background,
    border: Border {
      color: border_color,
      radius: radius::SUBTLE.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::assets::{State, abyssals::Filters};

  mod modal {
    use super::*;

    #[test]
    fn it_renders_the_catalog_with_a_family_variant_selected() {
      let mut state = State::new();
      let filters = Filters {
        source_type_id: Some(47785),
        ..Filters::default()
      };
      state.set_abyssals_for_test(vec![], vec![], filters, true);

      let _el: Element<'_, Message> = modal(&state);
    }

    #[test]
    fn it_renders_the_catalog_with_a_single_row_selected() {
      let mut state = State::new();
      let filters = Filters {
        source_type_id: Some(47702),
        ..Filters::default()
      };
      state.set_abyssals_for_test(vec![], vec![], filters, true);

      let _el: Element<'_, Message> = modal(&state);
    }

    #[test]
    fn it_renders_the_catalog_without_a_selection() {
      let state = State::new();

      let _el: Element<'_, Message> = modal(&state);
    }
  }

  mod modal_selected_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_qualifies_a_family_variant_with_its_size() {
      assert_eq!(modal_selected_label(47785), Some("Shield Booster (Medium)".to_owned()));
    }

    #[test]
    fn it_returns_none_for_an_unknown_type() {
      assert_eq!(modal_selected_label(1), None);
    }

    #[test]
    fn it_returns_the_label_for_a_single_row_type() {
      assert_eq!(modal_selected_label(47702), Some("Stasis Webifier".to_owned()));
    }
  }
}
