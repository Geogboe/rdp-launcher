#[cfg(target_os = "windows")]
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporaryCredential {
    pub target: String,
    pub username: String,
    pub password: String,
}

pub trait CredentialBridge {
    fn install(&self, credential: &TemporaryCredential) -> Result<(), CredentialBridgeError>;
    fn remove(&self, target: &str) -> Result<(), CredentialBridgeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CmdKeyCredentialBridge;

impl CredentialBridge for CmdKeyCredentialBridge {
    fn install(&self, credential: &TemporaryCredential) -> Result<(), CredentialBridgeError> {
        validate_temporary_credential(credential)?;

        #[cfg(target_os = "windows")]
        {
            let target = format!("TERMSRV/{}", credential.target);
            let status = Command::new("cmdkey")
                .arg(format!("/generic:{target}"))
                .arg(format!("/user:{}", credential.username))
                .arg(format!("/pass:{}", credential.password))
                .status()
                .map_err(CredentialBridgeError::Io)?;

            if status.success() {
                Ok(())
            } else {
                Err(CredentialBridgeError::Command(status.code()))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = credential;
            Err(CredentialBridgeError::UnsupportedPlatform)
        }
    }

    fn remove(&self, target: &str) -> Result<(), CredentialBridgeError> {
        validate_cmdkey_target(target)?;

        #[cfg(target_os = "windows")]
        {
            let status = Command::new("cmdkey")
                .arg(format!("/delete:TERMSRV/{target}"))
                .status()
                .map_err(CredentialBridgeError::Io)?;

            if status.success() {
                Ok(())
            } else {
                Err(CredentialBridgeError::Command(status.code()))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = target;
            Err(CredentialBridgeError::UnsupportedPlatform)
        }
    }
}

fn validate_temporary_credential(
    credential: &TemporaryCredential,
) -> Result<(), CredentialBridgeError> {
    validate_cmdkey_target(&credential.target)?;
    validate_cmdkey_username(&credential.username)
}

fn validate_cmdkey_target(target: &str) -> Result<(), CredentialBridgeError> {
    if target.is_empty() || target.chars().any(char::is_control) {
        return Err(CredentialBridgeError::InvalidTarget(target.to_owned()));
    }

    let (host, port) = match target.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && !host.contains(':') => (host, Some(port)),
        _ => (target, None),
    };

    let host_valid = host
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));
    let port_valid = port
        .map(|value| !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()))
        .unwrap_or(true);

    if host_valid && port_valid {
        Ok(())
    } else {
        Err(CredentialBridgeError::InvalidTarget(target.to_owned()))
    }
}

fn validate_cmdkey_username(username: &str) -> Result<(), CredentialBridgeError> {
    if username.is_empty() || username.chars().any(char::is_control) {
        Err(CredentialBridgeError::InvalidUsername)
    } else {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CredentialBridgeError {
    #[error("credential bridge command failed with exit code {0:?}")]
    Command(Option<i32>),
    #[error("credential bridge i/o error: {0}")]
    Io(std::io::Error),
    #[error("credential target contains unsupported characters: {0}")]
    InvalidTarget(String),
    #[error("credential username contains unsupported characters")]
    InvalidUsername,
    #[error("credential bridge is only available on Windows")]
    UnsupportedPlatform,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_validation_accepts_expected_host_patterns() {
        for target in [
            "build-03.lab.example",
            "192.0.2.10",
            "jumpbox01",
            "build-03.lab.example:3390",
        ] {
            validate_cmdkey_target(target).expect("valid target");
        }
    }

    #[test]
    fn target_validation_rejects_unsupported_characters() {
        for target in [
            "build 03.lab.example",
            "build/03.lab.example",
            "build-03.lab.example:rdp",
            "fe80::1",
        ] {
            assert!(matches!(
                validate_cmdkey_target(target),
                Err(CredentialBridgeError::InvalidTarget(_))
            ));
        }
    }

    #[test]
    fn username_validation_rejects_control_characters() {
        validate_cmdkey_username("CONTOSO\\builder").expect("valid username");
        assert!(matches!(
            validate_cmdkey_username("builder\nadmin"),
            Err(CredentialBridgeError::InvalidUsername)
        ));
    }
}
