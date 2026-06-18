---
title: Industry
section: Features
order: 7
---

# Industry

The Industry feature pulls four tools into one rail entry: a Jobs tab that tracks
running and finished industry jobs, a Blueprints tab that lists your blueprint
library, a Planner that works out what a build costs and what it needs, and an
Extractions tab that shows corporation moon-mining timers. Switch between the
four tabs from the row at the top of the window.

A scope picker sits above the tabs. It defaults to All, which combines every
authorized pilot and every corporation you own into one view. You can narrow it
to a single character or a single corporation. When a character is missing the
ESI scopes that a tab needs, that character drops out of the combined view and a
banner names the pilots you still need to re-authorize.

## Jobs

The Jobs tab lists the industry jobs Pod has synced from ESI for the current
scope. It covers every activity EVE runs through the industry system:
manufacturing, time research, material research, copying, invention, and
reactions. Each job carries a short activity tag so you can read the list at a
glance: MANUF for manufacturing, TE for time research, ME for material research,
COPY for copying, INVENT for invention, and REACT for reactions.

![Jobs tab](/docs/img/industry/jobs.png)

A filter bar across the top splits the jobs three ways. All shows every job and
is the default. In progress shows jobs that have not reached their end time yet.
Ready shows jobs that have passed their end time and are waiting for you to
deliver them. Each filter button carries a running count, so you can see how many
jobs are active and how many are ready without changing the view.

Next to the filter sits a group-by control with four options: None, Owner,
Activity, and Facility. None leaves the list flat. Owner groups the jobs under
each character or corporation. Activity groups them by job type. Facility groups
them by the station or structure the job runs in. Grouped lists get a header per
group that names the group and counts how many jobs it holds and how many are
ready.

Each row shows the job's progress. A running job displays a percentage; a
finished job reads as complete and shows a green check with Ready. The countdown
shows the time left, and for a finished job it shows the date and time it
completed instead. Run counts read in the units the activity uses: runs for
manufacturing, tries for invention, and copies for copying. Invention jobs also
show their success chance when ESI reports it.

A resizable detail rail on the right shows the selected job. Drag its edge to
widen or narrow it.

## Blueprints

The Blueprints tab lists your blueprint library with the efficiency and run data
that the Assets window does not carry. It reads from a dedicated ESI blueprints
endpoint, so it knows each blueprint's material efficiency, time efficiency, and
remaining runs.

![Blueprints tab](/docs/img/industry/blueprints.png)

A segmented control filters the list by kind: All, Originals, and Copies. An
original (BPO) has unlimited runs and shows an infinity marker where the run
count would go. A copy (BPC) has a finite number of runs and shows how many it
has left. Each filter button carries a count of how many blueprints fall under
it. You can sort the list by Name, by ME, or by Runs.

Each row names the blueprint, tags it as a BPO or a BPC, and shows what it makes
as a subtitle. Material efficiency and time efficiency each show their value next
to a ten-dot meter that fills in proportion to the value. Reaction blueprints
have no ME or TE, so those read as not applicable. The run column shows the
original marker for a BPO and the remaining runs for a BPC. The row also shows
where the blueprint sits, naming the station or structure and its system.

A Plan Build action on each row seeds that blueprint's product into the Planner
and switches you to the Planner tab, so you can go straight from a blueprint to a
full build plan.

## Planner

The Planner is a recursive build planner. You pick a product, and it works out
the full tree of jobs and materials needed to build it, sums the cost, and tells
you whether the build turns a profit at current prices.

![Planner overview](/docs/img/industry/planner-overview.png)

The Planner opens cold with no product selected. The left pane shows a product
search over the seeded build catalog. Type to filter by name and pick the item
you want to build. Nothing auto-selects, so the first thing you do is choose a
product. Once you pick one, the Planner builds the plan and fills in the rest of
the panes.

### Bill of materials and merged build order

The bill of materials lists every raw input the plan needs once the tree is
expanded down to materials you buy rather than build. Each line shows the item,
the quantity needed, the on-hand amount when you have stock, the unit price, and
the line total. The bill is searchable and collapsible, so a large build stays
readable.

The merged build order lists the jobs the plan runs. It collapses duplicate jobs
that share the same item, ME, TE, and facility into one line and sums their
demand, so you see one entry per distinct job instead of a repeated stack.
Producer jobs list before the jobs that consume their output. Each line names the
item, the runs, the ME and TE, and the facility.

### Per-type build cards

Each item type in the plan gets one build card. The card carries the settings
that decide how that type is made: a run count, an ME value, a TE value, a
facility, and the cost index read for that facility's system. Manufacturing
defaults to ME 10 and TE 20; reactions have no ME or TE and leave those fields
blank.

Every type also has a build-or-buy choice. Mark a type as built and the Planner
descends into its own recipe and adds its materials to the tree. Leave it as buy
and the Planner treats it as a raw input you purchase, and the bill of materials
lists it directly. Because the planner is recursive, switching a deep component
from buy to build re-expands that whole branch.

### Picking a facility

The facility for each type comes from a live-search combobox. Type at least three
characters and Pod runs an ESI character search for matching stations and
structures, debounced so it does not fire on every keystroke. Below three
characters it falls back to the facilities you already have access to. The list
covers NPC stations, your corporation's structures, and structures you have
pinned. Picking a facility sets the cost index that the economics use for that
type's jobs.

### Breaking down to raw materials

You do not have to expand the tree one component at a time. A break-down-all
control expands the entire plan to its raw materials in one step. Each buildable
line in the build order also carries its own breakdown action, so you can descend
a single branch while leaving the rest as bought components. The expansion stops
at any type you have left as buy.

### Using stock you already hold

When you hold materials at a build site, the Planner can draw from them instead of
buying. A type that has on-hand stock at its facility shows a use-stock toggle.
Turn it on and the Planner allocates from the on-hand pool and nets the needed
quantity in the bill of materials down by what it drew. The pool is shared, so
two jobs that both want the same material draw from it in the order you toggle
them, and the planner never counts the same unit twice or drops a needed quantity
below zero. A line drawing from stock shows a stock chip in place of the toggle.

### Economics

The detail pane sums the plan into a set of numbers. Material cost is the total
of every raw input the bill of materials says you must buy. Each in-house job adds
an install fee, computed as the job's output value times the facility's cost
index times the install rate, so the cost matches what EVE charges to start the
job. Revenue is the product's market price times the output quantity. Profit is
revenue minus material cost and install fees, and margin is that profit as a
percentage of revenue. The pane also shows the per-unit cost, the total build
time summed across every job, and an ISK-per-hour figure derived from profit over
build time. A profitable flag turns on when profit is above zero.

### Saved plans

A Plans tab on the right side of the Planner holds the plans you save. Saving
stores the product, the run count, and the per-type settings: built-or-buy, ME,
TE, facility, and use-stock. The list orders the newest plan first. Each saved
plan reprices itself against current market prices every time you view it, so the
economics stay live rather than frozen at the moment you saved. Load a plan to
restore its tree and settings, or delete one you no longer need.

## Extractions

The Extractions tab shows your corporations' moon-mining extraction timers. It is
a corporation feature, so it reads from the corporation mining extractions ESI
endpoint and needs the Station Manager role and the corporation mining
extractions scope. Enabling the feature prompts a one-time re-authorization to
add that scope.

![Extractions tab](/docs/img/industry/extractions.png)

Each extraction shows as a timer card, sorted so the soonest chunk arrives first.
A card names the moon, its structure, and its system with a security pill. A
state badge tracks where the extraction is: Extracting while the chunk is still
forming, Arriving soon when arrival is under a day away, Ready to fracture once
the chunk has arrived, and Auto-fractured once the natural decay time has passed.

The card shows a Chunk arrives countdown to the arrival time and a Natural
fracture countdown to the decay time, with a progress bar from the start of the
extraction to the chunk's arrival. Once a timer is past, its countdown reads as
done or passed rather than a duration. A label shows the day the extraction
started.
