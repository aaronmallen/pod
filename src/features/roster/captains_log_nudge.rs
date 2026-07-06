use chrono::{Datelike, NaiveDate};
use iced::{
  Background, Border, Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, container, text},
};

use super::{Message, Pane, State};
use crate::{
  features::roster::captains_log::{prompts, rollup},
  store::Database,
  ui::{
    components::{button::Button, icon::Icon},
    style::{color, radius, spacing, typography},
  },
};

const CARD_WIDTH: f32 = 340.0;

const ICON_TILE: f32 = 30.0;

const MARGIN: f32 = 20.0;

const PRIMARY_HEIGHT: f32 = 36.0;

const YC_EPOCH_OFFSET: i32 = 1898;

#[derive(Debug, Default)]
pub struct Nudge {
  complete: bool,
  dismissed_on: Option<NaiveDate>,
  today: Option<NaiveDate>,
}

impl Nudge {
  pub fn dismiss(&mut self) -> Option<NaiveDate> {
    self.dismissed_on = self.today;
    self.today
  }

  pub fn evaluated(&mut self, today: NaiveDate, complete: bool) {
    self.today = Some(today);
    self.complete = complete;
  }

  pub fn restore_dismissed(&mut self, dismissed_on: Option<NaiveDate>) {
    self.dismissed_on = dismissed_on;
  }

  pub fn visible(&self) -> bool {
    match self.today {
      Some(today) => !self.complete && self.dismissed_on != Some(today),
      None => false,
    }
  }
}

pub async fn evaluate(db: Database, date: String, character_ids: Vec<i64>) -> bool {
  match completeness(&db, &date, &character_ids).await {
    Some(completeness) => completeness.is_complete(),
    None => true,
  }
}

pub fn layer(state: &State) -> Option<Element<'_, Message>> {
  if state.active_pane != Pane::Characters || !state.captains_log_nudge.visible() {
    return None;
  }

  let today = state.captains_log_nudge.today?;
  Some(overlay(today))
}

async fn completeness(db: &Database, date: &str, character_ids: &[i64]) -> Option<prompts::Completeness> {
  let day = rollup::for_date(db, date).await.ok()?;
  let activity = day_activity(&day);

  prompts::completeness_for_day(db, date, character_ids, &activity)
    .await
    .ok()
}

fn card<'a>(today: NaiveDate) -> Element<'a, Message> {
  let tile = container(Icon::journal().size(16.0).color(color::accent()).render())
    .width(Length::Fixed(ICON_TILE))
    .height(Length::Fixed(ICON_TILE))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::accent(), 0.12))),
      border: Border {
        color: color::with_alpha(color::accent(), 0.3),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

  let kicker = text(t!("captains_log.nudge.kicker", date => eve_date_label(today)).into_owned())
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::accent()));

  let close = Button::ghost_icon(Icon::close()).on_press(Message::CaptainsLogNudgeDismissed);

  let header = Row::with_children(vec![
    tile.into(),
    kicker.into(),
    Space::new().width(Length::Fill).into(),
    close.into(),
  ])
  .spacing(spacing::SPACE_2)
  .align_y(Vertical::Center);

  let title = text(t!("captains_log.nudge.title").into_owned())
    .font(typography::body::MEDIUM)
    .size(16.0)
    .style(typography::colored(color::text::PRIMARY));

  let body = text(t!("captains_log.nudge.body").into_owned())
    .font(typography::body::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::secondary()));

  let primary = Button::primary(t!("captains_log.nudge.primary").into_owned())
    .icon_right(Icon::chevron_right())
    .block()
    .height(PRIMARY_HEIGHT)
    .on_press(Message::CaptainsLogNudgeOpened);

  let later = Button::secondary(t!("captains_log.nudge.dismiss").into_owned())
    .height(PRIMARY_HEIGHT)
    .on_press(Message::CaptainsLogNudgeDismissed);

  let actions = Row::with_children(vec![primary.into(), later.into()]).spacing(spacing::SPACE_2_5);

  let content = Column::with_children(vec![header.into(), title.into(), body.into(), actions.into()])
    .spacing(spacing::SPACE_2_5)
    .width(Length::Fill);

  container(content)
    .width(Length::Fixed(CARD_WIDTH))
    .padding(spacing::SPACE_4_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::with_alpha(color::accent(), 0.4),
        radius: radius::PANEL.into(),
        width: 1.0,
      },
      shadow: iced::Shadow {
        color: color::with_alpha(iced::Color::BLACK, 0.5),
        offset: iced::Vector {
          x: 0.0,
          y: 18.0,
        },
        blur_radius: 44.0,
      },
      ..container::Style::default()
    })
    .into()
}

fn day_activity(day: &rollup::DayRollup) -> prompts::DayActivity {
  let losses = day
    .combat
    .engagements
    .iter()
    .filter(|kill| !kill.is_kill)
    .map(|kill| prompts::LossEngagement {
      character_id: kill.character_id,
      killmail_id: kill.killmail_id,
    })
    .collect();

  prompts::DayActivity {
    engagement_count: day.combat.engagements.len() as u32,
    industry_count: day.industry.len() as u32,
    losses,
    skill_count: day.skills.len() as u32,
  }
}

fn eve_date_label(date: NaiveDate) -> String {
  format!(
    "YC{}.{:02}.{:02}",
    date.year() - YC_EPOCH_OFFSET,
    date.month(),
    date.day()
  )
}

fn overlay<'a>(today: NaiveDate) -> Element<'a, Message> {
  container(card(today))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Bottom)
    .padding(MARGIN)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 7, day).unwrap()
  }

  mod visible {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_stays_hidden_until_a_day_is_evaluated() {
      let nudge = Nudge::default();

      assert!(!nudge.visible());
    }

    #[test]
    fn it_shows_once_an_incomplete_day_is_evaluated() {
      let mut nudge = Nudge::default();
      nudge.evaluated(date(6), false);

      assert!(nudge.visible());
    }

    #[test]
    fn it_hides_when_todays_log_is_complete() {
      let mut nudge = Nudge::default();
      nudge.evaluated(date(6), true);

      assert!(!nudge.visible());
    }

    #[test]
    fn it_hides_after_being_dismissed_today() {
      let mut nudge = Nudge::default();
      nudge.evaluated(date(6), false);

      let dismissed = nudge.dismiss();

      assert_eq!(dismissed, Some(date(6)));
      assert!(!nudge.visible());
    }

    #[test]
    fn it_shows_again_once_a_new_day_is_evaluated() {
      let mut nudge = Nudge::default();
      nudge.evaluated(date(6), false);
      nudge.dismiss();

      nudge.evaluated(date(7), false);

      assert!(nudge.visible());
    }

    #[test]
    fn it_restores_a_prior_days_dismissal_without_suppressing_a_new_day() {
      let mut nudge = Nudge::default();
      nudge.restore_dismissed(Some(date(6)));

      nudge.evaluated(date(6), false);
      assert!(!nudge.visible());

      nudge.evaluated(date(7), false);
      assert!(nudge.visible());
    }
  }

  mod eve_date_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_the_date_as_yc_year_month_day() {
      assert_eq!(eve_date_label(date(5)), "YC128.07.05");
    }
  }
}
