# Code Style

This document describes the formatting and organization conventions used in Pod. Most formatting is handled
automatically by tools -- you generally just need to run `mise run format` before committing.

The code organization rules are the main things to keep in mind when writing new code. Project-specific rules will be
added to this document as the project matures.

## Running Formatters and Linters

```bash
mise run format       # Format all files
mise run lint         # Lint all files
```

## General Principles

These principles apply across all file types in the project:

| Principle            | Convention                                                    |
|----------------------|---------------------------------------------------------------|
| Maximum line width   | 120 characters                                                |
| Indentation          | 2 spaces (no tabs)                                            |
| Trailing whitespace  | None                                                          |
| Final newline        | All files end with a single newline                           |
| Import/include order | Alphabetical, grouped by origin (stdlib, external, local)     |
| Declaration ordering | Alphabetical within visibility groups (public before private) |

These conventions are enforced by `.editorconfig` and the project's linting tools.

## Rust Specifics

### Import Style

Prefer importing named types (structs, enums, traits) directly rather than using fully-qualified paths, unless there is
a name conflict. Functions and free-standing helpers may use the fully-qualified path.

```rust
// Good: import the trait and type, qualify only where there's a conflict (fmt::Result vs std::Result)
use std::fmt::{self, Display, Formatter};

impl Display for Foo {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    // ...
  }
}

// Bad: unnecessarily qualified types
impl fmt::Display for Foo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // ...
  }
}

// Fine: free functions can stay qualified
std::fs::create_dir_all(path)?;
```

### Code Organization

#### Module-Level Ordering

Order items within a module by:

1. **Constants**: All constant declarations first
2. **Type groups**: Each type definition immediately followed by its implementation blocks
3. **Free functions**: Any standalone helper functions after all type groups

Type groups are ordered **alphabetically** by type name, with **pub types before pub(crate) types before private types**
(each visibility group sorted alphabetically).

#### Derive Attributes

Traits listed in `#[derive(...)]` attributes should be ordered **alphabetically**.

```rust
// Good
#[derive(Clone, Debug, Eq, PartialEq)]
struct Foo;

// Bad
#[derive(Debug, Clone, PartialEq, Eq)]
struct Foo;
```

#### Enumeration Variants

Enumeration variants should be ordered **alphabetically**.

#### Struct/Record Fields

Struct or record fields should be ordered **alphabetically**, unless field order is semantically significant (e.g.,
positional arguments in a CLI framework).

#### Implementation Block Ordering

Order functions and methods within implementation blocks by:

1. **Static vs Instance**: Static/associated functions first, then instance methods
2. **Visibility**: Public items first, then private items
3. **Alphabetical**: Within each group, sort alphabetically

In test modules, fall back to purely alphabetical ordering when the static/instance/public/private structure doesn't
apply. See [testing] for test-specific conventions.

### Dead Code

**Never use `#[allow(dead_code)]`.** `dead_code` is set to `deny` in `[lints.rust]` (Cargo.toml), so dead code is a hard
error in every build. A bare `allow` would silence that permanently: it also hides newly-dead code, and a stale
annotation can never self-correct once the item goes live again. Use one of the options below instead.

For genuinely-dead code, pick by intent:

- **Foundation code ahead of its consumer** — use `#[expect(dead_code)]` (per item, never module-level) with a comment
  naming the awaited consumer. `expect` self-cleans: it warns the moment the item is used, forcing the annotation's
  removal.
- **Test-only items** — gate with `#[cfg(test)]`. Shared test helpers live in a `#[cfg(test)] pub mod test_support`
  block. For a fluent-builder method on a struct whose other methods are live, gate the method, not the whole `impl`.
- **No consumer and no near-term plan** — delete the item.

### Documentation Comments

**Default to no comment.** Most code — structs, functions, fields, enums — should carry *no* doc comment. Good names and
types document themselves; a comment that restates them is noise and will be removed. This is a `publish = false`
binary, not a library, so there is **no coverage rule** to satisfy. Comments are the rare exception, reserved for
genuinely non-obvious code.

Before writing any comment, apply this litmus test: *would a competent Rust reader be confused or surprised by this
without it?* If no, write nothing. Document **why** and the non-obvious — never **what** the name and signature already
say.

Add a comment only for things like:

- A security-relevant or surprising behavior (e.g. a JWT decoded *without* signature verification).
- An encoding/format gotcha (a thin-space thousands separator, unpadded base64url, a colon-delimited id).
- A load-bearing invariant or sentinel (`0 = 1` match-nothing vs `1 = 1` no-op; a reserved `__unassigned__` bucket).
- Ordering/precedence subtleties, units, failure modes, or FK/transaction ordering.
- A name that undersells what the code does (a `fresh_token` that actually refreshes and persists).

**Never document:** getters/setters, obvious constructors, plain derive-heavy structs, straightforward CRUD, obvious
enum variants, or **any test code** (no docs on test modules or test functions). When in doubt, leave it out.

#### `//!` vs `///` vs `//`

- `//!` — **module-level**, only when a module's purpose or place in the architecture is *not* obvious from its name
  and contents (a non-obvious protocol, algorithm, state machine, or a role the name doesn't reveal). Thin or obvious
  modules (a model struct, a simple re-export, an obvious CRUD repo, a style-constant file) get none.
- `///` — an **item** (struct, enum, trait, fn, method, const, field) whose behavior or contract clears the bar above.
  `///` attaches to items only.
- `//` — a confusing line *inside* a function body or a match arm. `///` cannot attach to statements, so explain those
  with a plain `//` comment (same high bar, used sparingly).

#### One-line summary style

When a comment is warranted, the first line is a single concise sentence. Additional detail follows after a blank line
only when non-obvious behavior warrants it.

```rust
// Good: explains a contract the signature can't convey
/// Returns the stored token, or refreshes and persists a new one when it is near expiry.
pub async fn fresh_token(...) -> Result<Option<Token>, Error> { ... }

// Bad: restates the name, adds nothing — delete it
/// Gets the data directory.
pub fn data_dir() -> PathBuf { ... }
```

#### Avoid name-restating redundancy

Don't write `/// The name of the user` above `pub name: String`. A comment must add information the name alone doesn't
convey — constraints, units, invariants, lifetimes, or context — or it should not exist.

#### No divider comments

Don't use Unicode box-drawing dividers or other section separators inside source files. If a file needs visual
structure beyond normal item grouping, consider splitting it.

[testing]: testing.md
