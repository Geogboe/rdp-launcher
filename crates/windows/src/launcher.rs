use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rdp_launch_core::{
    NewSessionHistoryEntry, ProfileStore, SerializedRdp, SessionHistoryEntry, SessionState, debug,
    error, info, warn,
};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::credential_state::{BridgeLeaseStore, BridgeLeaseStoreError};
use crate::credentials::{CredentialBridge, CredentialBridgeError, TemporaryCredential};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRuntimeRequest {
    pub profile_id: String,
    pub profile_name: String,
    pub target: String,
    pub serialized_rdp: SerializedRdp,
    pub temporary_credential: Option<TemporaryCredential>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedProcess {
    pub process_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchArtifacts {
    pub launch_id: String,
    pub rdp_path: PathBuf,
    pub process: SpawnedProcess,
    pub history: SessionHistoryEntry,
}

pub struct LaunchRuntime<B> {
    bridge: B,
}

impl<B> LaunchRuntime<B>
where
    B: CredentialBridge,
{
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }

    pub fn launch<S: ProfileStore>(
        &self,
        store: &S,
        app_root: impl Into<PathBuf>,
        request: LaunchRuntimeRequest,
    ) -> Result<LaunchArtifacts, LaunchRuntimeError> {
        let app_root = app_root.into();
        let launch_id = Uuid::now_v7().to_string();
        info(
            "runtime.launch.started",
            "starting mstsc launch runtime",
            serde_json::json!({
                "launch_id": &launch_id,
                "profile_id": &request.profile_id,
                "profile_name": &request.profile_name,
                "target": &request.target,
                "uses_temporary_credential": request.temporary_credential.is_some(),
            }),
        );
        let launch_dir = app_root.join("temp");
        fs::create_dir_all(&launch_dir).map_err(LaunchRuntimeError::CreateTempDir)?;
        let lease_store = BridgeLeaseStore::from_app_root(&app_root);

        let rdp_path = launch_dir.join(format!("{launch_id}.rdp"));
        fs::write(&rdp_path, request.serialized_rdp.text.as_bytes())
            .map_err(LaunchRuntimeError::WriteRdp)?;

        if let Some(credential) = &request.temporary_credential {
            self.bridge
                .install(credential)
                .map_err(LaunchRuntimeError::CredentialBridge)?;
            lease_store
                .record_target(&credential.target)
                .map_err(LaunchRuntimeError::BridgeLeaseStore)?;
            info(
                "runtime.bridge.installed",
                "installed temporary credential bridge entry",
                serde_json::json!({
                    "launch_id": &launch_id,
                    "target": &credential.target,
                }),
            );
        }

        let mut command = build_mstsc_command(&rdp_path);
        let child = command.spawn().map_err(LaunchRuntimeError::Spawn)?;
        let process = SpawnedProcess {
            process_id: child.id(),
        };

        let history = store.insert_session_history(NewSessionHistoryEntry {
            profile_id: request.profile_id,
            profile_name: request.profile_name,
            target: request.target,
            process_id: Some(process.process_id),
            state: SessionState::Launching,
            started_at: OffsetDateTime::now_utc(),
            ended_at: None,
            error_message: None,
        })?;

        Ok(LaunchArtifacts {
            launch_id,
            rdp_path,
            process,
            history,
        })
        .inspect(|artifacts| {
            info(
                "runtime.launch.succeeded",
                "mstsc launch runtime recorded session",
                serde_json::json!({
                    "launch_id": artifacts.launch_id,
                    "process_id": artifacts.process.process_id,
                    "rdp_path": artifacts.rdp_path.display().to_string(),
                }),
            );
        })
    }

    pub fn cleanup(
        &self,
        artifacts: &LaunchArtifacts,
        credential_target: Option<&str>,
    ) -> Result<(), LaunchRuntimeError> {
        if artifacts.rdp_path.exists() {
            fs::remove_file(&artifacts.rdp_path).map_err(LaunchRuntimeError::RemoveRdp)?;
            debug(
                "runtime.cleanup.removed_rdp",
                "removed temporary rdp file",
                serde_json::json!({
                    "launch_id": artifacts.launch_id,
                    "rdp_path": artifacts.rdp_path.display().to_string(),
                }),
            );
        }

        if let Some(target) = credential_target {
            self.bridge
                .remove(target)
                .map_err(LaunchRuntimeError::CredentialBridge)?;
            BridgeLeaseStore::from_app_root(
                artifacts
                    .rdp_path
                    .parent()
                    .and_then(|temp| temp.parent())
                    .ok_or(LaunchRuntimeError::MissingAppRoot)?,
            )
            .remove_target(target)
            .map_err(LaunchRuntimeError::BridgeLeaseStore)?;
            info(
                "runtime.cleanup.removed_bridge",
                "removed temporary credential bridge entry",
                serde_json::json!({
                    "launch_id": artifacts.launch_id,
                    "target": target,
                }),
            );
        }

        Ok(())
    }

    pub fn sweep_stale_credentials(
        &self,
        app_root: impl AsRef<Path>,
    ) -> Result<Vec<String>, LaunchRuntimeError> {
        let lease_store = BridgeLeaseStore::from_app_root(app_root);
        let targets = lease_store
            .list_targets()
            .map_err(LaunchRuntimeError::BridgeLeaseStore)?;
        let mut removed = Vec::new();

        if !targets.is_empty() {
            warn(
                "runtime.bridge.sweep.started",
                "sweeping stale credential bridge targets",
                serde_json::json!({
                    "target_count": targets.len(),
                }),
            );
        }

        for target in targets {
            self.bridge.remove(&target).map_err(|bridge_error| {
                error(
                    "runtime.bridge.sweep.failed",
                    "failed removing stale credential bridge target",
                    serde_json::json!({
                        "target": &target,
                        "error": bridge_error.to_string(),
                    }),
                );
                LaunchRuntimeError::CredentialBridge(bridge_error)
            })?;
            lease_store
                .remove_target(&target)
                .map_err(LaunchRuntimeError::BridgeLeaseStore)?;
            removed.push(target);
        }

        if !removed.is_empty() {
            info(
                "runtime.bridge.sweep.completed",
                "swept stale credential bridge targets",
                serde_json::json!({
                    "target_count": removed.len(),
                }),
            );
        }

        Ok(removed)
    }
}

fn build_mstsc_command(rdp_path: &Path) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("mstsc.exe");
        command.arg(rdp_path);
        command
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = rdp_path;
        let mut command = Command::new("sleep");
        command.arg("1");
        command
    }
}

#[derive(Debug, Error)]
pub enum LaunchRuntimeError {
    #[error("failed to create temporary launch directory: {0}")]
    CreateTempDir(std::io::Error),
    #[error("failed to write temporary .rdp file: {0}")]
    WriteRdp(std::io::Error),
    #[error("credential bridge failed: {0}")]
    CredentialBridge(#[from] CredentialBridgeError),
    #[error("failed to spawn mstsc runtime: {0}")]
    Spawn(std::io::Error),
    #[error("store error: {0}")]
    Store(#[from] rdp_launch_core::store::StoreError),
    #[error("failed to remove temporary .rdp file: {0}")]
    RemoveRdp(std::io::Error),
    #[error("failed to update bridge lease state: {0}")]
    BridgeLeaseStore(#[from] BridgeLeaseStoreError),
    #[error("unable to resolve app root from launch artifacts")]
    MissingAppRoot,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use rdp_launch_core::store::SqliteStore;
    use rdp_launch_core::{
        GatewayUsageMode, ProfileDraft, ProfileStore, PromptBehavior, ScreenMode, SecurityMode,
    };

    use super::*;

    #[derive(Default)]
    struct RecordingBridge {
        installed: RefCell<Vec<String>>,
        removed: RefCell<Vec<String>>,
    }

    impl CredentialBridge for RecordingBridge {
        fn install(&self, credential: &TemporaryCredential) -> Result<(), CredentialBridgeError> {
            self.installed.borrow_mut().push(credential.target.clone());
            Ok(())
        }

        fn remove(&self, target: &str) -> Result<(), CredentialBridgeError> {
            self.removed.borrow_mut().push(target.to_owned());
            Ok(())
        }
    }

    fn sample_profile(store: &SqliteStore) -> String {
        store
            .save_profile(ProfileDraft {
                name: "Build Agent".to_owned(),
                full_address: "build-03.lab.example".to_owned(),
                username: Some("lab\\builder".to_owned()),
                screen_mode: ScreenMode::Windowed,
                use_multimon: false,
                selected_monitors: None,
                redirect_clipboard: true,
                gateway_hostname: None,
                gateway_usage: GatewayUsageMode::Never,
                prompt_behavior: PromptBehavior::Helper,
                allow_windows_credential_bridge: true,
                security_mode: SecurityMode::Default,
            })
            .expect("profile")
            .id
    }

    #[test]
    fn launch_runtime_writes_rdp_and_records_history() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile_id = sample_profile(&store);
        let runtime = LaunchRuntime::new(RecordingBridge::default());
        let temp_dir = std::env::temp_dir().join(format!("rdp-launch-test-{}", Uuid::now_v7()));

        let artifacts = runtime
            .launch(
                &store,
                &temp_dir,
                LaunchRuntimeRequest {
                    profile_id: profile_id.clone(),
                    profile_name: "Build Agent".to_owned(),
                    target: "build-03.lab.example".to_owned(),
                    serialized_rdp: SerializedRdp {
                        text: "full address:s:build-03.lab.example\n".to_owned(),
                    },
                    temporary_credential: Some(TemporaryCredential {
                        target: "build-03.lab.example".to_owned(),
                        username: "lab\\builder".to_owned(),
                        password: "secret".to_owned(),
                    }),
                },
            )
            .expect("launch");

        assert!(artifacts.rdp_path.exists());

        let history = store
            .list_session_history(Default::default())
            .expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].profile_id, profile_id);

        runtime
            .cleanup(&artifacts, Some("build-03.lab.example"))
            .expect("cleanup");
        assert!(!artifacts.rdp_path.exists());
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn launch_runtime_sweeps_stale_lease_targets() {
        let runtime = LaunchRuntime::new(RecordingBridge::default());
        let temp_dir = std::env::temp_dir().join(format!("rdp-launch-test-{}", Uuid::now_v7()));
        let lease_store = BridgeLeaseStore::from_app_root(&temp_dir);
        lease_store
            .record_target("stale-target.example")
            .expect("record target");

        let removed = runtime
            .sweep_stale_credentials(&temp_dir)
            .expect("sweep stale");
        assert_eq!(removed, vec!["stale-target.example".to_owned()]);
        let _ = fs::remove_dir_all(temp_dir);
    }
}
