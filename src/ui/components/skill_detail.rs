#![cfg_attr(not(test), allow(dead_code))]

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, text},
};

use crate::{
  features::skills::{
    browse::AttrKey,
    format::{fmt_dur_short, sp_cost, sp_per_sec},
    queue_timing::roman,
  },
  store::{
    Database, Error,
    repo::{
      sde::{get_item_group, get_item_type},
      skills::{get_skill_metadata, skill_prereqs},
    },
  },
  ui::{
    components::{
      button::{Button, Size},
      icon::Icon,
      modal_overlay::modal_layers,
    },
    format::fmt_sp,
    style::{color, radius, shadow, spacing, typography},
  },
};

const ACCENT_BAR_WIDTH: f32 = 4.0;
const BODY_GAP: f32 = 16.0;
const CALLOUT_BORDER_ALPHA: f32 = 0.22;
const CALLOUT_FILL_ALPHA: f32 = 0.06;
const CALLOUT_RADIUS: f32 = 8.0;
const CHIP_BORDER_ALPHA: f32 = 0.35;
const CHIP_FILL_ALPHA: f32 = 0.12;
const DEFAULT_PRIMARY_ATTR: i64 = 167;
const DEFAULT_SECONDARY_ATTR: i64 = 166;
const HEADER_PAD_X: f32 = 18.0;
const HEADER_PAD_Y: f32 = 16.0;
const LADDER_LEVEL_WIDTH: f32 = 44.0;
const LADDER_NEXT_FILL_ALPHA: f32 = 0.05;
const LADDER_NUMERIC_WIDTH: f32 = 88.0;
const LADDER_ROW_PAD_X: f32 = 14.0;
const LADDER_ROW_PAD_Y: f32 = 8.0;
const MODAL_WIDTH: f32 = 620.0;
const PANEL_HEADER_PAD_X: f32 = 14.0;
const PANEL_HEADER_PAD_Y: f32 = 10.0;
const PANEL_PAD: f32 = 14.0;
const PANEL_RADIUS: f32 = 10.0;
const PIP_GAP: f32 = 8.0;
const PIP_HEIGHT: f32 = 8.0;
const PIP_RADIUS: f32 = 1.5;
const PIP_WIDTH: f32 = 13.0;
const PREREQ_ROW_PAD_X: f32 = 14.0;
const PREREQ_ROW_PAD_Y: f32 = 9.0;
const SECONDARY_CHIP_FILL_ALPHA: f32 = 0.05;

pub struct SkillDetail {
  pub description: String,
  pub group_name: String,
  pub name: String,
  pub per_level: Option<String>,
  pub prereqs: Vec<(String, u8)>,
  pub primary_attr: AttrKey,
  pub rank: u8,
  pub secondary_attr: AttrKey,
  pub skill_id: i64,
  pub sp_rate: f64,
  pub trained_level: u8,
}

struct LadderRow {
  cost: u64,
  level: u8,
  seconds: i64,
  state: LadderState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LadderState {
  Locked,
  Next,
  Trained,
}

pub async fn skill_detail(
  db: &Database,
  skill_id: i64,
  trained_level: u8,
  effective_attrs: [u32; 5],
) -> Result<Option<SkillDetail>, Error> {
  let Some(item_type) = get_item_type(db, skill_id).await? else {
    return Ok(None);
  };

  let group_name = match get_item_group(db, item_type.group_id()).await? {
    Some(group) => group.name().clone(),
    None => String::new(),
  };
  let metadata = get_skill_metadata(db, skill_id).await?;
  let rank = metadata
    .as_ref()
    .map(|m| m.rank())
    .unwrap_or(1)
    .clamp(1, i64::from(u8::MAX)) as u8;
  let primary_attr = AttrKey::from_eve_id(
    metadata
      .as_ref()
      .map(|m| m.primary_attribute())
      .unwrap_or(DEFAULT_PRIMARY_ATTR) as u8,
  );
  let secondary_attr = AttrKey::from_eve_id(
    metadata
      .as_ref()
      .map(|m| m.secondary_attribute())
      .unwrap_or(DEFAULT_SECONDARY_ATTR) as u8,
  );
  let description = strip_html(item_type.description().as_deref().unwrap_or_default());
  let per_level = extract_per_level(&description);
  let prereqs = skill_prereqs(db, skill_id).await?;
  let sp_rate = sp_per_sec(
    effective_attrs[primary_attr as usize],
    effective_attrs[secondary_attr as usize],
  );

  Ok(Some(SkillDetail {
    description,
    group_name,
    name: item_type.name().clone(),
    per_level,
    prereqs,
    primary_attr,
    rank,
    secondary_attr,
    skill_id,
    sp_rate,
    trained_level,
  }))
}

pub fn skill_detail_modal<'a, M>(detail: &'a SkillDetail, on_close: M) -> Vec<Element<'a, M>>
where
  M: Clone + 'a,
{
  modal_layers(on_close.clone(), card(detail, on_close))
}

pub fn info_button<'a, M>(on_press: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  Button::ghost_icon(Icon::info())
    .size(Size::Sm)
    .on_press(on_press)
    .into()
}

fn card<'a, M>(detail: &'a SkillDetail, on_close: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  container(Column::with_children(vec![header(detail, on_close), body(detail)]).width(Length::Fill))
    .width(Length::Fixed(MODAL_WIDTH))
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      shadow: shadow::CARD,
      ..container::Style::default()
    })
    .into()
}

fn header<'a, M>(detail: &'a SkillDetail, on_close: M) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let eyebrow = text(detail.group_name.as_str())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::accent()),
    });

  let title = Row::with_children(vec![
    text(detail.name.as_str())
      .font(typography::body::MEDIUM)
      .size(typography::size::LG)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    rank_badge(detail.rank),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Bottom);

  let trained_label = if detail.trained_level > 0 {
    t!("skills.detail.trained_to", level => roman(i64::from(detail.trained_level))).into_owned()
  } else {
    t!("skills.detail.not_trained").into_owned()
  };
  let pips = Row::with_children(vec![
    pip_ladder(detail.trained_level),
    text(trained_label)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let content =
    container(Column::with_children(vec![eyebrow.into(), title.into(), pips.into()]).spacing(spacing::SPACE_2))
      .width(Length::Fill)
      .padding(Padding {
        top: HEADER_PAD_Y,
        right: HEADER_PAD_X,
        bottom: HEADER_PAD_Y,
        left: HEADER_PAD_X,
      });

  let close = container(Button::ghost_icon(Icon::close()).size(Size::Sm).on_press(on_close))
    .padding(spacing::SPACE_3)
    .align_y(Vertical::Top);

  let accent_bar = container(Space::new())
    .width(Length::Fixed(ACCENT_BAR_WIDTH))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::accent())),
      ..container::Style::default()
    });

  container(Row::with_children(vec![accent_bar.into(), content.into(), close.into()]).align_y(Vertical::Top))
    .width(Length::Fill)
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

fn body<'a, M>(detail: &'a SkillDetail) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let mut sections: Vec<Element<'a, M>> = vec![description_block(detail), facts_panel(detail)];
  if let Some(attributes) = attributes_panel(detail) {
    sections.push(attributes);
  }
  if let Some(prereqs) = prereqs_panel(detail) {
    sections.push(prereqs);
  }
  sections.push(ladder_panel(detail));

  container(Column::with_children(sections).spacing(BODY_GAP))
    .width(Length::Fill)
    .padding(spacing::SPACE_4_5)
    .into()
}

fn description_block<'a, M>(detail: &'a SkillDetail) -> Element<'a, M>
where
  M: 'a,
{
  let mut children: Vec<Element<'a, M>> = vec![
    text(detail.description.as_str())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];

  if let Some(effect) = &detail.per_level {
    children.push(callout(effect));
  }

  Column::with_children(children).spacing(spacing::SPACE_3).into()
}

fn callout<'a, M>(effect: &'a str) -> Element<'a, M>
where
  M: 'a,
{
  container(
    Row::with_children(vec![
      text(t!("skills.detail.per_level").into_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::accent()),
        })
        .into(),
      text(effect)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Top),
  )
  .padding(Padding {
    top: PANEL_HEADER_PAD_Y,
    right: spacing::SPACE_3,
    bottom: PANEL_HEADER_PAD_Y,
    left: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(
      color::accent(),
      CALLOUT_FILL_ALPHA,
    ))),
    border: Border {
      color: color::with_alpha(color::accent(), CALLOUT_BORDER_ALPHA),
      width: 1.0,
      radius: CALLOUT_RADIUS.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn facts_panel<'a, M>(detail: &'a SkillDetail) -> Element<'a, M>
where
  M: 'a,
{
  let prereq_count = detail.prereqs.len();
  let (prereq_value, prereq_color) = if prereq_count > 0 {
    (prereq_count.to_string(), color::status::WARNING)
  } else {
    (t!("skills.detail.none").into_owned(), color::text::tertiary())
  };

  let grid = Row::with_children(vec![
    fact(
      t!("skills.detail.rank").into_owned(),
      t!("skills.hero.rank", rank => detail.rank).into_owned(),
      color::text::PRIMARY,
    ),
    fact_divider(),
    fact(
      t!("skills.detail.full_v_total").into_owned(),
      t!("skills.detail.full_v_value", sp => fmt_sp(total_sp_to_five(detail.rank) as i64)).into_owned(),
      color::text::PRIMARY,
    ),
    fact_divider(),
    fact(
      t!("skills.detail.prerequisites").into_owned(),
      prereq_value,
      prereq_color,
    ),
  ]);

  panel(t!("skills.detail.training").into_owned(), None, grid.into(), false)
}

fn fact<'a, M>(label: String, value: String, value_color: Color) -> Element<'a, M>
where
  M: 'a,
{
  container(
    Column::with_children(vec![
      text(label.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
      text(value)
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(move |_| text::Style {
          color: Some(value_color),
        })
        .into(),
    ])
    .spacing(spacing::UNIT),
  )
  .width(Length::FillPortion(1))
  .padding(Padding {
    top: PREREQ_ROW_PAD_Y,
    right: spacing::SPACE_3,
    bottom: PREREQ_ROW_PAD_Y,
    left: spacing::SPACE_3,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    ..container::Style::default()
  })
  .into()
}

fn fact_divider<'a, M>() -> Element<'a, M>
where
  M: 'a,
{
  container(Space::new())
    .width(Length::Fixed(1.0))
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
}

fn attributes_panel<'a, M>(detail: &'a SkillDetail) -> Option<Element<'a, M>>
where
  M: 'a,
{
  if detail.primary_attr == detail.secondary_attr {
    return None;
  }

  let hint = t!(
    "skills.detail.attr_hint_two",
    primary => detail.primary_attr.short(),
    secondary => detail.secondary_attr.short()
  )
  .into_owned();

  let row = Row::with_children(vec![
    attr_chip(detail.primary_attr.short(), true),
    attr_chip(detail.secondary_attr.short(), false),
    text(hint)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  Some(panel(
    t!("skills.detail.trains_on").into_owned(),
    None,
    row.into(),
    true,
  ))
}

fn prereqs_panel<'a, M>(detail: &'a SkillDetail) -> Option<Element<'a, M>>
where
  M: 'a,
{
  if detail.prereqs.is_empty() {
    return None;
  }

  let rows = detail.prereqs.iter().enumerate().map(|(index, (name, level))| {
    let row = Row::with_children(vec![
      text("\u{2937}")
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::status::WARNING),
        })
        .into(),
      text(name.as_str())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(|_| text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      text(roman(i64::from(*level)))
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        })
        .into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center);

    divided_row(row.into(), index == 0)
  });

  Some(panel(
    t!("skills.detail.prerequisites").into_owned(),
    None,
    Column::with_children(rows).into(),
    false,
  ))
}

fn ladder_panel<'a, M>(detail: &'a SkillDetail) -> Element<'a, M>
where
  M: 'a,
{
  let head = Row::with_children(vec![
    ladder_head(
      t!("skills.detail.col_level").into_owned(),
      Length::Fixed(LADDER_LEVEL_WIDTH),
      false,
    ),
    ladder_head(t!("skills.detail.col_status").into_owned(), Length::Fill, false),
    ladder_head(
      t!("skills.detail.col_sp").into_owned(),
      Length::Fixed(LADDER_NUMERIC_WIDTH),
      true,
    ),
    ladder_head(
      t!("skills.detail.col_time").into_owned(),
      Length::Fixed(LADDER_NUMERIC_WIDTH),
      true,
    ),
  ])
  .spacing(spacing::SPACE_2_5);

  let header_row = container(head)
    .width(Length::Fill)
    .padding(Padding {
      top: LADDER_ROW_PAD_Y,
      right: LADDER_ROW_PAD_X,
      bottom: LADDER_ROW_PAD_Y,
      left: LADDER_ROW_PAD_X,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  let mut rows: Vec<Element<'a, M>> = vec![header_row.into()];
  for row in ladder_rows(detail.rank, detail.trained_level, detail.sp_rate) {
    rows.push(ladder_row(&row));
  }

  panel(
    t!("skills.detail.ladder_label").into_owned(),
    Some(t!("skills.detail.ladder_right").into_owned()),
    Column::with_children(rows).into(),
    false,
  )
}

fn ladder_head<'a, M>(label: String, width: Length, align_right: bool) -> Element<'a, M>
where
  M: 'a,
{
  let mut label = text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .width(width)
    .style(|_| text::Style {
      color: Some(color::text::tertiary()),
    });
  if align_right {
    label = label.align_x(iced::alignment::Horizontal::Right);
  }
  label.into()
}

fn ladder_row<'a, M>(row: &LadderRow) -> Element<'a, M>
where
  M: 'a,
{
  let (status_key, status_color) = match row.state {
    LadderState::Locked => ("skills.detail.status_locked", color::text::tertiary()),
    LadderState::Next => ("skills.detail.status_next", color::accent()),
    LadderState::Trained => ("skills.detail.status_trained", color::status::ONLINE),
  };
  let numeral_color = match row.state {
    LadderState::Trained => color::text::PRIMARY,
    LadderState::Next => color::accent(),
    LadderState::Locked => color::text::secondary(),
  };
  let sp_color = if row.state == LadderState::Trained {
    color::text::tertiary()
  } else {
    color::text::PRIMARY
  };
  let time_value = if row.state == LadderState::Trained {
    "\u{2014}".to_owned()
  } else {
    fmt_dur_short(row.seconds)
  };

  let cells = Row::with_children(vec![
    text(roman(i64::from(row.level)))
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .width(Length::Fixed(LADDER_LEVEL_WIDTH))
      .style(move |_| text::Style {
        color: Some(numeral_color),
      })
      .into(),
    text(t!(status_key).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .width(Length::Fill)
      .style(move |_| text::Style {
        color: Some(status_color),
      })
      .into(),
    text(fmt_sp(row.cost as i64))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .width(Length::Fixed(LADDER_NUMERIC_WIDTH))
      .align_x(iced::alignment::Horizontal::Right)
      .style(move |_| text::Style {
        color: Some(sp_color),
      })
      .into(),
    text(time_value)
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .width(Length::Fixed(LADDER_NUMERIC_WIDTH))
      .align_x(iced::alignment::Horizontal::Right)
      .style(move |_| text::Style {
        color: Some(sp_color),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .align_y(Vertical::Center);

  let fill = if row.state == LadderState::Next {
    Some(Background::Color(color::with_alpha(
      color::accent(),
      LADDER_NEXT_FILL_ALPHA,
    )))
  } else {
    None
  };

  container(cells)
    .width(Length::Fill)
    .padding(Padding {
      top: LADDER_ROW_PAD_Y,
      right: LADDER_ROW_PAD_X,
      bottom: LADDER_ROW_PAD_Y,
      left: LADDER_ROW_PAD_X,
    })
    .style(move |_| container::Style {
      background: fill,
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn divided_row<'a, M>(row: Element<'a, M>, first: bool) -> Element<'a, M>
where
  M: 'a,
{
  let border = if first {
    Border::default()
  } else {
    Border {
      color: color::rule(),
      width: 1.0,
      radius: 0.0.into(),
    }
  };

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: PREREQ_ROW_PAD_Y,
      right: PREREQ_ROW_PAD_X,
      bottom: PREREQ_ROW_PAD_Y,
      left: PREREQ_ROW_PAD_X,
    })
    .style(move |_| container::Style {
      border,
      ..container::Style::default()
    })
    .into()
}

fn panel<'a, M>(label: String, right: Option<String>, body: Element<'a, M>, pad: bool) -> Element<'a, M>
where
  M: 'a,
{
  let mut head_children: Vec<Element<'a, M>> = vec![
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
    Space::new().width(Length::Fill).into(),
  ];
  if let Some(right) = right {
    head_children.push(
      text(right.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        })
        .into(),
    );
  }

  let head = container(Row::with_children(head_children).align_y(Vertical::Center))
    .width(Length::Fill)
    .padding(Padding {
      top: PANEL_HEADER_PAD_Y,
      right: PANEL_HEADER_PAD_X,
      bottom: PANEL_HEADER_PAD_Y,
      left: PANEL_HEADER_PAD_X,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: 0.0.into(),
      },
      ..container::Style::default()
    });

  let body = if pad {
    container(body).padding(PANEL_PAD).width(Length::Fill)
  } else {
    container(body).width(Length::Fill)
  };

  container(Column::with_children(vec![head.into(), body.into()]).width(Length::Fill))
    .width(Length::Fill)
    .clip(true)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        width: 1.0,
        radius: PANEL_RADIUS.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn pip_ladder<'a, M>(current: u8) -> Element<'a, M>
where
  M: 'a,
{
  let cells = (1..=5u8).map(|i| {
    let (fill, border) = if i <= current {
      (color::text::PRIMARY, color::text::PRIMARY)
    } else {
      (Color::TRANSPARENT, color::rule())
    };
    container(Space::new())
      .width(Length::Fixed(PIP_WIDTH))
      .height(Length::Fixed(PIP_HEIGHT))
      .style(move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
          color: border,
          width: 1.0,
          radius: PIP_RADIUS.into(),
        },
        ..container::Style::default()
      })
      .into()
  });

  Row::with_children(cells).spacing(PIP_GAP).into()
}

fn rank_badge<'a, M>(rank: u8) -> Element<'a, M>
where
  M: 'a,
{
  container(
    text(t!("skills.hero.rank", rank => rank).into_owned())
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    top: 2.0,
    right: 7.0,
    bottom: 2.0,
    left: 7.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::rule(),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn attr_chip<'a, M>(code: &'static str, primary: bool) -> Element<'a, M>
where
  M: 'a,
{
  let (fill, fg, border) = if primary {
    (
      color::with_alpha(color::accent(), CHIP_FILL_ALPHA),
      color::accent(),
      color::with_alpha(color::accent(), CHIP_BORDER_ALPHA),
    )
  } else {
    (
      color::with_alpha(color::text::PRIMARY, SECONDARY_CHIP_FILL_ALPHA),
      color::text::secondary(),
      color::rule(),
    )
  };

  let label = if primary {
    t!("skills.detail.chip_primary", code => code).into_owned()
  } else {
    t!("skills.detail.chip_secondary", code => code).into_owned()
  };

  container(
    text(label)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(fg),
      }),
  )
  .padding(Padding {
    top: 2.0,
    right: 7.0,
    bottom: 2.0,
    left: 7.0,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(fill)),
    border: Border {
      color: border,
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn strip_html(raw: &str) -> String {
  let mut out = String::with_capacity(raw.len());
  let mut in_tag = false;
  for ch in raw.chars() {
    match ch {
      '<' => in_tag = true,
      '>' => in_tag = false,
      _ if !in_tag => out.push(ch),
      _ => {}
    }
  }

  out
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&quot;", "\"")
    .replace("&#39;", "'")
    .replace("&nbsp;", " ")
    .replace("&amp;", "&")
    .trim()
    .to_owned()
}

fn extract_per_level(desc: &str) -> Option<String> {
  split_sentences(desc).into_iter().find_map(|sentence| {
    let lower = sentence.to_lowercase();
    if lower.contains("per level") || lower.contains("per skill level") {
      let trimmed = sentence.trim();
      (!trimmed.is_empty()).then(|| trimmed.to_owned())
    } else {
      None
    }
  })
}

fn split_sentences(text: &str) -> Vec<String> {
  let mut sentences = Vec::new();
  let mut current = String::new();
  for ch in text.chars() {
    current.push(ch);
    if matches!(ch, '.' | '!' | '\n') {
      sentences.push(std::mem::take(&mut current));
    }
  }
  if !current.trim().is_empty() {
    sentences.push(current);
  }

  sentences
}

fn ladder_rows(rank: u8, trained_level: u8, sp_rate: f64) -> Vec<LadderRow> {
  (1..=5u8)
    .map(|level| {
      let cost = sp_cost(f64::from(rank), level);
      let state = if level <= trained_level {
        LadderState::Trained
      } else if level == trained_level + 1 {
        LadderState::Next
      } else {
        LadderState::Locked
      };
      let seconds = if sp_rate > 0.0 {
        (cost as f64 / sp_rate).round() as i64
      } else {
        0
      };
      LadderRow {
        cost,
        level,
        seconds,
        state,
      }
    })
    .collect()
}

fn total_sp_to_five(rank: u8) -> u64 {
  (1..=5u8).map(|level| sp_cost(f64::from(rank), level)).sum()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod strip_html {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_removes_tags_and_decodes_entities() {
      assert_eq!(
        strip_html("Advanced <a href=\"x\">Broker</a> Relations &amp; more"),
        "Advanced Broker Relations & more"
      );
    }

    #[test]
    fn it_leaves_a_plain_description_untouched() {
      assert_eq!(strip_html("A simple skill description."), "A simple skill description.");
    }

    #[test]
    fn it_decodes_angle_bracket_entities() {
      assert_eq!(strip_html("compare &lt;x&gt; here"), "compare <x> here");
    }
  }

  mod extract_per_level {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_surfaces_a_per_level_bonus_sentence() {
      let desc = "Skill at operating shields. Grants a 5% bonus to shield capacity per level.";

      assert_eq!(
        extract_per_level(desc),
        Some("Grants a 5% bonus to shield capacity per level.".to_owned())
      );
    }

    #[test]
    fn it_matches_a_per_skill_level_phrasing() {
      let desc = "Improves targeting. +5% targeting speed per skill level.";

      assert_eq!(
        extract_per_level(desc),
        Some("+5% targeting speed per skill level.".to_owned())
      );
    }

    #[test]
    fn it_returns_none_when_no_per_level_sentence_exists() {
      let desc = "The foundational skill required to command frigate-class vessels.";

      assert_eq!(extract_per_level(desc), None);
    }
  }

  mod ladder_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_marks_trained_up_next_and_locked_states() {
      let rows = ladder_rows(1, 2, 60.0);

      assert_eq!(rows[0].state, LadderState::Trained);
      assert_eq!(rows[1].state, LadderState::Trained);
      assert_eq!(rows[2].state, LadderState::Next);
      assert_eq!(rows[3].state, LadderState::Locked);
      assert_eq!(rows[4].state, LadderState::Locked);
    }

    #[test]
    fn it_uses_the_sp_curve_scaled_by_rank_for_each_level() {
      let rows = ladder_rows(2, 0, 60.0);

      assert_eq!(rows[0].cost, sp_cost(2.0, 1));
      assert_eq!(rows[4].cost, sp_cost(2.0, 5));
    }

    #[test]
    fn it_derives_seconds_from_cost_over_rate() {
      let rows = ladder_rows(1, 0, 60.0);

      assert_eq!(rows[0].seconds, (sp_cost(1.0, 1) as f64 / 60.0).round() as i64);
    }

    #[test]
    fn it_yields_zero_seconds_when_the_rate_is_non_positive() {
      let rows = ladder_rows(1, 0, 0.0);

      assert_eq!(rows[0].seconds, 0);
    }
  }

  mod total_sp_to_five {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_sums_the_full_ladder_at_rank_one() {
      assert_eq!(total_sp_to_five(1), 250 + 1_414 + 8_000 + 45_255 + 256_000);
    }

    #[test]
    fn it_scales_with_rank() {
      assert_eq!(total_sp_to_five(2), total_sp_to_five(1) * 2);
    }
  }

  mod skill_detail {
    use super::*;
    use crate::store::{
      self,
      model::{ItemCategory, ItemGroup, ItemType, SkillMetadata},
      repo::{
        sde::{upsert_item_category, upsert_item_group, upsert_item_type},
        skills::upsert_skill_metadata,
      },
    };

    const ATTRS: [u32; 5] = [27, 21, 24, 20, 19];

    async fn seed_skill(db: &Database, description: &str) {
      upsert_item_category(
        db,
        &ItemCategory {
          id: 16,
          icon_id: None,
          name: "Skill".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      upsert_item_group(
        db,
        &ItemGroup {
          category_id: 16,
          icon_id: None,
          id: 255,
          name: "Gunnery".to_owned(),
          published: true,
        },
      )
      .await
      .unwrap();
      upsert_item_type(
        db,
        &ItemType {
          capacity: None,
          description: Some(description.to_owned()),
          dogma_attributes: "[]".to_owned(),
          group_id: 255,
          icon_id: None,
          id: 3300,
          market_group_id: None,
          name: "Gunnery".to_owned(),
          packaged_volume: None,
          portion_size: None,
          published: true,
          radius: None,
          volume: None,
        },
      )
      .await
      .unwrap();
      upsert_skill_metadata(
        db,
        &SkillMetadata {
          primary_attribute: 167,
          rank: 2,
          secondary_attribute: 166,
          skill_id: 3300,
        },
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unknown_skill_id() {
      let db = store::open_test().await.unwrap();

      assert!(skill_detail(&db, 999, 0, ATTRS).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn it_populates_a_detail_from_the_local_db() {
      let db = store::open_test().await.unwrap();
      seed_skill(&db, "Gunnery mastery. +2% turret rate of fire per level.").await;

      let detail = skill_detail(&db, 3300, 3, ATTRS).await.unwrap().unwrap();

      assert_eq!(detail.skill_id, 3300);
      assert_eq!(detail.name, "Gunnery");
      assert_eq!(detail.group_name, "Gunnery");
      assert_eq!(detail.rank, 2);
      assert_eq!(detail.trained_level, 3);
      assert_eq!(detail.primary_attr, AttrKey::Perception);
      assert_eq!(detail.secondary_attr, AttrKey::Memory);
      assert_eq!(detail.per_level.as_deref(), Some("+2% turret rate of fire per level."));
      assert!(detail.sp_rate > 0.0);
    }

    #[tokio::test]
    async fn it_omits_the_callout_when_no_per_level_sentence_is_present() {
      let db = store::open_test().await.unwrap();
      seed_skill(&db, "A foundational discipline with no per-line bonus copy here.").await;

      let detail = skill_detail(&db, 3300, 0, ATTRS).await.unwrap().unwrap();

      assert_eq!(detail.per_level, None);
    }
  }

  mod render {
    use super::*;

    fn detail() -> SkillDetail {
      SkillDetail {
        description: "A turret discipline.".to_owned(),
        group_name: "Gunnery".to_owned(),
        name: "Gunnery".to_owned(),
        per_level: Some("+2% turret rate of fire per level.".to_owned()),
        prereqs: vec![("Spaceship Command".to_owned(), 3)],
        primary_attr: AttrKey::Perception,
        rank: 2,
        secondary_attr: AttrKey::Memory,
        skill_id: 3300,
        sp_rate: 0.65,
        trained_level: 2,
      }
    }

    #[test]
    fn it_builds_the_modal_layers() {
      let detail = detail();
      let layers = skill_detail_modal::<()>(&detail, ());

      assert_eq!(layers.len(), 2);
    }

    #[test]
    fn it_builds_an_info_button() {
      let _el: Element<'_, ()> = info_button(());
    }

    #[test]
    fn it_builds_a_card_for_an_untrained_skill_without_prereqs_or_callout() {
      let mut detail = detail();
      detail.per_level = None;
      detail.prereqs = Vec::new();
      detail.trained_level = 0;

      let _layers = skill_detail_modal::<()>(&detail, ());
    }
  }
}
