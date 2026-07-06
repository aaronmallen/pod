//! Renders the interactive milestone header inserted between entry rows in the skill plan editor, not just a visual
//! separator: inline rename, segment stats, import/suggest/remove controls, and (when the milestone has a base
//! attribute set) a neural-remap readout row.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, button, column, container, row, text, text_input},
};

use super::{
  Attributes, EditMilestone, Message, MilestoneExportTarget, MilestoneImportSource, MilestoneStats, REMAP_ATTR_ORDER,
  attr_value, attribute_to_attr_key,
};
use crate::ui::{
  components::{
    anchored_dropdown::AnchoredDropdown,
    button::{Button, Size},
    eyebrow::eyebrow,
    icon::Icon,
  },
  format::{fmt_duration_coarse, fmt_sp_compact},
  style::{color, radius, typography},
};

const IMPORT_MENU_WIDTH: f32 = 180.0;

const INDEX_COL_WIDTH: f32 = 28.0;

pub(super) fn milestone_divider<'a>(
  milestone: &'a EditMilestone,
  stats: MilestoneStats,
  import_open: bool,
) -> Element<'a, Message> {
  let header = row(vec![
    index_mark(),
    Space::new().width(8.0).into(),
    title_block(milestone, stats.number),
    Space::new().width(Length::Fill).into(),
    segment_stats(stats),
    Space::new().width(6.0).into(),
    export_group(milestone.local_id),
    Space::new().width(6.0).into(),
    import_button(milestone.local_id, import_open),
    suggest_button(milestone.local_id, milestone.base.is_some(), stats.steps),
    remove_btn(milestone.local_id),
    Space::new().width(4.0).into(),
  ])
  .align_y(Vertical::Center)
  .spacing(10.0)
  .padding(Padding {
    top: 9.0,
    bottom: 9.0,
    left: 12.0,
    right: 12.0,
  });

  let mut body: Vec<Element<'a, Message>> = vec![header.into()];
  if let Some(base) = milestone.base {
    body.push(remap_row(milestone.local_id, base, milestone.auto_remap));
  }

  let band = container(column(body).width(Length::Fill))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent(), 0.06))),
      ..container::Style::default()
    });

  column(vec![hairline(), band.into(), hairline()])
    .width(Length::Fill)
    .into()
}

fn hairline<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(1.0))
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent(), 0.22))),
      ..container::Style::default()
    })
    .into()
}

fn index_mark<'a>() -> Element<'a, Message> {
  container(
    text("\u{25c6}")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::accent()),
      }),
  )
  .width(Length::Fixed(INDEX_COL_WIDTH))
  .align_x(Horizontal::Right)
  .align_y(Vertical::Center)
  .into()
}

fn title_block<'a>(milestone: &'a EditMilestone, number: usize) -> Element<'a, Message> {
  let count = number.to_string();
  let local_id = milestone.local_id;
  let name_input = text_input(&t!("skills.editor_milestone.name_placeholder"), &milestone.name)
    .on_input(move |value| Message::MilestoneRenamed(local_id, value))
    .width(Length::Fixed(220.0))
    .padding(0.0)
    .size(14.0)
    .font(typography::body::MEDIUM)
    .style(|_, _| text_input::Style {
      background: Background::Color(Color::TRANSPARENT),
      border: Border {
        color: Color::TRANSPARENT,
        radius: 0.0.into(),
        width: 0.0,
      },
      icon: color::text::secondary(),
      placeholder: color::text::tertiary(),
      value: color::text::PRIMARY,
      selection: color::accent_muted(),
    });

  column(vec![
    eyebrow(
      &t!("skills.editor_milestone.eyebrow", number => count),
      Some(color::accent()),
    ),
    Space::new().height(2.0).into(),
    name_input.into(),
  ])
  .into()
}

fn segment_stats<'a>(stats: MilestoneStats) -> Element<'a, Message> {
  let count = stats.steps.to_string();
  let sp = fmt_sp_compact(stats.sp);
  let time = fmt_duration_coarse(stats.sec.clamp(0.0, i64::MAX as f64) as i64);

  row(vec![
    stat_text(t!("skills.editor_milestone.steps_below", count => count).into_owned()),
    dot(),
    stat_text(t!("skills.header.sp_value", sp => sp).into_owned()),
    dot(),
    stat_text(time),
  ])
  .align_y(Vertical::Center)
  .spacing(10.0)
  .into()
}

fn stat_text<'a>(value: String) -> Element<'a, Message> {
  text(value)
    .font(typography::mono::REGULAR)
    .size(10.0)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into()
}

fn dot<'a>() -> Element<'a, Message> {
  text("\u{00b7}")
    .font(typography::mono::REGULAR)
    .size(10.0)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    })
    .into()
}

fn export_group<'a>(local_id: i64) -> Element<'a, Message> {
  row(vec![
    eyebrow(&t!("skills.editor_header.export"), Some(color::text::tertiary())),
    Space::new().width(2.0).into(),
    export_icon(local_id, MilestoneExportTarget::Clipboard, Icon::copy()),
    export_icon(local_id, MilestoneExportTarget::Csv, Icon::doc()),
    export_icon(local_id, MilestoneExportTarget::Psp, Icon::download()),
  ])
  .align_y(Vertical::Center)
  .spacing(2.0)
  .into()
}

fn export_icon<'a>(local_id: i64, target: MilestoneExportTarget, icon: Icon) -> Element<'a, Message> {
  Button::secondary_icon(icon)
    .size(Size::Sm)
    .on_press(Message::MilestoneExport(local_id, target))
    .into()
}

fn import_button<'a>(local_id: i64, open: bool) -> Element<'a, Message> {
  let trigger: Element<'a, Message> = Button::secondary(t!("skills.editor_header.import").into_owned())
    .icon_right(Icon::chevron_down())
    .size(Size::Sm)
    .on_press(Message::MilestoneImportMenuToggled(local_id))
    .into();

  let popover = open.then(|| import_menu(local_id));

  AnchoredDropdown::new(trigger, popover)
    .on_dismiss(Message::MilestoneImportMenuDismissed)
    .popover_width(IMPORT_MENU_WIDTH)
    .into()
}

fn import_menu<'a>(local_id: i64) -> Element<'a, Message> {
  let items = vec![
    import_menu_item(
      t!("skills.import_export.from_clipboard").into_owned(),
      Message::MilestoneImportPicked(local_id, MilestoneImportSource::Clipboard),
    ),
    import_menu_item(
      t!("skills.import_export.from_clipboard_eft").into_owned(),
      Message::MilestoneImportPicked(local_id, MilestoneImportSource::ClipboardEft),
    ),
    import_menu_item(
      t!("skills.import_export.from_file").into_owned(),
      Message::MilestoneImportPicked(local_id, MilestoneImportSource::File),
    ),
  ];

  container(column(items).width(Length::Fill))
    .width(Length::Fill)
    .padding(4.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::NAV_CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn import_menu_item<'a>(label: String, on_press: Message) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 8.0,
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
  })
  .on_press(on_press)
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.06)))
      }
      _ => None,
    },
    ..button::Style::default()
  })
  .into()
}

fn suggest_button<'a>(local_id: i64, has_remap: bool, steps: usize) -> Element<'a, Message> {
  let label = if has_remap {
    t!("skills.editor_milestone.resuggest")
  } else {
    t!("skills.editor_milestone.suggest")
  }
  .into_owned();

  let builder = if has_remap {
    Button::secondary(label)
  } else {
    Button::primary(label)
  };

  builder
    .mono()
    .size(Size::Sm)
    .on_press_maybe((steps > 0).then_some(Message::MilestoneRemapSuggested(local_id)))
    .into()
}

fn remove_btn<'a>(local_id: i64) -> Element<'a, Message> {
  button(
    container(
      text("\u{00d7}")
        .font(typography::mono::REGULAR)
        .size(13.0)
        .style(|_| text::Style {
          color: Some(color::status::DANGER),
        }),
    )
    .width(Length::Fixed(22.0))
    .height(Length::Fixed(22.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center),
  )
  .padding(0.0)
  .on_press(Message::MilestoneRemoved(local_id))
  .style(|_, status| button::Style {
    background: match status {
      button::Status::Hovered | button::Status::Pressed => {
        Some(Background::Color(color::with_alpha(color::status::DANGER, 0.12)))
      }
      _ => None,
    },
    border: Border {
      radius: 4.0.into(),
      ..Border::default()
    },
    text_color: color::status::DANGER,
    ..button::Style::default()
  })
  .into()
}

fn remap_row<'a>(local_id: i64, base: Attributes, auto: bool) -> Element<'a, Message> {
  let label = if auto {
    t!("skills.editor_milestone.neural_remap_auto")
  } else {
    t!("skills.editor_remap.neural_remap")
  };

  let readouts = REMAP_ATTR_ORDER.iter().fold(
    row(Vec::new()).align_y(Vertical::Center).spacing(4.0),
    |acc, &attribute| acc.push(readout(attribute, attr_value(base, attribute))),
  );

  row(vec![
    eyebrow(&label, Some(color::accent())),
    container(readouts.wrap())
      .width(Length::Fill)
      .align_x(Horizontal::Right)
      .into(),
    clear_btn(local_id),
  ])
  .align_y(Vertical::Center)
  .spacing(8.0)
  .padding(Padding {
    top: 0.0,
    bottom: 10.0,
    left: 48.0,
    right: 12.0,
  })
  .into()
}

fn readout<'a>(attribute: super::Attribute, value: u32) -> Element<'a, Message> {
  let key = attribute_to_attr_key(attribute);

  let body = row(vec![
    text(key.short())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().width(4.0).into(),
    text(value.to_string())
      .font(typography::mono::MEDIUM)
      .size(12.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ])
  .align_y(Vertical::Center);

  container(body)
    .padding(Padding {
      top: 3.0,
      bottom: 3.0,
      left: 8.0,
      right: 8.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent(), 0.08))),
      border: Border {
        color: color::with_alpha(color::accent(), 0.25),
        radius: 4.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn clear_btn<'a>(local_id: i64) -> Element<'a, Message> {
  button(
    text(t!("skills.editor_milestone.clear").into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 7.0,
    right: 7.0,
  })
  .on_press(Message::MilestoneRemapCleared(local_id))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: None,
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        radius: 4.0.into(),
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
