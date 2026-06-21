use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, mouse_area, scrollable, text, text_input},
};

use super::{
  BudgetDropTarget, BudgetFilterKind, BudgetMoveAnchor, Message, State,
  budget::{self, Category, CategoryDraft, Group, Mode, MoveDest, TargetKind, TargetState},
};
use crate::{
  features::budget as engine,
  store::model::{MatchMode, Rule, RuleField, RuleOp},
  ui::{
    components::{anchored_dropdown::AnchoredDropdown, icon::Icon},
    style::{color, spacing, typography},
  },
};

const ASSIGNED_COL: f32 = 152.0;
const ACTIVITY_COL: f32 = 146.0;
const AVAILABLE_COL: f32 = 172.0;
const DOT_COL: f32 = 28.0;
const MOVE_POPOVER_WIDTH: f32 = 306.0;
const SIDE_PADDING: f32 = 28.0;

pub(super) fn surface(state: &State) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![sub_nav(state)];

  if state.budget_mode() == Mode::Reflect {
    children.push(super::budget_reflect::reflect_surface(state));
  } else {
    children.push(toolbar(state));
    if let Some(banner) = review_banner(state) {
      children.push(banner);
    }
    children.push(plan_body(state));
  }

  Column::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The amber "Review & assign" banner: surfaces how many of the selected month's
/// entries still need a category and jumps to the ledger filtered to them.
fn review_banner(state: &State) -> Option<Element<'_, Message>> {
  let count = state.budget_review_total();
  if count == 0 {
    return None;
  }
  let noun = if count == 1 {
    "transaction needs"
  } else {
    "transactions need"
  };

  let review = button(
    text("Review & assign")
      .font(typography::body::MEDIUM)
      .size(typography::size::SM)
      .style(typography::colored(color::status::WARNING)),
  )
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_3,
  })
  .on_press(Message::BudgetFilterApplied(BudgetFilterKind::Uncategorized))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(color::with_alpha(
        color::status::WARNING,
        if active { 0.16 } else { 0.0 },
      ))),
      border: Border {
        color: color::status::WARNING,
        width: 1.0,
        radius: 8.0.into(),
      },
      text_color: color::status::WARNING,
      ..button::Style::default()
    }
  });

  let message = Row::with_children(vec![
    color_dot(Some("warning"), 8.0),
    text(format!("{count} {noun} a category"))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text("Until assigned, this spending won\u{2019}t show against any envelope.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
    Space::new().width(Length::Fill).into(),
    review.into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Center);

  Some(
    container(message)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_3,
        right: SIDE_PADDING,
        bottom: spacing::SPACE_3,
        left: SIDE_PADDING,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.09))),
        border: Border {
          color: color::with_alpha(color::status::WARNING, 0.28),
          width: 1.0,
          radius: 0.0.into(),
        },
        ..container::Style::default()
      })
      .into(),
  )
}

fn sub_nav(state: &State) -> Element<'_, Message> {
  let mode = state.budget_mode();
  let toggle = Row::with_children(vec![
    mode_button("Plan", mode == Mode::Plan, Mode::Plan, true),
    mode_button("Reflect", mode == Mode::Reflect, Mode::Reflect, false),
  ])
  .into();

  let blurb = match mode {
    Mode::Plan => "Give every ISK a job",
    Mode::Reflect => "Look back at where it went",
  };

  let mut row: Vec<Element<'_, Message>> = vec![
    bordered(toggle),
    mono_caption(blurb, color::text::tertiary(), typography::size::XS_PLUS),
    Space::new().width(Length::Fill).into(),
  ];
  if mode == Mode::Plan {
    row.push(edit_toggle(state.budget_edit_mode()));
  }

  container(
    Row::with_children(row)
      .spacing(spacing::SPACE_3_5)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: SIDE_PADDING,
    bottom: spacing::SPACE_3,
    left: SIDE_PADDING,
  })
  .style(crate::ui::style::control::bordered_pane)
  .into()
}

fn mode_button<'a>(label: &'a str, active: bool, mode: Mode, leading: bool) -> Element<'a, Message> {
  let text_color = if active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let background = if active {
    Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))
  } else {
    Background::Color(Color::TRANSPARENT)
  };
  let border = Border {
    color: color::rule(),
    width: if leading { 1.0 } else { 0.0 },
    radius: 0.0.into(),
  };

  button(
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: 7.0,
    right: 18.0,
    bottom: 7.0,
    left: 18.0,
  })
  .on_press_maybe((!active).then_some(Message::BudgetModeSelected(mode)))
  .style(move |_, _| button::Style {
    background: Some(background),
    border: Border {
      color: color::rule(),
      width: 0.0,
      ..border
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn edit_toggle<'a>(edit_mode: bool) -> Element<'a, Message> {
  let label = if edit_mode { "Done editing" } else { "Edit budget" };
  let text_color = if edit_mode {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let background = if edit_mode {
    Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))
  } else {
    Background::Color(Color::TRANSPARENT)
  };
  let border_color = if edit_mode {
    color::accent::PLASMA
  } else {
    color::rule()
  };

  button(
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(text_color)),
  )
  .padding(Padding {
    top: 7.0,
    right: spacing::SPACE_3_5,
    bottom: 7.0,
    left: spacing::SPACE_3_5,
  })
  .on_press(Message::BudgetEditToggled)
  .style(move |_, _| button::Style {
    background: Some(background),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: spacing::SPACE_2.into(),
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn bordered<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
  container(content)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: spacing::SPACE_2.into(),
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
}

fn toolbar(state: &State) -> Element<'_, Message> {
  let view = state.budget();
  let ready = view.map_or(0.0, |v| v.ready_to_assign);
  let overspent = view.map_or(0.0, |v| v.overspent);

  let mut row: Vec<Element<'_, Message>> = vec![month_nav(state), ready_hero(ready)];
  if overspent < 0.0 {
    row.push(overspent_button(overspent));
  }

  container(
    Row::with_children(row)
      .spacing(16.0)
      .align_y(Vertical::Center)
      .height(Length::Shrink),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 16.0,
    right: SIDE_PADDING,
    bottom: 16.0,
    left: SIDE_PADDING,
  })
  .style(crate::ui::style::control::bordered_pane)
  .into()
}

fn month_nav(state: &State) -> Element<'_, Message> {
  let label = budget::month_label(state.budget_month());

  let center = Column::with_children(vec![
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(budget::month_relative_label(state.budget_month()))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_x(Horizontal::Center)
  .spacing(spacing::UNIT / 2.0);

  let row = Row::with_children(vec![
    nav_arrow(Icon::chevron_left(), -1),
    container(center)
      .padding(Padding {
        top: spacing::UNIT,
        right: spacing::SPACE_3_5,
        bottom: spacing::UNIT,
        left: spacing::SPACE_3_5,
      })
      .into(),
    nav_arrow(Icon::chevron_right(), 1),
  ])
  .align_y(Vertical::Center);

  container(row)
    .padding(spacing::UNIT)
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: spacing::SPACE_2.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn nav_arrow<'a>(icon: Icon, delta: i32) -> Element<'a, Message> {
  button(icon.size(16.0).color(color::text::secondary()).render())
    .padding(Padding {
      top: 6.0,
      right: 7.0,
      bottom: 6.0,
      left: 7.0,
    })
    .on_press(Message::BudgetMonthStepped(delta))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

fn ready_hero<'a>(ready: f64) -> Element<'a, Message> {
  let zero = ready.abs() < 1.0;
  let positive = ready > 0.0;
  let (value_color, fill_alpha, border_color, message) = if zero {
    (
      color::status::ONLINE,
      color::with_alpha(color::status::ONLINE, 0.10),
      color::with_alpha(color::status::ONLINE, 0.3),
      "Every ISK has a job. Nothing left idle.",
    )
  } else if positive {
    (
      color::accent::PLASMA,
      color::with_alpha(color::accent::PLASMA, 0.10),
      color::with_alpha(color::accent::PLASMA, 0.3),
      "Idle ISK earns nothing. Give it a job.",
    )
  } else {
    (
      color::status::DANGER,
      color::with_alpha(color::status::DANGER, 0.12),
      color::with_alpha(color::status::DANGER, 0.35),
      "You\u{2019}ve assigned more than you hold. Pull some back.",
    )
  };

  let amount = Row::with_children(vec![
    text(crate::ui::format::fmt_isk_full(ready))
      .font(typography::body::MEDIUM)
      .size(30.0)
      .style(typography::colored(value_color))
      .into(),
    text(" ISK")
      .font(typography::body::MEDIUM)
      .size(16.0)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_y(Vertical::Bottom);

  let left = Column::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text("Ready to Assign", None).into(),
    amount.into(),
  ])
  .spacing(spacing::UNIT);

  let mut row: Vec<Element<'a, Message>> = vec![
    left.into(),
    Space::new().width(Length::Fill).into(),
    hero_message(message),
  ];
  if positive && !zero {
    row.push(auto_assign_button());
  }

  container(Row::with_children(row).spacing(18.0).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: 20.0,
      bottom: spacing::SPACE_3,
      left: 20.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(fill_alpha)),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: 10.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn hero_message<'a>(message: &'a str) -> Element<'a, Message> {
  container(
    text(message.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .max_width(240.0)
  .into()
}

fn auto_assign_button<'a>() -> Element<'a, Message> {
  button(
    text("Auto-Assign")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::on_fill(color::accent::PLASMA))),
  )
  .padding(Padding {
    top: 9.0,
    right: 16.0,
    bottom: 9.0,
    left: 16.0,
  })
  .on_press(Message::BudgetAutoAssign)
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::accent::PLASMA)),
    border: Border {
      radius: spacing::SPACE_2.into(),
      ..Border::default()
    },
    text_color: color::on_fill(color::accent::PLASMA),
    ..button::Style::default()
  })
  .into()
}

fn overspent_button<'a>(overspent: f64) -> Element<'a, Message> {
  let body = Column::with_children(vec![
    text("Overspent")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::status::DANGER))
      .into(),
    text(crate::ui::format::fmt_isk(overspent))
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::status::DANGER))
      .into(),
    text("Click to cover")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::UNIT - 1.0);

  button(body)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: 18.0,
      bottom: spacing::SPACE_2,
      left: 18.0,
    })
    .on_press(Message::BudgetCoverOverspending)
    .style(|_, _| button::Style {
      background: Some(Background::Color(color::with_alpha(color::status::DANGER, 0.10))),
      border: Border {
        color: color::with_alpha(color::status::DANGER, 0.35),
        width: 1.0,
        radius: 10.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn plan_body(state: &State) -> Element<'_, Message> {
  let table = container(
    scrollable(envelope_table(state))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill);

  Row::with_children(vec![
    table.into(),
    crate::ui::components::resizable_pane::pane_handle(Message::BudgetInspectorDragStart),
    inspector(state),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn envelope_table(state: &State) -> Element<'_, Message> {
  let Some(view) = state.budget() else {
    return empty_table("Loading budget\u{2026}");
  };
  if view.groups.is_empty() {
    return empty_table("No budget categories yet.");
  }

  let edit_mode = state.budget_edit_mode();
  let drop_target = state.budget_drop_target();
  let mut children: Vec<Element<'_, Message>> = vec![column_heads()];
  for group in &view.groups {
    children.push(group_header(
      group,
      state.budget_collapsed(group.id),
      state,
      drop_target,
    ));
    if !state.budget_collapsed(group.id) {
      for category in &group.categories {
        children.push(category_row(state, category, drop_target));
      }
      if edit_mode {
        children.push(add_category_row(group.id));
      }
    }
  }
  if edit_mode {
    children.push(add_group_button());
  }
  children.push(Space::new().height(Length::Fixed(40.0)).into());

  Column::with_children(children).width(Length::Fill).into()
}

fn add_category_row<'a>(group_id: i64) -> Element<'a, Message> {
  let label = Row::with_children(vec![
    text("\u{ff0b}")
      .font(typography::body::REGULAR)
      .size(15.0)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text("Add category")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(9.0)
  .align_y(Vertical::Center);

  button(label)
    .width(Length::Fill)
    .padding(Padding {
      top: 9.0,
      right: 16.0,
      bottom: 9.0,
      left: 38.0,
    })
    .on_press(Message::BudgetCategoryAdded(group_id))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    })
    .into()
}

fn add_group_button<'a>() -> Element<'a, Message> {
  let label = Row::with_children(vec![
    text("\u{ff0b}")
      .font(typography::body::REGULAR)
      .size(16.0)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text("New category group")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(9.0)
  .align_y(Vertical::Center);

  button(label)
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 16.0,
      bottom: 14.0,
      left: 16.0,
    })
    .on_press(Message::BudgetGroupAdded)
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      text_color: color::text::secondary(),
      ..button::Style::default()
    })
    .into()
}

fn empty_table(message: &str) -> Element<'_, Message> {
  container(
    text(message.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(spacing::SPACE_6)
  .into()
}

fn column_heads<'a>() -> Element<'a, Message> {
  let row = Row::with_children(vec![
    Space::new().width(Length::Fixed(DOT_COL)).into(),
    container(crate::ui::components::eyebrow::eyebrow_text("Category", None))
      .padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 10.0,
      })
      .width(Length::Fill)
      .into(),
    head_cell("Assigned", ASSIGNED_COL),
    head_cell("Activity", ACTIVITY_COL),
    head_cell("Available", AVAILABLE_COL),
  ])
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: 0.0,
      bottom: spacing::SPACE_3,
      left: 0.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn head_cell<'a>(label: &'a str, width: f32) -> Element<'a, Message> {
  container(
    crate::ui::components::eyebrow::eyebrow_text(label, None)
      .width(Length::Fill)
      .align_x(Horizontal::Right),
  )
  .padding(Padding {
    top: 0.0,
    right: 16.0,
    bottom: 0.0,
    left: 16.0,
  })
  .width(Length::Fixed(width))
  .into()
}

fn mono_caption<'a>(value: impl Into<String>, value_color: Color, size: f32) -> Element<'a, Message> {
  text(value.into())
    .font(typography::mono::REGULAR)
    .size(size)
    .style(typography::colored(value_color))
    .into()
}

fn group_header<'a>(
  group: &'a Group,
  collapsed: bool,
  state: &'a State,
  drop_target: Option<BudgetDropTarget>,
) -> Element<'a, Message> {
  let totals = group.totals();
  let edit_mode = state.budget_edit_mode();
  let caret = if collapsed {
    Icon::chevron_right()
  } else {
    Icon::chevron()
  };

  let caret_cell: Element<'a, Message> = if edit_mode {
    button(caret.size(14.0).color(color::text::secondary()).render())
      .padding(Padding::ZERO)
      .width(Length::Fixed(DOT_COL))
      .on_press(Message::BudgetGroupToggled(group.id))
      .style(|_, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        ..button::Style::default()
      })
      .into()
  } else {
    container(caret.size(14.0).color(color::text::secondary()).render())
      .width(Length::Fixed(DOT_COL))
      .align_x(Horizontal::Center)
      .into()
  };

  let name_cell: Element<'a, Message> = if edit_mode {
    group_name_editor(group, state)
  } else {
    container(
      text(group.name.to_uppercase())
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::PRIMARY)),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: 0.0,
      bottom: spacing::SPACE_2,
      left: 10.0,
    })
    .into()
  };

  // In edit mode the group is itself draggable: a grip leads the (Fill) name area
  // so the money columns stay aligned with the category rows below.
  let name_lead: Element<'a, Message> = if edit_mode {
    Row::with_children(vec![group_drag_grip(group.id), name_cell])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_2)
      .width(Length::Fill)
      .into()
  } else {
    name_cell
  };

  let row = Row::with_children(vec![
    caret_cell,
    name_lead,
    money_cell(totals.assigned, color::text::secondary(), ASSIGNED_COL, true),
    money_cell(totals.activity, color::text::secondary(), ACTIVITY_COL, true),
    money_cell(
      totals.available,
      if totals.available < 0.0 {
        color::status::DANGER
      } else {
        color::text::secondary()
      },
      AVAILABLE_COL,
      true,
    ),
  ])
  .align_y(Vertical::Center);

  // Highlight the header both when a category will drop into the group and when a
  // dragged group will land before it.
  let over =
    drop_target == Some(BudgetDropTarget::Group(group.id)) || state.budget_group_drop_target() == Some(group.id);
  let header = container(row).width(Length::Fill).style(move |_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: if over { color::accent::PLASMA } else { color::rule() },
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  });

  if edit_mode {
    let target = BudgetDropTarget::Group(group.id);
    return mouse_area(header)
      .on_enter(Message::BudgetDropTargetEntered(target))
      .on_exit(Message::BudgetDropTargetLeft)
      .into();
  }

  button(header)
    .padding(Padding::ZERO)
    .width(Length::Fill)
    .on_press(Message::BudgetGroupToggled(group.id))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      ..button::Style::default()
    })
    .into()
}

fn group_name_editor<'a>(group: &'a Group, state: &'a State) -> Element<'a, Message> {
  let group_id = group.id;
  let pending = state.budget_pending_group_delete() == Some(group_id);
  let input = text_input("", &group.name)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .padding(Padding {
      top: spacing::UNIT,
      right: spacing::SPACE_2,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2,
    })
    .width(Length::Fixed(240.0))
    .on_input(move |name| Message::BudgetGroupRenamed(group_id, name))
    .on_submit(Message::BudgetGroupRenameWritten)
    .style(group_name_input_style);

  let delete_label = if pending { "confirm?" } else { "delete" };
  let delete_color = if pending {
    color::status::DANGER
  } else {
    color::text::tertiary()
  };
  let delete = button(
    text(delete_label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(delete_color)),
  )
  .padding(Padding::ZERO)
  .on_press(Message::BudgetGroupDeleteRequested(group_id))
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color: color::text::tertiary(),
    ..button::Style::default()
  });

  container(
    Row::with_children(vec![input.into(), delete.into()])
      .spacing(10.0)
      .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: 0.0,
    bottom: spacing::SPACE_2,
    left: 10.0,
  })
  .into()
}

fn group_name_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::BASE),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 5.0.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    selection: color::with_alpha(color::accent::PLASMA, 0.3),
    value: color::text::PRIMARY,
  }
}

fn money_cell<'a>(value: f64, value_color: Color, width: f32, dim: bool) -> Element<'a, Message> {
  let resolved = if dim && value == 0.0 {
    color::text::tertiary()
  } else {
    value_color
  };
  container(
    text(crate::ui::format::fmt_isk_full(value))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .width(Length::Fill)
      .align_x(Horizontal::Right)
      .style(typography::colored(resolved)),
  )
  .padding(Padding {
    top: 0.0,
    right: 16.0,
    bottom: 0.0,
    left: 16.0,
  })
  .width(Length::Fixed(width))
  .into()
}

fn category_row<'a>(
  state: &'a State,
  category: &'a Category,
  drop_target: Option<BudgetDropTarget>,
) -> Element<'a, Message> {
  let selected = state.budget_selected() == Some(category.id);
  let edit_mode = state.budget_edit_mode();
  let status = category.status(state.budget_month());

  let lead: Element<'a, Message> = if edit_mode {
    drag_handle_cell(category.id)
  } else {
    dot_cell(category.tone.as_deref())
  };
  let tail: Element<'a, Message> = if edit_mode {
    delete_category_cell(category.id)
  } else {
    available_cell(state, category, &status, selected)
  };

  let hovered = state.budget_hovered_category() == Some(category.id);
  let over = drop_target == Some(BudgetDropTarget::Category(category.id));
  let background = if selected {
    Background::Color(color::with_alpha(color::accent::PLASMA, 0.07))
  } else {
    Background::Color(Color::TRANSPARENT)
  };
  let border_color = if selected { color::accent::PLASMA } else { color::rule() };

  if edit_mode {
    // The drag handle (lead) keeps its own on_press; selection moves to a separate
    // inner mouse_area over the remaining cells, and the outer mouse_area carries
    // only the drop enter/exit. Three non-overlapping press surfaces — handle,
    // selectable body, and a press-less drop wrapper — let a drag actually start
    // from the handle instead of being swallowed by the row's selection press.
    let selectable = mouse_area(
      Row::with_children(vec![
        name_cell(category, &status, hovered),
        assigned_cell(state, category),
        activity_cell(category.activity),
        tail,
      ])
      .align_y(Vertical::Center)
      .height(Length::Fixed(58.0))
      .width(Length::Fill),
    )
    .on_press(Message::BudgetCategorySelected(category.id));
    let inner = Row::with_children(vec![lead, selectable.into()])
      .align_y(Vertical::Center)
      .height(Length::Fixed(58.0));
    let body = container(inner)
      .width(Length::Fill)
      .style(move |_| category_drop_style(over, selected));
    let target = BudgetDropTarget::Category(category.id);
    return mouse_area(body)
      .on_enter(Message::BudgetDropTargetEntered(target))
      .on_exit(Message::BudgetDropTargetLeft)
      .into();
  }

  let row = Row::with_children(vec![
    lead,
    name_cell(category, &status, hovered),
    assigned_cell(state, category),
    activity_cell(category.activity),
    tail,
  ])
  .align_y(Vertical::Center)
  .height(Length::Fixed(58.0));

  let row_button = button(container(row).width(Length::Fill))
    .padding(Padding::ZERO)
    .width(Length::Fill)
    .on_press(Message::BudgetCategorySelected(category.id))
    .style(move |_, _| button::Style {
      background: Some(background),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: 0.0.into(),
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    });

  // Track hover so the row's "View transactions →" link reveals only on hover.
  mouse_area(row_button)
    .on_enter(Message::BudgetCategoryHovered(Some(category.id)))
    .on_exit(Message::BudgetCategoryHovered(None))
    .into()
}

/// The edit-mode row container style: a transparent top border that turns plasma
/// when a dragged category hovers over this row, matching the design's
/// top-border drop indicator.
fn category_drop_style(over: bool, selected: bool) -> container::Style {
  let background = if selected {
    Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.07)))
  } else {
    None
  };
  container::Style {
    background,
    border: Border {
      color: if over { color::accent::PLASMA } else { color::rule() },
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  }
}

fn drag_handle_cell<'a>(category_id: i64) -> Element<'a, Message> {
  let handle = text("\u{283f}")
    .font(typography::mono::REGULAR)
    .size(14.0)
    .style(typography::colored(color::text::tertiary()));

  let armed =
    mouse_area(container(handle).align_x(Horizontal::Center)).on_press(Message::BudgetDragStarted(category_id));

  container(armed)
    .width(Length::Fixed(DOT_COL))
    .align_x(Horizontal::Center)
    .into()
}

fn group_drag_grip<'a>(group_id: i64) -> Element<'a, Message> {
  let handle = text("\u{283f}")
    .font(typography::mono::REGULAR)
    .size(14.0)
    .style(typography::colored(color::text::tertiary()));

  mouse_area(container(handle).align_x(Horizontal::Center))
    .on_press(Message::BudgetGroupDragStarted(group_id))
    .into()
}

fn delete_category_cell<'a>(category_id: i64) -> Element<'a, Message> {
  let glyph = button(
    text("\u{00d7}")
      .font(typography::body::REGULAR)
      .size(14.0)
      .align_x(Horizontal::Center)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fixed(28.0))
  .height(Length::Fixed(28.0))
  .on_press(Message::BudgetCategoryDeleted(category_id))
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 6.0.into(),
    },
    text_color: color::text::secondary(),
    ..button::Style::default()
  });

  container(glyph)
    .width(Length::Fixed(AVAILABLE_COL))
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 0.0,
      right: 16.0,
      bottom: 0.0,
      left: 16.0,
    })
    .into()
}

fn dot_cell<'a>(tone: Option<&str>) -> Element<'a, Message> {
  container(color_dot(tone, 11.0))
    .width(Length::Fixed(DOT_COL))
    .align_x(Horizontal::Center)
    .into()
}

fn color_dot<'a>(tone: Option<&str>, size: f32) -> Element<'a, Message> {
  let fill = budget::tone_color(tone);
  container(Space::new())
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        radius: (size / 2.0).into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn name_cell<'a>(category: &'a Category, status: &budget::TargetStatus, hovered: bool) -> Element<'a, Message> {
  let mut head: Vec<Element<'a, Message>> = vec![
    text(category.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if let Some(by) = category.target.by_date.as_deref()
    && category.target.kind == TargetKind::GoalBy
  {
    head.push(due_pill(by));
  }
  // The "View transactions →" link only reveals on row hover, mirroring the design.
  if hovered {
    head.push(Space::new().width(Length::Fill).into());
    head.push(view_transactions_link(category.id, "View transactions \u{2192}"));
  }

  let underline = target_bar(status);

  Column::with_children(vec![
    Row::with_children(head)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    underline,
  ])
  .spacing(spacing::UNIT + 1.0)
  .width(Length::Fill)
  .padding(Padding {
    top: 10.0,
    right: 16.0,
    bottom: 10.0,
    left: 10.0,
  })
  .into()
}

/// A plasma "View transactions →" affordance that filters the ledger to a
/// category for the selected month and jumps to it.
fn view_transactions_link<'a>(category_id: i64, label: &'a str) -> Element<'a, Message> {
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::accent::PLASMA)),
  )
  .padding(0)
  .on_press(Message::BudgetFilterApplied(BudgetFilterKind::Category(category_id)))
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    ..button::Style::default()
  })
  .into()
}

fn due_pill<'a>(label: &'a str) -> Element<'a, Message> {
  container(
    text(format!("DUE {label}"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::status::WARNING)),
  )
  .padding(Padding {
    top: 1.0,
    right: 5.0,
    bottom: 1.0,
    left: 5.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.3),
      width: 1.0,
      radius: 3.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn target_bar<'a>(status: &budget::TargetStatus) -> Element<'a, Message> {
  let fill = status_color(status.state, true);
  let filled = (status.pct.max(0.02) * 1000.0) as u16;
  let empty = 1000_u16.saturating_sub(filled);

  let bar = container(
    Row::with_children(vec![
      progress_segment(filled, fill),
      progress_segment(empty, Color::TRANSPARENT),
    ])
    .width(Length::Fixed(120.0)),
  )
  .width(Length::Fixed(120.0))
  .height(Length::Fixed(3.0))
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.10))),
    border: Border {
      radius: 2.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  Row::with_children(vec![
    bar.into(),
    mono_caption(status.month_label.clone(), color::text::secondary(), 9.5),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .into()
}

fn progress_segment<'a>(portion: u16, fill: Color) -> Element<'a, Message> {
  if portion == 0 {
    return Space::new().width(Length::FillPortion(0)).into();
  }
  container(Space::new().width(Length::Fill).height(Length::Fill))
    .width(Length::FillPortion(portion))
    .height(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(fill)),
      ..container::Style::default()
    })
    .into()
}

fn activity_cell<'a>(activity: f64) -> Element<'a, Message> {
  let value_color = if activity > 0.0 {
    color::status::ONLINE
  } else if activity < 0.0 {
    color::text::secondary()
  } else {
    color::text::tertiary()
  };
  money_cell(activity, value_color, ACTIVITY_COL, true)
}

fn assigned_cell<'a>(state: &'a State, category: &'a Category) -> Element<'a, Message> {
  let editing = state.budget_editing().filter(|cell| cell.category_id == category.id);

  let inner: Element<'a, Message> = if let Some(cell) = editing {
    text_input("", &cell.draft)
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .padding(Padding {
        top: 5.0,
        right: 8.0,
        bottom: 5.0,
        left: 8.0,
      })
      .width(Length::Fixed(120.0))
      .align_x(Horizontal::Right)
      .on_input(Message::BudgetAssignDraftChanged)
      .on_submit(Message::BudgetAssignCommitted)
      .style(assigned_input_style)
      .into()
  } else {
    assigned_display(state, category)
  };

  container(inner)
    .width(Length::Fixed(ASSIGNED_COL))
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 0.0,
      right: 16.0,
      bottom: 0.0,
      left: 16.0,
    })
    .into()
}

fn assigned_display<'a>(state: &'a State, category: &'a Category) -> Element<'a, Message> {
  let value_color = if category.assigned == 0.0 {
    color::text::tertiary()
  } else {
    color::text::PRIMARY
  };
  let label = text(crate::ui::format::fmt_isk_full(category.assigned))
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(value_color));

  if state.budget_is_past() {
    return container(label).into();
  }

  button(label)
    .padding(Padding {
      top: 5.0,
      right: 8.0,
      bottom: 5.0,
      left: 8.0,
    })
    .on_press(Message::BudgetAssignEditBegan(category.id))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      text_color: color::text::PRIMARY,
      border: Border {
        radius: 5.0.into(),
        ..Border::default()
      },
      ..button::Style::default()
    })
    .into()
}

fn assigned_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::SUNKEN),
    border: Border {
      color: color::accent::PLASMA,
      width: 1.0,
      radius: 5.0.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    selection: color::with_alpha(color::accent::PLASMA, 0.3),
    value: color::text::PRIMARY,
  }
}

fn available_cell<'a>(
  state: &'a State,
  category: &'a Category,
  status: &budget::TargetStatus,
  selected: bool,
) -> Element<'a, Message> {
  let available = category.available();
  let (background, fg, border_color) = match status.state {
    TargetState::Over => (
      color::with_alpha(color::status::DANGER, 0.14),
      color::status::DANGER,
      color::with_alpha(color::status::DANGER, 0.4),
    ),
    TargetState::Met => (
      color::with_alpha(color::status::ONLINE, 0.14),
      color::status::ONLINE,
      color::with_alpha(color::status::ONLINE, 0.4),
    ),
    TargetState::Under if available > 0.0 => (
      color::with_alpha(color::text::PRIMARY, 0.06),
      color::text::PRIMARY,
      color::rule_strong(),
    ),
    TargetState::Under => (Color::TRANSPARENT, color::text::tertiary(), color::rule()),
  };

  let mut pill_children: Vec<Element<'a, Message>> = Vec::new();
  if status.state == TargetState::Over {
    pill_children.push(pill_glyph("!", fg));
  } else if status.state == TargetState::Met {
    pill_children.push(pill_glyph("\u{2713}", fg));
  }
  pill_children.push(
    text(crate::ui::format::fmt_isk_full(available))
      .font(typography::mono::MEDIUM)
      .size(typography::size::SM + 1.5)
      .style(typography::colored(fg))
      .into(),
  );

  let open = move_open_for(state, category.id, BudgetMoveAnchor::Pill);
  let border_final = if selected || open {
    color::accent::PLASMA
  } else {
    border_color
  };

  let on_press = if open {
    Message::BudgetMoveClosed
  } else {
    Message::BudgetMoveOpened(category.id, BudgetMoveAnchor::Pill)
  };
  let pill = button(Row::with_children(pill_children).spacing(7.0).align_y(Vertical::Center))
    .padding(Padding {
      top: 5.0,
      right: 11.0,
      bottom: 5.0,
      left: 11.0,
    })
    .on_press(on_press)
    .style(move |_, _| button::Style {
      background: Some(Background::Color(background)),
      border: Border {
        color: border_final,
        width: 1.0,
        radius: 13.0.into(),
      },
      ..button::Style::default()
    });

  let popover = open.then(|| move_money_popover(state, category));
  let trigger = AnchoredDropdown::new(pill, popover)
    .on_dismiss(Message::BudgetMoveClosed)
    .popover_width(MOVE_POPOVER_WIDTH);

  container(trigger)
    .width(Length::Fixed(AVAILABLE_COL))
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 0.0,
      right: 16.0,
      bottom: 0.0,
      left: 16.0,
    })
    .into()
}

/// Whether the Move Money popover is open and sourced on `category_id` from the
/// given `anchor`. The anchor disambiguates the row pill from the inspector
/// button so only the trigger that opened the move floats the popover.
fn move_open_for(state: &State, category_id: i64, anchor: BudgetMoveAnchor) -> bool {
  state
    .budget_move()
    .is_some_and(|open| open.from_id == category_id && open.anchor == anchor)
}

/// Destinations stay inert until the amount parses to a positive number, so a
/// stray click cannot move 0 ISK.
/// The transfer amount (ISK, rounded) and whether it is a usable positive
/// amount, parsed from the Move Money draft. Split off [`move_money_popover`]
/// so the parse/validity is unit-testable.
fn move_amount_state(draft: &str) -> (f64, bool) {
  let amount = crate::ui::format::parse_isk(draft).round();
  (amount, amount > 0.0)
}

/// The destination groups offered by the Move Money popover: every group with at
/// least one category other than the source, paired with those categories. Split
/// off [`move_money_popover`] so the eligibility rule is unit-testable.
fn eligible_move_dests(view: &budget::BudgetView, source_id: i64) -> Vec<(&str, Vec<&Category>)> {
  view
    .groups
    .iter()
    .filter_map(|group| {
      let dests: Vec<&Category> = group.categories.iter().filter(|c| c.id != source_id).collect();
      (!dests.is_empty()).then_some((group.name.as_str(), dests))
    })
    .collect()
}

fn move_money_popover<'a>(state: &'a State, source: &'a Category) -> Element<'a, Message> {
  let open = state.budget_move();
  let draft = open.map(|m| m.amount_draft.as_str()).unwrap_or_default();
  let (_amount, valid) = move_amount_state(draft);
  let available = source.available();

  let source_label = Row::with_children(vec![
    color_dot(source.tone.as_deref(), 12.0),
    text(source.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .width(Length::Fill)
      .into(),
    text(format!("{} avail", crate::ui::format::fmt_isk_full(available)))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(if available < 0.0 {
        color::status::DANGER
      } else {
        color::text::secondary()
      }))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let header = Column::with_children(vec![
    eyebrow_label("Move money from"),
    source_label.into(),
    move_amount_field(draft, available, valid).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .padding(spacing::SPACE_3);

  let mut rows: Vec<Element<'a, Message>> = vec![
    eyebrow_label("To"),
    move_dest_row(
      "Ready to Assign",
      color::accent::PLASMA,
      true,
      valid.then_some(Message::BudgetMoveCommitted(MoveDest::ReadyToAssign)),
    ),
  ];
  if let Some(view) = state.budget() {
    for (group_name, dests) in eligible_move_dests(view, source.id) {
      rows.push(
        text(group_name.to_owned())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      );
      for dest in dests {
        rows.push(move_dest_row(
          &dest.name,
          budget::tone_color(dest.tone.as_deref()),
          false,
          valid.then_some(Message::BudgetMoveCommitted(MoveDest::Category(dest.id))),
        ));
      }
    }
  }

  container(
    Column::with_children(vec![
      header.into(),
      scrollable(Column::with_children(rows).spacing(spacing::SPACE_2))
        .style(crate::ui::style::control::scrollbar)
        .height(Length::Fixed(280.0))
        .into(),
    ])
    .spacing(spacing::SPACE_2),
  )
  .padding(spacing::SPACE_2)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: 11.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

/// The "All" prefill value: `max(0, available)`, formatted, so a negative
/// available balance never seeds a negative transfer. Split off
/// [`move_amount_field`] for unit testing.
fn move_all_prefill(available: f64) -> String {
  crate::ui::format::fmt_isk(available.max(0.0))
}

/// "All" prefills `max(0, available)` so negative available does not seed a
/// negative transfer.
fn move_amount_field<'a>(draft: &str, available: f64, valid: bool) -> Row<'a, Message> {
  let input = text_input("0", draft)
    .on_input(Message::BudgetMoveAmountChanged)
    .font(typography::mono::MEDIUM)
    .size(typography::size::MD)
    .padding(Padding {
      top: 8.0,
      right: 11.0,
      bottom: 8.0,
      left: 11.0,
    })
    .style(move |_, _| text_input::Style {
      background: Background::Color(color::surface::SUNKEN),
      border: Border {
        color: if valid { color::accent::PLASMA } else { color::rule() },
        width: 1.0,
        radius: 7.0.into(),
      },
      icon: color::text::secondary(),
      placeholder: color::text::tertiary(),
      selection: color::with_alpha(color::accent::PLASMA, 0.3),
      value: color::text::PRIMARY,
    });

  let all = button(
    text("All")
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(Padding {
    top: 9.0,
    right: 13.0,
    bottom: 9.0,
    left: 13.0,
  })
  .on_press(Message::BudgetMoveAmountChanged(move_all_prefill(available)))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: 7.0.into(),
      },
      text_color: if active {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  });

  Row::with_children(vec![input.into(), all.into()])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
}

/// One destination row in the Move Money popover. `special` marks the
/// "Ready to Assign" pool with a hollow square rather than a filled tone dot.
/// `on_press` is `None` while the amount is invalid, leaving the row inert.
fn move_dest_row<'a>(label: &str, tone: Color, special: bool, on_press: Option<Message>) -> Element<'a, Message> {
  let dot: Element<'a, Message> = if special {
    container(Space::new())
      .width(Length::Fixed(11.0))
      .height(Length::Fixed(11.0))
      .style(move |_| container::Style {
        border: Border {
          color: tone,
          width: 1.5,
          radius: 3.0.into(),
        },
        ..container::Style::default()
      })
      .into()
  } else {
    container(Space::new())
      .width(Length::Fixed(11.0))
      .height(Length::Fixed(11.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(tone)),
        border: Border {
          radius: 5.5.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  };

  let row = Row::with_children(vec![
    dot,
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .width(Length::Fill)
      .into(),
    text("\u{2192}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let mut entry = button(row).width(Length::Fill).padding(Padding {
    top: 9.0,
    right: 14.0,
    bottom: 9.0,
    left: 14.0,
  });
  if let Some(message) = on_press {
    entry = entry.on_press(message);
  }
  entry
    .style(|_, status| {
      let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(if active {
          color::with_alpha(color::accent::PLASMA, 0.09)
        } else {
          Color::TRANSPARENT
        })),
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      }
    })
    .into()
}

fn eyebrow_label<'a>(label: &'a str) -> Element<'a, Message> {
  text(label)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn pill_glyph<'a>(glyph: &'a str, fg: Color) -> Element<'a, Message> {
  text(glyph.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(fg))
    .into()
}

fn status_color(state: TargetState, _bar: bool) -> Color {
  match state {
    TargetState::Over => color::status::DANGER,
    TargetState::Met => color::status::ONLINE,
    TargetState::Under => color::status::WARNING,
  }
}

fn inspector(state: &State) -> Element<'_, Message> {
  let selected = state
    .budget_selected()
    .and_then(|id| state.budget().and_then(|view| view.category(id)));

  let content: Element<'_, Message> = match selected {
    None => inspector_empty(),
    Some(category) => inspector_for(state, category),
  };

  container(
    scrollable(content)
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fixed(state.budget_inspector_width()))
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn inspector_empty<'a>() -> Element<'a, Message> {
  Column::with_children(vec![
    Icon::budget().size(34.0).color(color::text::tertiary()).render(),
    container(
      text("Select a category to inspect its target, set funding, and review activity.")
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .align_x(Horizontal::Center)
        .style(typography::colored(color::text::secondary())),
    )
    .max_width(220.0)
    .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_x(Horizontal::Center)
  .width(Length::Fill)
  .padding(32.0)
  .into()
}

fn inspector_for<'a>(state: &'a State, category: &'a Category) -> Element<'a, Message> {
  let status = category.status(state.budget_month());
  let editing = state.budget_editor().is_some() || state.budget_edit_mode();

  let mut children: Vec<Element<'a, Message>> = vec![inspector_header(state, category, &status)];

  // The bulk edit-mode table owns the inspector wholesale, so the Detail/Automation
  // tab bar only shows for a normal single-category inspection.
  if state.budget_edit_mode() {
    if let Some(draft) = state.budget_editor() {
      children.push(category_editor(draft));
    }
    return Column::with_children(children).width(Length::Fill).into();
  }

  children.push(inspector_tab_bar(state, category));

  match state.budget_inspector_tab() {
    budget::InspectorTab::Automation => children.push(automation_tab(state, category)),
    budget::InspectorTab::Detail if editing => {
      if let Some(draft) = state.budget_editor() {
        children.push(category_editor(draft));
      }
    }
    budget::InspectorTab::Detail => {
      children.push(target_block(&status));
      children.push(this_month_block(category, &status));
      children.push(quick_assign_block(category, state.budget_month()));
    }
  }

  Column::with_children(children).width(Length::Fill).into()
}

fn inspector_tab_bar<'a>(state: &'a State, category: &'a Category) -> Element<'a, Message> {
  let active = state.budget_inspector_tab();
  let rule_count = state
    .budget_rules()
    .iter()
    .filter(|rule| rule.category_id() == category.id)
    .count();

  let tabs = Row::with_children(vec![
    inspector_tab_button(
      "Detail",
      None,
      active == budget::InspectorTab::Detail,
      budget::InspectorTab::Detail,
    ),
    inspector_tab_button(
      "Automation",
      Some(rule_count),
      active == budget::InspectorTab::Automation,
      budget::InspectorTab::Automation,
    ),
  ])
  .spacing(20.0);

  container(tabs)
    .width(Length::Fill)
    .padding(Padding {
      top: 0.0,
      right: 20.0,
      bottom: 0.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn inspector_tab_button<'a>(
  label: &'a str,
  badge: Option<usize>,
  active: bool,
  tab: budget::InspectorTab,
) -> Element<'a, Message> {
  let text_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };

  let mut row: Vec<Element<'a, Message>> = vec![
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(text_color))
      .into(),
  ];
  if let Some(count) = badge.filter(|count| *count > 0) {
    row.push(count_badge(count, active));
  }

  // A 2px bottom underline marks the active tab (iced borders are uniform, so the
  // underline is its own hairline container rather than a button border edge).
  let underline = container(Space::new())
    .width(Length::Fill)
    .height(Length::Fixed(2.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(if active {
        color::accent::PLASMA
      } else {
        Color::TRANSPARENT
      })),
      ..container::Style::default()
    });

  let label_button = button(
    Row::with_children(row)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 11.0,
    right: 0.0,
    bottom: 9.0,
    left: 0.0,
  })
  .on_press(Message::BudgetInspectorTabSelected(tab))
  .style(move |_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    text_color,
    ..button::Style::default()
  });

  Column::with_children(vec![label_button.into(), underline.into()])
    .align_x(Horizontal::Center)
    .into()
}

fn count_badge<'a>(count: usize, active: bool) -> Element<'a, Message> {
  let tint = if active {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };
  container(
    text(count.to_string())
      .font(typography::mono::REGULAR)
      .size(9.5)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 1.0,
    right: 6.0,
    bottom: 1.0,
    left: 6.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(if active {
      color::with_alpha(color::accent::PLASMA, 0.12)
    } else {
      Color::TRANSPARENT
    })),
    border: Border {
      color: if active {
        color::with_alpha(color::accent::PLASMA, 0.3)
      } else {
        color::rule()
      },
      width: 1.0,
      radius: 9.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn inspector_header<'a>(
  state: &'a State,
  category: &'a Category,
  status: &budget::TargetStatus,
) -> Element<'a, Message> {
  let note = category.note.clone().unwrap_or_else(|| "No note".to_owned());

  let mut head: Vec<Element<'a, Message>> = vec![
    color_dot(category.tone.as_deref(), 14.0),
    Column::with_children(vec![
      text(category.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      text(note)
        .font(typography::mono::REGULAR)
        .size(9.5)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(spacing::UNIT / 2.0)
    .width(Length::Fill)
    .into(),
  ];
  if !state.budget_edit_mode() {
    head.push(editor_toggle_button(state.budget_editor().is_some()));
  }

  let available_color = match status.state {
    TargetState::Over => color::status::DANGER,
    TargetState::Met => color::status::ONLINE,
    TargetState::Under => color::text::PRIMARY,
  };
  let available_row = Row::with_children(vec![
    text(crate::ui::format::fmt_isk(category.available()))
      .font(typography::body::MEDIUM)
      .size(28.0)
      .style(typography::colored(available_color))
      .into(),
    crate::ui::components::eyebrow::eyebrow_text("available", None).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom);

  let mut body: Vec<Element<'a, Message>> = vec![
    Row::with_children(head).spacing(11.0).align_y(Vertical::Center).into(),
    available_row.into(),
  ];
  if !state.budget_edit_mode() {
    // Move money + Transactions sit side by side, each taking half the width
    // (mirrors the design's `gap: 8` action row under the available figure).
    body.push(
      Row::with_children(vec![
        inspector_move_button(state, category),
        inspector_transactions_button(category.id),
      ])
      .spacing(spacing::SPACE_2)
      .into(),
    );
  }

  Column::with_children(body)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .padding(section_padding())
    .into()
}

/// Same Move Money popover as the Available pill but anchored to this button;
/// renders only when this category's Inspector anchor is open.
fn inspector_move_button<'a>(state: &'a State, category: &'a Category) -> Element<'a, Message> {
  let open = move_open_for(state, category.id, BudgetMoveAnchor::Inspector);
  let on_press = if open {
    Message::BudgetMoveClosed
  } else {
    Message::BudgetMoveOpened(category.id, BudgetMoveAnchor::Inspector)
  };

  let trigger = button(
    Row::with_children(vec![
      text("\u{21C4}")
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::accent::PLASMA))
        .into(),
      text("Move money")
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(color::accent::PLASMA))
        .into(),
    ])
    .spacing(7.0)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 9.0,
    right: 10.0,
    bottom: 9.0,
    left: 10.0,
  })
  .on_press(on_press)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(color::with_alpha(
        color::accent::PLASMA,
        if active { 0.18 } else { 0.1 },
      ))),
      border: Border {
        color: color::accent::PLASMA,
        width: 1.0,
        radius: 7.0.into(),
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  });

  let popover = open.then(|| move_money_popover(state, category));
  AnchoredDropdown::new(trigger, popover)
    .on_dismiss(Message::BudgetMoveClosed)
    .popover_width(MOVE_POPOVER_WIDTH)
    .into()
}

/// Outline button beside Move money that filters the ledger to this category for
/// the selected month — the design's "Transactions" action. Plasma on hover.
fn inspector_transactions_button<'a>(category_id: i64) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      Icon::journal().size(14.0).color(color::text::PRIMARY).render(),
      text("Transactions")
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    ])
    .spacing(7.0)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 9.0,
    right: 10.0,
    bottom: 9.0,
    left: 10.0,
  })
  .on_press(Message::BudgetFilterApplied(BudgetFilterKind::Category(category_id)))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let accent = if active { color::accent::PLASMA } else { color::rule() };
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: accent,
        width: 1.0,
        radius: 7.0.into(),
      },
      text_color: if active {
        color::accent::PLASMA
      } else {
        color::text::PRIMARY
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn editor_toggle_button<'a>(active: bool) -> Element<'a, Message> {
  let tint = if active {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let border_color = if active { color::accent::PLASMA } else { color::rule() };
  let background = if active {
    Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))
  } else {
    Background::Color(Color::TRANSPARENT)
  };

  button(Icon::pencil().size(14.0).color(tint).render())
    .padding(8.0)
    .on_press(Message::BudgetEditorToggled)
    .style(move |_, _| button::Style {
      background: Some(background),
      border: Border {
        color: border_color,
        width: 1.0,
        radius: 7.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn section_padding() -> Padding {
  Padding {
    top: 18.0,
    right: 20.0,
    bottom: 18.0,
    left: 20.0,
  }
}

fn bordered_section<'a>(content: Column<'a, Message>) -> Element<'a, Message> {
  container(content)
    .width(Length::Fill)
    .padding(section_padding())
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn target_block<'a>(status: &budget::TargetStatus) -> Element<'a, Message> {
  let (state_label, state_color) = match status.state {
    TargetState::Over => ("Overspent", color::status::DANGER),
    TargetState::Met => ("Funded", color::status::ONLINE),
    TargetState::Under => ("Underfunded", color::status::WARNING),
  };

  let header = Row::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text("Target", None)
      .width(Length::Fill)
      .into(),
    text(state_label.to_uppercase())
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(state_color))
      .into(),
  ])
  .align_y(Vertical::Center);

  let bar_fill = status_color(status.state, true);
  let filled = (status.pct.max(0.02) * 1000.0) as u16;
  let empty = 1000_u16.saturating_sub(filled);
  let bar = container(
    Row::with_children(vec![
      progress_segment(filled, bar_fill),
      progress_segment(empty, Color::TRANSPARENT),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fixed(6.0))
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.10))),
    border: Border {
      radius: 3.0.into(),
      ..Border::default()
    },
    ..container::Style::default()
  });

  let mut footer: Vec<Element<'a, Message>> = vec![
    mono_caption(
      status.month_label.clone(),
      color::text::secondary(),
      typography::size::XS_PLUS,
    ),
    Space::new().width(Length::Fill).into(),
  ];
  if status.needed > 0.0 {
    footer.push(mono_caption(
      format!("{} to go", crate::ui::format::fmt_isk(status.needed)),
      color::status::WARNING,
      typography::size::XS_PLUS,
    ));
  }

  bordered_section(
    Column::with_children(vec![
      header.into(),
      text(status.label.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      bar.into(),
      Row::with_children(footer).align_y(Vertical::Center).into(),
    ])
    .spacing(spacing::SPACE_3),
  )
}

fn this_month_block<'a>(category: &'a Category, status: &budget::TargetStatus) -> Element<'a, Message> {
  let activity_color = if category.activity > 0.0 {
    color::status::ONLINE
  } else if category.activity < 0.0 {
    color::status::DANGER
  } else {
    color::text::tertiary()
  };

  let rows = Column::with_children(vec![
    breakdown_row(
      "Rolled over",
      crate::ui::format::fmt_isk_full(category.carry),
      color::text::secondary(),
    ),
    breakdown_row(
      "Assigned",
      crate::ui::format::fmt_isk_full(category.assigned),
      color::text::PRIMARY,
    ),
    breakdown_row("Activity", fmt_signed(category.activity), activity_color),
  ])
  .spacing(spacing::SPACE_2);

  let available_color = if status.state == TargetState::Over {
    color::status::DANGER
  } else {
    color::text::PRIMARY
  };
  let total = Row::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text("Available", None)
      .width(Length::Fill)
      .into(),
    text(crate::ui::format::fmt_isk_full(category.available()))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD + 1.0)
      .style(typography::colored(available_color))
      .into(),
  ])
  .align_y(Vertical::Center);

  bordered_section(
    Column::with_children(vec![
      crate::ui::components::eyebrow::eyebrow_text("This month", None).into(),
      rows.into(),
      crate::ui::components::rule::horizontal(),
      total.into(),
    ])
    .spacing(spacing::SPACE_3),
  )
}

fn breakdown_row<'a>(label: &'a str, value: String, value_color: Color) -> Element<'a, Message> {
  Row::with_children(vec![
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .width(Length::Fill)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text(value)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(value_color))
      .into(),
  ])
  .align_y(Vertical::Center)
  .into()
}

fn fmt_signed(value: f64) -> String {
  if value == 0.0 {
    return "0".to_owned();
  }
  let sign = if value > 0.0 { "+" } else { "-" };
  format!("{sign}{}", crate::ui::format::fmt_isk(value.abs()))
}

fn quick_assign_block<'a>(category: &'a Category, month: &str) -> Element<'a, Message> {
  let prev = budget::month_label(&budget::shift_month(month, -1));
  let mut suggestions: Vec<Element<'a, Message>> = Vec::new();
  if category.status(month).needed > 0.0 {
    suggestions.push(quick_assign_row(
      category.id,
      "Underfunded",
      Some("Meet this month\u{2019}s target".to_owned()),
      category.underfunded_assign(month),
    ));
  }
  suggestions.push(quick_assign_row(
    category.id,
    "Assigned last month",
    Some(prev),
    category.last_assigned,
  ));
  suggestions.push(quick_assign_row(
    category.id,
    "Spent last month",
    None,
    category.spent_last,
  ));
  suggestions.push(quick_assign_row(
    category.id,
    "Average assigned",
    Some("Trailing 3 months".to_owned()),
    category.avg_assigned,
  ));
  suggestions.push(quick_assign_row(category.id, "Set to zero", None, 0.0));

  Column::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text("Auto-assign", None).into(),
    Column::with_children(suggestions).spacing(7.0).into(),
  ])
  .spacing(spacing::SPACE_3)
  .width(Length::Fill)
  .padding(section_padding())
  .into()
}

fn quick_assign_row<'a>(category_id: i64, label: &'a str, hint: Option<String>, value: f64) -> Element<'a, Message> {
  let mut left: Vec<Element<'a, Message>> = vec![
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
  ];
  if let Some(hint) = hint {
    left.push(mono_caption(hint, color::text::tertiary(), typography::size::XS));
  }

  let row = Row::with_children(vec![
    Column::with_children(left)
      .spacing(spacing::UNIT / 2.0)
      .width(Length::Fill)
      .into(),
    text(crate::ui::format::fmt_isk(value))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD - 1.0)
      .style(typography::colored(color::accent::PLASMA))
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  button(row)
    .padding(Padding {
      top: 9.0,
      right: spacing::SPACE_3,
      bottom: 9.0,
      left: spacing::SPACE_3,
    })
    .width(Length::Fill)
    .on_press(Message::BudgetQuickAssign(category_id, value))
    .style(|_, _| button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 7.0.into(),
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    })
    .into()
}

fn category_editor(draft: &CategoryDraft) -> Element<'_, Message> {
  let mut children: Vec<Element<'_, Message>> = vec![
    editor_field("Name", text_field(&draft.name, "", Message::BudgetEditorNameChanged)),
    editor_field(
      "Note",
      text_field(&draft.note, "Optional", Message::BudgetEditorNoteChanged),
    ),
    editor_field("Colour", tone_picker(draft.tone.as_deref())),
    crate::ui::components::rule::horizontal(),
    editor_field("Target type", target_type_picker(draft.target_kind)),
    editor_field(draft.target_kind.amount_label(), money_field(&draft.target_amount_text)),
  ];
  if draft.target_kind == TargetKind::GoalBy {
    children.push(editor_field(
      "By date",
      text_field(&draft.by_date, "e.g. Jan 2028", Message::BudgetEditorByDateChanged),
    ));
  }
  children.push(editor_commit_button());

  Column::with_children(children)
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 18.0,
      left: 20.0,
    })
    .into()
}

fn editor_field<'a>(label: &'a str, control: Element<'a, Message>) -> Element<'a, Message> {
  Column::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text(label, None).into(),
    control,
  ])
  .spacing(7.0)
  .width(Length::Fill)
  .into()
}

fn text_field<'a>(value: &'a str, placeholder: &'a str, on_input: fn(String) -> Message) -> Element<'a, Message> {
  text_input(placeholder, value)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
    })
    .width(Length::Fill)
    .on_input(on_input)
    .on_submit(Message::BudgetEditorCommitted)
    .style(editor_input_style)
    .into()
}

fn money_field<'a>(value: &'a str) -> Element<'a, Message> {
  text_input("", value)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: spacing::SPACE_2,
      right: spacing::SPACE_2_5,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_2_5,
    })
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .on_input(Message::BudgetEditorAmountChanged)
    .on_submit(Message::BudgetEditorCommitted)
    .style(editor_input_style)
    .into()
}

fn editor_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(color::surface::BASE),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 7.0.into(),
    },
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    selection: color::with_alpha(color::accent::PLASMA, 0.3),
    value: color::text::PRIMARY,
  }
}

fn tone_picker<'a>(active: Option<&str>) -> Element<'a, Message> {
  let swatches = budget::tone_options()
    .into_iter()
    .map(|tone| tone_swatch(tone, active == Some(tone)))
    .collect::<Vec<Element<'a, Message>>>();

  Row::with_children(swatches).spacing(spacing::SPACE_2).into()
}

fn tone_swatch<'a>(tone: &'static str, active: bool) -> Element<'a, Message> {
  let fill = budget::tone_color(Some(tone));
  let border_color = if active {
    color::text::PRIMARY
  } else {
    Color::TRANSPARENT
  };

  button(Space::new().width(Length::Fixed(22.0)).height(Length::Fixed(22.0)))
    .padding(Padding::ZERO)
    .on_press(Message::BudgetEditorToneSelected(tone.to_owned()))
    .style(move |_, _| button::Style {
      background: Some(Background::Color(fill)),
      border: Border {
        color: border_color,
        width: 2.0,
        radius: 13.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn target_type_picker<'a>(active: TargetKind) -> Element<'a, Message> {
  let buttons = TargetKind::all()
    .into_iter()
    .map(|kind| target_type_button(kind, kind == active))
    .collect::<Vec<Element<'a, Message>>>();

  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  let mut iter = buttons.into_iter();
  while let Some(first) = iter.next() {
    let mut pair = vec![first];
    if let Some(second) = iter.next() {
      pair.push(second);
    } else {
      pair.push(Space::new().width(Length::Fill).into());
    }
    rows.push(Row::with_children(pair).spacing(6.0).into());
  }

  let hint = mono_caption_sans(active.hint());

  let mut children: Vec<Element<'a, Message>> = rows;
  children.push(hint);
  Column::with_children(children).spacing(6.0).width(Length::Fill).into()
}

fn target_type_button<'a>(kind: TargetKind, active: bool) -> Element<'a, Message> {
  let text_color = if active {
    color::text::PRIMARY
  } else {
    color::text::secondary()
  };
  let border_color = if active { color::accent::PLASMA } else { color::rule() };
  let background = if active {
    Background::Color(color::with_alpha(color::accent::PLASMA, 0.10))
  } else {
    Background::Color(Color::TRANSPARENT)
  };

  button(
    text(kind.label().to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(text_color)),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_2_5,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
  })
  .on_press(Message::BudgetEditorKindSelected(kind))
  .style(move |_, _| button::Style {
    background: Some(background),
    border: Border {
      color: border_color,
      width: 1.0,
      radius: 7.0.into(),
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn mono_caption_sans<'a>(value: &'a str) -> Element<'a, Message> {
  text(value.to_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()))
    .into()
}

fn editor_commit_button<'a>() -> Element<'a, Message> {
  button(
    text("Save category")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::on_fill(color::accent::PLASMA))),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 9.0,
    right: spacing::SPACE_3,
    bottom: 9.0,
    left: spacing::SPACE_3,
  })
  .on_press(Message::BudgetEditorCommitted)
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::accent::PLASMA)),
    border: Border {
      radius: 7.0.into(),
      ..Border::default()
    },
    text_color: color::on_fill(color::accent::PLASMA),
    ..button::Style::default()
  })
  .into()
}

fn automation_tab<'a>(state: &'a State, category: &'a Category) -> Element<'a, Message> {
  let outflows = state.budget_match_targets();
  let mine: Vec<&Rule> = state
    .budget_rules()
    .iter()
    .filter(|rule| rule.category_id() == category.id)
    .collect();
  let total_matched: usize = mine.iter().map(|rule| engine::match_count(rule, &outflows)).sum();

  let intro = Column::with_children(vec![automation_intro(category), new_rule_button(category.id)])
    .spacing(14.0)
    .width(Length::Fill);

  let mut children: Vec<Element<'a, Message>> = vec![bordered_section(intro)];

  if mine.is_empty() {
    children.push(automation_empty_state());
  } else {
    let cards = mine
      .iter()
      .map(|rule| rule_card(state, rule, engine::match_count(rule, &outflows)))
      .collect::<Vec<Element<'a, Message>>>();
    children.push(
      Column::with_children(cards)
        .spacing(8.0)
        .padding(Padding {
          top: 10.0,
          right: 14.0,
          bottom: 4.0,
          left: 14.0,
        })
        .into(),
    );
  }

  children.push(global_link(mine.len(), total_matched));

  Column::with_children(children).width(Length::Fill).into()
}

fn automation_intro<'a>(category: &'a Category) -> Element<'a, Message> {
  let line = Row::with_children(vec![
    text("Rules file matching spending into")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
    color_dot(category.tone.as_deref(), 9.0),
    text(category.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text("automatically. Manual picks always win.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center)
  .wrap();
  line.into()
}

fn new_rule_button<'a>(category_id: i64) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      text("+")
        .font(typography::body::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(color::accent::PLASMA))
        .into(),
      text("New rule")
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::accent::PLASMA))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 10.0,
    right: spacing::SPACE_3,
    bottom: 10.0,
    left: spacing::SPACE_3,
  })
  .on_press(Message::BudgetRuleNewOpened(category_id))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(color::with_alpha(
        color::accent::PLASMA,
        if active { 0.2 } else { 0.12 },
      ))),
      border: Border {
        color: color::accent::PLASMA,
        width: 1.0,
        radius: 8.0.into(),
      },
      text_color: color::accent::PLASMA,
      ..button::Style::default()
    }
  })
  .into()
}

fn automation_empty_state<'a>() -> Element<'a, Message> {
  Column::with_children(vec![
    Icon::budget().size(18.0).color(color::text::tertiary()).render(),
    container(
      text("No rules yet. Add one to stop hand-filing the same kind of transaction into this envelope.")
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .align_x(Horizontal::Center)
        .style(typography::colored(color::text::secondary())),
    )
    .max_width(220.0)
    .into(),
  ])
  .spacing(12.0)
  .align_x(Horizontal::Center)
  .width(Length::Fill)
  .padding(Padding {
    top: 34.0,
    right: 26.0,
    bottom: 34.0,
    left: 26.0,
  })
  .into()
}

fn rule_card<'a>(state: &'a State, rule: &'a Rule, count: usize) -> Element<'a, Message> {
  let dim = !rule.enabled();
  let name = rule_display_name(state, rule);
  let summary = engine::summarize_rule(
    rule,
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(state, key),
  );

  let header = Row::with_children(vec![
    rule_switch(rule.id(), rule.enabled()),
    text(name)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .width(Length::Fill)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    count_pill(count, rule.enabled()),
    rule_delete_button(rule.id()),
  ])
  .spacing(9.0)
  .align_y(Vertical::Center);

  let body = Column::with_children(vec![
    header.into(),
    container(
      text(summary)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::secondary())),
    )
    .padding(Padding {
      top: 8.0,
      right: 0.0,
      bottom: 0.0,
      left: 39.0,
    })
    .into(),
  ])
  .width(Length::Fill);

  let card = container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: 11.0,
      right: 12.0,
      bottom: 11.0,
      left: 12.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(if dim {
        color::with_alpha(color::surface::RAISED, 0.5)
      } else {
        color::surface::RAISED
      })),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 9.0.into(),
      },
      ..container::Style::default()
    });

  mouse_area(card)
    .on_press(Message::BudgetRuleEditOpened(rule.id()))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

fn rule_switch<'a>(rule_id: i64, on: bool) -> Element<'a, Message> {
  switch(on, Message::BudgetRuleToggled(rule_id, !on))
}

fn rule_delete_button<'a>(rule_id: i64) -> Element<'a, Message> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(5.0)
  .on_press(Message::BudgetRuleDeleted(rule_id))
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active {
          color::with_alpha(color::status::DANGER, 0.4)
        } else {
          color::rule()
        },
        width: 1.0,
        radius: 6.0.into(),
      },
      text_color: if active {
        color::status::DANGER
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

/// A compact pill toggle that reads as an on/off switch (iced has no native
/// switch widget). Plasma-tinted when on, hollow when off.
fn switch<'a>(on: bool, on_press: Message) -> Element<'a, Message> {
  let knob = container(Space::new())
    .width(Length::Fixed(13.0))
    .height(Length::Fixed(13.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(if on {
        color::accent::PLASMA
      } else {
        color::text::tertiary()
      })),
      border: Border {
        radius: 6.5.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });

  let track = Row::with_children(vec![
    if on {
      Space::new().width(Length::Fill).into()
    } else {
      Space::new().width(Length::Fixed(0.0)).into()
    },
    knob.into(),
    if on {
      Space::new().width(Length::Fixed(0.0)).into()
    } else {
      Space::new().width(Length::Fill).into()
    },
  ])
  .align_y(Vertical::Center);

  button(track)
    .width(Length::Fixed(32.0))
    .height(Length::Fixed(18.0))
    .padding(Padding {
      top: 1.0,
      right: 2.0,
      bottom: 1.0,
      left: 2.0,
    })
    .on_press(on_press)
    .style(move |_, _| button::Style {
      background: Some(Background::Color(if on {
        color::with_alpha(color::accent::PLASMA, 0.22)
      } else {
        color::with_alpha(color::text::PRIMARY, 0.05)
      })),
      border: Border {
        color: if on {
          color::accent::PLASMA
        } else {
          color::rule_strong()
        },
        width: 1.0,
        radius: 9.0.into(),
      },
      ..button::Style::default()
    })
    .into()
}

fn count_pill<'a>(count: usize, enabled: bool) -> Element<'a, Message> {
  let tint = if count > 0 && enabled {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };
  let label = format!("{count} match{}", if count == 1 { "" } else { "es" });
  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 2.0,
    right: 7.0,
    bottom: 2.0,
    left: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(if count > 0 {
      color::with_alpha(tint, 0.12)
    } else {
      Color::TRANSPARENT
    })),
    border: Border {
      color: if count > 0 {
        color::with_alpha(tint, 0.3)
      } else {
        color::rule()
      },
      width: 1.0,
      radius: 10.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn global_link<'a>(rule_count: usize, total_matched: usize) -> Element<'a, Message> {
  let link = button(
    Row::with_children(vec![
      text("Manage all rules & priority")
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .width(Length::Fill)
        .style(typography::colored(color::text::secondary()))
        .into(),
      text("\u{2192}")
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 10.0,
    right: 12.0,
    bottom: 10.0,
    left: 12.0,
  })
  .on_press(Message::BudgetGlobalRulesOpened)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: 8.0.into(),
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    }
  });

  let mut children: Vec<Element<'a, Message>> = vec![link.into()];
  if rule_count > 0 {
    let summary = format!(
      "{rule_count} rule{} \u{00b7} {total_matched} transaction{} filed here",
      if rule_count == 1 { "" } else { "s" },
      if total_matched == 1 { "" } else { "s" },
    );
    children.push(
      container(mono_caption(summary, color::text::tertiary(), 9.5))
        .width(Length::Fill)
        .align_x(Horizontal::Center)
        .padding(Padding {
          top: 9.0,
          right: 0.0,
          bottom: 0.0,
          left: 0.0,
        })
        .into(),
    );
  }

  container(Column::with_children(children).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 20.0,
      bottom: 22.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

/// Falls back through the user-given name, then the engine's auto-suggested name,
/// then an "Untitled rule" placeholder.
fn rule_display_name(state: &State, rule: &Rule) -> String {
  if !rule.name().is_empty() {
    return rule.name().clone();
  }
  let suggested = engine::suggest_name(
    rule,
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(state, key),
  );
  if suggested.is_empty() {
    "Untitled rule".to_owned()
  } else {
    suggested
  }
}

fn character_name(state: &State, key: &str) -> Option<String> {
  let id = key.trim().parse::<i64>().ok()?;
  state
    .roster()
    .iter()
    .find(|pilot| pilot.id == id)
    .map(|pilot| pilot.name.clone())
}

const RULE_MODAL_WIDTH: f32 = 860.0;
const RULE_PREVIEW_WIDTH: f32 = 332.0;

/// Mounted over the wallet shell via `modal_overlay`, and also reused as the edit
/// action of the global rules manager. Open it by seeding `State.budget_rule_editor`
/// via [`Message::BudgetRuleEditOpened`]; renders nothing when that is `None`.
pub(super) fn rule_editor_modal(state: &State) -> Element<'_, Message> {
  let Some(draft) = state.budget_rule_editor() else {
    return Space::new().into();
  };
  let category = state.budget().and_then(|view| view.category(draft.category_id));

  let panel = container(
    Column::with_children(vec![
      rule_modal_header(draft, category),
      rule_modal_body(state, draft, category),
      rule_modal_footer(draft),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fixed(RULE_MODAL_WIDTH))
  .max_height(720.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: 14.0.into(),
    },
    ..container::Style::default()
  });

  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(28.0)
    .into()
}

const GLOBAL_RULES_MODAL_WIDTH: f32 = 760.0;

/// The "Automation rules" manager: every rule across all envelopes in priority
/// order, drag-to-reorder. Mounted over the wallet shell via `modal_overlay`; the
/// edit action seeds `State.budget_rule_editor` so the editor stacks on top.
/// Renders nothing unless `budget_global_rules_open` is set.
pub(super) fn global_rules_modal(state: &State) -> Element<'_, Message> {
  let rules = state.budget_rules();
  let outflows = state.budget_match_targets();
  let enabled_count = rules.iter().filter(|rule| rule.enabled()).count();

  let mut sections: Vec<Element<'_, Message>> =
    vec![global_rules_header(rules.len(), enabled_count), global_rules_note()];

  if rules.is_empty() {
    sections.push(global_rules_empty_state());
  } else {
    let rows = rules
      .iter()
      .enumerate()
      .map(|(index, rule)| global_rule_row(state, rule, index, engine::match_count(rule, &outflows)))
      .collect::<Vec<Element<'_, Message>>>();
    sections.push(
      container(
        scrollable(Column::with_children(rows).width(Length::Fill))
          .style(crate::ui::style::control::scrollbar)
          .width(Length::Fill)
          .height(Length::Fill),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
    );
  }

  let panel = container(Column::with_children(sections).width(Length::Fill))
    .width(Length::Fixed(GLOBAL_RULES_MODAL_WIDTH))
    .max_height(680.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: 14.0.into(),
      },
      ..container::Style::default()
    });

  container(panel)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .padding(28.0)
    .into()
}

fn global_rules_header<'a>(rule_count: usize, enabled_count: usize) -> Element<'a, Message> {
  let summary = format!(
    "{rule_count} rule{} \u{00b7} {enabled_count} active \u{00b7} drag to set priority",
    if rule_count == 1 { "" } else { "s" },
  );

  let left = Column::with_children(vec![
    text("Automation rules")
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    mono_caption(summary, color::text::secondary(), typography::size::XS_PLUS),
  ])
  .spacing(4.0)
  .width(Length::Fill);

  let header = Row::with_children(vec![left.into(), global_rules_close_button()])
    .spacing(14.0)
    .align_y(Vertical::Center);

  container(header)
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 18.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn global_rules_close_button<'a>() -> Element<'a, Message> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(7.0)
  .on_press(Message::BudgetGlobalRulesClosed)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: 8.0.into(),
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

fn global_rules_note<'a>() -> Element<'a, Message> {
  let line = Row::with_children(vec![
    text("When a transaction matches more than one rule, the")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
    text("highest one wins")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(". Manual assignments override all rules.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(4.0)
  .align_y(Vertical::Center)
  .wrap();

  container(line)
    .width(Length::Fill)
    .padding(Padding {
      top: 11.0,
      right: 20.0,
      bottom: 11.0,
      left: 20.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn global_rules_empty_state<'a>() -> Element<'a, Message> {
  container(
    text("No rules yet. Create one from any budget envelope\u{2019}s Automation tab.")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .align_x(Horizontal::Center)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fill)
  .align_x(Horizontal::Center)
  .padding(Padding {
    top: 48.0,
    right: 24.0,
    bottom: 48.0,
    left: 24.0,
  })
  .into()
}

/// The tone color and display name of the category a rule files into, or no
/// tone and an empty label when it points at a category not in the active view.
/// Split off [`global_rule_row`] for unit testing.
fn global_rule_category_label<'a>(state: &'a State, rule: &Rule) -> (Option<&'a str>, String) {
  match state.budget().and_then(|view| view.category(rule.category_id())) {
    Some(category) => (category.tone.as_deref(), category.name.clone()),
    None => (None, String::new()),
  }
}

fn global_rule_row<'a>(state: &'a State, rule: &'a Rule, index: usize, count: usize) -> Element<'a, Message> {
  let rule_id = rule.id();
  let (tone, category_name) = global_rule_category_label(state, rule);
  let dragging = state.budget_rule_dragging() == Some(rule_id);
  let is_drop_target = state.budget_rule_drop_target() == Some(rule_id);

  let title = Row::with_children(vec![
    text(rule_display_name(state, rule))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    mono_caption(
      format!("\u{2192} {category_name}"),
      color::text::tertiary(),
      typography::size::XS,
    ),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let summary = engine::summarize_rule(
    rule,
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(state, key),
  );

  let detail = Column::with_children(vec![
    title.into(),
    mono_caption(summary, color::text::secondary(), typography::size::XS_PLUS),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    global_rule_drag_grip(rule_id),
    container(mono_caption(
      format!("{}", index + 1),
      color::text::secondary(),
      typography::size::SM,
    ))
    .width(Length::Fixed(18.0))
    .align_x(Horizontal::Right)
    .into(),
    color_dot(tone, 9.0),
    detail.into(),
    count_pill(count, rule.enabled()),
    rule_switch(rule_id, rule.enabled()),
    global_rule_edit_button(rule_id),
    rule_delete_button(rule_id),
  ])
  .spacing(12.0)
  .align_y(Vertical::Center);

  let body = container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 13.0,
      right: 20.0,
      bottom: 13.0,
      left: 20.0,
    })
    .style(move |_| container::Style {
      background: Some(Background::Color(if dragging {
        color::with_alpha(color::accent::PLASMA, 0.06)
      } else {
        Color::TRANSPARENT
      })),
      border: Border {
        color: if is_drop_target {
          color::accent::PLASMA
        } else {
          color::rule()
        },
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  mouse_area(body)
    .on_enter(Message::BudgetRuleDropTargetEntered(rule_id))
    .on_exit(Message::BudgetRuleDropTargetLeft)
    .into()
}

fn global_rule_drag_grip<'a>(rule_id: i64) -> Element<'a, Message> {
  let handle = text("\u{283f}")
    .font(typography::mono::REGULAR)
    .size(15.0)
    .style(typography::colored(color::text::tertiary()));

  mouse_area(container(handle).align_x(Horizontal::Center))
    .on_press(Message::BudgetRuleDragStarted(rule_id))
    .interaction(iced::mouse::Interaction::Grab)
    .into()
}

fn global_rule_edit_button<'a>(rule_id: i64) -> Element<'a, Message> {
  button(Icon::pencil().size(13.0).color(color::text::secondary()).render())
    .width(Length::Fixed(28.0))
    .height(Length::Fixed(28.0))
    .padding(Padding::ZERO)
    .on_press(Message::BudgetRuleEditOpened(rule_id))
    .style(|_, status| {
      let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
          color: if active { color::accent::PLASMA } else { color::rule() },
          width: 1.0,
          radius: 6.0.into(),
        },
        text_color: if active {
          color::accent::PLASMA
        } else {
          color::text::secondary()
        },
        ..button::Style::default()
      }
    })
    .into()
}

fn rule_modal_header<'a>(draft: &'a budget::RuleDraft, category: Option<&'a Category>) -> Element<'a, Message> {
  let eyebrow = if draft.rule_id.is_some() {
    "Edit rule"
  } else {
    "New rule"
  };

  let mut title: Vec<Element<'a, Message>> = vec![
    text("File matches into")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ];
  if let Some(category) = category {
    title.push(color_dot(category.tone.as_deref(), 9.0));
    title.push(
      text(category.name.clone())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    );
  }

  let left = Column::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text(eyebrow, None).into(),
    Row::with_children(title)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
  ])
  .spacing(5.0)
  .width(Length::Fill);

  let header = Row::with_children(vec![left.into(), rule_modal_close_button()])
    .spacing(14.0)
    .align_y(Vertical::Center);

  container(header)
    .width(Length::Fill)
    .padding(Padding {
      top: 16.0,
      right: 18.0,
      bottom: 16.0,
      left: 18.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn rule_modal_close_button<'a>() -> Element<'a, Message> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(7.0)
  .on_press(Message::BudgetRuleEditorClosed)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: 8.0.into(),
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

fn rule_modal_body<'a>(
  state: &'a State,
  draft: &'a budget::RuleDraft,
  category: Option<&'a Category>,
) -> Element<'a, Message> {
  let builder = container(
    scrollable(rule_builder(state, draft))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  });

  let preview = container(rule_preview(state, draft, category))
    .width(Length::Fixed(RULE_PREVIEW_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    });

  container(
    Row::with_children(vec![builder.into(), preview.into()])
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn rule_builder<'a>(state: &'a State, draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let mut children: Vec<Element<'a, Message>> = vec![rule_search_box(draft)];
  if draft.show_advanced {
    children.push(rule_advanced_block(state, draft));
  }
  children.push(rule_name_block(state, draft));

  Column::with_children(children)
    .spacing(16.0)
    .width(Length::Fill)
    .padding(Padding {
      top: 18.0,
      right: 20.0,
      bottom: 18.0,
      left: 20.0,
    })
    .into()
}

fn rule_search_box<'a>(draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let input = text_input("e.g. Cerberus, broker fee, Jita\u{2026}", draft.search_value())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 8.0,
      right: 11.0,
      bottom: 8.0,
      left: 11.0,
    })
    .width(Length::Fill)
    .on_input(Message::BudgetRuleEditorSearchChanged)
    .style(editor_input_style);

  let search_row = Row::with_children(vec![
    Icon::search().size(14.0).color(color::text::secondary()).render(),
    input.into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let caption = Row::with_children(vec![
    text("Searches reference, party, location & item.")
      .font(typography::body::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(Length::Fill)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    advanced_toggle(draft.show_advanced),
  ])
  .align_y(Vertical::Center);

  Column::with_children(vec![
    eyebrow_label("Match transactions containing"),
    search_row.into(),
    caption.into(),
  ])
  .spacing(9.0)
  .width(Length::Fill)
  .into()
}

fn advanced_toggle<'a>(advanced: bool) -> Element<'a, Message> {
  let tint = if advanced {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let label = if advanced { "Hide advanced" } else { "Add conditions" };
  button(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(tint)),
  )
  .padding(0)
  .on_press(Message::BudgetRuleEditorAdvancedToggled)
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::TRANSPARENT)),
    ..button::Style::default()
  })
  .into()
}

fn rule_advanced_block<'a>(state: &'a State, draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let mode_row = Row::with_children(vec![
    text("Match")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
    match_mode_segment(draft.match_mode),
    text("of these conditions")
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(10.0)
  .align_y(Vertical::Center);

  let removable = draft.conditions.len() > 1;
  let rows = draft
    .conditions
    .iter()
    .enumerate()
    .map(|(index, condition)| condition_row(state, draft, index, condition, removable))
    .collect::<Vec<Element<'a, Message>>>();

  let add = button(
    Row::with_children(vec![
      text("+")
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
      text("Add condition")
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .spacing(7.0)
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 7.0,
    right: 12.0,
    bottom: 7.0,
    left: 12.0,
  })
  .on_press(Message::BudgetRuleConditionAdded)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active {
          color::accent::PLASMA
        } else {
          color::rule_strong()
        },
        width: 1.0,
        radius: 7.0.into(),
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    }
  });

  let mut children: Vec<Element<'a, Message>> = vec![mode_row.into()];
  children.extend(rows);
  children.push(add.into());

  container(Column::with_children(children).spacing(8.0).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 0.0,
      bottom: 0.0,
      left: 0.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn condition_row<'a>(
  state: &'a State,
  draft: &'a budget::RuleDraft,
  index: usize,
  condition: &'a crate::store::model::RuleCondition,
  removable: bool,
) -> Element<'a, Message> {
  let row = Row::with_children(vec![
    field_select(draft, index, condition.field()),
    op_select(draft, index, condition.field(), condition.op()),
    condition_value_editor(state, draft, index, condition),
    remove_condition_button(index, removable),
  ])
  .spacing(7.0)
  .align_y(Vertical::Center);

  row.into()
}

fn field_select<'a>(draft: &'a budget::RuleDraft, index: usize, active: RuleField) -> Element<'a, Message> {
  let key = budget::RuleSelectKey::Field(index);
  let options = engine::rule_fields()
    .into_iter()
    .map(|field| {
      anchored_option(
        engine::field_label(field),
        field == active,
        Message::BudgetRuleConditionFieldChanged(index, field),
      )
    })
    .collect::<Vec<Element<'a, Message>>>();

  select_dropdown(
    engine::field_label(active),
    options,
    144.0,
    key,
    draft.open_select == Some(key),
  )
}

fn op_select<'a>(draft: &'a budget::RuleDraft, index: usize, field: RuleField, active: RuleOp) -> Element<'a, Message> {
  let key = budget::RuleSelectKey::Op(index);
  let options = engine::ops_for_field(field)
    .iter()
    .map(|op| {
      anchored_option(
        engine::op_label(*op),
        *op == active,
        Message::BudgetRuleConditionOpChanged(index, *op),
      )
    })
    .collect::<Vec<Element<'a, Message>>>();

  select_dropdown(
    engine::op_label(active),
    options,
    150.0,
    key,
    draft.open_select == Some(key),
  )
}

fn condition_value_editor<'a>(
  state: &'a State,
  draft: &'a budget::RuleDraft,
  index: usize,
  condition: &'a crate::store::model::RuleCondition,
) -> Element<'a, Message> {
  let open = draft.open_select == Some(budget::RuleSelectKey::Value(index));
  match engine::field_kind(condition.field()) {
    engine::FieldKind::Type => value_select(
      index,
      condition.value(),
      "Select type\u{2026}",
      rule_type_options(state),
      open,
    ),
    engine::FieldKind::Character => value_select(
      index,
      condition.value(),
      "Select character\u{2026}",
      rule_character_options(state),
      open,
    ),
    engine::FieldKind::Direction => value_select(
      index,
      condition.value(),
      "",
      engine::direction_options()
        .into_iter()
        .map(|(id, label)| (id.to_owned(), label.to_owned()))
        .collect(),
      open,
    ),
    engine::FieldKind::Amount if condition.op() == RuleOp::Between => amount_between_editor(index, condition),
    engine::FieldKind::Amount => amount_value_input(index, condition.value()),
    engine::FieldKind::Text => condition_text_input(index, condition.value()),
  }
}

fn condition_text_input<'a>(index: usize, value: &'a str) -> Element<'a, Message> {
  text_input("e.g. Cerberus", value)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 7.0,
      right: 10.0,
      bottom: 7.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .on_input(move |value| Message::BudgetRuleConditionValueChanged(index, value))
    .style(editor_input_style)
    .into()
}

fn amount_value_input<'a>(index: usize, value: &'a str) -> Element<'a, Message> {
  text_input("100M", value)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 7.0,
      right: 10.0,
      bottom: 7.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .on_input(move |value| Message::BudgetRuleConditionValueChanged(index, value))
    .style(editor_input_style)
    .into()
}

fn amount_between_editor<'a>(index: usize, condition: &'a crate::store::model::RuleCondition) -> Element<'a, Message> {
  let lower = text_input("100M", condition.value())
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 7.0,
      right: 10.0,
      bottom: 7.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .on_input(move |value| Message::BudgetRuleConditionValueChanged(index, value))
    .style(editor_input_style);

  let upper = text_input("1B", condition.value2().as_deref().unwrap_or(""))
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 7.0,
      right: 10.0,
      bottom: 7.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .on_input(move |value| Message::BudgetRuleConditionValue2Changed(index, value))
    .style(editor_input_style);

  Row::with_children(vec![
    lower.into(),
    text("\u{2013}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    upper.into(),
  ])
  .spacing(6.0)
  .align_y(Vertical::Center)
  .width(Length::Fill)
  .into()
}

fn value_select<'a>(
  index: usize,
  active: &'a str,
  placeholder: &'a str,
  options: Vec<(String, String)>,
  open: bool,
) -> Element<'a, Message> {
  let label = options
    .iter()
    .find(|(id, _)| id == active)
    .map(|(_, label)| label.clone())
    .unwrap_or_else(|| placeholder.to_owned());

  let rows = options
    .into_iter()
    .map(|(id, label)| {
      let selected = id == active;
      anchored_option(&label, selected, Message::BudgetRuleConditionValueChanged(index, id))
    })
    .collect::<Vec<Element<'a, Message>>>();

  select_dropdown(&label, rows, 0.0, budget::RuleSelectKey::Value(index), open)
}

fn remove_condition_button<'a>(index: usize, removable: bool) -> Element<'a, Message> {
  let mut remove = button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(if removable {
        color::text::secondary()
      } else {
        color::text::tertiary()
      })),
  )
  .padding(6.0)
  .style(move |_, status| {
    let active = removable && matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::status::DANGER } else { color::rule() },
        width: 1.0,
        radius: 6.0.into(),
      },
      text_color: if active {
        color::status::DANGER
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  });
  if removable {
    remove = remove.on_press(Message::BudgetRuleConditionRemoved(index));
  }
  remove.into()
}

fn rule_name_block<'a>(state: &'a State, draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let suggestion = rule_draft_suggestion(state, draft);
  let value = if draft.name_edited {
    draft.name.clone()
  } else if draft.name.is_empty() {
    suggestion.clone()
  } else {
    draft.name.clone()
  };
  let placeholder = if suggestion.is_empty() {
    "Name this rule".to_owned()
  } else {
    suggestion
  };

  let input = text_input(&placeholder, &value)
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .padding(Padding {
      top: 7.0,
      right: 10.0,
      bottom: 7.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .on_input(Message::BudgetRuleEditorNameChanged)
    .style(editor_input_style);

  container(
    Column::with_children(vec![eyebrow_label("Rule name"), input.into()])
      .spacing(8.0)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 16.0,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn rule_preview<'a>(
  state: &'a State,
  draft: &'a budget::RuleDraft,
  category: Option<&'a Category>,
) -> Element<'a, Message> {
  let outflows = state.budget_match_targets();
  let rule = draft_to_rule(draft);
  let other_rules = other_rules(state, draft);
  let manual = state.budget_manual_index();
  let rows = engine::preview_entries(&rule, &other_rules, &manual, draft.category_id, &outflows);
  let active_conditions = draft.conditions.iter().any(engine::is_active_condition);
  let will_assign = rows
    .iter()
    .filter(|(_, status)| *status == engine::PreviewStatus::Assign)
    .count();

  let count_color = if active_conditions {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };
  let count_row = Row::with_children(vec![
    text(rows.len().to_string())
      .font(typography::body::MEDIUM)
      .size(26.0)
      .style(typography::colored(count_color))
      .into(),
    text(if rows.len() == 1 { "match" } else { "matches" })
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom);

  let mut head: Vec<Element<'a, Message>> = vec![eyebrow_label("Live preview"), count_row.into()];
  if active_conditions {
    let name = category.map(|category| category.name.clone()).unwrap_or_default();
    head.push(mono_caption(
      format!("{will_assign} will file into {name}"),
      if will_assign > 0 {
        color::status::ONLINE
      } else {
        color::text::tertiary()
      },
      10.0,
    ));
  }

  let header = container(Column::with_children(head).spacing(8.0).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 16.0,
      bottom: 14.0,
      left: 16.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  let list: Element<'a, Message> = if !active_conditions {
    preview_empty("Type a search or add a condition to see which transactions this rule catches.")
  } else if rows.is_empty() {
    preview_empty("No spending matches yet. It\u{2019}ll still file matching transactions as they arrive.")
  } else {
    let cards = rows
      .iter()
      .map(|(index, status)| preview_row(&outflows[*index], *status))
      .collect::<Vec<Element<'a, Message>>>();
    scrollable(Column::with_children(cards).width(Length::Fill))
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  };

  Column::with_children(vec![
    header.into(),
    container(list).width(Length::Fill).height(Length::Fill).into(),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn preview_empty<'a>(message: &'a str) -> Element<'a, Message> {
  container(
    text(message.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .align_x(Horizontal::Center)
      .style(typography::colored(color::text::tertiary())),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .padding(Padding {
    top: 40.0,
    right: 22.0,
    bottom: 40.0,
    left: 22.0,
  })
  .into()
}

fn preview_row<'a>(target: &engine::MatchTarget, status: engine::PreviewStatus) -> Element<'a, Message> {
  let (label, tint) = preview_status_chip(status);
  let primary = if target.item.is_empty() {
    target.reference.clone()
  } else {
    target.item.clone()
  };

  let info = Column::with_children(vec![
    text(primary)
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(target.type_token.clone())
      .font(typography::mono::REGULAR)
      .size(9.5)
      .style(typography::colored(color::text::tertiary()))
      .into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let row = Row::with_children(vec![
    info.into(),
    text(format!("-{}", crate::ui::format::fmt_isk(target.amount)))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::status::DANGER))
      .into(),
    status_chip(label, tint),
  ])
  .spacing(10.0)
  .align_y(Vertical::Center);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: 9.0,
      right: 14.0,
      bottom: 9.0,
      left: 14.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn preview_status_chip(status: engine::PreviewStatus) -> (&'static str, Color) {
  match status {
    engine::PreviewStatus::Already => ("Already here", color::text::secondary()),
    engine::PreviewStatus::Assign => ("Will file here", color::status::ONLINE),
    engine::PreviewStatus::Manual => ("Manual, kept", color::status::WARNING),
    engine::PreviewStatus::Preempted => ("Higher rule wins", color::accent::PLASMA),
  }
}

fn status_chip<'a>(label: &'a str, tint: Color) -> Element<'a, Message> {
  container(
    text(label.to_owned())
      .font(typography::mono::REGULAR)
      .size(8.5)
      .style(typography::colored(tint)),
  )
  .padding(Padding {
    top: 3.0,
    right: 7.0,
    bottom: 3.0,
    left: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::with_alpha(tint, 0.11))),
    border: Border {
      color: color::with_alpha(tint, 0.32),
      width: 1.0,
      radius: 4.0.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn rule_modal_footer<'a>(draft: &'a budget::RuleDraft) -> Element<'a, Message> {
  let can_save = draft.conditions.iter().any(engine::is_active_condition);
  let save_label = if draft.rule_id.is_some() {
    "Save rule"
  } else {
    "Create rule"
  };

  let footer = Row::with_children(vec![
    text("Applies to matching past transactions and everything new. Manual assignments are never overridden.")
      .font(typography::body::REGULAR)
      .size(typography::size::XS_PLUS)
      .width(Length::Fill)
      .style(typography::colored(color::text::secondary()))
      .into(),
    cancel_button(),
    save_button(save_label, can_save),
  ])
  .spacing(12.0)
  .align_y(Vertical::Center);

  container(footer)
    .width(Length::Fill)
    .padding(Padding {
      top: 14.0,
      right: 18.0,
      bottom: 14.0,
      left: 18.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn cancel_button<'a>() -> Element<'a, Message> {
  button(
    text("Cancel")
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(Padding {
    top: 9.0,
    right: 16.0,
    bottom: 9.0,
    left: 16.0,
  })
  .on_press(Message::BudgetRuleEditorClosed)
  .style(|_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(Color::TRANSPARENT)),
      border: Border {
        color: if active { color::rule_strong() } else { color::rule() },
        width: 1.0,
        radius: 8.0.into(),
      },
      text_color: color::text::secondary(),
      ..button::Style::default()
    }
  })
  .into()
}

fn save_button<'a>(label: &'a str, enabled: bool) -> Element<'a, Message> {
  let mut save = button(
    text(label.to_owned())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(if enabled {
        color::on_fill(color::accent::PLASMA)
      } else {
        color::text::tertiary()
      })),
  )
  .padding(Padding {
    top: 9.0,
    right: 18.0,
    bottom: 9.0,
    left: 18.0,
  })
  .style(move |_, _| button::Style {
    background: Some(Background::Color(if enabled {
      color::accent::PLASMA
    } else {
      color::rule()
    })),
    border: Border {
      radius: 8.0.into(),
      ..Border::default()
    },
    text_color: if enabled {
      color::on_fill(color::accent::PLASMA)
    } else {
      color::text::tertiary()
    },
    ..button::Style::default()
  });
  if enabled {
    save = save.on_press(Message::BudgetRuleEditorCommitted);
  }
  save.into()
}

/// A select rendered as an `AnchoredDropdown`: a bordered trigger showing the
/// current label that floats its option list when `open`. Clicking the trigger
/// toggles the open select via [`Message::BudgetRuleSelectToggled`]; picking an
/// option emits its own message (which closes the select). The dropdown also
/// dismisses on outside click.
fn select_dropdown<'a>(
  label: &str,
  options: Vec<Element<'a, Message>>,
  fixed_width: f32,
  key: budget::RuleSelectKey,
  open: bool,
) -> Element<'a, Message> {
  let toggle = if open {
    Message::BudgetRuleSelectToggled(None)
  } else {
    Message::BudgetRuleSelectToggled(Some(key))
  };
  let trigger = select_trigger(label, toggle);

  let popover = open.then(|| {
    container(
      scrollable(Column::with_children(options).spacing(2.0))
        .style(crate::ui::style::control::scrollbar)
        .height(Length::Shrink),
    )
    .padding(spacing::SPACE_2)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: crate::ui::style::radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
  });

  let dropdown = AnchoredDropdown::new(trigger, popover).on_dismiss(Message::BudgetRuleSelectToggled(None));

  let element: Element<'a, Message> = dropdown.into();
  if fixed_width > 0.0 {
    container(element).width(Length::Fixed(fixed_width)).into()
  } else {
    container(element).width(Length::Fill).into()
  }
}

fn select_trigger<'a>(label: &str, toggle: Message) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      text(label.to_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .width(Length::Fill)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      text("\u{25BE}")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 7.0,
    right: 10.0,
    bottom: 7.0,
    left: 10.0,
  })
  .on_press(toggle)
  .style(|_, _| button::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 6.0.into(),
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn anchored_option<'a>(label: &str, selected: bool, on_press: Message) -> Element<'a, Message> {
  button(
    text(label.to_owned())
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(if selected {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      })),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2_5,
  })
  .on_press(on_press)
  .style(move |_, status| {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: Some(Background::Color(if selected || active {
        color::with_alpha(color::accent::PLASMA, 0.08)
      } else {
        Color::TRANSPARENT
      })),
      border: Border {
        radius: crate::ui::style::radius::SUBTLE.into(),
        ..Border::default()
      },
      text_color: if selected {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    }
  })
  .into()
}

fn match_mode_segment<'a>(active: MatchMode) -> Element<'a, Message> {
  let segments = [(MatchMode::All, "ALL"), (MatchMode::Any, "ANY")];
  let buttons = segments
    .into_iter()
    .map(|(mode, label)| {
      let is_active = mode == active;
      button(
        text(label)
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(typography::colored(if is_active {
            color::accent::PLASMA
          } else {
            color::text::secondary()
          })),
      )
      .padding(Padding {
        top: 6.0,
        right: 13.0,
        bottom: 6.0,
        left: 13.0,
      })
      .on_press(Message::BudgetRuleEditorMatchModeSelected(mode))
      .style(move |_, _| button::Style {
        background: Some(Background::Color(if is_active {
          color::with_alpha(color::accent::PLASMA, 0.14)
        } else {
          Color::TRANSPARENT
        })),
        text_color: if is_active {
          color::accent::PLASMA
        } else {
          color::text::secondary()
        },
        ..button::Style::default()
      })
      .into()
    })
    .collect::<Vec<Element<'a, Message>>>();

  container(Row::with_children(buttons))
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 7.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

/// Type options for the Type condition select: the humanized type tokens present
/// in the loaded outflows (journal ref_types + the two market sides), de-duped and
/// sorted by label.
fn rule_type_options(state: &State) -> Vec<(String, String)> {
  let mut seen = std::collections::BTreeMap::new();
  for target in state.budget_match_targets() {
    seen
      .entry(target.type_token.clone())
      .or_insert_with(|| engine::humanize_ref_type(&target.type_token));
  }
  let mut options: Vec<(String, String)> = seen.into_iter().collect();
  options.sort_by(|a, b| a.1.cmp(&b.1));
  options
}

fn rule_character_options(state: &State) -> Vec<(String, String)> {
  state
    .roster()
    .iter()
    .map(|pilot| (pilot.id.to_string(), pilot.name.clone()))
    .collect()
}

fn rule_draft_suggestion(state: &State, draft: &budget::RuleDraft) -> String {
  engine::suggest_name(
    &draft_to_rule(draft),
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(state, key),
  )
}

fn draft_to_rule(draft: &budget::RuleDraft) -> Rule {
  Rule {
    category_id: draft.category_id,
    conditions: draft.conditions.clone(),
    enabled: draft.enabled,
    id: draft.rule_id.unwrap_or(0),
    match_mode: draft.match_mode,
    name: draft.name.clone(),
  }
}

/// The live enabled rules excluding the one being edited, for preempt detection.
fn other_rules(state: &State, draft: &budget::RuleDraft) -> Vec<Rule> {
  state
    .budget_rules()
    .iter()
    .filter(|rule| Some(rule.id()) != draft.rule_id)
    .cloned()
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn category(id: i64, kind: TargetKind, amount: f64, assigned: f64) -> Category {
    Category {
      activity: -50.0,
      assigned,
      avg_assigned: 100.0,
      carry: 200.0,
      id,
      last_assigned: 120.0,
      name: format!("Category {id}"),
      note: Some("A note".to_owned()),
      spent_last: 80.0,
      target: budget::Target {
        amount,
        by_date: (kind == TargetKind::GoalBy).then(|| "Jan 2028".to_owned()),
        kind,
      },
      tone: Some("plasma".to_owned()),
    }
  }

  fn view() -> budget::BudgetView {
    budget::BudgetView {
      groups: vec![Group {
        categories: vec![
          category(1, TargetKind::Monthly, 1_000.0, 400.0),
          category(2, TargetKind::GoalBy, 50_000.0, 100.0),
        ],
        id: 10,
        name: "Bills".to_owned(),
      }],
      month: budget::current_month(),
      overspent: -50.0,
      pool: 5_000.0,
      ready_to_assign: 1_500.0,
    }
  }

  fn state_with_budget() -> State {
    let mut state = State::new(crate::config::FeatureFlags::default());
    state.tab = super::super::Tab::Budget;
    state.budget = Some(view());
    state.budget_selected = Some(1);
    state
  }

  fn sample_rule(id: i64, category_id: i64) -> Rule {
    Rule {
      category_id,
      conditions: vec![crate::store::model::RuleCondition {
        field: RuleField::Text,
        op: RuleOp::Contains,
        value: "Cerberus".to_owned(),
        value2: None,
      }],
      enabled: true,
      id,
      match_mode: MatchMode::All,
      name: "Doctrine hulls".to_owned(),
    }
  }

  mod automation {
    use super::*;

    #[test]
    fn it_renders_the_automation_tab_with_a_rule() {
      let mut state = state_with_budget();
      state.budget_inspector_tab = budget::InspectorTab::Automation;
      state.budget_chips.resolution.rules = vec![sample_rule(1, 1)];

      let _el: Element<'_, Message> = inspector(&state);
    }

    #[test]
    fn it_renders_the_automation_empty_state() {
      let mut state = state_with_budget();
      state.budget_inspector_tab = budget::InspectorTab::Automation;

      let _el: Element<'_, Message> = inspector(&state);
    }

    #[test]
    fn it_renders_the_rule_editor_modal_for_a_new_rule() {
      let mut state = state_with_budget();
      state.budget_rule_editor = Some(budget::RuleDraft::new(1));

      let _el: Element<'_, Message> = rule_editor_modal(&state);
    }

    #[test]
    fn it_renders_the_rule_editor_modal_in_advanced_mode() {
      let mut state = state_with_budget();
      let mut draft = budget::RuleDraft::from_rule(&sample_rule(5, 1));
      draft.show_advanced = true;
      draft.conditions.push(crate::store::model::RuleCondition {
        field: RuleField::Amount,
        op: RuleOp::Between,
        value: "100m".to_owned(),
        value2: Some("1b".to_owned()),
      });
      state.budget_rule_editor = Some(draft);

      let _el: Element<'_, Message> = rule_editor_modal(&state);
    }
  }

  mod surface {
    use super::*;

    #[test]
    fn it_renders_the_plan_surface_with_a_selection() {
      let state = state_with_budget();

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_renders_the_inspector_editor_when_open() {
      let mut state = state_with_budget();
      let selected = state.budget().unwrap().category(1).unwrap();
      state.budget_editor = Some(CategoryDraft::from_category(10, 0, selected));

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_renders_an_overspent_cover_affordance() {
      let state = state_with_budget();

      let _el: Element<'_, Message> = toolbar(&state);
    }

    #[test]
    fn it_renders_the_empty_inspector_without_a_selection() {
      let mut state = state_with_budget();
      state.budget_selected = None;

      let _el: Element<'_, Message> = inspector(&state);
    }

    #[test]
    fn it_renders_the_reflect_placeholder() {
      let mut state = state_with_budget();
      state.budget_mode = Mode::Reflect;

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_renders_a_collapsed_group() {
      let mut state = state_with_budget();
      state.budget_collapsed.insert(10);

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_renders_the_edit_mode_table() {
      let mut state = state_with_budget();
      state.budget_edit_mode = true;

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_highlights_a_drop_target_while_dragging() {
      let mut state = state_with_budget();
      state.budget_edit_mode = true;
      state.budget_dragging = Some(2);
      state.budget_drop_target = Some(BudgetDropTarget::Category(1));

      let _el: Element<'_, Message> = surface(&state);
    }

    #[test]
    fn it_highlights_a_group_drop_target_while_dragging_a_group() {
      let mut state = state_with_budget();
      state.budget_edit_mode = true;
      state.budget_group_dragging = Some(20);
      state.budget_group_drop_target = Some(10);

      let _el: Element<'_, Message> = surface(&state);
    }
  }

  mod review_banner {
    use super::*;

    #[test]
    fn it_renders_nothing_when_the_review_total_is_zero() {
      let state = state_with_budget();

      assert!(super::super::review_banner(&state).is_none());
    }

    #[test]
    fn it_renders_the_singular_banner_for_one_uncategorized_entry() {
      let mut state = state_with_budget();
      state.budget_review_total = 1;

      assert!(super::super::review_banner(&state).is_some());
    }

    #[test]
    fn it_renders_the_plural_banner_for_several_uncategorized_entries() {
      let mut state = state_with_budget();
      state.budget_review_total = 3;

      assert!(super::super::review_banner(&state).is_some());
    }
  }

  mod move_money {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::wallet::{BudgetMove, BudgetMoveAnchor};

    #[test]
    fn it_parses_and_validates_a_positive_amount() {
      assert_eq!(move_amount_state("550"), (550.0, true));
    }

    #[test]
    fn it_rejects_a_blank_or_zero_amount() {
      assert_eq!(move_amount_state(""), (0.0, false));
      assert_eq!(move_amount_state("0"), (0.0, false));
    }

    #[test]
    fn it_clamps_the_all_prefill_to_zero_for_a_negative_balance() {
      assert_eq!(move_all_prefill(-200.0), crate::ui::format::fmt_isk(0.0));
      assert_eq!(move_all_prefill(1_000.0), crate::ui::format::fmt_isk(1_000.0));
    }

    #[test]
    fn it_offers_every_group_with_a_non_source_destination() {
      let view = view();
      let dests = eligible_move_dests(&view, 1);

      assert_eq!(dests.len(), 1);
      let (group_name, categories) = &dests[0];
      assert_eq!(*group_name, "Bills");
      assert_eq!(categories.iter().map(|c| c.id).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn it_renders_the_move_popover_and_its_rows() {
      let mut state = state_with_budget();
      state.budget_move = Some(BudgetMove {
        amount_draft: "100".to_owned(),
        anchor: BudgetMoveAnchor::Pill,
        from_id: 1,
      });
      let source = state.budget().unwrap().category(1).unwrap();

      let _el: Element<'_, Message> = move_money_popover(&state, source);
      let _amount: Row<'_, Message> = move_amount_field("100", 1_000.0, true);
      let _ready: Element<'_, Message> = move_dest_row("Ready to Assign", color::accent::PLASMA, true, None);
      let _dest: Element<'_, Message> = move_dest_row(
        "Groceries",
        color::accent::PLASMA,
        false,
        Some(Message::BudgetMoveCommitted(MoveDest::ReadyToAssign)),
      );
    }
  }

  mod global_rules {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_labels_the_target_category_with_its_tone() {
      let state = state_with_budget();

      let (tone, name) = global_rule_category_label(&state, &sample_rule(1, 1));

      assert_eq!(tone, Some("plasma"));
      assert_eq!(name, "Category 1");
    }

    #[test]
    fn it_falls_back_to_an_empty_label_for_an_unknown_category() {
      let state = state_with_budget();

      let (tone, name) = global_rule_category_label(&state, &sample_rule(1, 999));

      assert_eq!(tone, None);
      assert_eq!(name, String::new());
    }

    #[test]
    fn it_renders_a_global_rule_row() {
      let state = state_with_budget();
      let rule = sample_rule(1, 1);

      let _el: Element<'_, Message> = global_rule_row(&state, &rule, 0, 3);
    }
  }

  mod condition_value_editor {
    use super::*;

    fn condition(field: RuleField, op: RuleOp) -> crate::store::model::RuleCondition {
      crate::store::model::RuleCondition {
        field,
        op,
        value: "100m".to_owned(),
        value2: Some("1b".to_owned()),
      }
    }

    #[test]
    fn it_renders_an_editor_for_every_field_kind() {
      let state = state_with_budget();
      let draft = budget::RuleDraft::new(1);
      let conditions = [
        condition(RuleField::Type, RuleOp::Is),
        condition(RuleField::Character, RuleOp::Is),
        condition(RuleField::Direction, RuleOp::Is),
        condition(RuleField::Amount, RuleOp::Between),
        condition(RuleField::Amount, RuleOp::GreaterThan),
        condition(RuleField::Text, RuleOp::Contains),
      ];

      for (index, condition) in conditions.iter().enumerate() {
        let _el: Element<'_, Message> = super::super::condition_value_editor(&state, &draft, index, condition);
      }
    }
  }
}
