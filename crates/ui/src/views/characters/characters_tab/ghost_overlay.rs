//! Full-overlay ghost element that follows the cursor during a drag operation.

use iced::{Background, Element, Length, Padding, Point, widget::container};
use pod_model::Character;

use super::{Message, ghost_card};
use crate::style::{color, spacing};

/// Non-interactive drag ghost rendered above the character grid.
///
/// Mirrors the full card layout with no mouse-area elements so all pointer
/// events fall through to the grid cards and empty slots beneath, preserving
/// `SlotEntered` behaviour.
pub struct Component<'a> {
  character: &'a Character,
  cursor: Point,
  portrait_handle: Option<&'a iced::widget::image::Handle>,
  window_width: f32,
}

impl<'a> Component<'a> {
  /// Creates a new `GhostOverlay` for the given character, cursor position, and window width.
  pub fn new(
    character: &'a Character,
    portrait_handle: Option<&'a iced::widget::image::Handle>,
    cursor: Point,
    window_width: f32,
  ) -> Self {
    Self {
      character,
      cursor,
      portrait_handle,
      window_width,
    }
  }

  /// Renders the positioned ghost overlay into an element.
  pub fn render(self) -> Element<'a, Message> {
    use crate::views::characters::characters_tab::grid_cols;

    let cols = grid_cols(self.window_width);
    let effective_width = self.window_width.min(spacing::layout::GRID_MAX_WIDTH);
    let card_width = (effective_width - spacing::SPACE_8 * 2.0 - spacing::SPACE_4 * (cols - 1) as f32) / cols as f32;

    let ghost_left = (self.cursor.x - card_width / 2.0).max(0.0);
    let ghost_top = (self.cursor.y - spacing::layout::CHARACTER_CARD_HEIGHT * 0.3).max(0.0);

    let ghost = ghost_element(self.character, self.portrait_handle);

    container(container(ghost).width(card_width))
      .width(Length::Fill)
      .height(Length::Fill)
      .padding(Padding {
        top: ghost_top,
        left: ghost_left,
        ..Padding::ZERO
      })
      .into()
  }
}

fn ghost_element<'a>(
  character: &'a Character,
  portrait_handle: Option<&'a iced::widget::image::Handle>,
) -> Element<'a, Message> {
  use iced::widget::column;

  use crate::{components, views::characters::characters_tab::character_card};

  let portrait = character_card::CharacterPortrait::new(character)
    .portrait_handle(portrait_handle)
    .render::<Message>();

  let identity = container(character_card::CharacterDetail::new(character).render::<Message>())
    .padding(Padding {
      right: spacing::SPACE_4,
      ..Padding::ZERO
    })
    .into();

  let tags = ghost_tags(character);
  let training = character_card::CharacterSkillTraining::new(character).render::<Message>();
  let stats = ghost_stats(character);

  let card_content = column([
    portrait,
    identity,
    tags,
    components::Separator::horizontal().render(),
    training,
    components::Separator::horizontal().render(),
    stats,
  ])
  .width(Length::Fill)
  .height(Length::Fill);

  ghost_card::Component::new(card_content).render()
}

fn ghost_stats<'a>(character: &'a Character) -> Element<'a, Message> {
  use iced::widget::row;

  use crate::views::characters::characters_tab::character_card;

  let location = character.location_name().as_deref();
  let divider: Element<'a, Message> = container(iced::widget::Space::new().height(Length::Fill))
    .width(1.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into();

  row([
    character_card::CharacterLocation::new(location).render::<Message>(),
    divider,
    character_card::CharacterWallet::new(character).render::<Message>(),
  ])
  .into()
}

fn ghost_tags<'a>(character: &'a Character) -> Element<'a, Message> {
  use iced::widget::row;

  use crate::components;

  let tag_children: Vec<Element<'a, Message>> = character
    .tags()
    .iter()
    .map(|(_, name, _)| components::Badge::tag(name).render::<Message>())
    .collect();

  if tag_children.is_empty() {
    iced::widget::Space::new().width(Length::Shrink).height(0).into()
  } else {
    container(row(tag_children).spacing(spacing::SPACE_1).wrap())
      .padding(Padding {
        top: 0.0,
        bottom: 10.0,
        left: spacing::SPACE_4,
        right: spacing::SPACE_4,
      })
      .width(Length::Fill)
      .into()
  }
}
