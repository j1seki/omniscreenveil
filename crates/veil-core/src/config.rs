use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::rules::Profile;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read profile at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse profile at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not write profile at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize profile: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Versioned JSON profile with atomic writes.
///
/// Deliberately dependency-free and synchronous: the profile is tiny and the
/// engine owns it from a single thread.
#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `~/.config/omniscreenveil/profile.json` on Linux,
    /// `%APPDATA%\omniscreenveil\profile.json` on Windows,
    /// `~/Library/Application Support/omniscreenveil/profile.json` on macOS.
    pub fn default_path() -> PathBuf {
        let base = if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join("Library/Application Support"))
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
                .unwrap_or_else(|| PathBuf::from("."))
        };
        base.join("omniscreenveil").join("profile.json")
    }

    pub fn load(&self) -> Result<Profile, ConfigError> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => {
                let mut profile: Profile =
                    serde_json::from_str(&text).map_err(|source| ConfigError::Parse {
                        path: self.path.clone(),
                        source,
                    })?;
                profile.normalize();
                Ok(profile)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Profile::new()),
            Err(source) => Err(ConfigError::Read {
                path: self.path.clone(),
                source,
            }),
        }
    }

    pub fn save(&self, profile: &Profile) -> Result<(), ConfigError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let json = serde_json::to_string_pretty(profile)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|source| ConfigError::Write {
            path: tmp.clone(),
            source,
        })?;
        std::fs::rename(&tmp, &self.path).map_err(|source| ConfigError::Write {
            path: self.path.clone(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::ProtectionMode;
    use crate::ids::AppKey;

    fn temp_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "veil-config-test-{}-{}-{}",
            std::process::id(),
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        dir.join("profile.json")
    }

    #[test]
    fn missing_profile_is_default() {
        let store = ConfigStore::new(temp_path("missing"));
        let profile = store.load().unwrap();
        assert_eq!(profile, Profile::new());
    }

    #[test]
    fn round_trip_preserves_rules() {
        let path = temp_path("roundtrip");
        let store = ConfigStore::new(path);
        let mut profile = Profile::new();
        profile.add_app_rule(AppKey::LinuxAppId("chrome".into()), ProtectionMode::Surface);
        profile.auto_arm = crate::rules::AutoArm::CaptureAppRunning;
        store.save(&profile).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.rules.len(), 1);
        assert_eq!(loaded.rules[0].mode, ProtectionMode::Surface);
        assert_eq!(loaded.auto_arm, crate::rules::AutoArm::CaptureAppRunning);
    }

    #[test]
    fn corrupt_profile_is_an_error_not_a_panic() {
        let path = temp_path("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        let err = ConfigStore::new(path).load().unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn default_path_is_absolute() {
        let path = ConfigStore::default_path();
        assert!(path.ends_with("omniscreenveil/profile.json"));
    }
}
