# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semver versioning](https://semver.org/).

## [Unreleased]

## [0.6.2]

### Added

- Received mail now displays with full formatting — bold, italic, underline, line breaks, and per-word font colors
  and sizes — so messages look the way they do in-game instead of as flattened plain text.
- Compose now has a formatting toolbar to make selected text bold or italic, plus a "Generate Link" button that
  inserts links to a web address, character, corporation, solar system, or station.
- Sent mail appears in your Sent folder the moment you hit send, instead of only showing up after the next sync.
- The Drafts folder works — closing the compose box, switching folders or characters, or quitting the app now saves
  your unfinished message, and reopening it picks up right where you left off.
- You can drag a message between the Inbox, Archive, and Trash boxes in the folder pane, not just onto a custom label.
- Trash now has a Delete action that permanently removes a mail from both Pod and your in-game mailbox.
- Mail left in Trash is now automatically deleted after 30 days, so it no longer piles up forever.
- Snoozing a mail now also updates its labels in EVE — it leaves your Inbox and gets a "Snoozed" label while asleep,
  then returns to the Inbox when it wakes.

### Changed

- Recipients in the compose To/Cc and link pickers now show as pills with a portrait and a remove button, and the
  field carries a search glyph and an "Add another…" prompt to match the rest of the app.

### Removed

- The Pin feature has been removed in favor of Star — mail no longer has a separate pinned section at the top of each
  folder; your existing stars are kept.

### Fixed

- The All Inboxes view no longer lists your own sent mail, so the message list now agrees with its unread count.

## [0.6.1]

### Added

- New **User Interface** settings tab - choose whether the navigation rail sits on the left or right edge of the
  window, and drag or reorder the rail's items to your liking. Changes apply instantly to every open window.
- Stockpiles can now be scoped to a set of characters with a simple query (for example `tag:pvp` or `corp:cobalt`)
  instead of always covering everyone - the scope tracks roster changes automatically without re-saving.
- Stockpile editor - search for an item and pick it to add a fully-filled row instantly; the old blank "+ Add item"
  row is gone, and items you've already added are hidden from the results.
- The stockpile location picker can now target any EVE tier - region, constellation, system, station, or structure -
  with a colored tier tag, security status, and region/system context shown for each result.
- A red **Fix Permissions** button now appears on the corner of a character portrait or corporation logo when
  re-authentication is needed, launching the fix in one click instead of hunting through the right-click menu.

### Changed

- All **About** information now lives on a Settings → About tab, which links to the project website
  (pod.aaronmallen.dev) and adds a "Support Pod" section.

### Fixed

- Calendar local-time labels now convert to your actual time zone - non-UTC users previously saw the local-time line
  showing the exact same time as the EVE (UTC) line.
- Skill plan editor - the search box now stays visible when a picker tab (Skills, Ships, Modules, Certs) has no
  results, so you can always clear or change a search that matched nothing.
- Long facility names in the Industry planner and Industry settings now wrap instead of being cut off, and NPC
  stations no longer show their system name twice.

### Performance

- The Industry Planner now opens almost instantly after the first visit - it previously rebuilt its entire build
  catalog (a 40-plus-second wait) every single time you opened it.

## [0.6.0]

### Added

- New **Industry** window - a dedicated rail feature for tracking and planning manufacturing. It opens with a Jobs tab
  showing your characters' and corporation's manufacturing, research, invention, reaction, and copy jobs with live
  countdowns and job-slot meters.
- Industry **Blueprints** tab - browse every owned blueprint original and copy with its material and time efficiency,
  runs remaining, location, and what it builds.
- Industry **Planner** tab - pick anything you can build and get a recursive plan: a rolled-up shopping list, a
  dependency-ordered build order, and live profit, margin, and ISK-per-hour. Break any component down to build it
  in-house, choose where each job runs (including a live search for any station or structure you can dock at), and save
  plans to reopen later.
- Industry Planner **Use Stock** - draw materials you already have on hand at the build site out of the shopping list,
  with a clear from-stock vs. to-buy split so you only buy what you're missing.
- Industry **Extractions** tab - live moon-mining extraction timers for your corporation, showing chunk arrival and
  natural-fracture countdowns.
- Industry settings - set a default install structure per activity (Manufacturing, Reactions) so new plans start at the
  right facility.
- New **Calendar** window - a dedicated rail feature with agenda, day, week, month, and year views of your in-game EVE
  calendar. Accept, decline, or mark events tentative, and see a badge for upcoming invites you haven't answered yet.
- The Calendar also overlays your own activity: skill-training completions, market-order and contract expirations,
  industry-job completions, and corporation moon-extraction timers, so you can plan around them in one place.
- **Corporation detail view** - click a corporation card to drill into it, with Standings, Contacts, and Kill Log tabs
  alongside its header (logo, ticker, members, tax rate, alliance, CEO, and headquarters).
- **Standings finder** - the Standings tab is now a searchable catalog of every faction, NPC corporation, and agent,
  with filters by faction, corporation, agent, and more, plus your effective standing after social skills.
- **Killmail detail** - click any kill or loss to open a detailed view with the victim, the destroyed-vs-dropped value
  split, fittings and cargo grouped by slot, and every attacker with their damage share. Older kills are filled in
  automatically so they open with full detail too.
- **Contract detail** - click a contract to see its items, bids, parties, and locations, and your corporation's
  contracts now sync and appear too.
- **Contacts editing** - add, edit, and remove contacts directly from the Contacts tab (it was previously view-only),
  with a standing slider, label assignment, and a watchlist toggle. Each contact now shows its portrait or logo.
- **Mail labels** - create, color, assign, and delete mail labels from the Mail window. Assign by dragging a message
  onto a label, with the reading-pane Label button, or via a right-click menu, and labels show as colored chips.
- **Skill plans from your queue** - select multiple rows in the skill queue (click, Shift-click, and Ctrl/Cmd-click)
  and create a skill plan from exactly that selection.
- **Accessibility settings** - an interface-scale slider and a high-contrast mode in Settings, both applied instantly
  across every window without a restart.
- **Feature toggles** - turn individual windows and sub-features (Calendar, Industry, Standings, Contacts, and more) on
  or off from Settings; the nav rail and background syncing follow your choices.
- A **log verbosity** picker (Quiet, Normal, Verbose) in Settings so you can control how much detail Pod writes to its
  log file.
- Icons on the Wallet and Assets content tab bars, matching the rest of the app.

### Changed

- The sync status indicator now tells the truth when syncing isn't running: it shows "Sync stopped" with a Restart
  button, or "Read-only - open on another machine" with a Take over button, instead of spinning forever.
- Taking over sync from another machine now warns you about possible data loss and tells you how long ago that machine
  was last active, and a read-only instance now reclaims sync automatically once the other machine releases it.
- A successful sync that simply had nothing new to fetch is no longer flagged as a warning.
- The Assets inventory now uses proportional column widths with a wider item-name column.
- Character cards show their headline ISK value in a bolder monospace so it stands out as the card's focal number.

### Fixed

- Negative ISK amounts in the Industry view now display scaled and signed (for example -5.00B) instead of a raw number.
- Recently-read mail, just-added or removed contacts, and fresh calendar RSVPs no longer briefly revert while a sync is
  running, and the mail reading pane no longer shows a stale message after you archive or quickly switch selection.
- The Inventory and Abyssals tab counts now reflect your full totals immediately instead of under-counting until you
  scroll.
- Removing a single skill from a mastery-based skill plan no longer collapses the whole plan.
- Removing and re-adding (or re-authorizing) a character no longer fails or stalls, and asset sync no longer gets stuck
  forever at certain NPC stations.
- Sync no longer risks corrupting the database when taking over on a shared network drive, and the build planner's cost
  figures can no longer be silently wiped by a momentary server hiccup.
- Character and corporation portraits no longer overflow their frame on Linux, the Windows taskbar and title bars now
  show the Pod icon, and the Linux Flatpak build no longer fails to launch.
- On Linux, the app no longer silently falls back to slow software rendering when a graphics backend is available, and a
  fade-to-black ghost behind the sync popup on Wayland is gone.
- Numerous Industry planner polish fixes: a searchable facility picker that shows real names, a cohesive runs stepper
  and editable runs field, item icons, and scroll position that's kept when you break a component down.
- Standings agents now appear under the All and Agents filters, EVE's built-in system mail labels no longer clutter the
  label list, and jobs for disabled features no longer linger in the sync popup.
- Corporation parties on a contract now show the corporation logo instead of a broken portrait.

### Performance

- Long, infinite-scrolling lists - the Assets inventory, the Abyssals grid, Mail, the Wallet ledgers, and the
  character-detail tabs - now render only what's on screen and load more from the database as you scroll, staying smooth
  no matter how much data you have.
- Item and party icons are resolved once when data loads instead of on every frame, so scrolling and hovering no longer
  stutter while looking them up.
- The Wallet, the skill-plan editor, the Abyssals grid, and the Industry Jobs and Planner tabs all recompute their
  derived views far less often, making them noticeably more responsive.
- Industry data loads faster by fetching related records together instead of one at a time, and new database indexes
  speed up on-hand stock and wallet-transaction lookups.
- Background syncing contends less with the app you're using: its data cache no longer competes with the screen you're
  looking at, and bursts of sync completions now trigger a single refresh that keeps your scroll position, expanded
  containers, and in-progress filters.
- Pod writes far less to its log file by default, and networked backups no longer pile up - they're written only on a
  genuine divergence and capped to the newest three.

## [0.5.6]

### Changed

- The column headers in the Inventory and in the Wallet's Market and Contracts tabs now stay pinned at the top while
  you scroll through the rows beneath them.
- Long item, character, and party names now wrap onto multiple lines across tables instead of being cut off - rows
  grow taller to fit so nothing is hidden.

### Fixed

- Corporation-owned assets on the Assets "Values" tab now show the owning corporation's name instead of a raw
  "Owner ID" placeholder.
- Unnamed items no longer show "None" as their label - they fall back to the item's type name.
- Corporation assets no longer vanish from the Assets view when some items can't be looked up by a custom name.

### Performance

- Character info, skills, implants, and financial data save noticeably faster during sync - many small writes are now
  batched into single operations.
- The wallet composition view renders faster, especially with many characters.

## [0.5.5]

### Added

- Your custom container and ship names now appear in the Inventory - the nickname you gave an item shows as the main
  label with its type name beneath it, for both your characters and your corporation.

### Changed

- Sync now shows your computer's real name instead of "unknown-host", and trying to take over while another machine is
  still active updates the banner to name that machine and when it was last seen instead of silently doing nothing.

### Fixed

- Adding a character no longer crashes Pod - signing in a new character could throw the roster into an impossible
  layout and abruptly close the app.
- Adding several characters at once no longer freezes Pod for tens of seconds, and newly added characters now sync
  reliably instead of failing quietly in the background.
- Fitted-ship killmails now show up in your killlog - losses that carried fitted modules had been silently dropped
  since 0.5.4, so only kills with empty ships appeared.
- Asset sync no longer fails forever at NPC stations whose owning corporation is run by an agent from a different
  corporation - your assets and net worth keep updating.

## [0.5.4]

### Added

- You can now save, name, and reuse asset filters - set up a search and category in the Inventory screen, save it with
  a star, and it appears in a "Saved filters" list you can apply or delete anytime.
- The Inventory search now filters live as you type instead of only when you press Enter, and its location filters
  (loc:, region:, system:, constellation:) finally return results - they previously matched nothing.
- New "Export logs…" button in Settings › Storage bundles recent logs into a zip - pick Last hour, Last 24h, Today, or
  Last 7 days - making it easy to attach diagnostics to a bug report.
- A character you've just signed in now appears in your roster right away, with its real name, instead of taking
  seconds (or a restart) to show up.
- The Assets Tracker graph now matches the wallet chart - hover to see a crosshair and a tooltip with the snapshot
  date, net asset value, and change.
- Killmails now load directly from EVE first and fill in their value from zKillboard when the kill is public; values
  keep improving in the background as kills become available there.

### Changed

- Resizable panes now scale to your window size - a pane you sized on a large window no longer overflows a smaller one
  on restore, and panes track the window as you resize it live.
- "Sync now" now does a real two-way sync and always tells you what happened - it pushes your local changes, pulls
  newer changes from the shared copy, and reports the result even when there was nothing to transfer or another machine
  holds the lock.
- A second machine's changes now arrive on their own while Pod is open, instead of only at startup or when you take
  over the lock.
- The "Sync this location across machines" toggle in Settings is now always usable; when Pod detects a network drive it
  shows a dismissible suggestion to turn sync on rather than forcing it.
- Killlog efficiency and ISK-lost stats are now based on the value actually destroyed rather than destroyed-plus-
  dropped, so they reflect your real losses.

### Fixed

- Pod no longer reopens as a broken, tiny, or off-screen window after a corrupted or stale saved size - the window size
  is validated and held to a sensible minimum on startup.
- Pod no longer hangs on startup when your data is on a slow or unreachable network drive - the slow disk work now
  happens off the main path so the window still opens.
- The wallet Market, Contracts, and Journal tab badges now show the true total for the current scope instead of
  starting at 50 and climbing as you scroll.
- The Skills screen no longer shows an already-finished skill stuck at 100% "Currently training" - it now shows the
  skill that's genuinely in progress, agreeing with the Character Manager.
- The Values tab now labels locations with their real station or structure names and uses distinct colors per category
  in the by-category chart, instead of "Loc 12345" headers and near-identical blue shades.
- Asset sync no longer aborts at NPC stations - it survives a station-owner corporation whose CEO can't be looked up,
  so your assets and net worth still load.
- Switching your data location or toggling sync no longer risks landing on an empty, duplicated, or overwritten
  database - Pod safely seeds, reconciles, and backs up before replacing any data, and never opens an empty database
  over real data.
- Closing Pod's last window now fully quits the app on every platform, with no hidden background process left running.
- On Linux, second-launch handoff and browser sign-in now work under Flatpak and other sandboxes, and a leftover lock
  file from a crash no longer blocks Pod from starting.
- The mail header picker is now a character selector you can always change - previously, once you picked a character you
  could never switch back to another from the dropdown.

### Removed

- The "Clear Cache" menu item is gone - Pod no longer deletes your images, database, or settings from disk; it only
  replaces them in place. This fixes item icons disappearing for good after an update.

### Performance

- Adding a character and other interactive actions stay responsive during background sync - roster refreshes are
  coalesced, the sync engine has its own database connection, asset updates are written in smaller batches that release
  the database lock, and freshly added characters sync promptly instead of waiting for the next cycle.
- Pod writes far less to its log files (it no longer records every database query), so logs take up dramatically less
  disk space.

## [0.5.3]

### Added

- You can now store your Pod database on a shared or network drive and use it from more than one machine - Pod keeps a
  fast local working copy, syncs it to the shared copy in the background, and coordinates so only one machine writes at
  a time. If another machine holds the lock you'll see a read-only banner with a one-click "Take over".
- The Settings screen now has Sync controls for the data location - a toggle to sync that location across machines, a
  live sync and lock-status line, and "Sync now" and "Release lock" actions.
- New Skills Compare window - open it from the Compare button in the Skills header to line up to three characters
  side by side, with a skill-level matrix, mastery averages, and summary stats (total SP, skills at V and IV+).
- The wallet net-worth chart now plots a separate liquid-ISK line alongside net worth, and the hover tooltip gains a
  "Liquid" row so you can see how much of your worth is cash on hand.
- A new About tab in Settings shows Pod's version, build, license, and GitHub link, plus the EVE Online trademark
  notice - giving Windows and Linux users the same information the macOS menu already provided.

### Changed

- Portraits and logos now reappear on their own if the system clears them from the image cache while Pod is open -
  including characters and corporations you don't own - instead of showing initials until you take some action.
- Changing your data location or toggling sync now safely migrates the database between local and shared layouts, so
  you never land on a locked, duplicated, or broken database after the switch.
- The wallet net-worth timeframe selector (3M / 6M / 1Y) now actually zooms the chart by calendar date - a short
  history crowds into the recent edge of a longer window instead of every timeframe looking identical.

### Fixed

- Item type icons no longer render blank on Linux (AppImage, .deb, and pacman packages) - Pod now finds its bundled
  icons in the packaged layout.
- Asset sync no longer gets permanently stuck when an item moves between two of your characters or corporations - it
  previously failed every retry with a database conflict until the item moved back.
- Removing a single character or corporation no longer wipes every other portrait or logo - only that subject's
  cached image is removed.

## [0.5.2]

> [!IMPORTANT]
> **This update must be installed manually.** A bug in 0.5.0 and 0.5.1 left the in-app updater turned off, so those
> versions can't detect or offer this release on their own. Download and install 0.5.2 by hand - automatic updates
> work again from this version onward.

### Fixed

- The in-app update notification works again - it was unintentionally turned off in 0.5.0, so 0.5.0 and 0.5.1 never
  checked for or offered new versions. Once you're on 0.5.2 you'll be notified about future updates as usual.
- Closing the startup splash screen now fully quits Pod. Previously it could leave a hidden background process running,
  which stopped the app from reopening until that process was killed manually.
- After signing in through your browser, the tab no longer sits spinning on EVE's login page - it now lands on a short
  Pod page that hands you back to the app, with an "Open Pod" button if it doesn't switch over automatically.

## [0.5.1]

> [!WARNING]
> **Pod 0.5.0 is a complete rewrite of the app and is not backwards compatible with earlier versions.**
> Updating clears your existing local Pod data and starts fresh, so the first time you open it you'll need to sign in
> and re-authorize all of your characters again.

### Added

- Stockpile cards now show ISK figures - an estimated total value when fully stocked and the ISK still needed to fill
  any shortfall, each labeled as an EVE-average estimate.
- You can now export a stockpile's shopping list to EVE's in-game Multibuy - right-click a stockpile, choose between the
  full target or just the remaining shortfall, and copy a paste-ready list to the clipboard.
- Stockpile cards gained a status strip showing "Ready to ship" or how many items are short, plus an expand/collapse
  control so long item lists no longer overflow the card.

### Changed

- The stockpile create, edit, import, and export screens have been redesigned as centered modals with clearer item
  previews and actions.
- The mail compose body and the stockpile multibuy import box are now multi-line editors, so you can write, paste, and
  review multi-line content in a scrollable box instead of one cramped line.
- Character portraits and corporation and alliance logos now refresh on their own - images older than about a week are
  re-fetched during sync, so they stay current when someone changes a portrait or a corp updates its logo.

### Fixed

- Stockpile cards now count items you already own even when they sit in a station, structure, or container inside the
  stockpile's location, and across your corporation - they previously showed 0 of N for stock you actually held.
- Stockpile progress bars no longer render full at 0% (or empty at 100%) - a display bug could make an empty stockpile
  look fully stocked.
- Importing a multibuy list now reads quantities written before the item name (like "25 Mobile Tractor Unit" or
  "x25 Mobile Tractor Unit"), not just quantities at the end of the line.
- Pod no longer gets stuck on the loading screen when EVE's static data fails to refresh - if your data is already
  present it continues with a small "refresh failed" notice, and a fresh install now offers a Retry button.
- Downloading EVE's static data no longer fails on slow or large connections - the download now has a much longer
  timeout and retries transient network errors instead of giving up on the first hiccup.
- Browser sign-in now reliably hands you back to Pod on Windows even after you move the app to a new folder, and on
  Linux AppImage builds where the link previously failed silently.
- Launching Pod a second time now brings the existing window to the front instead of opening a duplicate.
- Asset sync no longer fails with a database error in an edge case where a structure owner's CEO is personally enlisted
  in factional warfare.

### Performance

- Pod starts up faster when EVE's static data hasn't changed - it checks a small version marker and skips the large
  static-data download entirely when your local copy is already current.
- Initial setup is much faster when Pod's database lives on a network drive - seeding EVE's static data, which could
  take several minutes, now completes far more quickly.

## [0.5.0]

> [!WARNING]
> **Pod 0.5.0 is a complete rewrite of the app and is not backwards compatible with earlier versions.**
> Updating clears your existing local Pod data and starts fresh, so the first time you open it you'll need to sign in
> and re-authorize all of your characters again.

### Added

- A new Abyssals view in the assets tab lists your abyssal-rolled modules with their individual mutated stats, mutation
  tier, estimated value, and location - filter the grid by module type or by a range on any stat to find an exact roll.
- Squads let you group your characters into named, color-coded sets in the character manager, so a large roster stays
  organized the way you actually fly.
- Tags are now managed from a dedicated section in Settings, where you can create, rename, recolor, reorder, and delete
  the labels you apply to your characters.
- A new Storage section in Settings shows exactly where Pod keeps its database, logs, and image cache, and lets you
  move any of them to another folder or drive - handy for keeping the database off a small system disk.
- You can now build a stockpile by pasting an in-game multibuy list - Pod reads the item names and quantities, matches
  them against EVE's item catalog, and shows a preview to reconcile against the stockpile before anything is saved.

### Changed

- Character sign-in now completes in your browser and hands you straight back to Pod through a system link, instead of
  routing the EVE login through a local web server on a fixed port - no more firewall prompts or "port already in use"
  failures, and it behaves consistently across macOS, Windows, and Linux.
- Pod's background sync with EVE has been rebuilt so your character, wallet, asset, skill, and mail data stays current
  more reliably, with less redundant traffic to EVE's servers.
- The wallet's net-worth chart now breaks your total down into liquid ISK, asset value, and escrow, and lets you hover
  any day on the timeline to read the exact value and composition at that point.

## [0.4.9]

### Fixed

- Wallet and Asset performance improvements

## [0.4.8]

### Fixed

- Your character, wallet, skill, and asset data now keeps updating reliably the whole time the app is open - the app
  refreshes your EVE sign-in just before it expires instead of occasionally using it after it has already lapsed.
- Your assets no longer appear completely empty when you own items in a structure your character can't access (such
  as a citadel you've lost docking rights to) - that location now shows as "Unknown Structure" and the rest of your
  assets load normally.

## [0.4.7]

### Fixed

- Opening the wallet no longer hangs indefinitely when EVE's servers are slow or unresponsive - connections
  now time out after 30 seconds instead of blocking forever.
- The wallet now shows your cached journal, transactions, and contracts even when EVE's login service is
  unavailable or your session needs re-authentication.
- The wallet and contracts view no longer crash with a fatal error when certain data fields are missing or
  malformed in a server response.

## [0.4.6]

### Changed

- The inventory search box now spans the full width of the panel - category pills and item count
  move to a row below, giving you more space to type filter queries.
- Mail folder and inbox icons, and the pin icon on messages, are now crisp vector graphics instead
  of Unicode characters that could look different depending on your system font.

### Fixed

- Contracts received from another player now show that player's name as the counterparty - they
  were previously showing your own character's name.
- The Contracts, Journal, and Market tab counts now update when you switch between characters -
  previously the counts showed the unfiltered total instead of the rows visible for the selected
  character.
- The app update notification banner now has consistent height and visual balance across all of
  its states (checking, available, downloading, and ready to restart).

## [0.4.5]

### Added

- The inventory filter now supports structured queries - filter by name, group, category, region, constellation,
  system, location, and item type, combine multiple terms with spaces (AND), separate values with commas (OR), and
  prefix a term with `!` to exclude it.
- A help button in the inventory search bar opens a quick-reference panel showing every supported filter keyword with
  clickable example queries.
- The assets sidebar now organizes your items in a full region → constellation → system → location → container tree,
  making it easy to drill down to exactly where something is without leaving the tab.
- The assets sidebar can now be resized by dragging its edge - your preferred width is remembered between sessions.

### Fixed

- Stockpile locations now show the actual station or structure name instead of a raw numeric ID.
- Region names in the asset sidebar now display correctly instead of showing as "Region 12345678".
- Windows users who were unable to complete the add-character or add-corporation flow (authentication hanging or
  showing a "Request Too Long" error) should now authenticate successfully.
- A corrupted or out-of-bounds saved window position no longer prevents the app from opening - it falls back to a
  centered default.
- Closing the main window now exits the app cleanly instead of leaving a background process running.

## [0.4.4]

### Fixed

- Asset value is now consistent between the wallet and inventory tabs - previously the wallet fetched live prices from
  EVE's servers and could show a different total than the assets view.
- Rare and infrequently-traded items (supercarriers, faction modules, and similar) now show a price in your inventory
  and portfolio instead of 0 ISK - the app falls back to CCP's adjusted price when no Jita sell order is available.

### Performance

- Character data, clones, stockpiles, skill plans, and item types all load noticeably faster - related information is
  now fetched together in a single operation instead of separate round trips.

## [0.4.3]

### Added

- Your active ship now appears in the inventory alongside your other assets - modules, rigs, and cargo are grouped
  under the ship even when you are undocked in space.
- Corporation assets are now synced in the background during character refresh, so inventory data is always current
  without manually navigating to the assets tab.
- The inventory view now restores instantly when you navigate away and return - switching tabs no longer triggers a
  fresh reload.
- Re-authorizing a character on Windows is now more reliable; repeated auth attempts no longer fail due to a
  port-conflict error between flows.

### Fixed

- Ships docked inside player-owned structures (citadels, engineering complexes, etc.) now show the correct structure
  name and solar system in the inventory view.
- Character tags now survive background sync reliably - the fix in 0.4.2 was incomplete and only worked when the
  character list was the active tab.

## [0.4.2]

### Fixed

- Assets located in space - undocked ships, jetcans, and floating containers - now appear in the inventory view
  grouped under their solar system name instead of being invisible.
- Character tags now remain visible after a background sync - they no longer vanish until you navigate away and back.

## [0.4.1]

### Added

- The inventory tab now loads items progressively - the first 100 appear immediately and more load automatically as you
  scroll, keeping the app responsive with large inventories.
- ESI character data now syncs in the background after the splash screen, so the app is immediately usable without
  waiting for a network refresh.
- Drag-and-drop character reordering now works with empty grid slots - characters can be dragged to any position,
  including gaps between existing characters and the trailing empty row.
- Newly added characters are placed in their own unique grid slot; any characters that shared the same slot due to a
  legacy issue are automatically reassigned on startup.

### Fixed

- Blueprint copies (BPCs) no longer show a price or inflate portfolio totals in the inventory tab.
- Skill training ETAs now include the full year for long-duration skills (e.g. "22 May 2026 · 14:30" instead of
  "22 May · 14:30").
- The asset screen's character selector now correctly shows "All Characters" instead of "All Wallets".
- Opening the tag modal no longer resets the character grid's scroll position.
- Character portraits now load instantly on startup from a local cache rather than waiting for a network request.
- The portfolio tracker chart now refreshes prices every 30 minutes while the app is open, and shows today's value
  immediately on first launch rather than waiting until the next day.

### Changed

- Performance improvements to asset loading and ESI data fetching.

## [0.4.0]

### Added

- Ten user-facing feature flags replace the previous prototype set: `clone_monitoring`, `contacts`, `combat_log`,
  `eve_notifications`, `standings`, `location_tracking`, `skill_monitoring`, `mail`, `wallet`, and `asset_tracking`.
  All flags default to enabled.
- ESI OAuth scopes are now split between character and corporation flows. Corp wallet (`contracts`, `orders`, `wallet`)
  and corp asset scopes are only requested when their corresponding feature flags are enabled, so characters are never
  prompted for scopes they do not need.
- `granted_scopes` column on the characters table records which OAuth scopes were actually granted during
  authentication.
- On first launch after upgrading, granted scopes are backfilled for existing characters by decoding the current access
  token JWT locally - no network round-trip or re-authentication required.
- Nav items (Assets, Skills, Wallet, Mail) and character detail tabs (Clones, Contacts, Kill Log, Notifications,
  Standings) are hidden when their corresponding feature flag is disabled.
- When a character's granted scopes do not cover what the active feature set requires, each affected tab shows a
  scope-gap indicator with a **Re-authorize** button that restarts the OAuth flow for that character requesting the
  missing scopes.
- Background ESI fetch tasks and the location-refresh task are suppressed for disabled features, reducing unnecessary
  API calls.

### Fixed

- Settings changes now take effect immediately without restarting the app. The `ConfigUpdated` message was not
  propagating to the in-process config, so toggles appeared to save but reverted on next view.
- Re-authenticating a character now updates the existing row in place instead of inserting a duplicate record.

## [0.3.1]

### Fixed

- Native "Pod" menu now appears in the macOS menu bar. In daemon mode, `init_for_nsapp()` was called before NSApp was
  fully initialized; the menu is now re-attached once the first window opens.
- In-app update banner now appears correctly when a newer version is available. The update manifest was using `darwin-*`
  platform keys instead of the `macos-*` keys that `cargo-packager-updater` actually resolves, causing the version
  check to fail silently.

## [0.3.0]

### Added

- Native application menu with **Help → About Pod** item. The About dialog shows the current version number.
- Character grid now scrolls when more characters are added than fit on screen, with drag auto-scroll at the edges.
- Background ESI cache cleaner service purges stale cached entries so the cache does not grow unbounded between
  restarts.
- Website now shows release notes parsed directly from this changelog, with a beta channel badge and corrected
  navigation links.

### Fixed

- Application now exits cleanly when a database migration fails at startup instead of continuing with a partially
  migrated schema.
- `solar_system_id` is now persisted in the structure cache, so location data survives app restarts.
- Corporation net worth calculation no longer includes character personal assets and escrow buy orders, which were
  previously counted twice.
- Duplicate characters can no longer be added to the roster when the same character is authenticated more than once.

## [0.2.0]

### Added

- In-app auto-update: background check runs on startup and every 4 hours via `cargo-packager-updater`. A dismissible
  banner appears when a newer version is available; clicking **Update** downloads and installs the new binary in the
  background, then transitions the banner to **Restart Now**.
- File-based structured tracing with daily log rotation (7-file retention) written to the platform state directory
  under `pod/logs/`.

### Changed

- Switch to Semantic Versioning for releases.

### Fixed

- Space Grotesk now renders correctly on Windows. Bundled static TTF files were HTML documents saved with a `.ttf`
  extension, causing fontdb to fail and Windows to fall back to a symbol font (Wingdings-style rendering).
- Startup no longer opens a visible console/terminal window on Windows. Previously the default CONSOLE subsystem caused
  a PowerShell window to appear alongside the app; closing it sent CTRL\_CLOSE\_EVENT to the process, terminating Pod.
- App no longer closes immediately after the splash animation on Wayland (KDE Plasma 6 / CachyOS). In-place window
  mutation (`toggle_decorations` on a transparent frameless surface) caused the compositor to silently invalidate the
  handle; the transition now closes the splash and opens a fresh main window with correct settings from the start.

## 26.5.20

Initial beta release

[Unreleased]: https://github.com/aaronmallen/pod/compare/0.6.2...HEAD
[0.6.2]: https://github.com/aaronmallen/pod/compare/0.6.1...0.6.2
[0.6.1]: https://github.com/aaronmallen/pod/compare/0.6.0...0.6.1
[0.6.0]: https://github.com/aaronmallen/pod/compare/0.5.6...0.6.0
[0.5.6]: https://github.com/aaronmallen/pod/compare/0.5.5...0.5.6
[0.5.5]: https://github.com/aaronmallen/pod/compare/0.5.4...0.5.5
[0.5.4]: https://github.com/aaronmallen/pod/compare/0.5.3...0.5.4
[0.5.3]: https://github.com/aaronmallen/pod/compare/0.5.2...0.5.3
[0.5.2]: https://github.com/aaronmallen/pod/compare/0.5.1...0.5.2
[0.5.1]: https://github.com/aaronmallen/pod/compare/0.5.0...0.5.1
[0.5.0]: https://github.com/aaronmallen/pod/compare/0.4.9...0.5.0
[0.4.9]: https://github.com/aaronmallen/pod/compare/0.4.8...0.4.9
[0.4.8]: https://github.com/aaronmallen/pod/compare/0.4.7...0.4.8
[0.4.7]: https://github.com/aaronmallen/pod/compare/0.4.6...0.4.7
[0.4.6]: https://github.com/aaronmallen/pod/compare/0.4.5...0.4.6
[0.4.5]: https://github.com/aaronmallen/pod/compare/0.4.4...0.4.5
[0.4.4]: https://github.com/aaronmallen/pod/compare/0.4.3...0.4.4
[0.4.3]: https://github.com/aaronmallen/pod/compare/0.4.2...0.4.3
[0.4.2]: https://github.com/aaronmallen/pod/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/aaronmallen/pod/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/aaronmallen/pod/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/aaronmallen/pod/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/aaronmallen/pod/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/aaronmallen/pod/compare/26.5.20...0.2.0
