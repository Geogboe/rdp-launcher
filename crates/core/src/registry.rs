use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    String,
    Integer,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyScope {
    Connection,
    Display,
    Gateway,
    Security,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropertyValue {
    String(String),
    Integer(i64),
    Bool(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDefinition {
    pub key: &'static str,
    pub wire_type: &'static str,
    pub value_type: PropertyType,
    pub scope: PropertyScope,
    pub sensitive: bool,
}

pub type PropertyKey = &'static str;

pub const SLICE_PROPERTY_DEFINITIONS: [PropertyDefinition; 10] = [
    PropertyDefinition {
        key: "full address",
        wire_type: "s",
        value_type: PropertyType::String,
        scope: PropertyScope::Connection,
        sensitive: false,
    },
    PropertyDefinition {
        key: "username",
        wire_type: "s",
        value_type: PropertyType::String,
        scope: PropertyScope::Connection,
        sensitive: false,
    },
    PropertyDefinition {
        key: "screen mode id",
        wire_type: "i",
        value_type: PropertyType::Integer,
        scope: PropertyScope::Display,
        sensitive: false,
    },
    PropertyDefinition {
        key: "use multimon",
        wire_type: "i",
        value_type: PropertyType::Bool,
        scope: PropertyScope::Display,
        sensitive: false,
    },
    PropertyDefinition {
        key: "selectedmonitors",
        wire_type: "s",
        value_type: PropertyType::String,
        scope: PropertyScope::Display,
        sensitive: false,
    },
    PropertyDefinition {
        key: "redirectclipboard",
        wire_type: "i",
        value_type: PropertyType::Bool,
        scope: PropertyScope::Connection,
        sensitive: false,
    },
    PropertyDefinition {
        key: "gatewayhostname",
        wire_type: "s",
        value_type: PropertyType::String,
        scope: PropertyScope::Gateway,
        sensitive: false,
    },
    PropertyDefinition {
        key: "gatewayusagemethod",
        wire_type: "i",
        value_type: PropertyType::Integer,
        scope: PropertyScope::Gateway,
        sensitive: false,
    },
    PropertyDefinition {
        key: "enablerdsaadauth",
        wire_type: "i",
        value_type: PropertyType::Bool,
        scope: PropertyScope::Security,
        sensitive: false,
    },
    PropertyDefinition {
        key: "restricted admin mode",
        wire_type: "i",
        value_type: PropertyType::Integer,
        scope: PropertyScope::Security,
        sensitive: false,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct PropertyRegistry;

impl Default for PropertyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PropertyRegistry {
    pub const fn new() -> Self {
        Self
    }

    pub fn definitions(self) -> &'static [PropertyDefinition] {
        &SLICE_PROPERTY_DEFINITIONS
    }

    pub fn get(self, key: &str) -> Option<&'static PropertyDefinition> {
        self.definitions()
            .iter()
            .find(|definition| definition.key == key)
    }

    pub fn validate(
        self,
        key: &str,
        value: &PropertyValue,
    ) -> Result<&'static PropertyDefinition, PropertyRegistryError> {
        let definition = self
            .get(key)
            .ok_or_else(|| PropertyRegistryError::UnknownProperty(key.to_owned()))?;

        let matches = matches!(
            (&definition.value_type, value),
            (PropertyType::String, PropertyValue::String(_))
                | (PropertyType::Integer, PropertyValue::Integer(_))
                | (PropertyType::Bool, PropertyValue::Bool(_))
        );

        if matches {
            Ok(definition)
        } else {
            Err(PropertyRegistryError::InvalidType {
                key: key.to_owned(),
                expected: definition.value_type,
                actual: match value {
                    PropertyValue::String(_) => PropertyType::String,
                    PropertyValue::Integer(_) => PropertyType::Integer,
                    PropertyValue::Bool(_) => PropertyType::Bool,
                },
            })
        }
    }
}

#[derive(Debug, Error)]
pub enum PropertyRegistryError {
    #[error("unknown property `{0}`")]
    UnknownProperty(String),
    #[error("property `{key}` expected {expected:?} but received {actual:?}")]
    InvalidType {
        key: String,
        expected: PropertyType,
        actual: PropertyType,
    },
}
