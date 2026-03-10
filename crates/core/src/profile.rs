use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::registry::PropertyValue;

pub type ProfileId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenMode {
    Windowed,
    Fullscreen,
}

impl ScreenMode {
    pub const fn as_rdp_value(self) -> i64 {
        match self {
            Self::Windowed => 1,
            Self::Fullscreen => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayUsageMode {
    Never,
    Always,
    Detect,
    Default,
}

impl GatewayUsageMode {
    pub const fn as_rdp_value(self) -> i64 {
        match self {
            Self::Never => 0,
            Self::Always => 1,
            Self::Detect => 2,
            Self::Default => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptBehavior {
    Prompt,
    Helper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    Default,
    RemoteGuard,
    RestrictedAdmin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub full_address: String,
    pub username: Option<String>,
    pub screen_mode: ScreenMode,
    pub use_multimon: bool,
    pub selected_monitors: Option<String>,
    pub redirect_clipboard: bool,
    pub gateway_hostname: Option<String>,
    pub gateway_usage: GatewayUsageMode,
    pub prompt_behavior: PromptBehavior,
    pub allow_windows_credential_bridge: bool,
    pub security_mode: SecurityMode,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Profile {
    pub fn property_pairs(&self) -> Vec<(&'static str, PropertyValue)> {
        let mut properties = vec![
            (
                "full address",
                PropertyValue::String(self.full_address.clone()),
            ),
            (
                "screen mode id",
                PropertyValue::Integer(self.screen_mode.as_rdp_value()),
            ),
            ("use multimon", PropertyValue::Bool(self.use_multimon)),
            (
                "redirectclipboard",
                PropertyValue::Bool(self.redirect_clipboard),
            ),
            (
                "gatewayusagemethod",
                PropertyValue::Integer(self.gateway_usage.as_rdp_value()),
            ),
        ];

        if let Some(username) = &self.username {
            properties.push(("username", PropertyValue::String(username.clone())));
        }

        if let Some(selected_monitors) = &self.selected_monitors {
            properties.push((
                "selectedmonitors",
                PropertyValue::String(selected_monitors.clone()),
            ));
        }

        if let Some(gateway_hostname) = &self.gateway_hostname {
            properties.push((
                "gatewayhostname",
                PropertyValue::String(gateway_hostname.clone()),
            ));
        }

        match self.security_mode {
            SecurityMode::Default => {}
            SecurityMode::RemoteGuard => {
                properties.push(("enablerdsaadauth", PropertyValue::Bool(true)));
            }
            SecurityMode::RestrictedAdmin => {
                properties.push(("restricted admin mode", PropertyValue::Integer(1)));
            }
        }

        properties
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDraft {
    pub name: String,
    pub full_address: String,
    pub username: Option<String>,
    pub screen_mode: ScreenMode,
    pub use_multimon: bool,
    pub selected_monitors: Option<String>,
    pub redirect_clipboard: bool,
    pub gateway_hostname: Option<String>,
    pub gateway_usage: GatewayUsageMode,
    pub prompt_behavior: PromptBehavior,
    pub allow_windows_credential_bridge: bool,
    pub security_mode: SecurityMode,
}

impl ProfileDraft {
    pub fn into_profile(self, id: ProfileId, now: OffsetDateTime) -> Profile {
        Profile {
            id,
            name: self.name,
            full_address: self.full_address,
            username: self.username,
            screen_mode: self.screen_mode,
            use_multimon: self.use_multimon,
            selected_monitors: self.selected_monitors,
            redirect_clipboard: self.redirect_clipboard,
            gateway_hostname: self.gateway_hostname,
            gateway_usage: self.gateway_usage,
            prompt_behavior: self.prompt_behavior,
            allow_windows_credential_bridge: self.allow_windows_credential_bridge,
            security_mode: self.security_mode,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: ProfileId,
    pub name: String,
    pub full_address: String,
    pub username: Option<String>,
    pub security_mode: SecurityMode,
    pub last_used_at: Option<OffsetDateTime>,
}
