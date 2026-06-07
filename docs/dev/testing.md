# Testing

This guide covers how to write and organize tests in Cardlogue.

The goal is to test meaningful behavior without over-testing trivial code. When in doubt, focus on logic that could
break -- transformations, edge cases, and complex calculations.

## Core Principle

**Tests are the source of truth.** Tests define the expected behavior of the system. If a test passes, the behavior it
describes is correct by definition.

**Never modify existing integration tests unless the issue explicitly calls for behavioral change.** If an integration
test is failing, the implementation needs to change -- not the test. The only exception is when the issue or spec
explicitly requires a change in observable behavior.

## Running Tests

```bash
mise run test                        # Run all tests
mise run test -- --test xyz          # Run tests matching "xyz"
```

## What to Test

**Test:**

- Functions and methods with logic (arithmetic, transformations, conditionals)
- Display/formatting implementations
- Custom comparison or ordering implementations
- Edge cases and boundary conditions
- Inverse operations (roundtrip tests)
- Error paths and failure modes

**Skip:**

- Simple constructors that just assign fields
- Trivial getters that return field values
- Thin wrappers that only delegate to another function

Before writing a test, ask: "Does this test verify actual logic, or just that field assignment works?"

## General Conventions

**Naming:** Test functions use the pattern `it_<does_something>`. Group names match the function or method being tested.

**Ordering:** Test groups follow [code style][code-style] ordering -- static/associated functions first
(alphabetically), then instance methods (alphabetically).

**Test body structure:** Separate setup from assertions with a blank line. For tests with multiple assertion groups,
separate each group with a blank line.

**Integration tests:** Integration tests validate end-to-end behavior and are the strongest contract in the codebase.
They must not be modified to make a failing implementation pass. If an integration test fails after a code change, the
code change is wrong unless the issue explicitly calls for a behavioral change.

## Rust Specifics

### Assertions

Use [`pretty_assertions`][pretty-assertions] for `assert_eq!` and `assert_ne!`. Import the macros at the innermost
test module so they replace the standard versions for all tests in that group.

```rust
mod tests {
  mod some_function {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_returns_the_expected_value() {
      let result = some_function(42);

      assert_eq!(result, 84);
    }
  }
}
```

### Test Module Structure

Tests use nested modules to mirror the object and function hierarchy being tested. The outermost `tests` module is
annotated with `#[cfg(test)]` and imports the parent module. Each object under test gets its own submodule, and each
function or method gets a submodule within that.

```rust
#[cfg(test)]
mod tests {
  use super::*;

  mod the_object_being_tested {
    use super::*;

    mod the_fn_being_tested {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_does_a_thing() {
        // setup
        let input = 42;

        // assertions
        assert_eq!(input * 2, 84);
      }
    }
  }
}
```

This structure keeps tests organized and produces readable output when a test fails (e.g.,
`tests::my_struct::new::it_assigns_fields`).

[code-style]: code-style.md
[pretty-assertions]: https://crates.io/crates/pretty_assertions
