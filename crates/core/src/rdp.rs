use std::fmt::Write;

use thiserror::Error;

use crate::launch::LaunchPlan;
use crate::registry::{PropertyRegistry, PropertyValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdpFile {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedRdp {
    pub text: String,
}

pub struct RdpSerializer {
    registry: PropertyRegistry,
}

impl RdpSerializer {
    pub const fn new(registry: PropertyRegistry) -> Self {
        Self { registry }
    }

    pub fn serialize(&self, plan: &LaunchPlan) -> Result<SerializedRdp, RdpSerializeError> {
        let mut lines = Vec::with_capacity(plan.properties.len());

        for (key, value) in &plan.properties {
            let definition = self
                .registry
                .get(key)
                .ok_or_else(|| RdpSerializeError::UnknownProperty(key.to_owned()))?;
            lines.push(format!(
                "{}:{}:{}",
                key,
                definition.wire_type,
                encode_value(value)
            ));
        }

        lines.sort();

        let mut text = String::new();
        for line in lines {
            let _ = writeln!(text, "{line}");
        }

        Ok(SerializedRdp { text })
    }
}

fn encode_value(value: &PropertyValue) -> String {
    match value {
        PropertyValue::String(value) => value.clone(),
        PropertyValue::Integer(value) => value.to_string(),
        PropertyValue::Bool(value) => i64::from(*value).to_string(),
    }
}

#[derive(Debug, Error)]
pub enum RdpSerializeError {
    #[error("unknown property `{0}`")]
    UnknownProperty(String),
}
