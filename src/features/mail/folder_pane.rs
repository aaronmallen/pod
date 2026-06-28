use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, mouse_area, scrollable, text},
};

use super::{
  DropTarget, Folder, Message, StandardFolder, State,
  loaders::{FolderLabel, StandardFolderCounts},
};
use crate::ui::{
  components::{
    button::{Button, Size},
    icon::Icon,
    rule,
    section_header::section_header as shared_section_header,
  },
  style::{color, radius, spacing, typography},
};

const SIDE_PADDING: f32 = 16.0;
const SELECT_RAIL: f32 = 2.0;
const LABEL_DOT_RADIUS: f32 = 3.0;
const LABEL_DOT_SIZE: f32 = 10.0;

const STANDARD_FOLDER_ROWS: [StandardFolder; 7] = [
  StandardFolder::Inbox,
  StandardFolder::Starred,
  StandardFolder::Snoozed,
  StandardFolder::Sent,
  StandardFolder::Drafts,
  StandardFolder::Archive,
  StandardFolder::Trash,
];

pub(super) fn pane(state: &State, width: f32) -> Element<'_, Message> {
  let data = state.folder_data();
  let selected = state.folder();

  let dragging = state.dragging_mail().is_some();
  let drop_target = state.drop_target();

  let mut column = Column::new().width(Length::Fill);
  column = column.push(unified_section(state, selected));
  column = column.push(folders_section(&data.standard_counts, selected, dragging, drop_target));
  column = column.push(labels_section(data.labels.as_slice(), dragging, drop_target));

  let scroll = scrollable(column)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill);

  container(scroll)
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
}

fn unified_section(state: &State, selected: Folder) -> Element<'_, Message> {
  let active = selected == Folder::Unified;
  let unread = state.unified_unread();

  let entry = selectable_row(
    Folder::Unified,
    active,
    Row::with_children(vec![
      folder_icon(Icon::inbox_all(), active),
      text(t!("mail.folder.unified"))
        .size(typography::size::MD)
        .font(typography::body::MEDIUM)
        .width(Length::Fill)
        .style(move |_| text::Style {
          color: Some(if active {
            color::text::PRIMARY
          } else {
            color::text::secondary()
          }),
        })
        .into(),
      unread_badge(unread).unwrap_or_else(|| Space::new().into()),
    ])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5),
    true,
  );

  let subline = text(t!("mail.folder_pane.mailboxes_combined", count => state.roster().len()).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });

  let section = container(
    Column::with_children(vec![
      section_header(&t!("mail.folder_pane.unified_header")),
      entry,
      container(subline)
        .padding(Padding {
          top: spacing::SPACE_2,
          bottom: 0.0,
          left: spacing::SPACE_2 / 2.0,
          right: 0.0,
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2_5),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: SIDE_PADDING,
    bottom: SIDE_PADDING - 2.0,
    left: SIDE_PADDING,
    right: SIDE_PADDING,
  });

  Column::with_children(vec![section.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

/// The standard boxes that accept a dragged message row as a pure local move.
fn is_drop_box(folder: StandardFolder) -> bool {
  matches!(
    folder,
    StandardFolder::Archive | StandardFolder::Inbox | StandardFolder::Trash
  )
}

fn folders_section<'a>(
  counts: &StandardFolderCounts,
  selected: Folder,
  dragging: bool,
  drop_target: Option<DropTarget>,
) -> Element<'a, Message> {
  let mut column = Column::new().width(Length::Fill).spacing(1.0);
  column = column.push(inset_header(&t!("mail.folder_pane.folders_header")));

  for standard_folder in STANDARD_FOLDER_ROWS {
    let folder = Folder::Standard(standard_folder);
    let over = drop_target == Some(DropTarget::StandardFolder(standard_folder));
    column = column.push(folder_row(
      folder,
      standard_folder_icon(standard_folder),
      standard_folder_label(standard_folder),
      counts.unread_for(standard_folder),
      selected == folder,
      dragging && is_drop_box(standard_folder),
      over,
    ));
  }

  container(column)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_6 - 4.0,
      bottom: spacing::SPACE_2,
      left: 0.0,
      right: 0.0,
    })
    .into()
}

fn inset_header<'a>(label: &str) -> Element<'a, Message> {
  container(section_header(label))
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: SIDE_PADDING,
      right: SIDE_PADDING,
    })
    .into()
}

fn folder_row<'a>(
  folder: Folder,
  icon: Icon,
  name: &'a str,
  unread: i64,
  active: bool,
  drop_box: bool,
  over: bool,
) -> Element<'a, Message> {
  let content = Row::with_children(vec![
    folder_icon(icon, active || over),
    text(name.to_owned())
      .size(typography::size::MD)
      .font(if active {
        typography::body::MEDIUM
      } else {
        typography::body::REGULAR
      })
      .width(Length::Fill)
      .style(move |_| text::Style {
        color: Some(if active || over {
          color::text::PRIMARY
        } else {
          color::text::secondary()
        }),
      })
      .into(),
    unread_badge(unread).unwrap_or_else(|| Space::new().into()),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2_5);

  let row = selectable_row(folder, active, content, false);

  let Folder::Standard(standard_folder) = folder else {
    return row;
  };
  if !drop_box {
    return row;
  }

  let target = DropTarget::StandardFolder(standard_folder);
  let highlighted = container(row).style(move |_| drop_box_style(over));
  mouse_area(highlighted)
    .on_enter(Message::DropTargetEntered(target))
    .on_exit(Message::DropTargetLeft(target))
    .into()
}

fn drop_box_style(over: bool) -> container::Style {
  container::Style {
    background: over.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.14))),
    border: Border {
      color: if over {
        color::accent::PLASMA
      } else {
        iced::Color::TRANSPARENT
      },
      radius: radius::SUBTLE.into(),
      width: if over { 1.0 } else { 0.0 },
    },
    ..container::Style::default()
  }
}

fn standard_folder_label(standard_folder: StandardFolder) -> &'static str {
  match standard_folder {
    StandardFolder::Archive => super::tr_static("mail.folder.archive"),
    StandardFolder::Drafts => super::tr_static("mail.folder.drafts"),
    StandardFolder::Inbox => super::tr_static("mail.folder.inbox"),
    StandardFolder::Sent => super::tr_static("mail.folder.sent"),
    StandardFolder::Snoozed => super::tr_static("mail.folder.snoozed"),
    StandardFolder::Starred => super::tr_static("mail.folder.starred"),
    StandardFolder::Trash => super::tr_static("mail.folder.trash"),
  }
}

fn standard_folder_icon(standard_folder: StandardFolder) -> Icon {
  match standard_folder {
    StandardFolder::Archive => Icon::archive(),
    StandardFolder::Drafts => Icon::draft(),
    StandardFolder::Inbox => Icon::inbox(),
    StandardFolder::Sent => Icon::send(),
    StandardFolder::Snoozed => Icon::snooze(),
    StandardFolder::Starred => Icon::star(),
    StandardFolder::Trash => Icon::trash(),
  }
}

fn folder_icon<'a>(icon: Icon, active: bool) -> Element<'a, Message> {
  let tone = if active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  icon.size(16.0).color(tone).render::<Message>()
}

fn labels_section(labels: &[FolderLabel], dragging: bool, drop_target: Option<DropTarget>) -> Element<'_, Message> {
  let mut column = Column::new().width(Length::Fill).spacing(1.0);
  column = column.push(labels_header());

  if labels.is_empty() {
    column = column.push(
      container(
        text(t!("mail.folder_pane.no_custom_labels"))
          .size(typography::size::SM)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          }),
      )
      .padding(Padding {
        top: spacing::SPACE_2 - 1.0,
        bottom: spacing::SPACE_2 - 1.0,
        left: spacing::SPACE_2 + 18.0,
        right: spacing::SPACE_2_5,
      }),
    );
  } else {
    for label in labels {
      column = column.push(label_row(label, drop_target == Some(DropTarget::Label(label.label_id))));
    }
    if dragging {
      column = column.push(drag_hint());
    }
  }

  container(column)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_6 - 4.0,
      bottom: spacing::SPACE_2,
      left: SIDE_PADDING,
      right: SIDE_PADDING,
    })
    .into()
}

fn labels_header<'a>() -> Element<'a, Message> {
  let add = Button::ghost_icon(Icon::plus())
    .size(Size::Sm)
    .on_press(Message::LabelModalOpened);

  let row = Row::with_children(vec![
    shared_section_header::<Message>(&t!("mail.folder_pane.labels_header"), None),
    Space::new().width(Length::Fill).into(),
    add.into(),
  ])
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_2 / 2.0,
      right: 0.0,
    })
    .into()
}

fn label_row(label: &FolderLabel, over: bool) -> Element<'_, Message> {
  let label_id = label.label_id;
  let chip = container(label_entry(label, over))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2 - 1.0,
      bottom: spacing::SPACE_2 - 1.0,
      left: spacing::SPACE_2 + 2.0,
      right: spacing::SPACE_2_5,
    })
    .style(move |_| label_row_style(over));

  mouse_area(chip)
    .on_press(Message::FolderSelected(Folder::Label(label_id)))
    .on_right_press(Message::LabelDeleteRequested(label_id))
    .on_enter(Message::DropTargetEntered(DropTarget::Label(label_id)))
    .on_exit(Message::DropTargetLeft(DropTarget::Label(label_id)))
    .into()
}

fn label_row_style(over: bool) -> container::Style {
  container::Style {
    background: over.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.14))),
    border: Border {
      color: if over {
        color::accent::PLASMA
      } else {
        iced::Color::TRANSPARENT
      },
      radius: radius::SUBTLE.into(),
      width: if over { 1.0 } else { 0.0 },
    },
    ..container::Style::default()
  }
}

fn drag_hint<'a>() -> Element<'a, Message> {
  container(
    text(t!("mail.folder_pane.drag_hint"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      }),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    bottom: 0.0,
    left: spacing::SPACE_2_5,
    right: spacing::SPACE_2_5,
  })
  .into()
}

fn label_entry(label: &FolderLabel, over: bool) -> Element<'_, Message> {
  let fill = label
    .color
    .as_deref()
    .and_then(color::from_hex)
    .unwrap_or_else(|| color::with_alpha(color::text::PRIMARY, 0.3));

  let dot = container(Space::new())
    .width(Length::Fixed(LABEL_DOT_SIZE))
    .height(Length::Fixed(LABEL_DOT_SIZE))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: color::with_alpha(iced::Color::BLACK, 0.35),
        radius: LABEL_DOT_RADIUS.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let name_tone = if over {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };
  let name = text(label.name.clone())
    .size(typography::size::MD)
    .width(Length::Fill)
    .style(move |_| text::Style {
      color: Some(name_tone),
    });

  Row::with_children(vec![dot.into(), name.into()])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center)
    .into()
}

fn section_header<'a>(label: &str) -> Element<'a, Message> {
  container(shared_section_header(label, None))
    .padding(Padding {
      top: 0.0,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_2 / 2.0,
      right: 0.0,
    })
    .into()
}

fn selectable_row<'a>(
  folder: Folder,
  active: bool,
  content: impl Into<Element<'a, Message>>,
  bordered: bool,
) -> Element<'a, Message> {
  if bordered {
    let row = container(content)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2 - 1.0,
        bottom: spacing::SPACE_2 - 1.0,
        left: spacing::SPACE_2_5,
        right: spacing::SPACE_2_5,
      })
      .style(move |_| selectable_row_style(active, true));
    return mouse_area(row).on_press(Message::FolderSelected(folder)).into();
  }

  let rail = container(Space::new())
    .width(Length::Fixed(SELECT_RAIL))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(if active {
        color::accent::PLASMA
      } else {
        iced::Color::TRANSPARENT
      })),
      ..container::Style::default()
    });

  let body = container(content)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2 - 1.0,
      bottom: spacing::SPACE_2 - 1.0,
      left: SIDE_PADDING - SELECT_RAIL,
      right: SIDE_PADDING,
    })
    .style(move |_| selectable_row_style(active, false));

  let row = Row::with_children(vec![rail.into(), body.into()]).align_y(Vertical::Center);
  mouse_area(row).on_press(Message::FolderSelected(folder)).into()
}

fn selectable_row_style(active: bool, bordered: bool) -> container::Style {
  let plasma = color::accent::PLASMA;
  container::Style {
    background: Some(Background::Color(if active {
      color::with_alpha(plasma, 0.10)
    } else if bordered {
      color::with_alpha(color::text::PRIMARY, 0.03)
    } else {
      iced::Color::TRANSPARENT
    })),
    border: Border {
      color: if active && bordered {
        color::with_alpha(plasma, 0.35)
      } else if bordered {
        color::with_alpha(color::text::PRIMARY, 0.1)
      } else {
        iced::Color::TRANSPARENT
      },
      radius: if bordered { radius::SUBTLE.into() } else { 0.0.into() },
      width: if bordered { 1.0 } else { 0.0 },
    },
    ..container::Style::default()
  }
}

fn unread_badge<'a>(unread: i64) -> Option<Element<'a, Message>> {
  if unread <= 0 {
    return None;
  }
  Some(
    text(unread.to_string())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::accent::PLASMA),
      })
      .into(),
  )
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn it_hides_the_badge_when_nothing_is_unread() {
    assert!(unread_badge(0).is_none());
    assert!(unread_badge(-1).is_none());
    assert!(unread_badge(3).is_some());
  }

  #[test]
  fn it_lists_the_fixed_standard_folder_set_in_prototype_order() {
    let folders: Vec<StandardFolder> = STANDARD_FOLDER_ROWS.to_vec();

    assert_eq!(
      folders,
      vec![
        StandardFolder::Inbox,
        StandardFolder::Starred,
        StandardFolder::Snoozed,
        StandardFolder::Sent,
        StandardFolder::Drafts,
        StandardFolder::Archive,
        StandardFolder::Trash,
      ]
    );
  }

  #[test]
  fn it_accepts_drops_only_on_inbox_archive_and_trash() {
    assert!(is_drop_box(StandardFolder::Inbox));
    assert!(is_drop_box(StandardFolder::Archive));
    assert!(is_drop_box(StandardFolder::Trash));
    assert!(!is_drop_box(StandardFolder::Starred));
    assert!(!is_drop_box(StandardFolder::Snoozed));
    assert!(!is_drop_box(StandardFolder::Sent));
    assert!(!is_drop_box(StandardFolder::Drafts));
  }

  #[test]
  fn it_highlights_a_drop_box_only_while_hovered() {
    assert!(drop_box_style(true).background.is_some());
    assert_eq!(drop_box_style(true).border.width, 1.0);
    assert!(drop_box_style(false).background.is_none());
    assert_eq!(drop_box_style(false).border.width, 0.0);
  }

  #[test]
  fn it_styles_a_selectable_row_per_active_and_bordered_combination() {
    assert!(selectable_row_style(true, false).background.is_some());
    assert_eq!(selectable_row_style(true, true).border.width, 1.0);
    assert_eq!(selectable_row_style(false, true).border.width, 1.0);
    assert_eq!(selectable_row_style(false, false).border.width, 0.0);
    assert_eq!(
      selectable_row_style(false, false).border.color,
      iced::Color::TRANSPARENT
    );
  }
}
