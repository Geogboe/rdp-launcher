# Architecture

## Goal

Build a Windows desktop application that makes launching and managing `mstsc.exe` connections substantially better without replacing the underlying RDP client. The product should feel lighter than RDCMan, less Azure-centric than newer Microsoft remote desktop surfaces, and much more deliberate about configuration, visibility, and credential ergonomics.

## Product Shape

- Primary surface: desktop application.
- Secondary surface: CLI for scripting, diagnostics, and launch automation.
- Runtime target: Windows only.
- Execution engine: `mstsc.exe` launched with generated temporary `.rdp` files.

## Why MSTSC Is The Engine

Microsoft documents `mstsc.exe` as the inbox Remote Desktop Connection app and explicitly supports both `.rdp` file launch and `.rdp` file editing. The command line surface is intentionally small, but `.rdp` files expose the much larger configuration surface we need. This architecture keeps the app aligned with supported Windows behavior while letting the product focus on usability and control rather than protocol implementation.

Primary references:

- `mstsc` command documentation: <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/mstsc>
- Supported `.rdp` property catalog: <https://learn.microsoft.com/en-us/azure/virtual-desktop/rdp-properties>
- Microsoft guidance preferring MSTSC or the universal client over RDCMan: <https://learn.microsoft.com/troubleshoot/windows-client/remote/use-mstsc-universal-remote-desktop-client-instead-rdman>

## Architectural Principles

- Keep business logic below the UI. The desktop shell should be replaceable without rewriting launch planning, property handling, storage, or session tracking.
- Model the full supported `.rdp` property surface internally. The UI can be curated, but the engine should not be constrained by a shallow schema.
- Treat secrets as transient data. The app can request them from helpers and may bridge them into Windows facilities at launch time, but it should not become a secret store.
- Prefer explicit, typed models with provenance over lossy string blobs.
- Capture assumptions in docs and ADRs before code.

## Workspace Shape

- `core`: domain types, property registry, validation, launch planning, `.rdp` serialization, credential-helper contract, and session abstractions.
- `windows`: Win32 bindings, MSTSC launch orchestration, temporary file lifecycle, Windows Credential Manager bridge, process and window discovery.
- `desktop`: Dioxus shell and view-model layer over the core services.
- `cli`: thin command surface over the same core services.

Implementation note:

- The `desktop` and Win32-specific dependencies are gated behind `cfg(windows)` so non-Windows agent environments can still compile and test the shared logic.

## Core Domain Model

The domain model should separate user intent from launch-time materialization.

### Connection Profile

A saved remote endpoint definition with:

- identity: name, target host, tags, folder or collection membership.
- connection behavior: display, gateway, audio, device redirection, authentication, performance, and session behavior properties.
- security behavior: whether helper-based credentials are allowed, whether a Windows Credential Manager bridge is allowed, whether secure launch modes such as `/remoteGuard` or `/restrictedAdmin` are preferred.
- metadata: last used, notes, favorite, environment labels.

### Launch Preset

A named overlay for profile values so one target can support multiple launch modes such as:

- daily driver
- admin session
- low bandwidth
- multi-monitor

### Property Registry

A single internal registry of Microsoft-documented `.rdp` properties with metadata for:

- property key and wire syntax
- logical type
- supported values or ranges
- applies-to scope
- deprecation or compatibility notes
- UI grouping
- security or sensitivity flag
- CLI exposure and parse rules

The registry is the authoritative source for validation, generated forms, export, import later if added, and CLI schema inspection.

### Session Record

A local observation record for a launched MSTSC session:

- launch id
- profile and preset ids
- target
- process id
- window title or handle metadata when available
- start and end timestamps
- inferred state such as launching, active, disconnected, exited, failed

## Data Persistence

Use a local SQLite store under `%LocalAppData%\RdpLaunch\`.

Why SQLite instead of loose JSON files:

- strong footing for a long-lived profile library
- easier migrations and indexes
- better fit for history and session records
- less schema drift as the property model grows

Proposed logical tables:

- `connection_profiles`
- `launch_presets`
- `property_values`
- `session_history`
- `app_settings`

The app should store non-secret metadata only.

## Operational Logging

Operational logs should be written to disk under `%LocalAppData%\RdpLaunch\logs\app.log` as line-oriented structured records rather than stored in SQLite.

Why file logs instead of database rows:

- append-only operational telemetry should not compete with the domain store
- live tailing is useful during helper, launch, and session smoke tests
- retention and truncation are easier to manage outside the profile/session schema

The logging baseline should:

- record app startup, store open failures, helper execution outcomes, launch/runtime events, session refreshes, and UI actions that materially change state
- avoid secrets entirely, including passwords, raw helper payloads, and secret-bearing `.rdp` contents
- prefer stable identifiers such as `profile_id`, `preset_id`, `launch_id`, and `process_id`
- rotate `app.log` once it reaches roughly 10 MiB, keeping one previous `app.log.1` copy so long-running installs do not grow logs without bound

## Launch Flow

1. User selects a profile and optional preset.
2. Core resolves the effective property set and validates it against the property registry.
3. Desktop or CLI requests credentials from an external helper when the launch path requires it.
4. Windows layer optionally writes a temporary Windows credential entry if the launch method needs MSTSC-compatible saved credentials.
5. Core serializes the effective property set to a temporary `.rdp` file.
6. Windows layer launches `mstsc.exe` against the generated file and begins local process and window tracking.
7. On exit, temporary artifacts are cleaned up and the session record is finalized.

## Credential Strategy

The app should not store secrets. Instead, it should support an external helper contract that behaves more like `SSH_ASKPASS` than a vault SDK embedded in the app.

### Helper Contract Direction

- The app runs one configured helper executable.
- The app resolves the configured helper executable to a concrete filesystem path before spawn and rejects missing, non-file, or non-runnable paths up front.
- The app sends a JSON request on stdin.
- The app closes stdin after writing the request so helpers that read to EOF can complete.
- The helper returns JSON on stdout.
- The response can include username, domain, password, and helper-specific metadata.
- The app never persists the returned secret values.

### Why This Direction

- keeps the core vault-agnostic
- works with password managers, scripts, or custom adapters
- avoids binding the product to one vendor early
- preserves a clean security story

### Windows Credential Manager Bridge

Windows Credential Manager is acceptable only as an interoperability layer for MSTSC launch behavior. It is not the app's secret system. Profiles should opt into this per target because enterprise policy and personal preference may differ.

Relevant references:

- `cmdkey`: <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cmdkey>
- Windows credential storage overview: <https://learn.microsoft.com/windows-server/security/windows-authentication/credentials-processes-in-windows-authentication#credential-storage-and-validation>
- Remote Credential Guard: <https://learn.microsoft.com/windows/security/identity-protection/remote-credential-guard>

## UI Direction

The UI should feel like a deliberate launcher rather than a generic admin console.

The current planning bias is a utility-sized desktop window rather than a full-screen workspace. The home window should launch from a compact, monitor-aware size policy: roughly one third of the available display width, clamped into a calm utility range around 600-720px wide and 560-760px tall. The default launch footprint should favor the stacked vertical home view, while still allowing the layout to expand into a side-by-side split when the user widens the window. Secondary windows, drawers, or detail panes are acceptable, but the primary home view should stay comfortable in a compact footprint without becoming a stacked mini-dashboard.

The current preferred mockup baseline is [`docs/mockups/claude-round-1/home-with-inspector-calm.html`](mockups/claude-round-1/home-with-inspector-calm.html). That direction is preferred because it makes the home view do one job well: present a calm, scan-friendly list with active sessions pinned first, while moving detail and actions into a clearly subordinate inspector pane.

### Main Views

- Launchpad: one primary searchable list of targets with active sessions pinned at the top, quick launch affordances, favorites, and environment filters.
- Profile Editor: structured editing of full `.rdp` capability with a progressive disclosure model.
- Session Dashboard: active local MSTSC sessions, recent launches, last errors, and quick reconnect actions.
- Helper Diagnostics: inspect helper availability, profile-to-helper mapping, and credential launch policy behavior without exposing secrets.

### Progressive Disclosure

Although the internal model covers the full property surface, the UI should not throw every field at the user at once.

- Basic mode: common properties and safe defaults.
- Advanced mode: full categorized property editor generated from the registry.
- Power mode: direct property inspection with raw key visibility for expert users.

### Home View Interaction Model

The main window should have one dominant job: launch and inspect.

- show a unified vertical list rather than multiple equally weighted dashboard modules
- pin active sessions at the top of the list
- keep per-row metadata to the few signals that change a launch decision
- prefer text hierarchy, spacing, and selection state over pills, chips, and card-heavy row treatments
- open richer details in a secondary pane, side sheet, or dedicated inspector instead of crowding the home view
- let the inspector carry the detail burden so the main list remains lean
- move event history and helper diagnostics out of the main canvas unless explicitly expanded

## CLI Direction

The CLI should be first-class but intentionally narrow:

- `profiles list`
- `profiles show`
- `profiles create`
- `profiles set`
- `profiles unset`
- `launch`
- `sessions list`
- `schema list`
- `schema show`
- `helper probe`
- `doctor`

The CLI should not invent a separate model. It is a transport over the same services used by the desktop app.

## Session Awareness

The first version of session visibility should stay local and honest.

- track only locally launched MSTSC sessions
- infer state from process and window presence
- capture recent history and failures
- do not claim remote-side truth the app cannot actually prove

Out of scope for the first implementation:

- thumbnails
- remote-side process introspection
- broker-aware farm state
- deep remote session telemetry

## Open Questions To Keep Discussing

- How much metadata should a profile support beyond `.rdp` properties: notes, owner, change history, tags, health hints?
- Should the profile editor expose policy-aware warnings for risky combinations such as insecure authentication settings?
- How opinionated should the first library organization be: folders, tags, both, or something smarter?
- Should favorites and environment filtering return as a dropdown or menu once the home view behavior is implemented?
