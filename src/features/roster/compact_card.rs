use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  border::Radius,
  widget::{Column, Row, Space, Stack, button, container, mouse_area, svg, text},
};

use super::{
  Message,
  card::{CardModel, Sections, format_isk},
  name_link::name_link,
};
use crate::{
  store::model::ENTITY_TYPE_CHARACTER,
  sync::Phase,
  ui::{
    components::{
      avatar::Avatar,
      chip::{chip, overflow_chip, overflow_count},
      progress_bar::progress_bar,
      rule, status,
    },
    style::{color, radius, spacing, typography},
  },
};

const ACCENT_WIDTH: f32 = 3.0;
const ALERT_ICON: &[u8] = include_bytes!("../../../assets/images/icons/alert-triangle.svg");
const AVATAR_RADIUS: f32 = 9.0;
const AVATAR_SIZE: f32 = 46.0;
const CARD_PAD_X: f32 = 16.0;
const CHIP_CAP: usize = 3;
const CHIP_GAP: f32 = 5.0;
const CHIP_RADIUS: f32 = 999.0;
const GRIP_DOT: f32 = 3.0;
const GRIP_GAP: f32 = 3.0;
const HAIRLINE: f32 = 1.0;
const ISK_SIZE: f32 = 16.0;
const LOCATION_DOT: f32 = 5.0;
/// `1.3` mirrors iced's default `LineHeight::Relative(1.3)`, so the 2-line cap tracks how the
/// renderer actually lays out the location text.
const LOCATION_LINE_HEIGHT: f32 = typography::size::XS_PLUS * 1.3;
const LOCATION_MAX_HEIGHT: f32 = LOCATION_LINE_HEIGHT * 2.0;
const NAME_SIZE: f32 = 16.0;
const PLACEHOLDER: &str = "—";
const PROGRESS_HEIGHT: f32 = 4.0;
const TOKEN_ALERT_GAP: f32 = 5.0;
const TOKEN_ALERT_ICON: f32 = 11.0;
const TOKEN_ALERT_RADIUS: f32 = 5.0;
const TOKEN_BORDER_ALPHA: f32 = 0.55;
const TRAINING_GAP: f32 = 7.0;
const TRAINING_PAD_Y: f32 = 11.0;

pub(super) fn compact_card<'a>(
  model: &'a CardModel,
  failure: Option<Phase>,
  dragging: bool,
  sections: Sections,
) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![header(model, sections.detail_enabled), tag_row(model)];
  if sections.training_enabled {
    children.push(rule::horizontal());
    children.push(training_section(model));
  }
  children.push(rule::horizontal());
  children.push(footer(model, sections.location_enabled));

  if let Some(indicator) = sync_indicator(failure) {
    children.push(indicator);
  }

  let body = container(Column::with_children(children))
    .width(Length::Fill)
    .height(Length::Fixed(super::COMPACT_CARD_HEIGHT))
    .clip(true)
    .style(card_surface(model.accent.is_some(), dragging, model.needs_reauth));

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
      .align_x(iced::alignment::Horizontal::Left);
      Stack::with_children(vec![base.into(), bar.into()])
        .width(Length::Fill)
        .into()
    }
    None => body.into(),
  };

  mouse_area(composed)
    .on_right_press(Message::CardRightPressed(model.character_id))
    .into()
}

fn header(model: &CardModel, detail_enabled: bool) -> Element<'_, Message> {
  let portrait = Avatar::new(
    model.character_id,
    &model.name,
    Length::Fixed(AVATAR_SIZE),
    AVATAR_SIZE,
    model.portrait.path(),
  )
  .radius(AVATAR_RADIUS)
  .view::<Message>();

  let mut children: Vec<Element<'_, Message>> = vec![
    drag_grip(model.character_id),
    portrait,
    container(identity(model, detail_enabled)).width(Length::Fill).into(),
  ];
  if model.needs_reauth {
    children.push(token_alert(Message::ReauthCharacterRequested(model.character_id)));
  }

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: CARD_PAD_X,
    bottom: spacing::SPACE_3,
    left: CARD_PAD_X,
  })
  .into()
}

fn drag_grip<'a>(character_id: i64) -> Element<'a, Message> {
  let dot = || status::dot_sized(color::text::tertiary(), GRIP_DOT);
  let grip_row = || Row::with_children(vec![dot(), dot()]).spacing(GRIP_GAP).into();
  let dots = Column::with_children(vec![grip_row(), grip_row(), grip_row()]).spacing(GRIP_GAP);

  mouse_area(dots)
    .interaction(iced::mouse::Interaction::Grab)
    .on_press(Message::PickUpCard(character_id))
    .into()
}

fn identity(model: &CardModel, detail_enabled: bool) -> Element<'_, Message> {
  let name: Element<'_, Message> = if detail_enabled {
    name_link(
      model.name.clone(),
      NAME_SIZE,
      Message::CharacterSelected(model.character_id),
    )
  } else {
    text(model.name.clone())
      .font(typography::body::MEDIUM)
      .size(NAME_SIZE)
      .wrapping(text::Wrapping::None)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into()
  };

  let corp = text(model.corp_ticker.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .wrapping(text::Wrapping::None)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  Column::with_children(vec![name, corp.into()])
    .spacing(spacing::UNIT - 1.0)
    .into()
}

fn token_alert<'a>(on_press: Message) -> Element<'a, Message> {
  let glyph = svg(svg::Handle::from_memory(ALERT_ICON))
    .width(Length::Fixed(TOKEN_ALERT_ICON))
    .height(Length::Fixed(TOKEN_ALERT_ICON))
    .style(|_, _| svg::Style {
      color: Some(color::status::DANGER_INK),
    });

  let label = text(t!("roster.compact.token_badge").to_uppercase())
    .font(typography::mono::SEMIBOLD)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::status::DANGER_INK),
    });

  button(
    Row::with_children(vec![glyph.into(), label.into()])
      .spacing(TOKEN_ALERT_GAP)
      .align_y(Vertical::Center),
  )
  .padding([GRIP_GAP, spacing::SPACE_2])
  .on_press(on_press)
  .style(token_alert_style)
  .into()
}

fn token_alert_style(_theme: &iced::Theme, _status: button::Status) -> button::Style {
  button::Style {
    background: Some(Background::Color(color::status::DANGER)),
    text_color: color::status::DANGER_INK,
    border: Border {
      radius: TOKEN_ALERT_RADIUS.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn tag_row(model: &CardModel) -> Element<'_, Message> {
  let mut chips: Vec<Element<'_, Message>> = model
    .tags
    .iter()
    .take(CHIP_CAP)
    .map(|tag| chip(tag.name.clone(), tag.color))
    .collect();

  if let Some(extra) = overflow_count(model.tags.len(), CHIP_CAP) {
    chips.push(overflow_chip(extra));
  }
  chips.push(add_tag_affordance(model.character_id));

  container(Row::with_children(chips).spacing(CHIP_GAP).align_y(Vertical::Center))
    .padding(Padding {
      top: 0.0,
      right: CARD_PAD_X,
      bottom: spacing::SPACE_3,
      left: CARD_PAD_X,
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
  let body: Element<'_, Message> = match &model.training {
    Some(training) => {
      let skill = Row::with_children(vec![
        text(training.skill.clone())
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .wrapping(text::Wrapping::None)
          .style(|_| text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
        text(roman(training.level))
          .font(typography::body::REGULAR)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          })
          .into(),
      ])
      .spacing(spacing::UNIT + 2.0)
      .align_y(Vertical::Bottom);

      let remaining = text(training.remaining.clone())
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        });

      let heading = Row::with_children(vec![container(skill).width(Length::Fill).into(), remaining.into()])
        .align_y(Vertical::Bottom)
        .width(Length::Fill);

      let progress_color = if training.paused.is_some() {
        color::text::secondary()
      } else {
        color::accent()
      };

      Column::with_children(vec![
        heading.into(),
        progress_bar(training.progress, progress_color, PROGRESS_HEIGHT),
      ])
      .spacing(TRAINING_GAP)
      .into()
    }
    None => idle_state(),
  };

  container(body)
    .padding(Padding {
      top: TRAINING_PAD_Y,
      right: CARD_PAD_X,
      bottom: TRAINING_PAD_Y,
      left: CARD_PAD_X,
    })
    .into()
}

fn idle_state<'a>() -> Element<'a, Message> {
  Row::with_children(vec![
    status::dot(color::status::DANGER),
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

fn footer(model: &CardModel, location_enabled: bool) -> Element<'_, Message> {
  let isk = text(format_isk(model.wallet_balance))
    .font(typography::mono::MEDIUM)
    .size(ISK_SIZE)
    .style(|_| text::Style {
      color: Some(color::accent()),
    });

  let mut children: Vec<Element<'_, Message>> = Vec::with_capacity(2);
  if location_enabled {
    children.push(container(location(model)).width(Length::Fill).into());
  } else {
    children.push(Space::new().width(Length::Fill).into());
  }
  children.push(isk.into());

  container(
    Row::with_children(children)
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::SPACE_2_5,
    right: CARD_PAD_X,
    bottom: spacing::SPACE_2_5,
    left: CARD_PAD_X,
  })
  .into()
}

fn location(model: &CardModel) -> Element<'_, Message> {
  let dot_color = match model.docked {
    Some(false) => color::status::WARNING,
    Some(true) | None => color::text::tertiary(),
  };
  let name = model.location.clone().unwrap_or_else(|| PLACEHOLDER.to_owned());

  let dot = container(status::dot_sized(dot_color, LOCATION_DOT))
    .height(Length::Fixed(LOCATION_LINE_HEIGHT))
    .align_y(Vertical::Center);

  let label = container(
    text(name)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(Length::Fill)
      .wrapping(text::Wrapping::Word)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .width(Length::Fill)
  .max_height(LOCATION_MAX_HEIGHT)
  .clip(true);

  Row::with_children(vec![dot.into(), label.into()])
    .spacing(spacing::UNIT + 2.0)
    .align_y(Vertical::Top)
    .into()
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
      right: CARD_PAD_X,
      bottom: spacing::SPACE_3,
      left: CARD_PAD_X,
    })
    .into(),
  )
}

fn card_surface(has_accent: bool, dragging: bool, needs_reauth: bool) -> impl Fn(&iced::Theme) -> container::Style {
  move |theme: &iced::Theme| {
    let mut style = crate::ui::style::control::card(theme);
    if needs_reauth {
      style.border.color = color::with_alpha(color::status::DANGER, TOKEN_BORDER_ALPHA);
    }
    if dragging {
      style.border.color = color::accent();
    }
    if has_accent {
      style.border.radius = Radius {
        top_left: 0.0,
        bottom_left: 0.0,
        ..radius::CARD.into()
      };
    }
    style
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
  use crate::store::images;

  fn base_model() -> CardModel {
    use super::super::card::{TagChip, Training};

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
      tags: vec![
        TagChip {
          color: Some(color::accent()),
          id: 1,
          name: "Main".to_owned(),
        },
        TagChip {
          color: None,
          id: 2,
          name: "Trader".to_owned(),
        },
        TagChip {
          color: None,
          id: 3,
          name: "Hauler".to_owned(),
        },
        TagChip {
          color: None,
          id: 4,
          name: "Miner".to_owned(),
        },
        TagChip {
          color: None,
          id: 5,
          name: "Scout".to_owned(),
        },
      ],
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
    fn it_renders_a_training_card_with_capped_tags() {
      let model = base_model();

      let _el: Element<'_, Message> = compact_card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_an_idle_card() {
      let mut model = base_model();
      model.training = None;

      let _el: Element<'_, Message> = compact_card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_a_dragged_card() {
      let model = base_model();

      let _el: Element<'_, Message> = compact_card(&model, None, true, all_sections());
    }

    #[test]
    fn it_renders_a_reauth_card_with_the_token_alert() {
      let mut model = base_model();
      model.needs_reauth = true;

      let _el: Element<'_, Message> = compact_card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_an_accented_squad_card() {
      let mut model = base_model();
      model.accent = Some(color::accent());

      let _el: Element<'_, Message> = compact_card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_with_a_plain_name_when_detail_is_disabled() {
      let model = base_model();
      let sections = Sections {
        detail_enabled: false,
        ..all_sections()
      };

      let _el: Element<'_, Message> = compact_card(&model, None, false, sections);
    }

    #[test]
    fn it_renders_with_the_training_and_location_sections_hidden() {
      let model = base_model();
      let sections = Sections {
        detail_enabled: true,
        location_enabled: false,
        training_enabled: false,
      };

      let _el: Element<'_, Message> = compact_card(&model, None, false, sections);
    }

    #[test]
    fn it_renders_a_card_with_a_long_wrapping_location() {
      let mut model = base_model();
      model.location = Some("Jita IV - Moon 4 - Caldari Navy Assembly Plant".to_owned());

      let _el: Element<'_, Message> = compact_card(&model, None, false, all_sections());
    }

    #[test]
    fn it_renders_a_card_with_a_sync_error() {
      let model = base_model();

      for failure in [Phase::Failed, Phase::BackingOff] {
        let _el: Element<'_, Message> = compact_card(&model, Some(failure), false, all_sections());
      }
    }

    #[test]
    fn it_declares_the_compact_card_height() {
      use iced::advanced::Widget;
      use pretty_assertions::assert_eq;

      let model = base_model();

      let element = compact_card(&model, None, false, all_sections());
      assert_eq!(
        Widget::<Message, _, _>::size(element.as_widget()).height,
        Length::Fixed(crate::features::roster::COMPACT_CARD_HEIGHT),
      );
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
}
