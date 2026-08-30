# ADR 0030: Safe local-first notes workspace

- **Status:** Accepted
- **Date:** 2026-08-30

## Decision

SQLite remains the source of truth for notes. Note bodies remain Markdown and note IDs remain stable. Internal links resolve by stable note ID or human-readable title; unresolved links remain visible without creating phantom notes. Filesystem and network imports are explicit user actions. The renderer treats note text as untrusted data and never injects arbitrary note HTML. Existing row timestamps, tombstones, and sync merge semantics remain authoritative; derived link and tag indexes are rebuilt from note content after writes or merges.

The implementation uses the existing Tauri command/API split, existing Svelte 5 workspace, and existing safe Markdown renderer. It does not create a parallel note store or renderer. Metadata is JSON data with bounded validated keys/values, and all database queries use bound parameters with bounded result sets.

## Consequences

Notes gain durable organization and relationship indexes while old rows continue to load through migration defaults. Unresolved links and malformed syntax are inert, which avoids phantom records and script execution. Sync can rebuild derived data deterministically, but concurrent authored-content conflicts must remain explicit rather than silently overwriting either revision. External embeds are intentionally excluded from the initial scope.
