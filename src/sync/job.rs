use std::{collections::HashSet, time::Duration};

use super::{outcome::Outcome, subject::Subject};
use crate::{
  clients::{self, esi, esi::scopes, eve_image, eve_sso, eve_sso::Grant},
  config::{Feature, FeatureFlags, SubFeature},
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
  /// Pushes locally-managed sync-list contacts to the character's live ESI contacts (write direction);
  /// unlike `CharacterContacts`, which only reads them, this requires the contacts-write scope.
  CharacterContactSync,
  CharacterContracts,
  CharacterIndustryJobs,
  CharacterKillmails,
  CharacterMail,
  CharacterMarketOrders,
  CharacterNotifications,
  CharacterPlanets,
  CharacterProfile,
  CharacterSkills,
  CharacterStandings,
  CharacterTelemetry,
  CharacterWallet,
  CorporationAbyssals,
  CorporationBlueprints,
  CorporationContacts,
  CorporationContracts,
  CorporationCustomsOffices,
  CorporationIndustryJobs,
  CorporationKillmails,
  CorporationMarketOrders,
  CorporationMiningExtractions,
  CorporationProfile,
  CorporationStandings,
  CorporationStructures,
  CorporationWallet,
  IndustryCostIndices,
  KillmailDetailBackfill,
  KillmailReconcile,
  MarketPrices,
  NetWorthSnapshot,
  TokenAudit,
  WalletJournalReconcile,
}

impl JobKind {
  pub const ALL: &'static [JobKind] = &[
    JobKind::AssetSync,
    JobKind::CharacterAbyssals,
    JobKind::CharacterBlueprints,
    JobKind::CharacterCalendar,
    JobKind::CharacterClones,
    JobKind::CharacterContacts,
    JobKind::CharacterContactSync,
    JobKind::CharacterContracts,
    JobKind::CharacterIndustryJobs,
    JobKind::CharacterKillmails,
    JobKind::CharacterMail,
    JobKind::CharacterMarketOrders,
    JobKind::CharacterNotifications,
    JobKind::CharacterPlanets,
    JobKind::CharacterProfile,
    JobKind::CharacterSkills,
    JobKind::CharacterStandings,
    JobKind::CharacterTelemetry,
    JobKind::CharacterWallet,
    JobKind::CorporationAbyssals,
    JobKind::CorporationBlueprints,
    JobKind::CorporationContacts,
    JobKind::CorporationContracts,
    JobKind::CorporationCustomsOffices,
    JobKind::CorporationIndustryJobs,
    JobKind::CorporationKillmails,
    JobKind::CorporationMarketOrders,
    JobKind::CorporationMiningExtractions,
    JobKind::CorporationProfile,
    JobKind::CorporationStandings,
    JobKind::CorporationStructures,
    JobKind::CorporationWallet,
    JobKind::IndustryCostIndices,
    JobKind::KillmailDetailBackfill,
    JobKind::KillmailReconcile,
    JobKind::MarketPrices,
    JobKind::NetWorthSnapshot,
    JobKind::TokenAudit,
    JobKind::WalletJournalReconcile,
  ];

  pub fn for_subject(subject: Subject) -> Vec<JobKind> {
    Self::ALL
      .iter()
      .copied()
      .filter(|kind| kind.applies_to(subject))
      .collect()
  }

  // Job-to-feature roll-up kept live for `registry::feature_for_job`; awaiting a UI consumer.
  #[expect(dead_code)]
  pub fn feature(self) -> Option<Feature> {
    crate::features::shell::registry::feature_for_job(self)
  }

  pub fn owning_sub_features(self) -> Vec<SubFeature> {
    crate::features::shell::registry::sub_features_for_job(self)
  }

  pub fn is_feature_enabled(self, features: &FeatureFlags) -> bool {
    let owners = self.owning_sub_features();
    owners.is_empty() || owners.iter().any(|&sub| features.is_sub_enabled(sub))
  }

  pub fn is_global(self) -> bool {
    !self.applies_to(Subject::Character(0)) && !self.applies_to(Subject::Corporation(0))
  }

  pub fn is_language_dependent(self) -> bool {
    // Exhaustive on purpose: a new JobKind must consciously pick a side. See ADR-0041 section 1 for
    // why these jobs (and not the language-invariant universe/names path) carry localized text.
    match self {
      Self::AssetSync
      | Self::CharacterClones
      | Self::CharacterContacts
      | Self::CharacterContracts
      | Self::CharacterKillmails
      | Self::CharacterProfile
      | Self::CharacterSkills
      | Self::CharacterStandings
      | Self::CharacterTelemetry
      | Self::CorporationContacts
      | Self::CorporationContracts
      | Self::CorporationKillmails
      | Self::CorporationProfile
      | Self::CorporationStandings
      | Self::CorporationStructures => true,
      Self::CharacterAbyssals
      | Self::CharacterBlueprints
      | Self::CharacterCalendar
      | Self::CharacterContactSync
      | Self::CharacterIndustryJobs
      | Self::CharacterMail
      | Self::CharacterMarketOrders
      | Self::CharacterNotifications
      | Self::CharacterPlanets
      | Self::CharacterWallet
      | Self::CorporationAbyssals
      | Self::CorporationBlueprints
      | Self::CorporationCustomsOffices
      | Self::CorporationIndustryJobs
      | Self::CorporationMarketOrders
      | Self::CorporationMiningExtractions
      | Self::CorporationWallet
      | Self::IndustryCostIndices
      | Self::KillmailDetailBackfill
      | Self::KillmailReconcile
      | Self::MarketPrices
      | Self::NetWorthSnapshot
      | Self::TokenAudit
      | Self::WalletJournalReconcile => false,
    }
  }

  pub fn on_success_triggers(self) -> &'static [JobKind] {
    match self {
      Self::AssetSync => &[
        JobKind::MarketPrices,
        JobKind::CharacterAbyssals,
        JobKind::CorporationAbyssals,
      ],
      Self::CharacterWallet | Self::CorporationWallet => &[JobKind::MarketPrices, JobKind::WalletJournalReconcile],
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
      (Self::CharacterPlanets, _) => Some(scopes::CHARACTER_PLANETS),
      (Self::CharacterSkills, _) => Some(scopes::CHARACTER_SKILLS),
      (Self::CharacterStandings, _) => Some(scopes::CHARACTER_STANDINGS),
      (Self::CharacterTelemetry, _) => Some(scopes::CHARACTER_LOCATION),
      (Self::CharacterWallet, _) => Some(scopes::CHARACTER_WALLET),
      (Self::CorporationBlueprints, _) => Some(scopes::CORPORATION_BLUEPRINTS),
      (Self::CorporationContacts, _) => Some(scopes::CORPORATION_CONTACTS),
      (Self::CorporationContracts, _) => Some(scopes::CORPORATION_CONTRACTS),
      (Self::CorporationCustomsOffices, _) => Some(scopes::CORPORATION_CUSTOMS_OFFICES),
      (Self::CorporationIndustryJobs, _) => Some(scopes::CORPORATION_INDUSTRY_JOBS),
      (Self::CorporationKillmails, _) => Some(scopes::CORPORATION_KILLMAILS),
      (Self::CorporationMarketOrders, _) => Some(scopes::CORPORATION_ORDERS),
      (Self::CorporationMiningExtractions, _) => Some(scopes::CORPORATION_MINING_EXTRACTIONS),
      (Self::CorporationProfile, _) => Some(scopes::CORPORATION_ROLES),
      (Self::CorporationStandings, _) => Some(scopes::CORPORATION_STANDINGS),
      (Self::CorporationStructures, _) => Some(scopes::CORPORATION_STRUCTURES),
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
          | Self::CharacterContactSync
          | Self::CharacterContracts
          | Self::CharacterIndustryJobs
          | Self::CharacterKillmails
          | Self::CharacterMail
          | Self::CharacterMarketOrders
          | Self::CharacterNotifications
          | Self::CharacterPlanets
          | Self::CharacterProfile
          | Self::CharacterSkills
          | Self::CharacterStandings
          | Self::CharacterTelemetry
          | Self::CharacterWallet,
        Subject::Character(_)
      ) | (
        Self::AssetSync
          | Self::CorporationAbyssals
          | Self::CorporationBlueprints
          | Self::CorporationContacts
          | Self::CorporationContracts
          | Self::CorporationCustomsOffices
          | Self::CorporationIndustryJobs
          | Self::CorporationKillmails
          | Self::CorporationMarketOrders
          | Self::CorporationMiningExtractions
          | Self::CorporationProfile
          | Self::CorporationStandings
          | Self::CorporationStructures
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
      | Self::CharacterPlanets
      | Self::CharacterProfile
      | Self::CharacterStandings
      | Self::CharacterWallet
      | Self::CorporationAbyssals
      | Self::CorporationBlueprints
      | Self::CorporationContacts
      | Self::CorporationContracts
      | Self::CorporationIndustryJobs
      | Self::CorporationMarketOrders
      | Self::CorporationProfile
      | Self::CorporationStandings
      | Self::CorporationWallet
      | Self::IndustryCostIndices => Duration::from_secs(3600),
      Self::CorporationCustomsOffices => Duration::from_secs(3600),
      Self::CorporationStructures => Duration::from_secs(3600),
      Self::CorporationMiningExtractions => Duration::from_secs(1800),
      Self::CharacterSkills => Duration::from_secs(60),
      Self::CharacterCalendar
      | Self::CharacterContactSync
      | Self::CharacterKillmails
      | Self::CharacterMail
      | Self::CharacterNotifications
      | Self::CharacterTelemetry
      | Self::CorporationKillmails => Duration::from_secs(300),
      Self::KillmailDetailBackfill | Self::KillmailReconcile | Self::MarketPrices | Self::WalletJournalReconcile => {
        Duration::from_secs(6 * 3600)
      }
      Self::NetWorthSnapshot => Duration::from_secs(24 * 3600),
      // Re-validate every stored token and re-check feature scopes every 20 minutes (tunable). A
      // revoked refresh token or a newly-enabled feature needing an ungranted scope is caught within
      // one interval rather than waiting for a card load or never being noticed at all.
      Self::TokenAudit => Duration::from_secs(1200),
    }
  }

  pub fn required_scope(self) -> &'static [&'static str] {
    match self {
      Self::AssetSync => &[scopes::CHARACTER_ASSETS, scopes::CORPORATION_ASSETS],
      Self::CharacterBlueprints => &[scopes::CHARACTER_BLUEPRINTS],
      Self::CharacterCalendar => &[scopes::CHARACTER_CALENDAR_READ],
      Self::CharacterClones => &[scopes::CHARACTER_CLONES, scopes::CHARACTER_IMPLANTS],
      Self::CharacterContacts => &[scopes::CHARACTER_CONTACTS],
      Self::CharacterContactSync => &[scopes::CHARACTER_CONTACTS_WRITE],
      Self::CharacterContracts => &[scopes::CHARACTER_CONTRACTS],
      Self::CharacterIndustryJobs => &[scopes::CHARACTER_INDUSTRY_JOBS],
      Self::CharacterKillmails => &[scopes::CHARACTER_KILLMAILS, scopes::CORPORATION_KILLMAILS],
      Self::CharacterMail => &[scopes::CHARACTER_MAIL],
      Self::CharacterMarketOrders => &[scopes::CHARACTER_ORDERS],
      Self::CharacterNotifications => &[scopes::CHARACTER_NOTIFICATIONS],
      Self::CharacterPlanets => &[scopes::CHARACTER_PLANETS],
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
      Self::CorporationContacts => &[scopes::CORPORATION_CONTACTS],
      Self::CorporationContracts => &[scopes::CORPORATION_CONTRACTS],
      Self::CorporationCustomsOffices => &[scopes::CORPORATION_ROLES, scopes::CORPORATION_CUSTOMS_OFFICES],
      Self::CorporationIndustryJobs => &[scopes::CORPORATION_ROLES, scopes::CORPORATION_INDUSTRY_JOBS],
      Self::CorporationKillmails => &[scopes::CORPORATION_KILLMAILS],
      Self::CorporationMarketOrders => &[scopes::CORPORATION_ROLES, scopes::CORPORATION_ORDERS],
      Self::CorporationMiningExtractions => &[scopes::CORPORATION_ROLES, scopes::CORPORATION_MINING_EXTRACTIONS],
      Self::CorporationProfile => &[scopes::CORPORATION_ROLES],
      Self::CorporationStandings => &[scopes::CORPORATION_STANDINGS],
      Self::CorporationStructures => &[scopes::CORPORATION_ROLES, scopes::CORPORATION_STRUCTURES],
      Self::CorporationWallet => &[
        scopes::CORPORATION_ROLES,
        scopes::CORPORATION_WALLET,
        scopes::CORPORATION_DIVISIONS,
      ],
      Self::CharacterAbyssals
      | Self::CharacterProfile
      | Self::CorporationAbyssals
      | Self::IndustryCostIndices
      | Self::KillmailDetailBackfill
      | Self::KillmailReconcile
      | Self::MarketPrices
      | Self::NetWorthSnapshot
      | Self::TokenAudit
      | Self::WalletJournalReconcile => &[],
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
  pub sso: Option<&'a eve_sso::Client>,
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
    JobKind::CharacterContactSync => super::jobs::contact_sync::run(ctx).await,
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
    JobKind::CharacterPlanets => super::jobs::character_planets::run(ctx).await,
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
    JobKind::CorporationAbyssals => super::jobs::abyssals::run(ctx).await,
    JobKind::CorporationBlueprints => super::jobs::blueprints::run(ctx).await,
    JobKind::CorporationIndustryJobs => super::jobs::industry::run(ctx).await,
    JobKind::KillmailDetailBackfill => super::jobs::killmail_detail_backfill::run(ctx).await,
    JobKind::KillmailReconcile => super::jobs::killmail_reconcile::run(ctx).await,
    JobKind::CorporationContacts => super::jobs::corporation_contacts::run(ctx).await,
    JobKind::CorporationContracts => super::jobs::corporation_contracts::run(ctx).await,
    JobKind::CorporationCustomsOffices => super::jobs::corporation_customs_offices::run(ctx).await,
    JobKind::CorporationKillmails => super::jobs::corporation_killmails::run(ctx).await,
    JobKind::CorporationMarketOrders => super::jobs::corporation_market_orders::run(ctx).await,
    JobKind::CorporationMiningExtractions => super::jobs::mining_extractions::run(ctx).await,
    JobKind::CorporationProfile => super::jobs::corporation_profile::run(ctx).await,
    JobKind::CorporationStandings => super::jobs::corporation_standings::run(ctx).await,
    JobKind::CorporationStructures => super::jobs::corporation_structures::run(ctx).await,
    JobKind::CorporationWallet => super::jobs::corporation_wallet::run(ctx).await,
    JobKind::IndustryCostIndices => super::jobs::industry_cost_indices::run(ctx).await,
    JobKind::MarketPrices => super::jobs::market_prices::run(ctx).await,
    JobKind::NetWorthSnapshot => super::jobs::net_worth_snapshot::run(ctx).await,
    JobKind::TokenAudit => super::jobs::token_audit::run(ctx).await,
    JobKind::WalletJournalReconcile => super::jobs::wallet_journal_reconcile::run(ctx).await,
    _ => Ok(Outcome::synced()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod job_kind {
    use super::*;

    #[test]
    fn it_resolves_a_gating_scope_arm_for_every_kind_and_subject() {
      for kind in JobKind::ALL.iter().copied() {
        let _ = kind.gating_scope(Subject::Character(1));
        let _ = kind.gating_scope(Subject::Corporation(2));
      }
    }

    mod is_global {
      use super::*;

      #[test]
      fn it_marks_only_subjectless_kinds_as_global() {
        assert!(JobKind::KillmailReconcile.is_global());
        assert!(JobKind::MarketPrices.is_global());
        assert!(JobKind::NetWorthSnapshot.is_global());
        assert!(JobKind::WalletJournalReconcile.is_global());

        assert!(!JobKind::AssetSync.is_global());
        assert!(!JobKind::CharacterAbyssals.is_global());
        assert!(!JobKind::CorporationWallet.is_global());
      }
    }

    mod is_language_dependent {
      use pretty_assertions::assert_eq;

      use super::*;

      const LANGUAGE_DEPENDENT: [JobKind; 15] = [
        JobKind::AssetSync,
        JobKind::CharacterClones,
        JobKind::CharacterContacts,
        JobKind::CharacterContracts,
        JobKind::CharacterKillmails,
        JobKind::CharacterProfile,
        JobKind::CharacterSkills,
        JobKind::CharacterStandings,
        JobKind::CharacterTelemetry,
        JobKind::CorporationContacts,
        JobKind::CorporationContracts,
        JobKind::CorporationKillmails,
        JobKind::CorporationProfile,
        JobKind::CorporationStandings,
        JobKind::CorporationStructures,
      ];

      #[test]
      fn it_classifies_every_kind() {
        for kind in JobKind::ALL.iter().copied() {
          let expected = LANGUAGE_DEPENDENT.contains(&kind);

          assert_eq!(
            kind.is_language_dependent(),
            expected,
            "{kind:?} language-dependence must match its ADR-0041 section 1 membership"
          );
        }
      }

      #[test]
      fn it_excludes_language_invariant_jobs() {
        assert!(
          !JobKind::MarketPrices.is_language_dependent(),
          "prices are numeric and carry no localized text"
        );
        assert!(
          !JobKind::NetWorthSnapshot.is_language_dependent(),
          "the snapshot is numeric and carries no localized text"
        );
        assert!(
          !JobKind::CharacterWallet.is_language_dependent(),
          "wallet amounts are language-neutral"
        );
        assert!(
          !JobKind::CharacterMail.is_language_dependent(),
          "mail bodies are user-authored, never ESI-localized"
        );
      }

      #[test]
      fn it_marks_the_resolver_backed_jobs_as_language_dependent() {
        for kind in LANGUAGE_DEPENDENT {
          assert!(
            kind.is_language_dependent(),
            "{kind:?} persists localized reference text and must re-sync on a language switch"
          );
        }
      }
    }

    mod on_success_triggers {
      use pretty_assertions::assert_eq;

      use super::*;

      const GATHERS: [JobKind; 3] = [JobKind::AssetSync, JobKind::CharacterWallet, JobKind::CorporationWallet];

      const PROFILES: [JobKind; 2] = [JobKind::CharacterProfile, JobKind::CorporationProfile];

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

      #[test]
      fn it_chains_asset_sync_to_prices_and_its_derived_abyssals() {
        assert_eq!(
          JobKind::AssetSync.on_success_triggers(),
          [
            JobKind::MarketPrices,
            JobKind::CharacterAbyssals,
            JobKind::CorporationAbyssals
          ],
          "a fresh asset sync must re-fire the per-subject abyssal jobs, which are derived from the asset table",
        );
        assert!(
          JobKind::CharacterAbyssals.applies_to(Subject::Character(1)),
          "char abyssals is per-subject, so the engine routes it to the same character that synced assets"
        );
        assert!(
          !JobKind::CharacterAbyssals.applies_to(Subject::Corporation(1)),
          "char abyssals never routes to a corporation asset sync"
        );
        assert!(
          JobKind::CorporationAbyssals.applies_to(Subject::Corporation(1)),
          "corp abyssals is per-subject, so the engine routes it to the same corporation that synced assets"
        );
        assert!(
          !JobKind::CorporationAbyssals.applies_to(Subject::Character(1)),
          "corp abyssals never routes to a character asset sync"
        );
      }

      #[test]
      fn it_chains_prices_to_the_snapshot() {
        assert_eq!(JobKind::MarketPrices.on_success_triggers(), [JobKind::NetWorthSnapshot]);
      }

      #[test]
      fn it_chains_wallet_gathers_to_prices_and_reconcile() {
        for gather in [JobKind::CharacterWallet, JobKind::CorporationWallet] {
          assert_eq!(
            gather.on_success_triggers(),
            [JobKind::MarketPrices, JobKind::WalletJournalReconcile],
            "{gather:?} chains prices (then the snapshot) and the gap-detection reconcile",
          );
        }
      }

      #[test]
      fn it_chains_the_gap_detection_reconcile_off_every_wallet_sync() {
        for gather in [JobKind::CharacterWallet, JobKind::CorporationWallet] {
          assert!(
            gather.on_success_triggers().contains(&JobKind::WalletJournalReconcile),
            "{gather:?} must re-run balance-continuity gap detection after the re-fetch settles",
          );
        }
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
    }

    mod required_scope {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_pairs_corporation_jobs_with_the_roles_scope() {
        assert!(
          JobKind::CorporationWallet
            .required_scope()
            .contains(&scopes::CORPORATION_ROLES)
        );
        assert!(
          JobKind::CorporationBlueprints
            .required_scope()
            .contains(&scopes::CORPORATION_ROLES)
        );
        assert!(
          JobKind::CharacterWallet
            .required_scope()
            .contains(&scopes::CHARACTER_WALLET)
        );
      }

      #[test]
      fn it_resolves_a_static_scope_list_for_every_job_kind() {
        for kind in JobKind::ALL.iter().copied() {
          let scopes = kind.required_scope();
          let is_public = matches!(
            kind,
            JobKind::CharacterAbyssals
              | JobKind::CorporationAbyssals
              | JobKind::CharacterProfile
              | JobKind::IndustryCostIndices
              | JobKind::KillmailDetailBackfill
              | JobKind::KillmailReconcile
              | JobKind::MarketPrices
              | JobKind::NetWorthSnapshot
              | JobKind::TokenAudit
              | JobKind::WalletJournalReconcile
          );

          assert_eq!(
            scopes.is_empty(),
            is_public,
            "{kind:?} scope-emptiness must match its public status"
          );
        }
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
      fn it_lights_up_market_orders_because_the_orders_scope_is_now_requested() {
        let subject = Subject::Character(7);
        let everything: HashSet<&str> =
          crate::features::roster::auth::scopes_for(&crate::config::FeatureFlags::default())
            .into_iter()
            .collect();

        assert!(
          JobKind::CharacterMarketOrders.is_scope_granted(subject, &everything),
          "Pod now requests esi-markets.read_character_orders.v1, so the market-orders job is enrolled"
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

  mod run {
    use wiremock::MockServer;

    use super::*;
    use crate::{
      clients::{eve_image, eve_sso::Grant, http},
      store::{self, images},
    };

    const SUBJECT_ID: i64 = 42;

    const PROFILE_CREATORS: [JobKind; 2] = [JobKind::CharacterProfile, JobKind::CorporationProfile];

    fn subject_bound_jobs() -> Vec<(JobKind, Subject)> {
      let mut jobs = Vec::new();
      for kind in JobKind::ALL.iter().copied() {
        if PROFILE_CREATORS.contains(&kind) {
          continue;
        }
        for subject in [Subject::Character(SUBJECT_ID), Subject::Corporation(SUBJECT_ID)] {
          if kind.applies_to(subject) {
            jobs.push((kind, subject));
          }
        }
      }
      jobs
    }

    #[tokio::test]
    async fn it_returns_not_ready_for_every_subject_bound_job_when_the_parent_is_absent() {
      let server = MockServer::start().await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let esi = esi::Client::with_base_url(http.clone(), server.uri());
      let image = eve_image::Client::with_base_url(http, server.uri());
      let images_dir = tempfile::tempdir().unwrap();
      let image_store = images::Store::new(images_dir.path().to_path_buf());
      let grant = Grant::new_test("token", SUBJECT_ID);

      let jobs = subject_bound_jobs();
      assert!(
        !jobs.is_empty(),
        "the enumeration must cover at least one subject-bound job"
      );

      for (kind, subject) in jobs {
        let ctx = JobCtx {
          db: &db,
          esi: &esi,
          grant: Some(&grant),
          image: &image,
          image_store: &image_store,
          key: JobKey::new(kind, subject),
          sso: None,
        };

        let result = run(&ctx).await;

        assert!(
          matches!(result, Err(clients::Error::NotReady)),
          "{kind:?} on {subject:?} must guard the missing parent with NotReady, got {result:?}"
        );
      }
    }
  }
}
