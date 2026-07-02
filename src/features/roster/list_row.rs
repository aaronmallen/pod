use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, Stack, button, container, mouse_area, text},
};

use super::{
  Message,
  card::{CardModel, Sections, Training, format_isk, reauth_badge},
  name_link::name_link,
};
use crate::{
  store::model::ENTITY_TYPE_CHARACTER,
  sync::Phase,
  ui::{
    components::{avatar::avatar, chip::Chip, eyebrow::eyebrow, progress_bar::progress_bar, rule, status},
    style::{color, spacing, typography},
  },
};

const ACCENT_WIDTH: f32 = 3.0;
const CHIP_GAP: f32 = 5.0;
const CHIP_RADIUS: f32 = 999.0;
const DOCKED_PILL_INSET: f32 = 8.0;
const DOCKED_PILL_RADIUS: f32 = 4.0;
const DOCKED_PILL_TEXT: f32 = 8.5;
const DOCKED_PILL_TEXT_ALPHA: f32 = 0.7;
const GRIP_DOT: f32 = 3.0;
const GRIP_DOT_ALPHA: f32 = 0.55;
const GRIP_GAP: f32 = 3.0;
const HAIRLINE: f32 = 1.0;
const IDLE_DOT: f32 = 6.0;
const IDLE_GAP: f32 = 7.0;
const ISK_VALUE_SIZE: f32 = 20.0;
const LOCATION_DOT: f32 = 6.0;
const LOCATION_GAP: f32 = 7.0;
const MAX_TAGS: usize = 4;
const PLACEHOLDER: &str = "—";
const PORTRAIT_WIDTH: f32 = 120.0;
const PROGRESS_HEIGHT: f32 = 3.0;
const PROGRESS_WIDTH: f32 = 120.0;
const RAIL_WIDTH: f32 = 30.0;
const RIGHT_WIDTH: f32 = 140.0;
const ROW_GAP: f32 = 9.0;
const ROW_HEIGHT: f32 = 112.0;
const ROW_INLINE_GAP: f32 = 10.0;
const SKILL_MAX_WIDTH: f32 = 200.0;
const TINT_ALPHA: f32 = 0.55;
const TRACK_ALPHA: f32 = 0.1;

pub(super) fn list_row<'a>(
  model: &'a CardModel,
  failure: Option<Phase>,
  dragging: bool,
  sections: Sections,
) -> Element<'a, Message> {
  let tinted = model.needs_reauth || is_failing(failure);

  let mut columns: Vec<Element<'a, Message>> = Vec::with_capacity(7);
  if let Some(accent) = model.accent {
    columns.push(accent_stripe(accent));
  }
  columns.push(drag_rail());
  columns.push(rule::vertical_fill(TRACK_ALPHA));
  columns.push(portrait(model));
  columns.push(center(model, sections));
  columns.push(rule::vertical_fill(TRACK_ALPHA));
  columns.push(right_panel(model));

  let body = container(Row::with_children(columns).height(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fixed(ROW_HEIGHT))
    .clip(true)
    .style(surface(dragging, tinted));

  mouse_area(body)
    .on_right_press(Message::CardRightPressed(model.character_id))
    .into()
}

fn accent_stripe<'a>(accent: Color) -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(ACCENT_WIDTH))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(accent)),
      ..container::Style::default()
    })
    .into()
}

fn drag_rail<'a>() -> Element<'a, Message> {
  container(grip())
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

fn grip<'a>() -> Element<'a, Message> {
  let dot = || status::dot_sized(color::with_alpha(color::text::PRIMARY, GRIP_DOT_ALPHA), GRIP_DOT);
  let dots = || Row::with_children(vec![dot(), dot()]).spacing(GRIP_GAP).into();

  Column::with_children(vec![dots(), dots(), dots()])
    .spacing(GRIP_GAP)
    .into()
}

fn portrait(model: &CardModel) -> Element<'_, Message> {
  let strip = container(avatar(
    model.character_id,
    &model.name,
    Length::Fill,
    ROW_HEIGHT,
    model.portrait.path(),
  ))
  .width(Length::Fixed(PORTRAIT_WIDTH))
  .height(Length::Fixed(ROW_HEIGHT))
  .clip(true);

  match status_label(model.docked) {
    Some(label) => {
      let pill = container(
        text(label)
          .font(typography::mono::REGULAR)
          .size(DOCKED_PILL_TEXT)
          .style(|_| text::Style {
            color: Some(color::with_alpha(color::text::PRIMARY, DOCKED_PILL_TEXT_ALPHA)),
          }),
      )
      .padding([GRIP_GAP, spacing::UNIT + 2.0])
      .style(|_| container::Style {
        background: Some(Background::Color(color::state::OVERLAY_DARK)),
        border: Border {
          radius: DOCKED_PILL_RADIUS.into(),
          ..Border::default()
        },
        ..container::Style::default()
      });

      Stack::with_children(vec![
        strip.into(),
        container(pill)
          .width(Length::Fixed(PORTRAIT_WIDTH))
          .height(Length::Fixed(ROW_HEIGHT))
          .align_x(Horizontal::Left)
          .align_y(Vertical::Bottom)
          .padding(DOCKED_PILL_INSET)
          .into(),
      ])
      .width(Length::Fixed(PORTRAIT_WIDTH))
      .height(Length::Fixed(ROW_HEIGHT))
      .into()
    }
    None => strip.into(),
  }
}

fn center<'a>(model: &'a CardModel, sections: Sections) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = vec![identity_row(model, sections.detail_enabled), tag_row(model)];
  if sections.location_enabled || sections.training_enabled {
    rows.push(context_row(model, sections));
  }

  container(Column::with_children(rows).spacing(ROW_GAP).width(Length::Fill))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: spacing::SPACE_3 + HAIRLINE,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3 + HAIRLINE,
      left: spacing::SPACE_4_5,
    })
    .into()
}

fn identity_row(model: &CardModel, detail_enabled: bool) -> Element<'_, Message> {
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

  let mut children: Vec<Element<'_, Message>> = vec![name];
  if model.needs_reauth {
    children.push(reauth_badge(Message::ReauthCharacterRequested(model.character_id)));
  }
  children.push(Space::new().width(Length::Fill).into());
  children.push(
    text(model.corp_ticker.clone())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .wrapping(text::Wrapping::None)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  );

  Row::with_children(children)
    .spacing(ROW_INLINE_GAP)
    .align_y(Vertical::Center)
    .into()
}

fn tag_row(model: &CardModel) -> Element<'_, Message> {
  let mut chips: Vec<Element<'_, Message>> = model
    .tags
    .iter()
    .take(MAX_TAGS)
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

  let overflow = model.tags.len().saturating_sub(MAX_TAGS);
  if overflow > 0 {
    chips.push(
      text(format!("+{overflow}"))
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  chips.push(add_tag_affordance(model.character_id));

  Row::with_children(chips)
    .spacing(CHIP_GAP)
    .align_y(Vertical::Center)
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

fn context_row(model: &CardModel, sections: Sections) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = Vec::with_capacity(2);
  if sections.location_enabled {
    children.push(location_row(model));
  } else {
    children.push(Space::new().width(Length::Fill).into());
  }
  if sections.training_enabled {
    children.push(training_row(model));
  }

  Row::with_children(children)
    .spacing(spacing::SPACE_4_5)
    .align_y(Vertical::Center)
    .into()
}

fn location_row(model: &CardModel) -> Element<'_, Message> {
  let dot_color = match model.docked {
    Some(false) => color::status::WARNING,
    Some(true) | None => color::text::tertiary(),
  };
  let name = model.location.clone().unwrap_or_else(|| PLACEHOLDER.to_owned());

  container(
    Row::with_children(vec![
      status::dot_sized(dot_color, LOCATION_DOT),
      text(name)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .wrapping(text::Wrapping::None)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(LOCATION_GAP)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .clip(true)
  .into()
}

fn training_row(model: &CardModel) -> Element<'_, Message> {
  match &model.training {
    Some(training) => active_training(training),
    None => idle_training(),
  }
}

fn active_training(training: &Training) -> Element<'_, Message> {
  let skill = container(
    text(format!("{} {}", training.skill, roman(training.level)))
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .wrapping(text::Wrapping::None)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .max_width(SKILL_MAX_WIDTH)
  .clip(true);

  let fill = if training.paused.is_some() {
    color::text::secondary()
  } else {
    color::accent()
  };

  let bar = container(progress_bar(training.progress, fill, PROGRESS_HEIGHT)).width(Length::Fixed(PROGRESS_WIDTH));

  let remaining = text(training.paused.map_or_else(|| training.remaining.clone(), paused_label))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .wrapping(text::Wrapping::None)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  Row::with_children(vec![skill.into(), bar.into(), remaining.into()])
    .spacing(ROW_INLINE_GAP)
    .align_y(Vertical::Center)
    .into()
}

fn idle_training<'a>() -> Element<'a, Message> {
  Row::with_children(vec![
    container(status::dot_sized(color::status::DANGER, IDLE_DOT))
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
  .spacing(IDLE_GAP)
  .align_y(Vertical::Center)
  .into()
}

fn right_panel<'a>(model: &'a CardModel) -> Element<'a, Message> {
  container(stat_block(
    t!("roster.card.isk").into_owned(),
    format_isk(model.wallet_balance),
    typography::mono::MEDIUM,
    ISK_VALUE_SIZE,
    color::accent(),
  ))
  .width(Length::Fixed(RIGHT_WIDTH))
  .height(Length::Fill)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: spacing::SPACE_3 + HAIRLINE,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3 + HAIRLINE,
    left: spacing::SPACE_4_5,
  })
  .into()
}

fn stat_block<'a>(
  label: String,
  value: String,
  value_font: iced::Font,
  value_size: f32,
  value_color: Color,
) -> Element<'a, Message> {
  let label = container(eyebrow(&label, None))
    .width(Length::Fill)
    .align_x(Horizontal::Right);

  let value = container(
    text(value)
      .font(value_font)
      .size(value_size)
      .wrapping(text::Wrapping::None)
      .style(move |_| text::Style {
        color: Some(value_color),
      }),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Right);

  Column::with_children(vec![label.into(), value.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill)
    .into()
}

fn is_failing(failure: Option<Phase>) -> bool {
  matches!(failure, Some(Phase::BackingOff | Phase::Failed))
}

fn paused_label(queued: usize) -> String {
  let key = if queued == 1 {
    "roster.card.paused_queued_one"
  } else {
    "roster.card.paused_queued_other"
  };
  t!(key, count => queued).into_owned()
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

fn status_label(docked: Option<bool>) -> Option<String> {
  match docked {
    Some(true) => Some(t!("roster.card.docked").into_owned()),
    Some(false) => Some(t!("roster.card.in_space").into_owned()),
    None => None,
  }
}

fn surface(dragging: bool, tinted: bool) -> impl Fn(&iced::Theme) -> container::Style {
  move |theme: &iced::Theme| {
    let mut style = crate::ui::style::control::card(theme);
    if tinted {
      style.border.color = color::with_alpha(color::status::DANGER, TINT_ALPHA);
    }
    if dragging {
      style.border.color = color::accent();
    }
    style
  }
}

#[cfg(test)]
mod tests {
  use super::{super::card::TagChip, *};
  use crate::store::images;

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
    fn it_renders_a_training_list_row() {
      let model = base_model();

      let _el: Element<'_, Message> = list_row(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_an_accented_squad_row() {
      let mut model = base_model();
      model.accent = Some(color::accent());

      let _el: Element<'_, Message> = list_row(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_an_idle_row() {
      let mut model = base_model();
      model.training = None;

      let _el: Element<'_, Message> = list_row(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_a_needs_reauth_row_with_a_sync_failure() {
      let mut model = base_model();
      model.needs_reauth = true;

      let _el: Element<'_, Message> = list_row(&model, Some(Phase::Failed), false, all_sections());
    }

    #[test]
    fn it_renders_a_row_being_dragged() {
      let model = base_model();

      let _el: Element<'_, Message> = list_row(&model, None, true, all_sections());
    }

    #[test]
    fn it_renders_every_docked_state() {
      for docked in [Some(true), Some(false), None] {
        let mut model = base_model();
        model.docked = docked;

        let _el: Element<'_, Message> = list_row(&model, None, false, all_sections());
      }
    }

    #[test]
    fn it_caps_the_tag_row_and_shows_an_overflow_count() {
      let mut model = base_model();
      model.tags = (0..7)
        .map(|id| TagChip {
          color: None,
          id,
          name: format!("Tag {id}"),
        })
        .collect();

      let _el: Element<'_, Message> = list_row(&model, None, false, all_sections());
    }

    #[test]
    fn it_hides_the_training_and_location_sections_when_disabled() {
      let model = base_model();
      let sections = Sections {
        detail_enabled: true,
        location_enabled: false,
        training_enabled: false,
      };

      let _el: Element<'_, Message> = list_row(&model, None, false, sections);
    }
  }

  mod is_failing {
    use super::*;

    #[test]
    fn it_flags_only_failing_or_backing_off_phases() {
      assert!(is_failing(Some(Phase::Failed)));
      assert!(is_failing(Some(Phase::BackingOff)));

      assert!(!is_failing(None));
      assert!(!is_failing(Some(Phase::Syncing)));
      assert!(!is_failing(Some(Phase::Done)));
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
    fn it_labels_docked_in_space_and_stays_silent_when_unknown() {
      assert_eq!(status_label(Some(true)), Some(t!("roster.card.docked").into_owned()));
      assert_eq!(status_label(Some(false)), Some(t!("roster.card.in_space").into_owned()));
      assert_eq!(status_label(None), None);
    }
  }
}
