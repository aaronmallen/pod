# Comment Wash Bar

This is the frozen bar for the src/ comment wash. Every per-file wash agent receives this file verbatim in its prompt.
It does not replace the policy; it operationalizes it. Source of truth: docs/dev/code-style.md:121-177. If the two ever
disagree, code-style.md wins and this file is wrong.

The default is no comment. Most code carries none: good names and types document themselves. This is a `publish = false`
binary, not a library, so there is no coverage rule. A comment earns its place only when a competent Rust reader would
be confused or surprised without it, and the fact is irrecoverable from the name, signature, types, or body.

## Method

Audit then edit in place. Deletions only.

- For each existing comment block, classify it KEEP or CUT against the rules below.
- Delete CUT blocks with exact-match edits. Change nothing else.
- Leave every KEEP block byte for byte. Do not re-flow, reword, re-indent, or re-add it. Kept blocks stay identical.
- Never wipe a file and rebuild its comments. This is a removal pass, not a rewrite.
- The only additions allowed are the enumerated missing mandatory annotations: a one-line consumer note over a bare
  `#[expect(dead_code)]` that lacks one. Nothing else is added.
- Produce a KEEP/CUT ledger for central review: for each block, its location, the verdict, and a one-line reason.

Before editing a file, grep it for the mandatory-preserve markers (see the allow-list) and treat those line numbers as
do-not-touch.

## Keep rules

Keep a comment only when both hold: a competent Rust reader would be confused or surprised without it, and the fact is
irrecoverable from the name, signature, types, or body. Concretely, keep:

- Cross-system or external-format contracts the code cannot show (an EVE pricing rule, an ingest Worker regex, an ESI
  header semantic).
- Magic-number, encoding, or units gotchas (SQLite result codes; a negative cache value measured in KiB; a float
  roundoff epsilon; a numeric boundary between two id spaces).
- Load-bearing invariants and sentinels, including deliberate absences (`0 = 1` match-nothing; a reserved
  `__unassigned__` bucket; a clamp that is intentionally missing).
- Ordering or precedence subtleties where reordering silently breaks correctness (a scrub-before-write order; a
  comparator that must match an `ORDER BY`; close-before-swap on Windows file locking).
- Measured incident or regression history that justifies an otherwise arbitrary choice (a connection-pool size set
  against an observed storm; a tick cadence chosen to avoid starvation).
- Security or privacy carve-outs invisible in the body (scrub before disk; allow-list rather than deny-list).
- Framework-limitation rationale a reader would otherwise "fix" and break (an iced bounded-width requirement; iced
  uniform borders; iced local mouse coordinates).
- A duplicated constant that mirrors a private or un-importable module, or a design-source hex behind opaque RGB floats.

## Cut rules

Cut is the default. When in doubt, cut. A comment that is eloquent and accurate but recoverable from the code is still
cruft. Cut:

- Anything that restates the item name or signature; field or const docs that echo the name or the literal value.
- What-it-does narration (router, CRUD, or open-window play-by-play; view-layout description).
- All comments on test code (`//` and `///` in test modules, functions, and helpers). This is the single largest cut
  class. (`// SAFETY:` notes stay; see the allow-list.)
- Divider and section-header comments, unconditionally.
- Design-justification or refactor-rationale filler ("split out to keep cyclomatic complexity in check").
- Roadmap, phase, or task-tracking status comments that are not adjacent to an `#[expect(dead_code)]` or `#[allow]`
  attribute (a "consumed by B2+" stamp repeated over live functions).
- Obvious enum-variant docs; builder or setter docs, even with a cross-reference.
- `//!` module essays that merely narrate the module's items or carry a task taxonomy.
- An inline `//` that duplicates a `///` already kept on the same item.

## Mandatory-preserve allow-list

Never cut these, even when they read like status or narration. Check the structure, not the wording.

- Any comment adjacent to `#[expect(dead_code)]`, `#[cfg_attr(not(test), expect(dead_code))]`, or `#[allow(...)]`.
  Policy requires naming the awaited consumer. Critical: an identical-looking status line with no adjacent attribute is
  cruft. Check the next line for the attribute, not the text of the comment.
- `// SAFETY:` blocks, including inside test code where other test comments are cut.
- Real executable doctests inside `///` fences (a `rust` block that runs).
- Load-bearing `//!` whose role is non-obvious and irrecoverable (a dying-process rationale; a cross-codebase wire
  contract; a never-collected privacy boundary). Still cut `//!` that merely narrate the module's items.
- Golden-fixture contract comments shared with the telemetry/crash TypeScript Worker. Losing one silently breaks Worker
  conformance.

## Keep exemplars

Real comments from src/ that clear the bar. Keep them byte for byte.

Exemplar 1, src/store/asset_filter.rs:

```rust
// Unknown facet key compiles to a predicate that matches no rows.
_ => self.sql.push_str("0 = 1"),
```

Without it, `0 = 1` reads as a bug. The comment names it as a deliberate match-nothing sentinel. Irrecoverable from the
expression.

Exemplar 2, src/store.rs:

```rust
/// Warm the reader pool with a couple of live connections up front so the first interactive read
/// (the cold-open roster load) does not pay connection-establishment + WAL pragma setup latency, and
/// so idle connections are not reaped and re-created in bursts (the source of the observed
/// ~94-connection WAL-pragma storm). `min_connections` keeps these alive for the process lifetime.
```

Measured incident history that justifies an otherwise arbitrary `min_connections` value. The constant alone cannot
carry it.

Exemplar 3, src/store/repo/assets.rs:

```rust
// Per-type sum of each output material's mineral value, priced at the same global ESI prices the
// inventory already uses (unpriced materials COALESCE to 0, undervaluing rather than false-positiving).
```

Encodes a pricing contract and a deliberate undervalue-rather-than-overvalue tradeoff that the SQL does not reveal.

Exemplar 4, src/store/repo/character.rs:

```rust
// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
```

Mandatory preserve: it names the awaited consumer the attribute requires.

## Cut exemplars

Real comments from src/ (or the policy) that fail the bar. Delete them.

Exemplar 1, src/services/telemetry.rs:

```rust
// ---- Capture: cheap, non-blocking, no Settings branch. ----------------------
```

A section-divider header. Cut unconditionally.

Exemplar 2, src/store/repo/budget.rs:

```rust
// Budget storage foundation (B1); consumed by the Budget sync/UI in B2+. Some items are exercised only by
// unit tests until then.
pub async fn create_category(db: &Database, category: &NewCategory) -> Result<BudgetCategory, Error> {
```

A roadmap stamp on a live function with no adjacent attribute. Cut. Contrast the near-identical block in the same file
that does sit above `#[cfg_attr(not(test), expect(dead_code))]`: that one is kept (see KEEP exemplar 4).

Exemplar 3, the canonical bad example in code-style.md:

```rust
/// Gets the data directory.
pub fn data_dir() -> PathBuf { ... }
```

Restates the name and adds nothing. Cut.

Exemplar 4, src/store/repo/assets.rs, inside a test body:

```rust
// per_unit = 1000*5 + 500*10 = 10_000; floor(300/100) = 3; yield 0.5 => 10_000 * 0.5 * 3.
```

A play-by-play arithmetic comment in test code. All test-code comments are cut; only `// SAFETY:` survives.
