use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, Space, Stack, button, container, mouse_area, text},
};

use super::{Message, name_link::name_link};
pub(super) use crate::ui::format::{fmt_isk_opt as format_isk, fmt_sp_opt as format_sp};
use crate::{
  store::{images, model::ENTITY_TYPE_CHARACTER},
  sync::Phase,
  ui::{
    components::{
      avatar::avatar,
      chip::{Chip, chip},
      progress_bar::progress_bar,
      rule, status,
    },
    style::{color, radius, spacing, typography},
  },
};

const ACCENT_WIDTH: f32 = 3.0;
const CHIP_GAP: f32 = 5.0;
const CHIP_RADIUS: f32 = 999.0;
const GRAB_HANDLE_DOT: f32 = 3.0;
const GRAB_HANDLE_DOT_ALPHA: f32 = 0.55;
const GRAB_HANDLE_GAP: f32 = 3.0;
const GRAB_HANDLE_INSET: f32 = 8.0;
const GRAB_HANDLE_PADDING: f32 = 5.0;
const HAIRLINE: f32 = 1.0;
const ISK_VALUE_SIZE: f32 = 21.0;
const PLACEHOLDER: &str = "—";
const PORTRAIT_HEIGHT: f32 = 140.0;
const PROGRESS_HEIGHT: f32 = 4.0;
// Clears the grab handle pill (dots + padding + inset) so the reauth badge never overlaps it.
const REAUTH_BADGE_LEFT_INSET: f32 = 36.0;
const STATUS_PILL_INSET: f32 = 12.0;
const STATUS_PILL_RADIUS: f32 = 4.0;
const STATUS_PILL_TEXT_ALPHA: f32 = 0.7;

#[derive(Clone, Debug)]
pub struct CardModel {
  pub accent: Option<Color>,
  pub character_id: i64,
  pub corp_ticker: String,
  pub docked: Option<bool>,
  pub location: Option<String>,
  pub name: String,
  pub needs_reauth: bool,
  pub portrait: images::ImageState,
  pub position: i64,
  pub tags: Vec<TagChip>,
  pub total_sp: Option<i64>,
  pub training: Option<Training>,
  pub wallet_balance: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct TagChip {
  pub color: Option<Color>,
  pub id: i64,
  pub name: String,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Sections {
  pub detail_enabled: bool,
  pub location_enabled: bool,
  pub training_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct Training {
  pub level: i64,
  pub paused: Option<usize>,
  pub progress: f32,
  pub remaining: String,
  pub skill: String,
}

pub(super) fn card<'a>(
  model: &'a CardModel,
  failure: Option<Phase>,
  dragging: bool,
  sections: Sections,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![
    portrait(model),
    identity(model, sections.detail_enabled),
    tag_row(model),
  ];
  if sections.training_enabled {
    children.push(rule::horizontal());
    children.push(training_section(model));
  }
  children.push(rule::horizontal());
  children.push(stats_row(model, sections.location_enabled));

  if let Some(indicator) = sync_indicator(failure) {
    children.push(indicator);
  }

  let body = container(Column::with_children(children))
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::CARD_HEIGHT))
    .style(card_surface(model.accent.is_some(), dragging));

  let composed: Element<'a, Message> = match model.accent {
    Some(accent) => {
      let base = Row::with_children(vec![
        Space::new().width(Length::Fixed(ACCENT_WIDTH)).into(),
        body.into(),
      ])
      .width(Length::Fill);
      let bar = container(
        container(Space::new())
          .width(Length::Fixed(ACCENT_WIDTH))
          .height(Length::Fill)
          .style(move |_| container::Style {
            background: Some(Background::Color(accent)),
            ..container::Style::default()
          }),
      )
      .align_x(Horizontal::Left);
      Stack::with_children(vec![base.into(), bar.into()])
        .width(Length::Fill)
        .into()
    }
    None => body.into(),
  };

  // The six-dot handle is the only drag activator: presses anywhere else on the card must
  // not pick it up, so the card-wide mouse_area keeps just the right-click affordance.
  let layered = Stack::with_children(vec![composed, grab_handle(model.character_id)])
    .width(Length::Fill)
    .height(Length::Fixed(spacing::layout::CARD_HEIGHT));

  mouse_area(layered)
    .on_right_press(Message::CardRightPressed(model.character_id))
    .into()
}

fn grab_handle<'a>(character_id: i64) -> Element<'a, Message> {
  container(
    mouse_area(grab_handle_pill())
      .interaction(iced::mouse::Interaction::Grab)
      .on_press(Message::PickUpCard(character_id)),
  )
  .padding(GRAB_HANDLE_INSET)
  .into()
}

fn grab_handle_pill<'a>() -> Element<'a, Message> {
  let dot = || {
    status::dot_sized(
      color::with_alpha(color::text::PRIMARY, GRAB_HANDLE_DOT_ALPHA),
      GRAB_HANDLE_DOT,
    )
  };
  let row = || Row::with_children(vec![dot(), dot()]).spacing(GRAB_HANDLE_GAP).into();
  let dots = Column::with_children(vec![row(), row(), row()]).spacing(GRAB_HANDLE_GAP);

  container(dots)
    .padding(GRAB_HANDLE_PADDING)
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      border: Border {
        radius: STATUS_PILL_RADIUS.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

pub(super) fn ghost(model: &CardModel, sections: Sections) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![portrait(model), ghost_identity(model), ghost_tag_row(model)];
  if sections.training_enabled {
    children.push(rule::horizontal());
    children.push(training_section(model));
  }
  children.push(rule::horizontal());
  children.push(stats_row(model, sections.location_enabled));

  let body = container(Column::with_children(children))
    .width(Length::Fill)
    .style(ghost_surface(model.accent.is_some()));

  let composed: Element<'_, Message> = match model.accent {
    Some(accent) => {
      let base = Row::with_children(vec![
        Space::new().width(Length::Fixed(ACCENT_WIDTH)).into(),
        body.into(),
      ])
      .width(Length::Fill);
      let bar = container(
        container(Space::new())
          .width(Length::Fixed(ACCENT_WIDTH))
          .height(Length::Fill)
          .style(move |_| container::Style {
            background: Some(Background::Color(accent)),
            ..container::Style::default()
          }),
      )
      .align_x(Horizontal::Left);
      Stack::with_children(vec![base.into(), bar.into()])
        .width(Length::Fill)
        .into()
    }
    None => body.into(),
  };

  // Mirror the interactive card's grab handle so the drag preview matches what was grabbed.
  Stack::with_children(vec![
    composed,
    container(grab_handle_pill()).padding(GRAB_HANDLE_INSET).into(),
  ])
  .width(Length::Fill)
  .into()
}

fn ghost_identity(model: &CardModel) -> Element<'_, Message> {
  let name = text(model.name.clone())
    .font(typography::body::REGULAR)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let ticker = text(model.corp_ticker.clone())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  container(Column::with_children(vec![name.into(), ticker.into()]).spacing(spacing::UNIT))
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
      bottom: 0.0,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn ghost_tag_row(model: &CardModel) -> Element<'_, Message> {
  let chips: Vec<Element<'_, Message>> = model.tags.iter().map(|tag| chip(tag.name.clone(), tag.color)).collect();

  container(Row::with_children(chips).spacing(CHIP_GAP))
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn ghost_surface(has_accent: bool) -> impl Fn(&iced::Theme) -> container::Style {
  card_surface(has_accent, true)
}

fn status_label(docked: Option<bool>) -> Option<String> {
  match docked {
    Some(true) => Some(t!("roster.card.docked").into_owned()),
    Some(false) => Some(t!("roster.card.in_space").into_owned()),
    None => None,
  }
}

fn portrait(model: &CardModel) -> Element<'_, Message> {
  let splash = avatar(
    model.character_id,
    &model.name,
    Length::Fill,
    PORTRAIT_HEIGHT,
    model.portrait.path(),
  );

  let mut layers: Vec<Element<'_, Message>> = vec![splash];

  if let Some(label) = status_label(model.docked) {
    let pill = container(
      text(label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::with_alpha(color::text::PRIMARY, STATUS_PILL_TEXT_ALPHA)),
        }),
    )
    .padding([spacing::UNIT, spacing::SPACE_2])
    .style(|_| container::Style {
      background: Some(Background::Color(color::state::OVERLAY_DARK)),
      border: Border {
        radius: STATUS_PILL_RADIUS.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

    layers.push(
      container(pill)
        .width(Length::Fill)
        .height(Length::Fixed(PORTRAIT_HEIGHT))
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
        .padding(STATUS_PILL_INSET)
        .into(),
    );
  }

  if model.needs_reauth {
    layers.push(
      container(reauth_badge(Message::ReauthCharacterRequested(model.character_id)))
        .width(Length::Fill)
        .height(Length::Fixed(PORTRAIT_HEIGHT))
        .align_x(Horizontal::Left)
        .align_y(Vertical::Top)
        .padding(Padding {
          top: STATUS_PILL_INSET,
          right: STATUS_PILL_INSET,
          bottom: STATUS_PILL_INSET,
          left: REAUTH_BADGE_LEFT_INSET,
        })
        .into(),
    );
  }

  if layers.len() == 1 {
    return layers.pop().expect("splash layer is always present");
  }

  Stack::with_children(layers)
    .width(Length::Fill)
    .height(Length::Fixed(PORTRAIT_HEIGHT))
    .into()
}

fn identity(model: &CardModel, detail_enabled: bool) -> Element<'_, Message> {
  let name: Element<'_, Message> = if detail_enabled {
    name_link(
      model.name.clone(),
      typography::size::LG,
      Message::CharacterSelected(model.character_id),
    )
  } else {
    text(model.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into()
  };

  let ticker = text(model.corp_ticker.clone())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  container(Column::with_children(vec![name, ticker.into()]).spacing(spacing::UNIT))
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
      bottom: 0.0,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn tag_row(model: &CardModel) -> Element<'_, Message> {
  let mut chips: Vec<Element<'_, Message>> = model
    .tags
    .iter()
    .map(|tag| {
      Chip::new(tag.name.clone(), tag.color)
        .on_remove(Message::UnassignTag {
          entity_id: model.character_id,
          entity_type: ENTITY_TYPE_CHARACTER,
          tag_id: tag.id,
        })
        .view()
    })
    .collect();
  chips.push(add_tag_affordance(model.character_id));

  container(Row::with_children(chips).spacing(CHIP_GAP))
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .into()
}

fn add_tag_affordance<'a>(character_id: i64) -> Element<'a, Message> {
  button(
    text("+")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding([spacing::UNIT / 2.0, spacing::SPACE_2])
  .on_press(Message::OpenAddTagModal {
    entity_id: character_id,
    entity_type: ENTITY_TYPE_CHARACTER,
  })
  .style(|_, _| button::Style {
    background: None,
    text_color: color::text::secondary(),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      width: HAIRLINE,
      radius: CHIP_RADIUS.into(),
    },
    ..button::Style::default()
  })
  .into()
}

fn training_section(model: &CardModel) -> Element<'_, Message> {
  let label = text(t!("roster.card.training"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let header: Element<'_, Message> = match &model.training {
    Some(training) => Row::with_children(vec![
      label.into(),
      Space::new().width(Length::Fill).into(),
      text(training.paused.map_or_else(|| training.remaining.clone(), paused_label))
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .into(),
    None => label.into(),
  };

  let detail: Element<'_, Message> = match &model.training {
    Some(training) => training_detail(model.character_id, training),
    None => idle_state(),
  };

  container(Column::with_children(vec![header, detail]).spacing(spacing::SPACE_2))
    .padding(spacing::SPACE_3)
    .into()
}

fn paused_label(queued: usize) -> String {
  let key = if queued == 1 {
    "roster.card.paused_queued_one"
  } else {
    "roster.card.paused_queued_other"
  };
  t!(key, count => queued).into_owned()
}

fn training_detail(character_id: i64, training: &Training) -> Element<'_, Message> {
  let skill = button(
    text(format!("{} {}", training.skill, roman(training.level)))
      .font(typography::body::REGULAR)
      .size(typography::size::MD),
  )
  .padding(0)
  .on_press(Message::TrainingSkillClicked(character_id))
  .style(name_button);

  let progress_color = if training.paused.is_some() {
    color::text::secondary()
  } else {
    color::accent()
  };

  Column::with_children(vec![
    skill.into(),
    progress_bar(training.progress, progress_color, PROGRESS_HEIGHT),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn idle_state<'a>() -> Element<'a, Message> {
  Row::with_children(vec![
    container(status::dot(color::status::DANGER))
      .align_y(Vertical::Center)
      .into(),
    text(t!("roster.card.skill_queue_empty"))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::status::DANGER),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn stats_row(model: &CardModel, location_enabled: bool) -> Element<'_, Message> {
  let isk = stat(
    t!("roster.card.isk").into_owned(),
    format_isk(model.wallet_balance),
    typography::mono::MEDIUM,
    ISK_VALUE_SIZE,
    color::accent(),
  );

  if !location_enabled {
    return container(isk).width(Length::Fill).into();
  }

  let location = model.location.clone().unwrap_or_else(|| PLACEHOLDER.to_owned());
  let columns = Row::with_children(vec![
    stat(
      t!("roster.card.location").into_owned(),
      location,
      typography::body::REGULAR,
      typography::size::MD,
      color::text::PRIMARY,
    ),
    Space::new().width(Length::Fixed(HAIRLINE)).into(),
    isk,
  ])
  .width(Length::Fill);

  Stack::with_children(vec![
    columns.into(),
    container(rule::vertical_fill(0.1)).center_x(Length::Fill).into(),
  ])
  .width(Length::Fill)
  .into()
}

fn stat<'a>(
  label: String,
  value: String,
  value_font: iced::Font,
  value_size: f32,
  value_color: Color,
) -> Element<'a, Message> {
  let label = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let value = text(value)
    .font(value_font)
    .size(value_size)
    .style(move |_| text::Style {
      color: Some(value_color),
    });

  container(Column::with_children(vec![label.into(), value.into()]).spacing(spacing::UNIT))
    .width(Length::Fill)
    .padding(spacing::SPACE_3)
    .into()
}

pub(super) fn reauth_badge<'a>(on_press: Message) -> Element<'a, Message> {
  let on_danger = color::on_fill(color::status::DANGER);

  button(
    Row::with_children(vec![
      status::dot(on_danger),
      text(t!("roster.actions.fix_permissions"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(move |_| text::Style {
          color: Some(on_danger),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding([spacing::UNIT, spacing::SPACE_2])
  .on_press(on_press)
  .style(reauth_badge_style)
  .into()
}

fn reauth_badge_style(_theme: &iced::Theme, _status: button::Status) -> button::Style {
  button::Style {
    background: Some(Background::Color(color::status::DANGER)),
    text_color: color::on_fill(color::status::DANGER),
    border: Border {
      radius: STATUS_PILL_RADIUS.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn sync_indicator<'a>(failure: Option<Phase>) -> Option<Element<'a, Message>> {
  let label = match failure? {
    Phase::BackingOff => t!("roster.card.sync_backing_off"),
    Phase::Failed => t!("roster.card.sync_failed"),
    Phase::Blocked | Phase::Done | Phase::Empty | Phase::NotReady | Phase::Syncing => {
      return None;
    }
  };

  Some(
    container(
      text(label)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        }),
    )
    .padding(Padding {
      top: 0.0,
      right: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3_5,
    })
    .into(),
  )
}

fn card_surface(has_accent: bool, dragging: bool) -> impl Fn(&iced::Theme) -> container::Style {
  move |theme: &iced::Theme| {
    let mut style = crate::ui::style::control::card(theme);
    if has_accent {
      style.border.radius = Radius {
        top_left: 0.0,
        bottom_left: 0.0,
        ..radius::CARD.into()
      };
    }
    if dragging {
      style.border.color = color::accent();
    }
    style
  }
}

fn name_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let text_color = match status {
    button::Status::Hovered | button::Status::Pressed => color::accent(),
    _ => color::text::PRIMARY,
  };
  button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color,
    ..button::Style::default()
  }
}

fn roman(level: i64) -> String {
  match level {
    1 => "I".to_owned(),
    2 => "II".to_owned(),
    3 => "III".to_owned(),
    4 => "IV".to_owned(),
    5 => "V".to_owned(),
    other => other.to_string(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn base_model() -> CardModel {
    CardModel {
      accent: None,
      character_id: 12_345_678,
      corp_ticker: "CORP1".to_owned(),
      docked: Some(true),
      location: Some("Jita IV - Moon 4".to_owned()),
      name: "Test Pilot".to_owned(),
      needs_reauth: false,
      portrait: images::ImageState::Stale {
        id: 12_345_678,
        kind: images::ImageKind::CharacterPortrait,
      },
      position: 0,
      tags: vec![TagChip {
        color: Some(color::accent()),
        id: 1,
        name: "Main".to_owned(),
      }],
      total_sp: Some(82_000_000),
      training: Some(Training {
        level: 5,
        paused: None,
        progress: 0.71,
        remaining: "2d 14h".to_owned(),
        skill: "Caldari Cruiser".to_owned(),
      }),
      wallet_balance: Some(4_820_000_000.0),
    }
  }

  fn all_sections() -> Sections {
    Sections {
      detail_enabled: true,
      location_enabled: true,
      training_enabled: true,
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_a_card_being_dragged() {
      let model = base_model();

      let _el: Element<'_, Message> = card(&model, None, true, all_sections());
    }

    #[test]
    fn it_renders_a_card_with_a_sync_error() {
      let model = base_model();

      for failure in [Phase::Failed, Phase::BackingOff] {
        let _el: Element<'_, Message> = card(&model, Some(failure), false, all_sections());
      }
    }

    #[test]
    fn it_renders_a_needs_reauth_badge_on_a_flagged_character() {
      let mut model = base_model();
      model.needs_reauth = true;

      let _el: Element<'_, Message> = card(&model, None, false, all_sections());
      let _with_failure: Element<'_, Message> = card(&model, Some(Phase::Failed), false, all_sections());
    }

    #[test]
    fn it_renders_a_non_interactive_ghost_clone() {
      let mut model = base_model();
      model.accent = Some(color::accent());

      let _accented: Element<'_, Message> = ghost(&model, all_sections());

      let mut plain = base_model();
      plain.accent = None;
      plain.training = None;
      plain.tags = Vec::new();
      let _plain: Element<'_, Message> = ghost(&plain, all_sections());
    }

    #[test]
    fn it_renders_a_training_card() {
      let model = base_model();

      let _el: Element<'_, Message> = card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_an_accented_squad_card() {
      let mut model = base_model();
      model.accent = Some(color::accent());

      let _el: Element<'_, Message> = card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_an_idle_card() {
      let mut model = base_model();
      model.training = None;

      let _el: Element<'_, Message> = card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_a_paused_card() {
      let mut model = base_model();
      if let Some(training) = model.training.as_mut() {
        training.paused = Some(4);
      }

      let _el: Element<'_, Message> = card(&model, None, false, all_sections());
    }

    #[test]
    fn it_labels_a_paused_queue_with_its_real_count() {
      assert_eq!(
        paused_label(4),
        t!("roster.card.paused_queued_other", count => 4).into_owned()
      );
    }

    #[test]
    fn it_uses_the_singular_noun_for_a_one_skill_paused_queue() {
      assert_eq!(
        paused_label(1),
        t!("roster.card.paused_queued_one", count => 1).into_owned()
      );
    }

    #[test]
    fn it_renders_every_docked_state() {
      for docked in [Some(true), Some(false), None] {
        let mut model = base_model();
        model.docked = docked;

        let _el: Element<'_, Message> = card(&model, None, false, all_sections());
      }
    }

    #[test]
    fn it_renders_isk_present_and_placeholder() {
      let present = base_model();
      let mut absent = base_model();
      absent.wallet_balance = None;

      let _present: Element<'_, Message> = card(&present, None, false, all_sections());
      let _absent: Element<'_, Message> = card(&absent, None, false, all_sections());
    }

    #[test]
    fn it_renders_the_tag_row_with_the_add_affordance_and_no_inline_picker() {
      let model = base_model();

      let _el: Element<'_, Message> = card(&model, None, false, all_sections());
    }

    #[test]
    fn it_mounts_the_grab_handle_as_a_layer_above_the_card_body() {
      use iced::advanced::widget::Tree;
      use pretty_assertions::assert_eq;

      let model = base_model();

      let el: Element<'_, Message> = card(&model, None, false, all_sections());
      let tree = Tree::new(el.as_widget());

      // mouse_area > Stack > [card body, grab handle]: the handle rides above the body,
      // and it (not the card-wide mouse_area) is the only drag activator.
      assert_eq!(tree.children.len(), 1);
      assert_eq!(tree.children[0].children.len(), 2);
    }

    #[test]
    fn it_mirrors_the_grab_handle_on_the_ghost_preview() {
      use iced::advanced::widget::Tree;
      use pretty_assertions::assert_eq;

      let model = base_model();

      let el: Element<'_, Message> = ghost(&model, all_sections());
      let tree = Tree::new(el.as_widget());

      assert_eq!(tree.children.len(), 2);
    }

    #[test]
    fn it_renders_with_the_location_section_hidden() {
      let model = base_model();
      let sections = Sections {
        location_enabled: false,
        ..all_sections()
      };

      let _el: Element<'_, Message> = card(&model, None, false, sections);
    }

    #[test]
    fn it_renders_with_the_training_section_hidden() {
      let model = base_model();
      let sections = Sections {
        training_enabled: false,
        ..all_sections()
      };

      let _el: Element<'_, Message> = card(&model, None, false, sections);
    }
  }

  mod roman {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_levels_one_through_five() {
      assert_eq!(roman(1), "I");
      assert_eq!(roman(5), "V");
      assert_eq!(roman(7), "7");
    }
  }

  mod sizing {
    use iced::{Length, advanced::Widget};
    use pretty_assertions::assert_ne;

    use super::*;

    fn declared_height(model: &CardModel) -> Length {
      let element = card(model, None, false, all_sections());
      Widget::<Message, _, _>::size(element.as_widget()).height
    }

    #[test]
    fn it_declares_the_fixed_card_height() {
      use pretty_assertions::assert_eq;

      let model = base_model();

      assert_eq!(
        declared_height(&model),
        Length::Fixed(crate::ui::style::spacing::layout::CARD_HEIGHT),
      );
    }

    #[test]
    fn it_does_not_declare_a_fill_height_for_an_accented_card() {
      let mut model = base_model();
      model.accent = Some(color::accent());

      assert_ne!(declared_height(&model), Length::Fill);
    }

    #[test]
    fn it_does_not_declare_a_fill_height_for_an_idle_card() {
      let mut model = base_model();
      model.training = None;

      assert_ne!(declared_height(&model), Length::Fill);
    }

    #[test]
    fn it_does_not_declare_a_fill_height_for_an_unaccented_card() {
      let mut model = base_model();
      model.accent = None;

      assert_ne!(declared_height(&model), Length::Fill);
    }
  }

  mod status_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_labels_docked_in_space_and_renders_no_pill_when_unknown() {
      assert_eq!(status_label(Some(true)), Some(t!("roster.card.docked").into_owned()));
      assert_eq!(status_label(Some(false)), Some(t!("roster.card.in_space").into_owned()));
      assert_eq!(status_label(None), None);
    }
  }
}
