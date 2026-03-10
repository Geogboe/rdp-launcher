# rdp-launch Security Review & Rust Best Practices Analysis

**Date:** 2026-03-09
**Scope:** Full codebase review — all 4 workspace crates, all source files, dependencies, and architecture
**Reviewer:** Claude Opus 4.6

---

## Executive Summary

rdp-launch is a well-architected tool with a **strong security posture by design**. The decision to never store secrets (ADR-0003) is the single most important security choice and it's implemented correctly throughout. The Rust code is idiomatic and clean for an early-stage project, with a few areas that could be tightened.

**Overall Security Rating: Good** — no critical vulnerabilities found
**Overall Rust Quality Rating: Good** — clean, idiomatic code with minor improvement opportunities

---

## Part 1: Security Review

### 1.1 Credential Handling — STRONG

The #1 security property of this tool is that **it never becomes a secret store**. This is well-implemented:

- Passwords are never persisted to SQLite, config files, or logs
- Helper credentials exist only as in-memory function parameters during the launch flow
- Logging explicitly uses `has_password: bool` flags instead of values (`helper.rs:163-165`)
- The `CredentialFlow::HelperResolved` struct stores `password_present: bool`, not the password itself (`launch.rs:38`)
- RDP files written to disk never contain embedded credentials (`rdp.rs` serializes properties only)

**No issues found.**

### 1.2 External Process Execution — MEDIUM RISK

Three external processes are spawned. All use `Command` with argument arrays (not shell strings), which eliminates shell injection. However:

#### 1.2.1 Helper Executable (`helper.rs:203-208`)

```rust
Command::new(&self.config.executable)
    .args(&self.config.args)
```

**Finding SEC-1: No validation of helper executable path.**

The `--helper` CLI argument accepts an arbitrary path that is executed as a child process. While this is by design (the user chooses their helper), there are no guardrails:

- No check that the path exists before spawning
- No check that the file is executable
- No PATH resolution logging (so `--helper foo` silently resolves via PATH)
- The executable path comes from CLI args, which is acceptable for a local tool, but if profile-stored helper configs are added later, this becomes a stored-command-execution risk

**Recommendation:** Add a pre-flight check that the executable exists and is a file (not a directory). Log the resolved absolute path. This is defense-in-depth, not a vulnerability today.

#### 1.2.2 cmdkey Arguments (`credentials.rs:25-29`)

```rust
.arg(format!("/generic:TERMSRV/{}", credential.target))
.arg(format!("/user:{}", credential.username))
.arg(format!("/pass:{}", credential.password))
```

**Finding SEC-2: Unvalidated interpolation into cmdkey arguments.**

The `target` and `username` values flow from the helper response or profile data, through the launch plan, into cmdkey arguments. Because `Command::arg()` passes each argument directly (not through a shell), this is **not** a command injection vulnerability. However:

- A `target` containing spaces or special characters could cause cmdkey to misparse the `/generic:` argument
- A `username` with embedded colons or slashes might confuse cmdkey's argument parsing
- Windows `cmdkey.exe` has its own argument parsing quirks

**Recommendation:** Add basic validation that `target` matches a hostname pattern (alphanumeric, dots, hyphens) and `username` doesn't contain control characters. This prevents confusing cmdkey, not injection.

#### 1.2.3 Non-Windows Stub (`launcher.rs:233-240`)

```rust
#[cfg(not(target_os = "windows"))]
{
    let mut command = Command::new("sh");
    command.arg("-c")
        .arg(format!("sleep 1 # {}", rdp_path.display()));
    command
}
```

**Finding SEC-3: Path value interpolated into shell command string in non-Windows stub.**

This is a development/test stub, not production code, but the `rdp_path.display()` output is embedded in a string passed to `sh -c`. If the path contained shell metacharacters (e.g., `; rm -rf /`), it would be interpreted by the shell. Since this is `#[cfg(not(target_os = "windows"))]` and the path is generated internally (`{launch_id}.rdp` where launch_id is a UUID), exploitation is not realistic, but it's a bad pattern.

**Recommendation:** Change the stub to `Command::new("sleep").arg("1")` — don't pass the path through a shell at all.

### 1.3 File System Security — ACCEPTABLE

#### 1.3.1 Temporary RDP Files

- Written to `{app_root}/temp/{launch_id}.rdp` where `launch_id` is a UUID v7
- Cleaned up explicitly via `launcher.rs:135-136` in `cleanup()`
- **Finding SEC-4:** If the process crashes between writing the RDP file and cleanup, the file persists until the next CLI invocation calls `sweep_stale_credentials`. The RDP file contains no secrets (only connection properties), so the exposure is limited to infrastructure metadata (hostnames, usernames, gateway addresses).

**Recommendation:** Consider using `tempfile` crate for automatic cleanup on drop, or document that `.rdp` files in `temp/` may linger after crashes.

#### 1.3.2 Bridge Lease File (`credential_state.rs`)

- Written as `{app_root}/temp/bridge-leases.json`
- Contains only target hostnames (no credentials)
- **Finding SEC-5:** TOCTOU race in `load()` + `save()` — if two processes run concurrently (e.g., two CLI launches), they could both read, modify, and write the lease file, losing entries. This is a correctness issue, not a security issue, since the worst case is failing to clean up a stale cmdkey entry.

**Recommendation:** Use file locking (`fs2` crate or platform-specific advisory locks) if concurrent launches are expected.

#### 1.3.3 SQLite Database

- No encryption (plain SQLite file)
- Contains profile names, hostnames, usernames, gateway addresses, session history
- **No passwords or tokens stored**
- Protected by OS-level file permissions (Windows ACLs on AppData)

**Recommendation:** Document that the database contains infrastructure metadata. Consider `SQLCipher` if users operate in environments where local file access by other processes is a concern. Not urgent for v1.

#### 1.3.4 Log File

- Append-only JSON lines to `{app_root}/logs/app.log`
- No log rotation built in
- **Finding SEC-6:** No size limit on the log file. Over time, it could grow unbounded.

**Recommendation:** Add log rotation (by size or date) or document that users should manage this externally.

### 1.4 Database Security — GOOD

- All queries use parameterized statements (`params![]` macro) — **no SQL injection possible**
- Foreign keys enabled with `ON DELETE CASCADE` — referential integrity maintained
- Schema uses `CREATE TABLE IF NOT EXISTS` — safe for re-migration
- No dynamic SQL construction anywhere

**No issues found.**

### 1.5 Windows API Usage — ACCEPTABLE

#### 1.5.1 Process Existence Check (`sessions.rs:102-114`)

```rust
unsafe {
    let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
        return false;
    };
    let _ = CloseHandle(handle);
    true
}
```

Uses the minimum-privilege access flag (`PROCESS_QUERY_LIMITED_INFORMATION`). Handle is properly closed. **No issues.**

#### 1.5.2 Window Enumeration (`reveal.rs:18-46`)

**Finding SEC-7:** Raw pointer cast in the `EnumWindows` callback:

```rust
let search = unsafe { &mut *(lparam.0 as *mut Search) };
```

This is the standard pattern for Win32 callbacks and is safe as long as the `search` variable outlives the `EnumWindows` call, which it does (it's a stack variable in the same function). The `unsafe` blocks are well-scoped and minimal.

**No action needed** — this is correct Win32 interop.

### 1.6 Dependency Analysis — CLEAN

| Crate | Version | Assessment |
| --- | --- | --- |
| `rusqlite` 0.37 (bundled) | Mature, widely audited | Safe |
| `serde` / `serde_json` 1.0 | Industry standard | Safe |
| `thiserror` 2.0 | Trivial derive macro | Safe |
| `time` 0.3 | Well-maintained, no CVEs | Safe |
| `uuid` 1.18 | Standard ID generation | Safe |
| `clap` 4.5 | Mature CLI parser | Safe |
| `windows` 0.62 | Microsoft-maintained | Safe |
| `dioxus` 0.7 | Active development, newer crate | Monitor |

No git dependencies, no path overrides (except workspace members), no deprecated or unmaintained crates.

**Recommendation:** Run `cargo audit` periodically. Consider adding it to CI.

### 1.7 Threat Matrix Summary

| # | Threat | Severity | Status |
| --- | --- | --- | --- |
| SEC-1 | Unvalidated helper executable path | Low | Recommend pre-flight check |
| SEC-2 | cmdkey argument parsing with special chars | Low | Recommend hostname validation |
| SEC-3 | Shell metachar in non-Windows stub | Info | Recommend removing shell usage |
| SEC-4 | Temp RDP files persist after crash | Low | Acceptable, document |
| SEC-5 | TOCTOU race in bridge lease file | Low | Recommend file locking for v2 |
| SEC-6 | Unbounded log file growth | Low | Recommend rotation |
| SEC-7 | Unsafe Win32 callback pointer | Info | Correct pattern, no action |

---

## Part 2: Rust Best Practices Analysis

### 2.1 Error Handling — EXCELLENT

- Consistent use of `thiserror` for error type derivation across all crates
- Each module defines its own error enum with descriptive messages
- Error propagation uses `?` operator throughout — no `.unwrap()` in non-test code
- Error types are specific (e.g., `LaunchRuntimeError::WriteRdp` vs `LaunchRuntimeError::RemoveRdp` distinguish write vs delete failures of the same file type)
- `CliError` aggregates all subsystem errors with transparent `#[from]` conversions

**No issues found.**

### 2.2 Type Safety — EXCELLENT

- Strong domain types: `ProfileId`, `PresetId`, `SessionState`, `SecurityMode`, `ScreenMode`, `GatewayUsageMode`
- `PropertyValue` enum with `PropertyRegistry` validation ensures type-correct RDP properties
- `CredentialFlow` encodes the credential resolution outcome as an enum variant, not stringly-typed
- `PromptBehavior` and `LaunchPolicy` work together to enforce policy at the type level

### 2.3 API Design — GOOD with suggestions

**Finding RUST-1: `ProfileStore` trait uses `&self` but `SqliteStore` wraps a non-threadsafe `Connection`.**

The `ProfileStore` trait takes `&self` for all methods, which is correct for single-threaded use, but `SqliteStore` wraps a `rusqlite::Connection` which is `Send` but not `Sync`. This means `SqliteStore` can't be shared across threads without external synchronization. If the desktop app ever needs concurrent access, this will need `Mutex<Connection>` or connection pooling.

**Recommendation:** Fine for now. When desktop UI needs async, consider wrapping the connection in a `Mutex` or switching to `r2d2-sqlite`.

**Finding RUST-2: `ProfileStore` trait mixes profile, preset, and session concerns.**

The trait has 9 methods spanning 3 domains (profiles, presets, sessions). This makes it harder to mock in tests and violates interface segregation.

**Recommendation:** Consider splitting into `ProfileStore`, `PresetStore`, and `SessionStore` traits. Not urgent — the current approach works fine for the vertical slice.

**Finding RUST-3: `PropertyRegistry` is `Copy` but could be `&'static`.**

`PropertyRegistry` is a zero-sized struct that serves as a lookup namespace for the static `SLICE_PROPERTY_DEFINITIONS` array. It's constructed in multiple places (`PropertyRegistry::new()`). Since it's stateless, a module-level function or a `const` would be simpler.

**Recommendation:** Minor style preference. Current approach is fine.

### 2.4 Clone/Allocation Patterns — MINOR IMPROVEMENTS POSSIBLE

**Finding RUST-4: Unnecessary `.clone()` in `save_profile` (`store.rs:234`).**

```rust
let profile = draft.clone().into_profile(id.clone(), now);
```

`draft` is consumed by `into_profile`, so the clone is needed to reuse `draft` in `replace_profile_properties`. However, `replace_profile_properties` could take a `&Profile` instead of `&ProfileDraft`, avoiding the clone entirely since the profile is already constructed.

**Finding RUST-5: Repeated `serde_json::json!({})` allocations in logging calls.**

Every `info()`, `warn()`, `error()`, and `debug()` call constructs a `serde_json::Value` via the `json!()` macro, even when logging is not initialized (the global logger is `None`). The current code silently discards these when no logger is set (`log_global` returns `Ok(())` early), but the `json!()` macro still allocates.

**Recommendation:** This is negligible for a desktop app. If hot-path performance ever matters, consider lazy evaluation (pass a closure that produces the fields).

**Finding RUST-6: String allocations in `property_pairs()` return type.**

`Profile::property_pairs()` and `Preset::property_pairs()` return `Vec<(&'static str, PropertyValue)>`. The `PropertyValue::String` variants clone the underlying strings. This is called during planning and serialization. For a desktop app this is fine, but the API could return references instead.

**Recommendation:** Not worth changing for the current use case.

### 2.5 Concurrency & Thread Safety — ACCEPTABLE

**Finding RUST-7: Double-mutex in `FileLogger`.**

```rust
static GLOBAL_LOGGER: OnceLock<Mutex<Option<FileLogger>>> = OnceLock::new();

pub struct FileLogger {
    file: Mutex<File>,
}
```

When logging, the code acquires the outer `Mutex<Option<FileLogger>>` guard, then calls `logger.log()` which acquires the inner `Mutex<File>`. This is a double-lock but not a deadlock risk since the locks are always acquired in the same order. However, the outer mutex is held while writing to the file, which is unnecessary.

**Recommendation:** Remove the inner `Mutex<File>` or change the outer to `OnceLock<FileLogger>` (initializing once, then using the inner mutex for writes). The `Option` in the outer mutex suggests re-initialization support, but `init_global_logger` is called once at startup.

**Finding RUST-8: `let _ = store.update_session_history(...)` silently drops errors in session tracking (`sessions.rs:49`).**

When the session tracker detects a state change (e.g., process exited), it updates the database but ignores the result. A database write failure would be silently lost.

**Recommendation:** Log the error instead of discarding it: `if let Err(e) = store.update_session_history(...) { error(...) }`

### 2.6 Polling Loop in Helper Client — IMPROVEMENT NEEDED

**Finding RUST-9: Busy-wait polling loop (`helper.rs:226-298`).**

```rust
loop {
    if start.elapsed() > timeout { ... }
    match child.try_wait() {
        Some(status) => { /* read stdout/stderr */ }
        None => std::thread::sleep(Duration::from_millis(25)),
    }
}
```

Issues:

1. **stdin is not dropped before the wait loop.** The helper process may be blocked reading stdin, waiting for EOF. The `stdin` handle (taken via `child.stdin.as_mut()`) is not explicitly dropped before entering the loop. Rust's ownership means `child` still owns the stdin pipe. This could cause a deadlock: the helper waits for stdin EOF, this code waits for the helper to exit.

2. **Polling with `try_wait` + sleep is less efficient than `child.wait_timeout_output()`.** The `wait-timeout` crate provides this, or you can drop stdin and call `child.wait_with_output()` in a separate thread with a timeout.

**Recommendation (important):** Drop `child.stdin` before the polling loop to signal EOF to the helper:

```rust
drop(child.stdin.take()); // Signal EOF to helper
let start = std::time::Instant::now();
loop { ... }
```

This is the most impactful finding in the codebase. Without it, helpers that read stdin to completion before processing will deadlock.

### 2.7 Test Quality — GOOD

- Tests use in-memory SQLite for isolation
- `RecordingBridge` mock in launcher tests demonstrates good trait-based testing
- `active_sessions_with_checker` takes a closure for process checking, enabling deterministic tests
- Tests clean up temp directories

**Finding RUST-10:** Tests create temp directories with UUID-based names but some cleanup paths use `let _ = fs::remove_dir_all(...)` which silently ignores cleanup failures. This is fine for tests.

### 2.8 Module Organization — CLEAN

- Clear separation: `core` (pure domain logic, no OS deps), `windows` (Win32 interop), `cli` (clap UI), `desktop` (dioxus UI)
- Each crate has a focused responsibility
- `lib.rs` re-exports are clean and comprehensive
- No circular dependencies

### 2.9 Serde Usage — GOOD

- Consistent `#[serde(rename_all = "snake_case")]` on enums
- `PropertyValue` uses `#[serde(untagged)]` — correct for the RDP property model
- `HelperConfig` uses `#[serde(default)]` for optional args
- No custom serializers where derive works

### 2.10 Unsafe Code — MINIMAL AND CORRECT

Only two locations use `unsafe`:

1. `sessions.rs:107-113` — `OpenProcess` / `CloseHandle` (Win32 FFI, minimal scope)
2. `reveal.rs:18-46` — `EnumWindows` callback with pointer cast (standard Win32 pattern)

Both are behind `#[cfg(target_os = "windows")]` and have non-Windows fallbacks. The unsafe blocks are well-scoped and don't extend beyond what's necessary.

---

## Part 3: Prioritized Recommendations

### Must Fix (before v1)

| # | Issue | File | Effort |
| --- | --- | --- | --- |
| RUST-9 | **Drop stdin before helper wait loop** — potential deadlock | `helper.rs:222` | 1 line |
| SEC-3 | Remove shell usage in non-Windows stub | `launcher.rs:233-240` | 2 lines |

### Should Fix (before wider adoption)

| # | Issue | File | Effort |
| --- | --- | --- | --- |
| RUST-8 | Log errors from `update_session_history` instead of discarding | `sessions.rs:49` | 5 lines |
| SEC-2 | Validate cmdkey target/username characters | `credentials.rs` | ~20 lines |
| SEC-1 | Pre-flight check on helper executable | `helper.rs` | ~10 lines |
| SEC-6 | Add log rotation or size cap | `logging.rs` | ~30 lines |
| RUST-7 | Simplify double-mutex in logger | `logging.rs` | ~15 lines |

### Nice to Have (future improvement)

| # | Issue | File | Effort |
| --- | --- | --- | --- |
| RUST-2 | Split `ProfileStore` trait into focused traits | `store.rs` | Medium refactor |
| RUST-4 | Avoid unnecessary draft clone in `save_profile` | `store.rs:234` | Small refactor |
| SEC-5 | File locking for bridge lease state | `credential_state.rs` | ~20 lines |
| SEC-4 | Use `tempfile` crate for RDP files | `launcher.rs` | Small refactor |

---

## Appendix: Files Reviewed

| File | Lines | Purpose |
| --- | --- | --- |
| `crates/core/src/lib.rs` | 35 | Re-exports |
| `crates/core/src/helper.rs` | 323 | Helper process client |
| `crates/core/src/launch.rs` | 185 | Launch planning |
| `crates/core/src/logging.rs` | 199 | Structured file logging |
| `crates/core/src/preset.rs` | 118 | Preset domain model |
| `crates/core/src/profile.rs` | 179 | Profile domain model |
| `crates/core/src/rdp.rs` | 67 | RDP file serialization |
| `crates/core/src/registry.rs` | 180 | Property type registry |
| `crates/core/src/session.rs` | 38 | Session domain model |
| `crates/core/src/store.rs` | 1057 | SQLite persistence |
| `crates/windows/src/lib.rs` | 17 | Re-exports |
| `crates/windows/src/credentials.rs` | 79 | cmdkey credential bridge |
| `crates/windows/src/credential_state.rs` | 70 | Bridge lease tracking |
| `crates/windows/src/launcher.rs` | 369 | MSTSC launch runtime |
| `crates/windows/src/paths.rs` | 18 | AppData path discovery |
| `crates/windows/src/reveal.rs` | 61 | Win32 window enumeration |
| `crates/windows/src/sessions.rs` | 285 | Process session tracking |
| `crates/cli/src/main.rs` | 545 | CLI entry point |
| `Cargo.toml` | 33 | Workspace config |
