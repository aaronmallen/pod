---
title: Wallet
section: Features
order: 4
---

# Wallet

The Wallet feature tracks ISK across every character and corporation you have
added. It collects wallet journal entries, market transactions, and contracts,
and it builds a net-worth history from daily snapshots. The window has three
tabs: Journal, Market, and Contracts. A header at the top summarizes the active
scope, and a hero panel below it shows the net-worth graph and the breakdown of
what makes up your balance.

## Header and scope

The header runs across the top of the window. On the left sits the scope
picker. Open it to switch between three groups: All Wallets, your individual
characters, and your corporations. The All Wallets row combines every
character's liquid ISK and shows the count of characters folded together. Each
character row shows the pilot's name, corp, and liquid balance. Each
corporation row shows the corp name and ticker. If a character is missing a
scope the Wallet feature needs, its row carries a re-auth marker so you know to
sign that pilot in again.

Three stat blocks follow the picker. Liquid ISK is the cash on hand for the
active scope. Net worth shows the estimated total value, labeled "est." for a
single character or "all characters" when All Wallets is active. Change shows
the movement over the selected timeframe, written with a sign and colored green
for a gain or red for a loss. When there is not enough history to compute a
change, the field shows a dash instead of a number.

## Net-worth hero

Below the header, the hero panel leads with the net-worth figure for the active
scope in large type, followed by an ISK suffix. A small chip under the number
repeats the change for the current timeframe with an up or down arrow, green
when the value rose and red when it fell.

To the right of the number sit three composition chips that split net worth into
its parts. Liquid is your cash, drawn in plasma. Assets is the appraised value
of what you own, drawn in muted grey. Escrow is ISK locked in standing market
orders, drawn in red. Each chip pairs a colored dot with the formatted ISK
amount so you can see at a glance where your wealth sits.

A timeframe selector lets you choose how far back the panel looks: 1W, 1M, 3M,
6M, or 1Y. The default is 3M. The selection drives both the graph and the
change figures in the header and hero.

The graph itself is a line chart of net worth over the chosen window, with the
liquid portion drawn as a second series labeled Liquid. The line turns green
when net worth is up across the window and red when it is down. Hover anywhere
along the line and the hero number switches to the value at the nearest day, so
you can read a specific point in your history. The graph is built from daily
aggregation snapshots. If fewer than two days of history exist, the chart is
replaced with a note that the net-worth history will appear after the next
daily aggregation run.

![Wallet journal and net-worth hero](/docs/img/wallet/journal.png)

When All Wallets is active, a stacked bar under the graph splits net worth by
character. Each segment is sized by that character's share, and a legend below
the bar names every character with its ISK total and percentage of the whole.

## Journal tab

The Journal tab lists wallet journal entries: bounties, mission rewards,
transfers, market settlements, and every other ref type the game records. Each
row shows the entry description and a humanized ref type on the left, the
character's portrait and name in the middle, and the signed amount with a
relative timestamp on the right. Income is marked with a green badge and a plus
sign; spend is marked with a red badge and a minus sign.

The tab is paginated by cursor. It loads the first fifty entries and fetches the
next page as you scroll past roughly four fifths of the list, so a long history
streams in without loading everything at once. The total entry count appears on
the tab itself.

Two filters sit above the list. The sign filter has three options: All, In, and
Out. All shows every entry. In narrows the list to income, the entries with a
positive amount. Out narrows it to spend, the entries with a negative amount.
The timeframe selector reuses the same 1W through 1Y choices as the hero, so you
can scope the journal to a recent window or the full year.

## Market tab

The Market tab lists your market transactions as buy and sell orders. Each row
shows the side, the item, the quantity, the unit price, the order total, the
location, the character who placed it, and when it happened. Buy rows are marked
with a down arrow and "BUY" in red. Sell rows are marked with an up arrow and
"SELL" in green.

Like the Journal tab, the Market tab pages in fifty rows at a time by cursor and
fetches more as you scroll. A side filter across the top switches between All,
Buy, and Sell so you can isolate one direction of trade. The sign filter applies
here too: In shows sells, Out shows buys.

## Contracts tab

The Contracts tab lists item exchange, courier, and auction contracts tied to
your characters. Each row shows the contract type, its status, the issuer, the
counterparty, the value, the collateral, and when it was issued. The
counterparty resolves to the acceptor when one exists, otherwise the assignee,
otherwise the issuer.

Status is shown in uppercase next to a colored dot. Outstanding contracts read
in yellow, in-progress contracts in plasma, and finished contracts in green.
Cancelled and deleted contracts are greyed out, while outbid, failed, rejected,
and reversed contracts are red. A contract that is still outstanding or in
progress past its expiry date is shown as expired.

The Contracts tab pages by cursor in the same way as the other tabs and carries
its own total count. A side filter lets you narrow to buy or sell contracts.

![Contracts tab](/docs/img/wallet/contracts.png)

## Contract detail

Select any contract row to open the contract detail modal. The header names the
contract kind, its title or a fallback contract number, the location, when it
was issued, and the contract ID, with a colored status badge alongside.

A Parties section shows the people involved with their portraits. The issuer
appears first with their corporation, or "Public contract" when there is no
named recipient. Below a divider, the other party appears when one exists,
labeled Acceptor on a finished contract or Hauler on one in progress. When no
one has taken the contract, the section reads "Open to anyone" or "Assigned,
awaiting acceptance" depending on how it was offered.

A headline panel shows the contract's money figure in large type. The label
reflects the contract: Price, Reward, Current bid, or You pay. A small grid
beneath it lists the collateral and either the volume, for courier and exchange
contracts, or the buyout, for auctions. A Terms section spells out the type, the
availability (Public or Personal), the days allowed to complete the contract,
and when it expires or that it has completed.

For courier contracts, a Route section draws the path from pickup to
destination with the two locations and an arrow between them. The cargo itself
appears in a manifest panel, headed "Cargo manifest" for couriers or "Contract
items" otherwise, with an estimated total. Each item lists its icon, name,
quantity, and per-unit value, and items are flagged as assembled or requested
where that applies, so you can read the fitting and cargo item by item.

For auctions, a Bids section lists each bid with the bidder, when it was placed,
and the amount. The leading bid is highlighted and tagged as the high bid.

![Contract detail modal](/docs/img/wallet/contract-detail.png)

## Corporation wallets

Selecting a corporation in the scope picker switches the window to that corp's
books. Corporations keep their balances across seven wallet divisions, and a
division selector appears as a horizontal strip of buttons. Each button shows
the division name, falling back to "Division" and its number when the corp has
not named it, along with that division's balance. The first division is selected
by default. If no divisions have synced yet, the strip shows a note that corp
wallet sync will populate them.

The All Wallets scope sits at the top of the picker and combines every
character's liquid ISK into one view, with the net-worth graph and composition
split across all of them. Use it for a single read on your whole financial
position, then drop into a single character or a corporation division when you
need the detail.
