# Code Review

This document describes what to look for when reviewing changes.

## Review Checklist

### Correctness

- Logic errors, off-by-one mistakes, unhandled edge cases
- Does the code do what it claims?
- Are error conditions handled?

### Safety

- Resource leaks (file handles, connections, memory)
- Injection risks (SQL, command, template)
- Improper input handling or missing validation at system boundaries

### Error Handling

- Errors are surfaced clearly, not silently swallowed
- Error messages are actionable and include context
- Failures don't leave the system in an inconsistent state

### Style

- Follows conventions from the [Code Style Guide]
- Naming is clear and consistent with the codebase
- Code organization matches project structure

### Documentation

- Public APIs and non-obvious logic are documented
- Comments explain *why*, not *what*

### Test Coverage

- New functionality has corresponding tests (see the [Testing Guide])
- Edge cases are tested
- Existing tests are not weakened or removed without justification

### Dependency Hygiene

- No unnecessary new dependencies introduced
- Dependencies are up to date and maintained
- Transitive dependency impact is considered

## Severity Levels

Findings should be categorized by severity:

- **Blocking** – must be fixed before merge (bugs, correctness issues, test failures, security vulnerabilities)
- **Warning** – should be fixed but not a showstopper (style violations, missing tests for edge cases)
- **Suggestion** – optional improvements (refactoring ideas, alternative approaches, documentation improvements)

[Code Style Guide]: ../dev/code-style.md
[Testing Guide]: ../dev/testing.md
