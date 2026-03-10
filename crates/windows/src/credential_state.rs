use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BridgeLeaseState {
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BridgeLeaseStore {
    path: PathBuf,
}

impl BridgeLeaseStore {
    pub fn from_app_root(app_root: impl AsRef<Path>) -> Self {
        let path = app_root.as_ref().join("temp").join("bridge-leases.json");
        Self { path }
    }

    pub fn record_target(&self, target: &str) -> Result<(), BridgeLeaseStoreError> {
        let mut state = self.load()?;
        if !state.targets.iter().any(|existing| existing == target) {
            state.targets.push(target.to_owned());
        }
        self.save(&state)
    }

    pub fn remove_target(&self, target: &str) -> Result<(), BridgeLeaseStoreError> {
        let mut state = self.load()?;
        state.targets.retain(|existing| existing != target);
        self.save(&state)
    }

    pub fn list_targets(&self) -> Result<Vec<String>, BridgeLeaseStoreError> {
        Ok(self.load()?.targets)
    }

    fn load(&self) -> Result<BridgeLeaseState, BridgeLeaseStoreError> {
        if !self.path.exists() {
            return Ok(BridgeLeaseState::default());
        }

        let contents = fs::read_to_string(&self.path).map_err(BridgeLeaseStoreError::Read)?;
        serde_json::from_str(&contents).map_err(BridgeLeaseStoreError::Json)
    }

    fn save(&self, state: &BridgeLeaseState) -> Result<(), BridgeLeaseStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(BridgeLeaseStoreError::CreateDir)?;
        }
        let json = serde_json::to_string_pretty(state).map_err(BridgeLeaseStoreError::Json)?;
        fs::write(&self.path, json).map_err(BridgeLeaseStoreError::Write)
    }
}

#[derive(Debug, Error)]
pub enum BridgeLeaseStoreError {
    #[error("failed to create bridge lease directory: {0}")]
    CreateDir(std::io::Error),
    #[error("failed to read bridge lease file: {0}")]
    Read(std::io::Error),
    #[error("failed to write bridge lease file: {0}")]
    Write(std::io::Error),
    #[error("failed to decode bridge lease json: {0}")]
    Json(serde_json::Error),
}
