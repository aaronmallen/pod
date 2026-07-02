---
id: "0045"
title: pod_pack Portable File Codec
status: active
tags: [budget, codec, import-export, sharing]
created: 2026-07-01
---

# ADR-0045: pod_pack Portable File Codec

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod introduces a single shared codec, pod_pack, for its portable "share this thing" file formats. A pack is a
JSON envelope wrapped as magic + format tag + version + CRC over the JSON, then deflate-compressed and
base64-encoded. The codec is parameterized by a format tag and version so one module serves every pack type.
The first adopter is .pbr (Pod Budget Rules); .pfi (facility intel) and .psp (skill plans) adopt the same codec
under their own specs.

## Context

Several features need to hand a portable artifact to another pilot or install: budget automation rules,
facility intel, and skill plans. Each of these is a small, structured, model-derived payload that must survive
a round trip between installs and auto-heal missing local references on import.

A raw JSON file would work mechanically but is casually editable, gives no tamper signal, and reads as "just
edit the numbers." We want the shared properties across all three formats:

- Not casually editable / not plain-text readable - discourages hand-edits that would desync a shared rule set.
- Tamper-evident - a truncated, corrupted, or edited file is detected rather than silently half-imported.
- Self-describing - the file names its own format and version so the importer can reject a wrong-type or
  future-version file with a clear error instead of mis-parsing it.
- Fully reversible - export -> import reproduces the payload exactly.

Rather than invent a bespoke framing per feature, we define one codec and reuse it. This differs from the
whole-database export archive (ADR-0038), which is a zip of the entire dataset for backup/restore; pod_pack is
a small, single-feature, human-shareable payload.

## Decision

Add a pod_pack codec module (pure, unit-testable, no I/O) with two operations:

Encode (envelope -> bytes):

1. Serialize the typed envelope to JSON. The envelope always carries format (tag) and version fields plus the
   format-specific payload.
2. Compute a CRC32 checksum over the JSON bytes.
3. Frame as magic header + format tag + version + checksum + JSON.
4. Deflate-compress the frame (raw deflate).
5. Base64-encode the compressed bytes. The encoded string is the file body.

Decode (bytes -> envelope) reverses this and validates at every step:

1. Base64-decode; inflate.
2. Verify the magic header, the expected format tag, and the version is supported.
3. Verify the CRC over the JSON matches.
4. Parse the JSON envelope.

Any failure - bad base64, bad inflate, wrong/absent magic, wrong format tag, unsupported version, checksum
mismatch, or malformed JSON - returns a typed error with a clear message. Decode never panics and never returns
a partial/empty payload on a bad input. Plain text and bare JSON must fail the magic/tag/checksum gate and
never mis-decode as a pack.

The codec is generic over the format tag and version. Known tags:

| Tag                | File   | Adopter                  |
| ------------------ | ------ | ------------------------ |
| pod.budget-rules   | `.pbr` | This work (budget rules) |
| pod.facility-intel | `.pfi` | Spec rnqkmvyq (later)    |
| pod.skill-plan     | `.psp` | Task rktxommv (later)    |

Each feature owns its envelope schema and its enum key round-tripping (e.g. budget rules round-trip
RuleField/RuleOp/MatchMode via their existing stable as_str/from_key string keys); pod_pack only owns the
framing.

## Affected Areas

- New pod_pack codec module (encode/decode + typed errors).
- .pbr budget-rule serialize/parse built on top of it (this work).
- Future .pfi and .psp adopt the same codec under their own specs.
- Cargo.toml gains two direct dependencies.

## Dependencies

| Dependency | Version   | Purpose                                                        |
| ---------- | --------- | -------------------------------------------------------------- |
| flate2     | (current) | Raw deflate/inflate (today only zip's bundled deflate exists)  |
| crc32fast  | (current) | CRC32 checksum (today only transitive under zip)               |

base64 = "0.22" is already a direct dependency and is reused.

## Consequences

### Positive

- One codec, three (and future) file types - consistent framing, one place to fix bugs and evolve versioning.
- Files are opaque, tamper-evident, and self-describing; corrupt or wrong-type input fails loudly and touches
  no data.
- Pure and unit-testable independently of any feature UI or DB.

### Negative

- Adds two direct dependencies (both already present transitively).
- Obfuscation is not encryption - a determined user can still decode a pack; the goal is "not casually
  editable," not secrecy.
- A shared codec is a shared blast radius: a framing bug affects every format. Mitigated by version tagging and
  thorough round-trip/rejection tests.

## Future Work

- .pfi (spec rnqkmvyq) and .psp (task rktxommv) adopt this codec.
- Version-migration path when an envelope schema changes (out of scope now; the version field reserves the
  seam).

## References

- [ADR-0038]: data export/import archive format (whole-DB zip; contrast).
- [ADR-0044]: budget journal single source of truth.
- Design: tmp/design/budget-rules-data.jsx (serializeRulePack/parseRulePack).
- Spec oqlwqvvw (budget rule import/export + this codec).

[ADR-0038]: 0038-data-export-import-archive-format-and-restore-strategy.md
[ADR-0044]: 0044-budget-journal-single-source-of-truth.md
