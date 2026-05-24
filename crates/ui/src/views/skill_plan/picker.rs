//! Skill picker: three-tab accordion for adding skills, ships, and modules to a plan.

use std::collections::{HashMap, HashSet};

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, scrollable, text, text_input},
};
use pod_model::{Certificate, ItemTypeSummary};

use super::Message;

static EMPTY_MASTERY_MAP: std::sync::LazyLock<HashMap<i32, u8>> = std::sync::LazyLock::new(HashMap::new);

use crate::{
  components,
  components::tab_strip::{Component as TabStrip, TabItem},
  style::{
    color, spacing,
    typography::{body, mono},
  },
  views::skills::skill_data::{SkillDef, SkillGroupDef},
};

pub struct SkillPicker<'a> {
  groups: &'a [SkillGroupDef],
  planned_levels: &'a HashMap<String, u8>,
  search_query: &'a str,
  expanded_groups: &'a HashSet<String>,
  picker_tab: usize,
  ships: &'a [ItemTypeSummary],
  modules: &'a [ItemTypeSummary],
  certs: &'a [Certificate],
  ship_mastery_selection: &'a HashMap<i32, u8>,
  cert_proficiency_selection: &'a HashMap<i32, u8>,
  ships_loaded: bool,
  modules_loaded: bool,
  certs_loaded: bool,
}

impl<'a> SkillPicker<'a> {
  pub fn new(
    groups: &'a [SkillGroupDef],
    planned_levels: &'a HashMap<String, u8>,
    search_query: &'a str,
    expanded_groups: &'a HashSet<String>,
  ) -> Self {
    Self {
      groups,
      planned_levels,
      search_query,
      expanded_groups,
      picker_tab: 0,
      ships: &[],
      modules: &[],
      certs: &[],
      ship_mastery_selection: &EMPTY_MASTERY_MAP,
      cert_proficiency_selection: &EMPTY_MASTERY_MAP,
      ships_loaded: false,
      modules_loaded: false,
      certs_loaded: false,
    }
  }

  pub fn tab(mut self, tab: usize) -> Self {
    self.picker_tab = tab;
    self
  }

  pub fn ships(mut self, ships: &'a [ItemTypeSummary], mastery_selection: &'a HashMap<i32, u8>, loaded: bool) -> Self {
    self.ships = ships;
    self.ship_mastery_selection = mastery_selection;
    self.ships_loaded = loaded;
    self
  }

  pub fn modules(mut self, modules: &'a [ItemTypeSummary], loaded: bool) -> Self {
    self.modules = modules;
    self.modules_loaded = loaded;
    self
  }

  pub fn certs(mut self, certs: &'a [Certificate], proficiency: &'a HashMap<i32, u8>, loaded: bool) -> Self {
    self.certs = certs;
    self.cert_proficiency_selection = proficiency;
    self.certs_loaded = loaded;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let tabs = TabStrip::new(vec![
      TabItem {
        label: "Skills".to_string(),
        count: None,
      },
      TabItem {
        label: "Ships".to_string(),
        count: None,
      },
      TabItem {
        label: "Modules".to_string(),
        count: None,
      },
      TabItem {
        label: "Certs".to_string(),
        count: None,
      },
    ])
    .active(self.picker_tab)
    .render(Message::PickerTabChanged);

    let body: Element<'_, Message> = match self.picker_tab {
      1 => self.ships_tab(),
      2 => self.modules_tab(),
      3 => self.certs_tab(),
      _ => self.skills_tab(),
    };

    let content = column([tabs, body]).height(Length::Fill).width(Length::Fill);

    container(content)
      .width(Length::Fill)
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::SUBTLE,
          radius: 0.0.into(),
          width: 0.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn skills_tab(self) -> Element<'a, Message> {
    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();

    let mut items: Vec<Element<'_, Message>> = vec![search_bar(self.search_query, "Search skills\u{2026}")];

    for group in self.groups {
      let filtered: Vec<&SkillDef> = if searching {
        group
          .skills
          .iter()
          .filter(|s| s.name.to_lowercase().contains(&lc))
          .collect()
      } else {
        group.skills.iter().collect()
      };

      if filtered.is_empty() {
        continue;
      }

      let is_expanded = searching || self.expanded_groups.contains(&*group.name);
      let trained_count = group.skills.iter().filter(|s| s.level >= 5).count();
      items.push(group_header_row(
        &group.name,
        is_expanded,
        trained_count,
        group.skills.len(),
      ));

      if is_expanded {
        for skill in &filtered {
          let planned = self.planned_levels.get(skill.name.as_str()).copied().unwrap_or(0);
          items.push(skill_row(skill, planned));
        }
      }
    }

    items.push(Space::new().height(12.0).into());
    let content = column(items).width(Length::Fill);
    scrollable(content).height(Length::Fill).width(Length::Fill).into()
  }

  fn ships_tab(self) -> Element<'a, Message> {
    if !self.ships_loaded {
      return loading_placeholder("Loading ships\u{2026}");
    }

    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();
    let mut items: Vec<Element<'_, Message>> = vec![search_bar(self.search_query, "Search ships\u{2026}")];

    let mut groups: Vec<(&str, Vec<&ItemTypeSummary>)> = Vec::new();
    for ship in self.ships {
      if searching && !ship.name.to_lowercase().contains(&lc) {
        continue;
      }
      match groups.iter_mut().find(|(g, _)| *g == ship.group_name.as_str()) {
        Some((_, ships)) => ships.push(ship),
        None => groups.push((ship.group_name.as_str(), vec![ship])),
      }
    }

    for (group_name, ships) in groups {
      let is_expanded = searching || self.expanded_groups.contains(group_name);
      items.push(dyn_group_header(group_name, ships.len(), is_expanded));
      if is_expanded {
        for ship in ships {
          let mastery = self.ship_mastery_selection.get(&ship.id).copied().unwrap_or(1);
          items.push(ship_row(ship, mastery));
        }
      }
    }

    items.push(Space::new().height(12.0).into());
    let content = column(items).width(Length::Fill);
    scrollable(content).height(Length::Fill).width(Length::Fill).into()
  }

  fn modules_tab(self) -> Element<'a, Message> {
    if !self.modules_loaded {
      return loading_placeholder("Loading modules\u{2026}");
    }

    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();
    let mut items: Vec<Element<'_, Message>> = vec![search_bar(self.search_query, "Search modules\u{2026}")];

    let mut groups: Vec<(&str, Vec<&ItemTypeSummary>)> = Vec::new();
    for module in self.modules {
      if searching && !module.name.to_lowercase().contains(&lc) {
        continue;
      }
      match groups.iter_mut().find(|(g, _)| *g == module.group_name.as_str()) {
        Some((_, mods)) => mods.push(module),
        None => groups.push((module.group_name.as_str(), vec![module])),
      }
    }

    for (group_name, mods) in groups {
      let is_expanded = searching || self.expanded_groups.contains(group_name);
      items.push(dyn_group_header(group_name, mods.len(), is_expanded));
      if is_expanded {
        for module in mods {
          items.push(module_row(module));
        }
      }
    }

    items.push(Space::new().height(12.0).into());
    let content = column(items).width(Length::Fill);
    scrollable(content).height(Length::Fill).width(Length::Fill).into()
  }

  fn certs_tab(self) -> Element<'a, Message> {
    if !self.certs_loaded {
      return loading_placeholder("Loading certificates\u{2026}");
    }

    let lc = self.search_query.trim().to_lowercase();
    let searching = !lc.is_empty();
    let mut items: Vec<Element<'_, Message>> = vec![search_bar(self.search_query, "Search certificates\u{2026}")];

    for cert in self.certs {
      if searching && !cert.name.to_lowercase().contains(&lc) {
        continue;
      }
      let prof = self.cert_proficiency_selection.get(&cert.id).copied().unwrap_or(3);
      items.push(cert_row(cert, prof));
    }

    items.push(Space::new().height(12.0).into());
    scrollable(column(items).width(Length::Fill))
      .height(Length::Fill)
      .width(Length::Fill)
      .into()
  }
}

fn loading_placeholder<'a>(label: &'static str) -> Element<'a, Message> {
  container(
    text(label)
      .font(body::REGULAR)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding::from([24.0, 16.0]))
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn search_bar<'a>(query: &'a str, placeholder: &'static str) -> Element<'a, Message> {
  let search_row = container(
    row([
      components::Icon::search()
        .size(14.0)
        .color(color::text::SECONDARY)
        .render::<Message>(),
      search_input(query, placeholder),
    ])
    .spacing(10.0)
    .align_y(Vertical::Center),
  )
  .height(36.0)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    bottom: 0.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 8.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  container(search_row)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn search_input<'a>(query: &'a str, placeholder: &'static str) -> Element<'a, Message> {
  text_input(placeholder, query)
    .on_input(Message::PickerSearchChanged)
    .padding(Padding::ZERO)
    .size(13.0)
    .font(body::REGULAR)
    .style(|_, _| iced::widget::text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border::default(),
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::accent::PLASMA_SUBTLE,
    })
    .into()
}

fn group_header_row(
  name: &str,
  is_expanded: bool,
  trained_count: usize,
  total_skills: usize,
) -> Element<'static, Message> {
  let caret = if is_expanded { "\u{25bc}" } else { "\u{25b6}" };
  let rule = picker_separator();
  let count_label = format!("{}/{}", trained_count, total_skills);

  let btn = group_btn(name, caret, count_label);
  column([rule.into(), btn.into()]).into()
}

fn dyn_group_header<'a>(name: &str, count: usize, is_expanded: bool) -> Element<'a, Message> {
  let caret = if is_expanded { "\u{25bc}" } else { "\u{25b6}" };
  let rule = picker_separator();
  let count_label = count.to_string();

  let btn = group_btn(name, caret, count_label);
  column([rule.into(), btn.into()]).into()
}

fn picker_separator<'a>() -> iced::widget::Container<'a, Message> {
  container(Space::new().width(Length::Fill).height(1.0))
    .width(Length::Fill)
    .height(1.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
}

fn group_btn(name: &str, caret: &str, count_label: String) -> button::Button<'static, Message> {
  button(
    row([
      text(caret.to_string())
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().width(10.0).into(),
      text(name.to_string())
        .font(body::MEDIUM)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      text(count_label)
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(Message::PickerGroupToggled(name.to_string()))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: iced::Border::default(),
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
}

fn skill_row(skill: &SkillDef, planned_level: u8) -> Element<'static, Message> {
  let pip_row = skill_pip_row(skill, planned_level);

  let row_content = container(
    row([
      text(skill.name.clone())
        .font(body::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill)
        .into(),
      text(format!("\u{d7}{}", skill.rank))
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      Space::new().width(spacing::SPACE_2).into(),
      pip_row.into(),
    ])
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 30.0,
    right: spacing::SPACE_3,
  })
  .width(Length::Fill);

  column([picker_separator().into(), row_content.into()]).into()
}

fn skill_pip_row(skill: &SkillDef, planned_level: u8) -> iced::widget::Row<'static, Message> {
  let pips: Vec<Element<'_, Message>> = (1u8..=5)
    .map(|lv| {
      let trained = lv <= skill.level;
      let planned = !trained && lv <= planned_level;
      let (bg, border_col) = skill_pip_colors(trained, planned);
      let pip = skill_pip(bg, border_col);

      if trained {
        pip.into()
      } else {
        button(pip)
          .padding(0)
          .on_press(Message::SkillPicked(skill.name.to_string(), lv))
          .style(|_, status| button::Style {
            background: match status {
              button::Status::Hovered | button::Status::Pressed => {
                Some(Background::Color(color::accent::PLASMA_ACTIVE))
              }
              _ => None,
            },
            border: Border {
              color: Color::TRANSPARENT,
              radius: 1.5.into(),
              width: 0.0,
            },
            ..button::Style::default()
          })
          .into()
      }
    })
    .collect();

  row(pips).spacing(3.0).align_y(Vertical::Center)
}

fn skill_pip_colors(trained: bool, planned: bool) -> (Color, Color) {
  if trained {
    (color::text::PRIMARY, color::text::PRIMARY)
  } else if planned {
    (color::accent::PLASMA_MUTED, color::accent::PLASMA_HALF)
  } else {
    (Color::TRANSPARENT, color::border::SUBTLE)
  }
}

fn skill_pip(bg: Color, border_col: Color) -> iced::widget::Container<'static, Message> {
  container(Space::new())
    .width(10.0)
    .height(8.0)
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: border_col,
        radius: 1.5.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
}

fn ship_row<'a>(ship: &'a ItemTypeSummary, selected_mastery: u8) -> Element<'a, Message> {
  let type_id = ship.id;
  let ship_name = ship.name.clone();

  let name_el = text(ship.name.clone())
    .font(body::REGULAR)
    .size(13.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .width(Length::Fill);

  let add_btn = item_add_btn(Message::ShipSelected(type_id, ship_name, selected_mastery));
  let chips = mastery_chips(type_id, selected_mastery);

  let row_children: Vec<Element<'_, Message>> = vec![
    name_el.into(),
    row(chips).spacing(2.0).align_y(Vertical::Center).into(),
    Space::new().width(spacing::SPACE_2).into(),
    add_btn.into(),
  ];

  let row_content = container(row(row_children).align_y(Vertical::Center).spacing(spacing::SPACE_2))
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 30.0,
      right: spacing::SPACE_3,
    })
    .width(Length::Fill);

  column([picker_separator().into(), row_content.into()]).into()
}

fn module_row<'a>(module: &'a ItemTypeSummary) -> Element<'a, Message> {
  let type_id = module.id;
  let mod_name = module.name.clone();

  let row_content = container(
    button(
      text(module.name.clone())
        .font(body::REGULAR)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: 7.0,
      bottom: 7.0,
      left: 30.0,
      right: spacing::SPACE_3,
    })
    .on_press(Message::ModuleSelected(type_id, mod_name))
    .style(|_, status| button::Style {
      background: match status {
        button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
        _ => None,
      },
      border: Border::default(),
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }),
  )
  .width(Length::Fill);

  column([picker_separator().into(), row_content.into()]).into()
}

fn cert_row<'a>(cert: &'a Certificate, selected_prof: u8) -> Element<'a, Message> {
  let cert_id = cert.id;
  let cert_name = cert.name.clone();

  let name_el = text(cert.name.clone())
    .font(body::REGULAR)
    .size(13.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    })
    .width(Length::Fill);

  let add_btn = item_add_btn(Message::CertSelected(cert_id, cert_name, selected_prof));
  let chips = prof_chips(cert_id, selected_prof);

  let row_content = container(
    row([
      name_el.into(),
      row(chips).spacing(2.0).align_y(Vertical::Center).into(),
      Space::new().width(spacing::SPACE_2).into(),
      add_btn.into(),
    ])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: 30.0,
    right: spacing::SPACE_3,
  })
  .width(Length::Fill);

  column([picker_separator().into(), row_content.into()]).into()
}

fn item_add_btn(on_press: Message) -> button::Button<'static, Message> {
  button(
    text("Add")
      .font(body::MEDIUM)
      .size(11.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 8.0,
    right: 8.0,
  })
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::accent::PLASMA_HIGHLIGHT)),
      _ => Some(Background::Color(Color::TRANSPARENT)),
    },
    border: Border {
      color: color::accent::PLASMA,
      radius: 4.0.into(),
      width: 1.0,
    },
    text_color: color::accent::PLASMA,
    ..button::Style::default()
  })
}

fn mastery_chips(type_id: i32, selected_mastery: u8) -> Vec<Element<'static, Message>> {
  ["I", "II", "III", "IV", "V"]
    .into_iter()
    .enumerate()
    .map(|(i, label)| {
      let lv = (i + 1) as u8;
      let is_active = lv == selected_mastery;
      level_chip(label, is_active, Message::ShipMasteryChanged(type_id, lv))
    })
    .collect()
}

fn prof_chips(cert_id: i32, selected_prof: u8) -> Vec<Element<'static, Message>> {
  ["Basic", "Std", "Adv", "Elite"]
    .into_iter()
    .enumerate()
    .map(|(i, label)| {
      let prof = i as u8;
      let is_active = prof == selected_prof;
      level_chip(label, is_active, Message::CertProficiencyChanged(cert_id, prof))
    })
    .collect()
}

fn level_chip(label: &'static str, is_active: bool, on_press: Message) -> Element<'static, Message> {
  button(
    text(label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(move |_| iced::widget::text::Style {
        color: Some(if is_active {
          color::accent::PLASMA
        } else {
          color::text::SECONDARY
        }),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 5.0,
    right: 5.0,
  })
  .on_press(on_press)
  .style(move |_, status| button::Style {
    background: match (is_active, status) {
      (true, _) => Some(Background::Color(color::accent::PLASMA_ACTIVE)),
      (false, button::Status::Hovered | button::Status::Pressed) => {
        Some(Background::Color(color::state::HOVER_OVERLAY))
      }
      _ => None,
    },
    border: Border {
      color: if is_active {
        color::accent::PLASMA
      } else {
        color::border::SUBTLE
      },
      radius: 3.0.into(),
      width: 1.0,
    },
    text_color: if is_active {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    },
    ..button::Style::default()
  })
  .into()
}
