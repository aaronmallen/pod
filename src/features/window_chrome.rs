use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, mouse_area, text},
  window::Direction,
};

use crate::ui::style::{color, radius, spacing, typography};

// The shell, its chrome events, and these dimensions ship ahead of their first caller: the Killmail
// pilot and the Contract/Stockpile/Mail child windows, none of which exist yet.
#[allow(dead_code)]
pub const RESIZE_EDGE: f32 = 6.0;
#[allow(dead_code)]
pub const TITLE_BAR_HEIGHT: f32 = 40.0;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Event {
  Close,
  Drag,
  Resize(Direction),
}

pub fn panel_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      radius: radius::PANEL.into(),
      ..Border::default()
    },
    ..container::Style::default()
  }
}

#[allow(dead_code)]
pub fn shell<'a, M, F>(title: &str, content: Element<'a, M>, on_event: F) -> Element<'a, M>
where
  M: Clone + 'a,
  F: Fn(Event) -> M + Copy + 'a,
{
  let panel = container(
    Column::with_children(vec![title_bar(title, on_event), content])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(panel_style);

  framed(panel.into(), on_event)
}

#[allow(dead_code)]
fn framed<'a, M, F>(panel: Element<'a, M>, on_event: F) -> Element<'a, M>
where
  M: Clone + 'a,
  F: Fn(Event) -> M + Copy + 'a,
{
  let top = Row::with_children(vec![
    corner(Direction::NorthWest, on_event),
    edge(Direction::North, Length::Fill, RESIZE_EDGE, on_event),
    corner(Direction::NorthEast, on_event),
  ])
  .into();

  let middle = Row::with_children(vec![
    edge(Direction::West, RESIZE_EDGE, Length::Fill, on_event),
    container(panel).width(Length::Fill).height(Length::Fill).into(),
    edge(Direction::East, RESIZE_EDGE, Length::Fill, on_event),
  ])
  .height(Length::Fill)
  .into();

  let bottom = Row::with_children(vec![
    corner(Direction::SouthWest, on_event),
    edge(Direction::South, Length::Fill, RESIZE_EDGE, on_event),
    corner(Direction::SouthEast, on_event),
  ])
  .into();

  Column::with_children(vec![top, middle, bottom])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[allow(dead_code)]
fn corner<'a, M, F>(direction: Direction, on_event: F) -> Element<'a, M>
where
  M: Clone + 'a,
  F: Fn(Event) -> M + 'a,
{
  resize_zone(
    direction,
    Length::Fixed(RESIZE_EDGE),
    Length::Fixed(RESIZE_EDGE),
    on_event,
  )
}

#[allow(dead_code)]
fn edge<'a, M, F>(
  direction: Direction,
  width: impl Into<Length>,
  height: impl Into<Length>,
  on_event: F,
) -> Element<'a, M>
where
  M: Clone + 'a,
  F: Fn(Event) -> M + 'a,
{
  resize_zone(direction, width.into(), height.into(), on_event)
}

#[allow(dead_code)]
fn resize_zone<'a, M, F>(direction: Direction, width: Length, height: Length, on_event: F) -> Element<'a, M>
where
  M: Clone + 'a,
  F: Fn(Event) -> M + 'a,
{
  mouse_area(container(Space::new()).width(width).height(height))
    .interaction(resize_cursor(direction))
    .on_press(on_event(Event::Resize(direction)))
    .into()
}

#[allow(dead_code)]
fn resize_cursor(direction: Direction) -> iced::mouse::Interaction {
  match direction {
    Direction::East | Direction::West => iced::mouse::Interaction::ResizingHorizontally,
    Direction::North | Direction::South => iced::mouse::Interaction::ResizingVertically,
    Direction::NorthEast | Direction::SouthWest => iced::mouse::Interaction::ResizingDiagonallyUp,
    Direction::NorthWest | Direction::SouthEast => iced::mouse::Interaction::ResizingDiagonallyDown,
  }
}

#[allow(dead_code)]
fn title_bar<'a, M, F>(title: &str, on_event: F) -> Element<'a, M>
where
  M: Clone + 'a,
  F: Fn(Event) -> M + Copy + 'a,
{
  let label = text(title.to_owned())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));

  let bar = Row::with_children(vec![
    label.into(),
    Space::new().width(Length::Fill).into(),
    close_button(on_event),
  ])
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 0.0,
    right: spacing::SPACE_2,
    bottom: 0.0,
    left: spacing::SPACE_3,
  });

  mouse_area(
    container(bar)
      .width(Length::Fill)
      .height(Length::Fixed(TITLE_BAR_HEIGHT))
      .align_y(Vertical::Center)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::NAVIGATION)),
        border: Border {
          radius: iced::border::top(radius::PANEL),
          ..Border::default()
        },
        ..container::Style::default()
      }),
  )
  .interaction(iced::mouse::Interaction::Grab)
  .on_press(on_event(Event::Drag))
  .into()
}

#[allow(dead_code)]
fn close_button<'a, M, F>(on_event: F) -> Element<'a, M>
where
  M: Clone + 'a,
  F: Fn(Event) -> M + 'a,
{
  button(
    text("\u{2715}")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(Padding {
    top: spacing::SPACE_2 / 2.0,
    right: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2 / 2.0,
    left: spacing::SPACE_2_5,
  })
  .on_press(on_event(Event::Close))
  .style(|_, status| {
    let background = match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::status::DANGER)),
      _ => None,
    };
    button::Style {
      background,
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..button::Style::default()
    }
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod resize_cursor {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_horizontal_edges_to_a_horizontal_resize_cursor() {
      assert_eq!(
        resize_cursor(Direction::East),
        iced::mouse::Interaction::ResizingHorizontally
      );
      assert_eq!(
        resize_cursor(Direction::West),
        iced::mouse::Interaction::ResizingHorizontally
      );
    }

    #[test]
    fn it_maps_the_lead_diagonal_corners_to_a_down_diagonal_cursor() {
      assert_eq!(
        resize_cursor(Direction::NorthWest),
        iced::mouse::Interaction::ResizingDiagonallyDown
      );
      assert_eq!(
        resize_cursor(Direction::SouthEast),
        iced::mouse::Interaction::ResizingDiagonallyDown
      );
    }

    #[test]
    fn it_maps_the_off_diagonal_corners_to_an_up_diagonal_cursor() {
      assert_eq!(
        resize_cursor(Direction::NorthEast),
        iced::mouse::Interaction::ResizingDiagonallyUp
      );
      assert_eq!(
        resize_cursor(Direction::SouthWest),
        iced::mouse::Interaction::ResizingDiagonallyUp
      );
    }

    #[test]
    fn it_maps_vertical_edges_to_a_vertical_resize_cursor() {
      assert_eq!(
        resize_cursor(Direction::North),
        iced::mouse::Interaction::ResizingVertically
      );
      assert_eq!(
        resize_cursor(Direction::South),
        iced::mouse::Interaction::ResizingVertically
      );
    }
  }

  mod shell {
    use super::*;

    #[test]
    fn it_wraps_arbitrary_content_in_the_chrome() {
      let content: Element<'_, ()> = text("body").into();

      let _shell: Element<'_, ()> = shell("Killmail", content, |_| ());
    }
  }
}
