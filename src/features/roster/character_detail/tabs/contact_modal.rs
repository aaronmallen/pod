use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, slider, text},
};

use super::super::Message;
use crate::{
  store::{
    images::{self, ImageKind, ImageState},
    model::{CharacterContact, CharacterContactLabel},
  },
  ui::{
    components::{
      avatar::Avatar,
      entity_search::{EntityKind, EntityRef, EntitySearch, SingleSelect},
      icon::Icon,
      toggle,
    },
    style::{color, radius, spacing, typography},
  },
};

pub const CONTACT_STANDINGS: [(f64, &str); 5] = [
  (-10.0, "roster.contact_modal.tier_terrible"),
  (-5.0, "roster.contact_modal.tier_bad"),
  (0.0, "roster.contact_modal.tier_neutral"),
  (5.0, "roster.contact_modal.tier_good"),
  (10.0, "roster.contact_modal.tier_excellent"),
];

const DIALOG_WIDTH: f32 = 480.0;
const ENTITY_AVATAR: f32 = 36.0;

#[derive(Debug)]
pub struct ContactModal {
  /// The character's in-game contact labels, sourced from the contacts sync (never created here, only assigned).
  catalog: Vec<CharacterContactLabel>,
  edit: bool,
  entity: Option<EntityRef>,
  exclude: Vec<String>,
  image: Option<ImageState>,
  labels: Vec<i64>,
  search: EntitySearch,
  standing: f64,
  watch: bool,
  watch_visible: bool,
}

impl ContactModal {
  pub fn add(exclude: Vec<String>, catalog: Vec<CharacterContactLabel>) -> Self {
    ContactModal {
      catalog,
      edit: false,
      entity: None,
      exclude,
      image: None,
      labels: Vec::new(),
      search: EntitySearch::default(),
      standing: 0.0,
      watch: false,
      watch_visible: true,
    }
  }

  pub fn edit(contact: &CharacterContact, catalog: Vec<CharacterContactLabel>) -> Self {
    let kind = entity_kind(contact.contact_type());
    let image = resolve_image(contact.contact_id(), kind);
    ContactModal {
      catalog,
      edit: true,
      entity: Some(EntityRef {
        id: contact.contact_id(),
        kind,
        name: contact.contact_name().clone(),
        portrait: None,
      }),
      exclude: Vec::new(),
      image,
      labels: serde_json::from_str(contact.label_ids()).unwrap_or_default(),
      search: EntitySearch::default(),
      standing: snap_standing(contact.standing()),
      watch: contact.is_watched(),
      watch_visible: true,
    }
  }

  pub fn accept_results(&mut self, generation: u64, results: Vec<EntityRef>) -> bool {
    self.search.accept_results(generation, results)
  }

  pub fn can_submit(&self) -> bool {
    self.entity.is_some()
  }

  pub fn entity(&self) -> Option<&EntityRef> {
    self.entity.as_ref()
  }

  pub fn is_character(&self) -> bool {
    self
      .entity
      .as_ref()
      .is_none_or(|entity| entity.kind == EntityKind::Character)
  }

  pub fn is_edit(&self) -> bool {
    self.edit
  }

  pub fn labels(&self) -> &[i64] {
    &self.labels
  }

  pub fn refresh_image(&mut self) {
    self.image = self
      .entity
      .as_ref()
      .and_then(|entity| resolve_image(entity.id, entity.kind));
  }

  pub fn set_entity(&mut self, entity: Option<EntityRef>) {
    self.image = entity.as_ref().and_then(|entity| resolve_image(entity.id, entity.kind));
    self.entity = entity;
    self.search.clear();
  }

  pub fn set_query(&mut self, query: String) -> u64 {
    self.search.set_query(query)
  }

  pub fn set_standing(&mut self, standing: f64) {
    self.standing = snap_standing(standing);
  }

  pub fn stale_key(&self) -> Option<(ImageKind, i64)> {
    self.image.as_ref().and_then(ImageState::stale_key)
  }

  pub fn standing(&self) -> f64 {
    self.standing
  }

  pub fn toggle_label(&mut self, label_id: i64) {
    if let Some(index) = self.labels.iter().position(|id| *id == label_id) {
      self.labels.remove(index);
    } else {
      self.labels.push(label_id);
    }
  }

  pub fn toggle_watch(&mut self) {
    if self.is_character() {
      self.watch = !self.watch;
    }
  }

  /// Always false for non-character entities — ESI only allows watchlisting characters.
  pub fn watch(&self) -> bool {
    self.is_character() && self.watch
  }

  pub fn without_watch(mut self) -> Self {
    self.watch = false;
    self.watch_visible = false;
    self
  }
}

#[derive(Clone, Debug)]
pub struct DeleteConfirm {
  pub contact: CharacterContact,
}

pub fn snap_standing(value: f64) -> f64 {
  CONTACT_STANDINGS
    .iter()
    .min_by(|(a, _), (b, _)| {
      (a - value)
        .abs()
        .partial_cmp(&(b - value).abs())
        .unwrap_or(std::cmp::Ordering::Equal)
    })
    .map_or(0.0, |(tier, _)| *tier)
}

fn entity_kind(contact_type: &str) -> EntityKind {
  match contact_type.to_ascii_lowercase().as_str() {
    "alliance" => EntityKind::Alliance,
    "corporation" => EntityKind::Corporation,
    _ => EntityKind::Character,
  }
}

fn image_kind(kind: EntityKind) -> Option<ImageKind> {
  match kind {
    EntityKind::Alliance => Some(ImageKind::AllianceLogo),
    EntityKind::Character => Some(ImageKind::CharacterPortrait),
    EntityKind::Corporation => Some(ImageKind::CorporationLogo),
    EntityKind::SolarSystem | EntityKind::Station => None,
  }
}

fn resolve_image(id: i64, kind: EntityKind) -> Option<ImageState> {
  image_kind(kind).map(|kind| images::resolve(&images::default_store(), kind, id))
}

fn tier_label(value: f64) -> &'static str {
  CONTACT_STANDINGS
    .iter()
    .find(|(tier, _)| (tier - value).abs() < f64::EPSILON)
    .map_or("roster.contact_modal.tier_neutral", |(_, label)| *label)
}

fn standing_color(value: f64) -> Color {
  if value >= 5.0 {
    color::status::ONLINE
  } else if value > 0.0 {
    color::with_alpha(color::status::ONLINE, 0.72)
  } else if value > -5.0 && value < 0.0 {
    color::with_alpha(color::status::DANGER, 0.72)
  } else if value <= -5.0 {
    color::status::DANGER
  } else {
    color::text::secondary()
  }
}

pub fn modal(state: &ContactModal) -> Element<'_, Message> {
  let title = if state.edit {
    t!("roster.contact_modal.title_edit")
  } else {
    t!("roster.contact_modal.title_add")
  };

  let header = container(
    Row::with_children(vec![
      Column::with_children(vec![
        eyebrow(&t!("roster.contact_modal.contact")),
        text(title)
          .font(typography::body::MEDIUM)
          .size(typography::size::LG)
          .style(|_| text::Style {
            color: Some(color::text::PRIMARY),
          })
          .into(),
      ])
      .spacing(2.0)
      .width(Length::Fill)
      .into(),
      close_button(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    bottom: 16.0,
    left: 18.0,
    right: 18.0,
    top: 16.0,
  });

  let mut fields = vec![entity_field(state), standing_field(state)];
  if !state.catalog.is_empty() {
    fields.push(labels_field(state));
  }
  if state.watch_visible {
    fields.push(watchlist_field(state));
  }

  let body = Column::with_children(fields)
    .spacing(spacing::SPACE_6 - 2.0)
    .width(Length::Fill);

  let card = container(
    Column::with_children(vec![
      header.into(),
      crate::ui::components::rule::horizontal(),
      container(body).width(Length::Fill).padding(18.0).into(),
      crate::ui::components::rule::horizontal(),
      footer(state),
    ])
    .width(Length::Fill),
  )
  .width(Length::Fixed(DIALOG_WIDTH))
  .clip(true)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::RAISED)),
    border: Border {
      color: color::rule_strong(),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  });

  container(card)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

pub fn delete_confirm(confirm: &DeleteConfirm) -> Element<'_, Message> {
  let name = confirm.contact.contact_name().clone();
  crate::ui::components::confirm_modal::confirm_modal(
    name.clone(),
    t!("roster.contact_modal.remove_title"),
    t!("roster.contact_modal.remove_body", name => name),
    t!("roster.contact_modal.remove"),
    Message::ContactDeleteConfirmed,
    Message::ContactDeleteCancelled,
  )
}

fn entity_field(state: &ContactModal) -> Element<'_, Message> {
  let label = if state.edit {
    t!("roster.contact_modal.character_locked")
  } else {
    t!("roster.contact_modal.find_entity")
  };

  let control: Element<'_, Message> = if state.edit {
    locked_entity(state.entity.as_ref(), state.image.as_ref())
  } else {
    SingleSelect::new(
      state.search.query(),
      state.entity.as_ref(),
      state.search.results(),
      Message::ContactEntityInput,
      Message::ContactEntityChanged,
    )
    .exclude(&state.exclude)
    .searching(state.search.searching())
    .view()
  };

  field(&t!("roster.contact_modal.entity"), Some(&label), control)
}

fn standing_field(state: &ContactModal) -> Element<'_, Message> {
  field(
    &t!("roster.contact_modal.standing"),
    None,
    standing_slider(state.standing),
  )
}

fn labels_field(state: &ContactModal) -> Element<'_, Message> {
  let mut chips = Row::new().spacing(7.0).width(Length::Fill);
  let mut wrapped = Column::new().spacing(7.0).width(Length::Fill);
  let mut count = 0;
  for label in &state.catalog {
    let label_id = label.label_id();
    let active = state.labels.contains(&label_id);
    chips = chips.push(label_chip(label_id, label.label_name(), active));
    count += 1;
    if count % 4 == 0 {
      wrapped = wrapped.push(chips);
      chips = Row::new().spacing(7.0).width(Length::Fill);
    }
  }
  wrapped = wrapped.push(chips);

  let heading = Row::with_children(vec![
    modal_label(&t!("roster.contact_modal.labels")),
    container(
      text(t!("roster.contact_modal.labels_hint"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::tertiary()),
        }),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Right)
    .into(),
  ])
  .align_y(Vertical::Center)
  .width(Length::Fill);

  Column::with_children(vec![heading.into(), wrapped.into()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn watchlist_field(state: &ContactModal) -> Element<'_, Message> {
  let is_char = state.is_character();
  let on = state.watch();

  let icon = Icon::star()
    .size(16.0)
    .color(if on {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    })
    .render();

  let sub = if is_char {
    t!("roster.contact_modal.watch_alerts")
  } else {
    t!("roster.contact_modal.watch_characters_only")
  };
  let primary_color = if is_char {
    color::text::PRIMARY
  } else {
    color::text::tertiary()
  };

  let text_block = Column::with_children(vec![
    text(t!("roster.contact_modal.watch_this"))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(primary_color),
      })
      .into(),
    text(sub)
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::tertiary()),
      })
      .into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  let switch: Element<'_, Message> = toggle::toggle(on, Message::ContactWatchToggled);

  let row = Row::with_children(vec![icon, text_block.into(), switch])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let mut control = button(container(row).width(Length::Fill).padding(Padding {
    bottom: 12.0,
    left: 14.0,
    right: 14.0,
    top: 12.0,
  }))
  .width(Length::Fill)
  .style(move |_, _| button::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..button::Style::default()
  });
  if is_char {
    control = control.on_press(Message::ContactWatchToggled);
  }

  field(&t!("roster.contact_modal.watchlist"), None, control.into())
}

fn footer(state: &ContactModal) -> Element<'_, Message> {
  let status = match state.entity.as_ref() {
    Some(_) => format!(
      "{} \u{b7} {}{:.1}",
      t!(tier_label(state.standing)),
      if state.standing >= 0.0 { "+" } else { "" },
      state.standing
    ),
    None => t!("roster.contact_modal.no_entity").into_owned(),
  };

  let submit_label = if state.edit {
    t!("roster.contact_modal.save_changes")
  } else {
    t!("roster.contact_modal.add_contact")
  };
  let enabled = state.can_submit();
  let mut submit = button(
    text(submit_label)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(if enabled {
          color::accent::PLASMA
        } else {
          color::text::tertiary()
        }),
      }),
  )
  .padding(Padding {
    bottom: 8.0,
    left: 16.0,
    right: 16.0,
    top: 8.0,
  })
  .style(move |_, status| {
    let hover = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hover.then_some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
      border: Border {
        color: if enabled {
          color::with_alpha(color::accent::PLASMA, 0.5)
        } else {
          color::rule()
        },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  });
  if enabled {
    submit = submit.on_press(Message::ContactModalSubmitted);
  }

  let cancel = button(
    text(t!("roster.contact_modal.cancel"))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      }),
  )
  .padding(Padding {
    bottom: 8.0,
    left: 14.0,
    right: 14.0,
    top: 8.0,
  })
  .on_press(Message::ContactModalClosed)
  .style(|_, _| button::Style {
    background: None,
    border: Border {
      color: color::rule(),
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  });

  container(
    Row::with_children(vec![
      container(
        text(status)
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          }),
      )
      .width(Length::Fill)
      .into(),
      cancel.into(),
      submit.into(),
    ])
    .spacing(spacing::SPACE_2_5)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    bottom: 14.0,
    left: 18.0,
    right: 18.0,
    top: 14.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    ..container::Style::default()
  })
  .into()
}

fn standing_slider(value: f64) -> Element<'static, Message> {
  let tint = standing_color(value);
  let readout = Row::with_children(vec![
    text(format!("{}{:.1}", if value >= 0.0 { "+" } else { "" }, value))
      .font(typography::mono::MEDIUM)
      .size(30.0)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
    text(t!(tier_label(value)).to_uppercase())
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .style(move |_| text::Style {
        color: Some(tint),
      })
      .into(),
  ])
  .spacing(spacing::SPACE_3)
  .align_y(Vertical::Bottom);

  let mut ticks = Row::new().spacing(spacing::UNIT).width(Length::Fill);
  let tier_count = CONTACT_STANDINGS.len();
  for (index, (tier, label)) in CONTACT_STANDINGS.iter().enumerate() {
    let active = (tier - value).abs() < f64::EPSILON;
    let align = if index == 0 {
      Horizontal::Left
    } else if index == tier_count - 1 {
      Horizontal::Right
    } else {
      Horizontal::Center
    };
    let label_color = if active {
      standing_color(*tier)
    } else {
      color::text::tertiary()
    };
    let tier_value = *tier;
    ticks = ticks.push(
      button(
        container(
          text(t!(*label).to_uppercase())
            .font(typography::mono::REGULAR)
            .size(typography::size::XS)
            .style(move |_| text::Style {
              color: Some(label_color),
            }),
        )
        .width(Length::Fill)
        .align_x(align),
      )
      .padding(0)
      .width(Length::Fill)
      .on_press(Message::ContactStandingChanged(tier_value))
      .style(|_, _| button::Style {
        background: None,
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      }),
    );
  }

  Column::with_children(vec![readout.into(), standing_bar(value), ticks.into()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn standing_bar(value: f64) -> Element<'static, Message> {
  let tint = standing_color(value);

  slider(-10.0..=10.0, value, |raw| {
    Message::ContactStandingChanged(snap_standing(raw))
  })
  .step(5.0)
  .height(6.0)
  .style(move |_, _| slider::Style {
    rail: slider::Rail {
      backgrounds: (
        Background::Color(tint),
        Background::Color(color::with_alpha(color::text::PRIMARY, 0.1)),
      ),
      width: 6.0,
      border: Border {
        radius: 3.0.into(),
        width: 0.0,
        color: iced::Color::TRANSPARENT,
      },
    },
    handle: slider::Handle {
      shape: slider::HandleShape::Circle {
        radius: 10.0,
      },
      background: Background::Color(tint),
      border_color: color::surface::BASE,
      border_width: 3.0,
    },
  })
  .into()
}

fn label_chip(label_id: i64, label_name: &str, active: bool) -> Element<'_, Message> {
  let (border, background, text_color) = if active {
    (
      color::with_alpha(color::accent::PLASMA, 0.45),
      color::with_alpha(color::accent::PLASMA, 0.12),
      color::accent::PLASMA,
    )
  } else {
    (color::rule(), Color::TRANSPARENT, color::text::secondary())
  };

  button(
    text(label_name.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(move |_| text::Style {
        color: Some(text_color),
      }),
  )
  .padding(Padding {
    bottom: 5.0,
    left: 10.0,
    right: 10.0,
    top: 5.0,
  })
  .on_press(Message::ContactLabelToggled(label_id))
  .style(move |_, _| button::Style {
    background: Some(Background::Color(background)),
    border: Border {
      color: border,
      radius: radius::CONTROL.into(),
      width: 1.0,
    },
    text_color,
    ..button::Style::default()
  })
  .into()
}

fn locked_entity<'a>(entity: Option<&'a EntityRef>, image: Option<&'a ImageState>) -> Element<'a, Message> {
  let Some(entity) = entity else {
    return Space::new().width(Length::Shrink).height(Length::Shrink).into();
  };

  let identity = Column::with_children(vec![
    text(entity.name.clone())
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    text(entity.kind.label().to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into(),
  ])
  .spacing(3.0)
  .width(Length::Fill);

  container(
    Row::with_children(vec![
      entity_avatar(entity, image),
      identity.into(),
      Icon::lock().size(14.0).color(color::text::tertiary()).render(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    bottom: 11.0,
    left: 12.0,
    right: 12.0,
    top: 11.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule(),
      radius: radius::CARD.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn entity_avatar<'a>(entity: &'a EntityRef, image: Option<&'a ImageState>) -> Element<'a, Message> {
  Avatar::new(
    entity.id,
    entity.name.clone(),
    Length::Fixed(ENTITY_AVATAR),
    ENTITY_AVATAR,
    image.and_then(ImageState::path),
  )
  .radius(entity.kind.avatar_radius())
  .view()
}

fn close_button() -> Element<'static, Message> {
  button(
    container(Icon::close().size(16.0).color(color::text::secondary()).render())
      .width(Length::Fixed(30.0))
      .height(Length::Fixed(30.0))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
  )
  .padding(0)
  .on_press(Message::ContactModalClosed)
  .style(|_, _| button::Style {
    background: None,
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  })
  .into()
}

fn field<'a>(label: &str, sub: Option<&str>, control: Element<'a, Message>) -> Element<'a, Message> {
  let label: Element<'a, Message> = match sub {
    Some(sub) => modal_label(sub),
    None => modal_label(label),
  };

  Column::with_children(vec![label, control])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill)
    .into()
}

fn modal_label<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into()
}

fn eyebrow<'a>(label: &str) -> Element<'a, Message> {
  text(label.to_uppercase())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn contact(kind: &str, standing: f64, watched: bool, label_ids: &str) -> CharacterContact {
    CharacterContact {
      character_id: 42,
      contact_id: 95_001,
      contact_name: "Test Pilot".to_owned(),
      contact_type: kind.to_owned(),
      is_blocked: false,
      is_watched: watched,
      label_ids: label_ids.to_owned(),
      standing,
    }
  }

  fn corp_entity() -> EntityRef {
    EntityRef {
      id: 98_001,
      kind: EntityKind::Corporation,
      name: "Test Corp".to_owned(),
      portrait: None,
    }
  }

  fn catalog() -> Vec<CharacterContactLabel> {
    vec![
      CharacterContactLabel {
        character_id: 42,
        label_id: 1,
        label_name: "Fleet".to_owned(),
      },
      CharacterContactLabel {
        character_id: 42,
        label_id: 2,
        label_name: "Trusted".to_owned(),
      },
    ]
  }

  mod contact_modal {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_forces_watch_off_for_a_non_character_entity() {
      let mut modal = ContactModal::add(Vec::new(), Vec::new());
      modal.set_entity(Some(corp_entity()));

      modal.toggle_watch();

      assert!(!modal.watch(), "a corporation cannot be watchlisted");
      assert!(!modal.is_character());
    }

    #[test]
    fn it_only_submits_with_an_entity_selected() {
      let mut modal = ContactModal::add(Vec::new(), Vec::new());

      assert!(!modal.can_submit());

      modal.set_entity(Some(corp_entity()));

      assert!(modal.can_submit());
    }

    #[test]
    fn it_clears_the_image_when_the_entity_is_cleared() {
      let mut modal = ContactModal::add(Vec::new(), Vec::new());
      modal.set_entity(Some(corp_entity()));

      modal.set_entity(None);

      assert_eq!(
        modal.stale_key(),
        None,
        "no entity selected leaves no portrait to fetch"
      );
    }

    #[test]
    fn it_resolves_a_portrait_image_for_an_edited_contact() {
      let modal = ContactModal::edit(&contact("corporation", -5.0, false, "[]"), catalog());

      assert_eq!(
        modal.stale_key(),
        Some((ImageKind::CorporationLogo, 95_001)),
        "an uncached corp logo surfaces as a stale fetch key instead of falling back to initials forever"
      );
    }

    #[test]
    fn it_resolves_a_portrait_image_for_a_picked_entity() {
      let mut modal = ContactModal::add(Vec::new(), Vec::new());

      modal.set_entity(Some(corp_entity()));

      assert_eq!(modal.stale_key(), Some((ImageKind::CorporationLogo, 98_001)));
    }

    #[test]
    fn it_seeds_an_edit_modal_from_a_contact_with_a_snapped_standing() {
      let modal = ContactModal::edit(&contact("character", 8.5, true, "[1,2]"), catalog());

      assert!(modal.is_edit());
      assert_eq!(modal.standing(), 10.0);
      assert_eq!(modal.labels(), &[1, 2]);
      assert!(modal.watch());
    }

    #[test]
    fn it_hides_and_clears_the_watch_flag_when_built_without_watch() {
      let modal = ContactModal::edit(&contact("character", 8.5, true, "[]"), Vec::new()).without_watch();

      assert!(!modal.watch());
      assert!(!modal.watch_visible);
    }

    #[test]
    fn it_toggles_label_membership() {
      let mut modal = ContactModal::add(Vec::new(), Vec::new());

      modal.toggle_label(3);
      modal.toggle_label(5);
      modal.toggle_label(3);

      assert_eq!(modal.labels(), &[5]);
    }
  }

  mod render {
    use super::*;

    #[test]
    fn it_renders_the_add_modal() {
      let modal = ContactModal::add(vec!["Existing".to_owned()], catalog());

      let _el: Element<'_, Message> = super::super::modal(&modal);
    }

    #[test]
    fn it_renders_the_delete_confirm() {
      let confirm = DeleteConfirm {
        contact: contact("character", 0.0, false, "[]"),
      };

      let _el: Element<'_, Message> = super::super::delete_confirm(&confirm);
    }

    #[test]
    fn it_renders_the_add_modal_without_the_watchlist_field() {
      let modal = ContactModal::add(Vec::new(), Vec::new()).without_watch();

      let _el: Element<'_, Message> = super::super::modal(&modal);
    }

    #[test]
    fn it_renders_the_edit_modal_with_a_locked_corporation_entity() {
      let modal = ContactModal::edit(&contact("corporation", -5.0, false, "[]"), catalog());

      let _el: Element<'_, Message> = super::super::modal(&modal);
    }
  }

  mod snap_standing {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_snaps_an_arbitrary_value_to_the_nearest_tier() {
      assert_eq!(snap_standing(7.0), 5.0);
      assert_eq!(snap_standing(-3.0), -5.0);
      assert_eq!(snap_standing(1.0), 0.0);
      assert_eq!(snap_standing(10.0), 10.0);
    }
  }
}
