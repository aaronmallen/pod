use std::sync::OnceLock;

use iced::{
  Background, Border, Color, ContentFit, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Space, column, container, image, row, text},
};

use super::{super::Message, fmt_sp, section_label};
use crate::{
  clients::eve_image::Size,
  features::skills::plan_math::InjectorEstimate,
  store::images::{self, IconResolution},
  ui::{
    format::fmt_sp_short,
    style::{color, spacing, typography},
  },
};

const LARGE_INJECTOR_TYPE_ID: i64 = 40_520;
const SMALL_INJECTOR_TYPE_ID: i64 = 45_635;
const INJECTOR_ICON_SIZE: Size = Size::S64;

pub(super) fn injector_section(estimate: InjectorEstimate, remaining_plan_sp: u64) -> Element<'static, Message> {
  let pills = row(vec![
    injector_pill(true, estimate.large, estimate.yield_per.large),
    Space::new().width(spacing::SPACE_2).into(),
    injector_pill(false, estimate.small, estimate.yield_per.small),
  ])
  .width(Length::Fill);

  let caption = text(t!(
    "skills.summary_injector.caption",
    sp => fmt_sp(remaining_plan_sp)
  ))
  .font(typography::mono::REGULAR)
  .size(typography::size::XS)
  .style(|_| text::Style {
    color: Some(color::text::tertiary()),
  });

  container(
    column(vec![
      section_label(&t!("skills.summary_injector.heading")),
      Space::new().height(spacing::SPACE_3).into(),
      pills.into(),
      Space::new().height(6.0).into(),
      caption.into(),
    ])
    .width(Length::Fill),
  )
  .padding(Padding {
    top: spacing::SPACE_3,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_3_5,
    right: spacing::SPACE_3_5,
  })
  .width(Length::Fill)
  .into()
}

fn injector_pill(is_large: bool, count: u64, yield_per: u64) -> Element<'static, Message> {
  let (tile_bg, tile_border, tile_fg) = if is_large {
    (
      color::with_alpha(color::status::WARNING, 0.15),
      color::with_alpha(color::status::WARNING, 0.35),
      color::status::WARNING,
    )
  } else {
    (
      color::with_alpha(color::accent(), 0.10),
      color::with_alpha(color::accent(), 0.30),
      color::accent(),
    )
  };

  let type_id = if is_large {
    LARGE_INJECTOR_TYPE_ID
  } else {
    SMALL_INJECTOR_TYPE_ID
  };

  let tile = injector_tile(type_id, is_large, tile_bg, tile_border, tile_fg);

  let count_row = row(vec![
    text(count.to_string())
      .font(typography::mono::MEDIUM)
      .size(18.0)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    Space::new().width(6.0).into(),
    text("\u{00d7}")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .align_y(Vertical::Bottom);

  let size = if is_large {
    t!("skills.summary_injector.large")
  } else {
    t!("skills.summary_injector.small")
  };
  let label = text(t!(
    "skills.summary_injector.pill_label",
    size => size,
    sp => fmt_sp_short(yield_per)
  ))
  .font(typography::mono::REGULAR)
  .size(typography::size::XS)
  .style(|_| text::Style {
    color: Some(color::text::tertiary()),
  });

  let body = column(vec![count_row.into(), Space::new().height(1.0).into(), label.into()]).width(Length::Fill);

  container(
    row(vec![tile, Space::new().width(spacing::SPACE_2_5).into(), body.into()])
      .align_y(Vertical::Center)
      .width(Length::Fill),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 12.0,
    right: 12.0,
  })
  .width(Length::FillPortion(1))
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(color::text::PRIMARY, 0.1),
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn injector_tile(
  type_id: i64,
  is_large: bool,
  tile_bg: Color,
  tile_border: Color,
  tile_fg: Color,
) -> Element<'static, Message> {
  match injector_icon(type_id) {
    IconResolution::Found(path) => container(
      image(image::Handle::from_path(path.clone()))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Contain),
    )
    .width(26.0)
    .height(26.0)
    .clip(true)
    .into(),
    IconResolution::Missing => letter_tile(is_large, tile_bg, tile_border, tile_fg),
  }
}

fn injector_icon(type_id: i64) -> &'static IconResolution {
  static LARGE: OnceLock<IconResolution> = OnceLock::new();
  static SMALL: OnceLock<IconResolution> = OnceLock::new();

  let cell = if type_id == LARGE_INJECTOR_TYPE_ID {
    &LARGE
  } else {
    &SMALL
  };
  cell.get_or_init(|| images::default_store().resolve_type_icon(type_id, None, INJECTOR_ICON_SIZE))
}

fn letter_tile(is_large: bool, tile_bg: Color, tile_border: Color, tile_fg: Color) -> Element<'static, Message> {
  container(
    text(if is_large { "L" } else { "S" })
      .font(typography::mono::MEDIUM)
      .size(typography::size::SM)
      .style(move |_| text::Style {
        color: Some(tile_fg),
      }),
  )
  .width(26.0)
  .height(26.0)
  .align_x(Horizontal::Center)
  .align_y(Vertical::Center)
  .style(move |_| container::Style {
    background: Some(Background::Color(tile_bg)),
    border: Border {
      color: tile_border,
      radius: 4.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod injector_pill {
    use super::*;

    #[test]
    fn it_falls_back_to_the_letter_tile_when_the_icon_is_missing() {
      let _large: Element<'static, Message> = injector_tile(
        LARGE_INJECTOR_TYPE_ID,
        true,
        color::status::WARNING,
        color::status::WARNING,
        color::status::WARNING,
      );
      let _small: Element<'static, Message> = injector_tile(
        SMALL_INJECTOR_TYPE_ID,
        false,
        color::accent(),
        color::accent(),
        color::accent(),
      );
    }

    #[test]
    fn it_renders_both_injector_pills_without_panicking() {
      let _el: Element<'static, Message> = injector_pill(true, 3, 500_000);
      let _el: Element<'static, Message> = injector_pill(false, 7, 30_000);
    }
  }
}
