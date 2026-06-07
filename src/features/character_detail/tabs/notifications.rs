use iced::{
  Background, Border, Element, Length, Padding,
  alignment::{Horizontal, Vertical},
  widget::{Column, Row, Space, button, container, text},
};

use super::{
  super::{LoadState, Message},
  shared,
};
use crate::{
  store::model::CharacterNotification,
  ui::{
    components::{
      card,
      empty_state::{LoadStateView, empty_state, load_state_view},
      eyebrow::eyebrow_text,
      icon::Icon,
      segmented::segment_button_style,
    },
    style::{color, radius, spacing, typography},
  },
};

const CATEGORY_RULES: &[(&str, &[&str])] = &[
  ("reward", &["bounty", "reward"]),
  ("fw", &["facwar", "factionwar", "milmatch"]),
  ("war", &["war"]),
  ("sovereignty", &["sovereignty", "infrastructurehub"]),
  (
    "structure",
    &["structure", "tower", "citadel", "pos", "orbital", "customsoffice"],
  ),
  ("moon", &["moon", "mining", "extraction"]),
  ("incursion", &["incursion"]),
  ("combat", &["kill", "combat", "attack", "aggression", "destroyed"]),
  ("corp", &["corp", "alliance"]),
  ("contact", &["contact"]),
  ("contract", &["contract"]),
  ("clone", &["clone", "medical"]),
  ("standing", &["standing"]),
  ("insurance", &["insurance"]),
  ("market", &["market", "order", "escrow"]),
  ("industry", &["industry", "job", "manufactur", "research", "reaction"]),
  ("mission", &["mission", "agent"]),
];
const TYPE_OVERRIDES: &[(&str, &str)] = &[
  ("CorpBecameWarEligible", "war"),
  ("CorpNoLongerWarEligible", "war"),
  ("FacWarCorpJoinRequestMsg", "fw"),
  ("FacWarCorpJoinWithdrawMsg", "fw"),
  ("FacWarCorpLeaveRequestMsg", "fw"),
  ("FacWarCorpLeaveWithdrawMsg", "fw"),
  ("FacWarLPDisqualified", "fw"),
  ("FacWarLPPayoutEvent", "fw"),
  ("FacWarLPPayoutKill", "fw"),
  ("FactionWarCampaignOver", "fw"),
  ("FactionWarStalemate", "fw"),
  ("FacWarPlayerInactivityKickWarning", "fw"),
  ("FacWarPlayerKickFromOccupier", "fw"),
  ("FacWarPlayerKickedMsg", "fw"),
  ("CorpFwStandingLoss", "fw"),
  ("CloneActivationMsg", "clone"),
  ("CloneActivationMsg2", "clone"),
  ("CloneMovedMsg", "clone"),
  ("CloneRevokedMsg1", "clone"),
  ("CloneRevokedMsg2", "clone"),
  ("JumpCloneDeletedMsg1", "clone"),
  ("JumpCloneDeletedMsg2", "clone"),
  ("CharAppAcceptMsg", "standing"),
  ("CorpFriendlyFireEnableTimerStarted", "standing"),
  ("CorpFriendlyFireDisableTimerStarted", "standing"),
  ("CorpFriendlyFireEnableTimerCompleted", "standing"),
  ("CorpFriendlyFireDisableTimerCompleted", "standing"),
  ("AgentMoved", "mission"),
  ("MissionCanceledTriglavian", "mission"),
  ("MissionOfferExpirationMsg", "mission"),
  ("TutorialMsg", "system"),
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotificationsFilter {
  #[default]
  All,
  Combat,
  Corp,
  Structure,
  Unread,
  War,
}

impl NotificationsFilter {
  const SEGMENTS: [(NotificationsFilter, &'static str); 6] = [
    (NotificationsFilter::All, "All"),
    (NotificationsFilter::Combat, "Combat"),
    (NotificationsFilter::Corp, "Corp"),
    (NotificationsFilter::Structure, "Structure"),
    (NotificationsFilter::Unread, "Unread"),
    (NotificationsFilter::War, "War"),
  ];

  pub(in crate::features::character_detail) fn matches(self, notification: &CharacterNotification) -> bool {
    match self {
      NotificationsFilter::All => true,
      NotificationsFilter::Unread => !notification.is_read(),
      NotificationsFilter::Combat => category(notification.notif_type()) == "combat",
      NotificationsFilter::Corp => category(notification.notif_type()) == "corp",
      NotificationsFilter::Structure => category(notification.notif_type()) == "structure",
      NotificationsFilter::War => category(notification.notif_type()) == "war",
    }
  }
}

pub(in crate::features::character_detail) fn unread_count(notifications: &[CharacterNotification]) -> usize {
  notifications.iter().filter(|n| !n.is_read()).count()
}

pub(in crate::features::character_detail) fn body(
  notifications: &LoadState<Vec<CharacterNotification>>,
  filter: NotificationsFilter,
) -> Element<'_, Message> {
  let entries = match notifications {
    LoadState::Loaded(entries) => entries,
    LoadState::Loading => return load_state_view(LoadStateView::Loading("Loading notifications\u{2026}")),
    LoadState::Error(error) => return load_state_view(LoadStateView::Error(error)),
  };
  if entries.is_empty() {
    return load_state_view(LoadStateView::Empty(empty_state("No notifications recorded")));
  }

  let visible: Vec<&CharacterNotification> = entries.iter().filter(|n| filter.matches(n)).collect();
  let unread = unread_count(entries);

  let eyebrow = Row::with_children(vec![
    eyebrow_text(
      &format!("Notifications \u{00b7} {}", visible.len()),
      Some(color::text::SECONDARY),
    )
    .into(),
    Space::new().width(Length::Fixed(spacing::SPACE_2)).into(),
    eyebrow_text(&format!("{unread} unread"), Some(color::text::DIM)).into(),
    Space::new().width(Length::Fill).into(),
    segmented(filter),
  ])
  .align_y(Vertical::Center)
  .width(Length::Fill);

  let card = notifications_card(&visible);

  Column::with_children(vec![eyebrow.into(), card])
    .spacing(spacing::SPACE_3_5 + spacing::SPACE_2)
    .width(Length::Fill)
    .into()
}

fn segmented<'a>(active: NotificationsFilter) -> Element<'a, Message> {
  let mut buttons: Vec<Element<'a, Message>> = Vec::with_capacity(NotificationsFilter::SEGMENTS.len());
  for (filter, label) in NotificationsFilter::SEGMENTS {
    let selected = filter == active;
    let label_color = if selected {
      color::accent::PLASMA
    } else {
      color::text::SECONDARY
    };
    buttons.push(
      button(
        text(label)
          .font(typography::body::MEDIUM)
          .size(typography::size::SM)
          .style(move |_| text::Style {
            color: Some(label_color),
          }),
      )
      .padding(Padding {
        top: spacing::UNIT + 1.0,
        right: spacing::SPACE_3,
        bottom: spacing::UNIT + 1.0,
        left: spacing::SPACE_3,
      })
      .on_press(Message::NotificationsFilterChanged(filter))
      .style(move |_, status| segment_button_style(selected, status))
      .into(),
    );
  }

  container(Row::with_children(buttons).spacing(2.0))
    .padding(2.0)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::with_alpha(color::text::PRIMARY, 0.08),
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn notifications_card<'a>(visible: &[&'a CharacterNotification]) -> Element<'a, Message> {
  let mut rows: Vec<Element<'a, Message>> = Vec::new();
  if visible.is_empty() {
    rows.push(
      container(
        text("No notifications match this filter")
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(|_| text::Style {
            color: Some(color::text::SECONDARY),
          }),
      )
      .width(Length::Fill)
      .padding(spacing::SPACE_3_5 + spacing::SPACE_2)
      .into(),
    );
  } else {
    for (index, notification) in visible.iter().enumerate() {
      rows.push(notif_row(notification, index == visible.len() - 1));
    }
  }

  card::panel(Column::with_children(rows).width(Length::Fill), false)
}

fn notif_row<'a>(notification: &'a CharacterNotification, last: bool) -> Element<'a, Message> {
  let cat = category(notification.notif_type());
  let unread = !notification.is_read();

  let icon = icon_box(cat);
  let content = content_col(notification, cat, unread);
  let timestamp = container(
    text(relative_time(notification.timestamp()))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(|_| text::Style {
        color: Some(color::text::TERTIARY),
      }),
  )
  .align_x(Horizontal::Right);

  let inner = Row::with_children(vec![icon, content, timestamp.into()])
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Top)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_3_5 + spacing::SPACE_2,
      bottom: spacing::SPACE_3,
      left: if unread {
        spacing::SPACE_3_5
      } else {
        spacing::SPACE_3_5 + spacing::SPACE_2
      },
    })
    .width(Length::Fill);

  let border_bottom = if last { 0.0 } else { 1.0 };

  if unread {
    let plasma_bar = container(Space::new().width(Length::Fixed(2.0)).height(Length::Fill))
      .width(Length::Fixed(2.0))
      .style(|_| container::Style {
        background: Some(Background::Color(color::accent::PLASMA)),
        ..container::Style::default()
      });

    return button(Row::with_children(vec![plasma_bar.into(), inner.into()]).width(Length::Fill))
      .width(Length::Fill)
      .padding(Padding::ZERO)
      .on_press(Message::NotificationRead(notification.notification_id()))
      .style(move |_, _| button::Style {
        background: Some(Background::Color(color::with_alpha(color::accent::PLASMA, 0.06))),
        border: Border {
          color: color::with_alpha(color::text::PRIMARY, if border_bottom > 0.0 { 0.06 } else { 0.0 }),
          width: border_bottom,
          ..Border::default()
        },
        ..button::Style::default()
      })
      .into();
  }

  container(inner)
    .width(Length::Fill)
    .style(move |_| shared::row_rule_style(border_bottom))
    .into()
}

fn content_col<'a>(notification: &'a CharacterNotification, cat: &'static str, unread: bool) -> Element<'a, Message> {
  let cat_color = category_color(cat);
  let title_font = if unread {
    typography::mono::MEDIUM
  } else {
    typography::mono::REGULAR
  };

  let mut items: Vec<Element<'a, Message>> = vec![
    eyebrow_text(cat, Some(cat_color)).into(),
    text(humanise_type(notification.notif_type()))
      .font(title_font)
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
  ];

  if let Some(snippet) = body_snippet(notification) {
    items.push(
      text(snippet)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::SECONDARY),
        })
        .into(),
    );
  }

  Column::with_children(items).spacing(3.0).width(Length::Fill).into()
}

fn icon_box<'a>(cat: &'static str) -> Element<'a, Message> {
  let cat_color = category_color(cat);
  let icon = category_icon(cat).size(16.0).color(cat_color).render::<Message>();
  container(icon)
    .width(Length::Fixed(28.0))
    .height(Length::Fixed(28.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::text::PRIMARY, 0.05))),
      border: Border {
        color: cat_color,
        width: 1.0,
        radius: radius::CONTROL.into(),
      },
      ..container::Style::default()
    })
    .into()
}

fn body_snippet(notification: &CharacterNotification) -> Option<String> {
  let text = notification.text().as_deref()?;
  let first = text.lines().next().unwrap_or("").trim();
  if first.is_empty() {
    return None;
  }
  Some(if first.chars().count() > 80 {
    let truncated: String = first.chars().take(80).collect();
    format!("{truncated}\u{2026}")
  } else {
    first.to_owned()
  })
}

pub(in crate::features::character_detail) fn humanise_type(notif_type: &str) -> String {
  let mut out = String::with_capacity(notif_type.len() + 4);
  for c in notif_type.chars() {
    if c.is_uppercase() && !out.is_empty() && !out.ends_with(' ') {
      out.push(' ');
    }
    out.push(c);
  }
  out
}

pub(in crate::features::character_detail) fn category(notif_type: &str) -> &'static str {
  if let Some((_, cat)) = TYPE_OVERRIDES.iter().find(|(name, _)| *name == notif_type) {
    return cat;
  }

  let lower = notif_type.to_ascii_lowercase();
  CATEGORY_RULES
    .iter()
    .find(|(_, needles)| needles.iter().any(|needle| lower.contains(needle)))
    .map_or("system", |(cat, _)| cat)
}

fn category_color(category: &str) -> iced::Color {
  match category {
    "war" | "fw" | "combat" | "incursion" => color::status::DANGER,
    "corp" | "sovereignty" => color::with_alpha(color::status::DANGER, 0.7),
    "structure" | "moon" | "mission" | "industry" | "standing" => color::accent::PLASMA,
    "market" | "insurance" | "reward" => color::status::ONLINE,
    "contract" | "clone" | "contact" => color::with_alpha(color::accent::PLASMA, 0.7),
    _ => color::text::SECONDARY,
  }
}

fn category_icon(category: &str) -> Icon {
  match category {
    "war" => Icon::notif_war(),
    "fw" => Icon::notif_fw(),
    "combat" => Icon::notif_combat(),
    "incursion" => Icon::notif_incursion(),
    "corp" => Icon::notif_corp(),
    "sovereignty" => Icon::notif_structure(),
    "structure" => Icon::notif_structure(),
    "moon" => Icon::notif_industry(),
    "contact" => Icon::notif_contact(),
    "contract" => Icon::notif_contract(),
    "clone" => Icon::notif_clone(),
    "standing" => Icon::notif_standing(),
    "insurance" => Icon::notif_insurance(),
    "market" => Icon::notif_market(),
    "reward" => Icon::notif_reward(),
    "industry" => Icon::notif_industry(),
    "mission" => Icon::notif_mission(),
    _ => Icon::notif_system(),
  }
}

fn relative_time(iso: &str) -> String {
  super::killlog::relative_time(iso)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn notification(notification_id: i64, notif_type: &str, is_read: bool) -> CharacterNotification {
    CharacterNotification {
      character_id: 42,
      is_read,
      notif_type: notif_type.to_owned(),
      notification_id,
      sender_id: Some(1001),
      sender_type: Some("character".to_owned()),
      synced_at: "2024-01-02T00:00:00Z".to_owned(),
      text: Some("First line\nSecond line".to_owned()),
      timestamp: "2024-01-01T00:00:00Z".to_owned(),
    }
  }

  mod body {
    use super::*;

    #[test]
    fn it_renders_each_filter() {
      let loaded = LoadState::Loaded(vec![
        notification(1, "KillReportFinalBlow", false),
        notification(2, "CorpAppNewMsg", true),
        notification(3, "StructureUnderAttack", false),
        notification(4, "WarDeclared", true),
      ]);

      for filter in [
        NotificationsFilter::All,
        NotificationsFilter::Combat,
        NotificationsFilter::Corp,
        NotificationsFilter::Structure,
        NotificationsFilter::Unread,
        NotificationsFilter::War,
      ] {
        let _el: Element<'_, Message> = body(&loaded, filter);
      }
    }

    #[test]
    fn it_renders_the_empty_loading_and_error_states() {
      let empty = LoadState::Loaded(Vec::new());
      let loading: LoadState<Vec<CharacterNotification>> = LoadState::Loading;
      let error: LoadState<Vec<CharacterNotification>> = LoadState::Error("boom".to_owned());

      let _empty: Element<'_, Message> = body(&empty, NotificationsFilter::All);
      let _loading: Element<'_, Message> = body(&loading, NotificationsFilter::All);
      let _error: Element<'_, Message> = body(&error, NotificationsFilter::All);
    }
  }

  mod filter {
    use super::*;

    #[test]
    fn it_passes_everything_for_all() {
      let n = [notification(1, "KillReportFinalBlow", true)];
      assert!(NotificationsFilter::All.matches(&n[0]));
    }

    #[test]
    fn it_matches_unread_only() {
      let read = notification(1, "KillReportFinalBlow", true);
      let unread = notification(2, "KillReportFinalBlow", false);
      assert!(!NotificationsFilter::Unread.matches(&read));
      assert!(NotificationsFilter::Unread.matches(&unread));
    }

    #[test]
    fn it_matches_by_derived_category() {
      let combat = notification(1, "KillReportFinalBlow", false);
      let war = notification(2, "WarDeclared", false);
      assert!(NotificationsFilter::Combat.matches(&combat));
      assert!(!NotificationsFilter::Combat.matches(&war));
      assert!(NotificationsFilter::War.matches(&war));
    }
  }

  mod unread_count {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_only_unread() {
      let entries = vec![
        notification(1, "A", false),
        notification(2, "B", true),
        notification(3, "C", false),
      ];
      assert_eq!(unread_count(&entries), 2);
    }
  }

  mod category {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_buckets_known_type_prefixes() {
      assert_eq!(category("WarDeclared"), "war");
      assert_eq!(category("KillReportFinalBlow"), "combat");
      assert_eq!(category("StructureUnderAttack"), "structure");
      assert_eq!(category("CorpAppNewMsg"), "corp");
      assert_eq!(category("SomethingUnknown"), "system");
    }

    #[test]
    fn it_buckets_every_category_branch() {
      assert_eq!(category("WarSurrenderOffer"), "war");
      assert_eq!(category("StructureOnline"), "structure");
      assert_eq!(category("TowerAlertMsg"), "structure");
      assert_eq!(category("CitadelDestroyed"), "structure");
      assert_eq!(category("KillReportVictim"), "combat");
      assert_eq!(category("CombatOperationEnd"), "combat");
      assert_eq!(category("EntosisAttack"), "combat");
      assert_eq!(category("CorpVoteMsg"), "corp");
      assert_eq!(category("AllianceMaintenanceMsg"), "corp");
      assert_eq!(category("ContractAvailable"), "contract");
      assert_eq!(category("CloneActivationMsg"), "clone");
      assert_eq!(category("StandingsChanged"), "standing");
      assert_eq!(category("InsuranceExpirationMsg"), "insurance");
      assert_eq!(category("MarketEscrowRelease"), "market");
      assert_eq!(category("OrderExpiry"), "market");
      assert_eq!(category("BountyClaimMsg"), "reward");
      assert_eq!(category("RewardPayout"), "reward");
      assert_eq!(category("IndustryJobCompleted"), "industry");
      assert_eq!(category("JobFinished"), "industry");
      assert_eq!(category("MissionOfferMsg"), "mission");
      assert_eq!(category("AgentMoved"), "mission");
    }

    #[test]
    fn it_buckets_the_expanded_category_families() {
      assert_eq!(category("FacWarLPPayoutEvent"), "fw");
      assert_eq!(category("FactionWarCampaignOver"), "fw");
      assert_eq!(category("SovereigntyTCUDamageMsg"), "sovereignty");
      assert_eq!(category("InfrastructureHubBillAboutToExpire"), "sovereignty");
      assert_eq!(category("MoonminingExtractionStarted"), "moon");
      assert_eq!(category("MiningOperationFinished"), "moon");
      assert_eq!(category("IncursionCompletedMsg"), "incursion");
      assert_eq!(category("ContactAdded"), "contact");
      assert_eq!(category("CustomsOfficeAttacked"), "structure");
      assert_eq!(category("OrbitalAttacked"), "structure");
      assert_eq!(category("ResearchJobCompleted"), "industry");
      assert_eq!(category("ReactionFinished"), "industry");
      assert_eq!(category("MedicalCloneExpired"), "clone");
    }

    #[test]
    fn fw_types_win_over_the_generic_war_rule() {
      assert_eq!(category("FacWarCorpJoinRequestMsg"), "fw");
      assert_eq!(category("CorpFwStandingLoss"), "fw");
      assert_eq!(category("FactionWarStalemate"), "fw");
    }

    #[test]
    fn override_map_fixes_clone_standing_and_mission_edge_cases() {
      assert_eq!(category("JumpCloneDeletedMsg1"), "clone");
      assert_eq!(category("JumpCloneDeletedMsg2"), "clone");
      assert_eq!(category("CloneRevokedMsg1"), "clone");
      assert_eq!(category("CorpFriendlyFireEnableTimerStarted"), "standing");
      assert_eq!(category("CharAppAcceptMsg"), "standing");
      assert_eq!(category("AgentMoved"), "mission");
      assert_eq!(category("MissionOfferExpirationMsg"), "mission");
    }

    #[test]
    fn override_map_is_consulted_before_substring_fallback() {
      assert_eq!(category("CorpBecameWarEligible"), "war");
      assert_eq!(category("CorpNoLongerWarEligible"), "war");
    }

    #[test]
    fn every_override_target_is_a_known_category() {
      for (_, cat) in TYPE_OVERRIDES {
        let _icon = category_icon(cat);
        let _color = category_color(cat);
        assert_ne!(*cat, "");
      }
    }
  }

  mod category_icon {
    use super::*;

    #[test]
    fn it_returns_an_icon_for_every_bucket() {
      for cat in [
        "war",
        "fw",
        "combat",
        "incursion",
        "corp",
        "sovereignty",
        "structure",
        "moon",
        "contact",
        "contract",
        "clone",
        "standing",
        "insurance",
        "market",
        "reward",
        "industry",
        "mission",
        "system",
        "unmapped",
      ] {
        let _icon = category_icon(cat);
      }
    }
  }

  mod humanise_type {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_spaces_pascal_case() {
      assert_eq!(humanise_type("KillReportFinalBlow"), "Kill Report Final Blow");
      assert_eq!(humanise_type(""), "");
    }
  }

  mod body_snippet {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_takes_the_first_non_empty_line() {
      let n = notification(1, "A", false);
      assert_eq!(body_snippet(&n).as_deref(), Some("First line"));
    }

    #[test]
    fn it_is_none_for_missing_or_blank_text() {
      let mut n = notification(1, "A", false);
      n.text = None;
      assert_eq!(body_snippet(&n), None);
      n.text = Some("   \n".to_owned());
      assert_eq!(body_snippet(&n), None);
    }
  }
}
