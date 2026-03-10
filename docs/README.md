# Docs

This repository started as a docs-first planning repo and now contains the first implementation slice.

## Structure

- `architecture.md`: current architecture direction and major subsystem decisions.
- `credential-helper-contract.md`: the launch-time stdin/stdout JSON contract for external credential helpers.
- `release.md`: the Release Please flow, versioning model, and release operator expectations.
- `vertical-slice.md`: the first implementation target to build before expanding scope.
- `adr/`: architecture decision records.
- `research/`: source-backed notes from product and technical research.
- `mockups/`: HTML mockups and prompt packs for UI exploration.

## Current Direction

- Windows-only desktop application with a companion CLI.
- `mstsc.exe` remains the execution engine.
- Configuration is modeled around Microsoft-documented `.rdp` properties.
- Secrets are not stored by the app.
- The preferred home view is the calm list-plus-inspector pattern in `docs/mockups/claude-round-1/home-with-inspector-calm.html`.
- The current workspace contains `core`, `windows`, `cli`, and `desktop` Rust crates aligned to the architecture.

## Current Implementation Status

- The `Launch And Inspect` slice is implemented in code with a focused `.rdp` property subset, SQLite-backed profiles/presets/session history, helper contract support, CLI commands, and a Windows-gated desktop shell.
- Windows-specific integrations are kept behind platform gates so core logic, tests, and linting can still run in Linux-hosted agent environments.
- Desktop create/edit intentionally stays on MSTSC-native credential prompting for now; helper-specific launch policy still exists in shared core logic and the CLI, but is not a primary desktop UX surface in slice one.
- Desktop helper launches, when exercised, use `RDP_LAUNCH_HELPER` and optional `RDP_LAUNCH_HELPER_ARGS` from the process environment rather than persisted app settings.
- WSL-to-Windows smoke checks for `cmdkey.exe` and scripted `mstsc.exe` launch are supported through `scripts/smoke-mstsc-launch.ps1` and the `task smoke:mstsc` task.
- WSL-based smoke observation is supported through `scripts/observe-state.sh` plus the `task smoke:observe` and `task smoke:observe-once` tasks, which combine recent log lines with recent `session_history` rows.
- Windows desktop build and launch are exposed through `task desktop:build`, `task desktop:launch`, and `task desktop:run` with release variants for repeatable Windows-on-ARM workflows from WSL.
- Temporary WinCred bridge targets are tracked without secret material and swept on CLI/desktop startup to reduce stale credential leakage after crashes.
- The current Windows-native desktop build script targets `aarch64-pc-windows-msvc` for Windows on ARM machines.
- CLI and desktop write structured operational logs to `%LocalAppData%\\RdpLaunch\\logs\\app.log` so helper/launch/UI behavior can be tailed during smoke tests without storing logs in SQLite.
- Release automation is repo-level: Release Please bumps `[workspace.package].version`, opens a release PR, and publishes binaries plus an SBOM after that PR is merged and the follow-up `main` CI run succeeds.
- Follow-on work should focus on deeper preset support, fuller profile editing, richer desktop polish, and real Windows-hosted smoke testing of the Dioxus shell and MSTSC integration.
