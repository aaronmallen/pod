pub mod character_detail;
pub mod character_location;
pub mod character_portrait;
pub mod character_skill_training;
pub mod character_wallet;
pub mod status_pill;

pub use character_detail::Component as CharacterDetail;
pub use character_location::Component as CharacterLocation;
pub use character_portrait::Component as CharacterPortrait;
pub use character_skill_training::Component as CharacterSkillTraining;
pub use character_wallet::Component as CharacterWallet;
use iced::{
  Background, Border, Color, Element, Length, Padding,
  widget::{button, column, container, mouse_area, row, text},
};
use pod_model::Character;
pub use status_pill::Component as StatusPill;

use crate::{
  components,
  style::{color, radius, shadow, spacing, typography},
};

#[derive(Clone, Debug, Default)]
pub struct State;

#[derive(Clone, Debug)]
pub enum Message {
  CardEntered(i64),
  CardPressed(i64),
  CardRightPressed(i64, String),
  /// The character's name row was clicked; navigate to their detail view.
  NamePressed(i64),
  SkillTrainingPressed(i64),
  TagsPressed(i64),
  WalletPressed(i64),
}

pub struct Component<'a> {
  character: &'a Character,
  is_dragging: bool,
  is_elevated: bool,
  is_hover_target: bool,
  portrait_handle: Option<&'a iced::widget::image::Handle>,
}

impl<'a> Component<'a> {
  pub fn new(character: &'a Character) -> Self {
    Self {
      character,
      is_dragging: false,
      is_elevated: false,
      is_hover_target: false,
      portrait_handle: None,
    }
  }

  pub fn is_dragging(mut self, v: bool) -> Self {
    self.is_dragging = v;
    self
  }

  pub fn is_elevated(mut self, v: bool) -> Self {
    self.is_elevated = v;
    self
  }

  pub fn is_hover_target(mut self, v: bool) -> Self {
    self.is_hover_target = v;
    self
  }

  pub fn portrait_handle(mut self, handle: Option<&'a iced::widget::image::Handle>) -> Self {
    self.portrait_handle = handle;
    self
  }

  pub fn render(self) -> Element<'a, Message> {
    let id = *self.character.id();
    let is_elevated = self.is_elevated;
    let is_hover_target = self.is_hover_target;

    // Card has been picked up — leave an empty slot in the grid.
    if self.is_dragging {
      let mut bg = color::surface::RAISED;
      bg.a = 0.2;
      return container(iced::widget::Space::new().width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(spacing::layout::CHARACTER_CARD_HEIGHT)
        .style(move |_| container::Style {
          background: Some(Background::Color(bg)),
          border: Border {
            color: color::border::SUBTLE,
            radius: radius::PANEL.into(),
            width: 1.0,
          },
          ..container::Style::default()
        })
        .into();
    }

    let portrait = CharacterPortrait::new(self.character)
      .portrait_handle(self.portrait_handle)
      .render::<Message>();

    let identity = identity_row(self.character);
    let tags = tags_row(self.character);
    let training = training_row(self.character, id);
    let location_wallet = stats_row(self.character, id);

    let card_content = column([
      portrait,
      identity,
      tags,
      components::Separator::horizontal().render(),
      training,
      components::Separator::horizontal().render(),
      location_wallet,
    ])
    .width(Length::Fill)
    .height(Length::Fill);

    let border_color = if is_hover_target {
      color::accent::PLASMA
    } else {
      color::border::SUBTLE
    };
    let border_width = if is_hover_target { 2.0 } else { 1.0 };
    let bg = color::surface::RAISED;

    let card = container(card_content)
      .width(Length::Fill)
      .height(spacing::layout::CHARACTER_CARD_HEIGHT)
      .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
          color: border_color,
          radius: radius::PANEL.into(),
          width: border_width,
        },
        shadow: if is_elevated { shadow::POPOVER } else { shadow::NONE },
        ..container::Style::default()
      });

    let name = self.character.name().clone();
    mouse_area(card)
      .on_press(Message::CardPressed(id))
      .on_enter(Message::CardEntered(id))
      .on_right_press(Message::CardRightPressed(id, name))
      .interaction(iced::mouse::Interaction::Grab)
      .into()
  }
}

fn identity_row<'a>(character: &'a Character) -> Element<'a, Message> {
  let id = *character.id();
  mouse_area(
    container(CharacterDetail::new(character).render::<Message>()).padding(iced::Padding {
      right: spacing::SPACE_4,
      ..iced::Padding::ZERO
    }),
  )
  .on_press(Message::NamePressed(id))
  .interaction(iced::mouse::Interaction::Pointer)
  .into()
}

fn tags_row<'a>(character: &'a Character) -> Element<'a, Message> {
  let id = *character.id();
  let plus_btn = button(
    text("+")
      .font(typography::body::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 4.0,
    right: 4.0,
  })
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.08))),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..button::Style::default()
  })
  .on_press(Message::TagsPressed(id));

  let mut items: Vec<Element<'a, Message>> = character
    .tags()
    .iter()
    .map(|(_, name)| components::Badge::tag(name).render::<Message>())
    .collect();
  items.push(plus_btn.into());

  container(row(items).spacing(spacing::SPACE_1).wrap())
    .padding(Padding {
      top: 0.0,
      bottom: 10.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn training_row<'a>(character: &'a Character, id: i64) -> Element<'a, Message> {
  mouse_area(CharacterSkillTraining::new(character).render::<Message>())
    .on_press(Message::SkillTrainingPressed(id))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

fn stats_row<'a>(character: &'a Character, id: i64) -> Element<'a, Message> {
  let location = character.location_name().as_deref();
  row([
    CharacterLocation::new(location).render::<Message>(),
    stat_divider(),
    mouse_area(CharacterWallet::new(character).render::<Message>())
      .on_press(Message::WalletPressed(id))
      .interaction(iced::mouse::Interaction::Pointer)
      .into(),
  ])
  .into()
}

fn stat_divider<'a>() -> Element<'a, Message> {
  container(iced::widget::Space::new().height(Length::Fill))
    .width(1.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}
