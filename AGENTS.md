# AGENTS.md

## Purpose

This repository is for planning and building a Windows desktop launcher for `mstsc.exe`. The product is not a replacement RDP client; it is a higher-level launcher, profile manager, and local session monitor for the Windows inbox Remote Desktop Connection client.

## Working Rules

- Start with docs before app code. Capture architecture in `docs/architecture.md`, decisions in `docs/adr/`, research notes in `docs/research/`, and UI planning artifacts in `docs/mockups/`.
- When research changes how coding agents should work in this repo, update this file in the same change.
- Do not read `.env`, `.envrc`, or similar secret-bearing files.
- Do not print or echo secrets. This matters even in debugging notes.
- Ask before adding new dependencies. Candidate dependencies can be discussed in docs before approval, but they should not be added to the codebase without an explicit user decision.
- Prefer stable, Windows-supported behavior over clever workarounds. `mstsc.exe`, `.rdp` files, Win32 APIs, and Windows Credential Manager are the expected integration points unless a documented alternative is clearly superior.

## Planning Conventions

- Write research notes as concise, source-backed markdown files under `docs/research/`.
- Favor ADRs for decisions that materially affect interfaces, data modeling, security posture, or repo structure.
- Keep mockups in `docs/mockups/` as self-contained HTML or markdown so they can be reviewed without any app runtime.
- When a UI idea depends on an assumption, state the assumption in the mockup or adjacent markdown rather than hiding it.
- Prefer promoting one clear baseline mockup rather than leaving several competing "preferred" directions active in the repo.

## Product Assumptions

- Windows-only.
- Desktop app is the primary UX.
- CLI is a companion surface that shares the same core services and data model.
- The internal configuration model should cover the full Microsoft-documented `.rdp` property surface for Remote PC connections.
- The app should not own a secret store. External helpers may provide credentials at launch time, and Windows Credential Manager may be used only as an interoperability bridge when required for MSTSC behavior.
- The current preferred home-view baseline is `docs/mockups/claude-round-1/home-with-inspector-calm.html`.

## Agent Workflow

- Ground decisions in primary Microsoft documentation whenever behavior relates to `mstsc`, `.rdp` properties, WinCred, Credential Guard, or Windows RDP policy.
- Prefer repo artifacts over long chat summaries. If a conclusion should survive the conversation, write it into `docs/` or `AGENTS.md`.
- If a planning conversation produces a concrete decision, capture it in an ADR rather than leaving it implicit in architecture prose.
- If the Claude MCP server is present but its agent or transport path is broken, it is acceptable to use the local `claude -p "<prompt>"` CLI for mockup-generation help. Any output produced that way must still be reviewed and then promoted explicitly in repo docs before it becomes the baseline.
- Dioxus Desktop `0.7.x` injects a default native menu bar on Windows unless the app explicitly sets `Config::with_menu(None)`. Keep native window decorations for the safe path, but disable the default menu bar if it creates redundant chrome.
- The desktop compose flow currently treats credentials as MSTSC-native prompt behavior. The UI normalizes `display name + hostname + username/domain` into the single stored MSTSC `username` property and does not expose helper-specific credential controls in slice one.
- On Windows-on-ARM, native desktop builds should target `aarch64-pc-windows-msvc` while invoking `VsDevCmd.bat -arch=arm64 -host_arch=x64`; the x64-hosted cross tools are the practical working path.
- When building the Windows desktop binary from a WSL-hosted repo, disable Cargo incremental mode and use a Windows-local `CARGO_TARGET_DIR` to avoid UNC-path and session-lock issues.
- Operational logs belong in `%LocalAppData%\RdpLaunch\logs\app.log` as structured file output, not in SQLite tables. Log helper/launch/UI state transitions, but never log passwords, raw helper secret payloads, or secret-bearing `.rdp` contents.
- Rotate `%LocalAppData%\RdpLaunch\logs\app.log` once it reaches roughly 10 MiB, keeping one `app.log.1` rollover so validation and smoke work do not leave unbounded local logs behind.
- For Windows-only crates that are still developed from Linux-hosted agent environments, prefer `cfg(windows)` dependency gating and keep as much view-model and service logic as possible in platform-neutral Rust modules so tests and clippy can still run locally.
- The current desktop helper path reads `RDP_LAUNCH_HELPER` and optional whitespace-split `RDP_LAUNCH_HELPER_ARGS` from the process environment. Keep helper configuration at the app edge; do not move secret-bearing configuration into the SQLite store.
- Helper integrations must preflight the configured executable path and close stdin after request writes so EOF-driven helpers do not hang waiting for more input.
- In WSL environments with Windows interop, prefer visible repo scripts plus `powershell.exe -ExecutionPolicy Bypass -File ...` over opaque encoded commands when smoke-testing Windows-side behavior. This keeps the executed Windows command readable and reviewable.
- Release Please is configured as a single repo-level release, not per-crate Rust releases. Keep crate manifests on `version.workspace = true`, treat `[workspace.package].version` in the root `Cargo.toml` as the version source of truth, and do not point the Rust releaser at the workspace root manifest.
- Keep Release Please on its default operating model: it opens or updates the release PR, a human merges it, and the post-merge run creates the GitHub release plus attached artifacts. Do not reintroduce workflow-side self-approval or auto-merge without an explicit repo decision.
- For matrix-built GitHub release assets, upload uniquely named files directly. Do not rely on `gh release upload path#display-name` plus `--clobber` to create architecture-distinct assets; the CLI preserves the original asset name and later uploads can overwrite earlier ones.
