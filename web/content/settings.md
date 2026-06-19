---
title: Settings
section: Reference
order: 1
---

# Settings

Open Settings from the navigation rail to configure Pod. The window has a fixed
header reading "Pod · Preferences" and "Settings", with a "Reset to defaults"
button in the top right. That button only resets the category you are currently
viewing, so resetting Features leaves Storage untouched, and resetting
Accessibility leaves your tags alone.

A left pane lists the categories: Accessibility, Features, Storage, Tags, and
User Interface. Industry appears in that list only when the Industry feature is
turned on. About sits by itself at the bottom of the pane, fenced off from the
working categories. Each category shows a small badge that summarizes its
current state, so you can read the gist without opening the tab. The active
category is marked with a plasma indicator bar.

## About

The About tab states Pod's identity and carries the EVE Online Developer
License notice. It shows the name "Pod" next to the current version (for
example v0.6.0), a monospace build line in the form "Build" followed by the
git short SHA and the build date, and the license, which is the MIT License.

A "Support Pod" section explains that Pod is free and open source, built in
spare time, and asks you to consider supporting its development if it is useful
to you. A heart link opens the support page at pod.aaronmallen.dev/#support. A
second link shows pod.aaronmallen.dev and opens the project website. Both links
open in your external browser.

Below the support section is the full EVE Online Developer License trademark
notice. It credits EVE Online and the EVE logo as registered trademarks of
Fenris Creations (formerly CCP hf.), states that Fenris Creations has granted
permission to Pod to use those marks for promotional and information purposes
but does not endorse and is not affiliated with Pod, and that Fenris Creations
is not responsible for the content or functioning of this program. A copyright
line reading "© Fenris Creations. All rights reserved." closes the tab. None of
this is configurable; the tab exists to display the notice.

## Features

The Features tab lists every Pod capability you can turn on or off. The panel
blurb states that changes apply live across every window and your linked
characters, with no restart needed. The category badge reads enabled over
total, for example "12/12" when all twelve features are enabled.

A "Filter features…" field at the top narrows the list. It matches
case-insensitively against both the feature title and its description, so
typing "skill" or "training" both surface the skill feature. With the field
empty, every feature shows. If nothing matches, the panel shows "No features
match this search." with your query underneath.

Features are grouped into two sections. The Character section covers Clone
Monitoring (jump clones and the implants in each), Contacts (your personal
contact list and their standings), Combat Log (combat activity for
after-action review), EVE Notifications (in-game notifications), and Standings
(your standings toward characters, corporations, and alliances). The World
section covers Location Tracking (each character's current solar system,
station, and ship), Skill Monitoring (trained skills and the active training
queue), Industry (running manufacturing, research, and reaction jobs), Mail
(EVE mail headers and message bodies), Calendar (calendar events and invitation
responses), Wallet (wallet balances and the transaction journal), and Asset
Tracking (assets across stations, structures, and hangars).

Each feature has its own toggle and the short description quoted above. Turning
a feature off hides its rail icon and stops syncing that data. Turning Industry
off also removes the Industry category from the Settings pane. Toggles take
effect immediately across every open window.

## Accessibility

The Accessibility tab controls interface scale, high contrast, and how ISK
figures are drawn.

![The Accessibility tab with interface scale presets, the high-contrast toggle, and the contrast preview table](/docs/img/settings/accessibility.png)

Interface scale runs from 85% to 150%, with a default of 100%. Five preset
buttons set common values: XS is 85%, S is 92%, M is 100% (marked "Default"),
L is 125%, and XL is 150%. A "Fine scale" slider lands you between presets in
1% steps. A readout shows the current value, for example "Now: 112% · custom
(between steps)" for an off-preset value or "Now: 100% · M" when you are on a
preset. Scale applies live to every open window, so you do not restart to see
the new size.

The "High contrast" toggle firms up text and surface edges. It swaps the
secondary, tertiary, and dim text tiers from reduced-opacity overlays to solid
tuned values and strengthens surface borders, while leaving primary text and
the dark theme as they are. A preview table on the tab shows each tier (its
usage and target contrast) with the current and high-contrast colors side by
side, plus the surface-edge alpha values before and after. High contrast also
applies live.

The ISK monospace setting draws ISK figures in a monospace font so digits line
up in columns and balances are easier to compare at a glance.

The category badge summarizes the current state. It shows the scale percentage,
appends " · custom" when the scale sits off a preset, and appends " · HC" when
high contrast is on. So "100%", "112% · custom", "125% · HC", and "112% ·
custom · HC" are all valid readings.

## Industry

The Industry tab appears only when the Industry feature is enabled. It sets the
default build facilities the planner pre-selects when you install a job.

![The Industry tab with default Manufacturing and Reactions facility pickers](/docs/img/settings/industry.png)

There are two separate defaults: one for Manufacturing and one for Reactions.
When a default is set, the planner pre-selects that facility for jobs of that
activity. When no default is set, the picker reads "Ask each install", and the
planner prompts you each time. The category badge counts how many of the two
activities have a default chosen, so "0/2", "1/2", and "2/2"
are the possible values.

Each facility picker searches by name. Typing into the field runs a live
character search across stations and structures your characters can reach, and
results show the facility name along with its solar system, region, and
security status. The cost-index values for each facility appear next to the
results as read-only information drawn from the facility itself; the tab does
not let you edit cost indexes, it surfaces them so you can choose a cheaper
build site.

## Storage

The Storage tab controls where Pod keeps its files, how it syncs across
machines, and how you export logs for diagnostics. The category badge shows a
green "All defaults" reading when no paths are customized, or a count of custom
directories when you have overridden one or more of the three.

![The Storage tab with the three file locations, networked sync, and log export controls](/docs/img/settings/storage.png)

The tab lists three locations. "Shared data location" is the SQLite database
that holds the character cache, mail bodies, market snapshots, and skill plans;
by default it lives under your platform data directory. "Pod Cache" holds
portraits, item icons, and other ESI image cache, is safe to clear, and is
rebuilt on demand. "Pod Logs" holds rolling structured logs from the daemon and
the UI, rotated daily, keeping the five most recent daily files. Each location
shows its current path. You can Browse to a new folder, reset a location back to
its Default, and Reveal the logs folder in your file manager.

When you point a location at a new folder that you have used before, Pod asks
what to do with the existing files. A "Relocate store" prompt asks whether to
move that location to the new folder. It offers three choices: "Cancel" to back
out, a "Skip" choice that repoints only, which changes the path but leaves the
old files where they are, and "Move files" to relocate everything to the new
folder. The change applies on the next launch.

### Networked database and take-over

Point the shared data location at a shared volume to use the same data on more
than one machine. The toggle "Sync this location across machines" turns this on.
With it enabled, Pod keeps a fast local working copy of the database and syncs
it to the shared location, so the same data follows you between machines.

Only one Pod instance writes to the shared database at a time, and it holds a
lock while it does. The Storage tab shows the sync status: a green dot with
"Last synced" and a relative time when sync is healthy, a grey "Not synced yet"
before the first sync, and a red dot with "Currently open on" and the machine
name when another
instance holds the lock. A "Sync now" button triggers a manual sync. When
another machine holds the lock and you need to work here, "Release lock" takes
over the lease. Because taking over while another instance is mid-write can lose
unsaved changes, Pod gates the take-over behind a confirmation before it breaks
the lock.

### Exporting logs

Use the log export when you need to send diagnostics. The "Verbosity" control
sets how much the logs record, with three levels: Quiet (the default), Normal,
and Verbose. The export packs the log files for a chosen window into a single
ZIP, named with a pod-logs prefix and the start and end timestamps, together
with a diagnostics manifest. Four range buttons pick the window: "Last hour",
"Last 24h", "Last 7
days", and "Today". The button shows "Exporting…" while the ZIP is being
written.

## Tags

The Tags tab manages the custom labels you attach to characters, including each
tag's color and order. The category badge counts how many tags have a color
assigned.

![The Tags tab with the tag list, color swatches, and sort controls](/docs/img/settings/tags.png)

Create a tag by typing its name into the "Create a tag…" field and clicking
"Add"; the Add button is active only when the field has text, and the field
clears after you add the tag. Rename a tag by clicking its name in the list,
which opens an inline editor; type the new name and press Enter or click away to
save. A name that duplicates an existing tag (ignoring case) is rejected and the
rename is cancelled. Delete a tag with the × button at the end of its row.

To set a color, click the swatch next to a tag's name. A small color picker
opens at the cursor with a hex input, so you enter a value such as #FF8040. The
field validates the hex and flags an invalid value rather than applying it. A
"Clear" button in the picker removes the color. Any valid hex is accepted, so
the colors are yours to choose rather than a fixed palette.

Sort the list with the header buttons: "Manual" keeps your own drag order and is
the default, "A–Z" sorts alphabetically, and "Color" groups by color with
uncolored tags last. A filter field narrows the list by name. You can drag a tag
row to reorder it, but only in Manual sort with no filter active; otherwise the
tab shows "Reorder disabled in sorted view" or "Reorder disabled while
filtering".

## User Interface

The User Interface tab controls where the navigation rail sits and the order of
its icons. Both settings apply live across every Pod view.

![The User Interface tab with the rail-side control and the icon-order list](/docs/img/settings/ui.png)

The "Rail side" control docks the rail to the "Left" or the "Right" edge of the
workspace. The default is Left. The current side is marked on the chosen card,
and the category badge reads "Left" or "Right" to match.

The "Icon order" control reorders the rail. Each row shows a two-digit position
and a drag handle. Drag a row by its handle to a new spot, or use the up and
down arrows on the row to move it one place at a time; the top row cannot move
up and the bottom row cannot move down. While you drag, the row you are moving
is tinted and a plasma line marks where it will drop. Settings is always pinned
to the end of the rail and cannot be moved. A "Reset order" button restores the
default order and is disabled when the order is already the default.
