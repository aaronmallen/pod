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
  features::wallet::budget_engine as engine,
  store::model::Rule,
  ui::{
    components::{
      anchored_dropdown::AnchoredDropdown,
      button::{Button, Size},
      icon::Icon,
    },
    style::{color, spacing, typography},
  },
};

const ASSIGNED_COL: f32 = 152.0;
const ACTIVITY_COL: f32 = 146.0;
const AVAILABLE_COL: f32 = 172.0;
const DOT_COL: f32 = 28.0;
const MOVE_POPOVER_WIDTH: f32 = 306.0;
const RECONCILE_MODAL_WIDTH: f32 = 486.0;
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

fn review_banner(state: &State) -> Option<Element<'_, Message>> {
  let count = state.budget_review_total();
  if count == 0 {
    return None;
  }
  let noun = if count == 1 {
    t!("wallet.budget.review_noun_singular").into_owned()
  } else {
    t!("wallet.budget.review_noun_plural").into_owned()
  };

  let review = Button::secondary(t!("wallet.budget.review_assign"))
    .on_press(Message::BudgetFilterApplied(BudgetFilterKind::Uncategorized));

  let message = Row::with_children(vec![
    color_dot(Some("warning"), 8.0),
    text(t!("wallet.budget.review_count", count => count, noun => noun))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("wallet.budget.review_hint"))
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
    mode_button(
      super::i18n::tr_static("wallet.budget.mode_plan"),
      mode == Mode::Plan,
      Mode::Plan,
      true,
    ),
    mode_button(
      super::i18n::tr_static("wallet.budget.mode_reflect"),
      mode == Mode::Reflect,
      Mode::Reflect,
      false,
    ),
  ])
  .into();

  let blurb = match mode {
    Mode::Plan => t!("wallet.budget.blurb_plan").into_owned(),
    Mode::Reflect => t!("wallet.budget.blurb_reflect").into_owned(),
  };

  let mut row: Vec<Element<'_, Message>> = vec![
    bordered(toggle),
    mono_caption(blurb, color::text::tertiary(), typography::size::XS_PLUS),
    Space::new().width(Length::Fill).into(),
  ];
  if mode == Mode::Plan {
    if !state.budget_edit_mode() {
      row.push(
        Button::secondary(super::i18n::tr_static("wallet.budget.reconcile"))
          .icon(Icon::plus_minus())
          .on_press(Message::BudgetReconcileOpened)
          .into(),
      );
    }
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
  let label = if edit_mode {
    t!("wallet.budget.done_editing").into_owned()
  } else {
    t!("wallet.budget.edit_budget").into_owned()
  };
  let control = if edit_mode {
    Button::primary(label)
  } else {
    Button::secondary(label)
  };
  control.on_press(Message::BudgetEditToggled).into()
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

pub(super) fn reconcile_modal(state: &State) -> Element<'_, Message> {
  let draft = state.budget_reconcile().map_or("", String::as_str);
  let view = state.budget();
  let tracked = view.map_or(0.0, |v| v.pool);
  let rta = view.map_or(0.0, |v| v.ready_to_assign);

  let empty = draft.trim().is_empty();
  // Both operands are rounded to whole ISK (parse_isk rounds, tracked is rounded here), so the
  // exact `== 0.0` comparisons below and in reconcile_body/reconcile_result are safe.
  let diff = crate::ui::format::parse_isk(draft) - tracked.round();
  let matches = diff == 0.0;

  let panel = container(
    Column::with_children(vec![
      reconcile_header(),
      reconcile_divider(),
      reconcile_body(draft, tracked, rta, diff, empty),
      reconcile_footer(matches, empty),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .max_width(RECONCILE_MODAL_WIDTH)
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
    .padding(24.0)
    .into()
}

fn reconcile_header<'a>() -> Element<'a, Message> {
  let tile = container(Icon::budget().size(20.0).color(color::accent::PLASMA).render())
    .width(Length::Fixed(40.0))
    .height(Length::Fixed(40.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
      border: Border {
        color: color::with_alpha(color::accent::PLASMA, 0.32),
        width: 1.0,
        radius: 10.0.into(),
      },
      ..container::Style::default()
    });

  let copy = Column::with_children(vec![
    text(super::i18n::tr_static("wallet.budget.reconcile_title"))
      .font(typography::body::MEDIUM)
      .size(18.0)
      .style(typography::colored(color::text::PRIMARY))
      .into(),
    text(t!("wallet.budget.reconcile_subtitle").into_owned())
      .font(typography::body::REGULAR)
      .size(12.5)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let close = button(
    text("\u{00d7}")
      .font(typography::mono::REGULAR)
      .size(typography::size::LG)
      .style(typography::colored(color::text::secondary())),
  )
  .width(Length::Fixed(30.0))
  .height(Length::Fixed(30.0))
  .padding(Padding::ZERO)
  .on_press(Message::BudgetReconcileClosed)
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

  container(
    Row::with_children(vec![tile.into(), copy.into(), close.into()])
      .spacing(14.0)
      .align_y(Vertical::Top),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 22.0,
    right: 24.0,
    bottom: 18.0,
    left: 24.0,
  })
  .into()
}

fn reconcile_divider<'a>() -> Element<'a, Message> {
  container(Space::new().height(Length::Fixed(1.0)))
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn reconcile_body<'a>(draft: &'a str, tracked: f64, rta: f64, diff: f64, empty: bool) -> Element<'a, Message> {
  let matches = diff == 0.0;

  let tracked_row = container(
    Row::with_children(vec![
      Column::with_children(vec![
        crate::ui::components::eyebrow::eyebrow_text(
          super::i18n::tr_static("wallet.budget.reconcile_tracked_label"),
          None,
        )
        .into(),
        text(t!("wallet.budget.reconcile_tracked_caption").into_owned())
          .font(typography::body::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .spacing(4.0)
      .width(Length::Fill)
      .into(),
      mono_caption(crate::ui::format::fmt_isk_full(tracked), color::text::PRIMARY, 15.0),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 12.0,
    right: 14.0,
    bottom: 12.0,
    left: 14.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: 9.0.into(),
    },
    ..container::Style::default()
  });

  let input = text_input("0.00", draft)
    .on_input(Message::BudgetReconcileActualChanged)
    .on_submit(Message::BudgetReconcileCommitted)
    .font(typography::mono::MEDIUM)
    .size(20.0)
    .align_x(Horizontal::Right)
    .padding(Padding {
      top: 13.0,
      right: 15.0,
      bottom: 13.0,
      left: 15.0,
    })
    .style(move |_, _| text_input::Style {
      background: Background::Color(color::surface::SUNKEN),
      border: Border {
        color: if matches { color::rule() } else { color::accent::PLASMA },
        width: 1.0,
        radius: 9.0.into(),
      },
      icon: color::text::secondary(),
      placeholder: color::text::tertiary(),
      selection: color::with_alpha(color::accent::PLASMA, 0.3),
      value: color::text::PRIMARY,
    });

  let input_row = Row::with_children(vec![
    input.into(),
    mono_caption("ISK", color::text::secondary(), typography::size::SM),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  container(
    Column::with_children(vec![
      tracked_row.into(),
      Space::new().height(Length::Fixed(16.0)).into(),
      crate::ui::components::eyebrow::eyebrow_text(
        super::i18n::tr_static("wallet.budget.reconcile_actual_label"),
        None,
      )
      .into(),
      Space::new().height(Length::Fixed(8.0)).into(),
      input_row.into(),
      Space::new().height(Length::Fixed(8.0)).into(),
      text(t!("wallet.budget.reconcile_actual_hint").into_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::tertiary()))
        .into(),
      Space::new().height(Length::Fixed(18.0)).into(),
      reconcile_result(rta, diff, empty),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 20.0,
    right: 24.0,
    bottom: 8.0,
    left: 24.0,
  })
  .into()
}

fn reconcile_result<'a>(rta: f64, diff: f64, _empty: bool) -> Element<'a, Message> {
  if diff == 0.0 {
    return container(
      Row::with_children(vec![
        text("\u{2713}")
          .size(14.0)
          .style(typography::colored(color::status::ONLINE))
          .into(),
        text(t!("wallet.budget.reconcile_match").into_owned())
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
      ])
      .spacing(10.0)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: 13.0,
      right: 15.0,
      bottom: 13.0,
      left: 15.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::ONLINE, 0.10))),
      border: Border {
        color: color::with_alpha(color::status::ONLINE, 0.32),
        width: 1.0,
        radius: 9.0.into(),
      },
      ..container::Style::default()
    })
    .into();
  }

  let positive = diff > 0.0;
  let tone = if positive {
    color::status::ONLINE
  } else {
    color::status::DANGER
  };
  let sign = if positive { "+" } else { "\u{2212}" };
  let explanation = if positive {
    t!("wallet.budget.reconcile_under").into_owned()
  } else {
    t!("wallet.budget.reconcile_over").into_owned()
  };
  let new_rta = rta + diff;

  let adjustment = container(
    Column::with_children(vec![
      Row::with_children(vec![
        container(crate::ui::components::eyebrow::eyebrow_text(
          super::i18n::tr_static("wallet.budget.reconcile_adjustment"),
          None,
        ))
        .width(Length::Fill)
        .into(),
        mono_caption(
          format!("{sign}{}", crate::ui::format::fmt_isk_full(diff.abs())),
          tone,
          22.0,
        ),
      ])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .into(),
      Space::new().height(Length::Fixed(8.0)).into(),
      text(explanation)
        .font(typography::body::REGULAR)
        .size(12.0)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 14.0,
    right: 16.0,
    bottom: 14.0,
    left: 16.0,
  });

  let rta_row = container(
    Row::with_children(vec![
      container(crate::ui::components::eyebrow::eyebrow_text(
        super::i18n::tr_static("wallet.budget.ready_to_assign"),
        None,
      ))
      .width(Length::Fill)
      .into(),
      Row::with_children(vec![
        mono_caption(
          crate::ui::format::fmt_isk(rta),
          color::text::tertiary(),
          typography::size::MD,
        ),
        mono_caption("\u{2192}", color::text::secondary(), typography::size::MD),
        mono_caption(
          crate::ui::format::fmt_isk(new_rta),
          if new_rta < 0.0 {
            color::status::DANGER
          } else {
            color::text::PRIMARY
          },
          typography::size::MD,
        ),
      ])
      .spacing(10.0)
      .align_y(Vertical::Center)
      .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 12.0,
    right: 16.0,
    bottom: 12.0,
    left: 16.0,
  });

  let mut sections: Vec<Element<'a, Message>> = vec![adjustment.into(), reconcile_divider(), rta_row.into()];
  if new_rta < 0.0 {
    let warning = container(
      Row::with_children(vec![
        text("!")
          .font(typography::body::MEDIUM)
          .size(12.0)
          .style(typography::colored(color::status::WARNING))
          .into(),
        text(t!("wallet.budget.reconcile_negative_rta").into_owned())
          .font(typography::body::REGULAR)
          .size(11.5)
          .style(typography::colored(color::status::WARNING))
          .into(),
      ])
      .spacing(9.0)
      .align_y(Vertical::Top),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: 11.0,
      right: 16.0,
      bottom: 11.0,
      left: 16.0,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.08))),
      ..container::Style::default()
    });
    sections.push(reconcile_divider());
    sections.push(warning.into());
  }

  container(Column::with_children(sections).width(Length::Fill))
    .width(Length::Fill)
    .clip(true)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(tone, 0.07))),
      border: Border {
        color: color::with_alpha(tone, if positive { 0.30 } else { 0.32 }),
        width: 1.0,
        radius: 10.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn reconcile_footer<'a>(matches: bool, empty: bool) -> Element<'a, Message> {
  let hint = text(t!("wallet.budget.reconcile_footer_hint").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::text::tertiary()));

  let cancel =
    Button::secondary(super::i18n::tr_static("wallet.budget.cancel")).on_press(Message::BudgetReconcileClosed);

  let confirm = if matches {
    Button::primary(super::i18n::tr_static("wallet.budget.reconcile_done")).on_press(Message::BudgetReconcileClosed)
  } else {
    Button::primary(super::i18n::tr_static("wallet.budget.reconcile_post"))
      .on_press_maybe((!empty).then_some(Message::BudgetReconcileCommitted))
  };

  container(
    Row::with_children(vec![
      container(hint).width(Length::Fill).into(),
      cancel.into(),
      confirm.into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: 16.0,
    right: 24.0,
    bottom: 20.0,
    left: 24.0,
  })
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
      super::i18n::tr_static("wallet.budget.ready_message_zero"),
    )
  } else if positive {
    (
      color::accent::PLASMA,
      color::with_alpha(color::accent::PLASMA, 0.10),
      color::with_alpha(color::accent::PLASMA, 0.3),
      super::i18n::tr_static("wallet.budget.ready_message_positive"),
    )
  } else {
    (
      color::status::DANGER,
      color::with_alpha(color::status::DANGER, 0.12),
      color::with_alpha(color::status::DANGER, 0.35),
      super::i18n::tr_static("wallet.budget.ready_message_negative"),
    )
  };

  let amount = Row::with_children(vec![
    text(crate::ui::format::fmt_isk_full(ready))
      .font(typography::body::MEDIUM)
      .size(30.0)
      .style(typography::colored(value_color))
      .into(),
    text(t!("wallet.budget.isk_suffix"))
      .font(typography::body::MEDIUM)
      .size(16.0)
      .style(typography::colored(color::text::secondary()))
      .into(),
  ])
  .align_y(Vertical::Bottom);

  let left = Column::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text(super::i18n::tr_static("wallet.budget.ready_to_assign"), None).into(),
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
  Button::primary(t!("wallet.budget.auto_assign"))
    .on_press(Message::BudgetAutoAssign)
    .into()
}

fn overspent_button<'a>(overspent: f64) -> Element<'a, Message> {
  let body = Column::with_children(vec![
    text(t!("wallet.budget.overspent"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::status::DANGER))
      .into(),
    text(crate::ui::format::fmt_isk(overspent))
      .font(typography::mono::MEDIUM)
      .size(typography::size::LG)
      .style(typography::colored(color::status::DANGER))
      .into(),
    text(t!("wallet.budget.click_to_cover"))
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
    return empty_table(super::i18n::tr_static("wallet.budget.loading"));
  };
  if view.groups.is_empty() {
    return empty_table(super::i18n::tr_static("wallet.budget.no_categories"));
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
    text(t!("wallet.budget.add_category"))
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
    text(t!("wallet.budget.new_category_group"))
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
    container(crate::ui::components::eyebrow::eyebrow_text(
      super::i18n::tr_static("wallet.budget.col_category"),
      None,
    ))
    .padding(Padding {
      top: 0.0,
      right: 0.0,
      bottom: 0.0,
      left: 10.0,
    })
    .width(Length::Fill)
    .into(),
    head_cell(super::i18n::tr_static("wallet.budget.col_assigned"), ASSIGNED_COL),
    head_cell(super::i18n::tr_static("wallet.budget.col_activity"), ACTIVITY_COL),
    head_cell(super::i18n::tr_static("wallet.budget.col_available"), AVAILABLE_COL),
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

pub(super) fn mono_caption<'a, M: 'a>(value: impl Into<String>, value_color: Color, size: f32) -> Element<'a, M> {
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

  let delete_label = if pending {
    t!("wallet.budget.delete_confirm").into_owned()
  } else {
    t!("wallet.budget.delete").into_owned()
  };
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

  mouse_area(row_button)
    .on_enter(Message::BudgetCategoryHovered(Some(category.id)))
    .on_exit(Message::BudgetCategoryHovered(None))
    .into()
}

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

pub(super) fn color_dot<'a, M: 'a>(tone: Option<&str>, size: f32) -> Element<'a, M> {
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
  if hovered {
    head.push(Space::new().width(Length::Fill).into());
    head.push(view_transactions_link(
      category.id,
      super::i18n::tr_static("wallet.budget.view_transactions"),
    ));
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
    text(t!("wallet.budget.due", label => label))
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

fn move_open_for(state: &State, category_id: i64, anchor: BudgetMoveAnchor) -> bool {
  state
    .budget_move()
    .is_some_and(|open| open.from_id == category_id && open.anchor == anchor)
}

fn move_amount_state(draft: &str) -> (f64, bool) {
  let amount = crate::ui::format::parse_isk(draft).round();
  (amount, amount > 0.0)
}

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
    text(t!("wallet.budget.amount_avail", amount => crate::ui::format::fmt_isk_full(available)))
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
    eyebrow_label(super::i18n::tr_static("wallet.budget.move_money_from")),
    source_label.into(),
    move_amount_field(draft, available, valid).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .padding(spacing::SPACE_3);

  let mut rows: Vec<Element<'a, Message>> = vec![
    eyebrow_label(super::i18n::tr_static("wallet.budget.move_to")),
    move_dest_row(
      super::i18n::tr_static("wallet.budget.ready_to_assign"),
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

fn move_all_prefill(available: f64) -> String {
  crate::ui::format::fmt_isk(available.max(0.0))
}

fn move_amount_field<'a>(draft: &str, available: f64, valid: bool) -> Row<'a, Message> {
  let input = text_input(super::i18n::tr_static("wallet.budget.amount_placeholder"), draft)
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

  let all = Button::secondary(t!("wallet.budget.move_all"))
    .mono()
    .size(Size::Sm)
    .on_press(Message::BudgetMoveAmountChanged(move_all_prefill(available)));

  Row::with_children(vec![input.into(), all.into()])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
}

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

pub(super) fn eyebrow_label<'a, M: 'a>(label: &'a str) -> Element<'a, M> {
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
      text(t!("wallet.budget.inspector_empty"))
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
      super::i18n::tr_static("wallet.budget.tab_detail"),
      None,
      active == budget::InspectorTab::Detail,
      budget::InspectorTab::Detail,
    ),
    inspector_tab_button(
      super::i18n::tr_static("wallet.budget.tab_automation"),
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
  let note = category
    .note
    .clone()
    .unwrap_or_else(|| t!("wallet.budget.no_note").into_owned());

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
    crate::ui::components::eyebrow::eyebrow_text(super::i18n::tr_static("wallet.budget.available"), None).into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Bottom);

  let mut body: Vec<Element<'a, Message>> = vec![
    Row::with_children(head).spacing(11.0).align_y(Vertical::Center).into(),
    available_row.into(),
  ];
  if !state.budget_edit_mode() {
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
      text(t!("wallet.budget.move_money"))
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

fn inspector_transactions_button<'a>(category_id: i64) -> Element<'a, Message> {
  button(
    Row::with_children(vec![
      Icon::journal().size(14.0).color(color::text::PRIMARY).render(),
      text(t!("wallet.budget.transactions"))
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
    TargetState::Over => (t!("wallet.budget.target_overspent").into_owned(), color::status::DANGER),
    TargetState::Met => (t!("wallet.budget.target_funded").into_owned(), color::status::ONLINE),
    TargetState::Under => (
      t!("wallet.budget.target_underfunded").into_owned(),
      color::status::WARNING,
    ),
  };

  let header = Row::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text(super::i18n::tr_static("wallet.budget.target"), None)
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
      t!("wallet.budget.to_go", amount => crate::ui::format::fmt_isk(status.needed)).into_owned(),
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
      super::i18n::tr_static("wallet.budget.rolled_over"),
      crate::ui::format::fmt_isk_full(category.carry),
      color::text::secondary(),
    ),
    breakdown_row(
      super::i18n::tr_static("wallet.budget.assigned"),
      crate::ui::format::fmt_isk_full(category.assigned),
      color::text::PRIMARY,
    ),
    breakdown_row(
      super::i18n::tr_static("wallet.budget.activity"),
      fmt_signed(category.activity),
      activity_color,
    ),
  ])
  .spacing(spacing::SPACE_2);

  let available_color = if status.state == TargetState::Over {
    color::status::DANGER
  } else {
    color::text::PRIMARY
  };
  let total = Row::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text(super::i18n::tr_static("wallet.budget.col_available"), None)
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
      crate::ui::components::eyebrow::eyebrow_text(super::i18n::tr_static("wallet.budget.this_month"), None).into(),
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
      super::i18n::tr_static("wallet.budget.quick_underfunded"),
      Some(t!("wallet.budget.quick_underfunded_hint").into_owned()),
      category.underfunded_assign(month),
    ));
  }
  suggestions.push(quick_assign_row(
    category.id,
    super::i18n::tr_static("wallet.budget.quick_assigned_last"),
    Some(prev),
    category.last_assigned,
  ));
  suggestions.push(quick_assign_row(
    category.id,
    super::i18n::tr_static("wallet.budget.quick_spent_last"),
    None,
    category.spent_last,
  ));
  suggestions.push(quick_assign_row(
    category.id,
    super::i18n::tr_static("wallet.budget.quick_average"),
    Some(t!("wallet.budget.quick_average_hint").into_owned()),
    category.avg_assigned,
  ));
  suggestions.push(quick_assign_row(
    category.id,
    super::i18n::tr_static("wallet.budget.quick_set_zero"),
    None,
    0.0,
  ));

  Column::with_children(vec![
    crate::ui::components::eyebrow::eyebrow_text(super::i18n::tr_static("wallet.budget.auto_assign_eyebrow"), None)
      .into(),
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
    editor_field(
      super::i18n::tr_static("wallet.budget.field_name"),
      text_field(&draft.name, "", Message::BudgetEditorNameChanged),
    ),
    editor_field(
      super::i18n::tr_static("wallet.budget.field_note"),
      text_field(
        &draft.note,
        super::i18n::tr_static("wallet.budget.field_note_placeholder"),
        Message::BudgetEditorNoteChanged,
      ),
    ),
    editor_field(
      super::i18n::tr_static("wallet.budget.field_colour"),
      tone_picker(draft.tone.as_deref()),
    ),
    crate::ui::components::rule::horizontal(),
    editor_field(
      super::i18n::tr_static("wallet.budget.field_target_type"),
      target_type_picker(draft.target_kind),
    ),
    editor_field(draft.target_kind.amount_label(), money_field(&draft.target_amount_text)),
  ];
  if draft.target_kind == TargetKind::GoalBy {
    children.push(editor_field(
      super::i18n::tr_static("wallet.budget.field_by_date"),
      text_field(
        &draft.by_date,
        super::i18n::tr_static("wallet.budget.field_by_date_placeholder"),
        Message::BudgetEditorByDateChanged,
      ),
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

pub(super) fn editor_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
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
  Button::primary(t!("wallet.budget.save_category"))
    .block()
    .on_press(Message::BudgetEditorCommitted)
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
    text(t!("wallet.budget.automation_intro_prefix"))
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
    text(t!("wallet.budget.automation_intro_suffix"))
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
  Button::primary(t!("wallet.budget.new_rule"))
    .icon(Icon::plus())
    .block()
    .on_press(Message::BudgetRuleNewOpened(category_id))
    .into()
}

fn automation_empty_state<'a>() -> Element<'a, Message> {
  Column::with_children(vec![
    Icon::budget().size(18.0).color(color::text::tertiary()).render(),
    container(
      text(t!("wallet.budget.automation_empty"))
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
    rule_delete_button(Message::BudgetRuleDeleted(rule.id())),
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

pub(super) fn rule_delete_button<'a, M: Clone + 'a>(on_press: M) -> Element<'a, M> {
  button(
    text("\u{2715}")
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary())),
  )
  .padding(5.0)
  .on_press(on_press)
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
pub(super) fn switch<'a, M: Clone + 'a>(on: bool, on_press: M) -> Element<'a, M> {
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

pub(super) fn count_pill<'a, M: 'a>(count: usize, enabled: bool) -> Element<'a, M> {
  let tint = if count > 0 && enabled {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };
  let label = if count == 1 {
    t!("wallet.budget.count_match_singular", count => count).into_owned()
  } else {
    t!("wallet.budget.count_match_plural", count => count).into_owned()
  };
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
      text(t!("wallet.budget.manage_all_rules"))
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
    let rules_phrase = if rule_count == 1 {
      t!("wallet.budget.global_rules_singular", count => rule_count).into_owned()
    } else {
      t!("wallet.budget.global_rules_plural", count => rule_count).into_owned()
    };
    let txns_phrase = if total_matched == 1 {
      t!("wallet.budget.global_txns_singular", count => total_matched).into_owned()
    } else {
      t!("wallet.budget.global_txns_plural", count => total_matched).into_owned()
    };
    let summary = t!(
      "wallet.budget.global_summary",
      rules => rules_phrase,
      transactions => txns_phrase
    )
    .into_owned();
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

pub(super) fn rule_display_name(state: &State, rule: &Rule) -> String {
  if !rule.name().is_empty() {
    return rule.name().clone();
  }
  let suggested = engine::suggest_name(
    rule,
    |token| Some(engine::humanize_ref_type(token)),
    |key| character_name(state, key),
  );
  if suggested.is_empty() {
    t!("wallet.budget.untitled_rule").into_owned()
  } else {
    suggested
  }
}

pub(super) fn character_name(state: &State, key: &str) -> Option<String> {
  let id = key.trim().parse::<i64>().ok()?;
  state
    .roster()
    .iter()
    .find(|pilot| pilot.id == id)
    .map(|pilot| pilot.name.clone())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::model::{MatchMode, RuleField, RuleOp};

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

  mod reconcile {
    use super::*;

    #[test]
    fn it_renders_the_matching_resting_state() {
      let mut state = state_with_budget();
      state.budget_reconcile = Some(crate::ui::format::fmt_isk_full(5_000.0));

      let _el: Element<'_, Message> = reconcile_modal(&state);
    }

    #[test]
    fn it_renders_an_under_counted_inflow_diff() {
      let mut state = state_with_budget();
      state.budget_reconcile = Some("7,000".to_owned());

      let _el: Element<'_, Message> = reconcile_modal(&state);
    }

    #[test]
    fn it_renders_an_over_counted_diff_with_ready_to_assign_intact() {
      let mut state = state_with_budget();
      state.budget_reconcile = Some("4,500".to_owned());

      let _el: Element<'_, Message> = reconcile_modal(&state);
    }

    #[test]
    fn it_renders_the_negative_ready_to_assign_warning() {
      let mut state = state_with_budget();
      state.budget_reconcile = Some("1,000".to_owned());

      let _el: Element<'_, Message> = reconcile_modal(&state);
    }

    #[test]
    fn it_renders_the_empty_input_state() {
      let mut state = state_with_budget();
      state.budget_reconcile = Some(String::new());

      let _el: Element<'_, Message> = reconcile_modal(&state);
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
}
