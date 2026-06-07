use iced::{
  Background, Border, Color, Element, Length, Padding,
  alignment::Vertical,
  widget::{Space, column, container, row, text},
};

use super::{super::Message, ImplantEffect, fmt_time_long, fmt_time_short, section_label};
use crate::{
  features::skills::browse::AttrKey,
  ui::style::{color, spacing, typography},
};

fn bonus_value(effect: &ImplantEffect, key: AttrKey) -> u32 {
  match key {
    AttrKey::Charisma => effect.bonus.charisma,
    AttrKey::Intelligence => effect.bonus.intelligence,
    AttrKey::Memory => effect.bonus.memory,
    AttrKey::Perception => effect.bonus.perception,
    AttrKey::Willpower => effect.bonus.willpower,
  }
}

pub(super) fn has_implants(effect: &ImplantEffect) -> bool {
  AttrKey::ALL.iter().any(|&key| bonus_value(effect, key) > 0)
}

pub(super) fn implant_effect_section(effect: &ImplantEffect) -> Element<'static, Message> {
  let saved = (effect.without_sec - effect.with_sec).max(0.0);

  let comparison = row(vec![
    figure_column("WITHOUT", effect.without_sec, false),
    Space::new().width(8.0).into(),
    figure_column("WITH IMPLANTS", effect.with_sec, true),
  ])
  .width(Length::Fill);

  let bonus_pills: Vec<Element<'static, Message>> = AttrKey::ALL
    .iter()
    .filter_map(|&key| {
      let value = bonus_value(effect, key);
      (value > 0).then(|| bonus_pill(key, value))
    })
    .collect();

  let mut items: Vec<Element<'static, Message>> = vec![
    section_label("IMPLANT EFFECT"),
    Space::new().height(spacing::SPACE_3).into(),
    comparison.into(),
    Space::new().height(spacing::SPACE_3).into(),
    savings_callout(saved),
  ];

  if !bonus_pills.is_empty() {
    items.push(Space::new().height(spacing::SPACE_3).into());
    items.push(row(bonus_pills).spacing(6.0).into());
  }

  container(column(items).width(Length::Fill))
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    })
    .width(Length::Fill)
    .into()
}

fn figure_column(title: &'static str, sec: f64, highlight: bool) -> Element<'static, Message> {
  let (bg, border_color, value_color) = if highlight {
    (
      color::with_alpha(color::accent::PLASMA, 0.08),
      color::accent::PLASMA_MUTED,
      color::accent::PLASMA,
    )
  } else {
    (
      color::surface::SUNKEN,
      color::with_alpha(color::text::PRIMARY, 0.1),
      color::text::PRIMARY,
    )
  };

  let header = text(title)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(move |_| text::Style {
      color: Some(if highlight {
        color::accent::PLASMA
      } else {
        color::text::TERTIARY
      }),
    });

  let value = text(fmt_time_short(sec))
    .font(typography::mono::MEDIUM)
    .size(16.0)
    .style(move |_| text::Style {
      color: Some(value_color),
    });

  container(column(vec![header.into(), Space::new().height(6.0).into(), value.into()]).width(Length::Fill))
    .padding(Padding {
      top: 10.0,
      bottom: 10.0,
      left: 10.0,
      right: 10.0,
    })
    .width(Length::FillPortion(1))
    .style(move |_| container::Style {
      background: Some(Background::Color(bg)),
      border: Border {
        color: border_color,
        radius: 6.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn savings_callout(saved: f64) -> Element<'static, Message> {
  let label = if saved > 0.0 {
    format!("Implants save {}", fmt_time_long(saved))
  } else {
    "No training-time effect".to_owned()
  };

  let (bg, border_color, label_color): (Color, Color, Color) = if saved > 0.0 {
    (
      color::with_alpha(color::accent::PLASMA, 0.08),
      color::accent::PLASMA_MUTED,
      color::accent::PLASMA,
    )
  } else {
    (
      color::surface::SUNKEN,
      color::with_alpha(color::text::PRIMARY, 0.1),
      color::text::TERTIARY,
    )
  };

  container(
    text(label)
      .font(typography::mono::MEDIUM)
      .size(13.0)
      .style(move |_| text::Style {
        color: Some(label_color),
      }),
  )
  .padding(Padding {
    top: 10.0,
    bottom: 10.0,
    left: 14.0,
    right: 14.0,
  })
  .width(Length::Fill)
  .style(move |_| container::Style {
    background: Some(Background::Color(bg)),
    border: Border {
      color: border_color,
      radius: 6.0.into(),
      width: 1.0,
    },
    ..container::Style::default()
  })
  .into()
}

fn bonus_pill(key: AttrKey, value: u32) -> Element<'static, Message> {
  container(
    row(vec![
      text(key.short())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(|_| text::Style {
          color: Some(color::text::TERTIARY),
        })
        .into(),
      Space::new().width(4.0).into(),
      text(format!("+{value}"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::accent::PLASMA),
        })
        .into(),
    ])
    .align_y(Vertical::Center),
  )
  .padding(Padding {
    top: 3.0,
    bottom: 3.0,
    left: 7.0,
    right: 7.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.10))),
    border: Border {
      color: color::with_alpha(color::accent::PLASMA, 0.30),
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
  use crate::features::skills::optimizer::Attributes;

  mod has_implants {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_true_when_any_attribute_has_a_bonus() {
      let effect = ImplantEffect {
        bonus: Attributes {
          perception: 4,
          ..Attributes::default()
        },
        with_sec: 100.0,
        without_sec: 120.0,
      };
      assert_eq!(has_implants(&effect), true);
    }

    #[test]
    fn it_reports_false_when_no_attribute_has_a_bonus() {
      let effect = ImplantEffect {
        bonus: Attributes::default(),
        with_sec: 100.0,
        without_sec: 100.0,
      };
      assert_eq!(has_implants(&effect), false);
    }
  }

  mod figure_column {
    use super::*;

    #[test]
    fn it_renders_the_highlighted_and_plain_variants() {
      let _highlighted: Element<'_, Message> = super::figure_column("With implants", 3_600.0, true);
      let _plain: Element<'_, Message> = super::figure_column("Without", 7_200.0, false);
    }
  }

  mod savings_callout {
    use super::*;

    #[test]
    fn it_renders_the_savings_and_no_effect_variants() {
      let _saving: Element<'_, Message> = super::savings_callout(3_600.0);
      let _none: Element<'_, Message> = super::savings_callout(0.0);
    }
  }
}
