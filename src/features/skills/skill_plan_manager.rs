use iced::{
  Background, Border, ContentFit, Element, Length, Padding, Task,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, button, container, image, scrollable, text},
};

use crate::{
  store::{
    Database, images,
    repo::{character, org, skills},
  },
  ui::{
    components::{avatar::avatar, chip::chip, clip::clip_layer, eyebrow::eyebrow_text, header, icon::Icon},
    style::{color, radius, spacing, typography},
  },
};

pub const MANAGE_PLANS_WINDOW_HEIGHT: f32 = 620.0;
pub const MANAGE_PLANS_WINDOW_WIDTH: f32 = 940.0;

const RAIL_WIDTH: f32 = 256.0;
const RAIL_PORTRAIT: f32 = 32.0;
const DETAIL_PORTRAIT: f32 = 36.0;

#[derive(Clone, Debug)]
pub enum Message {
  CancelDelete,
  CharacterSelected(i64),
  ConfirmDelete(i64),
  CopyPlan { plan_id: i64, target_character_id: i64 },
  Loaded(Box<Roster>),
  NewPlan(i64),
  OpenPlan { character_id: i64, plan_id: i64 },
  RequestDelete(i64),
  ToggleCopyMenu(i64),
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlanRow {
  pub edited: String,
  pub entry_count: usize,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct Roster {
  pub entries: Vec<RosterEntry>,
}

impl Roster {
  pub fn plan_total(&self) -> usize {
    self.entries.iter().map(|entry| entry.plans.len()).sum()
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self
      .entries
      .iter()
      .filter_map(|entry| entry.portrait.stale_key())
      .collect()
  }
}

#[derive(Clone, Debug)]
pub struct RosterEntry {
  pub character_id: i64,
  pub corp: String,
  pub name: String,
  pub plans: Vec<PlanRow>,
  pub portrait: images::ImageState,
}

#[derive(Debug)]
pub struct State {
  confirm_delete: Option<i64>,
  copy_menu: Option<i64>,
  roster: Roster,
  selected: Option<i64>,
}

impl State {
  pub fn new() -> Self {
    State {
      confirm_delete: None,
      copy_menu: None,
      roster: Roster::default(),
      selected: None,
    }
  }

  pub fn arm_delete(&mut self, plan_id: i64) {
    self.confirm_delete = Some(plan_id);
    self.copy_menu = None;
  }

  pub fn clear_delete(&mut self) {
    self.confirm_delete = None;
  }

  pub fn close_copy_menu(&mut self) {
    self.copy_menu = None;
  }

  pub fn confirm_delete(&self) -> Option<i64> {
    self.confirm_delete
  }

  pub fn copy_menu(&self) -> Option<i64> {
    self.copy_menu
  }

  pub fn copy_targets(&self, source_character_id: i64) -> Vec<&RosterEntry> {
    self
      .roster
      .entries
      .iter()
      .filter(|entry| entry.character_id != source_character_id)
      .collect()
  }

  pub fn entries(&self) -> &[RosterEntry] {
    &self.roster.entries
  }

  pub fn select(&mut self, character_id: i64) {
    if self
      .roster
      .entries
      .iter()
      .any(|entry| entry.character_id == character_id)
    {
      self.selected = Some(character_id);
      self.confirm_delete = None;
      self.copy_menu = None;
    }
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub fn selected(&self) -> Option<i64> {
    self.selected
  }

  pub fn set_roster(&mut self, roster: Roster) {
    self.roster = roster;
    let selected_still_present = self
      .selected
      .is_some_and(|id| self.roster.entries.iter().any(|entry| entry.character_id == id));
    if !selected_still_present {
      self.selected = self.default_selection();
    }
    let plan_ids: Vec<i64> = self
      .roster
      .entries
      .iter()
      .flat_map(|entry| entry.plans.iter().map(|plan| plan.id))
      .collect();
    if self.confirm_delete.is_some_and(|id| !plan_ids.contains(&id)) {
      self.confirm_delete = None;
    }
    if self.copy_menu.is_some_and(|id| !plan_ids.contains(&id)) {
      self.copy_menu = None;
    }
  }

  pub fn toggle_copy_menu(&mut self, plan_id: i64) {
    self.copy_menu = if self.copy_menu == Some(plan_id) {
      None
    } else {
      self.confirm_delete = None;
      Some(plan_id)
    };
  }

  pub fn stale_images(&self) -> Vec<(images::ImageKind, i64)> {
    self.roster.stale_images()
  }

  fn default_selection(&self) -> Option<i64> {
    self
      .roster
      .entries
      .iter()
      .find(|entry| !entry.plans.is_empty())
      .or_else(|| self.roster.entries.first())
      .map(|entry| entry.character_id)
  }

  fn selected_entry(&self) -> Option<&RosterEntry> {
    let id = self.selected?;
    self.roster.entries.iter().find(|entry| entry.character_id == id)
  }
}

impl Default for State {
  fn default() -> Self {
    State::new()
  }
}

pub fn load(db: &Database) -> Task<Message> {
  let db = db.clone();
  Task::perform(async move { Box::new(load_roster(&db).await) }, Message::Loaded)
}

pub async fn load_roster(db: &Database) -> Roster {
  let owned = character::all_owned(db).await.unwrap_or_default();

  let mut entries = Vec::with_capacity(owned.len());
  for character in owned {
    let corp = org::get_corporation(db, character.corporation_id())
      .await
      .ok()
      .flatten()
      .map(|corp| corp.ticker().to_owned())
      .unwrap_or_default();
    let portrait = images::resolve(
      &images::default_store(),
      images::ImageKind::CharacterPortrait,
      character.id(),
    );
    let plans = load_plan_rows(db, character.id()).await;

    entries.push(RosterEntry {
      character_id: character.id(),
      corp,
      name: character.name().to_owned(),
      plans,
      portrait,
    });
  }

  entries.sort_by_key(|entry| entry.name.to_lowercase());
  Roster {
    entries,
  }
}

async fn load_plan_rows(db: &Database, character_id: i64) -> Vec<PlanRow> {
  let plans = skills::for_character(db, character_id).await.unwrap_or_default();

  let mut rows = Vec::with_capacity(plans.len());
  for plan in plans {
    let entry_count = skills::entries(db, plan.id())
      .await
      .map(|entries| entries.len())
      .unwrap_or(0);
    rows.push(PlanRow {
      edited: relative_time(plan.updated_at()),
      entry_count,
      id: plan.id(),
      name: plan.name().to_owned(),
    });
  }
  rows
}

pub fn view(state: &State) -> Element<'_, Message> {
  window_body(state)
}

fn window_body(state: &State) -> Element<'_, Message> {
  let body = Row::with_children(vec![rail(state), detail(state)])
    .width(Length::Fill)
    .height(Length::Fill);

  container(
    Column::with_children(vec![header(state), body.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::BASE)),
    ..container::Style::default()
  })
  .into()
}

fn header(state: &State) -> Element<'_, Message> {
  let total = state.roster.plan_total();
  let characters = state.roster.entries.len();
  let plan_word = if total == 1 {
    t!("skills.manager.plan_singular")
  } else {
    t!("skills.manager.plan_plural")
  };
  let char_word = if characters == 1 {
    t!("skills.manager.character_singular")
  } else {
    t!("skills.manager.character_plural")
  };
  let summary = t!(
    "skills.manager.header_summary",
    plan_count => total,
    plan_word => plan_word,
    char_count => characters,
    char_word => char_word
  );

  let info = Column::with_children(vec![
    text(t!("skills.manager.title").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(summary.into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT);

  header::header(vec![info.into()], Vec::new())
}

fn rail(state: &State) -> Element<'_, Message> {
  let mut items: Vec<Element<'_, Message>> = vec![
    container(eyebrow_text(
      t!("skills.manager.characters").as_ref(),
      Some(color::text::tertiary()),
    ))
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3_5,
    })
    .into(),
  ];
  for entry in &state.roster.entries {
    items.push(rail_item(entry, state.selected == Some(entry.character_id)));
  }

  container(
    scrollable(Column::with_children(items).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fill),
  )
  .width(Length::Fixed(RAIL_WIDTH))
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.06),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn rail_item(entry: &RosterEntry, active: bool) -> Element<'_, Message> {
  let mut lines: Vec<Element<'_, Message>> = vec![
    text(entry.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if !entry.corp.is_empty() {
    lines.push(
      text(entry.corp.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let row = Row::with_children(vec![
    portrait_tile(&entry.portrait, &entry.name, RAIL_PORTRAIT),
    Column::with_children(lines).spacing(2.0).width(Length::Fill).into(),
    chip(entry.plans.len().to_string(), count_tint(entry.plans.len(), active)),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  button(container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5 - 2.0,
  }))
  .padding(0.0)
  .on_press(Message::CharacterSelected(entry.character_id))
  .style(move |_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = if active {
      Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.10)))
    } else if hover {
      Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.04)))
    } else {
      None
    };
    button::Style {
      background,
      border: Border {
        color: if active {
          color::accent::PLASMA
        } else {
          iced::Color::TRANSPARENT
        },
        width: 0.0,
        radius: 0.0.into(),
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn detail(state: &State) -> Element<'_, Message> {
  let Some(entry) = state.selected_entry() else {
    return container(
      text(t!("skills.manager.no_characters").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into();
  };

  let body = container(
    scrollable(detail_plans(state, entry))
      .style(crate::ui::style::control::scrollbar)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill);

  container(
    Column::with_children(vec![detail_header(entry), body.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn detail_header(entry: &RosterEntry) -> Element<'_, Message> {
  let mut lines: Vec<Element<'_, Message>> = vec![
    text(entry.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD + 2.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if !entry.corp.is_empty() {
    lines.push(
      text(entry.corp.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    );
  }

  let identity = Row::with_children(vec![
    portrait_tile(&entry.portrait, &entry.name, DETAIL_PORTRAIT),
    Column::with_children(lines).spacing(2.0).width(Length::Fill).into(),
    new_plan_button(entry.character_id),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  container(identity)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5 + spacing::SPACE_2,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5 + spacing::SPACE_2,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.06),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn detail_plans<'a>(state: &'a State, entry: &'a RosterEntry) -> Element<'a, Message> {
  if entry.plans.is_empty() {
    return container(
      Column::with_children(vec![
        text(t!("skills.manager.no_plans_for", name => first_name(&entry.name)).into_owned())
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          })
          .into(),
        text(t!("skills.manager.no_plans_hint").into_owned())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          })
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(spacing::SPACE_3_5)
    .into();
  }

  let targets = state.copy_targets(entry.character_id);
  let cards: Vec<Element<'a, Message>> = entry
    .plans
    .iter()
    .map(|plan| {
      plan_card(
        plan,
        entry.character_id,
        state.confirm_delete() == Some(plan.id),
        state.copy_menu() == Some(plan.id),
        &targets,
      )
    })
    .collect();

  Column::with_children(cards)
    .spacing(spacing::SPACE_2_5)
    .padding(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn plan_card<'a>(
  plan: &PlanRow,
  character_id: i64,
  confirming_delete: bool,
  copy_menu_open: bool,
  targets: &[&RosterEntry],
) -> Element<'a, Message> {
  let count = plan.entry_count;
  let skill_word = if count == 1 {
    t!("skills.manager.skill_singular")
  } else {
    t!("skills.manager.skill_plural")
  };
  let meta = t!(
    "skills.manager.plan_meta",
    skill_count => count,
    skill_word => skill_word,
    edited => plan.edited
  )
  .into_owned();

  let info = Column::with_children(vec![
    text(plan.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(meta)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  let actions: Element<'a, Message> = if confirming_delete {
    Row::with_children(vec![
      text(t!("skills.manager.delete_confirm").into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        })
        .into(),
      ghost_button(t!("skills.manager.cancel").into_owned(), Message::CancelDelete),
      danger_button(
        t!("skills.manager.delete").into_owned(),
        Message::ConfirmDelete(plan.id),
      ),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
  } else {
    Row::with_children(vec![
      ghost_button(
        t!("skills.manager.open").into_owned(),
        Message::OpenPlan {
          character_id,
          plan_id: plan.id,
        },
      ),
      copy_to_button(plan.id, !targets.is_empty(), copy_menu_open),
      delete_button(plan.id),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
  };

  let row = container(
    Row::with_children(vec![info.into(), actions])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(spacing::SPACE_3)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.08),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  });

  if copy_menu_open && !targets.is_empty() {
    Column::with_children(vec![row.into(), copy_menu(plan.id, targets)])
      .spacing(spacing::SPACE_2)
      .width(Length::Fill)
      .into()
  } else {
    row.into()
  }
}

fn copy_menu<'a>(plan_id: i64, targets: &[&RosterEntry]) -> Element<'a, Message> {
  let mut items: Vec<Element<'a, Message>> = vec![
    container(eyebrow_text(
      t!("skills.manager.copy_to_character").as_ref(),
      Some(color::text::tertiary()),
    ))
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_3,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
    })
    .into(),
  ];
  for target in targets {
    items.push(copy_menu_item(plan_id, target));
  }

  container(Column::with_children(items).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn copy_menu_item<'a>(plan_id: i64, target: &RosterEntry) -> Element<'a, Message> {
  let mut lines: Vec<Element<'a, Message>> = vec![
    text(target.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];
  if !target.corp.is_empty() {
    lines.push(
      text(target.corp.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let row = Row::with_children(vec![
    portrait_tile(&target.portrait, &target.name, RAIL_PORTRAIT),
    Column::with_children(lines).spacing(2.0).width(Length::Fill).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let target_character_id = target.character_id;
  button(container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  }))
  .padding(0.0)
  .on_press(Message::CopyPlan {
    plan_id,
    target_character_id,
  })
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.04))),
      ..button::Style::default()
    }
  })
  .into()
}

fn new_plan_button<'a>(character_id: i64) -> Element<'a, Message> {
  button(
    text(t!("skills.manager.new_plan").into_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 7.0,
    right: spacing::SPACE_3,
    bottom: 7.0,
    left: spacing::SPACE_3,
  })
  .on_press(Message::NewPlan(character_id))
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(if hover {
        color::with_alpha(color::accent::PLASMA, 0.16)
      } else {
        color::with_alpha(color::accent::PLASMA, 0.10)
      })),
      border: Border {
        color: color::accent::PLASMA_MUTED,
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  })
  .into()
}

fn ghost_button<'a>(label: String, message: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 6.0,
    right: spacing::SPACE_2_5,
    bottom: 6.0,
    left: spacing::SPACE_2_5,
  })
  .on_press(message)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if active { 0.25 } else { 0.1 }),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      text_color: if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn copy_to_button<'a>(plan_id: i64, enabled: bool, menu_open: bool) -> Element<'a, Message> {
  let label_color = if enabled {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };
  let label = button(
    Row::with_children(vec![
      text(t!("skills.manager.copy_to").into_owned())
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(move |_| copy_button_label_style(enabled))
        .into(),
      Icon::chevron_down().size(13.0).color(label_color).render(),
    ])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 6.0,
    right: spacing::SPACE_2_5,
    bottom: 6.0,
    left: spacing::SPACE_2_5,
  })
  .style(move |_, status| copy_button_style(enabled, menu_open, status));

  if enabled {
    label.on_press(Message::ToggleCopyMenu(plan_id)).into()
  } else {
    label.into()
  }
}

fn copy_button_label_style(enabled: bool) -> text::Style {
  text::Style {
    color: Some(if enabled {
      color::accent::PLASMA
    } else {
      color::text::tertiary()
    }),
  }
}

fn copy_button_style(enabled: bool, menu_open: bool, status: button::Status) -> button::Style {
  let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let border_color = if menu_open || (enabled && hover) {
    color::accent::PLASMA
  } else if enabled {
    color::accent::PLASMA_MUTED
  } else {
    color::with_alpha(color::text::PRIMARY, 0.1)
  };
  button::Style {
    background: (enabled && hover).then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.10))),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    text_color: if enabled {
      color::accent::PLASMA
    } else {
      color::text::tertiary()
    },
    ..button::Style::default()
  }
}

fn danger_button<'a>(label: String, message: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      }),
  )
  .padding(Padding {
    top: 6.0,
    right: spacing::SPACE_2_5,
    bottom: 6.0,
    left: spacing::SPACE_2_5,
  })
  .on_press(message)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then(|| Background::Color(color::with_alpha(color::status::DANGER, 0.12))),
      border: Border {
        color: color::status::DANGER,
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      text_color: color::status::DANGER,
      ..button::Style::default()
    }
  })
  .into()
}

fn delete_button<'a>(plan_id: i64) -> Element<'a, Message> {
  button(
    text("\u{00d7}")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: 4.0,
    right: spacing::SPACE_2,
    bottom: 4.0,
    left: spacing::SPACE_2,
  })
  .on_press(Message::RequestDelete(plan_id))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: if active {
          color::status::DANGER
        } else {
          iced::Color::TRANSPARENT
        },
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      text_color: if active {
        color::status::DANGER
      } else {
        color::text::tertiary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn portrait_tile<'a, M: 'a>(portrait: &images::ImageState, name: &str, box_size: f32) -> Element<'a, M> {
  match portrait.path() {
    Some(path) => container(clip_layer(
      image(image::Handle::from_path(path))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
      Length::Fill,
      Length::Fill,
    ))
    .width(Length::Fixed(box_size))
    .height(Length::Fixed(box_size))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into(),
    None => avatar(0, name, Length::Fixed(box_size), box_size, None),
  }
}

fn count_tint(count: usize, active: bool) -> Option<iced::Color> {
  if count == 0 {
    Some(color::text::tertiary())
  } else if active {
    Some(color::accent::PLASMA)
  } else {
    Some(color::text::secondary())
  }
}

fn first_name(name: &str) -> String {
  name.split_whitespace().next().unwrap_or(name).to_owned()
}

pub fn relative_time(iso: &str) -> String {
  let Some(ts) = parse_iso8601(iso) else {
    return iso.to_owned();
  };
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let diff = now - ts;
  if diff < 60 {
    t!("skills.manager.time_just_now").into_owned()
  } else if diff < 3600 {
    t!("skills.manager.time_minutes_ago", count => diff / 60).into_owned()
  } else if diff < 86_400 {
    t!("skills.manager.time_hours_ago", count => diff / 3600).into_owned()
  } else {
    t!("skills.manager.time_days_ago", count => diff / 86_400).into_owned()
  }
}

fn days_since_epoch(y: i64, m: i64, d: i64) -> i64 {
  let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let doy = (153 * m + 2) / 5 + d - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146_097 + doe - 719_468
}

fn parse_iso8601(s: &str) -> Option<i64> {
  let s = s.trim().trim_end_matches('Z');
  let (date, time) = s.split_once('T')?;
  let date_parts: Vec<i64> = date.split('-').filter_map(|p| p.parse().ok()).collect();
  let time_parts: Vec<i64> = time
    .split('+')
    .next()
    .unwrap_or("")
    .split(':')
    .filter_map(|p| p.parse::<f64>().ok().map(|v| v as i64))
    .collect();
  if date_parts.len() < 3 || time_parts.len() < 3 {
    return None;
  }
  let days = days_since_epoch(date_parts[0], date_parts[1], date_parts[2]);
  Some(days * 86_400 + time_parts[0] * 3600 + time_parts[1] * 60 + time_parts[2])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn plan(id: i64, name: &str, entry_count: usize) -> PlanRow {
    PlanRow {
      edited: "2d ago".to_owned(),
      entry_count,
      id,
      name: name.to_owned(),
    }
  }

  fn entry(character_id: i64, name: &str, plans: Vec<PlanRow>) -> RosterEntry {
    RosterEntry {
      character_id,
      corp: "TST".to_owned(),
      name: name.to_owned(),
      plans,
      portrait: images::ImageState::Fresh("/tmp/p.jpg".into()),
    }
  }

  fn roster() -> Roster {
    Roster {
      entries: vec![
        entry(1, "Aria", vec![plan(10, "Combat", 5), plan(11, "Industry", 0)]),
        entry(2, "Borin", Vec::new()),
        entry(3, "Cassi", vec![plan(12, "Logi", 3)]),
      ],
    }
  }

  mod roster {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_sums_plans_across_every_character() {
      assert_eq!(super::roster().plan_total(), 3);
    }
  }

  mod set_roster {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_the_selection_to_the_first_character_with_plans() {
      let mut state = State::new();

      state.set_roster(super::roster());

      assert_eq!(state.selected(), Some(1));
    }

    #[test]
    fn it_falls_back_to_the_first_character_when_none_have_plans() {
      let mut state = State::new();
      let empty = Roster {
        entries: vec![entry(7, "Solo", Vec::new())],
      };

      state.set_roster(empty);

      assert_eq!(state.selected(), Some(7));
    }

    #[test]
    fn it_keeps_a_still_present_selection_across_reloads() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.select(3);

      state.set_roster(super::roster());

      assert_eq!(state.selected(), Some(3));
    }

    #[test]
    fn it_reselects_when_the_previous_character_is_gone() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.select(3);

      state.set_roster(Roster {
        entries: vec![entry(9, "New", vec![plan(20, "Fresh", 2)])],
      });

      assert_eq!(state.selected(), Some(9));
    }

    #[test]
    fn it_clears_the_selection_for_an_empty_roster() {
      let mut state = State::new();
      state.set_roster(super::roster());

      state.set_roster(Roster::default());

      assert_eq!(state.selected(), None);
    }
  }

  mod select {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ignores_an_unknown_character() {
      let mut state = State::new();
      state.set_roster(super::roster());

      state.select(999);

      assert_eq!(state.selected(), Some(1));
    }

    #[test]
    fn it_clears_armed_affordances_on_select() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(10);
      state.toggle_copy_menu(11);

      state.select(3);

      assert_eq!(state.confirm_delete(), None);
      assert_eq!(state.copy_menu(), None);
    }
  }

  mod copy_targets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_excludes_the_source_character() {
      let mut state = State::new();
      state.set_roster(super::roster());

      let target_ids: Vec<i64> = state.copy_targets(1).iter().map(|e| e.character_id).collect();

      assert_eq!(target_ids, vec![2, 3]);
    }
  }

  mod toggle_copy_menu {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_then_closes_the_same_plan_and_disarms_delete() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(10);

      state.toggle_copy_menu(10);
      assert_eq!(state.copy_menu(), Some(10));
      assert_eq!(
        state.confirm_delete(),
        None,
        "opening the menu disarms a pending delete"
      );

      state.toggle_copy_menu(10);
      assert_eq!(state.copy_menu(), None);
    }
  }

  mod arm_delete {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_arms_the_confirm_and_closes_an_open_menu() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.toggle_copy_menu(11);

      state.arm_delete(10);

      assert_eq!(state.confirm_delete(), Some(10));
      assert_eq!(state.copy_menu(), None);
    }

    #[test]
    fn it_drops_a_stale_confirm_when_the_plan_is_gone() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.arm_delete(10);

      state.set_roster(Roster {
        entries: vec![entry(3, "Cassi", vec![plan(12, "Logi", 3)])],
      });

      assert_eq!(state.confirm_delete(), None);
    }
  }

  mod stale_images {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_one_key_per_stale_portrait() {
      let roster = Roster {
        entries: vec![
          RosterEntry {
            character_id: 1,
            corp: "TST".to_owned(),
            name: "Aria".to_owned(),
            plans: Vec::new(),
            portrait: images::ImageState::Stale {
              id: 1,
              kind: images::ImageKind::CharacterPortrait,
            },
          },
          entry(2, "Borin", Vec::new()),
        ],
      };

      let keys = roster.stale_images();

      assert_eq!(keys, vec![(images::ImageKind::CharacterPortrait, 1)]);
    }
  }

  mod relative_time {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_buckets_a_parseable_timestamp_into_a_relative_label() {
      crate::i18n::set_locale(crate::i18n::Language::En);
      let label = relative_time("2000-01-01T00:00:00Z");

      assert!(label.ends_with("d ago"), "expected a days-ago bucket, got {label}");
    }

    #[test]
    fn it_falls_back_to_the_raw_string_for_an_unparseable_value() {
      assert_eq!(relative_time("not-a-date"), "not-a-date");
    }
  }

  mod load_roster {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self, Database,
      model::{Alliance, Bloodline, Character, Corporation, Gender, OwnerType, Race},
      repo::{character, infra, skills},
    };

    async fn seed_owned(db: &Database, id: i64, name: &str) {
      let corp_id = 90_000_001;
      let alliance_id = 99_000_001;
      let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
      let race = Race::new(2, alliance_id, "A race.", "Caldari");
      let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
      corp.set_ceo_id(id);
      corp.set_creator_id(id);
      corp.set_member_count(1);
      corp.set_tax_rate(0.0);
      let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
      let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, name);
      character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
        .await
        .unwrap();
      infra::upsert(db, id, OwnerType::Character, "tok", "rt", 9999, None, None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_builds_a_per_character_plan_count_and_header_totals() {
      let db = store::open_test().await.unwrap();
      seed_owned(&db, 42, "Aria").await;
      seed_owned(&db, 7, "Borin").await;
      let plan = skills::create(&db, 42, "Combat").await.unwrap();
      skills::insert_entry(&db, plan.id(), 3300, 5).await.unwrap();
      skills::create(&db, 42, "Industry").await.unwrap();

      let roster = load_roster(&db).await;

      assert_eq!(roster.entries.len(), 2);
      assert_eq!(roster.plan_total(), 2);
      let aria = roster.entries.iter().find(|entry| entry.character_id == 42).unwrap();
      assert_eq!(aria.corp, "TSC");
      assert_eq!(aria.plans.len(), 2);
      let combat = aria.plans.iter().find(|plan| plan.name == "Combat").unwrap();
      assert_eq!(combat.entry_count, 1);
      let borin = roster.entries.iter().find(|entry| entry.character_id == 7).unwrap();
      assert!(borin.plans.is_empty());
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_master_detail_window() {
      let mut state = State::new();
      state.set_roster(super::roster());

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_the_empty_detail_for_a_character_without_plans() {
      let mut state = State::new();
      state.set_roster(super::roster());
      state.select(2);

      let _el: Element<'_, Message> = view(&state);
    }

    #[test]
    fn it_renders_an_empty_roster() {
      let state = State::new();

      let _el: Element<'_, Message> = view(&state);
    }
  }

  mod copy_to_button {
    use super::*;

    #[test]
    fn it_builds_an_enabled_button_with_an_open_menu() {
      let _el: Element<'_, Message> = super::super::copy_to_button(10, true, true);
    }

    #[test]
    fn it_builds_an_enabled_button_with_a_closed_menu() {
      let _el: Element<'_, Message> = super::super::copy_to_button(10, true, false);
    }

    #[test]
    fn it_builds_a_disabled_button_when_there_are_no_targets() {
      let _el: Element<'_, Message> = super::super::copy_to_button(10, false, false);
    }
  }

  mod copy_button_label_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_accent_when_enabled() {
      assert_eq!(
        super::super::copy_button_label_style(true).color,
        Some(color::accent::PLASMA)
      );
    }

    #[test]
    fn it_dims_the_label_when_disabled() {
      assert_eq!(
        super::super::copy_button_label_style(false).color,
        Some(color::text::tertiary())
      );
    }
  }

  mod copy_button_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accents_the_border_for_an_open_menu_even_without_hover() {
      let style = super::super::copy_button_style(true, true, button::Status::Active);

      assert_eq!(style.border.color, color::accent::PLASMA);
      assert_eq!(style.background, None);
    }

    #[test]
    fn it_accents_and_fills_an_enabled_button_on_hover() {
      let style = super::super::copy_button_style(true, false, button::Status::Hovered);

      assert_eq!(style.border.color, color::accent::PLASMA);
      assert!(style.background.is_some());
      assert_eq!(style.text_color, color::accent::PLASMA);
    }

    #[test]
    fn it_uses_the_muted_border_for_an_enabled_resting_button() {
      let style = super::super::copy_button_style(true, false, button::Status::Active);

      assert_eq!(style.border.color, color::accent::PLASMA_MUTED);
      assert_eq!(style.background, None);
    }

    #[test]
    fn it_dims_the_border_and_text_for_a_disabled_button() {
      let style = super::super::copy_button_style(false, false, button::Status::Hovered);

      assert_eq!(style.border.color, color::with_alpha(color::text::PRIMARY, 0.1));
      assert_eq!(style.text_color, color::text::tertiary());
      assert_eq!(style.background, None, "a disabled button never fills on hover");
    }
  }
}
