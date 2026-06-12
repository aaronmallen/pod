use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  border::Radius,
  widget::{Column, Row, Space, Stack, button, container, mouse_area, text},
};

use super::Message;
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
const HAIRLINE: f32 = 1.0;
const PLACEHOLDER: &str = "—";
const PORTRAIT_HEIGHT: f32 = 140.0;
const PROGRESS_HEIGHT: f32 = 4.0;
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

#[derive(Clone, Debug)]
pub struct Training {
  pub level: i64,
  pub progress: f32,
  pub remaining: String,
  pub skill: String,
}

pub(super) fn card<'a>(model: &'a CardModel, failure: Option<Phase>, dragging: bool) -> Element<'a, Message> {
  let mut sections: Vec<Element<'a, Message>> = vec![
    portrait(model),
    identity(model),
    tag_row(model),
    rule::horizontal(),
    training_section(model),
    rule::horizontal(),
    stats_row(model),
  ];

  if model.needs_reauth {
    sections.push(reauth_affordance(model.character_id));
  }

  if let Some(indicator) = sync_indicator(failure) {
    sections.push(indicator);
  }

  let body = container(Column::with_children(sections))
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

  mouse_area(composed)
    .on_press(Message::PickUpCard(model.character_id))
    .on_right_press(Message::CardRightPressed(model.character_id))
    .into()
}

pub(super) fn ghost(model: &CardModel) -> Element<'_, Message> {
  let sections: Vec<Element<'_, Message>> = vec![
    portrait(model),
    ghost_identity(model),
    ghost_tag_row(model),
    rule::horizontal(),
    training_section(model),
    rule::horizontal(),
    stats_row(model),
  ];

  let body = container(Column::with_children(sections))
    .width(Length::Fill)
    .style(ghost_surface(model.accent.is_some()));

  match model.accent {
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
  }
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

fn status_label(docked: Option<bool>) -> Option<&'static str> {
  match docked {
    Some(true) => Some("DOCKED"),
    Some(false) => Some("IN SPACE"),
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

  let Some(label) = status_label(model.docked) else {
    return splash;
  };

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

  let overlay = container(pill)
    .width(Length::Fill)
    .height(Length::Fixed(PORTRAIT_HEIGHT))
    .align_x(Horizontal::Right)
    .align_y(Vertical::Top)
    .padding(STATUS_PILL_INSET);

  Stack::with_children(vec![splash, overlay.into()])
    .width(Length::Fill)
    .height(Length::Fixed(PORTRAIT_HEIGHT))
    .into()
}

fn identity(model: &CardModel) -> Element<'_, Message> {
  let name = button(
    text(model.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::LG),
  )
  .padding(0)
  .on_press(Message::CharacterSelected(model.character_id))
  .style(name_button);

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
  let label = text("Training")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let header: Element<'_, Message> = match &model.training {
    Some(training) => Row::with_children(vec![
      label.into(),
      Space::new().width(Length::Fill).into(),
      text(training.remaining.clone())
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

fn training_detail(character_id: i64, training: &Training) -> Element<'_, Message> {
  let skill = button(
    text(format!("{} {}", training.skill, roman(training.level)))
      .font(typography::body::REGULAR)
      .size(typography::size::MD),
  )
  .padding(0)
  .on_press(Message::TrainingSkillClicked(character_id))
  .style(name_button);

  Column::with_children(vec![
    skill.into(),
    progress_bar(training.progress, color::accent::PLASMA, PROGRESS_HEIGHT),
  ])
  .spacing(spacing::SPACE_2)
  .into()
}

fn idle_state<'a>() -> Element<'a, Message> {
  Row::with_children(vec![
    container(status::dot(color::status::DANGER))
      .align_y(Vertical::Center)
      .into(),
    text("Skill queue empty")
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

fn stats_row(model: &CardModel) -> Element<'_, Message> {
  let location = model.location.clone().unwrap_or_else(|| PLACEHOLDER.to_owned());
  let isk = format_isk(model.wallet_balance);

  let columns = Row::with_children(vec![
    stat("Location", location, typography::body::REGULAR),
    Space::new().width(Length::Fixed(HAIRLINE)).into(),
    stat("ISK", isk, typography::mono::REGULAR),
  ])
  .width(Length::Fill);

  Stack::with_children(vec![
    columns.into(),
    container(rule::vertical_fill(0.1)).center_x(Length::Fill).into(),
  ])
  .width(Length::Fill)
  .into()
}

fn stat<'a>(label: &'a str, value: String, value_font: iced::Font) -> Element<'a, Message> {
  let label = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let value = text(value)
    .font(value_font)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  container(Column::with_children(vec![label.into(), value.into()]).spacing(spacing::UNIT))
    .width(Length::Fill)
    .padding(spacing::SPACE_3)
    .into()
}

fn reauth_affordance<'a>(character_id: i64) -> Element<'a, Message> {
  container(
    button(
      text("Needs re-authorization")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS),
    )
    .padding(0)
    .on_press(Message::ReauthCharacterRequested(character_id))
    .style(reauth_button),
  )
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
  })
  .into()
}

fn reauth_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let text_color = match status {
    button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA,
    _ => color::status::DANGER,
  };
  button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color,
    ..button::Style::default()
  }
}

fn sync_indicator<'a>(failure: Option<Phase>) -> Option<Element<'a, Message>> {
  let label = match failure? {
    Phase::BackingOff => "Sync backing off",
    Phase::Failed => "Sync failed",
    Phase::Blocked | Phase::Done | Phase::Empty | Phase::NotReady | Phase::Syncing => return None,
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
      style.border.color = color::accent::PLASMA;
    }
    style
  }
}

fn name_button(_theme: &iced::Theme, status: button::Status) -> button::Style {
  let text_color = match status {
    button::Status::Hovered | button::Status::Pressed => color::accent::PLASMA,
    _ => color::text::PRIMARY,
  };
  button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color,
    ..button::Style::default()
  }
}

pub(super) fn format_isk(balance: Option<f64>) -> String {
  let Some(value) = balance else {
    return PLACEHOLDER.to_owned();
  };

  let magnitude = value.abs();
  if magnitude >= 1e9 {
    format!("{:.2}B", value / 1e9)
  } else if magnitude >= 1e6 {
    format!("{:.1}M", value / 1e6)
  } else if magnitude >= 1e3 {
    format!("{:.1}K", value / 1e3)
  } else {
    format!("{value:.0}")
  }
}

pub(super) fn format_sp(total: Option<i64>) -> String {
  match total {
    None | Some(0) => PLACEHOLDER.to_owned(),
    Some(value) => {
      let n = value as f64;
      if n >= 1e6 {
        format!("{:.1}M", n / 1e6)
      } else if n >= 1e3 {
        format!("{:.0}K", n / 1e3)
      } else {
        value.to_string()
      }
    }
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
        color: Some(color::accent::PLASMA),
        id: 1,
        name: "Main".to_owned(),
      }],
      total_sp: Some(82_000_000),
      training: Some(Training {
        level: 5,
        progress: 0.71,
        remaining: "2d 14h".to_owned(),
        skill: "Caldari Cruiser".to_owned(),
      }),
      wallet_balance: Some(4_820_000_000.0),
    }
  }

  mod format_isk {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_billions_millions_and_thousands_suffixes() {
      assert_eq!(format_isk(Some(4_820_000_000.0)), "4.82B");
      assert_eq!(format_isk(Some(890_000_000.0)), "890.0M");
      assert_eq!(format_isk(Some(12_400.0)), "12.4K");
      assert_eq!(format_isk(Some(42.0)), "42");
    }

    #[test]
    fn it_returns_the_placeholder_for_a_null_balance() {
      assert_eq!(format_isk(None), PLACEHOLDER);
    }
  }

  mod format_sp {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_placeholder_for_none_and_zero() {
      assert_eq!(format_sp(None), PLACEHOLDER);
      assert_eq!(format_sp(Some(0)), PLACEHOLDER);
    }

    #[test]
    fn it_uses_millions_and_thousands_suffixes() {
      assert_eq!(format_sp(Some(1_500_000)), "1.5M");
      assert_eq!(format_sp(Some(2_500)), "2K");
      assert_eq!(format_sp(Some(3_500)), "4K");
    }

    #[test]
    fn it_renders_small_counts_raw() {
      assert_eq!(format_sp(Some(420)), "420");
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

  mod status_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_labels_docked_in_space_and_renders_no_pill_when_unknown() {
      assert_eq!(status_label(Some(true)), Some("DOCKED"));
      assert_eq!(status_label(Some(false)), Some("IN SPACE"));
      assert_eq!(status_label(None), None);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_a_training_card() {
      let model = base_model();

      let _el: Element<'_, Message> = card(&model, None, false);
    }

    #[test]
    fn it_renders_an_idle_card() {
      let mut model = base_model();
      model.training = None;

      let _el: Element<'_, Message> = card(&model, None, false);
    }

    #[test]
    fn it_renders_isk_present_and_placeholder() {
      let present = base_model();
      let mut absent = base_model();
      absent.wallet_balance = None;

      let _present: Element<'_, Message> = card(&present, None, false);
      let _absent: Element<'_, Message> = card(&absent, None, false);
    }

    #[test]
    fn it_renders_every_docked_state() {
      for docked in [Some(true), Some(false), None] {
        let mut model = base_model();
        model.docked = docked;

        let _el: Element<'_, Message> = card(&model, None, false);
      }
    }

    #[test]
    fn it_renders_an_accented_squad_card() {
      let mut model = base_model();
      model.accent = Some(color::accent::PLASMA);

      let _el: Element<'_, Message> = card(&model, None, false);
    }

    #[test]
    fn it_renders_a_card_with_a_sync_error() {
      let model = base_model();

      for failure in [Phase::Failed, Phase::BackingOff] {
        let _el: Element<'_, Message> = card(&model, Some(failure), false);
      }
    }

    #[test]
    fn it_renders_the_tag_row_with_the_add_affordance_and_no_inline_picker() {
      let model = base_model();

      let _el: Element<'_, Message> = card(&model, None, false);
    }

    #[test]
    fn it_renders_a_card_being_dragged() {
      let model = base_model();

      let _el: Element<'_, Message> = card(&model, None, true);
    }

    #[test]
    fn it_renders_the_reauthorize_affordance_when_the_character_needs_reauth() {
      let mut model = base_model();
      model.needs_reauth = true;

      let _el: Element<'_, Message> = card(&model, Some(Phase::Failed), false);
    }

    #[test]
    fn it_renders_a_non_interactive_ghost_clone() {
      let mut model = base_model();
      model.accent = Some(color::accent::PLASMA);

      let _accented: Element<'_, Message> = ghost(&model);

      let mut plain = base_model();
      plain.accent = None;
      plain.training = None;
      plain.tags = Vec::new();
      let _plain: Element<'_, Message> = ghost(&plain);
    }
  }

  mod sizing {
    use iced::{Length, advanced::Widget};
    use pretty_assertions::assert_ne;

    use super::*;

    fn declared_height(model: &CardModel) -> Length {
      let element = card(model, None, false);
      Widget::<Message, _, _>::size(element.as_widget()).height
    }

    #[test]
    fn it_does_not_declare_a_fill_height_for_an_unaccented_card() {
      let mut model = base_model();
      model.accent = None;

      assert_ne!(declared_height(&model), Length::Fill);
    }

    #[test]
    fn it_does_not_declare_a_fill_height_for_an_accented_card() {
      let mut model = base_model();
      model.accent = Some(color::accent::PLASMA);

      assert_ne!(declared_height(&model), Length::Fill);
    }

    #[test]
    fn it_does_not_declare_a_fill_height_for_an_idle_card() {
      let mut model = base_model();
      model.training = None;

      assert_ne!(declared_height(&model), Length::Fill);
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
  }
}
