---
id: "0046"
title: Standing Orders Polymorphic Day Links
status: active
tags: [captains-log, standing-orders, storage, data-model]
created: 2026-07-09
---

# ADR-0046: Standing Orders Polymorphic Day Links

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Standing Orders lets a pilot pin durable objectives and thread day-by-day Captain's Log items onto them. A
link can point at any of five different sources: a log answer, a field note, a killmail, an industry delivery,
or a skill completion. Rather than five per-source link tables, we store every link in one polymorphic
`objective_links` table keyed by a `source_kind` discriminator plus a `source_ref` string that encodes the
item's stable identity within its day. A link references that identity directly, never a `captains_log` or
`captains_log_answer` row, so editing the prompt configuration never orphans or moves a link.

## Context

An objective's "thread" is a running record of the log items a pilot has tied to it over time. The items come
from unrelated tables with unrelated shapes: `captains_log_answer` is keyed by `(date, question_id)`,
`field_notes` by a surrogate `id`, and the combat, industry, and skill rollups are derived per day per
character from `character_killmails`, `character_industry_jobs`, and `skill_completion`. There is no single
row a link can foreign-key to, and the set of source kinds will grow as the log gains sources.

Two shapes were available:

- One link table per source kind, each with a typed foreign key.
- One polymorphic table following the `entity_tags` pattern (ADR-0004): a `source_kind` string plus an opaque
  `source_ref`, with a `date` column added because links are day-scoped.

Per-source tables give referential integrity for free but multiply the schema, the repository surface, and the
thread query by the number of sources, and each new source kind is another migration and another table to
join. The rollup sources are also not first-class rows a foreign key could target: a "killmail on this day for
this character" is an identity, not a table row we own the lifecycle of.

A separate hazard drove the identity question. Log answers live in `captains_log_answer` keyed by a
`question_id` that the user can rename, reorder, or delete through prompt configuration. A link that pointed at
a `captains_log_answer` row (or worse, a row offset) would silently move or vanish when the prompt config
changed. The link has to survive prompt edits.

## Decision

Store all links in one table:

```sql
CREATE TABLE objective_links (
  objective_id INTEGER NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
  date         TEXT    NOT NULL,
  source_kind  TEXT    NOT NULL,
  source_ref   TEXT    NOT NULL,
  PRIMARY KEY (objective_id, date, source_kind, source_ref)
);
```

`source_kind` is one of `log_answer`, `field_note`, `killmail`, `industry`, or `skill`. `source_ref` encodes
the item's identity within its day as text, per kind:

| source_kind  | source_ref encoding             | resolved against          |
|--------------|---------------------------------|---------------------------|
| `log_answer` | `question_id`                   | `captains_log_answer`     |
| `field_note` | `field_notes.id`                | `field_notes`             |
| `killmail`   | `character_id:killmail_id`      | `character_killmails`     |
| `industry`   | `character_id:product_type_id`  | `character_industry_jobs` |
| `skill`      | `character_id:skill_id`         | `skill_completion`        |

Multi-part references join their parts with a colon. The primary key is the full
`(objective_id, date, source_kind, source_ref)` tuple, so setting a link is idempotent and clearing one is an
exact-match delete. The table is indexed on `date` for the day-scoped lookup ("what does this day link to");
the objective-scoped lookup ("all links for this objective") is served by the primary key's leading column.

The identity choice is deliberate. A log-answer link stores the `question_id`, not the answer text or a row
id, so renaming a question's label, reordering questions, or clearing and re-entering an answer leaves the
link intact and still pointing at the same question. The thread query resolves each link to its underlying
text or identity at read time by joining `source_ref` back to its source table, so a link whose underlying
item is currently absent simply reads as a null-text entry rather than a dangling row.

## Affected Areas

- `migrations/0129_create_objectives.sql` — the three Standing Orders tables.
- `src/store/model/objective.rs` — the `LinkSource` enum that owns the `source_kind` and `source_ref`
  encoding, plus the objective, status, pilot, link, and thread-entry types.
- `src/store/repo/objective.rs` — link set/clear, the day and objective link reads, and the thread join that
  resolves links against `captains_log_answer`, `field_notes`, and the rollup sources.

## Consequences

### Positive

- One table, one repository surface, and one thread query serve every source kind; a new source kind is a new
  `source_kind` value and a new `LinkSource` variant, not a new table and migration.
- Links survive prompt-config edits because they key on stable item identity, never on a `captains_log` or
  `captains_log_answer` row.
- The `entity_tags` precedent (ADR-0004) means the polymorphic shape is already familiar in this codebase.

### Negative

- No database-level foreign key from a link to its underlying item, since the rollup sources are derived
  identities rather than owned rows. The thread query tolerates a missing item by returning a null-text entry,
  and the objective foreign key still cascades, so a deleted objective drops its links.
- `source_ref` is an opaque string that callers must encode and decode consistently. The `LinkSource` enum
  centralizes that encoding so the format lives in exactly one place.

## References

- ADR-0004: Polymorphic Entity Tags — the `entity_type` / `entity_id` precedent this table follows.
- `migrations/0011_create_tags.sql` — the `entity_tags` shape.
- `migrations/0119_captains_log.sql`, `migrations/0125_captains_log_prompt_config.sql` — the account-scoped log
  and its `captains_log_answer` store.
- `src/store/repo/captains_log_rollup.rs` — the combat, industry, and skill identity sources.
