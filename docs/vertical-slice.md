# Vertical Slice

## Goal
Define the first end-to-end implementation target that proves the architecture without trying to finish the whole product in one pass.

This slice should be enough to launch real MSTSC sessions from both desktop and CLI using persisted profiles. Helper-based credentials remain part of the core launch engine and CLI, but the desktop create/edit surface should stay focused on MSTSC-native credential prompting for now.

## Slice Name
Launch And Inspect

## Must Work
- Start the desktop app into the calm home view based on `docs/mockups/claude-round-1/home-with-inspector-calm.html`.
- Persist profiles and presets in local SQLite.
- Create a profile manually from the desktop app.
- List profiles and launch a profile from the CLI.
- Resolve a focused subset of `.rdp` properties through the core property registry and generate a temporary `.rdp` file.
- Launch `mstsc.exe` from desktop and CLI.
- Optionally resolve credentials through the helper contract.
- Optionally bridge credentials into Windows Credential Manager for MSTSC compatibility.
- Track locally launched MSTSC sessions and show them as active at the top of the home view.
- Show selected-row detail in the inspector for both idle connections and active sessions.
- Persist launch history records for later display.

## Included In Slice 1

### Desktop
- Home view with:
  - search box
  - refresh action for local session rescan
  - active sessions section
  - saved connections section
  - selection-driven inspector
- `+ New` flow for creating a profile with a focused field set in a dedicated compose surface rather than the inspector pane.
- `Launch` action from the inspector.
- `Edit` action from the inspector for the same focused field set.
- Double-click launch from saved-connection rows.
- Right-click row menus for common actions.
- Delete action for saved connections.
- Active-session inspector with `Reveal window`, `Launch new`, and `Open profile`.

### CLI
- `profiles list`
- `profiles create`
- `profiles show`
- `presets list`
- `presets create`
- `launch`
- `sessions list`
- `helper probe`

### Core
- Property registry infrastructure with enough definitions for the slice.
- Effective launch-plan resolution from profile + optional preset.
- `.rdp` serialization.
- Launch planning that understands prompt-only vs helper-resolved vs Windows-bridge launches.
- Session state model and local observation adapters.

## Minimal Field Set For Slice 1
The engine remains full-property-oriented, but the first create/edit UX only needs this field set:
- display name
- hostname or address
- username and domain
- screen mode
- use multimon
- selected monitors
- redirect clipboard
- gateway hostname and gateway usage
- prefer Remote Guard
- prefer Restricted Admin

Desktop create/edit should normalize `username + domain` into the single MSTSC `username` property:
- `DOMAIN\alice` should populate domain `DOMAIN` and username `alice`
- `alice@corp.example` should populate domain `corp.example` and username `alice`
- `.\alice` should resolve to `<hostname>\alice`

Desktop create/edit should not expose helper-specific credential controls in this slice. Users should rely on MSTSC-native prompting and saved Windows credentials while the deeper helper/bridge policy remains available in the core engine and companion CLI.

## Not In Slice 1
- Full advanced property editor.
- `.rdp` import or export workflows.
- RDCMan import.
- Tags, folders, and richer library organization.
- Empty-state polish beyond a basic first-run screen.
- Bulk operations.
- Detailed diagnostics or audit screens.
- Remote-side telemetry, thumbnails, or disconnect control.

## Acceptance Criteria
- A user can create a profile in the desktop app and launch it successfully through MSTSC.
- A user can create and launch a profile through the CLI against the same store.
- A user can create a preset through the CLI and launch a profile with that preset applied.
- A selected active session appears at the top of the home view and exposes session-specific inspector details.
- If the helper returns `resolved`, the launch uses those credentials without the app persisting them.
- If the helper returns `prompt`, the launch continues in interactive MSTSC mode.
- If the helper returns `cancelled` or `denied`, the launch stops cleanly.
- If the helper fails, fallback behavior follows profile policy.
- Temporary `.rdp` files and temporary Windows credentials are cleaned up on normal session exit.
- Stale temporary Windows bridge credentials from an interrupted prior run are swept on the next startup.

## What This Slice Proves
- The Rust workspace boundaries are sound.
- The SQLite and property-registry model can drive both desktop and CLI.
- The helper contract is sufficient for real launches.
- The calm list-plus-inspector home view can carry the day-to-day workflow.

## Implementation Notes
- The desktop shell currently discovers helper configuration from `RDP_LAUNCH_HELPER` and optional `RDP_LAUNCH_HELPER_ARGS`.
- Windows-only runtime behavior is kept behind `cfg(windows)` boundaries so the shared logic can still be linted and tested from Linux-hosted agent environments.
- CLI and desktop currently write structured operational logs to `%LocalAppData%\RdpLaunch\logs\app.log` for smoke-test visibility; these logs intentionally exclude secret values.

## Follow-On Slice
After this slice, the next likely build target is:
- fuller property editing driven by the registry
- richer presets
- right-click actions and keyboard-heavy utility behavior
- empty-state and first-run polish
