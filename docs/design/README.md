# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for the project.

The records are numbered by **domain group** rather than chronologically: foundational architecture
first, then the data model, authentication, static/reference data, and finally the domain-feature
ADRs (storage, assets, net worth, outbox, mail, logging, image assets).

## Index

| ID     | Title                                           | Status                  | Date       |
|--------|-------------------------------------------------|-------------------------|------------|
| [0001] | UI Architecture and Module Structure            | ![Active][badge-active] | 2026-06-06 |
| [0002] | Sync/Render Separation and Aggregation Chaining | ![Active][badge-active] | 2026-06-06 |
| [0003] | Canonical Data Model                            | ![Active][badge-active] | 2026-06-06 |
| [0004] | Polymorphic Entity Tags                         | ![Active][badge-active] | 2026-06-06 |
| [0005] | EVE SSO Authentication and Deeplink Transport   | ![Active][badge-active] | 2026-06-06 |
| [0006] | Static and Reference Data                       | ![Active][badge-active] | 2026-06-06 |
| [0007] | User-Configurable Storage Paths                 | ![Active][badge-active] | 2026-06-06 |
| [0008] | Assets Data Path                                | ![Active][badge-active] | 2026-06-06 |
| [0009] | Daily Net-Worth Snapshot                        | ![Active][badge-active] | 2026-06-06 |
| [0010] | ESI Write Path / Durable Outbox                 | ![Active][badge-active] | 2026-06-06 |
| [0011] | Eager Full-Body Mail Sync                       | ![Active][badge-active] | 2026-06-06 |
| [0012] | Logging and Observability Conventions           | ![Active][badge-active] | 2026-06-06 |
| [0013] | Committed Item Icons and Synced Portraits/Logos | ![Active][badge-active] | 2026-06-06 |
| [0014] | Persisted Sync Ledger and Honest Job Outcomes   | ![Active][badge-active] | 2026-06-06 |
| [0015] | ESI Request-Layer Policy                        | ![Active][badge-active] | 2026-06-08 |
| [0016] | Networked-Drive Storage-Sync Model              | ![Active][badge-active] | 2026-06-10 |
| [0017] | Interface Scale and Accessibility Config        | ![Active][badge-active] | 2026-06-11 |
| [0018] | Runtime High-Contrast Color Resolution          | ![Active][badge-active] | 2026-06-11 |
| [0019] | Central Feature Registry                        | ![Active][badge-active] | 2026-06-12 |
| [0020] | Calendar Feature                                | ![Active][badge-active] | 2026-06-12 |
| [0021] | Filterable Standings Data Model                 | ![Active][badge-active] | 2026-06-12 |
| [0022] | Linux Graphics Backend Diagnostics & GPU Stack  | ![Active][badge-active] | 2026-06-13 |
| [0023] | Industry Feature                                | ![Active][badge-active] | 2026-06-13 |
| [0024] | Forceful Lease-Takeover Safety Protocol         | ![Active][badge-active] | 2026-06-13 |
| [0025] | Virtualized Table Rendering                     | ![Active][badge-active] | 2026-06-13 |
| [0026] | Saved Build-Plan Node Identity                  | ![Active][badge-active] | 2026-06-14 |
| [0027] | Canonical Asset Valuation Chain                 | ![Active][badge-active] | 2026-06-19 |
| [0028] | Owner-Aware Budget Assignment Identity          | ![Active][badge-active] | 2026-06-20 |
| [0029] | Two-Level Feature Model w/ Tolerant Migration   | ![Active][badge-active] | 2026-06-21 |
| [0030] | Cross-Platform Keyboard Architecture            | ![Active][badge-active] | 2026-06-21 |
| [0032] | Skill Plans Store Full Set, Project Per-Char    | ![Active][badge-active] | 2026-06-22 |
| [0033] | Embedded MCP Server for Agent Automation        | ![Active][badge-active] | 2026-06-22 |
| [0034] | MCP Tool Input Schemas via Declarative Arg-Spec | ![Active][badge-active] | 2026-06-23 |
| [0035] | Post-Sync Cross-Owner Budget Mark Reconcile     | ![Active][badge-active] | 2026-06-23 |
| [0036] | Freshness-First Sync Status via Seed Events     | ![Active][badge-active] | 2026-06-23 |
| [0037] | Notification History Keyset Pagination          | ![Active][badge-active] | 2026-06-23 |
| [0038] | Data Export/Import Archive Format & Restore     | ![Active][badge-active] | 2026-06-23 |
| [0039] | Anonymous Opt-Out Telemetry                     | ![Active][badge-active] | 2026-06-25 |
| [0040] | Per-Wallet Journal Identity                     | ![Active][badge-active] | 2026-06-26 |
| [0041] | ESI Localized-Data Refresh on Language Change   | ![Active][badge-active] | 2026-06-27 |

ADRs document significant architectural decisions, the context in which they were made, and their consequences. See
[Writing ADRs] for the process and template.

[0001]: 0001-ui-architecture-and-module-structure.md
[0002]: 0002-sync-render-separation.md
[0003]: 0003-canonical-data-model.md
[0004]: 0004-polymorphic-entity-tags.md
[0005]: 0005-eve-sso-authentication-and-deeplink-transport.md
[0006]: 0006-static-and-reference-data.md
[0007]: 0007-user-configurable-storage-paths.md
[0008]: 0008-assets-data-path.md
[0009]: 0009-daily-net-worth-snapshot.md
[0010]: 0010-esi-write-path-outbox.md
[0011]: 0011-eager-full-body-mail-sync.md
[0012]: 0012-logging-and-observability.md
[0013]: 0013-committed-item-icon-set.md
[0014]: 0014-persisted-sync-ledger-and-honest-outcomes.md
[0015]: 0015-esi-request-layer-policy.md
[0016]: 0016-networked-drive-storage-sync.md
[0017]: 0017-interface-scale-and-accessibility-config.md
[0018]: 0018-runtime-high-contrast-color-resolution.md
[0019]: 0019-central-feature-registry.md
[0020]: 0020-calendar-feature.md
[0021]: 0021-filterable-standings.md
[0022]: 0022-linux-graphics-backend-diagnostics.md
[0023]: 0023-industry-feature.md
[0024]: 0024-forceful-lease-takeover.md
[0025]: 0025-virtualized-table-rendering.md
[0026]: 0026-saved-build-plan-node-identity.md
[0027]: 0027-canonical-asset-valuation.md
[0028]: 0028-owner-aware-budget-assignment-identity.md
[0029]: 0029-two-level-feature-model.md
[0030]: 0030-keyboard-architecture.md
[0032]: 0032-skill-plan-full-storage-projection.md
[0033]: 0033-embedded-mcp-server.md
[0034]: 0034-mcp-tool-input-schemas-via-declarative-arg-spec.md
[0035]: 0035-post-sync-cross-owner-budget-mark-reconciliation.md
[0036]: 0036-freshness-first-sync-status.md
[0037]: 0037-notification-history-keyset-pagination-and-time-based-retention.md
[0038]: 0038-data-export-import-archive-format-and-restore-strategy.md
[0039]: 0039-anonymous-opt-out-telemetry.md
[0040]: 0040-per-wallet-journal-identity.md
[0041]: 0041-esi-localized-data-refresh-on-language-change.md
[badge-active]: https://img.shields.io/badge/Active-green?style=for-the-badge
[Writing ADRs]: ../process/writing-adrs.md
