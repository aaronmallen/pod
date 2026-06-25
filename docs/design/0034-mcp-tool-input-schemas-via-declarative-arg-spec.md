---
id: 0034
title: MCP Tool Input Schemas via a Declarative Arg-Spec
status: active
tags: [mcp, automation, schema, interface]
created: 2026-06-23
---

# ADR-0034: MCP tool input schemas via a declarative arg-spec

## Status

Active

## Summary

The embedded MCP server (ADR-0033) declares each tool's input schema through a
hand-rolled declarative arg-spec abstraction rather than adopting schemars
derive-based schema generation. A single per-tool list of ArgSpec entries is
the one source of truth: it renders the tools/list JSON Schema and backs
handler argument extraction, with uniform lenient string→int coercion. This
fixes a latent bug — tools/list hardcoded an empty inputSchema for every tool,
so strict MCP clients stripped all arguments and every parameterized tool
failed — while keeping the project's hand-rolled-over-derive philosophy and
adding no new src dependency.

## Context

ADR-0033 fixed the tools/list slice of the protocol but omitted the tool input
schemas themselves. As implemented, tools_result returned a hardcoded empty
inputSchema ({ "type": "object", "properties": {} }) for every registered
tool. A schema with no declared properties tells a strict MCP client that the
tool takes no arguments, so the client strips the arguments it would otherwise
send. The result: zero-arg tools (ping) worked, but every parameterized tool
failed because its arguments never arrived. This was confirmed end to end by a
connected client agent — the agent could call argument-free tools but every
call carrying parameters came through empty.

The natural Rust answer is schemars with #[derive(JsonSchema, Deserialize)]
argument structs. But schemars is only a transitive dependency, pulled in by
rmcp's transport feature, and is unused anywhere in src/. ADR-0033
deliberately did not adopt the rmcp stack and hand-rolled the transport, the
auth, the JSON-RPC framing, and the bridge. Promoting schemars into
first-class src use to solve a small, well-bounded problem would reverse that
decision and contradict the project's established hand-rolled-over-derive
philosophy. The schemas Pod's tools need are narrow: a handful of
required/optional integer, string, and integer-array parameters. That is small
enough to declare directly.

There is also a correctness hazard worth designing against: the schema
advertised to the client and the extraction logic in the handler are two
descriptions of the same argument list. If they live in separate places they
drift — a renamed field, a required-vs-optional flip, or a type change updates
one and not the other, and the failure is silent until a client sends (or
omits) the wrong thing.

## Decision

### A declarative ArgSpec as the single source of truth

Each tool declares its arguments as a list of ArgSpec entries. An ArgSpec
names the argument, marks it required or optional, and gives its type from a
small closed set covering exactly what the tool catalog needs:

- integer
- string
- integer-array

This single declaration drives both directions:

- Schema rendering. tools/list walks a tool's arg-specs to build a real
  inputSchema: a properties object with the correct JSON Schema type per
  argument and a required array listing the required ones — no longer an empty
  placeholder.
- Handler extraction. The same arg-specs back argument extraction inside the
  handler, so a handler reads its arguments through the spec rather than
  re-describing them. The advertised schema and the extraction can no longer
  disagree, because they are generated from one declaration.

### Uniform lenient string→int coercion

Integer and integer-array extraction coerce numeric strings to integers
uniformly (e.g. an EVE id arriving as "123" is accepted as 123). MCP clients
vary in how they serialize numeric arguments, and tolerating numeric strings
makes the surface robust without a per-handler special case. The coercion
lives in the spec-backed extraction, so every tool gets identical, predictable
behavior.

### Consolidation of the require_* helpers

The previously duplicated per-handler require_* argument helpers are
consolidated into the spec-backed extraction. Argument access becomes uniform
across tools instead of each handler re-implementing its own required/optional
and coercion logic.

## Affected Areas

- src/mcp/ — the ArgSpec abstraction and the closed type set; tools/list
  schema rendering driven from each tool's arg-specs; spec-backed handler
  argument extraction with lenient string→int coercion; removal of the
  duplicated require_* helpers; each tool's per-tool arg-spec declaration.
- The MCP tool registry — registered tools now carry their arg-specs so the
  schema and extraction can be generated from one place.

## Consequences

### Positive

- Strict MCP clients now receive correct properties and required arrays, so
  parameterized tools work — the original failure is fixed.
- One source of truth per tool means the advertised schema and the handler
  extraction cannot drift.
- No new src dependency: schemars stays a transitive-only crate and the
  hand-rolled-over-derive philosophy (ADR-0033) holds.
- Uniform lenient string→int coercion tolerates numeric-string ids across
  heterogeneous clients without per-handler code.
- Consolidating the require_* helpers removes duplication and makes argument
  access uniform.

### Negative

- Per-tool argument declarations are maintained by hand, and every future tool
  must declare its arg-specs to be reachable with arguments.
- The closed type set (integer / string / integer-array) covers current needs;
  a tool requiring a richer schema (nested objects, enums, etc.) would need
  the abstraction extended.

## Future Work

- Extend the ArgSpec type set if a future tool needs schema shapes beyond the
  current closed set.
- Revisit schemars adoption only if Pod's tool schemas grow complex enough
  that hand declaration no longer pays for itself; the single-source-of-truth
  boundary keeps that swap localized.

## References

- Originating spec: pmzlnwwl.
- Foundation: ADR-0033 — embedded MCP server, the tools/list slice, and the
  hand-rolled-over-derive philosophy.
- MCP specification: <https://modelcontextprotocol.io>.
