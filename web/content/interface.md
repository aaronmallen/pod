---
title: The Interface
section: Guide
order: 2
description: Learn your way around Pod's main window — the navigation rail, the working area, the status bar, the sync popover, and the update banner — so you can read it at a glance.
---

# The Interface

Pod's main window has three persistent pieces: the navigation rail down one side,
the working area in the middle, and the status bar across the bottom. This page
explains each rail item, each status-bar indicator, the sync popover, and the
update banner, so you can read the window at a glance.

## The navigation rail

The rail is a narrow vertical strip, 68 pixels wide. By default it sits on the
left, but you can move it to the right side in Settings. The Pod mark sits at the
top, the feature buttons fill the body, and the Settings button is pinned to the
bottom with a small gap above it.

Each button is an icon, not a label. From top to bottom in the default order, the
feature buttons are Characters, Skills, Industry, Mail, Calendar, Wallet, and
Assets. You can reorder these seven buttons in Settings; the Pod mark and the
Settings button stay fixed. Click a button to switch the working area to that
screen.

### Active item and accent

The button for the screen you are on reads as active. Its icon brightens to the
primary text color, the icon cell takes a faint highlight, and a short vertical
plasma bar appears on the left edge of the cell. Every other button stays dimmed
until you hover or select it. There is one active button at a time.

### Unread badges

Two buttons can show an attention badge: a small plasma dot in the top-right
corner of the icon. Mail shows the dot whenever you have unread mail. Calendar
shows the dot whenever an event needs your attention, such as a pending response.
The dot is a yes-or-no signal and does not carry a number. It clears once the
underlying count returns to zero.

### Which buttons appear

Most rail buttons are tied to a feature you can turn off. Assets, Calendar,
Industry, Mail, Skills, and Wallet each belong to a feature. If you disable that
feature in Settings, its button disappears from the rail, and the screen behind it
is no longer reachable from the rail. Characters and Settings are not tied to any
feature, so they are always present. Turning a feature back on restores its button
in its previous position in the order.

### Characters and Settings

The Characters button opens the roster, where you add accounts, manage characters,
and open a character's detail view. It is the home screen and the place to pick
which pilot you are looking at. The Settings button at the bottom of the rail opens
the settings window. Settings groups its own options into categories, and the About
page, which shows the version and credits, lives at the bottom of the Settings
category list rather than as a separate rail button.

## The status bar

The status bar runs along the bottom of the window with a thin rule above it. It
reads as two zones. The left zone shows the EVE clock and the sync chip. The right
zone shows the ESI connection indicator, and, when relevant, a pending-mutations
indicator. Thin vertical dividers separate the groups.

### EVE clock

The left end shows the label `EVE` followed by a clock in `HH:MM:SS`. EVE Online
runs on UTC, so this is the in-game server time, not your local wall clock. It
ticks once per second.

### Sync chip

To the right of the clock is the sync chip: a colored dot followed by a short line
of text. The chip is Pod's at-a-glance summary of background syncing. Its dot color
and wording change with the engine state. Click anywhere on the chip to open the
sync popover described below; the chip tints faintly while the popover is open.

The chip can show these states.

- Idle. A green dot with `Synced` and a relative time such as `Synced · 2m ago`,
  meaning everything is up to date and the last full pass finished a while ago. If
  Pod has never completed a sync this session, the chip reads `Idle` instead.
- Running. While a sync pass is in flight, the dot pulses in plasma and the text
  reads `Syncing` with a thin progress bar and a `done/total` count of endpoints,
  such as `Syncing 5/10`.
- Attention. An amber dot with a count such as `2 pending` when some jobs are
  blocked or waiting on a dependency. These are not failures; they usually mean a
  missing scope or a job that has not had its prerequisites met yet.
- Error. A red dot with a count such as `2 sync errors` (or `1 sync error`) when
  one or more jobs failed or are backing off before a retry. Open the popover to
  see which endpoint failed and why.
- Read-only. An amber dot with `Read-only`. Another copy of Pod on your network
  holds the sync lease, so this instance reads the shared data but does not sync.
  When Pod knows the holder's hostname, the chip names the host that has it open.
  A `Take over` button appears next to the chip so you can claim the lease.
- Stopped. A red dot with `Sync stopped` when the sync engine is not running. A
  `Restart sync` button appears next to the chip to start it again.

Syncing is automatic. Pod polls your characters on a short cycle and you do not
trigger it by hand. The `Take over` and `Restart sync` buttons are the only sync
controls in the status bar, and they appear only in the read-only and stopped
states.

### Pending mutations

When you make a change that Pod sends back to EVE, such as sending mail or
responding to a calendar event, it queues as a mutation in an outbox. While any
mutation is in flight or retrying, a `MUTATIONS` indicator appears in the right
zone with a count. A plasma dot and a `↻` count show work still in progress; a red
dot and a `✕` count show mutations that failed. The indicator disappears once the
outbox is empty.

### ESI status

The right end shows a dot and the label `ESI`. ESI is EVE's data service, the
source for everything Pod syncs. A green dot means Pod is reaching ESI. A red dot
means it cannot. A red ESI dot explains why the sync chip is stalled or showing
errors: nothing can sync while ESI is unreachable.

## The sync popover

Click the sync chip to open the sync popover. It is a card that lists every sync
job, grouped by character, with live status for each one.

![Sync popover](/docs/img/interface/sync-popover.png)

The header reads `Sync queue` with a dot and a one-line summary. When idle, the
summary shows the last sync time, such as `· last sync 2m ago`, or `· idle` if
nothing has run yet. While a pass is active, the summary shows counts such as
`· 3 active · 5 queued · 60%`, and the dot pulses in plasma. A close glyph on the
right dismisses the card.

Below the header is one row per job. Each character contributes a fixed set of
jobs: Assets, Clones, Contacts, Profile, Skills, Telemetry, and Wallet. A job only
appears if its feature is enabled; Profile always runs and always shows. A short
colored bar on the left of each row matches the character's color, so you can scan
a character's jobs together. The list scrolls when there are more jobs than fit.

Each row carries a state, shown by its marker dot, its progress fill, and a glyph
on the right.

- Syncing. A plasma marker and the word `Syncing`; this job is fetching now.
- Queued. A dim marker and the word `Queued`; this job is waiting its turn.
- Done. A green marker, a check glyph, and a `Next in` countdown such as
  `Next in 42m` showing when it will run again.
- Empty. A green marker with `No data`; the job ran and there was nothing to fetch.
- Attention. An amber marker. The sub-line explains why, for example a missing
  scope or `Waiting on dependencies`.
- Error. A red marker with an exclamation glyph. The sub-line carries the failure
  reason, or `Backing off 30s` when the job is waiting before a retry.

The footer summarizes the whole queue: a `done / total endpoints` count, a
`· N retry pending` note in red when any job is retrying, and a reminder that Pod
syncs automatically on its short cycle.

## Updates

Pod checks for new releases and, when one is found, surfaces an update banner with
the `UPDATE` eyebrow. The banner walks through the update as it progresses.

- When a new version is found, the banner reads `Version X is available.` with an
  `Update now` button. Press it to start the download.
- While the download runs, the banner reads `Downloading version X…` with no
  button.
- When the download is ready, the banner reads `Version X is ready. Restart to
  finish.` with a `Restart` button that relaunches Pod into the new version.
- If the update fails, the banner switches to a red `UPDATE FAILED` eyebrow and
  shows the error message.

The same update notice can also appear as a small dismissible toast in the
bottom-right corner. Dismissing the toast hides that reminder; it does not cancel
the update.
