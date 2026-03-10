# ADR 0004: Desktop-First Product With A Shared CLI

## Status

Accepted

## Context

The product's main value is a better launch and management experience on Windows, which strongly suggests a desktop UI. At the same time, automation, diagnostics, and repeatable operations benefit from a CLI.

The alternatives were:

- desktop only
- CLI first
- equal-weight desktop and CLI with duplicated logic

## Decision

Make the desktop application the primary product and ship a companion CLI that sits on the same service layer and data model.

## Consequences

- The user experience can be optimized for day-to-day interactive use.
- Automation and scripting still have a first-class path.
- The core service boundary must stay clean and UI-agnostic.
- The CLI remains intentionally narrow and should not grow a separate domain model.
