# Release process

How we cut a release of Pod. The short version: we publish **release candidates**, wait for CI to pass on a candidate,
and only then publish the real release tag on that **same commit**. We never move or delete a tag.

This describes the process and the operations involved, independent of which version-control tool you drive it with.
The operations ("tag this commit", "push the tag", "advance the main branch") are the same whether you use git, jj, or
anything else; only the exact command syntax differs.

## Why release candidates

A release is triggered by pushing a version tag (for example `0.6.7`). Two GitHub Actions run on that tag:

- **Build** (`.github/workflows/build.yml`) — lint, type/compile check, the test suite, the telemetry contract
  conformance job, and a packaged build on Linux, macOS, and Windows.
- **Release** (`.github/workflows/release.yml`) — runs the full package gauntlet (macOS notarization, Windows
  installers, Linux AppImage/Flatpak/pacman) in parallel with Build. For a real release it also creates the GitHub
  release and, once Build has passed, generates the updater manifest, publishes the release, and deploys the site.

We used to push the version tag directly. When a release failed, we would delete the tag, push a fix commit, and
re-create the **same** tag pointing at the new commit. That is hostile to every version-control tool and to anyone
consuming the tag:

- Moving a tag is a force update. Anyone who already fetched the old tag keeps a stale, now-divergent reference, and a
  normal fetch will not update it without forcing.
- Release artifacts, checksums, and any external link to the tag can end up pointing at a commit that no longer matches
  what the tag names.
- The record of "what was actually released" becomes ambiguous, because the tag's target changed over time.

Tags are meant to be immutable. So instead of mutating a release tag, we never publish a release tag until we already
know the underlying commit passes. Release candidates are how we prove a commit before naming it the release.

## How the tag controls the release

The Release workflow looks at the tag name:

- A tag containing `-rc.` (for example `0.6.7-rc.1`) is a **release candidate**. It runs the full build and package
  gauntlet and uploads the installers as throwaway workflow artifacts, but it does **not** create a GitHub release,
  generate the updater manifest, or deploy the site. Those steps are intentionally skipped, so seeing them skipped on a
  candidate is expected, not a failure.
- A bare numeric tag (for example `0.6.7`) is the **real release**. It does everything a candidate does, and then
  creates the release, uploads the installers to it, publishes the updater manifest, and deploys the site.

Because the real tag does strictly more than a candidate, we first prove the commit as a candidate, then publish the
real tag on the exact same commit.

## The flow

1. **Prepare the release commit.** Bump the version, finalize the changelog entry for the new version, and commit
   (conventionally `chore: prepare vX.Y.Z release`). Make this commit the head of the main branch.

2. **Cut the first release candidate.** Tag the prepare commit `X.Y.Z-rc.1` and push the branch and the tag. Both Build
   and Release run on the candidate (the workflows trigger on any tag starting with a digit, so `-rc.N` tags qualify).

3. **Watch both actions.** You want **both** Build and Release to finish green on the candidate. For a candidate, the
   release-only jobs (create release, publish, deploy) are skipped; the run still counts as success when the build and
   package jobs pass.

   - **If either action fails for a real reason**, fix it on a new commit, then cut the **next** candidate. Increment
     the rc number every time; never reuse a candidate number. Each fix gets its own candidate (`-rc.2`, `-rc.3`, ...).

   - **If a job fails on the known "download icons" step** (the installer-packaging jobs download the icon set produced
     earlier in the same run, and that download occasionally flakes on Windows), that is a flake, not a code problem.
     Just re-run the failed jobs of that run. Do **not** cut a new candidate, and do **not** change any code.

4. **Publish the real release.** Once a candidate passes both actions, tag the real release on the **exact same commit**
   the passing candidate points at, and push it. The first passing candidate and the real release are always the same
   commit, with no commits added in between.

5. **Watch the real tag's actions too.** Pushing `X.Y.Z` re-runs Build and Release on that commit. They should pass,
   because the identical commit already passed as the candidate. If a job hits the "download icons" flake again, re-run
   it; the commit is already proven, so no new candidate is needed.

## Rules of thumb

- **Never move or delete a release tag.** If something is wrong, the answer is a new candidate, not a re-pointed tag.
- **The first passing candidate and the real release point at the same commit.** Do not add commits between proving the
  candidate and tagging the release.
- **Increment the candidate number on every fix.** `-rc.1`, `-rc.2`, `-rc.3`, ... Each candidate is immutable too.
- **The "download icons" failure is a flake.** Re-run the failed jobs; do not bump the candidate.
- **Both Build and Release must be green** on a candidate before the real tag goes out.

## Command reference

The operations are tool-independent. Git commands are shown as the baseline; if you drive the repo with another tool
(such as jj), run the equivalent operation it provides.

```sh
# First candidate (after the prepare commit is the head of main):
git tag X.Y.Z-rc.1
git push origin main --tags

# A candidate failed for a real reason -> commit the fix, advance main, cut the next candidate:
git commit -am "<fix message>"
git tag X.Y.Z-rc.2          # rc.3, rc.4, ... as needed
git push origin main --tags

# A candidate passed both actions -> publish the real release on the same commit:
git tag X.Y.Z               # same commit the passing candidate points at
git push origin main --tags

# "download icons" flake -> just re-run the failed jobs, no new candidate:
gh run rerun <run-id> --failed
```

> [!TIP]
> Driving the repo with jj instead of git? The equivalents are `jj bookmark` to advance `main`, `jj tag set <name> -r
> <change-id>` to place the tag, and `jj git push && git push origin --tags` to publish. The process above is identical.
