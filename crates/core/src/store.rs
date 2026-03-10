use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::preset::{Preset, PresetDraft, PresetSummary};
use crate::profile::{
    GatewayUsageMode, Profile, ProfileDraft, ProfileId, ProfileSummary, PromptBehavior, ScreenMode,
    SecurityMode,
};
use crate::session::{SessionHistoryEntry, SessionState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
    pub database: PathBuf,
    pub logs_dir: PathBuf,
    pub app_log: PathBuf,
}

impl AppPaths {
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let logs_dir = root.join("logs");
        Self {
            database: root.join("rdp-launch.db"),
            app_log: logs_dir.join("app.log"),
            logs_dir,
            root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSessionHistoryEntry {
    pub profile_id: String,
    pub profile_name: String,
    pub target: String,
    pub process_id: Option<u32>,
    pub state: SessionState,
    pub started_at: OffsetDateTime,
    pub ended_at: Option<OffsetDateTime>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHistoryUpdate {
    pub state: SessionState,
    pub ended_at: Option<OffsetDateTime>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHistoryFilter {
    pub limit: usize,
}

impl Default for SessionHistoryFilter {
    fn default() -> Self {
        Self { limit: 20 }
    }
}

pub trait ProfileStore {
    fn save_profile(&self, draft: ProfileDraft) -> Result<Profile, StoreError>;
    fn update_profile(&self, profile_id: &str, draft: ProfileDraft) -> Result<Profile, StoreError>;
    fn delete_profile(&self, profile_id: &str) -> Result<(), StoreError>;
    fn list_profiles(&self) -> Result<Vec<ProfileSummary>, StoreError>;
    fn get_profile(&self, profile_id: &str) -> Result<Option<Profile>, StoreError>;
    fn save_preset(&self, draft: PresetDraft) -> Result<Preset, StoreError>;
    fn list_presets(&self, profile_id: &str) -> Result<Vec<PresetSummary>, StoreError>;
    fn get_preset(&self, preset_id: &str) -> Result<Option<Preset>, StoreError>;
    fn insert_session_history(
        &self,
        entry: NewSessionHistoryEntry,
    ) -> Result<SessionHistoryEntry, StoreError>;
    fn update_session_history(
        &self,
        launch_id: &str,
        update: SessionHistoryUpdate,
    ) -> Result<(), StoreError>;
    fn list_session_history(
        &self,
        filter: SessionHistoryFilter,
    ) -> Result<Vec<SessionHistoryEntry>, StoreError>;
}

#[derive(Debug)]
pub struct SqliteStore {
    connection: Connection,
}

impl SqliteStore {
    pub fn open(paths: &AppPaths) -> Result<Self, StoreError> {
        fs::create_dir_all(&paths.root).map_err(StoreError::CreateDir)?;
        let connection = Connection::open(&paths.database).map_err(StoreError::Sql)?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory().map_err(StoreError::Sql)?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.connection
            .execute_batch(
                "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                prompt_behavior TEXT NOT NULL,
                allow_windows_credential_bridge INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS profile_properties (
                profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
                property_key TEXT NOT NULL,
                property_value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (profile_id, property_key)
            );

            CREATE TABLE IF NOT EXISTS presets (
                id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS preset_properties (
                preset_id TEXT NOT NULL REFERENCES presets(id) ON DELETE CASCADE,
                property_key TEXT NOT NULL,
                property_value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (preset_id, property_key)
            );

            CREATE TABLE IF NOT EXISTS session_history (
                launch_id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
                profile_name TEXT NOT NULL,
                target TEXT NOT NULL,
                process_id INTEGER,
                state TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                error_message TEXT
            );
            ",
            )
            .map_err(StoreError::Sql)
    }

    fn replace_profile_properties(
        &self,
        profile_id: &str,
        draft: &ProfileDraft,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        self.connection
            .execute(
                "DELETE FROM profile_properties WHERE profile_id = ?1",
                params![profile_id],
            )
            .map_err(StoreError::Sql)?;

        for (key, value) in draft
            .clone()
            .into_profile(profile_id.to_owned(), now)
            .property_pairs()
        {
            let value_json = serde_json::to_string(&value).map_err(StoreError::Json)?;
            self.connection
                .execute(
                    "INSERT INTO profile_properties (profile_id, property_key, property_value_json, updated_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![profile_id, key, value_json, now.format(&time::format_description::well_known::Rfc3339).map_err(StoreError::Time)?],
                )
                .map_err(StoreError::Sql)?;
        }

        Ok(())
    }

    fn replace_preset_properties(
        &self,
        preset_id: &str,
        preset: &Preset,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        self.connection
            .execute(
                "DELETE FROM preset_properties WHERE preset_id = ?1",
                params![preset_id],
            )
            .map_err(StoreError::Sql)?;

        let updated_at = now
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(StoreError::Time)?;

        for (key, value) in preset.property_pairs() {
            let value_json = serde_json::to_string(&value).map_err(StoreError::Json)?;
            self.connection
                .execute(
                    "INSERT INTO preset_properties (preset_id, property_key, property_value_json, updated_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![preset_id, key, value_json, &updated_at],
                )
                .map_err(StoreError::Sql)?;
        }

        Ok(())
    }
}

impl ProfileStore for SqliteStore {
    fn save_profile(&self, draft: ProfileDraft) -> Result<Profile, StoreError> {
        let id = Uuid::now_v7().to_string();
        let now = OffsetDateTime::now_utc();
        let profile = draft.clone().into_profile(id.clone(), now);

        self.connection
            .execute(
                "INSERT INTO profiles
                (id, name, prompt_behavior, allow_windows_credential_bridge, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &profile.id,
                    &profile.name,
                    prompt_behavior_name(profile.prompt_behavior),
                    i64::from(profile.allow_windows_credential_bridge),
                    profile
                        .created_at
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(StoreError::Time)?,
                    profile
                        .updated_at
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(StoreError::Time)?
                ],
            )
            .map_err(StoreError::Sql)?;
        self.replace_profile_properties(&id, &draft, now)?;
        Ok(profile)
    }

    fn update_profile(&self, profile_id: &str, draft: ProfileDraft) -> Result<Profile, StoreError> {
        let now = OffsetDateTime::now_utc();
        self.connection
            .execute(
                "UPDATE profiles
                 SET name = ?2, prompt_behavior = ?3, allow_windows_credential_bridge = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![
                    profile_id,
                    draft.name,
                    prompt_behavior_name(draft.prompt_behavior),
                    i64::from(draft.allow_windows_credential_bridge),
                    now.format(&time::format_description::well_known::Rfc3339).map_err(StoreError::Time)?
                ],
            )
            .map_err(StoreError::Sql)?;
        self.replace_profile_properties(profile_id, &draft, now)?;
        self.get_profile(profile_id)?
            .ok_or_else(|| StoreError::MissingProfile(profile_id.to_owned()))
    }

    fn delete_profile(&self, profile_id: &str) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM profiles WHERE id = ?1", params![profile_id])
            .map_err(StoreError::Sql)?;
        Ok(())
    }

    fn list_profiles(&self) -> Result<Vec<ProfileSummary>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT p.id, p.name, p.prompt_behavior, p.allow_windows_credential_bridge,
                MAX(CASE WHEN pp.property_key = 'full address' THEN pp.property_value_json END) AS full_address,
                MAX(CASE WHEN pp.property_key = 'username' THEN pp.property_value_json END) AS username,
                MAX(CASE WHEN pp.property_key = 'restricted admin mode' THEN pp.property_value_json END) AS restricted_admin,
                MAX(CASE WHEN pp.property_key = 'enablerdsaadauth' THEN pp.property_value_json END) AS remote_guard,
                p.updated_at AS last_used_at
             FROM profiles p
             LEFT JOIN profile_properties pp ON pp.profile_id = p.id
             GROUP BY p.id, p.name, p.prompt_behavior, p.allow_windows_credential_bridge, p.updated_at
             ORDER BY p.name COLLATE NOCASE",
        ).map_err(StoreError::Sql)?;

        let rows = statement
            .query_map([], |row| {
                let full_address_json: Option<String> = row.get(4)?;
                let full_address = full_address_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok())
                    .unwrap_or_default();
                let username = row
                    .get::<_, Option<String>>(5)?
                    .and_then(|value| serde_json::from_str::<String>(&value).ok());
                let restricted_admin = row
                    .get::<_, Option<String>>(6)?
                    .and_then(|value| serde_json::from_str::<i64>(&value).ok())
                    .unwrap_or_default();
                let remote_guard = row
                    .get::<_, Option<String>>(7)?
                    .and_then(|value| serde_json::from_str::<bool>(&value).ok())
                    .unwrap_or(false);
                let last_used_at = row.get::<_, Option<String>>(8)?.and_then(|value| {
                    OffsetDateTime::parse(&value, &time::format_description::well_known::Rfc3339)
                        .ok()
                });
                Ok(ProfileSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    full_address,
                    username,
                    security_mode: if restricted_admin == 1 {
                        SecurityMode::RestrictedAdmin
                    } else if remote_guard {
                        SecurityMode::RemoteGuard
                    } else {
                        SecurityMode::Default
                    },
                    last_used_at,
                })
            })
            .map_err(StoreError::Sql)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sql)
    }

    fn get_profile(&self, profile_id: &str) -> Result<Option<Profile>, StoreError> {
        let profile_row = self.connection.query_row(
            "SELECT id, name, prompt_behavior, allow_windows_credential_bridge, created_at, updated_at
             FROM profiles WHERE id = ?1",
            params![profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        ).optional().map_err(StoreError::Sql)?;

        let Some((
            id,
            name,
            prompt_behavior_name,
            allow_windows_credential_bridge,
            created_at,
            updated_at,
        )) = profile_row
        else {
            return Ok(None);
        };

        let mut statement = self.connection.prepare(
            "SELECT property_key, property_value_json FROM profile_properties WHERE profile_id = ?1",
        ).map_err(StoreError::Sql)?;
        let mut rows = statement
            .query(params![profile_id])
            .map_err(StoreError::Sql)?;

        let mut full_address = String::new();
        let mut username = None;
        let mut screen_mode = ScreenMode::Windowed;
        let mut use_multimon = false;
        let mut selected_monitors = None;
        let mut redirect_clipboard = true;
        let mut gateway_hostname = None;
        let mut gateway_usage = GatewayUsageMode::Never;
        let prompt_behavior = parse_prompt_behavior(&prompt_behavior_name);
        let allow_windows_credential_bridge = allow_windows_credential_bridge == 1;
        let mut security_mode = SecurityMode::Default;

        while let Some(row) = rows.next().map_err(StoreError::Sql)? {
            let key: String = row.get(0).map_err(StoreError::Sql)?;
            let value_json: String = row.get(1).map_err(StoreError::Sql)?;
            match key.as_str() {
                "full address" => {
                    full_address = serde_json::from_str(&value_json).map_err(StoreError::Json)?
                }
                "username" => {
                    username = Some(serde_json::from_str(&value_json).map_err(StoreError::Json)?)
                }
                "screen mode id" => {
                    let value: i64 = serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    screen_mode = if value == 2 {
                        ScreenMode::Fullscreen
                    } else {
                        ScreenMode::Windowed
                    };
                }
                "use multimon" => {
                    use_multimon = serde_json::from_str(&value_json).map_err(StoreError::Json)?
                }
                "selectedmonitors" => {
                    selected_monitors =
                        Some(serde_json::from_str(&value_json).map_err(StoreError::Json)?)
                }
                "redirectclipboard" => {
                    redirect_clipboard =
                        serde_json::from_str(&value_json).map_err(StoreError::Json)?
                }
                "gatewayhostname" => {
                    gateway_hostname =
                        Some(serde_json::from_str(&value_json).map_err(StoreError::Json)?)
                }
                "gatewayusagemethod" => {
                    let value: i64 = serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    gateway_usage = match value {
                        1 => GatewayUsageMode::Always,
                        2 => GatewayUsageMode::Detect,
                        4 => GatewayUsageMode::Default,
                        _ => GatewayUsageMode::Never,
                    };
                }
                "enablerdsaadauth" => {
                    let enabled: bool =
                        serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    if enabled {
                        security_mode = SecurityMode::RemoteGuard;
                    }
                }
                "restricted admin mode" => {
                    let value: i64 = serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    if value == 1 {
                        security_mode = SecurityMode::RestrictedAdmin;
                    }
                }
                _ => {}
            }
        }

        Ok(Some(Profile {
            id,
            name,
            full_address,
            username,
            screen_mode,
            use_multimon,
            selected_monitors,
            redirect_clipboard,
            gateway_hostname,
            gateway_usage,
            prompt_behavior,
            allow_windows_credential_bridge,
            security_mode,
            created_at: OffsetDateTime::parse(
                &created_at,
                &time::format_description::well_known::Rfc3339,
            )
            .map_err(StoreError::TimeParse)?,
            updated_at: OffsetDateTime::parse(
                &updated_at,
                &time::format_description::well_known::Rfc3339,
            )
            .map_err(StoreError::TimeParse)?,
        }))
    }

    fn save_preset(&self, draft: PresetDraft) -> Result<Preset, StoreError> {
        let id = Uuid::now_v7().to_string();
        let now = OffsetDateTime::now_utc();
        let preset = draft.into_preset(id.clone(), now);

        self.connection
            .execute(
                "INSERT INTO presets (id, profile_id, name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    &preset.id,
                    &preset.profile_id,
                    &preset.name,
                    preset
                        .created_at
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(StoreError::Time)?,
                    preset
                        .updated_at
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(StoreError::Time)?,
                ],
            )
            .map_err(StoreError::Sql)?;

        self.replace_preset_properties(&id, &preset, now)?;
        Ok(preset)
    }

    fn list_presets(&self, profile_id: &str) -> Result<Vec<PresetSummary>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, profile_id, name, updated_at
                 FROM presets
                 WHERE profile_id = ?1
                 ORDER BY name COLLATE NOCASE",
            )
            .map_err(StoreError::Sql)?;

        let rows = statement
            .query_map(params![profile_id], |row| {
                Ok(PresetSummary {
                    id: row.get(0)?,
                    profile_id: row.get(1)?,
                    name: row.get(2)?,
                    updated_at: OffsetDateTime::parse(
                        &row.get::<_, String>(3)?,
                        &time::format_description::well_known::Rfc3339,
                    )
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
                })
            })
            .map_err(StoreError::Sql)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sql)
    }

    fn get_preset(&self, preset_id: &str) -> Result<Option<Preset>, StoreError> {
        let preset_row = self
            .connection
            .query_row(
                "SELECT id, profile_id, name, created_at, updated_at
                 FROM presets WHERE id = ?1",
                params![preset_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(StoreError::Sql)?;

        let Some((id, profile_id, name, created_at, updated_at)) = preset_row else {
            return Ok(None);
        };

        let mut statement = self
            .connection
            .prepare(
                "SELECT property_key, property_value_json FROM preset_properties WHERE preset_id = ?1",
            )
            .map_err(StoreError::Sql)?;
        let mut rows = statement
            .query(params![preset_id])
            .map_err(StoreError::Sql)?;

        let mut screen_mode = None;
        let mut use_multimon = None;
        let mut selected_monitors = None;
        let mut redirect_clipboard = None;
        let mut gateway_hostname = None;
        let mut gateway_usage = None;
        let mut security_mode = None;

        while let Some(row) = rows.next().map_err(StoreError::Sql)? {
            let key: String = row.get(0).map_err(StoreError::Sql)?;
            let value_json: String = row.get(1).map_err(StoreError::Sql)?;
            match key.as_str() {
                "screen mode id" => {
                    let value: i64 = serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    screen_mode = Some(if value == 2 {
                        ScreenMode::Fullscreen
                    } else {
                        ScreenMode::Windowed
                    });
                }
                "use multimon" => {
                    use_multimon =
                        Some(serde_json::from_str(&value_json).map_err(StoreError::Json)?)
                }
                "selectedmonitors" => {
                    selected_monitors =
                        Some(serde_json::from_str(&value_json).map_err(StoreError::Json)?)
                }
                "redirectclipboard" => {
                    redirect_clipboard =
                        Some(serde_json::from_str(&value_json).map_err(StoreError::Json)?)
                }
                "gatewayhostname" => {
                    gateway_hostname =
                        Some(serde_json::from_str(&value_json).map_err(StoreError::Json)?)
                }
                "gatewayusagemethod" => {
                    let value: i64 = serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    gateway_usage = Some(match value {
                        1 => GatewayUsageMode::Always,
                        2 => GatewayUsageMode::Detect,
                        4 => GatewayUsageMode::Default,
                        _ => GatewayUsageMode::Never,
                    });
                }
                "enablerdsaadauth" => {
                    let enabled: bool =
                        serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    if enabled {
                        security_mode = Some(SecurityMode::RemoteGuard);
                    } else if security_mode.is_none() {
                        security_mode = Some(SecurityMode::Default);
                    }
                }
                "restricted admin mode" => {
                    let value: i64 = serde_json::from_str(&value_json).map_err(StoreError::Json)?;
                    if value == 1 {
                        security_mode = Some(SecurityMode::RestrictedAdmin);
                    } else if security_mode.is_none() {
                        security_mode = Some(SecurityMode::Default);
                    }
                }
                _ => {}
            }
        }

        Ok(Some(Preset {
            id,
            profile_id,
            name,
            screen_mode,
            use_multimon,
            selected_monitors,
            redirect_clipboard,
            gateway_hostname,
            gateway_usage,
            security_mode,
            created_at: OffsetDateTime::parse(
                &created_at,
                &time::format_description::well_known::Rfc3339,
            )
            .map_err(StoreError::TimeParse)?,
            updated_at: OffsetDateTime::parse(
                &updated_at,
                &time::format_description::well_known::Rfc3339,
            )
            .map_err(StoreError::TimeParse)?,
        }))
    }

    fn insert_session_history(
        &self,
        entry: NewSessionHistoryEntry,
    ) -> Result<SessionHistoryEntry, StoreError> {
        let launch_id = Uuid::now_v7().to_string();
        self.connection
            .execute(
                "INSERT INTO session_history
                (launch_id, profile_id, profile_name, target, process_id, state, started_at, ended_at, error_message)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    launch_id,
                    entry.profile_id,
                    entry.profile_name,
                    entry.target,
                    entry.process_id.map(i64::from),
                    session_state_name(entry.state),
                    entry.started_at.format(&time::format_description::well_known::Rfc3339).map_err(StoreError::Time)?,
                    entry.ended_at
                        .map(|value| value.format(&time::format_description::well_known::Rfc3339))
                        .transpose()
                        .map_err(StoreError::Time)?,
                    entry.error_message,
                ],
            )
            .map_err(StoreError::Sql)?;

        Ok(SessionHistoryEntry {
            launch_id,
            profile_id: entry.profile_id,
            profile_name: entry.profile_name,
            target: entry.target,
            process_id: entry.process_id,
            state: entry.state,
            started_at: entry.started_at,
            ended_at: entry.ended_at,
            error_message: entry.error_message,
        })
    }

    fn list_session_history(
        &self,
        filter: SessionHistoryFilter,
    ) -> Result<Vec<SessionHistoryEntry>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT launch_id, profile_id, profile_name, target, process_id, state, started_at, ended_at, error_message
             FROM session_history
             ORDER BY started_at DESC
             LIMIT ?1",
        ).map_err(StoreError::Sql)?;

        let rows = statement
            .query_map(params![filter.limit as i64], |row| {
                Ok(SessionHistoryEntry {
                    launch_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    profile_name: row.get(2)?,
                    target: row.get(3)?,
                    process_id: row.get::<_, Option<i64>>(4)?.map(|value| value as u32),
                    state: parse_session_state(&row.get::<_, String>(5)?),
                    started_at: OffsetDateTime::parse(
                        &row.get::<_, String>(6)?,
                        &time::format_description::well_known::Rfc3339,
                    )
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
                    ended_at: row
                        .get::<_, Option<String>>(7)?
                        .map(|value| {
                            OffsetDateTime::parse(
                                &value,
                                &time::format_description::well_known::Rfc3339,
                            )
                        })
                        .transpose()
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                    error_message: row.get(8)?,
                })
            })
            .map_err(StoreError::Sql)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sql)
    }

    fn update_session_history(
        &self,
        launch_id: &str,
        update: SessionHistoryUpdate,
    ) -> Result<(), StoreError> {
        self.connection
            .execute(
                "UPDATE session_history
                 SET state = ?2, ended_at = ?3, error_message = ?4
                 WHERE launch_id = ?1",
                params![
                    launch_id,
                    session_state_name(update.state),
                    update
                        .ended_at
                        .map(|value| value.format(&time::format_description::well_known::Rfc3339))
                        .transpose()
                        .map_err(StoreError::Time)?,
                    update.error_message,
                ],
            )
            .map_err(StoreError::Sql)?;

        Ok(())
    }
}

fn parse_session_state(value: &str) -> SessionState {
    match value {
        "launching" => SessionState::Launching,
        "active" => SessionState::Active,
        "failed" => SessionState::Failed,
        _ => SessionState::Exited,
    }
}

fn parse_prompt_behavior(value: &str) -> PromptBehavior {
    match value {
        "helper" => PromptBehavior::Helper,
        _ => PromptBehavior::Prompt,
    }
}

fn prompt_behavior_name(value: PromptBehavior) -> &'static str {
    match value {
        PromptBehavior::Prompt => "prompt",
        PromptBehavior::Helper => "helper",
    }
}

fn session_state_name(state: SessionState) -> &'static str {
    match state {
        SessionState::Launching => "launching",
        SessionState::Active => "active",
        SessionState::Exited => "exited",
        SessionState::Failed => "failed",
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sql(rusqlite::Error),
    #[error("json error: {0}")]
    Json(serde_json::Error),
    #[error("time format error: {0}")]
    Time(time::error::Format),
    #[error("time parse error: {0}")]
    TimeParse(time::error::Parse),
    #[error("failed to create app directory: {0}")]
    CreateDir(std::io::Error),
    #[error("missing profile `{0}`")]
    MissingProfile(ProfileId),
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::launch::{LaunchContext, LaunchIntent, LaunchPlanner, LaunchPolicy};
    use crate::preset::PresetDraft;
    use crate::profile::{
        GatewayUsageMode, ProfileDraft, PromptBehavior, ScreenMode, SecurityMode,
    };
    use crate::rdp::RdpSerializer;
    use crate::registry::PropertyRegistry;

    use super::{NewSessionHistoryEntry, ProfileStore, SessionHistoryFilter, SqliteStore};
    use crate::helper::{HelperResolve, ResolveCredentials, ResolveResult};
    use crate::session::SessionState;

    fn sample_draft() -> ProfileDraft {
        ProfileDraft {
            name: "SQL Cluster - Primary".to_owned(),
            full_address: "sql-pri-01.corp.example".to_owned(),
            username: Some("CONTOSO\\admin-user".to_owned()),
            screen_mode: ScreenMode::Fullscreen,
            use_multimon: true,
            selected_monitors: Some("1,2".to_owned()),
            redirect_clipboard: true,
            gateway_hostname: Some("gateway.corp.example".to_owned()),
            gateway_usage: GatewayUsageMode::Always,
            prompt_behavior: PromptBehavior::Helper,
            allow_windows_credential_bridge: true,
            security_mode: SecurityMode::RemoteGuard,
        }
    }

    #[test]
    fn sqlite_store_round_trips_profile_and_history() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store.save_profile(sample_draft()).expect("profile saved");
        let loaded = store
            .get_profile(&profile.id)
            .expect("load profile")
            .expect("profile exists");

        assert_eq!(loaded.full_address, "sql-pri-01.corp.example");
        assert_eq!(loaded.gateway_usage, GatewayUsageMode::Always);

        let history = store
            .insert_session_history(NewSessionHistoryEntry {
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                target: profile.full_address.clone(),
                process_id: Some(14444),
                state: SessionState::Active,
                started_at: OffsetDateTime::now_utc(),
                ended_at: None,
                error_message: None,
            })
            .expect("insert history");

        let all = store
            .list_session_history(SessionHistoryFilter { limit: 5 })
            .expect("list history");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].launch_id, history.launch_id);
    }

    #[test]
    fn sqlite_store_deletes_profile_and_cascades_related_rows() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store.save_profile(sample_draft()).expect("profile saved");
        store
            .insert_session_history(NewSessionHistoryEntry {
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                target: profile.full_address.clone(),
                process_id: Some(14444),
                state: SessionState::Exited,
                started_at: OffsetDateTime::now_utc(),
                ended_at: Some(OffsetDateTime::now_utc()),
                error_message: None,
            })
            .expect("insert history");

        store.delete_profile(&profile.id).expect("delete profile");

        assert!(
            store
                .get_profile(&profile.id)
                .expect("load profile")
                .is_none()
        );
        assert!(store.list_profiles().expect("list profiles").is_empty());
        assert!(
            store
                .list_session_history(SessionHistoryFilter { limit: 5 })
                .expect("history")
                .is_empty()
        );
    }

    #[test]
    fn launch_plan_supports_helper_resolution_and_rdp_serialization() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store.save_profile(sample_draft()).expect("profile saved");
        let preset = store
            .save_preset(PresetDraft {
                profile_id: profile.id.clone(),
                name: "Admin".to_owned(),
                screen_mode: Some(ScreenMode::Windowed),
                use_multimon: Some(false),
                selected_monitors: None,
                redirect_clipboard: Some(false),
                gateway_hostname: None,
                gateway_usage: Some(GatewayUsageMode::Detect),
                security_mode: Some(SecurityMode::RestrictedAdmin),
            })
            .expect("preset saved");
        let planner = LaunchPlanner::new(PropertyRegistry::new());
        let outcome = planner
            .plan(
                LaunchIntent {
                    profile,
                    preset: Some(preset),
                    policy: LaunchPolicy {
                        allow_prompt: true,
                        allow_helper: true,
                        allow_windows_credential_bridge: true,
                    },
                    context: LaunchContext {
                        surface: "cli".to_owned(),
                        reason: "user_launch".to_owned(),
                    },
                },
                Some(HelperResolve {
                    result: ResolveResult::Resolved,
                    credentials: Some(ResolveCredentials {
                        username: Some("CONTOSO\\admin-user".to_owned()),
                        password: Some("secret".to_owned()),
                        domain: Some("CONTOSO".to_owned()),
                    }),
                    ttl_seconds: Some(60),
                    display_message: Some("Resolved from test helper".to_owned()),
                }),
            )
            .expect("plan");

        let serialized = RdpSerializer::new(PropertyRegistry::new())
            .serialize(&outcome.plan)
            .expect("serialize");

        assert!(
            serialized
                .text
                .contains("full address:s:sql-pri-01.corp.example")
        );
        assert!(serialized.text.contains("gatewayusagemethod:i:2"));
        assert!(serialized.text.contains("use multimon:i:0"));
        assert!(serialized.text.contains("restricted admin mode:i:1"));
    }

    #[test]
    fn helper_prompt_fallback_requires_policy_permission() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store.save_profile(sample_draft()).expect("profile saved");
        let planner = LaunchPlanner::new(PropertyRegistry::new());

        let error = planner
            .plan(
                LaunchIntent {
                    profile,
                    preset: None,
                    policy: LaunchPolicy {
                        allow_prompt: false,
                        allow_helper: true,
                        allow_windows_credential_bridge: true,
                    },
                    context: LaunchContext {
                        surface: "desktop".to_owned(),
                        reason: "user_launch".to_owned(),
                    },
                },
                Some(HelperResolve {
                    result: ResolveResult::Prompt,
                    credentials: None,
                    ttl_seconds: None,
                    display_message: None,
                }),
            )
            .expect_err("prompt fallback should fail");

        assert!(error.to_string().contains("prompt fallback"));
    }

    #[test]
    fn sqlite_store_round_trips_presets() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store.save_profile(sample_draft()).expect("profile saved");
        let preset = store
            .save_preset(PresetDraft {
                profile_id: profile.id.clone(),
                name: "Low bandwidth".to_owned(),
                screen_mode: Some(ScreenMode::Windowed),
                use_multimon: Some(false),
                selected_monitors: None,
                redirect_clipboard: Some(false),
                gateway_hostname: None,
                gateway_usage: Some(GatewayUsageMode::Never),
                security_mode: Some(SecurityMode::Default),
            })
            .expect("preset saved");

        let listed = store.list_presets(&profile.id).expect("list presets");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, preset.id);

        let loaded = store
            .get_preset(&preset.id)
            .expect("load preset")
            .expect("preset exists");
        assert_eq!(loaded.redirect_clipboard, Some(false));
    }
}
