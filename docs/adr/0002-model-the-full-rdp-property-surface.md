# ADR 0002: Model The Full RDP Property Surface

## Status
Accepted

## Context
The product is intended to be "right from the go" rather than a minimal wrapper over a few MSTSC flags. Microsoft documents a broad `.rdp` property catalog covering connections, session behavior, display settings, redirection, and more. Limiting the internal model to a small hand-picked subset would create a future migration problem and would weaken the product's core.

The alternatives were:
- support only a curated subset of properties
- support a hybrid model with a small typed core and a catch-all override bag

## Decision
Model the full Microsoft-documented `.rdp` property surface internally through a property registry with metadata, validation rules, and UI hints.

The UI may still present the properties progressively through basic and advanced views, but the engine and persistence layer should not depend on a shallow schema.

## Consequences
- More upfront design work in the registry and editor model.
- Better long-term stability for import, export, validation, CLI schema inspection, and power-user workflows.
- The UI can be generated from the registry instead of duplicating field definitions.
- The app can include deprecation and compatibility notes in one place.

## References
- <https://learn.microsoft.com/en-us/azure/virtual-desktop/rdp-properties>
