use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{ListModel, Message, Pilot, State, pilot_avatar};
use crate::ui::{
  components::{button::Button, icon::Icon},
  style::{color, radius, spacing, typography},
};

const ACTION_SIZE: f32 = 32.0;
const AVATAR_OVERLAP: f32 = -7.0;
const CARD_AVATAR: f32 = 26.0;
const CARD_GAP: f32 = 18.0;
const HINT_MAX_WIDTH: f32 = 380.0;
const MAX_TARGET_AVATARS: usize = 5;
const TALLY_DOT: f32 = 7.0;

pub(super) fn screen(state: &State) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![meta_row(state)];

  if state.lists.is_empty() {
    children.push(empty_state());
  } else {
    for list in &state.lists {
      children.push(list_card(state, list));
    }
  }

  Column::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn action_button<'a>(icon: Icon, danger: bool, message: Message) -> Element<'a, Message> {
  button(
    container(icon.size(15.0).color(color::text::tertiary()).render())
      .width(Length::Fixed(ACTION_SIZE))
      .height(Length::Fixed(ACTION_SIZE))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(message)
  .style(move |_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let (tint, background) = if hover && danger {
      (color::status::DANGER, color::with_alpha(color::status::DANGER, 0.12))
    } else if hover {
      (color::text::PRIMARY, color::with_alpha(color::text::PRIMARY, 0.06))
    } else {
      (color::text::tertiary(), iced::Color::TRANSPARENT)
    };
    button::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: iced::Color::TRANSPARENT,
        radius: radius::CONTROL.into(),
        width: 0.0,
      },
      text_color: tint,
      ..button::Style::default()
    }
  })
  .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  let title = text(t!("contact_sync.empty_title"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG - 1.0)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });
  let hint = text(t!("contact_sync.empty_hint").to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });
  let create = Button::primary(t!("contact_sync.new_list"))
    .icon(Icon::plus())
    .on_press(Message::CreateList)
    .into();

  container(
    Column::with_children(vec![title.into(), hint.into(), create])
      .spacing(spacing::SPACE_3_5)
      .align_x(Horizontal::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 64.0,
    right: spacing::SPACE_6,
    bottom: 64.0,
    left: spacing::SPACE_6,
  })
  .align_x(Horizontal::Center)
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn list_card<'a>(state: &'a State, list: &'a ListModel) -> Element<'a, Message> {
  let count = list.contacts.len();
  let count_key = if count == 1 {
    t!("contact_sync.contact_count_one")
  } else {
    t!("contact_sync.contact_count", count => count)
  };
  let title = Row::with_children(vec![
    text(list.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(count_key.into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let identity = Column::with_children(vec![title.into(), standing_tally(list)])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill);

  let actions = Row::with_children(vec![
    action_button(Icon::pencil(), false, Message::ListOpened(list.id)),
    action_button(Icon::trash(), true, Message::ListDeleteRequested(list.id)),
  ])
  .spacing(spacing::UNIT)
  .align_y(Vertical::Center);

  let row = Row::with_children(vec![identity.into(), target_cluster(state, list), actions.into()])
    .spacing(CARD_GAP)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  button(container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_3_5 + 2.0,
    right: CARD_GAP,
    bottom: spacing::SPACE_3_5 + 2.0,
    left: CARD_GAP,
  }))
  .padding(0)
  .width(Length::Fill)
  .on_press(Message::ListOpened(list.id))
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: if hover { color::rule_strong() } else { color::rule() },
        radius: radius::CARD.into(),
        width: 1.0,
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  })
  .into()
}

fn meta_row(state: &State) -> Element<'_, Message> {
  let eyebrow = text(t!("contact_sync.sync_lists").to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });
  let count = text(state.lists.len().to_string())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::accent()),
    });
  let rule = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    });
  let hint = container(
    text(t!("contact_sync.index_hint"))
      .font(typography::body::REGULAR)
      .size(typography::size::SM + 1.0)
      .align_x(Horizontal::Right)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .max_width(HINT_MAX_WIDTH);

  Row::with_children(vec![eyebrow.into(), count.into(), rule.into(), hint.into()])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn standing_tally(list: &ListModel) -> Element<'_, Message> {
  let red = list.contacts.iter().filter(|contact| contact.standing() < 0).count();
  let neutral = list.contacts.iter().filter(|contact| contact.standing() == 0).count();
  let blue = list.contacts.iter().filter(|contact| contact.standing() > 0).count();

  if red == 0 && neutral == 0 && blue == 0 {
    return text(t!("contact_sync.no_standings"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into();
  }

  let mut cells: Vec<Element<'_, Message>> = Vec::new();
  if red > 0 {
    let label = if red == 1 {
      t!("contact_sync.tally_red_one").into_owned()
    } else {
      t!("contact_sync.tally_red", count => red).into_owned()
    };
    cells.push(tally_cell(label, color::status::DANGER));
  }
  if neutral > 0 {
    cells.push(tally_cell(
      t!("contact_sync.tally_neutral", count => neutral).into_owned(),
      color::text::secondary(),
    ));
  }
  if blue > 0 {
    let label = if blue == 1 {
      t!("contact_sync.tally_blue_one").into_owned()
    } else {
      t!("contact_sync.tally_blue", count => blue).into_owned()
    };
    cells.push(tally_cell(label, color::status::ONLINE));
  }

  Row::with_children(cells)
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center)
    .into()
}

fn tally_cell<'a>(label: String, tint: iced::Color) -> Element<'a, Message> {
  let dot = container(
    Space::new()
      .width(Length::Fixed(TALLY_DOT))
      .height(Length::Fixed(TALLY_DOT)),
  )
  .style(move |_| container::Style {
    background: Some(Background::Color(tint)),
    border: Border {
      color: iced::Color::TRANSPARENT,
      radius: 999.0.into(),
      width: 0.0,
    },
    ..container::Style::default()
  });
  let label = text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  Row::with_children(vec![dot.into(), label.into()])
    .spacing(spacing::UNIT + 1.0)
    .align_y(Vertical::Center)
    .into()
}

fn target_cluster<'a>(state: &'a State, list: &'a ListModel) -> Element<'a, Message> {
  let targets: Vec<&Pilot> = list
    .target_ids
    .iter()
    .filter_map(|id| state.pilots.iter().find(|pilot| pilot.character_id == *id))
    .collect();

  if targets.is_empty() {
    return text(t!("contact_sync.no_pilots").to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::status::WARNING),
      })
      .into();
  }

  let avatars = Row::with_children(
    targets
      .iter()
      .take(MAX_TARGET_AVATARS)
      .map(|pilot| pilot_avatar(pilot, CARD_AVATAR, Some(color::surface::RAISED)))
      .collect::<Vec<_>>(),
  )
  .spacing(AVATAR_OVERLAP);

  let overflow = targets.len().saturating_sub(MAX_TARGET_AVATARS);
  let label = if targets.len() == 1 {
    t!("contact_sync.pilot_count_one").into_owned()
  } else {
    t!("contact_sync.pilot_count", count => targets.len()).into_owned()
  };
  let summary = if overflow > 0 {
    format!("+{overflow} {label}")
  } else {
    label
  };

  Row::with_children(vec![
    avatars.into(),
    text(summary)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center)
  .into()
}
