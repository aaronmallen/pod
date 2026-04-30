//! Corporation card displayed in the Corporations tab grid.
//!
//! Layout: 140 px ticker plate (logo + big ticker + alliance) → name + alliance
//! row → members / tax stats.

use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{button, column, container, image, mouse_area, row, text},
};
use pod_model::Corporation;

use crate::{
  components,
  style::{color, radius, spacing, typography},
};

/// Messages emitted by a corporation card.
#[derive(Clone, Debug)]
pub enum Message {
  /// The card was right-clicked; carries corporation id and name.
  CardRightPressed(i64, String),
  /// The "+" tag button was pressed; carries corporation id.
  TagsPressed(i64),
}

/// Builder for a single corporation card element.
pub struct Component<'a> {
  ceo_name: Option<String>,
  corporation: &'a Corporation,
  icon_handle: Option<&'a iced::widget::image::Handle>,
}

impl<'a> Component<'a> {
  /// Creates a new corporation card for the given corporation.
  pub fn new(corporation: &'a Corporation) -> Self {
    Self {
      ceo_name: None,
      corporation,
      icon_handle: None,
    }
  }

  /// Sets the resolved CEO name to display on the card.
  pub fn ceo_name(mut self, name: Option<String>) -> Self {
    self.ceo_name = name;
    self
  }

  /// Provides a cached image handle for the corporation icon.
  pub fn icon_handle(mut self, handle: Option<&'a iced::widget::image::Handle>) -> Self {
    self.icon_handle = handle;
    self
  }

  /// Renders the corporation card into an iced element.
  pub fn render(self) -> Element<'a, Message> {
    let id = *self.corporation.id();
    let name = self.corporation.name().clone();
    let ceo_name = self.ceo_name;

    let card_content = column([
      render_ticker_plate(self.corporation, self.icon_handle),
      render_identity(self.corporation),
      render_tags(self.corporation),
      components::Separator::horizontal().render(),
      render_stats(self.corporation),
      components::Separator::horizontal().render(),
      render_ceo_hq(self.corporation, ceo_name.as_deref()),
    ])
    .width(Length::Fill);

    let card = container(card_content)
      .width(Length::Fill)
      .height(spacing::layout::CORPORATION_CARD_HEIGHT)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::border::SUBTLE,
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      });

    mouse_area(card)
      .on_right_press(Message::CardRightPressed(id, name))
      .into()
  }
}

fn render_ticker_plate<'a>(corp: &'a Corporation, icon_handle: Option<&'a image::Handle>) -> Element<'a, Message> {
  let logo = render_logo(corp, icon_handle);

  let ticker_el = text(corp.ticker())
    .font(typography::mono::MEDIUM)
    .size(34.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::accent::PLASMA),
    });

  let alliance_el: Element<'a, Message> = match corp.alliance_name() {
    Some(a) => text(format!("‹ {} ›", a))
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::SECONDARY),
      })
      .into(),
    None => text("UNAFFILIATED")
      .font(typography::mono::REGULAR)
      .size(10.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::text::TERTIARY),
      })
      .into(),
  };

  let ticker_col = column([ticker_el.into(), alliance_el]).spacing(8.0);

  container(
    row([logo, container(ticker_col).center_y(Length::Fill).into()])
      .spacing(18.0)
      .align_y(Vertical::Center)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: 22.0,
        right: 22.0,
      }),
  )
  .width(Length::Fill)
  .center_y(140.0)
  .style(|_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border::default(),
    ..container::Style::default()
  })
  .into()
}

fn render_logo<'a>(corp: &'a Corporation, handle: Option<&'a image::Handle>) -> Element<'a, Message> {
  match handle {
    Some(h) => container(image(h).width(72.0).height(72.0))
      .width(72.0)
      .height(72.0)
      .style(|_| container::Style {
        border: Border {
          color: color::border::SUBTLE,
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into(),
    None => {
      let initial = corp.ticker().chars().next().map(|c| c.to_string()).unwrap_or_default();
      container(
        text(initial)
          .size(24.0)
          .font(typography::mono::MEDIUM)
          .style(|_| iced::widget::text::Style {
            color: Some(color::accent::PLASMA),
          }),
      )
      .width(72.0)
      .height(72.0)
      .center_x(72.0)
      .center_y(72.0)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::BASE)),
        border: Border {
          color: color::border::SUBTLE,
          radius: radius::PANEL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
    }
  }
}

fn render_identity<'a>(corporation: &'a Corporation) -> Element<'a, Message> {
  let name_el = text(corporation.name())
    .font(typography::body::MEDIUM)
    .size(17.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let alliance_str = corporation
    .alliance_name()
    .as_deref()
    .unwrap_or("No alliance")
    .to_uppercase();
  let alliance_el = text(alliance_str)
    .font(typography::mono::REGULAR)
    .size(10.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  container(column([name_el.into(), alliance_el.into()]).spacing(spacing::SPACE_1))
    .padding(Padding {
      top: spacing::SPACE_3_5,
      bottom: spacing::SPACE_2_5,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn render_stats<'a>(corporation: &'a Corporation) -> Element<'a, Message> {
  let members_label = text("Members")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let tax_label = text("Tax Rate")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let labels_row = row([
    members_label.into(),
    iced::widget::Space::new().width(Length::Fill).into(),
    tax_label.into(),
  ]);

  let member_str = format_members(*corporation.member_count());
  let members_val = text(member_str)
    .font(typography::mono::MEDIUM)
    .size(22.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let tax_pct = corporation.tax_rate() * 100.0;
  let tax_str = format!("{:.1}%", tax_pct);
  let tax_val = text(tax_str)
    .font(typography::mono::MEDIUM)
    .size(22.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let values_row = row([
    members_val.into(),
    iced::widget::Space::new().width(Length::Fill).into(),
    tax_val.into(),
  ]);

  container(column([labels_row.into(), values_row.into()]).spacing(8.0))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn render_tags<'a>(corporation: &'a Corporation) -> Element<'a, Message> {
  let id = *corporation.id();

  let plus_btn = button(
    text("+")
      .font(typography::body::MEDIUM)
      .size(13.0)
      .style(|_| iced::widget::text::Style {
        color: Some(color::accent::PLASMA),
      }),
  )
  .padding(Padding {
    top: 2.0,
    bottom: 2.0,
    left: 4.0,
    right: 4.0,
  })
  .style(|_, _| button::Style {
    background: Some(Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.08))),
    border: Border {
      color: color::border::SUBTLE,
      radius: radius::FULL.into(),
      width: 1.0,
    },
    ..button::Style::default()
  })
  .on_press(Message::TagsPressed(id));

  let mut items: Vec<Element<'a, Message>> = corporation
    .tags()
    .iter()
    .map(|(_, name)| components::Badge::tag(name).render::<Message>())
    .collect();
  items.push(plus_btn.into());

  container(row(items).spacing(spacing::SPACE_1).wrap())
    .padding(Padding {
      top: 0.0,
      bottom: 10.0,
      left: spacing::SPACE_4,
      right: spacing::SPACE_4,
    })
    .width(Length::Fill)
    .into()
}

fn render_ceo_hq<'a>(corporation: &'a Corporation, ceo_name: Option<&str>) -> Element<'a, Message> {
  let ceo_label = text("CEO")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let ceo_str = ceo_name.unwrap_or("—").to_string();
  let ceo_val = text(ceo_str)
    .font(typography::body::REGULAR)
    .size(13.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::PRIMARY),
    });

  let ceo_col = container(column([ceo_label.into(), ceo_val.into()]).spacing(spacing::SPACE_1))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_4,
      right: spacing::SPACE_3,
    })
    .width(Length::FillPortion(1));

  let hq_label = text("HQ")
    .font(typography::mono::REGULAR)
    .size(9.0)
    .style(|_| iced::widget::text::Style {
      color: Some(color::text::SECONDARY),
    });

  let hq_str = corporation.hq_name().as_deref().unwrap_or("—").to_string();
  let hq_color = if corporation.hq_name().is_some() {
    color::text::PRIMARY
  } else {
    color::text::TERTIARY
  };
  let hq_val = text(hq_str)
    .font(typography::body::REGULAR)
    .size(13.0)
    .style(move |_| iced::widget::text::Style {
      color: Some(hq_color),
    });

  let hq_col = container(column([hq_label.into(), hq_val.into()]).spacing(spacing::SPACE_1))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3,
      right: spacing::SPACE_4,
    })
    .width(Length::FillPortion(1));

  row([ceo_col.into(), stat_divider(), hq_col.into()])
    .width(Length::Fill)
    .into()
}

fn stat_divider<'a>() -> Element<'a, Message> {
  container(iced::widget::Space::new().height(Length::Fill))
    .width(1.0)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::border::SUBTLE)),
      ..container::Style::default()
    })
    .into()
}

fn format_members(n: i32) -> String {
  if n >= 1_000_000 {
    format!("{:.1}M", n as f64 / 1_000_000.0)
  } else if n >= 1_000 {
    let thousands = n / 1_000;
    let remainder = n % 1_000;
    format!("{thousands},{remainder:03}")
  } else {
    n.to_string()
  }
}
