use std::{collections::HashSet, time::Duration};

use super::{outcome::Outcome, subject::Subject};
use crate::{
  clients::{self, esi, esi::scopes, eve_image, eve_sso::Grant},
  config::{Feature, FeatureFlags},
  store::{Database, images},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum JobKind {
  AssetSync,
  CharacterAbyssals,
  CharacterBlueprints,
  CharacterCalendar,
  CharacterClones,
  CharacterContacts,
  CharacterContracts,
  CharacterIndustryJobs,
  CharacterKillmails,
  CharacterMail,
  CharacterMarketOrders,
  CharacterNotifications,
  CharacterProfile,
  CharacterSkills,
  CharacterStandings,
  CharacterTelemetry,
  CharacterWallet,
  CorporationBlueprints,
  CorporationIndustryJobs,
  CorporationProfile,
  CorporationWallet,
  KillmailReconcile,
  MarketPrices,
  NetWorthSnapshot,
}

impl JobKind {
  pub const ALL: &'static [JobKind] = &[
    JobKind::AssetSync,
    JobKind::CharacterAbyssals,
    JobKind::CharacterBlueprints,
    JobKind::CharacterCalendar,
    JobKind::CharacterClones,
    JobKind::CharacterContacts,
    JobKind::CharacterContracts,
    JobKind::CharacterIndustryJobs,
    JobKind::CharacterKillmails,
    JobKind::CharacterMail,
    JobKind::CharacterMarketOrders,
    JobKind::CharacterNotifications,
    JobKind::CharacterProfile,
    JobKind::CharacterSkills,
    JobKind::CharacterStandings,
    JobKind::CharacterTelemetry,
    JobKind::CharacterWallet,
    JobKind::CorporationBlueprints,
    JobKind::CorporationIndustryJobs,
    JobKind::CorporationProfile,
    JobKind::CorporationWallet,
    JobKind::KillmailReconcile,
    JobKind::MarketPrices,
    JobKind::NetWorthSnapshot,
  ];

  pub fn for_subject(subject: Subject) -> Vec<JobKind> {
    Self::ALL
      .iter()
      .copied()
      .filter(|kind| kind.applies_to(subject))
      .collect()
  }

  pub fn feature(self) -> Option<Feature> {
    crate::features::registry::feature_for_job(self)
  }

  pub fn is_feature_enabled(self, features: &FeatureFlags) -> bool {
    self.feature().is_none_or(|feature| features.is_enabled(feature))
  }

  pub fn is_global(self) -> bool {
    !self.applies_to(Subject::Character(0)) && !self.applies_to(Subject::Corporation(0))
  }

  pub fn on_success_triggers(self) -> &'static [JobKind] {
    match self {
      Self::AssetSync => &[JobKind::MarketPrices, JobKind::CharacterAbyssals],
      Self::CharacterWallet | Self::CorporationWallet => &[JobKind::MarketPrices],
      Self::CharacterProfile => &[JobKind::AssetSync, JobKind::CharacterWallet],
      Self::CorporationProfile => &[JobKind::AssetSync, JobKind::CorporationWallet],
      Self::MarketPrices => &[JobKind::NetWorthSnapshot],
      _ => &[],
    }
  }

  pub fn public_for_subject(subject: Subject) -> Vec<JobKind> {
    Self::for_subject(subject)
      .into_iter()
      .filter(|kind| kind.required_scope().is_empty())
      .collect()
  }

  pub fn granted_for_subject(subject: Subject, granted: &HashSet<&str>) -> Vec<JobKind> {
    Self::for_subject(subject)
      .into_iter()
      .filter(|kind| kind.is_scope_granted(subject, granted))
      .collect()
  }

  pub fn is_scope_granted(self, subject: Subject, granted: &HashSet<&str>) -> bool {
    self.gating_scope(subject).is_none_or(|scope| granted.contains(scope))
  }

  pub fn gating_scope(self, subject: Subject) -> Option<&'static str> {
    match (self, subject) {
      (Self::AssetSync, Subject::Corporation(_)) => Some(scopes::CORPORATION_ASSETS),
      (Self::AssetSync, Subject::Character(_)) => Some(scopes::CHARACTER_ASSETS),
      (Self::CharacterBlueprints, _) => Some(scopes::CHARACTER_BLUEPRINTS),
      (Self::CharacterCalendar, _) => Some(scopes::CHARACTER_CALENDAR_READ),
      (Self::CharacterClones, _) => Some(scopes::CHARACTER_CLONES),
      (Self::CharacterContacts, _) => Some(scopes::CHARACTER_CONTACTS),
      (Self::CharacterContracts, _) => Some(scopes::CHARACTER_CONTRACTS),
      (Self::CharacterIndustryJobs, _) => Some(scopes::CHARACTER_INDUSTRY_JOBS),
      (Self::CharacterKillmails, _) => Some(scopes::CHARACTER_KILLMAILS),
      (Self::CharacterMail, _) => Some(scopes::CHARACTER_MAIL),
      (Self::CharacterMarketOrders, _) => Some(scopes::CHARACTER_ORDERS),
      (Self::CharacterNotifications, _) => Some(scopes::CHARACTER_NOTIFICATIONS),
      (Self::CharacterSkills, _) => Some(scopes::CHARACTER_SKILLS),
      (Self::CharacterStandings, _) => Some(scopes::CHARACTER_STANDINGS),
      (Self::CharacterTelemetry, _) => Some(scopes::CHARACTER_LOCATION),
      (Self::CharacterWallet, _) => Some(scopes::CHARACTER_WALLET),
      (Self::CorporationBlueprints, _) => Some(scopes::CORPORATION_BLUEPRINTS),
      (Self::CorporationIndustryJobs, _) => Some(scopes::CORPORATION_INDUSTRY_JOBS),
      (Self::CorporationProfile, _) => Some(scopes::CORPORATION_ROLES),
      (Self::CorporationWallet, _) => Some(scopes::CORPORATION_WALLET),
      _ => None,
    }
  }

  pub fn applies_to(self, subject: Subject) -> bool {
    matches!(
      (self, subject),
      (
        Self::AssetSync
          | Self::CharacterAbyssals
          | Self::CharacterBlueprints
          | Self::CharacterCalendar
          | Self::CharacterClones
          | Self::CharacterContacts
          | Self::CharacterContracts
          | Self::CharacterIndustryJobs
          | Self::CharacterKillmails
          | Self::CharacterMail
          | Self::CharacterMarketOrders
          | Self::CharacterNotifications
          | Self::CharacterProfile
          | Self::CharacterSkills
          | Self::CharacterStandings
          | Self::CharacterTelemetry
          | Self::CharacterWallet,
        Subject::Character(_)
      ) | (
        Self::AssetSync
          | Self::CorporationBlueprints
          | Self::CorporationIndustryJobs
          | Self::CorporationProfile
          | Self::CorporationWallet,
        Subject::Corporation(_)
      )
    )
  }

  pub fn interval(self) -> Duration {
    match self {
      Self::AssetSync
      | Self::CharacterAbyssals
      | Self::CharacterBlueprints
      | Self::CharacterClones
      | Self::CharacterContacts
      | Self::CharacterContracts
      | Self::CharacterIndustryJobs
      | Self::CharacterMarketOrders
      | Self::CharacterProfile
      | Self::CharacterStandings
      | Self::CharacterWallet
      | Self::CorporationBlueprints
      | Self::CorporationIndustryJobs
      | Self::CorporationProfile
      | Self::CorporationWallet => Duration::from_secs(3600),
      Self::CharacterSkills => Duration::from_secs(60),
      Self::CharacterCalendar
      | Self::CharacterKillmails
      | Self::CharacterMail
      | Self::CharacterNotifications
      | Self::CharacterTelemetry => Duration::from_secs(300),
      Self::KillmailReconcile | Self::MarketPrices => Duration::from_secs(6 * 3600),
      Self::NetWorthSnapshot => Duration::from_secs(24 * 3600),
    }
  }

  pub fn required_scope(self) -> &'static [&'static str] {
    match self {
      Self::AssetSync => &[scopes::CHARACTER_ASSETS, scopes::CORPORATION_ASSETS],
      Self::CharacterAbyssals => &[],
      Self::CharacterBlueprints => &[scopes::CHARACTER_BLUEPRINTS],
      Self::CharacterCalendar => &[scopes::CHARACTER_CALENDAR_READ],
      Self::CharacterClones => &[scopes::CHARACTER_CLONES, scopes::CHARACTER_IMPLANTS],
      Self::CharacterContacts => &[scopes::CHARACTER_CONTACTS],
      Self::CharacterContracts => &[scopes::CHARACTER_CONTRACTS],
      Self::CharacterIndustryJobs => &[scopes::CHARACTER_INDUSTRY_JOBS],
      Self::CharacterKillmails => &[scopes::CHARACTER_KILLMAILS, scopes::CORPORATION_KILLMAILS],
      Self::CharacterMail => &[scopes::CHARACTER_MAIL],
      Self::CharacterMarketOrders => &[scopes::CHARACTER_ORDERS],
      Self::CharacterNotifications => &[scopes::CHARACTER_NOTIFICATIONS],
      Self::CharacterProfile => &[],
      Self::CharacterSkills => &[
        scopes::CHARACTER_SKILLS,
        scopes::CHARACTER_SKILLQUEUE,
        scopes::CHARACTER_IMPLANTS,
      ],
      Self::CharacterTelemetry => &[
        scopes::CHARACTER_LOCATION,
        scopes::CHARACTER_ONLINE,
        scopes::CHARACTER_SHIP,
      ],
      Self::CharacterStandings => &[scopes::CHARACTER_STANDINGS],
      Self::CharacterWallet => &[scopes::CHARACTER_WALLET],
      Self::CorporationBlueprints => &[scopes::CORPORATION_ROLES, scopes::CORPORATION_BLUEPRINTS],
      Self::CorporationIndustryJobs => &[scopes::CORPORATION_ROLES, scopes::CORPORATION_INDUSTRY_JOBS],
      Self::CorporationProfile => &[scopes::CORPORATION_ROLES],
      Self::CorporationWallet => &[
        scopes::CORPORATION_ROLES,
        scopes::CORPORATION_WALLET,
        scopes::CORPORATION_DIVISIONS,
      ],
      Self::KillmailReconcile | Self::MarketPrices => &[],
      Self::NetWorthSnapshot => &[],
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct JobKey {
  pub kind: JobKind,
  pub subject: Subject,
}

impl JobKey {
  pub fn new(kind: JobKind, subject: Subject) -> Self {
    Self {
      kind,
      subject,
    }
  }
}

pub struct JobCtx<'a> {
  pub db: &'a Database,
  pub esi: &'a esi::Client,
  pub grant: Option<&'a Grant>,
  pub image: &'a eve_image::Client,
  pub image_store: &'a images::Store,
  pub key: JobKey,
}

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, clients::Error> {
  match run_character_job(ctx).await {
    Some(result) => result,
    None => run_shared_job(ctx).await,
  }
}

async fn run_character_job(ctx: &JobCtx<'_>) -> Option<Result<Outcome, clients::Error>> {
  if let Some(result) = run_character_job_a(ctx).await {
    return Some(result);
  }
  run_character_job_b(ctx).await
}

async fn run_character_job_a(ctx: &JobCtx<'_>) -> Option<Result<Outcome, clients::Error>> {
  if let Some(result) = run_character_job_a1(ctx).await {
    return Some(result);
  }
  if let Some(result) = run_character_job_a2(ctx).await {
    return Some(result);
  }
  run_character_job_a3(ctx).await
}

async fn run_character_job_a1(ctx: &JobCtx<'_>) -> Option<Result<Outcome, clients::Error>> {
  Some(match ctx.key.kind {
    JobKind::CharacterAbyssals => super::jobs::abyssals::run(ctx).await,
    JobKind::CharacterBlueprints => super::jobs::blueprints::run(ctx).await,
    JobKind::CharacterCalendar => super::jobs::character_calendar::run(ctx).await,
    JobKind::CharacterClones => super::jobs::character_clones::run(ctx).await,
    _ => return None,
  })
}

async fn run_character_job_a2(ctx: &JobCtx<'_>) -> Option<Result<Outcome, clients::Error>> {
  Some(match ctx.key.kind {
    JobKind::CharacterContacts => super::jobs::character_contacts::run(ctx).await,
    JobKind::CharacterContracts => super::jobs::character_contracts::run(ctx).await,
    JobKind::CharacterIndustryJobs => super::jobs::industry::run(ctx).await,
    _ => return None,
  })
}

async fn run_character_job_a3(ctx: &JobCtx<'_>) -> Option<Result<Outcome, clients::Error>> {
  Some(match ctx.key.kind {
    JobKind::CharacterKillmails => super::jobs::character_killmails::run(ctx).await,
    JobKind::CharacterMail => super::jobs::character_mail::run(ctx).await,
    JobKind::CharacterMarketOrders => super::jobs::character_market_orders::run(ctx).await,
    _ => return None,
  })
}

async fn run_character_job_b(ctx: &JobCtx<'_>) -> Option<Result<Outcome, clients::Error>> {
  Some(match ctx.key.kind {
    JobKind::CharacterNotifications => super::jobs::character_notifications::run(ctx).await,
    JobKind::CharacterProfile => super::jobs::character_profile::run(ctx).await,
    JobKind::CharacterSkills => super::jobs::character_skills::run(ctx).await,
    JobKind::CharacterStandings => super::jobs::character_standings::run(ctx).await,
    JobKind::CharacterTelemetry => super::jobs::character_telemetry::run(ctx).await,
    JobKind::CharacterWallet => super::jobs::character_wallet::run(ctx).await,
    _ => return None,
  })
}

async fn run_shared_job(ctx: &JobCtx<'_>) -> Result<Outcome, clients::Error> {
  match ctx.key.kind {
    JobKind::AssetSync => super::jobs::asset_sync::run(ctx).await,
    JobKind::CorporationBlueprints => super::jobs::blueprints::run(ctx).await,
    JobKind::CorporationIndustryJobs => super::jobs::industry::run(ctx).await,
    JobKind::KillmailReconcile => super::jobs::killmail_reconcile::run(ctx).await,
    JobKind::CorporationProfile => super::jobs::corporation_profile::run(ctx).await,
    JobKind::CorporationWallet => super::jobs::corporation_wallet::run(ctx).await,
    JobKind::MarketPrices => super::jobs::market_prices::run(ctx).await,
    JobKind::NetWorthSnapshot => super::jobs::net_worth_snapshot::run(ctx).await,
    _ => Ok(Outcome::synced()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod job_kind {
    use super::*;

    mod on_success_triggers {
      use pretty_assertions::assert_eq;

      use super::*;

      const GATHERS: [JobKind; 3] = [JobKind::AssetSync, JobKind::CharacterWallet, JobKind::CorporationWallet];

      #[test]
      fn it_chains_wallet_gathers_to_prices_only() {
        for gather in [JobKind::CharacterWallet, JobKind::CorporationWallet] {
          assert_eq!(
            gather.on_success_triggers(),
            [JobKind::MarketPrices],
            "{gather:?} should chain prices, then let prices cascade to the snapshot",
          );
        }
      }

      #[test]
      fn it_chains_asset_sync_to_prices_and_its_derived_abyssals() {
        assert_eq!(
          JobKind::AssetSync.on_success_triggers(),
          [JobKind::MarketPrices, JobKind::CharacterAbyssals],
          "a fresh asset sync must re-fire CharacterAbyssals, which is derived from the asset table",
        );
        assert!(
          JobKind::CharacterAbyssals.applies_to(Subject::Character(1)),
          "abyssals is per-subject, so the engine routes it to the same character that synced assets"
        );
        assert!(
          !JobKind::CharacterAbyssals.applies_to(Subject::Corporation(1)),
          "abyssals is character-only, so a corporation asset sync does not route it"
        );
      }

      #[test]
      fn it_chains_prices_to_the_snapshot() {
        assert_eq!(JobKind::MarketPrices.on_success_triggers(), [JobKind::NetWorthSnapshot]);
      }

      #[test]
      fn it_drops_the_direct_gather_to_snapshot_edge() {
        for gather in GATHERS {
          assert!(
            !gather.on_success_triggers().contains(&JobKind::NetWorthSnapshot),
            "{gather:?} must reach the snapshot through prices, never directly",
          );
        }
      }

      const PROFILES: [JobKind; 2] = [JobKind::CharacterProfile, JobKind::CorporationProfile];

      #[test]
      fn it_triggers_nothing_for_other_kinds() {
        for kind in JobKind::ALL.iter().copied() {
          if matches!(kind, JobKind::MarketPrices) || GATHERS.contains(&kind) || PROFILES.contains(&kind) {
            continue;
          }

          assert!(
            kind.on_success_triggers().is_empty(),
            "{kind:?} should chain no follow-up jobs, got {:?}",
            kind.on_success_triggers()
          );
        }
      }

      #[test]
      fn it_refires_the_gather_jobs_when_a_profile_lands() {
        assert_eq!(
          JobKind::CharacterProfile.on_success_triggers(),
          [JobKind::AssetSync, JobKind::CharacterWallet],
          "a freshly-persisted character must re-fire its gather jobs immediately, not wait an interval"
        );
        assert_eq!(
          JobKind::CorporationProfile.on_success_triggers(),
          [JobKind::AssetSync, JobKind::CorporationWallet],
        );
      }

      #[test]
      fn it_chains_a_per_subject_kind_only_to_a_subject_its_source_shares() {
        for kind in JobKind::ALL.iter().copied() {
          if PROFILES.contains(&kind) {
            continue;
          }
          for &triggered in kind.on_success_triggers() {
            if triggered.is_global() {
              continue;
            }
            let shares_subject = [Subject::Character(1), Subject::Corporation(1)]
              .into_iter()
              .any(|subject| kind.applies_to(subject) && triggered.applies_to(subject));
            assert!(
              shares_subject,
              "{kind:?} chains per-subject {triggered:?}, but shares no subject for the engine to route it to"
            );
          }
        }
      }
    }

    mod is_global {
      use super::*;

      #[test]
      fn it_marks_only_subjectless_kinds_as_global() {
        assert!(JobKind::KillmailReconcile.is_global());
        assert!(JobKind::MarketPrices.is_global());
        assert!(JobKind::NetWorthSnapshot.is_global());

        assert!(!JobKind::AssetSync.is_global());
        assert!(!JobKind::CharacterAbyssals.is_global());
        assert!(!JobKind::CorporationWallet.is_global());
      }
    }

    mod scope_gating {
      use super::*;
      use crate::clients::esi::scopes;

      #[test]
      fn it_enrolls_only_kinds_the_partial_grant_covers() {
        let subject = Subject::Character(7);
        let granted: HashSet<&str> = [scopes::CHARACTER_WALLET].into_iter().collect();

        let kinds = JobKind::granted_for_subject(subject, &granted);

        assert!(
          kinds.contains(&JobKind::CharacterWallet),
          "the granted wallet job is enrolled, got {kinds:?}"
        );
        assert!(
          kinds.contains(&JobKind::CharacterProfile),
          "the public profile job needs no scope and is always enrolled, got {kinds:?}"
        );
        assert!(
          !kinds.contains(&JobKind::CharacterMarketOrders),
          "an ungranted scope must not be enrolled, sparing the permanent 401 loop, got {kinds:?}"
        );
      }

      #[test]
      fn it_never_grants_market_orders_since_that_scope_is_never_requested() {
        let subject = Subject::Character(7);
        let everything: HashSet<&str> = crate::features::auth::scopes_for(&crate::config::Feature::ALL)
          .into_iter()
          .collect();

        assert!(
          !JobKind::CharacterMarketOrders.is_scope_granted(subject, &everything),
          "esi-markets.read_character_orders.v1 is requested by no feature, so the job can never run"
        );
      }

      #[test]
      fn it_picks_the_subject_relevant_asset_scope() {
        let character_grant: HashSet<&str> = [scopes::CHARACTER_ASSETS].into_iter().collect();
        let corp_grant: HashSet<&str> = [scopes::CORPORATION_ASSETS].into_iter().collect();

        assert!(JobKind::AssetSync.is_scope_granted(Subject::Character(7), &character_grant));
        assert!(!JobKind::AssetSync.is_scope_granted(Subject::Character(7), &corp_grant));
        assert!(JobKind::AssetSync.is_scope_granted(Subject::Corporation(2), &corp_grant));
      }
    }
  }
}
