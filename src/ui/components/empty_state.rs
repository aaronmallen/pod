use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Space, button, container, text},
};

use crate::ui::{
  components::icon::Icon,
  style::{color, radius, spacing, typography},
};

const CONTENT_SPACING: f32 = spacing::UNIT;
const ICON_SIZE: f32 = 28.0;
const SECTION_GAP: f32 = spacing::SPACE_6;

pub struct EmptyState<'a, M> {
  action: Option<(&'a str, M)>,
  icon: Option<Icon>,
  subtitle: Option<&'a str>,
  title: &'a str,
}

impl<'a, M: Clone + 'static> EmptyState<'a, M> {
  pub fn action(mut self, label: &'a str, on_press: M) -> Self {
    self.action = Some((label, on_press));
    self
  }

  #[allow(dead_code)] // component API surface; exercised only by unit tests
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn render(self) -> Element<'a, M> {
    let mut children: Vec<Element<'a, M>> = Vec::new();

    if let Some(icon) = self.icon {
      children.push(icon.color(color::text::tertiary()).size(ICON_SIZE).render());
      children.push(Space::new().height(Length::Fixed(CONTENT_SPACING)).into());
    }

    children.push(
      text(self.title.to_owned())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    );

    if let Some(subtitle) = self.subtitle {
      children.push(Space::new().height(Length::Fixed(CONTENT_SPACING)).into());
      children.push(
        text(subtitle.to_owned())
          .font(typography::body::REGULAR)
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          })
          .into(),
      );
    }

    if let Some((label, on_press)) = self.action {
      children.push(Space::new().height(Length::Fixed(SECTION_GAP)).into());
      children.push(action_button(label, on_press));
    }

    container(Column::with_children(children).align_x(Horizontal::Center))
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center)
      .padding(SECTION_GAP)
      .into()
  }

  pub fn subtitle(mut self, subtitle: &'a str) -> Self {
    self.subtitle = Some(subtitle);
    self
  }
}

pub enum LoadStateView<'a, M> {
  Empty(EmptyState<'a, M>),
  Error(&'a str),
  Loading(&'a str),
}

pub fn empty_state<M>(title: &str) -> EmptyState<'_, M> {
  EmptyState {
    action: None,
    icon: None,
    subtitle: None,
    title,
  }
}

pub fn load_state_view<M: Clone + 'static>(view: LoadStateView<'_, M>) -> Element<'_, M> {
  match view {
    LoadStateView::Empty(empty) => empty.render(),
    LoadStateView::Error(message) => placeholder(message, color::status::DANGER),
    LoadStateView::Loading(message) => placeholder(message, color::text::secondary()),
  }
}

fn action_button<'a, M: Clone + 'a>(label: &'a str, on_press: M) -> Element<'a, M> {
  button(
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 7.0,
    bottom: 7.0,
    left: spacing::SPACE_3,
    right: spacing::SPACE_3,
  })
  .on_press(on_press)
  .style(|_, status| {
    let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(if hover {
        color::with_alpha(color::accent::PLASMA, 0.12)
      } else {
        iced::Color::TRANSPARENT
      })),
      border: Border {
        color: color::accent::PLASMA_MUTED,
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  })
  .into()
}

fn placeholder<'a, M: 'a>(message: &'a str, text_color: iced::Color) -> Element<'a, M> {
  container(
    text(message.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(text_color),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(SECTION_GAP)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug, Eq, PartialEq)]
  enum Message {
    ClearFilters,
  }

  mod empty_state {
    use super::*;

    #[test]
    fn it_renders_a_bare_title() {
      let _el: Element<'_, Message> = super::super::empty_state("No assets").render();
    }

    #[test]
    fn it_renders_with_a_subtitle_but_no_icon_or_action() {
      let _el: Element<'_, Message> = super::super::empty_state("Nothing here")
        .subtitle("Sync to populate this view.")
        .render();
    }

    #[test]
    fn it_renders_with_an_icon_subtitle_and_action() {
      let _el: Element<'_, Message> = super::super::empty_state("No matches")
        .icon(Icon::filter())
        .subtitle("Try widening your filters.")
        .action("Clear filters", Message::ClearFilters)
        .render();
    }
  }

  mod load_state_view {
    use super::*;

    #[test]
    fn it_renders_the_error_branch() {
      let _el: Element<'_, Message> = load_state_view(LoadStateView::Error("Couldn\u{2019}t load"));
    }

    #[test]
    fn it_renders_the_loaded_empty_branch() {
      let _el: Element<'_, Message> = load_state_view(LoadStateView::Empty(super::super::empty_state("No rows")));
    }

    #[test]
    fn it_renders_the_loading_branch() {
      let _el: Element<'_, Message> = load_state_view(LoadStateView::Loading("Loading\u{2026}"));
    }
  }
}
