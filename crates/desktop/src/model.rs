#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use rdp_launch_core::{
    GatewayUsageMode, ObservedSession, PresetSummary, Profile, ProfileDraft, ProfileStore,
    ProfileSummary, PromptBehavior, ScreenMode, SecurityMode,
};
use rdp_launch_windows::{SessionTracker, SessionTrackerError};
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    None,
    Profile(usize),
    Session(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeSurface {
    Closed,
    New,
    Edit(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileForm {
    pub display_name: String,
    pub hostname: String,
    pub username: String,
    pub domain: String,
    pub screen_mode: ScreenMode,
    pub use_multimon: bool,
    pub selected_monitors: String,
    pub redirect_clipboard: bool,
    pub gateway_hostname: String,
    pub gateway_usage: GatewayUsageMode,
    pub security_mode: SecurityMode,
}

impl Default for ProfileForm {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            hostname: String::new(),
            username: String::new(),
            domain: String::new(),
            screen_mode: ScreenMode::Windowed,
            use_multimon: false,
            selected_monitors: String::new(),
            redirect_clipboard: true,
            gateway_hostname: String::new(),
            gateway_usage: GatewayUsageMode::Never,
            security_mode: SecurityMode::Default,
        }
    }
}

impl ProfileForm {
    pub fn from_profile(profile: &Profile) -> Self {
        let (username, domain) =
            split_username_and_domain(&profile.full_address, profile.username.as_deref());
        Self {
            display_name: profile.name.clone(),
            hostname: profile.full_address.clone(),
            username,
            domain,
            screen_mode: profile.screen_mode,
            use_multimon: profile.use_multimon,
            selected_monitors: profile.selected_monitors.clone().unwrap_or_default(),
            redirect_clipboard: profile.redirect_clipboard,
            gateway_hostname: profile.gateway_hostname.clone().unwrap_or_default(),
            gateway_usage: profile.gateway_usage,
            security_mode: profile.security_mode,
        }
    }

    pub fn to_draft(&self) -> ProfileDraft {
        ProfileDraft {
            name: self.display_name.trim().to_owned(),
            full_address: self.hostname.trim().to_owned(),
            username: normalize_username(&self.hostname, &self.username, &self.domain),
            screen_mode: self.screen_mode,
            use_multimon: self.use_multimon,
            selected_monitors: optional_string(&self.selected_monitors),
            redirect_clipboard: self.redirect_clipboard,
            gateway_hostname: optional_string(&self.gateway_hostname),
            gateway_usage: self.gateway_usage,
            prompt_behavior: PromptBehavior::Prompt,
            allow_windows_credential_bridge: false,
            security_mode: self.security_mode,
        }
    }

    pub fn apply_username_input(&mut self, input: &str) {
        let trimmed = input.trim();
        if let Some((username, domain)) = split_inline_username_input(&self.hostname, trimmed) {
            self.username = username;
            self.domain = domain;
        } else {
            self.username = trimmed.to_owned();
        }
    }

    pub fn apply_domain_input(&mut self, input: &str) {
        self.domain = normalize_domain_input(&self.hostname, input);
    }
}

#[derive(Debug, Clone)]
pub struct HomeViewModel {
    pub search: String,
    pub profiles: Vec<ProfileSummary>,
    pub sessions: Vec<ObservedSession>,
    pub presets: Vec<PresetSummary>,
    pub selected_preset_id: Option<String>,
    pub selection: Selection,
    pub compose: ComposeSurface,
    pub profile_form: ProfileForm,
}

impl HomeViewModel {
    pub fn load<S: ProfileStore, T: SessionTracker>(
        store: &S,
        tracker: &T,
    ) -> Result<Self, HomeViewModelError> {
        let profiles = store.list_profiles()?;
        let sessions = tracker.active_sessions(store)?;
        let selection = if sessions.is_empty() && profiles.is_empty() {
            Selection::None
        } else if !sessions.is_empty() {
            Selection::Session(0)
        } else {
            Selection::Profile(0)
        };
        let (presets, selected_preset_id) = match selection {
            Selection::Session(index) => {
                let presets = store.list_presets(&sessions[index].profile_id)?;
                let selected = presets.first().map(|preset| preset.id.clone());
                (presets, selected)
            }
            Selection::Profile(index) => {
                let presets = store.list_presets(&profiles[index].id)?;
                let selected = presets.first().map(|preset| preset.id.clone());
                (presets, selected)
            }
            _ => (Vec::new(), None),
        };

        Ok(Self {
            search: String::new(),
            profiles,
            sessions,
            presets,
            selected_preset_id,
            selection,
            compose: ComposeSurface::Closed,
            profile_form: ProfileForm::default(),
        })
    }

    pub fn filtered_profiles(&self) -> Vec<(usize, &ProfileSummary)> {
        let search = self.search.trim().to_ascii_lowercase();
        self.profiles
            .iter()
            .enumerate()
            .filter(|(_, profile)| {
                search.is_empty()
                    || profile.name.to_ascii_lowercase().contains(&search)
                    || profile.full_address.to_ascii_lowercase().contains(&search)
            })
            .collect()
    }

    pub fn filtered_sessions(&self) -> Vec<(usize, &ObservedSession)> {
        let search = self.search.trim().to_ascii_lowercase();
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                search.is_empty()
                    || session.profile_name.to_ascii_lowercase().contains(&search)
                    || session.target.to_ascii_lowercase().contains(&search)
            })
            .collect()
    }

    pub fn begin_new_profile(&mut self) {
        self.profile_form = ProfileForm::default();
        self.compose = ComposeSurface::New;
    }

    pub fn begin_edit_profile<S: ProfileStore>(
        &mut self,
        store: &S,
        index: usize,
    ) -> Result<(), HomeViewModelError> {
        if let Some(profile) = self.profiles.get(index) {
            let full_profile = store
                .get_profile(&profile.id)?
                .ok_or_else(|| HomeViewModelError::MissingProfile(profile.id.clone()))?;
            self.profile_form = ProfileForm::from_profile(&full_profile);
            self.presets = store.list_presets(&profile.id)?;
            self.selected_preset_id = None;
            self.selection = Selection::Profile(index);
            self.compose = ComposeSurface::Edit(index);
        }
        Ok(())
    }

    pub fn select_profile<S: ProfileStore>(
        &mut self,
        store: &S,
        index: usize,
    ) -> Result<(), HomeViewModelError> {
        if let Some(profile) = self.profiles.get(index) {
            self.presets = store.list_presets(&profile.id)?;
            self.selected_preset_id = self.presets.first().map(|preset| preset.id.clone());
        }
        self.selection = Selection::Profile(index);
        self.compose = ComposeSurface::Closed;
        Ok(())
    }

    pub fn select_session<S: ProfileStore>(
        &mut self,
        store: &S,
        index: usize,
    ) -> Result<(), HomeViewModelError> {
        if let Some(session) = self.sessions.get(index) {
            self.presets = store.list_presets(&session.profile_id)?;
            self.selected_preset_id = self.presets.first().map(|preset| preset.id.clone());
        }
        self.selection = Selection::Session(index);
        self.compose = ComposeSurface::Closed;
        Ok(())
    }

    pub fn set_selected_preset(&mut self, preset_id: Option<String>) {
        self.selected_preset_id = preset_id;
    }

    pub fn close_compose(&mut self) {
        self.compose = ComposeSurface::Closed;
    }

    pub fn select_profile_by_id<S: ProfileStore>(
        &mut self,
        store: &S,
        profile_id: &str,
    ) -> Result<(), HomeViewModelError> {
        if let Some(index) = self
            .profiles
            .iter()
            .position(|profile| profile.id == profile_id)
        {
            self.select_profile(store, index)?;
        }
        Ok(())
    }
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn split_username_and_domain(hostname: &str, username: Option<&str>) -> (String, String) {
    let Some(raw_username) = username.map(str::trim).filter(|value| !value.is_empty()) else {
        return (String::new(), String::new());
    };

    if let Some((username, domain)) = split_upn_username(raw_username) {
        return (username, domain);
    }

    if let Some((domain, username)) = raw_username.split_once('\\') {
        return (
            username.trim().to_owned(),
            normalize_domain_input(hostname, domain),
        );
    }

    (raw_username.to_owned(), String::new())
}

fn split_inline_username_input(hostname: &str, input: &str) -> Option<(String, String)> {
    if let Some((username, domain)) = split_upn_username(input) {
        return Some((username, domain));
    }

    let (domain, username) = input.split_once('\\')?;
    Some((
        username.trim().to_owned(),
        normalize_domain_input(hostname, domain),
    ))
}

fn split_upn_username(input: &str) -> Option<(String, String)> {
    let (username, domain) = input.split_once('@')?;
    let username = username.trim();
    let domain = domain.trim();
    if username.is_empty() || domain.is_empty() {
        return None;
    }

    Some((username.to_owned(), domain.to_owned()))
}

fn normalize_domain_input(hostname: &str, input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let trimmed = trimmed.trim_end_matches('\\');
    if trimmed == "." {
        hostname.trim().to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn normalize_username(hostname: &str, username: &str, domain: &str) -> Option<String> {
    let username = username.trim();
    if username.is_empty() {
        return None;
    }

    if let Some((username, domain)) = split_inline_username_input(hostname, username) {
        if domain.is_empty() {
            return Some(username);
        }
        return Some(format!("{domain}\\{username}"));
    }

    let domain = normalize_domain_input(hostname, domain);
    if domain.is_empty() {
        Some(username.to_owned())
    } else {
        Some(format!("{domain}\\{username}"))
    }
}

#[derive(Debug, Error)]
pub enum HomeViewModelError {
    #[error("{0}")]
    Store(#[from] rdp_launch_core::StoreError),
    #[error("{0}")]
    SessionTracker(#[from] SessionTrackerError),
    #[error("missing profile `{0}`")]
    MissingProfile(String),
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use rdp_launch_core::{
        NewSessionHistoryEntry, ProfileDraft, PromptBehavior, ScreenMode, SecurityMode,
        SessionState, SqliteStore,
    };
    use rdp_launch_windows::ProcessSessionTracker;

    use super::*;

    #[test]
    fn home_model_prefers_active_session_selection() {
        let store = SqliteStore::open_in_memory().expect("store");
        let profile = store
            .save_profile(ProfileDraft {
                name: "SQL Cluster - Primary".to_owned(),
                full_address: "sql-pri-01.corp.example".to_owned(),
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
                target: "sql-pri-01.corp.example".to_owned(),
                process_id: Some(std::process::id()),
                state: SessionState::Active,
                started_at: OffsetDateTime::now_utc(),
                ended_at: None,
                error_message: None,
            })
            .expect("history");

        let model = HomeViewModel::load(&store, &ProcessSessionTracker).expect("model");
        assert_eq!(model.selection, Selection::Session(0));
        assert_eq!(model.filtered_sessions().len(), 1);
    }

    #[test]
    fn home_model_filters_connections_by_search() {
        let store = SqliteStore::open_in_memory().expect("store");
        store
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
            .save_profile(ProfileDraft {
                name: "Build Agent".to_owned(),
                full_address: "build-03.lab.example".to_owned(),
                username: None,
                screen_mode: ScreenMode::Windowed,
                use_multimon: false,
                selected_monitors: None,
                redirect_clipboard: true,
                gateway_hostname: None,
                gateway_usage: GatewayUsageMode::Never,
                prompt_behavior: PromptBehavior::Helper,
                allow_windows_credential_bridge: true,
                security_mode: SecurityMode::RestrictedAdmin,
            })
            .expect("profile");

        let mut model = HomeViewModel::load(&store, &ProcessSessionTracker).expect("model");
        model.search = "build".to_owned();

        let filtered = model.filtered_profiles();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1.name, "Build Agent");
    }

    #[test]
    fn empty_home_model_starts_without_compose_surface() {
        let store = SqliteStore::open_in_memory().expect("store");

        let model = HomeViewModel::load(&store, &ProcessSessionTracker).expect("model");
        assert_eq!(model.selection, Selection::None);
        assert_eq!(model.compose, ComposeSurface::Closed);
    }

    #[test]
    fn begin_new_profile_opens_compose_surface_without_changing_selection() {
        let store = SqliteStore::open_in_memory().expect("store");
        store
            .save_profile(ProfileDraft {
                name: "Build Agent".to_owned(),
                full_address: "build-03.lab.example".to_owned(),
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

        let mut model = HomeViewModel::load(&store, &ProcessSessionTracker).expect("model");
        assert_eq!(model.selection, Selection::Profile(0));

        model.begin_new_profile();

        assert_eq!(model.selection, Selection::Profile(0));
        assert_eq!(model.compose, ComposeSurface::New);
    }

    #[test]
    fn begin_edit_profile_opens_compose_surface_and_keeps_profile_selected() {
        let store = SqliteStore::open_in_memory().expect("store");
        store
            .save_profile(ProfileDraft {
                name: "Build Agent".to_owned(),
                full_address: "build-03.lab.example".to_owned(),
                username: Some("operator".to_owned()),
                screen_mode: ScreenMode::Fullscreen,
                use_multimon: true,
                selected_monitors: Some("1,2".to_owned()),
                redirect_clipboard: true,
                gateway_hostname: None,
                gateway_usage: GatewayUsageMode::Never,
                prompt_behavior: PromptBehavior::Prompt,
                allow_windows_credential_bridge: false,
                security_mode: SecurityMode::Default,
            })
            .expect("profile");

        let mut model = HomeViewModel::load(&store, &ProcessSessionTracker).expect("model");
        model.begin_edit_profile(&store, 0).expect("edit profile");

        assert_eq!(model.selection, Selection::Profile(0));
        assert_eq!(model.compose, ComposeSurface::Edit(0));
        assert_eq!(model.profile_form.display_name, "Build Agent");
        assert_eq!(model.profile_form.hostname, "build-03.lab.example");
        assert_eq!(model.profile_form.username, "operator");
    }

    #[test]
    fn profile_form_splits_domain_qualified_username_for_editing() {
        let profile = Profile {
            id: "profile-1".to_owned(),
            name: "Domain Joined".to_owned(),
            full_address: "jumpbox01".to_owned(),
            username: Some("CONTOSO\\alice".to_owned()),
            screen_mode: ScreenMode::Windowed,
            use_multimon: false,
            selected_monitors: None,
            redirect_clipboard: true,
            gateway_hostname: None,
            gateway_usage: GatewayUsageMode::Never,
            prompt_behavior: PromptBehavior::Prompt,
            allow_windows_credential_bridge: false,
            security_mode: SecurityMode::Default,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        let form = ProfileForm::from_profile(&profile);
        assert_eq!(form.display_name, "Domain Joined");
        assert_eq!(form.hostname, "jumpbox01");
        assert_eq!(form.username, "alice");
        assert_eq!(form.domain, "CONTOSO");
    }

    #[test]
    fn profile_form_normalizes_local_machine_domain_to_hostname() {
        let mut form = ProfileForm {
            display_name: "Local Admin".to_owned(),
            hostname: "rdp-host-01".to_owned(),
            username: String::new(),
            domain: String::new(),
            ..ProfileForm::default()
        };

        form.apply_username_input(".\\alice");
        let draft = form.to_draft();

        assert_eq!(form.username, "alice");
        assert_eq!(form.domain, "rdp-host-01");
        assert_eq!(draft.username.as_deref(), Some("rdp-host-01\\alice"));
        assert_eq!(draft.full_address, "rdp-host-01");
        assert_eq!(draft.name, "Local Admin");
    }
}
