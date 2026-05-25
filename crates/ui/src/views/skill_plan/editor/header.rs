//! Editor header bar: plan name input, dirty indicator, and action buttons.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, button, column, container, row, text, text_input},
};

use super::super::Message;
use crate::{
  components,
  style::{
    color, radius, spacing,
    typography::{body, mono},
  },
};

/// The header bar rendered at the top of the plan editor.
pub struct EditorHeader<'a> {
  dirty: bool,
  picker_open: bool,
  plan_name: &'a str,
}

impl<'a> EditorHeader<'a> {
  /// Creates a new `EditorHeader`.
  pub fn new(plan_name: &'a str, dirty: bool, picker_open: bool) -> Self {
    Self {
      dirty,
      picker_open,
      plan_name,
    }
  }

  /// Renders the header bar into an [`Element`].
  pub fn render(self) -> Element<'a, Message> {
    let close_btn = header_close_btn();
    let name_input = header_name_input(self.plan_name);
    let dirty_dot = header_dirty_dot(self.dirty);
    let import_trigger = import_trigger_btn();
    let export_trigger = export_trigger_btn();
    let picker_label = if self.picker_open { "Hide picker" } else { "Add skills" };
    let picker_btn =
      components::Button::ghost(text(picker_label).font(body::REGULAR).size(13.0)).on_press(Message::PickerToggled);
    let save_btn = header_save_btn(self.dirty);

    let header_row = row([
      close_btn.into(),
      Space::new().width(spacing::SPACE_2).into(),
      name_input.into(),
      dirty_dot,
      Space::new().width(Length::Fill).into(),
      import_trigger.into(),
      Space::new().width(spacing::SPACE_2).into(),
      export_trigger.into(),
      Space::new().width(spacing::SPACE_2).into(),
      picker_btn.into(),
      Space::new().width(spacing::SPACE_2).into(),
      save_btn.into(),
    ])
    .align_y(Vertical::Center)
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    });

    container(column([
      container(header_row)
        .height(52.0)
        .width(Length::Fill)
        .align_y(Vertical::Center)
        .into(),
      components::Separator::horizontal().render(),
    ]))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      ..container::Style::default()
    })
    .into()
  }
}

fn dropdown_trigger_btn(label: &'static str, on_press: Message) -> button::Button<'static, Message> {
  button(
    row([
      text(label).font(body::REGULAR).size(13.0).into(),
      container(Space::new().width(1.0).height(14.0))
        .width(1.0)
        .height(14.0)
        .style(|_| container::Style {
          background: Some(Background::Color(color::border::SUBTLE)),
          ..container::Style::default()
        })
        .into(),
      text("\u{25be}").font(body::REGULAR).size(13.0).into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 12.0,
    right: 12.0,
  })
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: None,
    border: Border {
      color: match status {
        button::Status::Hovered | button::Status::Pressed => color::border::DEFAULT,
        _ => color::border::SUBTLE,
      },
      radius: 8.0.into(),
      width: 1.0,
    },
    text_color: match status {
      button::Status::Hovered | button::Status::Pressed => color::text::PRIMARY,
      _ => color::text::SECONDARY,
    },
    ..button::Style::default()
  })
}

fn export_trigger_btn() -> button::Button<'static, Message> {
  dropdown_trigger_btn("Export", Message::ExportDropdownToggled)
}

fn header_close_btn() -> button::Button<'static, Message> {
  button(
    text("←")
      .font(mono::REGULAR)
      .size(14.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .padding(Padding {
    top: 6.0,
    bottom: 6.0,
    left: 10.0,
    right: 10.0,
  })
  .on_press(Message::CloseRequested)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => Some(Background::Color(color::state::HOVER_OVERLAY)),
      _ => None,
    },
    border: Border {
      radius: radius::CHIP.into(),
      ..Border::default()
    },
    text_color: color::text::SECONDARY,
    ..button::Style::default()
  })
}

fn header_dirty_dot(dirty: bool) -> Element<'static, Message> {
  if dirty {
    container(Space::new().width(6.0).height(6.0))
      .width(6.0)
      .height(6.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::GOLD)),
        border: Border {
          radius: 3.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  } else {
    Space::new().width(0.0).height(0.0).into()
  }
}

fn header_name_input(plan_name: &str) -> iced::widget::TextInput<'_, Message> {
  text_input("Untitled plan", plan_name)
    .on_input(Message::NameChanged)
    .padding(Padding {
      top: 6.0,
      bottom: 6.0,
      left: 8.0,
      right: 8.0,
    })
    .size(15.0)
    .font(body::MEDIUM)
    .style(|_, _| iced::widget::text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border {
        color: Color::TRANSPARENT,
        radius: 4.0.into(),
        width: 0.0,
      },
      icon: color::text::SECONDARY,
      placeholder: color::text::TERTIARY,
      value: color::text::PRIMARY,
      selection: color::accent::PLASMA_SUBTLE,
    })
}

fn header_save_btn(dirty: bool) -> button::Button<'static, Message> {
  if dirty {
    components::Button::primary(text("Save").font(body::MEDIUM).size(13.0)).on_press(Message::SaveRequested)
  } else {
    components::Button::primary(
      text("Save")
        .font(body::MEDIUM)
        .size(13.0)
        .style(|_| iced::widget::text::Style {
          color: Some(color::text::TERTIARY),
        }),
    )
  }
}

fn import_trigger_btn() -> button::Button<'static, Message> {
  dropdown_trigger_btn("Import", Message::ImportDropdownToggled)
}
