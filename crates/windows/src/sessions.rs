use rdp_launch_core::{
    ObservedSession, ProfileStore, SessionHistoryFilter, SessionHistoryUpdate, SessionState,
};
use rdp_launch_core::{debug, error, info};
use thiserror::Error;
use time::OffsetDateTime;

pub trait SessionTracker {
    fn active_sessions<S: ProfileStore>(
        &self,
        store: &S,
    ) -> Result<Vec<ObservedSession>, SessionTrackerError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessSessionTracker;

impl SessionTracker for ProcessSessionTracker {
    fn active_sessions<S: ProfileStore>(
        &self,
        store: &S,
    ) -> Result<Vec<ObservedSession>, SessionTrackerError> {
        active_sessions_with_checker(store, process_exists)
    }
}

fn active_sessions_with_checker<S, F>(
    store: &S,
    mut process_checker: F,
) -> Result<Vec<ObservedSession>, SessionTrackerError>
where
    S: ProfileStore,
    F: FnMut(u32) -> bool,
{
    let history = store.list_session_history(SessionHistoryFilter { limit: 100 })?;
    let sessions = history
        .into_iter()
        .filter(|entry| matches!(entry.state, SessionState::Launching | SessionState::Active))
        .filter_map(|entry| {
            let pid = entry.process_id?;
            let state = if process_checker(pid) {
                SessionState::Active
            } else {
                SessionState::Exited
            };

            if state != entry.state {
                let ended_at = matches!(state, SessionState::Exited).then(OffsetDateTime::now_utc);
                if let Err(update_error) = store.update_session_history(
                    &entry.launch_id,
                    SessionHistoryUpdate {
                        state,
                        ended_at,
                        error_message: entry.error_message.clone(),
                    },
                ) {
                    error(
                        "sessions.state_update_failed",
                        "failed to persist tracked session state update",
                        serde_json::json!({
                            "launch_id": entry.launch_id,
                            "process_id": pid,
                            "state": match state {
                                SessionState::Launching => "launching",
                                SessionState::Active => "active",
                                SessionState::Exited => "exited",
                                SessionState::Failed => "failed",
                            },
                            "error": update_error.to_string(),
                        }),
                    );
                } else if matches!(state, SessionState::Active) {
                    debug(
                        "sessions.state_promoted",
                        "promoted tracked session to active",
                        serde_json::json!({
                            "launch_id": entry.launch_id,
                            "process_id": pid,
                        }),
                    );
                } else {
                    info(
                        "sessions.state_finalized",
                        "finalized tracked session as exited",
                        serde_json::json!({
                            "launch_id": entry.launch_id,
                            "process_id": pid,
                        }),
                    );
                }
            }

            if matches!(state, SessionState::Active) {
                Some(ObservedSession {
                    launch_id: entry.launch_id,
                    profile_id: entry.profile_id,
                    profile_name: entry.profile_name,
                    target: entry.target,
                    process_id: pid,
                    state,
                    started_at: entry.started_at,
                    ended_at: if matches!(state, SessionState::Exited) {
                        Some(OffsetDateTime::now_utc())
                    } else {
                        None
                    },
                    window_title: None,
                })
            } else {
                None
            }
        })
        .collect();
    Ok(sessions)
}

#[cfg(target_os = "windows")]
fn process_exists(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let _ = CloseHandle(handle);
        true
    }
}

#[cfg(not(target_os = "windows"))]
fn process_exists(pid: u32) -> bool {
    std::path::PathBuf::from(format!("/proc/{pid}")).exists()
}

#[derive(Debug, Error)]
pub enum SessionTrackerError {
    #[error("store error: {0}")]
    Store(#[from] rdp_launch_core::store::StoreError),
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use rdp_launch_core::store::SqliteStore;
    use rdp_launch_core::{GatewayUsageMode, PromptBehavior, ScreenMode, SecurityMode};
    use rdp_launch_core::{NewSessionHistoryEntry, ProfileDraft, ProfileStore};

    use super::*;

    #[test]
    fn tracker_filters_to_running_processes() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store
            .save_profile(ProfileDraft {
                name: "Design Workstation".to_owned(),
                full_address: "192.0.2.10".to_owned(),
                username: None,
                screen_mode: ScreenMode::Windowed,
                use_multimon: false,
                selected_monitors: None,
                redirect_clipboard: true,
                gateway_hostname: None,
                gateway_usage: GatewayUsageMode::Never,
                prompt_behavior: PromptBehavior::Prompt,
                allow_windows_credential_bridge: false,
                security_mode: SecurityMode::Default,
            })
            .expect("profile");

        store
            .insert_session_history(NewSessionHistoryEntry {
                profile_id: profile.id,
                profile_name: profile.name,
                target: profile.full_address,
                process_id: Some(std::process::id()),
                state: SessionState::Active,
                started_at: OffsetDateTime::now_utc(),
                ended_at: None,
                error_message: None,
            })
            .expect("history");

        let sessions = ProcessSessionTracker
            .active_sessions(&store)
            .expect("sessions");
        assert_eq!(sessions.len(), 1);

        let history = store
            .list_session_history(rdp_launch_core::SessionHistoryFilter { limit: 5 })
            .expect("history");
        assert_eq!(history[0].state, SessionState::Active);
    }

    #[test]
    fn tracker_finalizes_missing_processes() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store
            .save_profile(ProfileDraft {
                name: "Design Workstation".to_owned(),
                full_address: "192.0.2.10".to_owned(),
                username: None,
                screen_mode: ScreenMode::Windowed,
                use_multimon: false,
                selected_monitors: None,
                redirect_clipboard: true,
                gateway_hostname: None,
                gateway_usage: GatewayUsageMode::Never,
                prompt_behavior: PromptBehavior::Prompt,
                allow_windows_credential_bridge: false,
                security_mode: SecurityMode::Default,
            })
            .expect("profile");

        store
            .insert_session_history(NewSessionHistoryEntry {
                profile_id: profile.id,
                profile_name: profile.name,
                target: profile.full_address,
                process_id: Some(u32::MAX),
                state: SessionState::Launching,
                started_at: OffsetDateTime::now_utc(),
                ended_at: None,
                error_message: None,
            })
            .expect("history");

        let sessions = ProcessSessionTracker
            .active_sessions(&store)
            .expect("sessions");
        assert!(sessions.is_empty());

        let history = store
            .list_session_history(rdp_launch_core::SessionHistoryFilter { limit: 5 })
            .expect("history");
        assert_eq!(history[0].state, SessionState::Exited);
        assert!(history[0].ended_at.is_some());
    }

    #[test]
    fn tracker_persists_launch_lifecycle_across_refreshes() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store
            .save_profile(ProfileDraft {
                name: "Design Workstation".to_owned(),
                full_address: "192.0.2.10".to_owned(),
                username: None,
                screen_mode: ScreenMode::Windowed,
                use_multimon: false,
                selected_monitors: None,
                redirect_clipboard: true,
                gateway_hostname: None,
                gateway_usage: GatewayUsageMode::Never,
                prompt_behavior: PromptBehavior::Prompt,
                allow_windows_credential_bridge: false,
                security_mode: SecurityMode::Default,
            })
            .expect("profile");

        store
            .insert_session_history(NewSessionHistoryEntry {
                profile_id: profile.id,
                profile_name: profile.name,
                target: profile.full_address,
                process_id: Some(4242),
                state: SessionState::Launching,
                started_at: OffsetDateTime::now_utc(),
                ended_at: None,
                error_message: None,
            })
            .expect("history");

        let process_running = Cell::new(true);
        let running_sessions =
            active_sessions_with_checker(&store, |pid| process_running.get() && pid == 4242)
                .expect("sessions");
        assert_eq!(running_sessions.len(), 1);
        assert_eq!(running_sessions[0].state, SessionState::Active);

        let history = store
            .list_session_history(rdp_launch_core::SessionHistoryFilter { limit: 5 })
            .expect("history");
        assert_eq!(history[0].state, SessionState::Active);
        assert!(history[0].ended_at.is_none());

        process_running.set(false);
        let exited_sessions =
            active_sessions_with_checker(&store, |pid| process_running.get() && pid == 4242)
                .expect("sessions");
        assert!(exited_sessions.is_empty());

        let history = store
            .list_session_history(rdp_launch_core::SessionHistoryFilter { limit: 5 })
            .expect("history");
        assert_eq!(history[0].state, SessionState::Exited);
        assert!(history[0].ended_at.is_some());
    }
}
