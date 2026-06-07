---
id: "0004"
title: Polymorphic Entity Tags
status: active
tags: [architecture, database, tags]
created: 2026-06-06
---

# ADR-0004: Polymorphic Entity Tags

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Tag membership is generalized from the character-only `character_tags` join to a single polymorphic
`entity_tags` table keyed by `(tag_id, entity_type, entity_id)`. One join now serves every taggable
entity — characters today, corporations next (spec ylwxzoun), more later — instead of a parallel
join table per type. The entity kind is a free `TEXT` discriminator whose in-use values
(`'character'`, `'corporation'`) are defined once as `&str` constants rather than a Rust enum, and
the table deliberately carries **no foreign key on `entity_id`**: it cannot reference more than one
parent table, so per-entity cleanup on delete is done explicitly in the application instead of by an
`ON DELETE CASCADE`.

## Context

The original `character_tags` join keyed on `(character_id, tag_id)` with a foreign key
`character_id → characters(id) ON DELETE CASCADE`. Tags are app-owned organizational data (ADR-0003):
user-authored, never written by the sync engine. The shape worked while only characters were
taggable.

The corporations feature (spec ylwxzoun) needs tags on corporations too, and the plan anticipates
further taggable entities. The choices were:

- **A join table per entity type** (`character_tags`, `corporation_tags`, …). Each gets its own FK
  and cascade, but the repo and every membership query fork per type, and the tag-system surface area
  grows linearly with entity kinds.
- **One polymorphic join** keyed by an entity-type discriminator plus an entity id. A single table,
  a single repo, and a single set of type-scoped queries serve every entity.

The 0.5.0 prototype validated the polymorphic shape, so this ADR adopts it as the foundation the rest
of the tagging plan builds on. The cost of one shared table is that a single `entity_id` column
cannot carry a foreign key to several different parent tables, which forces a decision about how
membership rows are cleaned up when their owner is deleted.

## Decision

Replace `character_tags` with a polymorphic `entity_tags` table and scope every access by entity type.

### Schema

```sql
CREATE TABLE IF NOT EXISTS entity_tags (
  tag_id      INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  entity_type TEXT    NOT NULL,
  entity_id   INTEGER NOT NULL,
  PRIMARY KEY (tag_id, entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_entity_tags_entity ON entity_tags(entity_type, entity_id);
```

The composite primary key keeps membership non-exclusive (an entity may hold many tags) and
idempotent (re-assigning the same tag is a no-op). The index on `(entity_type, entity_id)` serves the
common lookup — "the tags for this entity". The `tag_id → tags(id) ON DELETE CASCADE` foreign key is
retained: deleting a tag still cascades its membership rows away, because that single relationship
always points at one table.

This is an unpublished project, so the change is made **in place** in the original migration
(`migrations/0013_create_tags.sql`); there is no `ALTER`, no backfill, and dev databases are reset
and reseeded.

### Dropped FK on `entity_id` and app-level cleanup

`entity_id` carries **no foreign key**, because it may name a row in any parent table and SQLite
cannot express a conditional FK. The consequence is that deleting an owning row (e.g. a character) no
longer cascades its membership rows away. Each delete path therefore removes its own membership rows
explicitly, scoped to its entity type, inside the same transaction as the parent delete:

```sql
DELETE FROM entity_tags WHERE entity_type = 'character' AND entity_id = ?;
```

`character::delete` does this for characters; the corporation delete path will do the equivalent when
it lands. The trade-off is deliberate: the database no longer guarantees referential integrity for
`entity_id`, so correctness depends on every delete path remembering to clean up, but in return one
table serves all entity types.

### `&str`-const discriminator

The entity kind is a plain `TEXT` discriminator. Its in-use values live as `&str` constants
(`ENTITY_TYPE_CHARACTER`, `ENTITY_TYPE_CORPORATION`) on the membership model rather than a Rust enum.
A `TEXT` column with a string constant keeps the SQL readable, lets a new taggable entity be added
with a one-line constant and no enum/exhaustiveness churn, and avoids an enum-to-string mapping layer
between the model and the column. The cost is that the type system does not constrain the column to a
known set — an unknown `entity_type` is simply a row that nothing queries.

### Type-scoped repo and queries

Every membership operation is scoped by `entity_type`:

- `tag::assign` / `tag::unassign` take an `entity_type`.
- `tag::members(tag_id, entity_type)` returns the ids of that one type.
- `tag::memberships(entity_type)` returns that type's rows.

The character roster (`character::search` and `character::attach_tags`) pins every tag subquery — the
free-text tag arm, the `tag:` key predicate, and the chip-attach query — to `entity_type =
'character'`, so character roster search and tagging behave exactly as before the generalization.

## Affected Areas

- `migrations/0013_create_tags.sql` — `character_tags` replaced in place by `entity_tags`.
- `src/store/model/entity_tag.rs` — renamed from `character_tag.rs`; `Model` carries
  `entity_type`/`entity_id`, and the discriminator `&str` constants live here.
- `src/store/model.rs` — module/re-export renamed `CharacterTag` → `EntityTag`; the constants are
  re-exported.
- `src/store/repo/tag.rs` — `assign`/`unassign`/`members`/`memberships` are entity-type-scoped.
- `src/store/repo/character.rs` — `attach_tags`, the free-text tag arm, and the `tag:` predicate are
  scoped to `'character'`; `delete` gains an explicit `entity_tags` cleanup.
- `src/features/character_manager.rs` — the roster loader and tag-write path pass
  `ENTITY_TYPE_CHARACTER`.

## Consequences

### Positive

- One join table, one repo, and one set of type-scoped queries serve every taggable entity; adding a
  new entity kind is a constant plus its delete-path cleanup, not a new table.
- Character roster search and tagging are behaviorally unchanged.
- The `tag_id` cascade is preserved, so deleting a tag still drops its memberships automatically.

### Negative

- `entity_id` has no foreign key, so the database no longer enforces referential integrity for it;
  every delete path must remember to remove its membership rows or they orphan.
- The `entity_type` discriminator is unconstrained at the type level; a typo'd or unknown value
  produces dead rows rather than a compile error.

## Future Work

- Corporation tagging (spec ylwxzoun) reuses this table via `ENTITY_TYPE_CORPORATION` and adds the
  matching delete-path cleanup.
- The shared roster-search SQL helpers (`escape_like`, `like_pattern`, the predicate pushers) remain
  in `repo/character.rs` for now; they can be lifted to a shared home when `corporation::search`
  needs them.

## References

- ADR-0003 — Canonical Data Model (`0003-canonical-data-model.md`)
