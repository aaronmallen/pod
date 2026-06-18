---
title: Assets
section: Features
order: 5
---

# Assets

The Assets window holds everything you own across your characters and corporations.
It has five tabs along the top: Inventory, Abyssals, Stockpiles, Values, and Tracker.
Each tab carries a count badge. Inventory shows the total item count in scope, Abyssals
shows the number of mutated modules, Stockpiles shows how many stockpiles you have defined,
and Values shows the number of owners in scope. Tracker carries no badge.

## Inventory

The Inventory tab pairs a location tree on the left with a filterable, sortable item table
on the right.

![Inventory tab](/docs/img/assets/inventory.png)

### Category pills and the search box

Above the table sits a row of ten category pills: All, Ships, Modules, Drones, Charges,
Implants, Blueprints, Materials, Skill Books, and Commodities. All is selected by default
and shows every item. Picking any other pill narrows the table to that one category. The
pills map directly onto the `category:` query token, so the Ships pill is the same as typing
`category:ship`.

The search box sits next to the pills. Its placeholder reads
`Filter assets…  try  name:Rifter  or  category:ship`. Typing filters the table as you go.
Type a plain word to search loosely, or use a prefix token to target one field. The three
summary badges below the table, Rows, Value, and Volume, reflect the rows currently loaded
into the page.

### The filter query language

The filter box understands a small token language. You can mix any number of tokens in one
query, separated by spaces. Tokens are combined with AND, so every token must match for a
row to appear.

A token is either a bare word or a `key:value` pair. Keys are case-insensitive. Wrap a value
in double quotes to keep spaces inside it, for example `region:"The Forge"`. Put a comma
between values to match any of them, so `category:drone,ship` returns drones or ships. Put a
`-` in front of a token to exclude its matches, so `-category:ship` hides all ships.

These are the recognized keys:

- `name:` (alias `n:`) matches the item type name loosely. `name:rifter` finds anything whose
  name contains "rifter".
- `group:` (alias `g:`) matches the item group name loosely. `group:frigate` covers every
  frigate group.
- `category:` (alias `cat:`) matches the category exactly and ignores case. Valid values are
  `ship`, `module`, `drone`, `charge`, `implant`, `blueprint`, `material`, `book`, and
  `commodity`.
- `region:` (alias `r:`) matches the region name exactly and ignores case. Quote multi-word
  region names.
- `constellation:` (alias `c:`) matches the constellation name exactly and ignores case.
- `system:` (alias `s:`) matches the solar system name loosely. `system:jita` covers Jita and
  any name that contains it.
- `location:` (alias `loc:`) matches the station or structure name loosely.
- `owner:` accepts the single value `me`. `owner:me` keeps only items owned by your active
  character. Any other value matches nothing.
- `type:` accepts `bpc` for blueprint copies, `bpo` for blueprint originals, `singleton` for
  unstacked single items, and `stack` for stacked quantities.

Loose keys match a value anywhere inside the field, so the value is treated as a substring.
Exact keys (`category`, `region`, `constellation`) require the full name. A bare word with no
prefix searches four fields at once: item name, type name, group name, and location name. So
typing `tritanium` finds it whether that text lands in the item name or the location. An
unknown key like `clone:` matches no rows rather than falling back to free text.

### Sorting

Click a column header to sort by it. The sortable columns are Item, Group, Category, Qty,
Volume, Unit, Value, and Owner. The table opens sorted by Value, highest first. Click the
same header again to flip the direction. The Location column is fixed and does not sort.

### The location tree

The left tree groups your assets by where they sit in space. It nests from region down to
constellation, then solar system, then the individual station or structure. System rows show
their security status. Every node rolls up the item count and the total ISK value of
everything beneath it, so a region row sums its constellations and a constellation row sums
its systems.

Click a node to filter the table to that location. Locations that cannot be resolved to a
system, such as a structure you can no longer dock at, collect under an orphan group at the
bottom of the tree.

## Abyssals

The Abyssals tab lists your mutated modules. Mutated modules roll random stats within a band,
so they each carry their own values rather than a fixed type price.

![Abyssals tab](/docs/img/assets/abyssals.png)

Each module appears as a card with its icon, name, owner, location, and an estimated value.
A colored tier badge marks the mutation grade: Unstable, Gravid, and Decayed, plus the
Glorified variants. The badge color encodes the tier so you can scan grades at a glance.

The sidebar holds a range filter, one slider per rolled stat for the selected module type.
Each slider has a low handle and a high handle. Drag them to keep only the modules whose stat
falls inside the band you set. A slider you have narrowed off its full range is highlighted so
you can see which stats are active. Cards load in pages as you scroll.

## Stockpiles

A stockpile is a named target list of items you want on hand at a location. The Stockpiles tab
shows each one as a card with a fill bar.

![Stockpiles tab](/docs/img/assets/stockpiles.png)

Every item line in a stockpile shows three numbers: have, target, and the deficit. Have is how
many you currently own across the characters in scope, target is how many you want, and the
deficit is the shortfall, which is target minus have when have falls short. A per-item progress
bar shows the fill as have over target. The card also rolls up an overall fill percentage and
the ISK cost to close the gap.

Right-click a stockpile card to open its context menu. It has three actions: Edit opens the
stockpile editor, Export to Multibuy copies the contents in EVE multibuy format, and Delete
removes the stockpile. When you export to multibuy, a Target / Remaining toggle controls
whether the list contains the full target quantities or only the remaining deficit.

### The stockpile editor

The editor opens in a modal where you set the name, the scope, the location, and the item
targets.

![Stockpile editor](/docs/img/assets/stockpile-editor.png)

Scope is a set of characters. Picking which characters count toward the stockpile updates the
have figures live, so you can preview which pilots' assets the stockpile pools as you change
the selection.

The location field is a combobox with live search. Type part of a station or structure name and
it queries your reachable locations, both from a search and from the offline cache, then lists
the matches with their security status. Pick one to anchor the stockpile there.

To add items, type a name into the item search and pick a match from the suggestions. Each item
row gets a target quantity field you can edit, plus a control to remove the row.

### Importing a multibuy list

Instead of adding items one at a time, paste an EVE multibuy list into the import field and let
the editor resolve it.

![Stockpile import](/docs/img/assets/stockpile-import.png)

The parser reads the formats EVE produces. It accepts a name followed by a quantity, a quantity
followed by a name, and the `x1000` and `1000x` shorthands. It reads grouped digits whether the
separator is a comma, a period, an apostrophe, an underscore, or a space, so `1,000` and `1 000`
both read as one thousand. A bare name with no quantity counts as one. Duplicate names are
summed. Once resolved, the matched items are appended to the editor's item list.

## Values

The Values tab breaks your net worth down by who owns what and where it sits.

![Values tab](/docs/img/assets/values.png)

The main view is a matrix. Rows are owners, your characters and corporations, and columns are
locations. The leftmost column header reads Owner. Each cell holds the ISK value that owner
holds at that location, and the row and column totals give you the per-owner and per-location
sums. The tab also lists the top ten items by value under "Top items by value" and breaks the
total down by category under "By category". The grand total is your net worth across everything
in scope.

## Tracker

The Tracker tab charts your net asset value over time.

![Tracker tab](/docs/img/assets/tracker.png)

The chart covers the last 90 days. Net asset value is your total holdings recorded as a daily
snapshot, so the line shows how your wealth has moved day to day. Above the chart sit five
tiles: Current is the latest value, 90-day change is the difference across the window with its
percentage and a color for the direction, High and Low are the peak and trough inside the
window, and 30d avg is the average of the most recent 30 days.
