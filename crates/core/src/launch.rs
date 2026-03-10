use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::helper::{HelperResolve, ResolveResult};
use crate::preset::Preset;
use crate::profile::{Profile, PromptBehavior, SecurityMode};
use crate::registry::{PropertyRegistry, PropertyRegistryError, PropertyValue};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchPolicy {
    pub allow_prompt: bool,
    pub allow_helper: bool,
    pub allow_windows_credential_bridge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchContext {
    pub surface: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchIntent {
    pub profile: Profile,
    pub preset: Option<Preset>,
    pub policy: LaunchPolicy,
    pub context: LaunchContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialFlow {
    PromptOnly,
    HelperPromptFallback,
    HelperResolved {
        username: Option<String>,
        password_present: bool,
        domain: Option<String>,
        windows_bridge: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchPlan {
    pub profile_id: String,
    pub profile_name: String,
    pub preset_id: Option<String>,
    pub preset_name: Option<String>,
    pub target: String,
    pub security_mode: SecurityMode,
    pub credential_flow: CredentialFlow,
    pub properties: Vec<(String, PropertyValue)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchOutcome {
    pub plan: LaunchPlan,
    pub helper_message: Option<String>,
}

pub struct LaunchPlanner {
    registry: PropertyRegistry,
}

impl LaunchPlanner {
    pub const fn new(registry: PropertyRegistry) -> Self {
        Self { registry }
    }

    pub fn plan(
        &self,
        intent: LaunchIntent,
        helper_result: Option<HelperResolve>,
    ) -> Result<LaunchOutcome, LaunchPlanError> {
        let credential_flow = self.resolve_credentials(&intent, helper_result.as_ref())?;
        let mut properties = intent
            .profile
            .property_pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<Vec<_>>();

        if let Some(preset) = &intent.preset {
            for (key, value) in preset.property_pairs() {
                if let Some(existing) = properties
                    .iter_mut()
                    .find(|(existing_key, _)| existing_key == key)
                {
                    existing.1 = value;
                } else {
                    properties.push((key.to_owned(), value));
                }
            }
        }

        let properties = properties
            .into_iter()
            .map(|(key, value)| {
                self.registry.validate(&key, &value)?;
                Ok((key, value))
            })
            .collect::<Result<Vec<_>, PropertyRegistryError>>()?;

        let security_mode = intent
            .preset
            .as_ref()
            .and_then(|preset| preset.security_mode)
            .unwrap_or(intent.profile.security_mode);

        Ok(LaunchOutcome {
            plan: LaunchPlan {
                profile_id: intent.profile.id.clone(),
                profile_name: intent.profile.name.clone(),
                preset_id: intent.preset.as_ref().map(|preset| preset.id.clone()),
                preset_name: intent.preset.as_ref().map(|preset| preset.name.clone()),
                target: intent.profile.full_address.clone(),
                security_mode,
                credential_flow,
                properties,
            },
            helper_message: helper_result.and_then(|result| result.display_message),
        })
    }

    fn resolve_credentials(
        &self,
        intent: &LaunchIntent,
        helper_result: Option<&HelperResolve>,
    ) -> Result<CredentialFlow, LaunchPlanError> {
        match intent.profile.prompt_behavior {
            PromptBehavior::Prompt => Ok(CredentialFlow::PromptOnly),
            PromptBehavior::Helper => {
                if !intent.policy.allow_helper {
                    return Err(LaunchPlanError::HelperNotAllowed);
                }

                let helper_result = helper_result.ok_or(LaunchPlanError::MissingHelperResult)?;
                match helper_result.result {
                    ResolveResult::Resolved => {
                        let credentials = helper_result
                            .credentials
                            .as_ref()
                            .ok_or(LaunchPlanError::MissingCredentials)?;
                        Ok(CredentialFlow::HelperResolved {
                            username: credentials.username.clone(),
                            password_present: credentials.password.is_some(),
                            domain: credentials.domain.clone(),
                            windows_bridge: credentials.password.is_some()
                                && intent.policy.allow_windows_credential_bridge
                                && intent.profile.allow_windows_credential_bridge,
                        })
                    }
                    ResolveResult::Prompt => {
                        if intent.policy.allow_prompt {
                            Ok(CredentialFlow::HelperPromptFallback)
                        } else {
                            Err(LaunchPlanError::PromptFallbackNotAllowed)
                        }
                    }
                    ResolveResult::Cancelled => Err(LaunchPlanError::LaunchCancelled),
                    ResolveResult::Denied => Err(LaunchPlanError::LaunchDenied),
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum LaunchPlanError {
    #[error("property validation failed: {0}")]
    PropertyRegistry(#[from] PropertyRegistryError),
    #[error("helper use is not allowed by policy")]
    HelperNotAllowed,
    #[error("launch required helper credentials but no helper result was provided")]
    MissingHelperResult,
    #[error("helper returned resolved without credentials")]
    MissingCredentials,
    #[error("helper requested prompt fallback but the profile does not allow it")]
    PromptFallbackNotAllowed,
    #[error("launch cancelled by helper")]
    LaunchCancelled,
    #[error("launch denied by helper")]
    LaunchDenied,
}
