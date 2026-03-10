# ADR 0005: Use A Normalized SQLite Store With Property Value Tables

## Status
Accepted

## Context
The product needs a local store for profiles, presets, settings, and launch history. It also needs to model the full `.rdp` property surface without turning the schema into one column per property or a pile of ad hoc JSON files.

The store must serve both the desktop app and the CLI and should support future migrations without destabilizing the rest of the app.

## Decision
Use SQLite as the local store and keep the schema normalized around app concepts, while storing individual `.rdp` property assignments in dedicated property-value tables rather than per-property columns.

Logical tables:
- `profiles`
- `profile_properties`
- `presets`
- `preset_properties`
- `session_history`
- `app_settings`
- `profile_tags`

Property assignment rows should use:
- owning object id
- property key
- canonical value encoding
- updated timestamp

The canonical value encoding should be JSON text so the property registry can round-trip typed values without schema churn for every Microsoft property.

## Why This Shape
- SQLite is strong enough for long-lived local state and history.
- Separate profile and preset property tables preserve object boundaries and foreign-key clarity.
- JSON value encoding keeps the schema stable while the property registry evolves.
- The registry remains the source of truth for type validation and interpretation.

## Consequences
- The database stays compact and migration-friendly.
- Desktop and CLI can share the same persistence layer cleanly.
- Querying individual properties requires joins or targeted lookups rather than fixed columns.
- The app must validate and decode property values through the registry before use.
- Live active-session state should still be derived from process and window observation; it does not require a separate always-authoritative `active_sessions` table.

## Rejected Alternatives
- Loose JSON files per profile: too easy to drift and awkward for history.
- One giant table with columns for many `.rdp` properties: brittle and expensive to evolve.
- A single generic property table with polymorphic owner references: simpler at first glance, but weaker foreign-key guarantees and harder to reason about than separate profile and preset property tables.
