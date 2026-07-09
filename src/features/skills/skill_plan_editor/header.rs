use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text, text_input, tooltip},
};

use super::Message;
use crate::ui::{
  components::{
    badge::badge,
    button::{Button, Size},
    icon::Icon,
    rule, status,
  },
  style::{color, radius, spacing, typography},
};

const BACK_GAP: f32 = 4.0;

const HEADER_HEIGHT: f32 = 52.0;

const HEADER_PAD_X: f32 = 16.0;

const NAME_INPUT_WIDTH: f32 = 420.0;

pub(super) fn header<'a>(
  plan_name: &'a str,
  dirty: bool,
  picker_open: bool,
  is_template: bool,
  is_manual: bool,
  last_entry_id: Option<i64>,
) -> Element<'a, Message> {
  let picker_label = if picker_open {
    t!("skills.editor_header.hide_picker")
  } else {
    t!("skills.editor_header.add_skills")
  };

  let mut children: Vec<Element<'a, Message>> = vec![close_btn(), Space::new().width(BACK_GAP).into()];
  if is_template {
    children.push(template_badge());
  }
  children.push(name_input(plan_name));
  if dirty {
    children.push(dirty_dot());
  }
  children.extend([
    Space::new().width(Length::Fill).into(),
    inert_trigger(t!("skills.editor_header.import").into_owned(), Message::ImportRequested),
    inert_trigger(t!("skills.editor_header.export").into_owned(), Message::ExportRequested),
    milestone_btn(is_manual, last_entry_id),
    secondary_btn(picker_label.into_owned(), Message::PickerToggled),
    save_btn(dirty),
  ]);

  let header_row = row(children)
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: HEADER_PAD_X,
      right: HEADER_PAD_X,
    });

  container(column(vec![
    container(header_row)
      .height(HEADER_HEIGHT)
      .width(Length::Fill)
      .align_y(Vertical::Center)
      .into(),
    rule::horizontal(),
  ]))
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    ..container::Style::default()
  })
  .into()
}

fn close_btn<'a>() -> Element<'a, Message> {
  Button::ghost("\u{2190}")
    .size(Size::Sm)
    .on_press(Message::CloseRequested)
    .into()
}

fn dirty_dot<'a>() -> Element<'a, Message> {
  status::dot_sized(color::status::WARNING, 6.0)
}

fn secondary_btn<'a>(label: String, on_press: Message) -> Element<'a, Message> {
  Button::secondary(label).size(Size::Sm).on_press(on_press).into()
}

/// The header "+ Milestone" button. It appends a milestone after the last skill (top of plan when
/// empty), but only in manual sort — milestones anchor to a hand-ordered plan. When disabled, the
/// tooltip explains how to re-enable it; when enabled it hints at the right-click alternative.
fn milestone_btn<'a>(is_manual: bool, last_entry_id: Option<i64>) -> Element<'a, Message> {
  let button: Element<'a, Message> = Button::secondary(t!("skills.editor_header.milestone").into_owned())
    .size(Size::Sm)
    .on_press_maybe(is_manual.then_some(Message::RemapInserted(last_entry_id)))
    .into();

  let hint = if is_manual {
    t!("skills.editor_header.milestone_hint")
  } else {
    t!("skills.editor_header.milestone_disabled_hint")
  };

  tooltip(
    button,
    container(
      text(hint.into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        }),
    )
    .max_width(240.0)
    .padding(spacing::SPACE_2_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    }),
    tooltip::Position::Bottom,
  )
  .gap(spacing::SPACE_2)
  .into()
}

fn inert_trigger<'a>(label: String, on_press: Message) -> Element<'a, Message> {
  button(
    row(vec![
      text(label).font(typography::body::REGULAR).size(13.0).into(),
      Space::new().width(spacing::SPACE_2).into(),
      Icon::chevron_down().size(13.0).color(color::text::secondary()).render(),
    ])
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(on_press)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, if active { 0.25 } else { 0.12 }),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn name_input<'a>(plan_name: &'a str) -> Element<'a, Message> {
  let placeholder = t!("skills.editor_header.name_placeholder");
  text_input(&placeholder, plan_name)
    .on_input(Message::NameChanged)
    .width(Length::Fixed(NAME_INPUT_WIDTH))
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 8.0,
      right: 8.0,
    })
    .size(15.0)
    .font(typography::body::MEDIUM)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border {
        color: Color::TRANSPARENT,
        radius: 4.0.into(),
        width: 0.0,
      },
      icon: color::text::secondary(),
      placeholder: color::text::tertiary(),
      value: color::text::PRIMARY,
      selection: color::accent_muted(),
    })
    .into()
}

fn template_badge<'a>() -> Element<'a, Message> {
  badge(
    t!("skills.editor_header.template_badge").to_uppercase(),
    Some(color::accent()),
  )
}

fn save_btn<'a>(dirty: bool) -> Element<'a, Message> {
  Button::primary(t!("skills.editor_header.save"))
    .size(Size::Sm)
    .on_press_maybe(dirty.then_some(Message::SaveRequested))
    .into()
}
