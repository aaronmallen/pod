use std::path::PathBuf;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, button, container, scrollable, svg, text},
};

use crate::ui::{
  components::{avatar::avatar, icon::Icon},
  style::{color, radius, spacing, typography},
};

static LOCK_ICON: &[u8] = include_bytes!("../../../assets/images/icons/lock.svg");

const BADGE_ICON_SIZE: f32 = 20.0;
const BADGE_SIZE: f32 = 38.0;
const CARET_SIZE: f32 = 14.0;
const DROPDOWN_WIDTH: f32 = 360.0;
const HEADER_PAD_X: f32 = 14.0;
const HEADER_PAD_Y: f32 = 10.0;
const LIST_MAX_HEIGHT: f32 = 420.0;
const LOCK_SIZE: f32 = 15.0;
const ROW_PORTRAIT: f32 = 30.0;
const SCROLLBAR_WIDTH: f32 = 6.0;
const TRIGGER_PAD: f32 = 6.0;
const TRIGGER_PORTRAIT: f32 = 38.0;

pub struct PickerGroup<'a, M> {
  pub items: Vec<Element<'a, M>>,
  pub title: Option<String>,
}

pub struct TriggerPortrait {
  pub id: i64,
  pub name: String,
  pub path: Option<PathBuf>,
}

#[allow(clippy::too_many_arguments)] // cohesive row-builder params; a params struct would only relocate the same fields
pub fn picker_character_row<'a, M: 'a + Clone>(
  id: i64,
  name: impl Into<String>,
  sub: impl Into<String>,
  portrait: Option<PathBuf>,
  trailing: Option<Element<'a, M>>,
  selected: bool,
  reauth: Option<&'a str>,
  on_press: M,
) -> Element<'a, M> {
  let name = name.into();

  let portrait_cell = container(avatar(id, &name, Length::Fixed(ROW_PORTRAIT), ROW_PORTRAIT, portrait))
    .width(Length::Fixed(ROW_PORTRAIT))
    .height(Length::Fixed(ROW_PORTRAIT))
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        radius: radius::SUBTLE.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let (subtitle, subtitle_color) = match reauth {
    Some(noun) => (format!("{noun} not authorized"), color::status::WARNING),
    None => (sub.into(), color::text::secondary()),
  };

  let identity = Column::with_children(vec![
    text(name)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(subtitle)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(subtitle_color),
      })
      .into(),
  ])
  .spacing(2.0)
  .width(Length::Fill);

  let mut cells: Vec<Element<'a, M>> = vec![portrait_cell.into(), identity.into()];
  match reauth {
    Some(_) => cells.push(
      container(
        svg(svg::Handle::from_memory(LOCK_ICON))
          .width(Length::Fixed(LOCK_SIZE))
          .height(Length::Fixed(LOCK_SIZE))
          .style(|_, _| svg::Style {
            color: Some(color::status::WARNING),
          }),
      )
      .align_y(Vertical::Center)
      .into(),
    ),
    None => {
      if let Some(trailing) = trailing {
        cells.push(trailing);
      }
    }
  }

  let inner = Row::with_children(cells)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  button(inner)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: HEADER_PAD_X,
      bottom: spacing::SPACE_2_5,
      left: HEADER_PAD_X,
    })
    .on_press(on_press)
    .style(move |_, status| picker_row_style(selected, status))
    .into()
}

pub fn picker_dropdown<'a, M: 'a + Clone>(groups: Vec<PickerGroup<'a, M>>) -> Element<'a, M> {
  let mut rows: Vec<Element<'a, M>> = Vec::new();
  for group in groups {
    if let Some(title) = group.title {
      rows.push(section_header(title));
    }
    rows.extend(group.items);
  }

  let list = scrollable(Column::with_children(rows).width(Length::Fill))
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Shrink)
    .direction(scrollable::Direction::Vertical(
      scrollable::Scrollbar::new()
        .width(SCROLLBAR_WIDTH)
        .scroller_width(SCROLLBAR_WIDTH),
    ));

  container(container(list).max_height(LIST_MAX_HEIGHT))
    .width(Length::Fixed(DROPDOWN_WIDTH))
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: 10.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

pub fn picker_row<'a, M: 'a + Clone>(label: impl Into<String>, selected: bool, on_press: M) -> Element<'a, M> {
  let label = text(label.into())
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  button(label)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      right: HEADER_PAD_X,
      bottom: spacing::SPACE_2_5,
      left: HEADER_PAD_X,
    })
    .on_press(on_press)
    .style(move |_, status| picker_row_style(selected, status))
    .into()
}

pub fn picker_trigger<'a, M: 'a + Clone>(content: Element<'a, M>, open: bool, on_toggle: M) -> Element<'a, M> {
  button(content)
    .padding(TRIGGER_PAD)
    .on_press(on_toggle)
    .style(move |_, status| trigger_style(open, status))
    .into()
}

pub fn trigger_badge_identity<'a, M: 'static>(
  icon: Icon,
  title: impl Into<String>,
  subtitle: impl Into<String>,
) -> Element<'a, M> {
  let badge = container(icon.color(color::accent()).size(BADGE_ICON_SIZE).render::<M>())
    .width(Length::Fixed(BADGE_SIZE))
    .height(Length::Fixed(BADGE_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent(), 0.1))),
      border: Border {
        color: color::with_alpha(color::accent(), 0.3),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  Row::with_children(vec![badge.into(), trigger_identity(title, subtitle, None)])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .into()
}

pub fn trigger_identity<'a, M: 'static>(
  title: impl Into<String>,
  subtitle: impl Into<String>,
  portrait: Option<TriggerPortrait>,
) -> Element<'a, M> {
  let identity = Column::with_children(vec![
    text(title.into())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(subtitle.into().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::UNIT - 1.0);

  let caret = Icon::chevron_down()
    .color(color::text::secondary())
    .size(CARET_SIZE)
    .render::<M>();

  let mut cells: Vec<Element<'a, M>> = Vec::with_capacity(3);
  if let Some(portrait) = portrait {
    let tile = container(avatar(
      portrait.id,
      &portrait.name,
      Length::Fixed(TRIGGER_PORTRAIT),
      TRIGGER_PORTRAIT,
      portrait.path,
    ))
    .width(Length::Fixed(TRIGGER_PORTRAIT))
    .height(Length::Fixed(TRIGGER_PORTRAIT))
    .clip(true)
    .style(|_| container::Style {
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
    cells.push(tile.into());
  }
  cells.push(identity.into());
  cells.push(caret);

  Row::with_children(cells)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .into()
}

fn picker_row_style(selected: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let background = if selected {
    Some(color::with_alpha(color::accent(), 0.12))
  } else if hovered {
    Some(color::with_alpha(color::text::PRIMARY, 0.06))
  } else {
    None
  };

  button::Style {
    background: background.map(Background::Color),
    text_color: color::text::PRIMARY,
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

fn section_header<'a, M: 'a>(title: String) -> Element<'a, M> {
  let label = text(title.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  container(label)
    .width(Length::Fill)
    .padding(Padding {
      top: HEADER_PAD_Y,
      right: HEADER_PAD_X,
      bottom: HEADER_PAD_Y,
      left: HEADER_PAD_X,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.1),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn trigger_style(open: bool, status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  let background = if open {
    Some(color::with_alpha(color::text::PRIMARY, 0.06))
  } else if hovered {
    Some(color::with_alpha(color::text::PRIMARY, 0.04))
  } else {
    None
  };

  button::Style {
    background: background.map(Background::Color),
    border: Border {
      color: if open {
        color::rule_strong()
      } else {
        iced::Color::TRANSPARENT
      },
      width: 1.0,
      radius: radius::CARD.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Debug, Eq, PartialEq)]
  enum Msg {
    Selected(i64),
    Toggle,
  }

  fn trigger_content() -> Element<'static, Msg> {
    text("Cinder Vex").into()
  }

  fn groups() -> Vec<PickerGroup<'static, Msg>> {
    vec![
      PickerGroup {
        title: Some("Characters".to_string()),
        items: vec![
          picker_row("Cinder Vex", true, Msg::Selected(1)),
          picker_row("Mara Quill", false, Msg::Selected(2)),
        ],
      },
      PickerGroup {
        title: Some("Corporations".to_string()),
        items: vec![
          picker_row("Pale Horse", false, Msg::Selected(3)),
          picker_row("Brave Newbies", false, Msg::Selected(4)),
        ],
      },
    ]
  }

  mod picker_character_row {
    use std::path::PathBuf;

    use super::{super::picker_character_row, *};

    #[test]
    fn it_builds_a_row_with_a_portrait() {
      let portrait = Some(PathBuf::from("/tmp/p.png"));

      let _el: Element<'_, Msg> =
        picker_character_row(1, "Cinder Vex", "PALE", portrait, None, true, None, Msg::Selected(1));
    }

    #[test]
    fn it_builds_a_row_with_a_trailing_column() {
      let trailing: Element<'_, Msg> = text("47.3M SP").into();

      let _el: Element<'_, Msg> = picker_character_row(
        3,
        "Vala Rook",
        "BRAVE",
        None,
        Some(trailing),
        false,
        None,
        Msg::Selected(3),
      );
    }

    #[test]
    fn it_builds_a_row_without_a_portrait() {
      let _el: Element<'_, Msg> =
        picker_character_row(2, "Mara Quill", "BRAVE", None, None, false, None, Msg::Selected(2));
    }

    #[test]
    fn it_flags_a_missing_scope_pilot_with_the_reauth_label() {
      let _el: Element<'_, Msg> = picker_character_row(
        4,
        "Sable Renn",
        "PALE",
        None,
        None,
        false,
        Some("Mail"),
        Msg::Selected(4),
      );
    }
  }

  mod picker_dropdown {
    use super::*;

    #[test]
    fn it_builds_a_dropdown_from_titled_groups() {
      let _el: Element<'_, Msg> = picker_dropdown(groups());
    }

    #[test]
    fn it_builds_an_untitled_single_group() {
      let group = vec![PickerGroup {
        title: None,
        items: vec![picker_row("All Characters", true, Msg::Selected(0))],
      }];
      let _el: Element<'_, Msg> = picker_dropdown(group);
    }
  }

  mod picker_trigger {
    use super::*;

    #[test]
    fn it_builds_a_closed_trigger() {
      let _el: Element<'_, Msg> = picker_trigger(trigger_content(), false, Msg::Toggle);
    }

    #[test]
    fn it_builds_an_open_trigger() {
      let _el: Element<'_, Msg> = picker_trigger(trigger_content(), true, Msg::Toggle);
    }
  }

  mod trigger_badge_identity {
    use super::{super::trigger_badge_identity, *};

    #[test]
    fn it_builds_a_badge_identity() {
      let _el: Element<'_, Msg> = trigger_badge_identity(Icon::industry(), "All Industry", "3 pilots combined");
    }
  }

  mod trigger_identity {
    use std::path::PathBuf;

    use super::{super::trigger_identity, *};

    #[test]
    fn it_builds_an_identity_with_a_portrait() {
      let portrait = Some(TriggerPortrait {
        id: 1,
        name: "Cinder Vex".to_owned(),
        path: Some(PathBuf::from("/tmp/p.png")),
      });

      let _el: Element<'_, Msg> = trigger_identity("Cinder Vex", "Pale Horse", portrait);
    }

    #[test]
    fn it_builds_an_identity_without_a_portrait() {
      let _el: Element<'_, Msg> = trigger_identity("All Inboxes", "12 unread", None);
    }
  }
}
