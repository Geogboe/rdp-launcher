use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::{GatewayUsageMode, ScreenMode, SecurityMode};
use crate::registry::PropertyValue;

pub type PresetId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub id: PresetId,
    pub profile_id: String,
    pub name: String,
    pub screen_mode: Option<ScreenMode>,
    pub use_multimon: Option<bool>,
    pub selected_monitors: Option<String>,
    pub redirect_clipboard: Option<bool>,
    pub gateway_hostname: Option<String>,
    pub gateway_usage: Option<GatewayUsageMode>,
    pub security_mode: Option<SecurityMode>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Preset {
    pub fn property_pairs(&self) -> Vec<(&'static str, PropertyValue)> {
        let mut properties = Vec::new();

        if let Some(screen_mode) = self.screen_mode {
            properties.push((
                "screen mode id",
                PropertyValue::Integer(screen_mode.as_rdp_value()),
            ));
        }
        if let Some(use_multimon) = self.use_multimon {
            properties.push(("use multimon", PropertyValue::Bool(use_multimon)));
        }
        if let Some(selected_monitors) = &self.selected_monitors {
            properties.push((
                "selectedmonitors",
                PropertyValue::String(selected_monitors.clone()),
            ));
        }
        if let Some(redirect_clipboard) = self.redirect_clipboard {
            properties.push(("redirectclipboard", PropertyValue::Bool(redirect_clipboard)));
        }
        if let Some(gateway_hostname) = &self.gateway_hostname {
            properties.push((
                "gatewayhostname",
                PropertyValue::String(gateway_hostname.clone()),
            ));
        }
        if let Some(gateway_usage) = self.gateway_usage {
            properties.push((
                "gatewayusagemethod",
                PropertyValue::Integer(gateway_usage.as_rdp_value()),
            ));
        }
        match self.security_mode {
            Some(SecurityMode::RemoteGuard) => {
                properties.push(("enablerdsaadauth", PropertyValue::Bool(true)));
                properties.push(("restricted admin mode", PropertyValue::Integer(0)));
            }
            Some(SecurityMode::RestrictedAdmin) => {
                properties.push(("restricted admin mode", PropertyValue::Integer(1)));
                properties.push(("enablerdsaadauth", PropertyValue::Bool(false)));
            }
            Some(SecurityMode::Default) => {
                properties.push(("restricted admin mode", PropertyValue::Integer(0)));
                properties.push(("enablerdsaadauth", PropertyValue::Bool(false)));
            }
            None => {}
        }

        properties
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetDraft {
    pub profile_id: String,
    pub name: String,
    pub screen_mode: Option<ScreenMode>,
    pub use_multimon: Option<bool>,
    pub selected_monitors: Option<String>,
    pub redirect_clipboard: Option<bool>,
    pub gateway_hostname: Option<String>,
    pub gateway_usage: Option<GatewayUsageMode>,
    pub security_mode: Option<SecurityMode>,
}

impl PresetDraft {
    pub fn into_preset(self, id: PresetId, now: OffsetDateTime) -> Preset {
        Preset {
            id,
            profile_id: self.profile_id,
            name: self.name,
            screen_mode: self.screen_mode,
            use_multimon: self.use_multimon,
            selected_monitors: self.selected_monitors,
            redirect_clipboard: self.redirect_clipboard,
            gateway_hostname: self.gateway_hostname,
            gateway_usage: self.gateway_usage,
            security_mode: self.security_mode,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetSummary {
    pub id: PresetId,
    pub profile_id: String,
    pub name: String,
    pub updated_at: OffsetDateTime,
}
