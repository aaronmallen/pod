use iced::{
  Background, Border, Element, Length, Padding, Point,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use crate::ui::style::{color, radius, shadow, typography};

pub const MENU_WIDTH: f32 = 220.0;
const DIVIDER_INSET: f32 = 6.0;
const DIVIDER_PAD_Y: f32 = 4.0;
const GLYPH_GAP: f32 = 10.0;
const GLYPH_WIDTH: f32 = 16.0;
const PANEL_PAD: f32 = 4.0;
const ROW_PAD_X: f32 = 10.0;
const ROW_PAD_Y: f32 = 7.0;
const TITLE_PAD_BOTTOM: f32 = 6.0;
const TITLE_PAD_TOP: f32 = 8.0;
const TITLE_PAD_X: f32 = 10.0;

pub enum Item<MSG> {
  Row {
    label: String,
    glyph: Option<String>,
    on_press: Option<MSG>,
    tone: Tone,
  },
  Separator,
}

impl<MSG> Item<MSG> {
  pub fn action(label: impl Into<String>, on_press: MSG) -> Self {
    Self::Row {
      label: label.into(),
      glyph: None,
      on_press: Some(on_press),
      tone: Tone::Default,
    }
  }

  pub fn danger(label: impl Into<String>, on_press: MSG) -> Self {
    Self::Row {
      label: label.into(),
      glyph: None,
      on_press: Some(on_press),
      tone: Tone::Danger,
    }
  }

  pub fn disabled(label: impl Into<String>) -> Self {
    Self::Row {
      label: label.into(),
      glyph: None,
      on_press: None,
      tone: Tone::Default,
    }
  }

  pub fn separator() -> Self {
    Self::Separator
  }

  pub fn warning(label: impl Into<String>, on_press: MSG) -> Self {
    Self::Row {
      label: label.into(),
      glyph: None,
      on_press: Some(on_press),
      tone: Tone::Warning,
    }
  }

  pub fn with_glyph(mut self, glyph: impl Into<String>) -> Self {
    if let Self::Row {
      glyph: slot, ..
    } = &mut self
    {
      *slot = Some(glyph.into());
    }
    self
  }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Tone {
  Danger,
  Default,
  Warning,
}

impl Tone {
  fn accent(self) -> iced::Color {
    match self {
      Tone::Danger => color::status::DANGER,
      Tone::Default => color::text::PRIMARY,
      Tone::Warning => color::status::WARNING,
    }
  }
}

pub fn context_menu<'a, MSG>(title: &str, items: Vec<Item<MSG>>, cursor: Point) -> Element<'a, MSG>
where
  MSG: Clone + 'a,
{
  let mut rows: Vec<Element<'a, MSG>> = vec![menu_title(title)];
  for item in items {
    rows.push(match item {
      Item::Separator => divider(),
      Item::Row {
        label,
        glyph,
        on_press,
        tone,
      } => menu_row(tone, label, glyph, on_press),
    });
  }

  let panel = container(Column::with_children(rows).width(Length::Fill))
    .width(Length::Fixed(MENU_WIDTH))
    .padding(PANEL_PAD)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    });

  let top = cursor.y.max(0.0);
  let left = cursor.x.max(0.0);
  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
      top,
      left,
      ..Padding::ZERO
    })
    .into()
}

fn divider<'a, MSG>() -> Element<'a, MSG>
where
  MSG: 'a,
{
  let line = container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.1))),
      ..container::Style::default()
    });

  container(line)
    .width(Length::Fill)
    .padding(Padding {
      top: DIVIDER_PAD_Y,
      right: DIVIDER_INSET,
      bottom: DIVIDER_PAD_Y,
      left: DIVIDER_INSET,
    })
    .into()
}

fn menu_row<'a, MSG>(tone: Tone, label: String, glyph: Option<String>, on_press: Option<MSG>) -> Element<'a, MSG>
where
  MSG: Clone + 'a,
{
  let label_color = if on_press.is_none() {
    color::text::tertiary()
  } else {
    tone.accent()
  };

  let label = text(label)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(move |_| text::Style {
      color: Some(label_color),
    });

  let content: Element<'a, MSG> = match glyph {
    Some(glyph) => {
      let glyph_color = match tone {
        Tone::Danger => color::status::DANGER,
        Tone::Default | Tone::Warning => color::accent(),
      };
      let glyph = container(
        text(glyph)
          .font(typography::mono::REGULAR)
          .size(typography::size::MD)
          .style(move |_| text::Style {
            color: Some(glyph_color),
          }),
      )
      .width(Length::Fixed(GLYPH_WIDTH))
      .align_x(Horizontal::Center);
      Row::with_children(vec![glyph.into(), label.into()])
        .spacing(GLYPH_GAP)
        .align_y(Vertical::Center)
        .into()
    }
    None => label.into(),
  };

  let mut row = button(content).width(Length::Fill).padding(Padding {
    top: ROW_PAD_Y,
    right: ROW_PAD_X,
    bottom: ROW_PAD_Y,
    left: ROW_PAD_X,
  });
  if let Some(message) = on_press {
    row = row.on_press(message);
  }
  row.style(move |_, status| row_style(tone, status)).into()
}

fn menu_title<'a, MSG>(title: &str) -> Element<'a, MSG>
where
  MSG: 'a,
{
  container(
    text(title.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: TITLE_PAD_TOP,
    right: TITLE_PAD_X,
    bottom: TITLE_PAD_BOTTOM,
    left: TITLE_PAD_X,
  })
  .into()
}

fn row_style(tone: Tone, status: button::Status) -> button::Style {
  let background = match status {
    button::Status::Hovered | button::Status::Pressed => {
      Some(Background::Color(color::with_alpha(tone.accent(), 0.12)))
    }
    _ => None,
  };
  button::Style {
    background,
    text_color: tone.accent(),
    border: Border {
      radius: radius::SUBTLE.into(),
      ..Border::default()
    },
    ..button::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod context_menu {
    use iced::advanced::widget::Tree;
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Msg {
      Copy,
      Edit,
      Remove,
    }

    #[test]
    fn it_renders_a_title_actions_an_explicit_separator_and_a_danger_row() {
      let items = vec![
        Item::action("Copy name", Msg::Copy),
        Item::action("Edit tags", Msg::Edit),
        Item::separator(),
        Item::danger("Remove from app", Msg::Remove),
      ];

      let menu: Element<'_, Msg> = context_menu("Test Pilot", items, Point::new(40.0, 60.0));
      let tree = Tree::new(menu.as_widget());

      assert_eq!(tree.children.len(), 5);
    }

    #[test]
    fn it_renders_disabled_and_multi_separator_menus() {
      let items = vec![
        Item::action("Edit squad", Msg::Edit),
        Item::action("Collapse", Msg::Copy),
        Item::separator(),
        Item::disabled("Move pilots to Unassigned"),
        Item::separator(),
        Item::danger("Delete squad", Msg::Remove),
      ];

      let menu: Element<'_, Msg> = context_menu("Supers", items, Point::new(40.0, 60.0));
      let tree = Tree::new(menu.as_widget());

      assert_eq!(tree.children.len(), 7);
    }
  }

  mod item {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn with_glyph_attaches_to_a_row_and_noops_on_a_separator() {
      let row = Item::<()>::action("Add milestone above", ()).with_glyph("\u{2191}");
      match row {
        Item::Row {
          glyph, ..
        } => assert_eq!(glyph.as_deref(), Some("\u{2191}")),
        Item::Separator => panic!("expected a row"),
      }
      assert!(matches!(Item::<()>::separator().with_glyph("x"), Item::Separator));
    }
  }

  mod row_style {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn a_danger_row_uses_the_danger_text_color() {
      let danger = row_style(Tone::Danger, button::Status::Active);
      let normal = row_style(Tone::Default, button::Status::Active);

      assert_eq!(danger.text_color, color::status::DANGER);
      assert_eq!(normal.text_color, color::text::PRIMARY);
    }

    #[test]
    fn a_hovered_row_washes_its_background_and_an_idle_row_has_none() {
      let hovered = row_style(Tone::Default, button::Status::Hovered);
      let pressed = row_style(Tone::Default, button::Status::Pressed);
      let idle = row_style(Tone::Default, button::Status::Active);

      assert!(hovered.background.is_some());
      assert!(pressed.background.is_some());
      assert!(idle.background.is_none());
    }

    #[test]
    fn a_warning_row_uses_the_warning_text_color() {
      let warning = row_style(Tone::Warning, button::Status::Active);

      assert_eq!(warning.text_color, color::status::WARNING);
    }
  }
}
