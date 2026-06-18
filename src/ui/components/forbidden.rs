use std::collections::HashSet;

use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use crate::ui::{
  components::icon::Icon,
  style::{color, radius, spacing, typography},
};

const BODY_MAX_WIDTH: f32 = 420.0;
const CHIP_DOT: f32 = 5.0;
const SECTION_GAP: f32 = spacing::SPACE_6;
const TILE_ICON: f32 = 22.0;
const TILE_SIZE: f32 = 52.0;

pub fn forbidden<'a, M: Clone + 'static>(
  noun: &'a str,
  character_name: &str,
  missing: &[&str],
  on_reauth: M,
) -> Element<'a, M> {
  let lowered = noun.to_lowercase();

  let tile = container(Icon::lock().color(color::status::WARNING).size(TILE_ICON).render::<M>())
    .width(Length::Fixed(TILE_SIZE))
    .height(Length::Fixed(TILE_SIZE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.1))),
      border: Border {
        color: color::with_alpha(color::status::WARNING, 0.35),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let eyebrow = text("ESI 403 \u{00B7} FORBIDDEN")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::status::WARNING),
    });

  let heading = text(format!("{noun} access not authorized"))
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(|_| text::Style {
      color: Some(color::text::PRIMARY),
    });

  let copy = forbidden_copy(character_name, &lowered);
  let body = container(
    text(copy)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .align_x(Horizontal::Center)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .max_width(BODY_MAX_WIDTH);

  let mut children: Vec<Element<'a, M>> = vec![
    tile.into(),
    Space::new().height(Length::Fixed(spacing::SPACE_3)).into(),
    eyebrow.into(),
    Space::new().height(Length::Fixed(spacing::SPACE_2)).into(),
    heading.into(),
    Space::new().height(Length::Fixed(spacing::SPACE_2)).into(),
    body.into(),
  ];

  if !missing.is_empty() {
    children.push(Space::new().height(Length::Fixed(SECTION_GAP)).into());
    children.push(scopes_section(missing));
  }

  children.push(Space::new().height(Length::Fixed(SECTION_GAP)).into());
  children.push(reauth_button(character_name, on_reauth));
  children.push(Space::new().height(Length::Fixed(spacing::SPACE_3)).into());
  children.push(
    text("OPENS EVE ONLINE SSO \u{00B7} LOGIN.EVEONLINE.COM")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  );

  container(
    Column::with_children(children)
      .align_x(Horizontal::Center)
      .width(Length::Shrink),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(SECTION_GAP)
  .into()
}

#[allow(dead_code)] // helper exercised only by unit tests
pub fn is_scope_missing(granted: Option<&str>, required: &[&str]) -> bool {
  !missing_scopes(granted, required).is_empty()
}

pub fn missing_scopes<'a>(granted: Option<&str>, required: &[&'a str]) -> Vec<&'a str> {
  let granted: HashSet<&str> = granted.unwrap_or_default().split_whitespace().collect();
  required
    .iter()
    .copied()
    .filter(|scope| !granted.contains(scope))
    .collect()
}

fn forbidden_copy(character_name: &str, lowered: &str) -> String {
  let lead = format!("Pod doesn\u{2019}t have permission to read {character_name}\u{2019}s {lowered}.");
  let context = format!("This character was authorized before the {lowered} feature existed.");
  format!("{lead} {context} Re-authenticate to grant the missing scope.")
}

fn reauth_button<'a, M: Clone + 'static>(character_name: &str, on_reauth: M) -> Element<'a, M> {
  let label = Row::with_children(vec![
    Icon::characters().color(color::surface::BASE).size(13.0).render::<M>(),
    text(format!("Re-authenticate {character_name}"))
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::surface::BASE),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  button(label)
    .padding(Padding {
      top: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_6,
      right: spacing::SPACE_6,
    })
    .on_press(on_reauth)
    .style(|_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(if hover {
          color::with_alpha(color::accent::PLASMA, 0.85)
        } else {
          color::accent::PLASMA
        })),
        border: Border {
          radius: radius::CONTROL.into(),
          ..Border::default()
        },
        text_color: color::surface::BASE,
        ..button::Style::default()
      }
    })
    .into()
}

fn scope_chip<'a, M: 'a>(scope: &str) -> Element<'a, M> {
  let dot = container(Space::new())
    .width(Length::Fixed(CHIP_DOT))
    .height(Length::Fixed(CHIP_DOT))
    .style(|_| container::Style {
      background: Some(Background::Color(color::status::WARNING)),
      border: Border {
        radius: (CHIP_DOT / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let label = text(scope.to_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  container(
    Row::with_children(vec![dot.into(), label.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: spacing::UNIT,
    bottom: spacing::UNIT,
    left: spacing::SPACE_2,
    right: spacing::SPACE_2,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn scopes_section<'a, M: 'a>(missing: &[&str]) -> Element<'a, M> {
  let eyebrow = text("SCOPES REQUESTED")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });

  let chips: Vec<Element<'a, M>> = missing.iter().map(|scope| scope_chip(scope)).collect();

  Column::with_children(vec![
    eyebrow.into(),
    Space::new().height(Length::Fixed(spacing::SPACE_2)).into(),
    Column::with_children(chips)
      .spacing(spacing::SPACE_2)
      .align_x(Horizontal::Center)
      .into(),
  ])
  .align_x(Horizontal::Center)
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::clients::esi::scopes;

  mod forbidden {
    use super::*;

    #[test]
    fn it_renders_with_missing_scope_chips() {
      let _el: Element<'_, ()> = super::super::forbidden(
        "Mail",
        "Ash Korr",
        &[scopes::CHARACTER_MAIL, scopes::CHARACTER_MAIL_SEND],
        (),
      );
    }

    #[test]
    fn it_renders_without_any_scope_chips() {
      let _el: Element<'_, ()> = super::super::forbidden("Wallet", "Mara Quill", &[], ());
    }
  }

  mod is_scope_missing {
    use super::*;

    #[test]
    fn it_is_false_when_nothing_is_required() {
      assert!(!is_scope_missing(None, &[]));
      assert!(!is_scope_missing(Some(scopes::CHARACTER_CLONES), &[]));
    }

    #[test]
    fn it_is_false_when_the_grant_is_a_superset() {
      let granted = format!("{} {}", scopes::CHARACTER_CLONES, scopes::CHARACTER_STANDINGS);

      assert!(!is_scope_missing(Some(&granted), &[scopes::CHARACTER_CLONES]));
    }

    #[test]
    fn it_is_true_when_a_required_scope_is_not_granted() {
      assert!(is_scope_missing(
        Some(scopes::CHARACTER_CLONES),
        &[scopes::CHARACTER_STANDINGS]
      ));
      assert!(is_scope_missing(None, &[scopes::CHARACTER_KILLMAILS]));
      assert!(is_scope_missing(Some("   "), &[scopes::CHARACTER_KILLMAILS]));
    }
  }

  mod missing_scopes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_empty_when_the_grant_covers_every_required_scope() {
      let granted = format!("{} {}", scopes::CHARACTER_CLONES, scopes::CHARACTER_STANDINGS);

      assert!(missing_scopes(Some(&granted), &[scopes::CHARACTER_CLONES]).is_empty());
    }

    #[test]
    fn it_lists_every_required_scope_absent_from_the_grant() {
      let granted = scopes::CHARACTER_CLONES;
      let required = [scopes::CHARACTER_CLONES, scopes::CHARACTER_STANDINGS];

      assert_eq!(
        missing_scopes(Some(granted), &required),
        vec![scopes::CHARACTER_STANDINGS]
      );
    }

    #[test]
    fn it_reports_all_required_scopes_when_the_grant_is_absent() {
      let required = [scopes::CHARACTER_CONTACTS];

      assert_eq!(missing_scopes(None, &required), vec![scopes::CHARACTER_CONTACTS]);
    }
  }
}
