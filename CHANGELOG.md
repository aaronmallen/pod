# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semver versioning](https://semver.org/).

## [Unreleased]

## [0.7.1]

### Added

- Planetary colony cards now show at a glance how full each colony's storage is and which character owns it, with
  the owner's portrait and name along the bottom — so you can spot a colony that's backing up and who it belongs to
  without opening it.
- Your calendar now marks when each planetary colony is projected to fill its storage and stall production, accounting
  for extractors slowing down as they run — so you know when a colony needs emptying before it stops producing.

## [0.7.0]

### Added

- **Market** — a full market screen with four tabs. Browse any item's live order book by region or accessible
  structure, using a searchable category tree, and view a price-history chart with median, daily range, and volume
  over a range from one month to a year. My Orders lists your live buy and sell orders across every character (and your
  corporation's), floats outbid orders to the top, and can open an item straight in the EVE client. Watchlist tracks
  buy or sell price targets for individual items and alerts you when one is crossed. Compare puts several markets
  side by side to spot arbitrage between them.
- Market outbid and price-target alerts now appear in your notifications and link straight to the relevant tab.
- **Structure Alerts** — a new Roster screen that gathers every Upwell structure and customs office (POCO) you can
  manage, loudest alert first, warning you about low or expiring fuel, reinforcement and attack, offline services,
  anchoring, and POCO vulnerability windows — drawn from both synced structure state and your in-game notifications.
  Structures and offices now appear for any Director or Station Manager, not only the character that first set up corp
  access, and you can record a structure's rig fitting so the industry planner treats it as a facility.
- **Colonies** — a new tab in Industry for planetary interaction. Each colony shows its planet, command-center pips,
  exported commodity with units and ISK per day, and a live extractor-expiry countdown, alongside a summary band
  (total output, colonies expiring within a day, idle ones) and sorting by expiry, value, or tier. A detail drawer
  breaks down the production chain, extractor heads, factories, and launchpad fill.
- You can now choose a default market location in Settings under Facilities — any region, system, station, or structure
  you can dock at.
- In the Skills queue and plan editor you can now select several rows and copy them with Cmd/Ctrl+C as
  EVE-importable text.
- AI assistants connected to Pod can now read your market watchlist and an item's price history, add, update, or remove
  watchlist entries, and pull a full narrative snapshot of any date range from Captain's Log.

### Changed

- Ready-to-Assign in the wallet budget is now a single figure that stays the same whichever month you're viewing, and
  ISK earmarked for a future month no longer counts as available today.
- The Industry sidebar and tab strip now list Planner ahead of Colonies and Extractions, and agree with each other.

### Fixed

- Inventory search now reveals matching items tucked inside containers and corporation hangars, instead of leaving them
  hidden in a collapsed container while the result count still climbed.
- Captain's Log no longer shows future-dated day cards for fleet ops or other calendar events scheduled ahead of today.
- Sending a mail no longer leaves a leftover copy behind in your drafts.
- A budget category whose money nets to exactly zero no longer shows as overspent or displays "-0" from a fraction of
  an ISK left over.
- Error messages from AI-assistant tools now come through in full instead of a generic "Tool execution failed".

## [0.6.20]

### Added

- **Standing Orders** — a new board in Captain's Log for tracking account-wide objectives. Create, edit, and complete
  orders, then link log entries (daily questions, killmails, industry deliveries, skill completions, and field notes) to
  the objectives they advance; each objective shows its linked activity as a running thread.
- Every character now has a Dossier tab showing their purpose, marching orders, and current training — it's available
  for all characters, even ones you haven't granted extra access to.
- Facility Intel entries can now carry a ship fit — paste an EFT fit into a facility and Pod pulls out its rigs
  automatically.
- The Captain's Log guided wizard now shows evidence strips for skill and industry completions (the pilot, the skill or
  built item, and run counts) and ends with a Commander's Log narrative step.
- AI assistants connected to Pod can now read and manage your standing orders and view character dossiers.

### Fixed

- Finished skills now show as trained right away instead of staying "untrained" until your next in-game login — Pod
  fills in completed skills from your skill queue.
- Days with no real activity no longer count as active in Captain's Log — a trading day that nets to zero ISK, or a day
  you marked complete but left empty, drops off the list (nothing is deleted).
- Facility Intel now sorts by the name shown on each card, so an entry displayed as "Trade Hub" is no longer sorted
  under "J" for its full "Jita - Trade Hub" name.

## [0.6.19]

### Added

- The "Currently training" skill card at the top of the Skills screen is now selectable — click it (with the usual
  Shift and Cmd shortcuts) to include the skill you're training in a plan built from your queue selection.

### Changed

- Corporation net worth now counts the value of the corp's assets, not just its cash — the All Wallets net-worth chart
  no longer drops when a corp turns ISK into assets like market buys or buy orders.
- Milestones are now added from a "+ Milestone" button in the skill-plan editor header (when sorting manually) or by
  right-clicking a skill and choosing "Add milestone above" or "below" — replacing the old hover band between rows.
- Tagging a single asset stack is now done by right-clicking it and choosing "Edit Tags"; the hover-only "+ Tag" button
  on each row has been removed.

### Fixed

- Hovering over the skills list or the assets list no longer makes rows flicker, shift, or jump under your cursor.
- The inventory list no longer goes blank when you change region or filter after scrolling down — it snaps back to the
  top of the new results instead.

## [0.6.18]

### Fixed

- Trades you make on behalf of a corporation show up on the Market tab again — a change in 0.6.17 hid them until the
  matching corporation wallet entry had synced, which for characters who trade entirely through a corporation wallet
  hid nearly every trade.

## [0.6.17]

### Added

- Corp office contents in your asset inventory now group into named hangar divisions — expanding an office reveals its
  hangars (and Corp Deliveries) as collapsible sub-nodes labeled with your corporation's division names, so items are
  easier to find. Division names now also appear for Directors who lack an Accountant role.

### Fixed

- Trades a character makes on behalf of a corporation no longer appear as orphaned rows on the Market tab — they stay
  hidden until the matching corporation wallet entry has synced, so budgeting can actually assign them.
- The right-click "Plan to" menu now works on skills already in your plan queue, not just the skill picker — right-click
  a queued skill to raise its target to a higher level.
- Assets whose icon can't be resolved now show a placeholder image instead of a blank tile.
- Blueprint icons now appear in the industry "Next to Complete" rail, which previously showed blank tiles.
- The skill detail card no longer stretches to fill the whole window — it now sizes to its content and centers on
  screen.

## [0.6.16]

### Added

- **Customizable Captain's Log** — a new Settings panel lets you edit the log's sections and questions: rename them,
  add your own, and control which appear each day. Your daily prompts, the guided wizard, and past-day views all follow
  your configuration, and the shipped defaults stay exactly as before until you change them. Fully translated across all
  languages.
- **Field notes** — jot a running list of free-form, timestamped notes on any day's Captain's Log, kept alongside the
  guided questions without affecting whether the day counts as complete.
- **Skill details** — an info button on both the Browse tab and the plan editor's skill picker opens a detail card
  showing a skill's group, rank, trained level, training attributes, prerequisites, description, and the SP and time
  needed for each level.
- **Skill plan quick actions** — right-click a skill in the plan editor's picker for a menu to plan it up to level II–V
  (each disabled once trained or already planned) or show its info.
- **AI assistant support for custom logs and notes** — connected AI assistants can now read your customized log
  structure and add, list, and delete field notes for you.

### Changed

- Expanded character and corporation cards now show at most three tag chips with a +N indicator for the rest, so tags
  no longer run off the card edge or clip the add-tag button.
- The Commander's Log one-liner and empty prompt fields can now be written or edited on any past day, not just today.

### Fixed

- Moving ISK between a personal wallet and an owned corporation's wallet on the same day no longer shows a phantom
  net-worth swing of hundreds of millions of ISK when your total net worth hasn't actually changed. Closes #43.

## [0.6.15]

### Added

- **Captain's Log** — a daily journal for your account, reachable from the roster's utilities menu and the sidebar.
  Every day gets an automated rollup of what happened across your characters: ISK earned and spent, net-worth change,
  kills and losses with ship icons, skills completed, industry jobs delivered, and calendar events. On top of that you
  can write a one-line Commander's Log, answer guided prompts (goals, blockers, lessons learned) through a
  step-by-step wizard, and add a personal note to any calendar event. Past days are browsable read-only with a
  jump-to-day calendar, dates show alongside their EVE (YC) equivalents, and incomplete days are flagged with a
  one-click way to mark them done.
- **Killmail after-action reports** — the killmail window now has Overview and Report tabs. The Report tab holds a
  debrief form: how it went (clean fight, costly, or learning experience), what happened, what you'd do differently,
  and the key takeaway. The same report appears in the Captain's Log combat step, so you can write it from either
  place. Kill notifications now open straight onto the Report tab and offer a Write debrief shortcut.
- **Daily log reminder** — a once-per-day notification and a small popup on the roster remind you to fill in today's
  log; both stay quiet once the day is complete, and the popup can be dismissed for the day.
- **AI assistant access to the log** — connected AI assistants can now read your log days and daily rollups, and
  (with local-write permission granted) fill in narratives, prompt answers, and kill reports for you.
- **Milestone export** — export a single milestone's skills to the clipboard, a CSV file, or a PSP file.
- **Collapsible milestones** — milestone sections in the plan editor can be folded up to navigate long plans.
- **Drag-to-reorder plans** — reorder skill plans and templates in the Manage Plans window by dragging.

### Changed

- **Facility Intel grid** — the facility intel screen in settings is now a compact three-column grid with sortable
  columns.
- **Sorting a skill plan no longer rewrites it** — sort is now just a view of the plan, so your manual order is never
  destroyed. Column headers cycle ascending, descending, and back to Manual, where insertion pills and drag return.
  Exports use whatever order is on screen.
- **Milestones no longer require a neural remap** — new milestones are plain section markers; only attaching a remap
  consumes a remap slot. Empty plans show a getting-started screen with quick actions.
- **Bigger default window** — new installs open at 1360x860 instead of 1280x800, so wide views like Facility Intel
  fit without cramping. Your saved window size is untouched.
- **Unresolved structures get a real label** — locations the app can't name now show "Structure #" with the
  structure's id number instead of a bare "Unknown location".

### Fixed

- Structures you can only reach through corp hangars no longer show as "Unknown location" — the app now asks every
  character you own with the right permissions to resolve the name, instead of only the character that synced.
- Renaming a milestone updates the name immediately — it previously appeared frozen while the plan was sorted — and
  the milestone header no longer overflows the window; Export gets its own labelled menu matching Import.
- The Manage Plans list refreshes when you save or close a plan editor, so new names and counts show up right away.
- Progress bars in the plan manager now fill all the way on finished plans.
- Manually picking a facility in the industry planner now uses the exact structure you picked.
- Fixed a potential crash when stacked progress bars had an empty segment.

## [0.6.14]

### Added

- **Skill plan milestones** — plans are now organized into named sections, each with an optional neural remap that
  can be auto-suggested or left unset. Sorting keeps skills inside their section, imports can target a specific
  milestone, and existing remap points convert to milestones automatically.
- **Milestone progress** — each plan in the Manage Skill Plans list shows a progress bar and a done/total milestone
  count, turning green when every milestone is complete.

### Changed

- **Side-by-side plan editors** — you can open several skill plan editors at once and compare plans, and the plan
  manager stays open when you open one. Reopening an already-open plan focuses its window instead of duplicating it.
- **Plans tab layout** — the New Plan and From Queue buttons moved above the plan list, From selected sits on its own
  row, and step-count badges match the design's rounded-square style.

### Fixed

- Use Stock in the industry planner now finds materials nested inside containers at your build sites — items stored
  deeper than one level (like minerals in a container inside a corp Office) were invisible before and never offset
  build requirements.
- Custom names on corporation assets now appear — EVE rejected the entire naming request whenever it included an item
  that can't hold a name, so no corporation names were ever saved.
- A skill queue whose last skill has finished now shows as empty instead of appearing to still be training in the
  queue view and on the roster card.
- Skill plan exports no longer include levels the character has already trained, and time-sorted plans keep
  prerequisite skills ahead of the skills that require them.
- The import-to and copy-to-character menus now scroll when you have many characters, so no entry is out of reach.
- Queue tab time breakdowns with tied durations no longer shuffle their order every redraw.
- Long station names on compact roster cards now wrap to a second line instead of painting over the ISK value.

## [0.6.13]

### Added

- **Queue tab** — the Skills window's right panel opens on a new Queue tab showing the live training queue's totals,
  skill injector needs, and time breakdowns by group and attribute pair, with plain-text and CSV export. Tabs are now
  ordered Queue, Attributes, Skills, Plans, and Browse is renamed Skills.
- **Skill plan CSV** — the plan editor's Export menu splits into To clipboard, To psp, and To csv, and From file… now
  imports CSV plans alongside `.psp` and plain text.
- **Template training times** — the Manage Skill Plans template list shows each template's total training time, and a
  template's editor summary adds total time plus the attribute optimization panel with an UNMAPPED column. Times are
  costed for a fresh unmapped character (all attributes at 17) and always agree between the list and the editor.
- **Esc closes overlays** — pressing Esc dismisses the topmost open modal or overlay, one per press when stacked,
  exactly like clicking outside it.

### Changed

- **Booster-aware attribute bars** — the Attributes tab splits each bar into base, implant, and booster segments, so
  an active cerebral accelerator is visible at a glance and training rates reflect it.

### Fixed

- Skill plan templates no longer show zero training time everywhere — per-step times, totals, the by-group and by-pair
  breakdowns, and exported CSV durations were all 0 before.
- Implanted or boosted characters get real remap recommendations instead of a permanent "out of spec" warning and a
  useless "already optimal" suggestion — attribute math no longer counts implants twice in the Attributes tab and the
  plan editor.
- When EVE returns mismatched attribute and implant data, Pod shows a notice and holds the remap suggestion until the
  next clean sync instead of recommending from bad numbers.
- Clicking labels, spacing, or any empty area inside a dialog no longer closes it and loses your in-progress input —
  clicking the dimmed background still dismisses.
- Corp-installed industry jobs now count against the installing character's Job Slots meters — running corp jobs left
  the meters near empty while the character was actually slot-capped in game; the corporation row is gone since
  corporations have no slot pool of their own.
- The MCP access token show and hide buttons show their proper labels in every language instead of a raw key.

## [0.6.12]

### Added

- **Roster view modes** — the Characters and Corporations panes each get a three-way density toggle in the search bar:
  full cards, compact cards, or full-width list rows. Pod remembers the choice per pane across restarts, and list rows
  support the same drag-to-reorder and squad assignment as cards.
- **Contact Sync** — build named lists of contacts and standings under Utilities, pick target characters, and Pod keeps
  those characters' in-game contacts reconciled in the background.
- **Accent color** — a new setting under User Interface offers eight presets or any custom hex color, applies live
  across every view, and is restored before first paint at launch.
- **Skill plan templates** — Manage Skill Plans gains a Templates tab with reusable, characterless plans costed from
  level 0; open one in the editor or import it onto any pilot.
- **EFT fit import** — paste an EFT fit from the clipboard and the skill plan editor stages the fit's required skills
  as plan entries.
- **Portable skill plan files** — plans export as tamper-checked `.psp` files instead of raw JSON; import auto-detects
  `.psp`, legacy JSON, and EVE plain text.
- **Budget reconciliation** — a Reconcile button on the Budget tab compares Pod's tracked balance with your actual
  liquid ISK and posts a single correcting entry, so Ready to Assign matches reality again.
- **Budget rules window and rule packs** — automation rules now live in their own window with drag-to-order priority,
  and can be shared as `.pbr` packs: export selected rules, preview an import, and skip likely duplicates.
- **Facility intel sharing** — export the structures and rigs you track as a `.pfi` pack and import one entirely
  offline; imports no longer need a signed-in character or dockable access to the structures.
- **Open pack files from your file manager** — installers and portable builds register `.pbr`, `.pfi`, and `.psp` with
  the OS, so double-clicking a pack (even several at once) opens Pod, detects the format from the file's contents, and
  asks before importing into the right view. Damaged or unsupported files are refused with a clear message.
- **Remaining-steps pill on Manage Skill Plans** — each plan row now shows how many steps its character still has to
  train, with 0 meaning fully trained.

### Changed

- **Skills header** — the queue ETA moved into the header stat cluster, plan actions folded into a Plan dropdown, and
  Manage Plans and Compare now sit in the header.
- **Roster names are links** — character and corporation names render as underlined links with a small arrow that open
  the detail view; clicking anywhere else on a corporation card no longer navigates.
- **Drag by the handle** — cards are picked up only by the six-dot grab handle in their corner, so clicking names,
  tags, or buttons can no longer start an accidental drag.
- **Card menu shortcuts** — a character card's right-click menu now jumps straight to each detail section (Clones,
  Contacts, Kill Log, Notifications, Standings).
- **One combined budget** — the Budget tab drops its scope picker; activity and Ready to Assign always cover every
  wallet together, while the ledger tabs keep per-scope filtering.
- **Facility intel is yours to keep** — tracked structures stay visible, editable, and exportable in Settings even
  after you lose docking access; cleaning up intel for a destroyed structure is now your call.

### Removed

- **Structure pinning** — the facility picker no longer keeps structures around after you lose access to them. It now
  shows only NPC stations, your corporations' structures, and live search results; a default facility you can still
  access is re-resolved automatically at planner load.

### Fixed

- Take over on a shared drive no longer gets stuck — a dead holder whose clock ran ahead, a failed sync pull, or a
  network share that rejects file replacement could each leave the Take over buttons doing nothing.
- The industry planner now counts items in corporation office hangars as usable on-site stock — corp hangar materials
  were silently ignored while character hangar stock matched.
- Tags created or edited in Settings now appear on roster cards and in the tag picker immediately instead of after a
  restart.
- Characters can no longer become unreachable in the roster grid after squad moves left gaps in the layout — affected
  cards now fold back into the first free slot.
- Opening a menu, modal, or picker no longer resets the scroll position of the view underneath it.
- The Manage Plans copy-to menu now floats above the plan cards instead of being clipped by them.
- The storage settings Release lock button now shows its proper label in every language instead of a raw key.

## [0.6.11]

### Added

- **Facilities settings** — the Industry settings tab is now Facilities, where you can record the structures you build
  in, fit each with up to three rigs, and see the resulting material efficiency, time efficiency, and install-fee
  effects. Each facility card shows its type and tier, security, system and region, and resolved owner.
- **Rig bonuses in the industry planner** — build plans now apply your tracked structures' fitted-rig bonuses (scaled by
  security band), so material amounts, job times, install fees, cost, and profit reflect the structures you actually
  build in. Untracked or unrigged structures are unchanged.
- **More for connected AI agents** — Pod's built-in AI-agent connection gains tools to list blueprints, market orders,
  and corporations, look up live Jita buy and sell prices and daily traded volume, and turn EVE ids into names. Read
  tools can now return a corporation's data as well as a character's, results include readable names alongside ids, and
  the tools that make changes offer a "dry run" preview and no longer create duplicates if a request is retried.
- **Take over a shared data drive** — when another copy of Pod is using data on a drive you share, you can now
  request to take it over, with a banner showing the request while it is in progress.

### Changed

- Your budget's category activity and Ready-to-Assign are now calculated directly from your wallet journal, so they
  always agree with your wallet and a transfer between two of your own corporations nets to zero.

### Fixed

- Assigning a budget category to a market transaction made on behalf of a corporation now takes effect — previously the
  assignment silently did nothing.
- The training-time estimate on the skills screen's featured pilot now matches the roster card, including when a
  training booster is active.
- Marking notifications as read now updates the History tab right away instead of waiting until you reopen the
  notification center.
- The update prompt on the splash screen now fits its window with the buttons fully visible and clickable — the
  "Update & restart" button could previously be clipped or ignore clicks.

## [0.6.10]

### Added

- You can now filter your roster by a trained skill — search `skill:` followed by a skill name, optionally with a
  minimum level (I-V or 1-5), to list every character who has it.

### Fixed

- Importing a skill plan copied straight from EVE's in-game window now works on Windows — it used to do nothing at all,
  and you'll now get a message confirming the import or explaining why it didn't work.
- Squad readiness now counts paused pilots separately instead of lumping them in with pilots still training — paused
  pilots show in amber and idle pilots in red.

## [0.6.9]

### Added

- **Pod now speaks your language** — German, Spanish, French, Japanese, Korean, Russian, and Chinese join English.
  Pick one from Settings → Accessibility or the new setup wizard, and Pod re-fetches game data (item, region, and
  faction names and the like) in that language, then restarts to apply it.
- **First-run setup wizard** — new installs now open a guided wizard that walks you through a welcome, choosing your
  language, turning features on or off, and picking where Pod keeps its data before the first launch.
- **Update check at launch** — Pod now checks for an available update before loading your data, so a release that
  fails to start can be updated past from the splash screen instead of leaving you stranded.
- **Missing-entry warnings for wallets** — Pod now notices when a wallet ledger has a gap (a break in its running
  balance, usually an entry that aged out of EVE's history) and warns you once per gap.

### Changed

- **More accurate asset prices** — your held items are priced from zKillboard market data wherever it has a price,
  falling back to EVE's own price only when it does not, so inventory and net-worth values track the real market more
  closely.
- The main window now opens wider by default on new installs.

### Fixed

- Internal wallet transfers — between two of your characters, or between two of a corporation's wallet divisions — are
  no longer silently dropped or counted twice, so wallet balances and your budget's income and spending totals are now
  correct. Pod re-fetches your full wallet history once after updating to repair past records.
- Expanding a container while a search or filter is active now shows only the items that match, instead of every
  unrelated item that happened to share the container.
- The "worth reprocessing" highlight on inventory rows now appears as a thin bar on the left edge instead of tinting the
  whole row.
- Dropdown menus and the navigation rail flyout now layer in the correct order, and clicking outside an open dropdown
  both closes it and acts on what you clicked in a single click.

### Removed

- The wallet's right-hand summary pane (Flow, Recent activity, and By category) has been removed — it could show totals
  that disagreed with the rest of the app; Budget → Reflect gives the same breakdown correctly over your whole journal.

## [0.6.8]

### Fixed

- Pod now starts correctly on Windows — a recent update could leave some Windows installs unable to open with a
  startup error, and Pod now repairs the affected data automatically on launch, with nothing lost.

## [0.6.7]

### Added

- **Anonymous, opt-out usage data** — a new Telemetry section in Settings lets you share anonymous usage, performance,
  crash, and environment data to help improve Pod. It is on by default with a master switch and a toggle per stream,
  shows a "never collected" list and a live preview of exactly what would be sent, and uses a random id that is never
  linked to you. Your IP is never recorded, and crash reports are stripped of personal details before they leave your
  machine.
- **Tag your items** — label inventory rows with your own tags, select several rows to tag them at once, filter the
  inventory by `tag:`, and manage your asset tags from a new tab in Settings → Tags.
- **Worth-reprocessing badge** — inventory rows worth more reprocessed than sold now show a warning badge and a
  reprocessed-value line, so you can spot reprocessing opportunities at a glance.
- **Sort the location tree** — the Assets location sidebar has a new A–Z / Value toggle (your choice is remembered), and
  containers holding items that match your filter now expand automatically.
- **Back up and move your data** — export everything to a single archive file from Settings → Storage and import it on
  another machine. Importing backs up the current data first and refuses an archive from a newer version of Pod.
- **Search contracts by their contents** — the Contracts filter now matches the names of items inside a contract, not
  just the contract title and parties.
- **Mail date separators** — the message list groups messages under Today, Yesterday, This Month, and month headers,
  with a date on every row.

### Changed

- **Native windows** — the detached killmail, contract, mail compose, skill plan manager, and stockpile editor windows
  now use your operating system's own title bar and controls, and multibuy import and calendar event details open in
  their own windows too — you can have several calendar events open at once.
- **Notifications** — the notification center now has New and History tabs with a "Mark all read" button; marking a
  notification read removes it from New, History scrolls back through about 90 days, and toasts show a colored icon for
  each kind of event.
- **Calmer sync status** — the sync indicator leads with a plain status ("Up to date", "Catching up", or how many items
  need attention), and the details popover spells out each row's state more clearly.
- **Roster** — holding a dragged character card near the top or bottom edge now scrolls the roster, and a paused skill
  queue is shown distinctly from an empty one on the card and the skills screen.
- **Skill plans** — plan cards now show how many steps remain and how many distinct skills are involved, and a Manage
  Plans button sits in the skills header next to Compare.
- **Supercapitals and rarely-traded hulls** now show a reference price in your inventory and net worth instead of 0 ISK.
- **Connect an AI agent** — the MCP settings tab gives honest, per-app setup guidance, and the built-in server now uses
  the current Streamable-HTTP transport with typed inputs for every tool.

### Fixed

- Right-clicking a row in Assets, the wallet journal, or transactions now opens the menu at your cursor instead of the
  top-left corner, and no longer jumps the list back to the top.
- The wallet ledger keeps its place when the budget-assign menu opens, instead of scrolling to the top.
- Marking a notification as read now removes it from the New list, matching the rest of the redesign.
- Killmails no longer list characters who were not actually involved in the kill.
- Budget assignments stay consistent across your characters and corporation after a wallet sync.
- Creating a tag with a name that already exists (in any capitalization) now reuses the existing tag instead of making
  a duplicate, which was most noticeable when tagging characters in the roster.

### Removed

- The "Clear all" button in the notification center — reading notifications or letting them age out keeps the list tidy.

## [0.6.6]

### Added

- **In-app notifications** — a bell on the navigation rail shows your unread count and opens a center listing recent
  notifications, and new events also pop as toasts in the corner. Pod notifies you once for new mail, killmails,
  finished skills, completed industry jobs, calendar reminders, and moon extraction events, and clicking one jumps
  straight to the relevant screen.
- **Detached windows** — the killmail viewer, contract details, the stockpile editor, and the mail composer now open as
  separate resizable windows you can move around and keep open side by side, instead of panels inside the main window.
  You can have several open at once, and an unsent mail is saved to Drafts when you close its window.
- **Manage Skill Plans** — a new window lists every owned character with their plans in one place, so you can open,
  create, or delete a plan for any character and copy a plan from one character onto another.
- **MCP server** — an optional local server (off by default) lets AI agents such as Claude Desktop or Claude Code read
  your Pod data and take actions for you. You turn it on, set the port, copy a bearer token, and choose which read and
  write permissions to grant from the new MCP tab in Settings.
- The command palette now offers **Compose mail**, **Create stockpile**, and **Manage skill plans** commands so you can
  open those windows from anywhere without first opening the related screen.
- You can now **pin and reorder the wallet balance sections**, and the net-worth hero on the ledger tabs is collapsible.

### Changed

- **Skill plans now store the complete plan** — a plan keeps every skill and prerequisite and works out what is left to
  train per character, so the same plan reads correctly for any character and copying or importing it onto a
  less-trained character reproduces the full plan. The plan view now hides levels a character has already trained
  instead of dimming them.

### Fixed

- **Budget Ready-to-Assign is accurate again** — transfers between your own wallets are recognized and kept out of the
  budget, money is conserved when you assign it, assignments only go to the wallet owner that holds the entry, and the
  figures stay correct as you sync or switch month and scope.
- Budget needs-review now counts only uncategorized expenses, automation rule previews match what actually gets
  assigned, and rule names and conditions are checked before a rule is saved.

## [0.6.5]

### Added

- **Command palette** — press `/` (or click the rail's palette button) to open a searchable overlay that jumps to any
  screen or sub-tab, runs common commands, and opens any of your characters or corporations.
- **Keyboard shortcuts** — Ctrl/Cmd+K focuses the current screen's search (Mail, Industry planner, Skills, Roster,
  Settings, and the Wallet ledger), Ctrl/Cmd+, opens Settings, and Ctrl/Cmd+Q quits, now on every platform.
- Hovering a rail icon now pops a **flyout** to its sub-tabs so you can jump straight to one; a new User Interface
  setting switches the rail's sub-navigation between this flyout, a persistent sub-rail column, or none.
- **Granular feature toggles** — the Features settings tab is now a two-level list where each feature group can be
  switched on or off as a whole or tuned sub-feature by sub-feature (for example, turn off just Budget or just Location
  tracking), and the matching tabs, character-card sections, and data syncs follow your choice immediately.
- Industry build-order jobs can be **split across pilots** — right-click a job to split its runs into segments or merge
  them back, then assign each segment a pilot and one of their clones, and the assigned pilot's industry skills and
  clone implants shorten that segment's build time.
- Budget now keeps each trade's full cost together — assigning a market transaction also files its transaction tax and
  broker's fee into the same envelope, and a corp-on-behalf trade that lands in both a character and a corporation
  wallet shows a combined portrait-and-logo avatar and assigns across every copy at once.

### Changed

- The Characters screen is now labeled **Roster** in the navigation.
- Industry planner install fees now use EVE's real job-cost formula (estimated item value × cost index × structure
  bonus, plus facility tax and the SCC surcharge), so the planner's profit, margin, and ISK-per-hour figures line up
  with what the game actually charges.
- In the Wallets balance tab, each row's share bar now fills relative to that section's subtotal, so the bars read as a
  composition that adds up to the whole instead of always pinning the largest wallet to full width.

### Fixed

- Ready to Assign no longer swings wildly negative — ISK transfers between your own wallets now cancel out correctly
  instead of inflating income.
- The budget "review & assign" count now reflects every uncategorized entry for the month from your full ledger,
  instead of under-reporting until you scrolled.
- The Roster no longer blanks out to "Couldn't load characters" on a brief database hiccup — transient timeouts retry,
  and a failed refresh keeps showing your last-good roster.
- Hard-to-price, infrequently-traded items now value from zKillboard's live price instead of a years-old snapshot that
  could read far too low.
- Assigning a batch of ledger rows to an envelope no longer jumps the list back to the top, and a duplicate
  "View transactions" button was removed from the inspector.
- Wallets balance cards now draw evenly weighted dividers between rows instead of a darker line between rows than at the
  card edge.
- The Rail Side preview in Settings now lays out at a sensible width instead of stretching across half the column.

### Performance

- The app stays responsive during a data sync — character and wallet screens no longer freeze or time out while a large
  background sync is writing, because reads and writes no longer contend over the database.
- Syncs put less strain on the app — pages are fetched in bounded batches, cache updates are written in fewer larger
  operations, and the roster refreshes less frantically during a sync burst.

## [0.6.4]

### Added

- Budget envelopes can now have **automation rules** — set up search-based rules from a category's new Automation tab,
  or manage them all in one place with the global rules manager, and matching ledger entries file themselves into the
  right envelope automatically.
- Automation rules (and manual overrides) match both spending and income — money a rule files into an envelope is held
  out of Ready to Assign so it's never counted twice.
- **Move money** between budget envelopes — from a category's Available pill or its inspector, move any amount to
  another category or back to Ready to Assign, the YNAB way.
- New **Wallets** tab in the Wallet feature — a read-only, sortable balance overview that opens by default, listing
  every pilot's wallet and each corporation wallet broken out by division.
- You can now select multiple Journal or Transactions rows — plain, Ctrl/Cmd, or Shift click — and assign them all to a
  budget envelope at once from the right-click menu.
- The All-Wallets Journal and Transactions tabs now show corporation rows alongside character rows, so corp money is
  visible and assignable instead of counting toward the budget while staying hidden.
- Budget category **groups** can now be dragged to reorder them, not just the categories inside them.

### Changed

- Goal-by-date targets now pace toward their due date — the monthly amount needed shrinks as the goal funds and grows
  as the deadline nears, instead of always showing the full remaining amount.

### Fixed

- Ready to Assign is now a single running balance across all months, so ISK assigned in a future month can no longer be
  assigned twice, and spending in any earlier month now carries forward correctly.
- A budget category scoped to All Wallets no longer over-reports spending — a single market trade mirrored across a
  character and corporation wallet is now counted exactly once.
- Dragging budget categories to reorder them works again, and the per-row budget category picker no longer comes up
  empty under a character or corporation filter and now flips open upward when near the bottom of the screen.
- Adding, deleting, renaming, or reordering a budget category now updates the ledger's envelope picker immediately
  instead of leaving it stale until you revisit the tab.

## [0.6.3]

### Added

- New **Budget** tab in Wallet — a zero-based (envelope) budget you can plan and review. Assign each month's ISK to
  categories, watch your Ready-to-Assign balance, cover overspending in a click, and see unspent balances carry forward
  month to month. The Plan view's detail inspector is drag-resizable and remembers its width.
- The Budget tab's **Reflect** view reports your net change, income vs. spending by month (3- or 6-month range), the
  average age of the ISK you spend, spending by category, and which targets need attention.
- Every Wallet Journal and Transactions row shows a budget category chip you can click to assign or reassign which
  envelope it counts against — and a market trade and its matching journal entry stay in sync so it's only counted once.
- A "Review & assign" banner shows how many of the month's entries still need a category and jumps you straight to them,
  with per-category "View transactions" links to filter the ledger to one envelope.
- The Assets **Values** tab total is now computed over your entire asset set instead of only the items currently
  loaded on screen, so it no longer under-reports until you scroll.

### Changed

- Wallet tabs now sit at the top of the page (matching Industry and Assets), the **Market** tab is renamed
  **Transactions**, and each row's Buy/Sell now shows as a colored chip.
- Abyssal modules and other hard-to-price items now value more accurately — character and corporation abyssals are
  priced from MutaMarket, and items EVE doesn't price fall back to zKillboard market data instead of showing 0 ISK.

### Fixed

- Portraits in the contact add/edit window now self-heal — broken or missing character and corporation images reload
  instead of staying blank.
- Window backgrounds — notably the Skill Plan editor and the Skills Compare header — now use Pod's dark surface color
  instead of a lighter built-in shade that washed out the depth of cards.

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

[Unreleased]: https://github.com/aaronmallen/pod/compare/0.7.1...HEAD
[0.7.1]: https://github.com/aaronmallen/pod/compare/0.7.0...0.7.1
[0.7.0]: https://github.com/aaronmallen/pod/compare/0.6.20...0.7.0
[0.6.20]: https://github.com/aaronmallen/pod/compare/0.6.19...0.6.20
[0.6.19]: https://github.com/aaronmallen/pod/compare/0.6.18...0.6.19
[0.6.18]: https://github.com/aaronmallen/pod/compare/0.6.17...0.6.18
[0.6.17]: https://github.com/aaronmallen/pod/compare/0.6.16...0.6.17
[0.6.16]: https://github.com/aaronmallen/pod/compare/0.6.15...0.6.16
[0.6.15]: https://github.com/aaronmallen/pod/compare/0.6.14...0.6.15
[0.6.14]: https://github.com/aaronmallen/pod/compare/0.6.13...0.6.14
[0.6.13]: https://github.com/aaronmallen/pod/compare/0.6.12...0.6.13
[0.6.12]: https://github.com/aaronmallen/pod/compare/0.6.11...0.6.12
[0.6.11]: https://github.com/aaronmallen/pod/compare/0.6.10...0.6.11
[0.6.10]: https://github.com/aaronmallen/pod/compare/0.6.9...0.6.10
[0.6.9]: https://github.com/aaronmallen/pod/compare/0.6.8...0.6.9
[0.6.8]: https://github.com/aaronmallen/pod/compare/0.6.7...0.6.8
[0.6.7]: https://github.com/aaronmallen/pod/compare/0.6.6...0.6.7
[0.6.6]: https://github.com/aaronmallen/pod/compare/0.6.5...0.6.6
[0.6.5]: https://github.com/aaronmallen/pod/compare/0.6.4...0.6.5
[0.6.4]: https://github.com/aaronmallen/pod/compare/0.6.3...0.6.4
[0.6.3]: https://github.com/aaronmallen/pod/compare/0.6.2...0.6.3
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
