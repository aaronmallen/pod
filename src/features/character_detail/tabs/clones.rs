use iced::{
  Border, ContentFit, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, container, image, text},
};

use super::{
  super::{LoadState, Message},
  shared,
};
use crate::{
  clients::eve_image::Size,
  store::{
    images::{self, IconResolution},
    model::{
      CharacterCloneImplant,
      character_clone_view::{CharacterClones, CloneWithImplants},
    },
  },
  ui::{
    components::{
      card,
      empty_state::{LoadStateView, empty_state, load_state_view},
      icon_tile::icon_tile,
      panel_header::panel_header,
      section_header::section_header,
    },
    style::{color, radius, spacing, typography},
  },
};

const IMPLANT_SLOTS: usize = 10;
const ICON_SIZE: Size = Size::S64;
const ICON_BOX: f32 = 32.0;

pub(in crate::features::character_detail) fn body(clones: &LoadState<Option<CharacterClones>>) -> Element<'_, Message> {
  let clones = match clones {
    LoadState::Loaded(Some(clones)) => clones,
    LoadState::Loaded(None) => return load_state_view(LoadStateView::Empty(empty_state("No clones synced yet"))),
    LoadState::Loading => return load_state_view(LoadStateView::Loading("Loading clones\u{2026}")),
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };

  Column::with_children(vec![active_section(&clones.active), jump_section(&clones.jump_clones)])
    .spacing(spacing::SPACE_6)
    .width(Length::Fill)
    .into()
}

fn active_section(active: &CloneWithImplants<crate::store::model::CharacterClone>) -> Element<'_, Message> {
  let clone = &active.clone;
  let title = clone
    .home_location_name()
    .clone()
    .unwrap_or_else(|| format!("Location {}", clone.home_location_id()));

  let header = panel_header(title, None, Some("active".to_owned()), true);
  let grid = implant_grid(&active.implants, 2);
  let card = card::panel(Column::with_children(vec![header, grid]).width(Length::Fill), true);

  Column::with_children(vec![section_header("Active clone", None), card])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn jump_section(jumps: &[CloneWithImplants<crate::store::model::CharacterJumpClone>]) -> Element<'_, Message> {
  let eyebrow = section_header("Jump clones", Some(&format!("{} installed", jumps.len())));

  if jumps.is_empty() {
    let card = card::panel(
      container(
        text("No jump clones installed")
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(|_| text::Style {
            color: Some(color::text::secondary()),
          }),
      )
      .width(Length::Fill)
      .padding(spacing::SPACE_3_5),
      false,
    );
    return Column::with_children(vec![eyebrow, card])
      .spacing(spacing::SPACE_2_5)
      .width(Length::Fill)
      .into();
  }

  let mut cards: Vec<Element<'_, Message>> = Vec::with_capacity(jumps.len());
  for jump in jumps {
    cards.push(half_width(jump_card(jump)));
  }

  Column::with_children(vec![
    eyebrow,
    Column::with_children(cards)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .width(Length::Fill)
  .into()
}

fn jump_card(jump: &CloneWithImplants<crate::store::model::CharacterJumpClone>) -> Element<'_, Message> {
  let clone = &jump.clone;
  let title = clone
    .name()
    .clone()
    .filter(|name| !name.is_empty())
    .or_else(|| clone.location_name().clone())
    .unwrap_or_else(|| format!("Location {}", clone.location_id()));
  let subtitle = clone.location_name().clone();

  let right = if jump.implants.is_empty() {
    "empty".to_owned()
  } else {
    format!("{} implants", jump.implants.len())
  };

  let header = panel_header(title, subtitle, Some(right), false);
  let grid = implant_grid(&jump.implants, 1);

  card::panel(Column::with_children(vec![header, grid]).width(Length::Fill), false)
}

fn half_width(card: Element<'_, Message>) -> Element<'_, Message> {
  Row::with_children(vec![
    container(card).width(Length::FillPortion(1)).into(),
    Space::new().width(Length::FillPortion(1)).into(),
  ])
  .width(Length::Fill)
  .into()
}

fn implant_grid<'a>(implants: &'a [CharacterCloneImplant], cols: usize) -> Element<'a, Message> {
  let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(IMPLANT_SLOTS);
  for slot in 0..IMPLANT_SLOTS {
    cells.push(implant_cell(slot + 1, implants.get(slot)));
  }

  let body: Element<'a, Message> = if cols >= 2 {
    let mid = IMPLANT_SLOTS.div_ceil(2);
    let mut right_cells = cells.split_off(mid);
    let left = Column::with_children(cells).width(Length::Fill);
    let right = Column::with_children(std::mem::take(&mut right_cells)).width(Length::Fill);
    Row::with_children(vec![left.into(), right.into()])
      .spacing(spacing::SPACE_6)
      .width(Length::Fill)
      .into()
  } else {
    Column::with_children(cells).width(Length::Fill).into()
  };

  container(body)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::UNIT,
      right: spacing::SPACE_2,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2,
    })
    .into()
}

fn implant_cell(slot: usize, implant: Option<&CharacterCloneImplant>) -> Element<'_, Message> {
  let index_color = if implant.is_some() {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };
  let index = text(format!("{slot:02}"))
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(move |_| text::Style {
      color: Some(index_color),
    });

  let icon: Element<'_, Message> = match implant {
    Some(implant) => implant_icon(implant.type_id()),
    None => empty_icon(),
  };

  let label: Element<'_, Message> = match implant {
    Some(implant) => text(implant.name().clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .width(Length::Fill)
      .into(),
    None => text("\u{2014} empty slot \u{2014}")
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::rule_strong()),
      })
      .width(Length::Fill)
      .into(),
  };

  container(
    Row::with_children(vec![container(index).width(Length::Fixed(22.0)).into(), icon, label])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2,
    right: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2,
  })
  .style(|_| shared::row_rule_style(1.0))
  .into()
}

fn implant_icon<'a>(type_id: i64) -> Element<'a, Message> {
  match images::default_store().resolve_type_icon(type_id, None, ICON_SIZE) {
    IconResolution::Found(path) => icon_tile(
      image(image::Handle::from_path(path))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Contain),
      ICON_BOX,
    ),
    IconResolution::Missing => empty_icon(),
  }
}

fn empty_icon<'a>() -> Element<'a, Message> {
  container(Space::new())
    .width(Length::Fixed(ICON_BOX))
    .height(Length::Fixed(ICON_BOX))
    .style(|_| container::Style {
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.12),
        width: 1.0,
        radius: radius::SUBTLE.into(),
      },
      ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::model::{CharacterClone, CharacterJumpClone};

  fn implant(type_id: i64, name: &str) -> CharacterCloneImplant {
    CharacterCloneImplant {
      character_id: 42,
      clone_id: None,
      icon: None,
      name: name.to_owned(),
      type_id,
    }
  }

  fn active_clone() -> CloneWithImplants<CharacterClone> {
    CloneWithImplants {
      clone: CharacterClone {
        character_id: 42,
        home_location_id: 60_003_760,
        home_location_name: Some("Jita IV - Moon 4".to_owned()),
        home_location_type: "station".to_owned(),
        last_clone_jump_date: None,
        last_station_change_date: None,
      },
      implants: vec![implant(9899, "Ocular Filter"), implant(9941, "Memory Augmentation")],
    }
  }

  fn jump_clone(id: i64, name: Option<&str>) -> CloneWithImplants<CharacterJumpClone> {
    CloneWithImplants {
      clone: CharacterJumpClone {
        character_id: 42,
        jump_clone_id: id,
        location_id: 60_008_494,
        location_name: Some("Amarr VIII".to_owned()),
        location_type: "station".to_owned(),
        name: name.map(str::to_owned),
      },
      implants: Vec::new(),
    }
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_active_and_jump_clones() {
      let loaded = LoadState::Loaded(Some(CharacterClones {
        active: active_clone(),
        jump_clones: vec![jump_clone(1, Some("Battle Clone")), jump_clone(2, None)],
      }));

      let _el: Element<'_, Message> = body(&loaded);
    }

    #[test]
    fn it_renders_with_no_jump_clones() {
      let loaded = LoadState::Loaded(Some(CharacterClones {
        active: active_clone(),
        jump_clones: Vec::new(),
      }));

      let _el: Element<'_, Message> = body(&loaded);
    }

    #[test]
    fn it_renders_the_awaiting_loading_and_error_states() {
      let none: LoadState<Option<CharacterClones>> = LoadState::Loaded(None);
      let loading: LoadState<Option<CharacterClones>> = LoadState::Loading;
      let error: LoadState<Option<CharacterClones>> = LoadState::Error("boom".to_owned());

      let _none: Element<'_, Message> = body(&none);
      let _loading: Element<'_, Message> = body(&loading);
      let _error: Element<'_, Message> = body(&error);
    }
  }
}
