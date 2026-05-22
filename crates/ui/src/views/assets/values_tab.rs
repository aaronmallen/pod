//! Values tab — character × location value matrix, category breakdown, and top items.

use std::collections::HashMap;

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding, Theme,
  widget::{Space, column, container, image, row, scrollable, text},
};

use super::{CategoryValue, CharacterStructureCell, State, TopItem, cat_color_rgb};
use crate::{
  format,
  style::{
    color,
    typography::{body, mono},
  },
};

/// Messages produced by the values tab.
#[derive(Clone, Debug)]
pub enum Message {}

fn category_display_name(key: &str) -> &'static str {
  match key {
    "ship" => "Ships",
    "module" => "Modules",
    "drone" => "Drones",
    "charge" => "Charges",
    "implant" => "Implants",
    "blueprint" => "Blueprints",
    "material" => "Materials",
    "book" => "Skill Books",
    "commodity" => "Commodities",
    _ => "Other",
  }
}

fn category_color(key: &str) -> Color {
  let (r, g, b) = cat_color_rgb(key);
  Color::from_rgb(r, g, b)
}

fn hdr_cell(label: String, width: impl Into<Length> + Copy) -> Element<'static, Message> {
  container(
    text(label)
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(width)
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn matrix_total_hdr_cell() -> Element<'static, Message> {
  container(
    text("TOTAL")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn matrix_header_row(structures: &[String]) -> Element<'static, Message> {
  let mut header_cells: Vec<Element<'static, Message>> = vec![hdr_cell("CHARACTER".to_string(), 160.0)];
  for s in structures {
    let short = s.split(" · ").next().unwrap_or(s.as_str()).to_string();
    header_cells.push(hdr_cell(short, Length::Fixed(120.0)));
  }
  header_cells.push(matrix_total_hdr_cell());

  container(
    row(header_cells)
      .width(Length::Fill)
      .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn matrix_value_cell(v: f64, row_total: f64) -> Element<'static, Message> {
  let intensity = if row_total > 0.0 { (v / row_total) as f32 } else { 0.0 };
  let bg = if v > 0.0 {
    Some(Background::Color(Color::from_rgba(
      0.247,
      0.722,
      0.859,
      0.04 + 0.16 * intensity,
    )))
  } else {
    None
  };
  let label = if v == 0.0 {
    "\u{2014}".to_string()
  } else {
    format::fmt_isk(v)
  };
  let text_color = if v == 0.0 {
    color::text::TERTIARY
  } else {
    color::text::PRIMARY
  };

  container(
    text(label)
      .font(mono::REGULAR)
      .size(11.0)
      .style(move |_: &Theme| iced::widget::text::Style {
        color: Some(text_color),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(move |_| container::Style {
    background: bg,
    ..container::Style::default()
  })
  .into()
}

fn matrix_row_total_cell(row_total: f64) -> Element<'static, Message> {
  container(
    text(format::fmt_isk(row_total))
      .font(mono::MEDIUM)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn matrix_char_name_cell(char_name: &str) -> Element<'static, Message> {
  container(
    text(char_name.to_string())
      .font(body::MEDIUM)
      .size(13.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .width(160.0)
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .into()
}

fn matrix_char_data_row(
  cells: &[CharacterStructureCell],
  char_id: i64,
  char_name: &str,
  structures: &[String],
) -> Element<'static, Message> {
  let row_total: f64 = cells
    .iter()
    .filter(|c| c.character_id == char_id)
    .map(|c| c.value)
    .sum();

  let mut row_cells: Vec<Element<'static, Message>> = vec![matrix_char_name_cell(char_name)];
  for struct_name in structures {
    let v = cells
      .iter()
      .filter(|c| c.character_id == char_id && &c.structure_name == struct_name)
      .map(|c| c.value)
      .sum::<f64>();
    row_cells.push(matrix_value_cell(v, row_total));
  }
  row_cells.push(matrix_row_total_cell(row_total));

  container(
    row(row_cells)
      .width(Length::Fill)
      .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn totals_label_cell() -> Element<'static, Message> {
  container(
    text("COLUMN TOTAL")
      .font(mono::REGULAR)
      .size(9.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(160.0)
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn totals_col_cell(col_total: f64) -> Element<'static, Message> {
  container(
    text(format::fmt_isk(col_total))
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn totals_grand_total_cell(total_value: f64) -> Element<'static, Message> {
  container(
    text(format::fmt_isk(total_value))
      .font(mono::MEDIUM)
      .size(14.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .width(Length::Fixed(120.0))
  .padding(Padding {
    top: 12.0,
    bottom: 12.0,
    left: 18.0,
    right: 18.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn matrix_totals_row(
  cells: &[CharacterStructureCell],
  structures: &[String],
  total_value: f64,
) -> Element<'static, Message> {
  let mut totals_cells: Vec<Element<'static, Message>> = vec![totals_label_cell()];

  for struct_name in structures {
    let col_total: f64 = cells
      .iter()
      .filter(|c| &c.structure_name == struct_name)
      .map(|c| c.value)
      .sum();
    totals_cells.push(totals_col_cell(col_total));
  }
  totals_cells.push(totals_grand_total_cell(total_value));

  container(
    row(totals_cells)
      .width(Length::Fill)
      .align_y(iced::alignment::Vertical::Center),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::border::DEFAULT,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn matrix_title_row(n_chars: usize, n_structs: usize) -> Element<'static, Message> {
  container(
    row([
      text("Value · character × location")
        .font(body::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().width(12.0).into(),
      text(format!("{n_chars} char · {n_structs} loc"))
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    ])
    .align_y(iced::alignment::Vertical::Center),
  )
  .padding(Padding {
    top: 14.0,
    bottom: 14.0,
    left: 18.0,
    right: 18.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn sorted_structures(cells: &[CharacterStructureCell]) -> Vec<String> {
  let mut structures: Vec<String> = cells
    .iter()
    .map(|c| c.structure_name.clone())
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();
  structures.sort_by(|a, b| {
    let a_total: f64 = cells.iter().filter(|c| &c.structure_name == a).map(|c| c.value).sum();
    let b_total: f64 = cells.iter().filter(|c| &c.structure_name == b).map(|c| c.value).sum();
    b_total.partial_cmp(&a_total).unwrap_or(std::cmp::Ordering::Equal)
  });
  structures
}

fn sorted_characters(cells: &[CharacterStructureCell]) -> Vec<(i64, String)> {
  let mut characters: Vec<(i64, String)> = cells
    .iter()
    .map(|c| (c.character_id, c.character_name.clone()))
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();
  characters.sort_by_key(|(id, _)| *id);
  characters
}

fn matrix_panel(cells: &[CharacterStructureCell], total_value: f64) -> Element<'static, Message> {
  let structures = sorted_structures(cells);
  let characters = sorted_characters(cells);

  let n_chars = characters.len();
  let n_structs = structures.len();

  let header_row = matrix_header_row(&structures);

  let data_rows: Vec<Element<'static, Message>> = characters
    .iter()
    .map(|(char_id, char_name)| matrix_char_data_row(cells, *char_id, char_name, &structures))
    .collect();

  let totals_row = matrix_totals_row(cells, &structures, total_value);
  let title_row = matrix_title_row(n_chars, n_structs);

  let mut body_rows: Vec<Element<'static, Message>> = vec![header_row];
  body_rows.extend(data_rows);
  body_rows.push(totals_row);

  container(column([
    title_row,
    scrollable(column(body_rows).width(Length::Fill))
      .width(Length::Fill)
      .height(Length::Fill)
      .into(),
  ]))
  .width(Length::Fill)
  .height(Length::Fill)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn category_stacked_bar(cats: &[CategoryValue], total_value: f64) -> Element<'static, Message> {
  let mut bar_segments: Vec<Element<'static, Message>> = Vec::new();
  for c in cats {
    if c.value <= 0.0 || total_value <= 0.0 {
      continue;
    }
    let pct = (c.value / total_value * 100.0) as u16;
    let col = category_color(&c.category_name);
    bar_segments.push(
      container(Space::new().width(Length::Fill).height(10.0))
        .width(Length::FillPortion(pct.max(1)))
        .style(move |_| container::Style {
          background: Some(Background::Color(col)),
          ..container::Style::default()
        })
        .into(),
    );
  }

  container(row(bar_segments).width(Length::Fill).height(10.0))
    .width(Length::Fill)
    .height(10.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      border: Border {
        radius: 5.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .into()
}

fn category_legend_row(c: &CategoryValue) -> Element<'static, Message> {
  let col = category_color(&c.category_name);
  let display = category_display_name(&c.category_name);
  let isk = format::fmt_isk(c.value);
  let pct_str = format!("{:.1}%", c.pct * 100.0);

  row([
    container(Space::new().width(10.0).height(10.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(col)),
        border: Border {
          radius: 2.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into(),
    Space::new().width(10.0).into(),
    text(display)
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    text(isk)
      .font(mono::REGULAR)
      .size(11.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    Space::new().width(8.0).into(),
    text(pct_str)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .width(44.0)
      .into(),
  ])
  .align_y(iced::alignment::Vertical::Center)
  .into()
}

fn category_panel(cats: &[CategoryValue], total_value: f64) -> Element<'static, Message> {
  let stacked_bar = category_stacked_bar(cats, total_value);

  let legend_rows: Vec<Element<'static, Message>> =
    cats.iter().filter(|c| c.value > 0.0).map(category_legend_row).collect();

  container(
    column([
      text("By category")
        .font(body::MEDIUM)
        .size(14.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::PRIMARY),
        })
        .into(),
      Space::new().height(4.0).into(),
      text(format!("{} ISK total", format::fmt_isk_full(total_value)))
        .font(mono::REGULAR)
        .size(9.0)
        .style(|_: &Theme| iced::widget::text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
      Space::new().height(14.0).into(),
      stacked_bar,
      Space::new().height(14.0).into(),
      column(legend_rows).spacing(6.0).into(),
    ])
    .padding(Padding {
      top: 16.0,
      bottom: 16.0,
      left: 18.0,
      right: 18.0,
    }),
  )
  .width(360.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::border::SUBTLE,
      radius: 10.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn top_item_icon(item: &TopItem, icons: &HashMap<(i32, String), image::Handle>) -> Element<'static, Message> {
  let col = category_color(&item.category_name);
  if let Some(handle) = icons.get(&(item.type_id, "icon".to_string())) {
    container(
      image(handle.clone())
        .width(24.0)
        .height(24.0)
        .content_fit(ContentFit::Cover),
    )
    .width(24.0)
    .height(24.0)
    .style(|_| container::Style {
      border: Border {
        radius: 4.0.into(),
        ..Border::default()
      },
      ..container::Style::default()
    })
    .clip(true)
    .into()
  } else {
    container(Space::new().width(24.0).height(24.0))
      .style(move |_| container::Style {
        background: Some(Background::Color(Color::from_rgba(col.r, col.g, col.b, 0.18))),
        border: Border {
          radius: 4.0.into(),
          ..Border::default()
        },
        ..container::Style::default()
      })
      .into()
  }
}

fn top_item_name_col(type_name: String, group_label: String) -> Element<'static, Message> {
  column([
    text(type_name)
      .font(body::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(group_label)
      .font(mono::REGULAR)
      .size(10.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
  ])
  .width(Length::Fill)
  .into()
}

fn top_item_isk_cell(isk: String) -> Element<'static, Message> {
  text(isk)
    .font(mono::MEDIUM)
    .size(12.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::accent::PLASMA),
    })
    .into()
}

fn top_item_rank_cell(rank_str: String) -> Element<'static, Message> {
  text(rank_str)
    .font(mono::REGULAR)
    .size(9.0)
    .style(|_: &Theme| iced::widget::text::Style {
      color: Some(color::text::TERTIARY),
    })
    .width(18.0)
    .into()
}

fn top_item_row(
  rank: usize,
  item: &TopItem,
  icons: &HashMap<(i32, String), image::Handle>,
) -> Element<'static, Message> {
  let rank_str = format!("{:02}", rank + 1);
  let group_label = format!(
    "{} · ×{}",
    category_display_name(&item.category_name),
    format::fmt_count(item.total_quantity as u64)
  );

  container(
    row([
      top_item_rank_cell(rank_str),
      Space::new().width(4.0).into(),
      top_item_icon(item, icons),
      Space::new().width(10.0).into(),
      top_item_name_col(item.type_name.clone(), group_label),
      top_item_isk_cell(format::fmt_isk(item.value)),
    ])
    .align_y(iced::alignment::Vertical::Center)
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 18.0,
      right: 18.0,
    }),
  )
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into()
}

fn top_items_panel(items: &[TopItem], icons: &HashMap<(i32, String), image::Handle>) -> Element<'static, Message> {
  let title_row: Element<'static, Message> = container(text("Top items by value").font(body::MEDIUM).size(14.0).style(
    |_: &Theme| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    },
  ))
  .padding(Padding {
    top: 14.0,
    bottom: 14.0,
    left: 18.0,
    right: 18.0,
  })
  .width(Length::Fill)
  .style(|_| container::Style {
    border: Border {
      color: color::border::SUBTLE,
      width: 1.0,
      ..Border::default()
    },
    ..container::Style::default()
  })
  .into();

  let item_rows: Vec<Element<'static, Message>> = items
    .iter()
    .enumerate()
    .map(|(i, item)| top_item_row(i, item, icons))
    .collect();

  let mut all_rows: Vec<Element<'static, Message>> = vec![title_row];
  all_rows.extend(item_rows);

  container(column(all_rows))
    .width(360.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::border::SUBTLE,
        radius: 10.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn empty_panel() -> Element<'static, Message> {
  container(
    text("Loading asset values\u{2026}")
      .font(mono::REGULAR)
      .size(12.0)
      .style(|_: &Theme| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      }),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .center(Length::Fill)
  .into()
}

/// Builder for the values tab.
pub struct Component<'a> {
  state: &'a State,
}

impl<'a> Component<'a> {
  /// Creates a new values tab for the given state.
  pub fn new(state: &'a State) -> Self {
    Self {
      state,
    }
  }

  /// Renders the values tab into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let state = self.state;
    let Some(data) = &state.asset_values_data else {
      return scrollable(
        container(empty_panel())
          .padding(Padding {
            top: 20.0,
            bottom: 32.0,
            left: 28.0,
            right: 28.0,
          })
          .width(Length::Fill)
          .height(Length::Fill),
      )
      .height(Length::Fill)
      .into();
    };

    let matrix = matrix_panel(&data.character_structure_cells, data.total_value);
    let right_col: Element<'static, Message> = column([
      category_panel(&data.category_breakdown, data.total_value),
      Space::new().height(16.0).into(),
      top_items_panel(&data.top_items, &state.item_icons),
    ])
    .width(360.0)
    .into();

    scrollable(
      container(
        row([matrix, Space::new().width(20.0).into(), right_col])
          .width(Length::Fill)
          .align_y(iced::alignment::Vertical::Center),
      )
      .padding(Padding {
        top: 20.0,
        bottom: 32.0,
        left: 28.0,
        right: 28.0,
      })
      .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
  }
}
