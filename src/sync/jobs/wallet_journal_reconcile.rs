use crate::{
  clients::Error,
  store::{
    Database,
    model::{
      CharacterWalletJournal, CorporationWalletJournal, NewNotification, NotificationDestination, NotificationKind,
      NotificationOwner, NotificationTarget,
    },
    repo::{character, finance, notifications, org},
  },
  sync::{job::JobCtx, outcome::Outcome},
};

// EVE stores ISK to two decimals, so a real dropped journal entry shifts the running balance by far
// more than rounding noise. A small absolute tolerance absorbs float drift while still catching a
// missing entry; it matches the transfer-netting epsilon the budget engine already uses.
const BALANCE_EPSILON: f64 = 0.5;

const CORPORATION_DIVISIONS: std::ops::RangeInclusive<i64> = 1..=7;

// One adjacent-row balance discontinuity: the ids that bracket a missing entry. `before` is the last
// entry whose post-entry balance was known; `after` is the next entry whose balance no longer agrees
// with `before`'s balance plus its own amount, so at least one entry between them never reached the
// table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Gap {
  after: i64,
  before: i64,
}

// Post-sync balance-continuity gap detection (ADR-0040 section 5). Runs as a global job chained off
// every CharacterWallet / CorporationWallet sync, after the one-time forced re-fetch has had a chance
// to re-pull and close any gap inside ESI's retention window. For each wallet (a character, and each
// corporation wallet division) it walks the journal in id order and checks that each entry's running
// balance equals the prior balance plus this entry's amount; a break means an entry is missing (a leg
// that aged out before the re-fetch, or a page ESI no longer serves) and is surfaced once via a stable
// dedup_key so it never re-nags.
pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let mut surfaced = 0_usize;

  for character in character::all(ctx.db).await? {
    let owner = NotificationOwner::Character(character.id());
    let mut entries = finance::wallet_journal(ctx.db, character.id()).await?;
    entries.reverse();
    surfaced += emit_gaps(ctx.db, owner, None, &character_continuity(&entries)).await?;
  }

  for corporation in org::all_owned_corporations(ctx.db).await? {
    let owner = NotificationOwner::Corporation(corporation.id());
    for division in CORPORATION_DIVISIONS {
      let mut entries = finance::corporation_wallet_journal(ctx.db, corporation.id(), division).await?;
      entries.reverse();
      surfaced += emit_gaps(ctx.db, owner, Some(division), &corporation_continuity(&entries)).await?;
    }
  }

  Ok(Outcome::from_rows(surfaced))
}

fn character_continuity(entries: &[CharacterWalletJournal]) -> Vec<Gap> {
  find_gaps(
    entries
      .iter()
      .map(|entry| (entry.id(), entry.amount(), entry.balance())),
  )
}

fn corporation_continuity(entries: &[CorporationWalletJournal]) -> Vec<Gap> {
  find_gaps(
    entries
      .iter()
      .map(|entry| (entry.id(), entry.amount(), entry.balance())),
  )
}

// Walk the journal in id order and flag every adjacent pair of balance-bearing entries whose running
// balance breaks `balance(n) == balance(n-1) + amount(n)`. Entries with a null balance or amount are
// skipped (some ref types omit them) rather than treated as a gap, so a comparison always runs against
// the most recent entry that carried a balance. The first balance-bearing entry has no predecessor and
// is never flagged, so a wallet's genesis entry never reports a gap.
fn find_gaps(entries: impl Iterator<Item = (i64, Option<f64>, Option<f64>)>) -> Vec<Gap> {
  let mut gaps = Vec::new();
  let mut previous: Option<(i64, f64)> = None;
  for (id, amount, balance) in entries {
    let (Some(amount), Some(balance)) = (amount, balance) else {
      continue;
    };
    if let Some((before, before_balance)) = previous
      && ((balance - before_balance) - amount).abs() > BALANCE_EPSILON
    {
      gaps.push(Gap {
        after: id,
        before,
      });
    }
    previous = Some((id, balance));
  }
  gaps
}

async fn emit_gaps(
  db: &Database,
  owner: NotificationOwner,
  division: Option<i64>,
  gaps: &[Gap],
) -> Result<usize, Error> {
  let mut surfaced = 0;
  for gap in gaps {
    let emitted = notifications::emit(
      db,
      &NewNotification {
        body: format!(
          "A wallet entry between {} and {} is missing and predates ESI's retention window, so it \
          cannot be recovered automatically.",
          gap.before, gap.after
        ),
        dedup_key: dedup_key(owner, division, gap),
        kind: NotificationKind::WalletGap,
        owner,
        target: NotificationTarget {
          character: match owner {
            NotificationOwner::Character(id) => Some(id),
            NotificationOwner::Corporation(_) => None,
          },
          destination: NotificationDestination::Wallet,
          sub: None,
        },
        title: t!("shell.notification.wallet_gap_title").into_owned(),
      },
    )
    .await?;
    if emitted.is_some() {
      surfaced += 1;
    }
  }
  Ok(surfaced)
}

// A per-wallet, per-gap-location key so the same discontinuity surfaces once and never re-nags on a
// later sync: it carries the owner identity (and the division, which is the wallet of a corporation)
// plus the ids that bracket the gap. A new gap at a different location takes a distinct key and still
// surfaces.
fn dedup_key(owner: NotificationOwner, division: Option<i64>, gap: &Gap) -> String {
  match division {
    Some(division) => format!(
      "wallet_gap:{}:{}:div{}:{}-{}",
      owner.owner_type(),
      owner.owner_id(),
      division,
      gap.before,
      gap.after
    ),
    None => format!(
      "wallet_gap:{}:{}:{}-{}",
      owner.owner_type(),
      owner.owner_id(),
      gap.before,
      gap.after
    ),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    clients::{esi, eve_image, http},
    store::{self, Database, images},
    sync::{
      job::{JobKey, JobKind},
      subject::Subject,
    },
  };

  async fn seed_character(db: &Database, id: i64) {
    use store::{
      model::{Alliance, Bloodline, Character, Corporation, Gender, Race},
      repo::character,
    };
    let corp_id = 90_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  async fn seed_journal(db: &Database, character_id: i64, id: i64, amount: Option<f64>, balance: Option<f64>) {
    sqlx::query(
      "INSERT INTO character_wallet_journal (id, character_id, date, description, ref_type, amount, balance) \
        VALUES (?, ?, '2026-06-01T00:00:00Z', '', 'player_donation', ?, ?)",
    )
    .bind(id)
    .bind(character_id)
    .bind(amount)
    .bind(balance)
    .execute(db.writer())
    .await
    .unwrap();
  }

  async fn run_job(db: &Database) -> Outcome {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    let esi = esi::Client::with_base_url(http.clone(), "http://localhost".to_owned());
    let image = eve_image::Client::with_base_url(http, "http://localhost".to_owned());
    let images_dir = tempfile::tempdir().unwrap();
    let image_store = images::Store::new(images_dir.path().to_path_buf());
    let ctx = JobCtx {
      db,
      esi: &esi,
      image: &image,
      image_store: &image_store,
      key: JobKey::new(JobKind::WalletJournalReconcile, Subject::Character(0)),
      grant: None,
      sso: None,
    };
    run(&ctx).await.unwrap()
  }

  async fn gap_keys(db: &Database) -> Vec<String> {
    notifications::list(db, 200)
      .await
      .unwrap()
      .into_iter()
      .filter(|n| n.kind() == NotificationKind::WalletGap)
      .map(|n| n.dedup_key().clone())
      .collect()
  }

  mod find_gaps {
    use pretty_assertions::assert_eq;

    use super::*;

    fn detect(entries: &[(i64, Option<f64>, Option<f64>)]) -> Vec<Gap> {
      find_gaps(entries.iter().copied())
    }

    #[test]
    fn it_flags_no_gap_for_a_continuous_ledger() {
      let entries = [
        (1, Some(100.0), Some(100.0)),
        (2, Some(-30.0), Some(70.0)),
        (3, Some(50.0), Some(120.0)),
      ];

      assert!(detect(&entries).is_empty());
    }

    #[test]
    fn it_flags_the_pair_bracketing_a_missing_entry() {
      // entry 2 (id 5) reports a balance that, given its amount, can only hold if an entry between
      // id 1 and id 5 is missing: 100 + (-30) = 70, but the balance jumped to 1070.
      let entries = [(1, Some(100.0), Some(100.0)), (5, Some(-30.0), Some(1070.0))];

      assert_eq!(
        detect(&entries),
        [Gap {
          after: 5,
          before: 1
        }]
      );
    }

    #[test]
    fn it_never_flags_the_genesis_entry() {
      // A single first-ever entry has no predecessor to compare against, however far its balance sits
      // from its amount, so it can never be a gap.
      let entries = [(1, Some(-9_000_000_000.0), Some(500.0))];

      assert!(detect(&entries).is_empty());
    }

    #[test]
    fn it_skips_rows_with_a_null_balance_or_amount() {
      // The middle row omits balance/amount (some ref types do); it is skipped, and the comparison
      // runs from id 1 straight to id 3, which stays continuous (100 + 25 = 125).
      let entries = [
        (1, Some(100.0), Some(100.0)),
        (2, None, None),
        (3, Some(25.0), Some(125.0)),
      ];

      assert!(detect(&entries).is_empty());
    }

    #[test]
    fn it_tolerates_sub_epsilon_float_drift() {
      let entries = [(1, Some(100.0), Some(100.0)), (2, Some(50.0), Some(150.4))];

      assert!(detect(&entries).is_empty());
    }
  }

  mod run {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_surfaces_no_gap_for_a_continuous_character_ledger() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_journal(&db, 42, 1, Some(100.0), Some(100.0)).await;
      seed_journal(&db, 42, 2, Some(-30.0), Some(70.0)).await;

      run_job(&db).await;

      assert!(gap_keys(&db).await.is_empty());
    }

    #[tokio::test]
    async fn it_surfaces_one_gap_with_a_stable_dedup_key_for_a_discontinuous_ledger() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_journal(&db, 42, 1, Some(100.0), Some(100.0)).await;
      seed_journal(&db, 42, 5, Some(-30.0), Some(1070.0)).await;

      run_job(&db).await;

      assert_eq!(gap_keys(&db).await, ["wallet_gap:character:42:1-5"]);
    }

    #[tokio::test]
    async fn it_is_idempotent_across_repeated_runs() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, 42).await;
      seed_journal(&db, 42, 1, Some(100.0), Some(100.0)).await;
      seed_journal(&db, 42, 5, Some(-30.0), Some(1070.0)).await;

      run_job(&db).await;
      let after_first = gap_keys(&db).await;
      run_job(&db).await;
      let after_second = gap_keys(&db).await;

      assert_eq!(after_first, ["wallet_gap:character:42:1-5"]);
      assert_eq!(
        after_first, after_second,
        "a second pass re-detects the same gap but its stable dedup_key keeps emit() a no-op"
      );
    }
  }
}
